//! Recover the original durable owner; never recover a sendable driver job.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::action::ElapsedTick;
use fa_reference::Error;
use std::cell::Cell;

fn request(rig: &Rig, operation: u64) -> StopRequest {
    let control = rig.driver.supervisor().host().unwrap().inspect().control;
    StopRequest { operation, expected_control_sequence: control.sequence, expected_authority_epoch: control.ledger.epoch }
}

#[test]
fn reopened_driver_recovers_only_outcomes_and_fresh_work_needs_a_new_request_and_both_keys() {
    for boundary in ["review", "reserved", "dispatched", "published"] {
        let mut rig = Rig::new(); let original = rig.proposal();
        let old_ticket = rig.submit(1); rig.reviewed(1);
        let old_key = rig.human(1001, 30);
        match boundary {
            "reserved" => {
                let ticks = Cell::new(0); let inputs = rig.inputs.clone();
                assert!(rig.driver.step_with_evidence(|| {
                    ticks.set(ticks.get() + 1); ElapsedTick(if ticks.get() < 3 { 1 } else { 0 })
                }, |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), Some(&old_key)).is_err());
                assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
            }
            "dispatched" | "published" => {
                assert!(matches!(rig.step(Some(&old_key)), FileDriverEvent::Dispatched { .. }));
                if boundary == "published" { assert!(matches!(rig.step(None), FileDriverEvent::Published { .. })); }
            }
            _ => {}
        }
        drop(rig.driver);
        assert!(matches!(rig.port.poll(&old_ticket), Knowledge::Unknown { .. }));
        let (host, reviewer) = FileOversight::open(rig.root.store(), profile()).unwrap();
        let revision = host.revision();
        let (port, mut driver) = host.into_supervised_driver();
        let ticket = port.submit(1, &original).unwrap();
        assert_eq!(driver.supervisor().host().unwrap().revision(), revision);
        let sent = matches!(boundary, "dispatched" | "published");
        if sent {
            assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
            driver.resume_reconciliation(1).unwrap();
            assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
            let result = driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("recovery requested helper data"), None).unwrap();
            if boundary == "published" {
                assert!(matches!(result, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. }));
                assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
            } else {
                assert!(matches!(result, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
                assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                host.seal_unexecuted(revision, 1).unwrap();
            }
        } else {
            assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
            assert!(driver.resume_reconciliation(1).is_err());
            assert_eq!(driver.phase(), FileDriverPhase::Idle);
            assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
        }
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, u64::from(boundary == "published"));
        assert_eq!(driver.supervisor().host().unwrap().retained_requests(), 1);
        rig.driver = driver; rig.port = port; rig.reviewer = reviewer; rig.clients.clear(); rig.inputs = None;
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            host.observe_time(revision, ElapsedTick(3)).unwrap();
        }
        rig.submit(2); rig.reviewed(2);
        let fresh_key = rig.human(1002, 30);
        let inputs = rig.inputs.clone();
        assert_eq!(rig.driver.step_with_evidence(|| ElapsedTick(3), |_, _| Ok(DriverEvidence {
            snapshot: snapshot(), inputs: inputs.clone(),
        }), Some(&old_key)).unwrap_err(), JournalError::Contract(Error::Binding));
        assert!(matches!(rig.step(Some(&fresh_key)), FileDriverEvent::Dispatched { .. }));
        assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1 + u64::from(boundary == "published"));
    }
}

#[test]
fn a_publication_write_failure_reopens_as_query_only_without_a_resend_or_inferred_refund() {
    let mut rig = Rig::new(); let original = rig.proposal(); rig.submit(1); rig.reviewed(1);
    let key = rig.human(1001, 30);
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
    std::fs::write(rig.root.store().join("delivery.pending"), b"blocked staging slot").unwrap();
    assert!(matches!(rig.step(None), FileDriverEvent::PublicationUnknown { error: JournalError::Io(_), .. }));
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    assert_eq!(rig.driver.step_with_evidence(|| panic!("faulted clock"), |_, _| panic!("faulted source"), None).unwrap_err(), JournalError::Unavailable);
    let acknowledged = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(acknowledged.executions, 0);
    assert_eq!(acknowledged.control.ledger.charged, 16);
    drop(rig.driver);
    let (host, _) = FileOversight::open(rig.root.store(), profile()).unwrap();
    let (port, mut driver) = host.into_supervised_driver();
    let ticket = port.submit(1, &original).unwrap();
    driver.resume_reconciliation(1).unwrap();
    assert!(matches!(driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("source during recovery"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    let state = driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.charged, 16);
    assert_eq!(driver.supervisor().host().unwrap().retained_requests(), 1);
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        host.seal_unexecuted(revision, 1).unwrap();
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn a_durable_fence_between_dispatch_and_publication_switches_the_driver_to_reconciliation() {
    let mut rig = Rig::new(); rig.submit(1); rig.reviewed(1);
    let key = rig.human(1001, 30); rig.step(Some(&key));
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        host.fence(revision).unwrap();
    }
    let event = rig.driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("fenced effect requested new evidence"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    let state = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.charged, 16);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
}

#[test]
fn cancellation_releases_only_undispatched_work_and_never_publishes_a_cancelled_local_job() {
    for boundary in ["reviewed", "dispatched", "published"] {
        let mut rig = Rig::new(); let ticket = rig.submit(1); rig.reviewed(1);
        let key = rig.human(1001, 30);
        if boundary != "reviewed" { rig.step(Some(&key)); }
        if boundary == "published" { rig.step(None); }
        rig.driver.cancel_active().unwrap();
        if boundary == "reviewed" {
            assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
            assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
        } else {
            assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
            let event = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("cancel recovery source"), None).unwrap();
            if boundary == "published" {
                assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. }));
            } else { assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. })); }
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
        }
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, u64::from(boundary == "published"));
    }
}

#[test]
fn invalid_stop_preserves_a_review_but_committed_stop_does_not_wait_for_evidence_or_helpers() {
    let mut rig = Rig::new(); let ticket = rig.submit(1); rig.start(1, 101);
    let stop = request(&rig, 20);
    assert_eq!(rig.driver.request_stop(StopRequest { expected_control_sequence: stop.expected_control_sequence + 1, ..stop }).unwrap_err(),
        JournalError::Contract(Error::Stale));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Reviewing { request: 1 });
    let receipt = rig.driver.request_stop(stop).unwrap();
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert!(rig.driver.progress_stop(ElapsedTick(0)).is_err());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().stop, Some(receipt));
    assert!(rig.driver.progress_stop(ElapsedTick(1)).unwrap().progress.drained());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn endpoint_expiry_is_not_a_refund_until_the_original_receipt_is_reconciled() {
    let mut rig = Rig::new(); let ticket = rig.submit(1); rig.reviewed(1);
    let key = rig.human(1001, 30); rig.step(Some(&key));
    let event = rig.driver.step_with_evidence(|| ElapsedTick(30), |_, _| panic!("publication source"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Published { outcome: EndpointOutcome::NotExecuted {
        reason: NonExecutionReason::DeadlineElapsed }, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    let event = rig.driver.step_with_evidence(|| ElapsedTick(30), |_, _| panic!("acknowledgment source"), None).unwrap();
    assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::NotExecuted { .. }), .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
}

#[test]
fn a_replaced_host_with_identical_numeric_request_ids_cannot_receive_the_original_jobs_publication() {
    let mut rig = Rig::new(); rig.submit(1); rig.reviewed(1);
    let key = rig.human(1001, 30); rig.step(Some(&key));
    let other_root = Directory::new();
    let (mut other, reviewer) = helper::create(&other_root);
    let spec = helper::spec(&other, b"foreign effect");
    other.submit_request(other.revision(), 1, spec, snapshot()).unwrap();
    let action = other.request_action(1).unwrap().clone();
    let input = helper::inputs(&action, b"foreign complete evidence");
    helper::oversight::review_existing(&mut other, 1, 101, &input);
    let automatic = other.authorize(other.revision(), 1, &input, snapshot()).unwrap();
    let request = other.request_human_approval(other.revision(), 1001, 1, &input, ElapsedTick(30)).unwrap();
    let revision = other.revision();
    let human = reviewer.approve(&mut other, revision, &request).unwrap();
    other.dispatch(other.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    let original = {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        std::mem::replace(&mut *host, other)
    };
    assert_eq!(rig.driver.step_with_evidence(|| panic!("foreign owner clock"), |_, _| panic!("foreign owner source"), None).unwrap_err(),
        JournalError::Contract(Error::Binding));
    assert_eq!(rig.driver.cancel_active().unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(original.inspect().executions, 0);
    let other = {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        std::mem::replace(&mut *host, original)
    };
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().payload, b"publication");
    assert_eq!(other.inspect().executions, 0);
}
