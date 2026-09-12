//! Terminal stop integrates the original active job, mailbox, and offline owner.
#![cfg(unix)]

#[path = "support/supervised_driver.rs"]
#[allow(dead_code)]
mod support;

use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::supervised::{DriverError, DriverEvent, DriverPhase};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use support::{Rig, proposal, snapshot};

fn stop(rig: &Rig) -> StopRequest {
    let state = rig.driver.supervisor().broker().inspect();
    StopRequest { operation: 1, expected_control_sequence: state.sequence,
        expected_authority_epoch: state.ledger.epoch }
}

#[test]
fn stopping_an_active_review_does_not_wait_for_a_missing_helper_or_accept_new_work() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    rig.start(1, 1);
    let queued = rig.port.submit(2, &proposal()).unwrap();
    let request = stop(&rig);
    rig.driver.request_stop(request).unwrap();
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    for ticket in [&ticket, &queued] {
        assert!(matches!(rig.port.poll(ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
    assert_eq!(rig.port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert!(rig.driver.progress_stop(ElapsedTick(1)).unwrap().progress.drained());
    assert!(rig.driver.stop_progress().unwrap().quiesced());
    assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(), DriverEvent::Idle));
    assert!(rig.driver.accept_next(&snapshot()).unwrap().is_none());
}

#[test]
fn a_stop_while_waiting_for_human_approval_needs_neither_key_nor_snapshot() {
    let (mut rig, reviewer) = Rig::new(true);
    let ticket = rig.accept(1);
    rig.start(1, 1);
    rig.finish_review([Verdict::Allow; 2]);
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::AwaitingHuman { .. }));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 0);
    drop(reviewer);
    let request = stop(&rig);
    rig.driver.request_stop(request).unwrap();
    rig.inputs = None;
    rig.clients.clear();
    assert!(rig.driver.progress_stop(ElapsedTick(1)).unwrap().progress.drained());
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert!(rig.driver.supervisor().broker().human_review_required());
}

#[test]
fn offline_stop_moves_no_authority_and_cannot_resume_a_retained_reserved_permit() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    rig.start(1, 1);
    rig.finish_review([Verdict::Allow; 2]);
    rig.driver.supervisor_mut().broker_mut().restart_dispatcher().unwrap();
    assert_eq!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap_err(), DriverError::Control(Error::Incomplete));
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
    let request = stop(&rig);
    let (mut offline, endpoint) = rig.driver.detach_endpoint();
    let receipt = offline.request_stop(request).unwrap();
    assert_eq!(receipt.refunded_units(), 16);
    assert_eq!(offline.supervisor().broker().inspect().ledger.available, 100);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    let mut driver = match offline.reconnect(endpoint, ElapsedTick(1)) {
        Ok(driver) => driver,
        Err(failure) => panic!("original endpoint refused: {failure:?}"),
    };
    assert_eq!(driver.phase(), DriverPhase::Idle);
    assert_eq!(driver.supervisor().broker().inspect().ledger.stages[&1], ActionState::Cancelled);
    assert!(driver.progress_stop(ElapsedTick(1)).unwrap().progress.drained());
    assert_eq!(driver.request_stop(request).unwrap(), receipt);
    assert_eq!(driver.endpoint().execution_count(), 0);
    assert_eq!(rig.port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
}

#[test]
fn lower_level_stop_releases_the_review_even_when_the_clock_update_refuses() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(1);
    rig.start(1, 1);
    let queued = rig.port.submit(2, &proposal()).unwrap();
    let request = stop(&rig);
    rig.driver.supervisor_mut().broker_mut().request_stop(request).unwrap();
    assert!(!rig.driver.stop_progress().unwrap().review_released);
    assert!(!rig.driver.stop_progress().unwrap().quiesced());
    assert_eq!(rig.driver.progress_stop(ElapsedTick(0)), Err(Error::Stale));
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert!(rig.driver.stop_progress().unwrap().review_released);
    assert!(rig.driver.helpers_reaped());
    assert!(!rig.driver.stop_progress().unwrap().effects.endpoint_fenced);
    for ticket in [&ticket, &queued] {
        assert!(matches!(rig.port.poll(ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
    assert_eq!(rig.port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert!(rig.driver.progress_stop(ElapsedTick(1)).unwrap().progress.drained());
    assert!(rig.driver.stop_progress().unwrap().quiesced());
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}
