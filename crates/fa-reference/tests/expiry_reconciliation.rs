//! Endpoint-backed expiry resolution is not a timeout-based refund.

#[path = "support/two_key_delivery.rs"]
mod support;

use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, NonExecutionReason};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;

#[test]
fn missing_message_can_be_resolved_only_after_endpoint_expiry_and_refunds_once() {
    let (mut broker, mut endpoint, delayed) = support::dispatched(Some(3));
    broker.acknowledgment_lost(1).unwrap();
    let query = broker.status_query(1).unwrap();
    endpoint.observe_time(ElapsedTick(2)).unwrap();
    assert_eq!(endpoint.resolve_expired(&query).unwrap_err(), Error::Incomplete);
    assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    assert_eq!(endpoint.payload(), b"old");
    assert_eq!(broker.inspect().ledger.charged, 16);
    endpoint.observe_time(ElapsedTick(3)).unwrap();
    let receipt = endpoint.resolve_expired(&query).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    assert_eq!(receipt.request(), delayed.request());
    assert_eq!(endpoint.resolve_expired(&query).unwrap(), receipt);
    assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::Resolved(receipt.clone()));
    assert_eq!(broker.inspect().ledger.charged, 16);
    broker.reconcile_status(&query, EndpointStatus::Resolved(receipt.clone())).unwrap();
    broker.reconcile_status(&query, EndpointStatus::Resolved(receipt.clone())).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(broker.inspect().ledger.charged, 0);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(endpoint.deliver(&delayed).unwrap(), receipt);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(endpoint.target().expected_version, 1);
    assert_eq!(endpoint.payload(), b"old");
}

#[test]
fn executed_receipt_wins_over_expiry_and_never_becomes_a_refund() {
    let (mut broker, mut endpoint, delayed) = support::dispatched(Some(3));
    let query = broker.status_query(1).unwrap();
    endpoint.observe_time(ElapsedTick(2)).unwrap();
    let receipt = endpoint.deliver(&delayed).unwrap();
    assert_eq!(endpoint.resolve_expired(&query).unwrap(), receipt);
    broker.acknowledgment_lost(1).unwrap();
    endpoint.observe_time(ElapsedTick(4)).unwrap();
    assert_eq!(endpoint.resolve_expired(&query).unwrap(), receipt);
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    broker.reconcile_status(&query, EndpointStatus::Resolved(receipt)).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.inspect().ledger.available, 84);
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(endpoint.payload(), b"publish");
}

#[test]
fn expiry_resolution_after_restart_requires_the_new_fence_and_original_key_binding() {
    let (mut broker, mut endpoint, delayed) = support::dispatched(Some(3));
    let old_query = broker.status_query(1).unwrap();
    let fence = broker.restart_dispatcher().unwrap();
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    endpoint.observe_time(ElapsedTick(3)).unwrap();
    assert_eq!(endpoint.resolve_expired(&old_query).unwrap_err(), Error::Stale);
    let queries = broker.pending_reconciliation().unwrap();
    assert_eq!(queries.len(), 1);
    let receipt = endpoint.resolve_expired(&queries[0]).unwrap();
    assert_eq!(receipt.request(), delayed.request());
    assert_eq!(receipt.request().approval().unwrap().request(), 101);
    broker.reconcile_status(&queries[0], EndpointStatus::Resolved(receipt)).unwrap();
    assert!(broker.pending_reconciliation().unwrap().is_empty());
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(endpoint.deliver(&delayed).unwrap_err(), Error::Stale);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn expired_retention_is_not_a_license_to_invent_nonexecution() {
    let (mut broker, mut endpoint, delayed) = support::dispatched(Some(3));
    let query = broker.status_query(1).unwrap();
    endpoint.observe_time(delayed.retained_until()).unwrap();
    assert_eq!(endpoint.resolve_expired(&query).unwrap_err(), Error::Stale);
    let status = endpoint.status(&query).unwrap();
    assert_eq!(status, EndpointStatus::RetentionExpired);
    broker.reconcile_status(&query, status).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.inspect().ledger.available, 84);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert!(broker.cancel(1).is_err());
    assert_eq!(endpoint.deliver(&delayed).unwrap_err(), Error::Stale);
}

#[test]
fn identical_numeric_keys_do_not_cross_endpoint_or_broker_brands() {
    let (mut first, mut endpoint, _) = support::dispatched(Some(3));
    let (mut second, mut other_endpoint, _) = support::dispatched(Some(3));
    let query = first.status_query(1).unwrap();
    endpoint.observe_time(ElapsedTick(3)).unwrap();
    other_endpoint.observe_time(ElapsedTick(3)).unwrap();
    assert_eq!(other_endpoint.resolve_expired(&query).unwrap_err(), Error::Binding);
    let receipt = endpoint.resolve_expired(&query).unwrap();
    let before = second.inspect();
    assert_eq!(second.accept_receipt(receipt.clone()).unwrap_err(), Error::Binding);
    assert_eq!(second.inspect(), before);
    first.accept_receipt(receipt).unwrap();
    assert_eq!(first.inspect().ledger.available, 100);
    assert_eq!(second.inspect().ledger.available, 84);
    assert_eq!(other_endpoint.status(&second.status_query(1).unwrap()).unwrap(), EndpointStatus::AwaitingResolution);
}

#[test]
fn ordinary_profile_resolves_only_at_its_original_action_deadline() {
    let (mut broker, mut endpoint, _) = support::dispatched(None);
    let query = broker.status_query(1).unwrap();
    endpoint.observe_time(ElapsedTick(99)).unwrap();
    assert_eq!(endpoint.resolve_expired(&query).unwrap_err(), Error::Incomplete);
    endpoint.observe_time(ElapsedTick(100)).unwrap();
    let receipt = endpoint.resolve_expired(&query).unwrap();
    assert_eq!(receipt.request().approval(), None);
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(endpoint.execution_count(), 0);
}
