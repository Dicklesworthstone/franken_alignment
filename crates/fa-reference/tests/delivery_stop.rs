//! Stop admission, externally fence, and resolve each ORIGINAL effect once.

#[path = "support/two_key_delivery.rs"]
mod support;

use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, NonExecutionReason, StopRequest};
use fa_reference::action::consequence::oversight::OversightBroker;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, VERSION};
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;
use support::{Fixture, dispatched};

fn stop(broker: &OversightBroker, operation: u64) -> StopRequest {
    let state = broker.inspect();
    StopRequest { operation, expected_control_sequence: state.sequence,
        expected_authority_epoch: state.ledger.epoch }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}

#[test]
fn both_key_profiles_fence_delayed_envelopes_and_refund_only_endpoint_nonexecution() {
    for expiry in [None, Some(50)] {
        let (mut broker, mut endpoint, delayed) = dispatched(expiry);
        let old_ack = endpoint.install_fence(broker.fence_request()).unwrap();
        let request = stop(&broker, 90);
        let receipt = broker.request_stop(request).unwrap();
        assert!(broker.inspect().suspended);
        assert_eq!(receipt.revocation_floor(), 1);
        assert_eq!(receipt.refunded_units(), 0);
        assert!(receipt.cancelled().is_empty());
        assert_eq!(broker.inspect().ledger.charged, 16);
        assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
        assert!(!broker.stop_progress().unwrap().endpoint_fenced);
        assert!(!broker.stop_progress().unwrap().drained());
        assert_eq!(broker.confirm_fence(old_ack), Err(Error::Stale));
        let swept = broker.progress_stop(&mut endpoint).unwrap();
        let terminal = match swept.outcomes.get(&1).unwrap().as_ref().unwrap() {
            EndpointStatus::Resolved(receipt) => receipt.clone(),
            other => panic!("expected endpoint evidence, got {other:?}"),
        };
        assert_eq!(terminal.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
        assert!(swept.progress.drained());
        assert_eq!(swept.progress.charged_units, 0);
        assert_eq!(broker.inspect().ledger.available, 100);
        assert_eq!(broker.inspect().ledger.stages[&1], ActionState::ConfirmedNotExecuted);
        assert_eq!(endpoint.deliver(&delayed), Err(Error::Stale));
        assert_eq!(endpoint.execution_count(), 0);
        assert_eq!(endpoint.payload(), b"old");
        assert!(!broker.accept_receipt(terminal).unwrap());
        assert!(broker.progress_stop(&mut endpoint).unwrap().outcomes.is_empty());
        assert_eq!(broker.request_stop(request).unwrap(), receipt);
        assert_eq!(broker.inspect().ledger.epoch, 1);
    }
}

#[test]
fn execution_before_endpoint_acknowledgment_wins_over_a_local_stop() {
    let (mut broker, mut endpoint, delayed) = dispatched(Some(50));
    let request = stop(&broker, 1);
    broker.request_stop(request).unwrap();
    // The endpoint has not received the new fence yet. Local stop is NOT proof
    // that an already-admitted message cannot still execute during this gap.
    let executed = endpoint.deliver(&delayed).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    let sweep = broker.progress_stop(&mut endpoint).unwrap();
    assert_eq!(sweep.outcomes[&1], Ok(EndpointStatus::Resolved(executed.clone())));
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(broker.inspect().ledger.available, 84);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert!(!broker.accept_receipt(executed).unwrap());
    assert_eq!(endpoint.deliver(&delayed), Err(Error::Stale));
}

#[test]
fn one_sweep_preserves_execution_and_seals_a_second_missing_request() {
    let mut f = Fixture::new(true);
    let first = f.dispatch(1, Some(50));
    let executed = f.endpoint.deliver(&first).unwrap();
    f.broker.acknowledgment_lost(1).unwrap();
    let second = f.dispatch(2, Some(50));
    let request = stop(&f.broker, 7);
    f.broker.request_stop(request).unwrap();
    assert_eq!(f.broker.inspect().ledger.charged, 32);
    f.broker.inputs_unavailable(1, f.broker.input_revision(1).unwrap()).unwrap();
    f.broker.inputs_unavailable(2, f.broker.input_revision(2).unwrap()).unwrap();
    let sweep = f.broker.progress_stop(&mut f.endpoint).unwrap();
    assert_eq!(sweep.outcomes.len(), 2);
    assert_eq!(sweep.outcomes[&1], Ok(EndpointStatus::Resolved(executed)));
    assert!(sweep.progress.drained());
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert_eq!(f.broker.inspect().ledger.available, 84);
    assert_eq!(f.broker.inspect().ledger.stages[&2], ActionState::ConfirmedNotExecuted);
    assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.endpoint.target().expected_version, 2);
    assert_eq!(f.endpoint.deliver(&second), Err(Error::Stale));
}

#[test]
fn stop_request_identity_is_exact_and_stale_predecessors_do_not_change_any_state() {
    let (mut broker, mut endpoint, _) = dispatched(None);
    let request = stop(&broker, 8);
    let before = broker.inspect();
    assert_eq!(broker.request_stop(StopRequest { operation: 0, ..request }), Err(Error::InvalidInput));
    assert_eq!(broker.request_stop(StopRequest { expected_control_sequence: request.expected_control_sequence + 1, ..request }), Err(Error::Stale));
    assert_eq!(broker.request_stop(StopRequest { expected_authority_epoch: request.expected_authority_epoch + 1, ..request }), Err(Error::Stale));
    assert_eq!(broker.inspect(), before);
    assert!(broker.stop_receipt().is_none());
    assert_eq!(broker.progress_stop(&mut endpoint), Err(Error::Incomplete));
    let receipt = broker.request_stop(request).unwrap();
    let stopped = broker.inspect();
    assert_eq!(broker.request_stop(request).unwrap(), receipt);
    assert_eq!(broker.request_stop(StopRequest { expected_authority_epoch: 9, ..request }), Err(Error::Binding));
    assert_eq!(broker.request_stop(StopRequest { operation: 9, ..request }), Err(Error::Duplicate));
    assert_eq!(broker.inspect(), stopped);
}

#[test]
fn foreign_endpoint_cannot_satisfy_the_barrier_and_restart_requires_fresh_acknowledgment() {
    let (mut broker, mut endpoint, _) = dispatched(None);
    let (_, mut foreign, _) = dispatched(None);
    broker.request_stop(stop(&broker, 3)).unwrap();
    let before = broker.stop_progress().unwrap();
    assert_eq!(broker.progress_stop(&mut foreign), Err(Error::Binding));
    assert_eq!(broker.stop_progress().unwrap(), before);
    assert!(broker.progress_stop(&mut endpoint).unwrap().progress.drained());
    broker.restart_dispatcher().unwrap();
    assert!(!broker.stop_progress().unwrap().drained());
    let repeat = broker.progress_stop(&mut endpoint).unwrap();
    assert!(repeat.progress.drained());
    assert!(repeat.outcomes.is_empty());
    assert!(repeat.progress.dispatcher_epoch > repeat.progress.receipt.dispatcher_epoch());
}

#[test]
fn expired_retention_never_turns_unknown_execution_into_a_successful_drain() {
    let (mut broker, mut endpoint, delayed) = dispatched(None);
    broker.request_stop(stop(&broker, 4)).unwrap();
    endpoint.observe_time(delayed.retained_until()).unwrap();
    let sweep = broker.progress_stop(&mut endpoint).unwrap();
    assert_eq!(sweep.outcomes[&1], Ok(EndpointStatus::RetentionExpired));
    assert!(sweep.progress.endpoint_fenced);
    assert!(!sweep.progress.drained());
    assert_eq!(sweep.progress.unresolved, vec![1]);
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(broker.inspect().ledger.available, 84);
}

#[test]
fn abandoned_obligations_do_not_disappear_from_stop_progress() {
    let (mut broker, mut endpoint, _) = dispatched(None);
    broker.acknowledgment_lost(1).unwrap();
    broker.abandon_unknown(1).unwrap();
    broker.request_stop(stop(&broker, 5)).unwrap();
    let sweep = broker.progress_stop(&mut endpoint).unwrap();
    assert!(sweep.outcomes.is_empty());
    assert_eq!(sweep.progress.irrecoverable, vec![1]);
    assert_eq!(sweep.progress.unresolved, vec![1]);
    assert!(!sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
}

#[test]
fn a_stopped_broker_cannot_create_new_work_under_its_advanced_epoch() {
    let (mut broker, mut endpoint, old) = dispatched(None);
    broker.request_stop(stop(&broker, 6)).unwrap();
    broker.progress_stop(&mut endpoint).unwrap();
    let spec = ActionSpec {
        version: VERSION, scope: old.request().scope(), target: Some(endpoint.target()),
        payload: b"new publication".to_vec(), required_witnesses: vec![],
        policy_epoch: broker.inspect().ledger.epoch, deadline: ElapsedTick(100), units: 16,
    };
    assert_eq!(broker.propose(2, spec, &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn observation_loss_does_not_block_stopping_or_endpoint_backed_refunds() {
    use fa_reference::action::{Purpose, Scope};
    use fa_reference::action::consequence::oversight::policy_state::{StateEvent, StateLimits, StateSource};
    for two_key in [false, true] {
        let mut f = Fixture::new(two_key);
        let source = StateSource { source: 1, generation: 1, scope: Scope {
            tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect,
        } };
        let writer = f.broker.enable_policy_state(source, StateLimits::default()).unwrap();
        writer.record(1, &StateEvent::Snapshot { semantic_epoch: 1, values: snapshot().values }).unwrap();
        writer.close(1, 1).unwrap();
        let message = f.dispatch(1, two_key.then_some(50));
        writer.withdraw();
        drop(writer);
        assert!(f.broker.capture_policy_state().is_err());
        f.broker.request_stop(stop(&f.broker, 30)).unwrap();
        let sweep = f.broker.progress_stop(&mut f.endpoint).unwrap();
        assert!(sweep.progress.drained());
        assert_eq!(sweep.progress.charged_units, 0);
        assert_eq!(f.endpoint.execution_count(), 0);
        assert_eq!(f.endpoint.deliver(&message), Err(Error::Stale));
    }
}

#[test]
fn an_expired_earlier_obligation_does_not_prevent_settling_a_later_one() {
    let mut f = Fixture::new(false);
    let older = f.dispatch(1, None);
    f.broker.observe_time(ElapsedTick(50)).unwrap();
    f.endpoint.observe_time(ElapsedTick(50)).unwrap();
    let later = f.dispatch(2, None);
    f.broker.request_stop(stop(&f.broker, 40)).unwrap();
    f.endpoint.observe_time(older.retained_until()).unwrap();
    let swept = f.broker.progress_stop(&mut f.endpoint).unwrap();
    assert_eq!(swept.outcomes[&1], Ok(EndpointStatus::RetentionExpired));
    assert!(matches!(swept.outcomes[&2], Ok(EndpointStatus::Resolved(_))));
    assert_eq!(swept.progress.unresolved, vec![1]);
    assert_eq!(swept.progress.charged_units, 16);
    assert!(!swept.progress.drained());
    assert_eq!(f.broker.inspect().ledger.stages[&2], ActionState::ConfirmedNotExecuted);
    assert_eq!(f.broker.inspect().ledger.available, 84);
    assert_eq!(f.endpoint.deliver(&later), Err(Error::Stale));
}
