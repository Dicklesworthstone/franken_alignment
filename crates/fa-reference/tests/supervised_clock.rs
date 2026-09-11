//! Fresh-clock and missing-evidence controls at the integrated dispatch boundary.
#![cfg(unix)]
#[path = "support/supervised_driver.rs"]
mod support;
use support::{Rig, proposal, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::helper_client::ClientProgress;
use fa_reference::action::consequence::oversight::supervised::{DriverEvent, DriverPhase};
use fa_reference::action::ElapsedTick;
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn a_clock_crossing_the_deadline_after_reservation_cannot_backdate_dispatch() {
    for final_tick in [99, 100] {
        let (mut rig, _) = Rig::new(false);
        rig.accept(42);
        rig.start(42, 11);
        rig.finish_review([Verdict::Allow; 2]);
        let mut samples = 0;
        let result = rig.driver.step_with_clock(|| {
            samples += 1;
            ElapsedTick(if samples == 1 { 1 } else { final_tick })
        }, rig.inputs.as_ref(), &snapshot(), None);
        assert_eq!(samples, 2);
        if final_tick == 99 {
            assert!(matches!(result.unwrap(), DriverEvent::PublicationResolved { .. }));
            assert_eq!(rig.driver.endpoint().execution_count(), 1);
        } else {
            assert!(result.is_err());
            let ledger = rig.driver.supervisor().broker().inspect().ledger;
            assert_eq!((ledger.available, ledger.reserved, ledger.charged), (84, 16, 0));
            assert_eq!(ledger.elapsed, Some(ElapsedTick(100)));
            assert_eq!(rig.driver.endpoint().execution_count(), 0);
            assert!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).is_err());
            rig.driver.cancel_active().unwrap();
            assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
        }
    }
}

#[test]
fn a_clock_rollback_after_reservation_preserves_the_one_original_permit() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(42);
    rig.start(42, 11);
    rig.finish_review([Verdict::Allow; 2]);
    let mut ticks = [ElapsedTick(2), ElapsedTick(1)].into_iter();
    assert!(rig.driver.step_with_clock(|| ticks.next().expect("only two dispatch observations"),
        rig.inputs.as_ref(), &snapshot(), None).is_err());
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert!(matches!(rig.driver.step(ElapsedTick(2), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::PublicationResolved { .. }));
    let ledger = rig.driver.supervisor().broker().inspect().ledger;
    assert_eq!((ledger.available, ledger.reserved, ledger.charged), (84, 0, 16));
}

#[test]
fn missing_current_input_refuses_continue_but_does_not_block_a_restrictive_review() {
    for verdict in [Verdict::Allow, Verdict::Hold] {
        let (mut rig, _) = Rig::new(false);
        rig.accept(42);
        rig.start(42, 11);
        let mut completed = false;
        for _ in 0..128 {
            for (member, client) in &mut rig.clients {
                if client.step().unwrap() == ClientProgress::NeedsInference {
                    assert_eq!(client.input().unwrap().actual_input(), rig.inputs.as_ref().unwrap().views()[member].actual_input());
                    client.respond(verdict, member.as_bytes()).unwrap();
                }
            }
            match rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap() {
                DriverEvent::Workers { .. } => {}
                DriverEvent::ReviewRejected { error: Error::Incomplete, .. } if verdict == Verdict::Allow => {
                    completed = true;
                    break;
                }
                DriverEvent::ReviewApplied { receipt, .. } if verdict == Verdict::Hold => {
                    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
                    completed = true;
                    break;
                }
                other => panic!("unexpected completion: {other:?}"),
            }
        }
        assert!(completed);
        assert_eq!(rig.driver.phase(), DriverPhase::Idle);
        assert!(rig.driver.supervisor_mut().authorize_request(42, rig.inputs.as_ref(), &snapshot()).is_err());
        assert_eq!(rig.driver.endpoint().execution_count(), 0);
        assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::Idle));
    }
}

#[test]
fn queued_actor_work_is_not_dequeued_during_an_active_control_predecessor() {
    let (mut rig, _) = Rig::new(false);
    rig.accept(42);
    rig.start(42, 11);
    rig.port.submit(9, &proposal()).unwrap();
    assert_eq!(rig.driver.accept_next(&snapshot()).unwrap_err(), Error::WrongState);
    rig.finish_review([Verdict::Hold; 2]);
    let intake = rig.driver.accept_next(&snapshot()).unwrap().unwrap();
    assert_eq!(intake.request, 9);
    assert!(intake.result.unwrap().is_some());
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
}
