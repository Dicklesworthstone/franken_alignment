#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, governance::PolicyUpdate};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::round::Verdict;
use fa_reference::Error;

fn command(rig: &Rig, id: u64, generation: u64, limit: usize) -> PolicyUpdate {
    let state = rig.driver.supervisor().host().unwrap().inspect().control;
    PolicyUpdate::new(id, state.sequence, state.ledger.epoch,
        Policy::new(generation, vec![Predicate::PayloadAtMost(limit)]).unwrap()).unwrap()
}
fn revision(rig: &Rig) -> u64 { rig.driver.supervisor().host().unwrap().revision() }

#[test]
fn interrupted_review_closes_before_clock_or_provider_then_new_epoch_work_can_publish() {
    let mut rig = Rig::new();
    let original = rig.proposal();
    let ticket = rig.submit(1); rig.start(1, 101);
    let update = command(&rig, 1, 2, 128);
    let rev = revision(&rig);
    let receipt = rig.driver.replace_policy(rev, &update).unwrap();
    assert_eq!(receipt.change().cancelled, vec![1]);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    let stopped = rig.driver.step_with_evidence(|| panic!("cancelled work requested a clock"),
        |_, _| panic!("cancelled work requested evidence"), None).unwrap();
    assert!(matches!(stopped, FileDriverEvent::Stopped { request: 1, stage: ActionState::Cancelled }));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert!(rig.port.submit(1, &original).is_ok());
    let ticket = rig.submit(2); rig.reviewed(2);
    let human = rig.human(1002, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn retained_automatic_reservation_and_real_human_grant_are_withdrawn_once() {
    let mut rig = Rig::new(); rig.submit(1); rig.reviewed(1);
    let rightful = rig.human(1001, 31);
    let mut foreign = Rig::new(); foreign.submit(1); foreign.reviewed(1);
    let wrong = foreign.human(1001, 31);
    let input = rig.inputs.clone();
    assert_eq!(rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _|
        Ok(DriverEvidence { snapshot: snapshot(), inputs: input.clone() }), Some(&wrong)).unwrap_err(),
        JournalError::Contract(Error::Binding));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
    let update = command(&rig, 2, 2, 128); let rev = revision(&rig);
    let receipt = rig.driver.replace_policy(rev, &update).unwrap();
    assert_eq!(receipt.change().refunded_units, 16);
    assert_eq!(rig.driver.supervisor().host().unwrap().human_status(rightful.request()).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    let rev = revision(&rig);
    assert_eq!(rig.driver.replace_policy(0, &update).unwrap(), receipt);
    assert_eq!(revision(&rig), rev);
    assert!(matches!(rig.step(Some(&rightful)), FileDriverEvent::Stopped { .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn committed_dispatch_keeps_its_publication_phase_after_policy_replacement() {
    let mut rig = Rig::new(); let ticket = rig.submit(1); rig.reviewed(1);
    let human = rig.human(1001, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    let update = command(&rig, 3, 2, 0); let rev = revision(&rig);
    let receipt = rig.driver.replace_policy(rev, &update).unwrap();
    assert_eq!(receipt.change().refunded_units, 0);
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingPublication { request: 1 });
    assert!(matches!(rig.driver.step_with_evidence(|| ElapsedTick(1),
        |_, _| panic!("admitted publication must not rerun its review"), None).unwrap(), FileDriverEvent::Published { .. }));
    assert!(matches!(rig.driver.step_with_evidence(|| ElapsedTick(1),
        |_, _| panic!("reconciliation must not reacquire evidence"), None).unwrap(), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn refused_update_preserves_the_existing_socket_review_and_original_publication_path() {
    let mut rig = Rig::new(); rig.submit(1); rig.start(1, 101);
    let valid = command(&rig, 4, 2, 128);
    let stale = PolicyUpdate::new(4, valid.expected_control_sequence() + 1,
        valid.expected_authority_epoch(), valid.policy().clone()).unwrap();
    let rev = revision(&rig);
    assert_eq!(rig.driver.replace_policy(rev, &stale), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(revision(&rig), rev);
    assert!(matches!(rig.review_event(Verdict::Allow, false), FileDriverEvent::ReviewApplied { .. }));
    let human = rig.human(1001, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn exact_historical_retry_cannot_cancel_a_newer_review_by_reusing_old_receipt_contents() {
    let mut rig = Rig::new(); rig.submit(1); rig.start(1, 101);
    let update = command(&rig, 5, 2, 128); let rev = revision(&rig);
    let receipt = rig.driver.replace_policy(rev, &update).unwrap();
    assert_eq!(receipt.change().cancelled, vec![1]);
    assert!(matches!(rig.step(None), FileDriverEvent::Stopped { .. }));
    rig.submit(2); rig.start(2, 102);
    let rev = revision(&rig);
    assert_eq!(rig.driver.replace_policy(0, &update).unwrap(), receipt);
    assert_eq!(revision(&rig), rev);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Reviewing { request: 2 });
    assert!(matches!(rig.review_event(Verdict::Allow, false), FileDriverEvent::ReviewApplied { .. }));
    let human = rig.human(1002, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}
