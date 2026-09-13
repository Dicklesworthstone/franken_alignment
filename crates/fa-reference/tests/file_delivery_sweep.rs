#![cfg(unix)]
#[path = "support/file_delivery.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;

#[test]
fn recovered_batch_preserves_unknown_until_expiry_and_accepts_execution_without_resend() {
    let root = Directory::new();
    let mut host = create(&root);
    dispatched(&mut host, 1, b"already published");
    host.publish(host.revision(), 1).unwrap();
    dispatched(&mut host, 2, b"not published");
    assert_eq!(host.inspect().control.ledger.charged, 32);
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    let before = host.inspect();
    assert_eq!(host.reconcile_pending(host.revision()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(host.inspect(), before);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let revision = host.revision();
    assert_eq!(host.reconcile_pending(revision - 1).unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(host.revision(), revision);
    let results = host.reconcile_pending(revision).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(results[&2], Ok(Reconciliation::AwaitingResolution));
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Unknown);
    assert!(host.publish(host.revision(), 2).is_err());
    host.observe_time(host.revision(), ElapsedTick(100)).unwrap();
    let results = host.reconcile_pending(host.revision()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted {
        reason: NonExecutionReason::DeadlineElapsed })));
    assert!(host.reconcile_pending(host.revision()).unwrap().is_empty());
    let settled = host.inspect();
    assert_eq!(settled.control.ledger.available, 84);
    assert_eq!(settled.control.ledger.charged, 16);
    assert_eq!(settled.executions, 1);
    assert_eq!(settled.payload, b"already published");
    drop(host);
    let host = FileDelivery::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn retention_expiry_is_not_a_refund_even_when_the_same_batch_can_resolve_a_younger_request() {
    let root = Directory::new();
    let mut host = create(&root);
    dispatched(&mut host, 1, b"too old");
    host.observe_time(host.revision(), ElapsedTick(500)).unwrap();
    let mut later = spec(&host, b"younger");
    later.deadline = ElapsedTick(900);
    let action = host.propose(host.revision(), 2, later, snapshot()).unwrap();
    host.review(host.revision(), review(2, 102, fa_reference::round::Verdict::Allow)).unwrap();
    let permit = host.authorize(host.revision(), 2, snapshot()).unwrap();
    host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap();
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
    let results = host.reconcile_pending(host.revision()).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[&1], Ok(Reconciliation::RetentionExpired));
    assert_eq!(results[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted {
        reason: NonExecutionReason::DeadlineElapsed })));
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 0);
    assert!(host.seal_unexecuted(host.revision(), 1).is_err());
    drop(host);
    let mut host = FileDelivery::open(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1002)).unwrap();
    let again = host.reconcile_pending(host.revision()).unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[&1], Ok(Reconciliation::RetentionExpired));
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn a_durable_expiry_receipt_prevents_late_delivery_through_a_retained_original_envelope() {
    let root = Directory::new();
    let mut host = create(&root);
    dispatched(&mut host, 1, b"must not appear");
    host.observe_time(host.revision(), ElapsedTick(100)).unwrap();
    let outcome = EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed };
    assert_eq!(host.reconcile_pending(host.revision()).unwrap()[&1], Ok(Reconciliation::Resolved(outcome)));
    assert_eq!(host.publish(host.revision(), 1).unwrap(), outcome);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().payload, b"initial");
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(FileDelivery::read_publication(root.store(), &profile()).unwrap().control.ledger.available, 100);
}
