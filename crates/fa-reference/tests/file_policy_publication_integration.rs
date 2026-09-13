//! Policy replacement across the already integrated guarded publication path.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, governance::PolicyUpdate};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::Snapshot;

fn guarded() -> Rig {
    let mut rig = Rig::new();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision(); host.enable_publication_guard(revision).unwrap();
    }
    rig
}
fn update(rig: &Rig, operation: u64, limit: usize) -> (u64, PolicyUpdate) {
    let host = rig.driver.supervisor().host().unwrap(); let control = host.inspect().control;
    (host.revision(), PolicyUpdate::new(operation, control.sequence, control.ledger.epoch,
        Policy::new(host.current_policy().unwrap().generation() + 1,
            vec![Predicate::PayloadAtMost(limit)]).unwrap()).unwrap())
}

#[test]
fn new_policy_rejects_first_execution_but_preserves_already_executed_history() {
    for executed in [false, true] {
        let mut rig = guarded(); rig.submit(1); rig.reviewed(1);
        let human = rig.human(1001, 31);
        assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
        if executed {
            assert!(matches!(rig.step(None), FileDriverEvent::PublicationChecked { publication, .. }
                if publication.basis == PublicationBasis::Revalidated
                    && matches!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 })));
        }
        let (revision, command) = update(&rig, 70, 0);
        let receipt = rig.driver.replace_policy(revision, &command).unwrap();
        assert_eq!(receipt.change().refunded_units, 0);
        assert!(receipt.change().cancelled.is_empty());
        {
            let host = rig.driver.supervisor().host().unwrap();
            assert!(host.publication_guard_required());
            assert_eq!(host.human_status(human.request()).unwrap().disposition, HumanDisposition::Consumed);
            assert_eq!(host.inspect().control.ledger.charged, 16);
        }
        let expected = if executed { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } };
        if executed {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            let prior = host.publish_checked(revision, 1, None, Snapshot::default(), ElapsedTick(1)).unwrap();
            assert_eq!(prior.basis, PublicationBasis::PreviouslyResolved);
            assert_eq!(prior.outcome, expected);
        } else {
            assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingPublication { request: 1 });
            let inputs = rig.inputs.clone(); let mut calls = 0;
            let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| {
                calls += 1; Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
            }, None).unwrap();
            assert_eq!(calls, 1);
            assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, source_failure: None, .. }
                if matches!(publication.basis, PublicationBasis::Rejected(_)) && publication.outcome == expected));
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
        }
        let event = rig.driver.step_with_evidence(|| ElapsedTick(1),
            |_, _| panic!("policy repair caused evidence acquisition during reconciliation"), None).unwrap();
        assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(value), .. } if value == expected));
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(executed));
        assert_eq!(host.inspect().control.ledger.charged, if executed { 16 } else { 0 });
        assert_eq!(host.inspect().control.ledger.available, if executed { 84 } else { 100 });
    }
}

#[test]
fn recovering_an_old_update_receipt_does_not_invalidate_fresh_guarded_publication() {
    let mut rig = guarded(); rig.submit(1); rig.start(1, 101);
    let (revision, command) = update(&rig, 80, 128);
    let receipt = rig.driver.replace_policy(revision, &command).unwrap();
    assert_eq!(receipt.change().cancelled, vec![1]);
    assert!(matches!(rig.step(None), FileDriverEvent::Stopped { .. }));
    rig.submit(2); rig.reviewed(2);
    let human = rig.human(1002, 31);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { request: 2, .. }));
    let before = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(rig.driver.replace_policy(0, &command).unwrap(), receipt);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert!(matches!(rig.step(None), FileDriverEvent::PublicationChecked { request: 2, publication, source_failure: None }
        if publication.basis == PublicationBasis::Revalidated
            && matches!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 })));
    let event = rig.driver.step_with_evidence(|| ElapsedTick(1),
        |_, _| panic!("acknowledgment requested new helper data"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { request: 2,
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }) }));
    let host = rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.current_policy().unwrap(), command.policy());
    assert_eq!(host.inspect().control.ledger.charged, 16);
}
