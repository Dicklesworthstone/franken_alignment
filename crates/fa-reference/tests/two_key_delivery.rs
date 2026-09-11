//! Public delayed-delivery regressions; no real user, network or durability claim.

#[path = "support/two_key_delivery.rs"]
mod support;

use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, NonExecutionReason};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;

#[test]
fn first_delivery_requires_both_deadlines_without_rewriting_the_frozen_action() {
    for (tick, executed) in [(2, true), (3, false), (4, false), (99, false)] {
        let (mut broker, mut endpoint, message) = support::dispatched(Some(3));
        let approval = message.request().approval().unwrap();
        assert_eq!(approval.request(), 101);
        assert_eq!(approval.reviewer(), 90);
        assert_eq!(approval.issued_at(), ElapsedTick(1));
        assert_eq!(approval.expires_at(), ElapsedTick(3));
        assert_eq!(message.request().deadline(), ElapsedTick(100));
        assert_eq!(message.request().execution_deadline(), ElapsedTick(3));
        assert_eq!(message.request().payload(), b"publish");
        assert_eq!(broker.human_request(101).unwrap().action().spec().deadline, ElapsedTick(100));
        endpoint.observe_time(ElapsedTick(tick)).unwrap();
        let receipt = endpoint.deliver(&message).unwrap();
        assert_eq!(receipt.request(), message.request());
        assert_eq!(receipt.outcome(), if executed {
            EndpointOutcome::Executed { resulting_version: 2 }
        } else {
            EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed }
        });
        assert_eq!(endpoint.execution_count(), u64::from(executed));
        assert_eq!(endpoint.payload(), if executed { &b"publish"[..] } else { &b"old"[..] });
        assert_eq!(endpoint.target().expected_version, if executed { 2 } else { 1 });
        assert!(broker.accept_receipt(receipt.clone()).unwrap());
        assert!(!broker.accept_receipt(receipt.clone()).unwrap());
        assert_eq!(endpoint.deliver(&message).unwrap(), receipt);
        assert_eq!(broker.inspect().ledger.available, if executed { 84 } else { 100 });
        assert_eq!(broker.inspect().ledger.charged, if executed { 16 } else { 0 });
        assert_eq!(broker.inspect().ledger.stages[&1], if executed { ActionState::Confirmed } else { ActionState::ConfirmedNotExecuted });
        assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    }
}

#[test]
fn execution_before_expiry_remains_executed_when_the_acknowledgment_arrives_late() {
    let (mut broker, mut endpoint, message) = support::dispatched(Some(3));
    endpoint.observe_time(ElapsedTick(2)).unwrap();
    let executed = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    endpoint.observe_time(ElapsedTick(4)).unwrap();
    assert_eq!(endpoint.deliver(&message).unwrap(), executed);
    let query = broker.status_query(1).unwrap();
    let status = endpoint.status(&query).unwrap();
    assert_eq!(status, EndpointStatus::Resolved(executed.clone()));
    broker.reconcile_status(&query, status).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.inspect().ledger.available, 84);
    assert_eq!(endpoint.execution_count(), 1);
    assert!(!broker.accept_receipt(executed).unwrap());
}

#[test]
fn elapsed_human_deadline_and_missing_status_are_not_nonexecution_evidence() {
    let (mut broker, mut endpoint, message) = support::dispatched(Some(3));
    broker.observe_time(ElapsedTick(4)).unwrap();
    endpoint.observe_time(ElapsedTick(4)).unwrap();
    let query = broker.status_query(1).unwrap();
    assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
    broker.reconcile_status(&query, endpoint.status(&query).unwrap()).unwrap();
    let before = broker.inspect();
    assert_eq!(before.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(before.ledger.charged, 16);
    assert!(broker.cancel(1).is_err());
    assert_eq!(broker.inspect(), before);
    let receipt = endpoint.deliver(&message).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
}

#[test]
fn dispatcher_restart_carries_original_second_key_into_sealing_and_reconciliation() {
    let (mut broker, mut endpoint, message) = support::dispatched(Some(3));
    let fence = broker.restart_dispatcher().unwrap();
    broker.confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    endpoint.observe_time(ElapsedTick(4)).unwrap();
    let query = broker.status_query(1).unwrap();
    let receipt = endpoint.seal_unexecuted(&query).unwrap();
    assert_eq!(receipt.request(), message.request());
    assert_eq!(receipt.request().approval().unwrap().expires_at(), ElapsedTick(3));
    broker.reconcile_status(&query, EndpointStatus::Resolved(receipt.clone())).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(endpoint.deliver(&message).unwrap_err(), Error::Stale);
    assert!(!broker.accept_receipt(receipt).unwrap());
}

#[test]
fn ordinary_delivery_keeps_its_original_deadline_and_has_no_second_key() {
    for (tick, executed) in [(99, true), (100, false)] {
        let (mut broker, mut endpoint, message) = support::dispatched(None);
        assert_eq!(message.request().approval(), None);
        assert_eq!(message.request().execution_deadline(), ElapsedTick(100));
        endpoint.observe_time(ElapsedTick(tick)).unwrap();
        let receipt = endpoint.deliver(&message).unwrap();
        assert_eq!(receipt.outcome(), if executed {
            EndpointOutcome::Executed { resulting_version: 2 }
        } else {
            EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed }
        });
        broker.accept_receipt(receipt).unwrap();
        assert_eq!(endpoint.execution_count(), u64::from(executed));
    }
}
