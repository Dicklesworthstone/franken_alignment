//! Integrated actor -> socket helpers -> congress -> original permit -> endpoint.
//! Deterministic helpers exercise protocol behavior, not empirical model safety.
#![cfg(unix)]
#[path = "support/supervised_driver.rs"]
mod support;

use support::{Rig, proposal, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::{DriverError, DriverEvent, DriverPhase};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn a_complete_socket_review_then_a_distinct_dispatch_step_publishes_once() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(42);
    rig.start(42, 11);
    let receipt = match rig.finish_review([Verdict::Allow; 2]) {
        DriverEvent::ReviewApplied { receipt, .. } => receipt,
        other => panic!("{other:?}"),
    };
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert_eq!(rig.driver.phase(), DriverPhase::AwaitingDispatch { request: 42 });
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    let result = rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap();
    let DriverEvent::PublicationResolved { request, receipt } = result else { panic!("{result:?}"); };
    assert_eq!(request, 42);
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(rig.driver.endpoint().payload(), b"publish");
    assert_eq!(rig.driver.endpoint().execution_count(), 1);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let duplicate = rig.port.submit(42, &proposal()).unwrap();
    assert_eq!(rig.port.poll(&ticket), rig.port.poll(&duplicate));
    assert!(rig.driver.accept_next(&snapshot()).unwrap().is_none());
    assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(), DriverEvent::Idle));
    assert_eq!(rig.driver.endpoint().execution_count(), 1);
}

#[test]
fn adverse_review_stops_without_a_fresh_round_or_a_fabricated_permission() {
    let (mut rig, _) = Rig::new(false);
    let ticket = rig.accept(42);
    rig.start(42, 11);
    let result = rig.finish_review([Verdict::Hold, Verdict::Allow]);
    let DriverEvent::ReviewApplied { receipt, .. } = result else { panic!("{result:?}"); };
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
    assert_eq!(rig.driver.endpoint().payload(), b"old");
    for _ in 0..3 {
        assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::Idle));
    }
    let attempt = rig.driver.supervisor().attempt(42).unwrap();
    assert!(rig.driver.supervisor_mut().broker_mut().authorize(attempt, rig.inputs.as_ref(), &snapshot()).is_err());
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
}

#[test]
fn cancellation_during_review_or_after_approval_never_sends_the_effect() {
    for completed in [false, true] {
        let (mut rig, _) = Rig::new(false);
        let ticket = rig.accept(42);
        rig.start(42, 11);
        if completed { rig.finish_review([Verdict::Allow; 2]); }
        rig.port.cancel(&ticket).unwrap();
        let result = rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap();
        assert!(matches!(result, DriverEvent::Stopped { state: ActionState::Cancelled, .. }));
        assert_eq!(rig.driver.phase(), DriverPhase::Idle);
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        assert_eq!(rig.driver.endpoint().execution_count(), 0);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    }
}

#[test]
fn missing_or_changed_current_evidence_cannot_be_replaced_by_retained_review_bytes() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(42);
    rig.start(42, 11);
    rig.finish_review([Verdict::Allow; 2]);
    assert_eq!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap_err(), DriverError::Control(Error::Incomplete));
    let mut changed = snapshot();
    changed.values.insert(7, vec![8]);
    assert!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &changed, None).is_err());
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::PublicationResolved { .. }));
}

#[test]
fn missing_dispatcher_fence_retains_one_reservation_for_repair_or_explicit_cancel() {
    for cancel in [false, true] {
        let (mut rig, _) = Rig::new(false);
        rig.accept(42);
        rig.start(42, 11);
        rig.finish_review([Verdict::Allow; 2]);
        rig.driver.supervisor_mut().broker_mut().restart_dispatcher().unwrap();
        for _ in 0..2 {
            assert_eq!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap_err(), DriverError::Control(Error::Incomplete));
            let ledger = rig.driver.supervisor().broker().inspect().ledger;
            assert_eq!((ledger.available, ledger.reserved, ledger.charged), (84, 16, 0));
        }
        if cancel {
            rig.driver.cancel_active().unwrap();
            assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
            assert_eq!(rig.driver.endpoint().execution_count(), 0);
        } else {
            rig.driver.confirm_dispatcher_fence().unwrap();
            assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::PublicationResolved { .. }));
            assert_eq!(rig.driver.endpoint().execution_count(), 1);
        }
    }
}

#[test]
fn two_key_mode_waits_without_reserving_then_requires_the_original_live_human_key() {
    for expire in [false, true] {
        let (mut rig, reviewer) = Rig::new(true);
        rig.accept(42);
        rig.start(42, 11);
        rig.finish_review([Verdict::Allow; 2]);
        assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::AwaitingHuman { request: 42 }));
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
        let request = rig.driver.request_human_approval(101, rig.inputs.as_ref(), ElapsedTick(10)).unwrap();
        let key = reviewer.unwrap().approve(&request, ElapsedTick(1)).unwrap();
        let result = rig.driver.step(ElapsedTick(if expire { 10 } else { 1 }), rig.inputs.as_ref(), &snapshot(), Some(&key));
        if expire {
            assert_eq!(result.unwrap_err(), DriverError::Control(Error::Stale));
            assert_eq!(rig.driver.endpoint().execution_count(), 0);
            assert_eq!(rig.driver.supervisor().broker().human_status(101).unwrap().disposition, HumanDisposition::Approved);
        } else {
            assert!(matches!(result.unwrap(), DriverEvent::PublicationResolved { .. }));
            assert_eq!(rig.driver.endpoint().execution_count(), 1);
            assert_eq!(rig.driver.supervisor().broker().human_status(101).unwrap().disposition, HumanDisposition::Consumed);
        }
    }
}

#[test]
fn a_missing_stream_cannot_change_the_roster_or_consume_the_input_revision() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(42);
    let mut launch = rig.launch(42, 11);
    launch.streams.remove("bob");
    assert_eq!(rig.driver.start_review(launch, &snapshot()).unwrap_err(), DriverError::Control(Error::Binding));
    let attempt = rig.driver.supervisor().attempt(42).unwrap();
    assert_eq!(rig.driver.supervisor().broker().input_revision(attempt).unwrap(), 0);
    rig.start(42, 11);
    rig.finish_review([Verdict::Allow; 2]);
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::PublicationResolved { .. }));
}

#[test]
fn disconnected_worker_remains_missing_while_the_healthy_worker_finishes() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(42);
    rig.start(42, 11);
    rig.clients.remove("alice");
    for now in [1, 5] {
        for _ in 0..24 {
            assert!(matches!(rig.tick(now, [Verdict::Allow; 2], None), DriverEvent::Workers { .. }));
        }
    }
    let result = rig.tick(10, [Verdict::Allow; 2], None);
    let DriverEvent::ReviewApplied { receipt, .. } = result else { panic!("{result:?}"); };
    assert_ne!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
}
