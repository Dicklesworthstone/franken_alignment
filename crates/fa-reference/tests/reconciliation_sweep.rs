//! A public recovery sweep settles evidence without reauthorizing effects.

#[path = "support/two_key_delivery.rs"]
mod support;

use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, NonExecutionReason};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;

#[test]
fn mixed_sweep_settles_executed_and_expired_items_but_preserves_live_unknown_liability() {
    let mut fixture = support::Fixture::new(true);
    let first = fixture.dispatch(1, Some(3));
    let second = fixture.dispatch(2, Some(3));
    let third = fixture.dispatch(3, Some(6));
    assert_eq!(fixture.broker.inspect().ledger.available, 52);
    fixture.endpoint.observe_time(ElapsedTick(2)).unwrap();
    let executed = fixture.endpoint.deliver(&first).unwrap();
    fixture.endpoint.observe_time(ElapsedTick(4)).unwrap();
    let outcomes = fixture.broker.reconcile_pending(&mut fixture.endpoint).unwrap();
    assert_eq!(outcomes.len(), 3);
    assert_eq!(outcomes[&1], Ok(EndpointStatus::Resolved(executed.clone())));
    let expired = fixture.broker.resolution(2).unwrap().unwrap().clone();
    assert_eq!(expired.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    assert_eq!(outcomes[&2], Ok(EndpointStatus::Resolved(expired.clone())));
    assert_eq!(outcomes[&3], Ok(EndpointStatus::AwaitingResolution));
    let state = fixture.broker.inspect();
    assert_eq!(state.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(state.ledger.stages[&2], ActionState::ConfirmedNotExecuted);
    assert_eq!(state.ledger.stages[&3], ActionState::Unknown);
    assert_eq!(state.ledger.available, 68);
    assert_eq!(state.ledger.charged, 32);
    for id in 101..=103 {
        assert_eq!(fixture.broker.human_status(id).unwrap().disposition, HumanDisposition::Consumed);
    }
    assert_eq!(fixture.endpoint.execution_count(), 1);
    assert_eq!(fixture.endpoint.deliver(&first).unwrap(), executed);
    assert_eq!(fixture.endpoint.deliver(&second).unwrap(), expired);
    let repeated = fixture.broker.reconcile_pending(&mut fixture.endpoint).unwrap();
    assert_eq!(repeated.len(), 1);
    assert_eq!(repeated[&3], Ok(EndpointStatus::AwaitingResolution));
    assert_eq!(fixture.broker.inspect(), state);
    fixture.endpoint.observe_time(ElapsedTick(6)).unwrap();
    let final_outcomes = fixture.broker.reconcile_pending(&mut fixture.endpoint).unwrap();
    assert_eq!(final_outcomes.len(), 1);
    let final_receipt = fixture.broker.resolution(3).unwrap().unwrap().clone();
    assert_eq!(final_outcomes[&3], Ok(EndpointStatus::Resolved(final_receipt.clone())));
    assert_eq!(fixture.endpoint.deliver(&third).unwrap(), final_receipt);
    assert_eq!(fixture.broker.inspect().ledger.available, 84);
    assert_eq!(fixture.broker.inspect().ledger.charged, 16);
    assert!(fixture.broker.reconcile_pending(&mut fixture.endpoint).unwrap().is_empty());
}

#[test]
fn sweep_requires_confirmed_current_fence_before_mutating_endpoint_or_broker() {
    let (mut broker, mut endpoint, delayed) = support::dispatched(Some(3));
    let fence = broker.restart_dispatcher().unwrap();
    endpoint.observe_time(ElapsedTick(3)).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.reconcile_pending(&mut endpoint).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect(), before);
    let ack = endpoint.install_fence(fence).unwrap();
    assert_eq!(broker.reconcile_pending(&mut endpoint).unwrap_err(), Error::Incomplete);
    assert_eq!(endpoint.status(&broker.status_query(1).unwrap()).unwrap(), EndpointStatus::AwaitingResolution);
    broker.confirm_fence(ack).unwrap();
    let outcomes = broker.reconcile_pending(&mut endpoint).unwrap();
    assert!(matches!(outcomes[&1], Ok(EndpointStatus::Resolved(_))));
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(endpoint.deliver(&delayed).unwrap_err(), Error::Stale);
}

#[test]
fn sweep_rejects_a_foreign_endpoint_before_touching_either_accounting_domain() {
    let (mut broker, mut endpoint, _) = support::dispatched(Some(3));
    let (other, mut foreign, _) = support::dispatched(Some(3));
    endpoint.observe_time(ElapsedTick(3)).unwrap();
    foreign.observe_time(ElapsedTick(3)).unwrap();
    let before = broker.inspect();
    let other_before = other.inspect();
    assert_eq!(broker.reconcile_pending(&mut foreign).unwrap_err(), Error::Binding);
    assert_eq!(broker.inspect(), before);
    assert_eq!(other.inspect(), other_before);
    assert_eq!(foreign.status(&other.status_query(1).unwrap()).unwrap(), EndpointStatus::AwaitingResolution);
    assert_eq!(endpoint.status(&broker.status_query(1).unwrap()).unwrap(), EndpointStatus::AwaitingResolution);
    broker.reconcile_pending(&mut endpoint).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn expired_retention_remains_explicit_and_charged_on_every_sweep() {
    let (mut broker, mut endpoint, message) = support::dispatched(Some(3));
    endpoint.observe_time(message.retained_until()).unwrap();
    for _ in 0..2 {
        let outcomes = broker.reconcile_pending(&mut endpoint).unwrap();
        assert_eq!(outcomes[&1], Ok(EndpointStatus::RetentionExpired));
        assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
        assert_eq!(broker.inspect().ledger.charged, 16);
        assert_eq!(broker.inspect().ledger.available, 84);
    }
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn recovery_does_not_require_live_helper_inputs_or_a_reviewer_role() {
    let (mut broker, mut endpoint, _) = support::dispatched(Some(3));
    let revision = broker.input_revision(1).unwrap();
    broker.inputs_unavailable(1, revision).unwrap();
    endpoint.observe_time(ElapsedTick(3)).unwrap();
    let outcomes = broker.reconcile_pending(&mut endpoint).unwrap();
    assert!(matches!(outcomes[&1], Ok(EndpointStatus::Resolved(_))));
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert!(broker.captured_input_bytes() > 0);
}
