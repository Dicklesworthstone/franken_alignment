//! The stop mailbox retains observations while refusing new intake.

#[path = "support/actor_gateway.rs"]
#[allow(dead_code)]
mod support;

use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, IntakeLimits, Knowledge};
use fa_reference::action::ActionState;
use fa_reference::Error;
use support::{fixture, proposal, review, snapshot};

#[test]
fn stop_cancels_queued_and_reserved_requests_but_keeps_exact_retries_and_tickets() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let accepted = port.submit(10, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 10, 11);
    let permit = supervisor.authorize_request(10, Some(&inputs), &snapshot()).unwrap();
    let queued = port.submit(20, &proposal()).unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.reserved, 16);
    let before = supervisor.broker().inspect();
    let request = StopRequest { operation: 1, expected_control_sequence: before.sequence,
        expected_authority_epoch: before.ledger.epoch };
    let receipt = supervisor.request_stop(request).unwrap();
    assert_eq!(receipt.cancelled(), &[supervisor.attempt(10).unwrap()]);
    assert_eq!(receipt.refunded_units(), 16);
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    for ticket in [&accepted, &queued] {
        assert!(matches!(port.poll(ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
    assert_eq!(port.submit(30, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(port.submit(10, &proposal()).unwrap().request(), 10);
    assert_eq!(port.submit(20, &proposal()).unwrap().request(), 20);
    let mut changed = proposal(); changed.payload.push(b'x');
    assert_eq!(port.submit(10, &changed).unwrap_err(), ActorError::IdempotencyConflict);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    assert_eq!(supervisor.dispatch_request(10, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap_err(), Error::WrongState);
    assert!(supervisor.progress_stop(&mut endpoint).unwrap().progress.drained());
    assert_eq!(supervisor.request_stop(request).unwrap(), receipt);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn failed_stop_preflight_does_not_close_intake_or_cancel_a_live_queue() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let ticket = port.submit(1, &proposal()).unwrap();
    let wrong = StopRequest { operation: 1, expected_control_sequence: 1, expected_authority_epoch: 0 };
    assert_eq!(supervisor.request_stop(wrong), Err(Error::Stale));
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    assert!(port.submit(2, &proposal()).is_ok());
    assert!(supervisor.accept_next(&snapshot()).unwrap().unwrap().result.is_ok());
}

#[test]
fn stop_is_not_controller_loss_and_a_late_real_receipt_still_reaches_the_actor() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let ticket = port.submit(1, &proposal()).unwrap();
    supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut supervisor, 1, 1);
    let permit = supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let envelope = supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    let executed = endpoint.deliver(&envelope).unwrap();
    let state = supervisor.broker().inspect();
    supervisor.request_stop(StopRequest { operation: 5, expected_control_sequence: state.sequence,
        expected_authority_epoch: state.ledger.epoch }).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    assert_eq!(supervisor.broker().inspect().ledger.stages[&1], ActionState::Unknown);
    supervisor.accept_receipt(executed).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    assert!(supervisor.progress_stop(&mut endpoint).unwrap().progress.drained());
    assert_eq!(port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
}

#[test]
fn direct_broker_stop_is_propagated_by_both_intake_and_projection_handoffs() {
    for synchronize in [false, true] {
        let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
        let ticket = port.submit(40, &proposal()).unwrap();
        let before = supervisor.broker().inspect();
        supervisor.broker_mut().request_stop(StopRequest {
            operation: 7, expected_control_sequence: before.sequence,
            expected_authority_epoch: before.ledger.epoch,
        }).unwrap();
        if synchronize {
            supervisor.synchronize().unwrap();
        } else {
            let mut unavailable = snapshot(); unavailable.complete = false;
            assert!(supervisor.accept_next(&unavailable).unwrap().is_none());
        }
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        assert_eq!(port.submit(41, &proposal()).unwrap_err(), ActorError::Unavailable);
        assert_eq!(port.submit(40, &proposal()).unwrap().request(), 40);
        assert!(supervisor.broker().inspect().ledger.stages.is_empty());
        assert_eq!(supervisor.broker().inspect().ledger.available, 100);
        assert!(supervisor.progress_stop(&mut endpoint).unwrap().progress.drained());
        assert_eq!(endpoint.execution_count(), 0);
    }
}
