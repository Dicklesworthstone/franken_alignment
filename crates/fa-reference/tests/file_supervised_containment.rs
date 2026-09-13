//! Native actor reset joins the original durable driver and actor observations.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::containment::*;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::action::consequence::gate::containment::ActorState;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::round::Verdict;
use fa_reference::Error;

fn checkpoint(rig: &mut Rig) -> FileCheckpoint {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let state = host.actor_snapshot().unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    host.capture_actor_checkpoint(revision, 77, state.actor_revision, epoch).unwrap()
}
fn changed(rig: &mut Rig) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let state = host.actor_snapshot().unwrap();
    let update = FileStateUpdate { operation: 1, expected_actor_revision: state.actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        state: ActorState::new(state.state.profile(), vec![8, 9], vec![0, 255], vec![128, 1], 2).unwrap() };
    let revision = host.revision(); host.record_actor_state(revision, update).unwrap();
}
fn request(rig: &Rig) -> FileResetRequest {
    let host = rig.driver.supervisor().host().unwrap();
    FileResetRequest { operation: 1, expected_control_sequence: host.inspect().control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        binding: ReviewBinding { round: 10001, evidence_root: ROOT, reducer_generation: 1 },
        retained_targets: vec![host.inspect().target] }
}
fn stopped(rig: &mut Rig) {
    assert!(matches!(rig.driver.step_with_evidence(|| panic!("cancelled job must not read the clock"),
        |_, _| panic!("cancelled job must not read a source"), None).unwrap(),
        FileDriverEvent::Stopped { request: 1, stage: ActionState::Cancelled }));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
}
fn publish(rig: &mut Rig, request: u64) {
    rig.reviewed(request); let human = rig.human(1000 + request, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. }));
}

#[test]
fn reset_retires_abandoned_socket_review_but_old_retry_does_not_stop_new_work() {
    let mut rig = Rig::new(); let saved = checkpoint(&mut rig); changed(&mut rig);
    let ticket = rig.submit(1); rig.start(1, 101); let request = request(&rig);
    let receipt = rig.driver.reset_actor(&saved, request.clone()).unwrap();
    assert_eq!(receipt.incident_count, 1);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    stopped(&mut rig);
    let _fresh = rig.submit(2); rig.start(2, 102);
    let before = rig.driver.supervisor().host().unwrap().revision();
    assert_eq!(rig.driver.reset_actor(&saved, request).unwrap(), receipt);
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), before);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Reviewing { request: 2 });
    assert!(matches!(rig.review_event(Verdict::Allow, false), FileDriverEvent::ReviewApplied { .. }));
    let human = rig.human(1002, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { request: 2, .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    rig.step(None);
    assert_eq!(rig.driver.supervisor().host().unwrap().actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn rejected_reset_leaves_original_review_and_both_key_publication_usable() {
    let mut rig = Rig::new(); let saved = checkpoint(&mut rig);
    let _ticket = rig.submit(1); rig.start(1, 101);
    let mut bad = request(&rig); bad.expected_authority_epoch += 1;
    let before = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(rig.driver.reset_actor(&saved, bad), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert!(matches!(rig.review_event(Verdict::Allow, false), FileDriverEvent::ReviewApplied { .. }));
    let human = rig.human(1001, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    rig.step(None);
    assert_eq!(rig.driver.supervisor().host().unwrap().actor_snapshot().unwrap().incident_count, 0);
}

#[test]
fn reset_cancels_an_actual_retained_reservation_and_does_not_reuse_its_human_key() {
    let mut rig = Rig::new(); let saved = checkpoint(&mut rig);
    let ticket = rig.submit(1); rig.reviewed(1); let human = rig.human(1001, 31);
    let inputs = rig.inputs.clone(); let mut calls = 0;
    assert_eq!(rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| {
        calls += 1;
        if calls == 2 { Err(Error::Incomplete) }
        else { Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }) }
    }, Some(&human)).unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
    let request = request(&rig); let receipt = rig.driver.reset_actor(&saved, request).unwrap();
    assert_eq!(receipt.refunded_units, 16);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    stopped(&mut rig);
    let _fresh = rig.submit(2); publish(&mut rig, 2);
    let ledger = rig.driver.supervisor().host().unwrap().inspect().control.ledger;
    assert_eq!(ledger.charged, 16); assert_eq!(ledger.available, 84); assert_eq!(ledger.reserved, 0);
}

#[test]
fn reset_after_dispatch_preserves_ordered_effect_or_original_guarded_sealing_until_ack() {
    for guard in [false, true] {
        let mut rig = Rig::new();
        if guard {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.enable_publication_guard(revision).unwrap();
        }
        let saved = checkpoint(&mut rig); let ticket = rig.submit(1); rig.reviewed(1);
        let human = rig.human(1001, 31); rig.step(Some(&human));
        let request = request(&rig); let receipt = rig.driver.reset_actor(&saved, request).unwrap();
        assert_eq!(receipt.refunded_units, 0);
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
        let outcome = match rig.step(None) {
            FileDriverEvent::Published { outcome, .. } if !guard => outcome,
            FileDriverEvent::PublicationChecked { publication, .. } if guard => publication.outcome,
            event => panic!("wrong original execution path: {event:?}"),
        };
        assert_eq!(outcome, if guard { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
            else { EndpointOutcome::Executed { resulting_version: 2 } });
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
        assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(result), .. } if result == outcome));
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, if guard { 0 } else { 16 });
    }
}

#[test]
fn policy_changed_since_checkpoint_does_not_revert_when_actor_memory_is_restored() {
    use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
    use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    let mut rig = Rig::new(); let saved = checkpoint(&mut rig); changed(&mut rig);
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let state = host.inspect();
        let update = PolicyUpdate::new(1, state.control.sequence, state.control.ledger.epoch,
            Policy::new(2, vec![Predicate::UnitsAtMost(16)]).unwrap()).unwrap();
        let revision = host.revision(); host.replace_policy(revision, &update).unwrap();
    }
    let request = request(&rig); rig.driver.reset_actor(&saved, request).unwrap();
    assert_eq!(rig.driver.supervisor().host().unwrap().current_policy().unwrap().generation(), 2);
    let _ticket = rig.submit(1); publish(&mut rig, 1);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}
