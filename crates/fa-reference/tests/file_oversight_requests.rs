//! Exact external retries must not bypass original inputs or either live key.
#![cfg(unix)]
#[path = "support/file_oversight.rs"]
mod support;
use support::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::*;
use fa_reference::action::consequence::delivery::persistent::requests::{FileRequestDisposition, FileRequestStatus, MAX_FILE_REQUESTS};
use fa_reference::{Error, Snapshot};

fn id(status: FileRequestStatus) -> u64 {
    match status.disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("request was not admitted: {other:?}"),
    }
}
fn stage(host: &FileOversight, request: u64) -> ActionState {
    match host.request_status(request).unwrap().disposition {
        FileRequestDisposition::Admitted { stage, .. } => stage,
        other => panic!("request was not admitted: {other:?}"),
    }
}
fn keys(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64, round: u64) -> Keys {
    let attempt = id(host.request_status(request).unwrap());
    let action = host.request_action(request).unwrap().clone();
    let input = inputs(&action, b"the complete original helper source");
    review_existing(host, attempt, round, &input);
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let human_request = host.request_human_approval(host.revision(), request + 1000, attempt,
        &input, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &human_request).unwrap();
    Keys { action, inputs: input, automatic, human, request: human_request }
}

#[test]
fn full_input_two_key_publication_survives_recovery_as_one_original_request() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let spec = spec(&host, b"published");
    let admitted = host.submit_request(host.revision(), 90, spec.clone(), snapshot()).unwrap();
    let attempt = id(admitted); assert_ne!(attempt, 90);
    let keys = keys(&mut host, &reviewer, 90, 7);
    assert_eq!(host.request_status(90).unwrap().generation, admitted.generation);
    dispatch(&mut host, &keys);
    host.publish(host.revision(), attempt).unwrap();
    assert_eq!(stage(&host, 90), ActionState::Dispatching);
    let generation = host.request_status(90).unwrap().generation;
    drop(host); drop(reviewer);
    let (mut recovered, new_reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    drop(new_reviewer); // Outcome reconciliation must not depend on this role.
    let revision = recovered.revision();
    let retried = recovered.submit_request(0, 90, spec.clone(), Snapshot::default()).unwrap();
    assert_eq!(id(retried), attempt); assert_eq!(retried.generation, generation);
    assert_eq!(stage(&recovered, 90), ActionState::Unknown);
    assert_eq!(recovered.revision(), revision); assert_eq!(recovered.retained_requests(), 1);
    assert!(!recovered.clock_ready());
    assert!(recovered.dispatch(revision, &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    assert!(recovered.publish(recovered.revision(), attempt).is_err());
    let result = recovered.reconcile_pending(recovered.revision()).unwrap();
    assert_eq!(result[&attempt], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(stage(&recovered, 90), ActionState::Confirmed);
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    assert_eq!(recovered.inspect().executions, 1);
    let terminal = recovered.request_status(90).unwrap(); let revision = recovered.revision();
    assert_eq!(recovered.submit_request(0, 90, spec, Snapshot::default()).unwrap(), terminal);
    assert_eq!(recovered.revision(), revision);
}

#[test]
fn reserved_keys_and_reviews_do_not_resurrect_but_fresh_epoch_work_can_publish() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let spec = spec(&host, b"old");
    host.submit_request(host.revision(), 10, spec.clone(), snapshot()).unwrap();
    let previous = keys(&mut host, &reviewer, 10, 7);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    drop(host);
    let (mut host, current_reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let revision = host.revision();
    host.submit_request(0, 10, spec, Snapshot::default()).unwrap();
    assert_eq!(host.revision(), revision); assert_eq!(stage(&host, 10), ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(reviewer.approve(&mut host, revision, &previous.request).is_err());
    assert!(host.dispatch(revision, &previous.automatic, &previous.human, &previous.action, &previous.inputs, snapshot()).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let fresh = support::spec(&host, b"new");
    host.submit_request(host.revision(), 11, fresh, snapshot()).unwrap();
    let next = keys(&mut host, &current_reviewer, 11, 8);
    dispatch(&mut host, &next); host.publish(host.revision(), next.automatic.attempt()).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"new");
}

#[test]
fn exact_retry_cannot_refresh_changed_helper_evidence_or_expired_human_approval() {
    for changed_input in [false, true] {
        let root = Directory::new(); let (mut host, reviewer) = create(&root);
        let spec = spec(&host, b"reviewed");
        host.submit_request(host.revision(), 1, spec.clone(), snapshot()).unwrap();
        let keys = keys(&mut host, &reviewer, 1, 7); let attempt = keys.automatic.attempt();
        if changed_input {
            let changed = inputs(&keys.action, b"different full evidence");
            host.record_inputs(host.revision(), attempt, host.input_revision(attempt).unwrap(), changed).unwrap();
        } else { host.observe_time(host.revision(), ElapsedTick(31)).unwrap(); }
        let before = host.inspect();
        host.submit_request(0, 1, spec, Snapshot::default()).unwrap();
        assert_eq!(host.inspect(), before);
        assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 16);
    }
}

#[test]
fn a_refused_key_and_its_allocation_survive_recovery_and_improved_observations() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let mut bad = spec(&host, b"bad"); bad.target.as_mut().unwrap().object += 1;
    let refused = host.submit_request(host.revision(), 8, bad.clone(), snapshot()).unwrap();
    assert_eq!(refused.disposition, FileRequestDisposition::NotAdmitted(Error::Binding));
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap(); let revision = host.revision();
    assert_eq!(host.submit_request(0, 8, bad, Snapshot::default()).unwrap(), refused);
    assert_eq!(host.revision(), revision); assert!(host.request_action(8).is_err());
    let good = spec(&host, b"good");
    assert_eq!(host.submit_request(revision, 8, good.clone(), snapshot()), Err(JournalError::Contract(Error::Binding)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let next = host.submit_request(host.revision(), 9, good, snapshot()).unwrap();
    assert_eq!(id(next), 2); assert_eq!(host.retained_requests(), 2);
}

#[test]
fn stop_cancels_reserved_requests_but_sent_requests_require_original_endpoint_evidence() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    for request in [20, 21] {
        let spec = spec(&host, b"payload");
        host.submit_request(host.revision(), request, spec, snapshot()).unwrap();
        let keys = keys(&mut host, &reviewer, request, request + 100);
        if request == 20 { dispatch(&mut host, &keys); }
    }
    let before = host.inspect();
    host.cancel_request(host.revision(), 20).unwrap(); assert_eq!(host.inspect(), before);
    let request = StopRequest { operation: 1, expected_control_sequence: before.control.sequence,
        expected_authority_epoch: before.control.ledger.epoch };
    host.request_stop(host.revision(), request).unwrap();
    assert_eq!(stage(&host, 21), ActionState::Cancelled);
    assert_eq!(stage(&host, 20), ActionState::Unknown);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let exact = host.request_action(20).unwrap().spec().clone(); let revision = host.revision();
    host.submit_request(0, 20, exact, Snapshot::default()).unwrap(); assert_eq!(host.revision(), revision);
    assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
    assert_eq!(stage(&host, 20), ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn shared_request_limits_and_complete_idempotency_binding_apply_to_the_two_key_owner() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    let mut rejected = spec(&host, b"payload"); rejected.target.as_mut().unwrap().object += 1;
    for request in 1..=MAX_FILE_REQUESTS as u64 {
        host.submit_request(host.revision(), request, rejected.clone(), snapshot()).unwrap();
    }
    assert_eq!(host.retained_requests(), MAX_FILE_REQUESTS);
    let before = host.inspect();
    assert_eq!(host.submit_request(host.revision(), 1000, rejected.clone(), snapshot()), Err(JournalError::Contract(Error::Limit)));
    for field in 0..8 {
        let mut changed = rejected.clone();
        match field {
            0 => changed.payload.push(0), 1 => changed.units += 1, 2 => changed.deadline.0 += 1,
            3 => changed.policy_epoch += 1, 4 => changed.target.as_mut().unwrap().expected_version += 1,
            5 => changed.target.as_mut().unwrap().generation += 1, 6 => changed.scope.principal += 1,
            _ => changed.version += 1,
        }
        assert_eq!(host.submit_request(0, 1, changed, snapshot()), Err(JournalError::Contract(Error::Binding)));
    }
    host.submit_request(0, 1, rejected, Snapshot::default()).unwrap();
    assert_eq!(host.inspect(), before);
}
