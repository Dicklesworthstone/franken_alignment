//! Durable actor identity, original review/permit transitions, and real files.
#![cfg(unix)]
#[path = "support/file_delivery.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick, MAX_PAYLOAD_BYTES};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::delivery::persistent::requests::*;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};

fn attempt(status: FileRequestStatus) -> u64 {
    match status.disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("admission failed: {other:?}"),
    }
}
fn stage(host: &FileDelivery, key: u64) -> ActionState {
    match host.request_status(key).unwrap().disposition {
        FileRequestDisposition::Admitted { stage, .. } => stage,
        other => panic!("admission failed: {other:?}"),
    }
}
fn submitted(host: &mut FileDelivery, key: u64) -> u64 {
    let spec = spec(host, b"payload");
    attempt(host.submit_request(host.revision(), key, spec, snapshot()).unwrap())
}
fn permit(host: &mut FileDelivery, key: u64) -> FilePermit {
    let id = attempt(host.request_status(key).unwrap());
    host.review(host.revision(), review(id, id + 100, Verdict::Allow)).unwrap();
    host.authorize(host.revision(), id, snapshot()).unwrap()
}

#[test]
fn publication_and_request_identity_survive_reopen_without_reproposal_or_resend() {
    let root = Directory::new(); let mut host = create(&root);
    let submitted_spec = spec(&host, b"payload");
    let original = host.submit_request(host.revision(), 99, submitted_spec.clone(), snapshot()).unwrap();
    let id = attempt(original); assert_ne!(id, 99);
    let key = permit(&mut host, 99);
    assert_eq!(host.request_status(99).unwrap().generation, original.generation);
    let action = host.request_action(99).unwrap().clone();
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    host.publish(host.revision(), id).unwrap();
    assert_eq!(stage(&host, 99), ActionState::Dispatching);
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    let revision = host.revision();
    let recovered = host.submit_request(0, 99, submitted_spec.clone(), Snapshot::default()).unwrap();
    assert_eq!(stage(&host, 99), ActionState::Unknown);
    assert_eq!(host.revision(), revision); assert_eq!(attempt(recovered), id);
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.publish(host.revision(), id).is_err());
    assert!(matches!(host.reconcile(host.revision(), id).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
    assert_eq!(stage(&host, 99), ActionState::Confirmed);
    let settled = host.request_status(99).unwrap(); let revision = host.revision();
    assert_eq!(host.submit_request(0, 99, submitted_spec, Snapshot::default()).unwrap(), settled);
    assert_eq!(host.revision(), revision); assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn recovery_cancels_a_reservation_but_exact_retry_cannot_recreate_it() {
    let root = Directory::new(); let mut host = create(&root);
    let original = spec(&host, b"payload"); submitted(&mut host, 8);
    let key = permit(&mut host, 8); let action = host.request_action(8).unwrap().clone();
    assert_eq!(host.inspect().control.ledger.reserved, 16); drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap(); let revision = host.revision();
    assert_eq!(stage(&host, 8), ActionState::Cancelled);
    let retained = host.submit_request(0, 8, original, Snapshot::default()).unwrap();
    assert_eq!(retained.disposition, FileRequestDisposition::Admitted { attempt: key.attempt(), stage: ActionState::Cancelled });
    assert_eq!(host.revision(), revision); assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(host.dispatch(host.revision(), &key, &action, snapshot()).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    submitted(&mut host, 9); let next = permit(&mut host, 9);
    let action = host.request_action(9).unwrap().clone();
    host.dispatch(host.revision(), &next, &action, snapshot()).unwrap();
    host.publish(host.revision(), next.attempt()).unwrap();
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn a_durable_refusal_is_not_retried_under_better_evidence_or_another_epoch() {
    let root = Directory::new(); let mut host = create(&root);
    let mut bad = spec(&host, b"payload"); bad.target.as_mut().unwrap().object += 1;
    let refused = host.submit_request(host.revision(), 20, bad.clone(), snapshot()).unwrap();
    assert_eq!(refused.disposition, FileRequestDisposition::NotAdmitted(Error::Binding));
    assert!(host.request_action(20).is_err()); drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap(); let revision = host.revision();
    assert_eq!(host.submit_request(0, 20, bad.clone(), Snapshot::default()).unwrap(), refused);
    assert_eq!(host.revision(), revision);
    let good = spec(&host, b"payload");
    assert_eq!(host.submit_request(revision, 20, good.clone(), snapshot()), Err(JournalError::Contract(Error::Binding)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let fresh = host.submit_request(host.revision(), 21, good, snapshot()).unwrap();
    assert_eq!(attempt(fresh), 2); assert_eq!(host.retained_requests(), 2);
}

#[test]
fn every_execution_field_binds_idempotence_and_conflicts_never_touch_the_ledger() {
    let root = Directory::new(); let mut host = create(&root);
    let original = spec(&host, b"payload");
    host.submit_request(host.revision(), 1, original.clone(), snapshot()).unwrap();
    let before = host.inspect();
    for field in 0..8 {
        let mut changed = original.clone();
        match field {
            0 => changed.payload.push(0), 1 => changed.units += 1,
            2 => changed.deadline.0 += 1, 3 => changed.policy_epoch += 1,
            4 => changed.target.as_mut().unwrap().expected_version += 1,
            5 => changed.target.as_mut().unwrap().generation += 1,
            6 => changed.scope.principal += 1, _ => changed.version += 1,
        }
        assert_eq!(host.submit_request(0, 1, changed, snapshot()), Err(JournalError::Contract(Error::Binding)));
        assert_eq!(host.inspect(), before);
    }
    host.submit_request(0, 1, original, Snapshot::default()).unwrap();
    assert_eq!(host.inspect(), before);
}

#[test]
fn request_cancellation_distinguishes_reserved_from_sent_work() {
    for sent in [false, true] {
        let root = Directory::new(); let mut host = create(&root);
        let id = submitted(&mut host, 1); let key = permit(&mut host, 1);
        if sent {
            let action = host.request_action(1).unwrap().clone();
            host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
        }
        host.cancel_request(host.revision(), 1).unwrap();
        let revision = host.revision(); host.cancel_request(0, 1).unwrap();
        assert_eq!(host.revision(), revision);
        if sent {
            assert_eq!(stage(&host, 1), ActionState::Dispatching);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            host.seal_unexecuted(host.revision(), id).unwrap();
            assert_eq!(stage(&host, 1), ActionState::ConfirmedNotExecuted);
        } else { assert_eq!(stage(&host, 1), ActionState::Cancelled); }
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn full_request_retention_does_not_evict_refusals_or_prevent_exact_retries() {
    let root = Directory::new(); let mut host = create(&root);
    let mut bad = spec(&host, b"payload"); bad.target.as_mut().unwrap().object += 1;
    for key in 1..=MAX_FILE_REQUESTS as u64 {
        assert!(matches!(host.submit_request(host.revision(), key, bad.clone(), snapshot()).unwrap().disposition,
            FileRequestDisposition::NotAdmitted(_)));
    }
    assert_eq!(host.retained_requests(), MAX_FILE_REQUESTS);
    let revision = host.revision();
    assert_eq!(host.submit_request(revision, 1000, bad.clone(), snapshot()), Err(JournalError::Contract(Error::Limit)));
    host.submit_request(0, 1, bad, Snapshot::default()).unwrap();
    assert_eq!(host.revision(), revision); assert!(host.inspect().control.ledger.stages.is_empty());
}

#[test]
fn aggregate_payload_limit_counts_terminal_requests_and_is_not_refunded() {
    let root = Directory::new(); let mut host = create(&root);
    let mut bad = spec(&host, &vec![0; MAX_PAYLOAD_BYTES]);
    bad.units = MAX_PAYLOAD_BYTES as u64; bad.target.as_mut().unwrap().object += 1;
    for key in 1..=(MAX_FILE_REQUEST_BYTES / MAX_PAYLOAD_BYTES) as u64 {
        host.submit_request(host.revision(), key, bad.clone(), snapshot()).unwrap();
        host.cancel_request(host.revision(), key).unwrap();
    }
    assert_eq!(host.retained_request_bytes(), MAX_FILE_REQUEST_BYTES);
    let mut extra = bad.clone(); extra.payload = vec![1];
    assert_eq!(host.submit_request(host.revision(), 1000, extra, snapshot()), Err(JournalError::Contract(Error::Limit)));
    let revision = host.revision(); host.submit_request(0, 1, bad, Snapshot::default()).unwrap();
    assert_eq!(host.revision(), revision);
}

#[test]
fn storage_failure_exposes_no_candidate_admission_and_requires_exclusive_recovery() {
    let root = Directory::new(); let mut host = create(&root); submitted(&mut host, 1);
    let current = host.request_status(1).unwrap();
    let spec = spec(&host, b"second"); let before = host.inspect();
    std::fs::write(root.store().join("delivery.pending"), b"occupied").unwrap();
    assert!(matches!(host.submit_request(host.revision(), 2, spec.clone(), snapshot()), Err(JournalError::Io(_))));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.request_status(1), Err(JournalError::Unavailable));
    assert_eq!(host.submit_request(0, 2, spec, snapshot()), Err(JournalError::Unavailable));
    drop(host);
    let host = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(host.retained_requests(), 1); assert!(host.request_status(2).is_err());
    assert_eq!(attempt(host.request_status(1).unwrap()), attempt(current));
    assert_eq!(stage(&host, 1), ActionState::Cancelled);
}

#[test]
fn actor_key_cannot_select_a_ledger_id_and_stop_preserves_request_observations() {
    let root = Directory::new(); let mut host = create(&root);
    host.propose(host.revision(), 31, spec(&host, b"operator"), snapshot()).unwrap();
    let original = spec(&host, b"payload");
    let id = attempt(host.submit_request(host.revision(), u64::MAX, original.clone(), snapshot()).unwrap());
    assert_eq!(id, 32);
    let before = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 1,
        expected_control_sequence: before.sequence, expected_authority_epoch: before.ledger.epoch }).unwrap();
    let revision = host.revision();
    assert_eq!(stage(&host, u64::MAX), ActionState::Cancelled);
    host.submit_request(0, u64::MAX, original.clone(), Snapshot::default()).unwrap();
    assert_eq!(host.revision(), revision);
    assert!(host.submit_request(revision, 1, original, snapshot()).is_err());
    assert_eq!(host.retained_requests(), 1); assert_eq!(host.inspect().executions, 0);
}
