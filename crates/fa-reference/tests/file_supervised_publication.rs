//! Drive real socket reviews and original keys through first-publication checks.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::publication::{CheckedPublication, PublicationBasis};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::Error;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn sent(guard: bool) -> Rig {
    let mut rig = Rig::new();
    if guard {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_publication_guard(revision).unwrap();
    }
    let _ticket = rig.submit(1);
    rig.reviewed(1);
    let key = rig.human(1001, 31);
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { request: 1, attempt: 1 }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    rig
}
fn checked(event: FileDriverEvent) -> (CheckedPublication, Option<Error>) {
    match event {
        FileDriverEvent::PublicationChecked { request: 1, publication, source_failure } => (publication, source_failure),
        event => panic!("unexpected publication event: {event:?}"),
    }
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
fn settle(rig: &mut Rig, expected: EndpointOutcome) {
    let now = rig.driver.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
    let event = rig.driver.step_with_evidence(|| now, |_, _| panic!("settlement must not read evidence"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { request: 1, outcome: Reconciliation::Resolved(outcome) } if outcome == expected));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    let host = rig.driver.supervisor().host().unwrap();
    let ledger = host.inspect().control.ledger;
    assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
    assert_eq!(ledger.reserved, 0);
}

#[test]
fn guarded_driver_publishes_after_a_new_capture_and_only_then_acknowledges() {
    let mut rig = sent(true);
    let inputs = rig.inputs.clone();
    let mut calls = 0;
    let event = rig.driver.step_with_evidence(|| ElapsedTick(2), |_, _| {
        calls += 1;
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, None).unwrap();
    assert_eq!(calls, 1);
    let (publication, failure) = checked(event);
    assert_eq!(failure, None);
    assert_eq!(publication.basis, PublicationBasis::Revalidated);
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    {
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().payload, b"publication");
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Dispatching);
        assert_eq!(host.inspect().control.ledger.charged, 16);
    }
    settle(&mut rig, publication.outcome);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn last_moment_drift_seals_without_refund_or_a_better_evidence_retry() {
    for mode in 0..6 {
        let mut rig = sent(true);
        let original = rig.inputs.clone();
        let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let changed = helper::inputs(&action, b"changed complete provider input");
        let mut calls = 0;
        let event = rig.driver.step_with_evidence(|| ElapsedTick(2), |_, _| {
            calls += 1;
            let mut state = snapshot();
            let mut inputs = original.clone();
            match mode {
                0 => { state.values.insert(7, b"changed".to_vec()); }
                1 => state.semantic_epoch += 1,
                2 => state.complete = false,
                3 => inputs = Some(changed.clone()),
                4 => inputs = None,
                _ => return Err(Error::Binding),
            }
            Ok(DriverEvidence { snapshot: state, inputs })
        }, None).unwrap();
        assert_eq!(calls, 1);
        let (publication, failure) = checked(event);
        assert!(matches!(publication.basis, PublicationBasis::Rejected(_)));
        assert_eq!(publication.outcome, sealed());
        assert_eq!(failure, match mode { 2 | 4 => Some(Error::Incomplete), 5 => Some(Error::Binding), _ => None });
        {
            let host = rig.driver.supervisor().host().unwrap();
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().payload, b"initial");
            assert_eq!(host.inspect().control.ledger.charged, 16);
        }
        // The original endpoint is terminal even if a caller separately supplies
        // the formerly valid basis. No driver or endpoint retry may execute it.
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            let retry = host.publish_checked(revision, 1, original.as_ref(), snapshot(), ElapsedTick(2)).unwrap();
            assert_eq!(retry.basis, PublicationBasis::PreviouslyResolved);
            assert_eq!(retry.outcome, sealed());
        }
        settle(&mut rig, sealed());
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn unrelated_policy_state_does_not_disable_a_valid_guarded_publication() {
    let mut rig = sent(true);
    let inputs = rig.inputs.clone();
    let event = rig.driver.step_with_evidence(|| ElapsedTick(2), |_, _| {
        let mut state = snapshot();
        state.values.insert(99, b"unrelated row".to_vec());
        Ok(DriverEvidence { snapshot: state, inputs: inputs.clone() })
    }, None).unwrap();
    let (publication, failure) = checked(event);
    assert_eq!(publication.basis, PublicationBasis::Revalidated);
    assert_eq!(failure, None);
    settle(&mut rig, publication.outcome);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn provider_duration_cannot_extend_the_original_human_execution_deadline() {
    for already_expired in [false, true] {
        let mut rig = sent(true);
        let inputs = rig.inputs.clone();
        let tick = Cell::new(if already_expired { 31 } else { 1 });
        let mut calls = 0;
        let event = rig.driver.step_with_evidence(|| ElapsedTick(tick.get()), |_, _| {
            calls += 1;
            tick.set(31);
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, None).unwrap();
        assert_eq!(calls, usize::from(!already_expired));
        let (publication, failure) = checked(event);
        assert_eq!(failure, None);
        assert_eq!(publication.basis, PublicationBasis::DeadlineElapsed);
        assert_eq!(publication.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
        settle(&mut rig, publication.outcome);
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn existing_endpoint_outcomes_win_without_contacting_a_provider() {
    for executed in [false, true] {
        let mut rig = sent(true);
        let inputs = rig.inputs.clone();
        let prior = {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            host.publish_checked(revision, 1, if executed { inputs.as_ref() } else { None }, snapshot(), ElapsedTick(1)).unwrap()
        };
        let event = rig.driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("historical outcome must not depend on a provider"), None).unwrap();
        let (publication, failure) = checked(event);
        assert_eq!(publication.outcome, prior.outcome);
        assert_eq!(publication.basis, PublicationBasis::PreviouslyResolved);
        assert_eq!(failure, None);
        settle(&mut rig, prior.outcome);
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, u64::from(executed));
    }
}

#[test]
fn caught_provider_unwind_leaves_only_query_and_retains_the_unknown_charge() {
    let mut rig = sent(true);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("provider failure"), None);
    }));
    assert!(result.is_err());
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("never rerun the provider or publication"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let revision = host.revision();
    assert_eq!(host.seal_unexecuted(revision, 1).unwrap(), Reconciliation::Resolved(sealed()));
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn rejected_post_capture_clock_does_not_create_a_retryable_send() {
    let mut rig = sent(true);
    let inputs = rig.inputs.clone();
    let tick = Cell::new(1);
    let event = rig.driver.step_with_evidence(|| ElapsedTick(tick.get()), |_, _| {
        tick.set(0);
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, None).unwrap();
    assert!(matches!(event, FileDriverEvent::PublicationUnknown { error: JournalError::Contract(Error::Stale), .. }));
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("rejected publication cannot resend"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn guarded_storage_failure_preserves_original_recovery_instead_of_retrying() {
    let mut rig = sent(true);
    let inputs = rig.inputs.clone();
    let store = rig.root.store();
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| {
        std::fs::write(store.join("delivery.pending"), b"occupied stage").unwrap();
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, None).unwrap();
    assert!(matches!(event, FileDriverEvent::PublicationUnknown { error: JournalError::Io(_), .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(FileOversight::read_publication(&store, &profile()).unwrap().executions, 0);
    assert!(matches!(rig.driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("faulted owner must not contact sources"), None), Err(JournalError::Unavailable)));
    drop(rig.driver.release());
    drop(rig.port);
    let (mut host, _) = FileOversight::open(&store, profile()).unwrap();
    assert!(host.publication_guard_required());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.seal_unexecuted(host.revision(), 1).unwrap(), Reconciliation::Resolved(sealed()));
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn legacy_owner_remains_explicitly_unguarded_without_an_extra_provider_read() {
    let mut rig = sent(false);
    let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("legacy publication has no evidence callback"), None).unwrap();
    let FileDriverEvent::Published { outcome, .. } = event else { panic!("legacy result shape changed"); };
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    settle(&mut rig, outcome);
    assert!(!rig.driver.supervisor().host().unwrap().publication_guard_required());
}
