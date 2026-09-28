//! Original learned checkpoint/reset through the existing mailbox, real helper
//! sockets and retained file endpoint. Tiny weights and supplied ballots are
//! deterministic controls, not empirical model or restart qualification.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
#[allow(dead_code)]
mod numerical;
#[path = "support/supervised_learned.rs"]
mod driving;

use driving::{Rig, proposal, snapshot, target};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;
use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, FilePublicationLimits, PublicationEndpoint};
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::learned_host::LearnedHostStopCause;
use fa_reference::action::consequence::oversight::learned_source::LearnedAvailability;
use fa_reference::action::consequence::oversight::supervised::{DriverError, DriverEvent, DriverPhase};
use fa_reference::Error;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn learned_reset_preserves_actor_keys_then_fresh_inference_and_socket_review_publish() {
    for two_key in [false, true] {
        let (mut rig, reviewer) = Rig::new(two_key, 0, false);
        rig.advance();
        let checkpoint = rig.checkpoint(1);
        let old_source = rig.driver.supervisor().broker().hosted_learned_observation().unwrap();
        let abandoned_next = rig.advance();
        let before = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
        let old = rig.accept(1);
        rig.start(1, 1);
        let queued = rig.port.submit(2, &proposal()).unwrap();
        let request = rig.request(&checkpoint, 100);
        let reset = rig.driver.reset_hosted_learned(request).unwrap();
        assert!(reset.control.restored);
        assert_eq!(reset.control.cancelled, vec![1]);
        assert_eq!(reset.position, 1);
        assert_eq!(reset.resumed_stream, Some(22));
        assert_eq!(rig.driver.phase(), DriverPhase::Idle);
        assert_eq!(old_source.availability(), LearnedAvailability::Closed);
        assert_eq!(rig.driver.supervisor().broker().hosted_learned_generation().unwrap().cumulative_work,
            before.cumulative_work);
        for ticket in [&old, &queued] {
            assert!(matches!(rig.port.poll(ticket), Knowledge::Known {
                value: ActorOutcome::CancelledBeforeDispatch, ..
            }));
        }
        assert_eq!(rig.port.submit(1, &proposal()).unwrap().request(), old.request());
        assert_eq!(rig.port.submit(2, &proposal()).unwrap().request(), queued.request());
        assert!(rig.driver.accept_next(&snapshot()).unwrap().is_none());
        rig.assert_empty_source();
        let fresh_next = rig.advance();
        assert_eq!(fresh_next.sample(), abandoned_next.sample());
        assert_eq!(numerical::logits(&fresh_next.accepted().unwrap().logits),
            numerical::logits(&abandoned_next.accepted().unwrap().logits));
        let mut next = proposal();
        next.expected_policy_epoch = reset.control.revocation_floor;
        let fresh = rig.port.submit(3, &next).unwrap();
        let intake = rig.driver.accept_next(&snapshot()).unwrap().unwrap();
        assert_eq!(intake.request, 3);
        assert!(intake.result.unwrap().is_some());
        rig.start(3, 3);
        rig.finish_review();
        let human = reviewer.as_ref().map(|reviewer| {
            let request = rig.driver.request_human_approval(500, rig.inputs.as_ref(), ElapsedTick(40)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), human.as_ref()).unwrap(),
            DriverEvent::PublicationResolved { .. }));
        assert!(matches!(rig.port.poll(&fresh), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(rig.driver.endpoint().execution_count(), 1);
        assert_eq!(rig.driver.endpoint().payload(), b"publish");
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    }
}

#[test]
fn learned_reset_releases_the_original_reserved_permit_and_never_resends_it() {
    let (mut rig, _) = Rig::new(false, 0, false);
    rig.advance();
    let checkpoint = rig.checkpoint(1);
    let ticket = rig.accept(1);
    rig.start(1, 1);
    rig.finish_review();
    rig.driver.supervisor_mut().broker_mut().restart_dispatcher().unwrap();
    assert_eq!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap_err(),
        DriverError::Control(Error::Incomplete));
    assert_eq!(rig.driver.phase(), DriverPhase::AwaitingDispatch { request: 1 });
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 16);
    let request = rig.request(&checkpoint, 100);
    let reset = rig.driver.reset_hosted_learned(request).unwrap();
    assert_eq!(reset.control.refunded_units, 16);
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.reserved, 0);
    rig.driver.confirm_dispatcher_fence().unwrap();
    assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(), DriverEvent::Idle));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known {
        value: ActorOutcome::CancelledBeforeDispatch, ..
    }));
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}

#[test]
fn refused_preflight_preserves_real_review_but_successful_actor_reset_retires_its_queue() {
    for restore in [false, true] {
        let (mut rig, _) = Rig::new(false, 0, false);
        rig.advance();
        let revision = rig.driver.supervisor().broker().actor_revision();
        let checkpoint = rig.driver.supervisor_mut().capture_hosted_learned_checkpoint(1, revision).unwrap();
        let ticket = rig.accept(1);
        rig.start(1, 1);
        let queued = rig.port.submit(2, &proposal()).unwrap();
        let before = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
        let usage = rig.driver.supervisor().broker().hosted_learned_recovery_usage().unwrap();
        for invalid in 0..5 {
            let mut bad = rig.request(&checkpoint, 100);
            let expected = match invalid {
                0 => { bad.expected_actor_revision += 1; Error::Stale }
                1 => { bad.expected_control_sequence += 1; Error::Stale }
                2 => { bad.expected_authority_epoch += 1; Error::Stale }
                3 => { bad.binding.evidence_root = [0; 32]; Error::InvalidInput }
                _ => { bad.restart_budget.cache_values = 0; Error::Limit }
            };
            assert_eq!(rig.driver.reset_hosted_learned(bad).unwrap_err(), expected);
            assert_eq!(rig.driver.phase(), DriverPhase::Reviewing { request: 1 });
            assert_eq!(rig.driver.supervisor().broker().hosted_learned_generation().unwrap(), before);
            assert_eq!(rig.driver.supervisor().broker().hosted_learned_recovery_usage().unwrap(), usage);
            assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
            assert!(matches!(rig.port.poll(&queued), Knowledge::Pending { .. }));
        }
        let mut bad = rig.request(&checkpoint, 100);
        bad.expected_actor_revision += 1;
        assert_eq!(rig.driver.supervisor_mut().reset_hosted_learned(bad).unwrap_err(), Error::Stale);
        if restore {
            let request = rig.request(&checkpoint, 100);
            let reset = rig.driver.supervisor_mut().reset_hosted_learned(request).unwrap();
            assert!(reset.control.restored);
            for ticket in [&ticket, &queued] {
                assert!(matches!(rig.port.poll(ticket), Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch, ..
                }));
            }
            // The lower-level supervisor did not replace the driver. Its next
            // ordinary step observes the original cancelled job without input.
            assert!(matches!(rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap(),
                DriverEvent::Stopped { request: 1, state: ActionState::Cancelled }));
            assert_eq!(rig.driver.phase(), DriverPhase::Idle);
            assert_eq!(rig.driver.endpoint().execution_count(), 0);
        } else {
            rig.finish_review();
            assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(),
                DriverEvent::PublicationResolved { .. }));
            assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
            assert!(matches!(rig.port.poll(&queued), Knowledge::Pending { .. }));
            assert_eq!(rig.driver.endpoint().execution_count(), 1);
        }
        assert_eq!(rig.port.submit(1, &proposal()).unwrap().request(), 1);
        assert_eq!(rig.port.submit(2, &proposal()).unwrap().request(), 2);
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-supervised-learned-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("learned reset fixture cleanup failed: {error}"); }
    }
}

#[test]
fn offline_learned_reset_keeps_unknown_effect_charged_until_original_file_reconciliation() {
    for executed in [false, true] {
        let directory = Directory::new();
        let (endpoint, key) = PublicationEndpoint::create_file_publication(directory.0.join("publication"),
            target(), b"old".to_vec(), 200, 16,
            FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
        let (mut rig, reviewer) = Rig::with_endpoint(endpoint, true, 0, false);
        rig.advance();
        let revision = rig.driver.supervisor().broker().actor_revision();
        let (mut offline, endpoint) = rig.driver.detach_endpoint();
        let checkpoint = offline.capture_hosted_learned_checkpoint(1, revision).unwrap();
        rig.driver = offline.reconnect(endpoint, ElapsedTick(1)).unwrap();
        let ticket = rig.accept(1);
        rig.start(1, 1);
        rig.finish_review();
        let delayed = rig.dispatch(1, reviewer.as_ref());
        if executed { let _lost = rig.driver.endpoint_mut().deliver(&delayed).unwrap(); }
        rig.driver.supervisor_mut().acknowledgment_lost(1).unwrap();
        let queued = rig.port.submit(2, &proposal()).unwrap();
        rig.remove_live_inputs(1);
        drop(reviewer);
        let request = rig.request(&checkpoint, 100);
        let before = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
        let (mut offline, endpoint) = rig.driver.detach_endpoint();
        let mut bad = request.clone();
        bad.expected_authority_epoch += 1;
        assert_eq!(offline.reset_hosted_learned(bad).unwrap_err(), Error::Stale);
        assert_eq!(offline.supervisor().broker().hosted_learned_generation().unwrap(), before);
        assert!(matches!(rig.port.poll(&queued), Knowledge::Pending { .. }));
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
        let reset = offline.reset_hosted_learned(request).unwrap();
        assert!(reset.control.restored);
        assert_eq!(reset.control.refunded_units, 0);
        assert!(reset.control.cancelled.is_empty());
        assert_eq!(offline.supervisor().broker().inspect().ledger.stages[&1], ActionState::Unknown);
        assert_eq!(offline.supervisor().broker().inspect().ledger.charged, 16);
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
        assert!(matches!(rig.port.poll(&queued), Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch, ..
        }));
        // No endpoint access occurred during reset. Reopening the original
        // retained file authority and fencing it precedes either settlement.
        drop(endpoint);
        let endpoint = key.reopen().unwrap();
        let mut driver = offline.reconnect(endpoint, ElapsedTick(2)).unwrap();
        assert_eq!(driver.phase(), DriverPhase::Idle);
        assert_eq!(driver.endpoint_mut().deliver(&delayed), Err(Error::Stale));
        let reconciled = driver.reconcile_pending(ElapsedTick(2)).unwrap();
        assert_eq!(reconciled.len(), 1);
        let completed_at = if executed { ElapsedTick(2) } else { delayed.request().execution_deadline() };
        let settled = if executed { reconciled } else {
            // A fence is not a terminal nonexecution receipt. The original
            // expiry protocol must settle the absent but still-valid dispatch.
            assert_eq!(reconciled[&1], Ok(EndpointStatus::AwaitingResolution));
            assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
            assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
            assert!(completed_at < delayed.retained_until());
            driver.reconcile_pending(completed_at).unwrap()
        };
        let Ok(EndpointStatus::Resolved(receipt)) = &settled[&1] else { panic!("original endpoint did not resolve: {settled:?}"); };
        assert!(if executed { matches!(receipt.outcome(), EndpointOutcome::Executed { .. }) }
            else { matches!(receipt.outcome(), EndpointOutcome::NotExecuted { .. }) });
        assert!(driver.reconcile_pending(completed_at).unwrap().is_empty());
        let outcome = if executed { ActorOutcome::Executed } else { ActorOutcome::ConfirmedNotExecuted };
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value, .. } if value == outcome));
        assert_eq!(driver.endpoint().execution_count(), u64::from(executed));
        assert_eq!(driver.endpoint().payload(), if executed { b"publish".as_slice() } else { b"old".as_slice() });
        assert_eq!(driver.supervisor().broker().inspect().ledger.charged, if executed { 16 } else { 0 });
        assert_eq!(driver.supervisor().broker().dispatched_learned_decoder_evidence(1).unwrap().unwrap().stream(), 21);
        assert!(driver.supervisor().broker().human_review_required());
        assert_eq!(rig.port.submit(1, &proposal()).unwrap().request(), 1);
        assert_eq!(rig.port.submit(2, &proposal()).unwrap().request(), 2);
    }
}

#[test]
fn computed_learned_alarm_cancels_unfinished_socket_review_and_queued_intake() {
    let (mut rig, _) = Rig::new(false, 2, true);
    rig.advance();
    let ticket = rig.accept(1);
    rig.start(1, 1);
    let queued = rig.port.submit(2, &proposal()).unwrap();
    let before = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
    let result = rig.driver.advance_hosted_learned(before.actor_revision, before.position, || ElapsedTick(1));
    assert!(result.inference.unwrap().accepted().is_none());
    result.synchronization.unwrap();
    let sweep = result.containment.unwrap().unwrap();
    assert!(sweep.progress.drained());
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert!(rig.driver.stop_progress().unwrap().quiesced());
    assert_eq!(rig.driver.supervisor().broker().learned_host_stop_incident().unwrap().cause(),
        LearnedHostStopCause::Monitoring(MonitorOutcome::Alarm));
    assert_eq!(rig.driver.supervisor().broker().hosted_learned_generation().unwrap().status,
        GenerationStatus::Held(MonitorOutcome::Alarm));
    for ticket in [&ticket, &queued] {
        assert!(matches!(rig.port.poll(ticket), Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch, ..
        }));
    }
    assert_eq!(rig.port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(rig.port.submit(1, &proposal()).unwrap().request(), 1);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
}

#[test]
fn failed_post_inference_clock_keeps_real_learned_work_and_unknown_charge_until_fence() {
    for executed_during_gap in [false, true] {
        let (mut rig, _) = Rig::new(false, 2, true);
        rig.advance();
        let ticket = rig.accept(1);
        rig.start(1, 1);
        rig.finish_review();
        let delayed = rig.dispatch(1, None);
        let before = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
        let mut calls = 0;
        let result = rig.driver.advance_hosted_learned(before.actor_revision, before.position, || {
            calls += 1;
            ElapsedTick(if calls == 1 { 1 } else { 0 })
        });
        assert!(result.inference.unwrap().accepted().is_none());
        result.synchronization.unwrap();
        assert_eq!(calls, 2);
        assert_eq!(result.containment, Some(Err(Error::Stale)));
        assert_eq!(rig.driver.phase(), DriverPhase::Idle);
        assert!(!rig.driver.stop_progress().unwrap().effects.endpoint_fenced);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
        let held = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
        assert_eq!(held.status, GenerationStatus::Held(MonitorOutcome::Alarm));
        assert!(held.cumulative_work.reserved_decoder_products > before.cumulative_work.reserved_decoder_products);
        if executed_during_gap { let _lost = rig.driver.endpoint_mut().deliver(&delayed).unwrap(); }
        let sweep = rig.driver.service_hosted_stop(|| ElapsedTick(2)).unwrap().unwrap();
        assert!(sweep.progress.drained());
        assert_eq!(sweep.progress.charged_units, if executed_during_gap { 16 } else { 0 });
        let outcome = if executed_during_gap { ActorOutcome::Executed } else { ActorOutcome::ConfirmedNotExecuted };
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value, .. } if value == outcome));
        assert_eq!(rig.driver.endpoint_mut().deliver(&delayed), Err(Error::Stale));
        let retry = rig.driver.advance_hosted_learned(held.actor_revision, held.position, || ElapsedTick(2));
        assert_eq!(retry.inference.err(), Some(Error::WrongState));
        assert!(retry.containment.unwrap().unwrap().outcomes.is_empty());
        assert_eq!(rig.driver.supervisor().broker().hosted_learned_generation().unwrap(), held);
        assert_eq!(rig.driver.endpoint().execution_count(), u64::from(executed_during_gap));
    }
}

#[test]
fn ordinary_driver_step_services_original_learned_stop_without_live_input_or_human_role() {
    let (mut rig, reviewer) = Rig::new(true, 2, true);
    rig.advance();
    let ticket = rig.accept(1);
    rig.start(1, 1);
    rig.finish_review();
    let delayed = rig.dispatch(1, reviewer.as_ref());
    let queued = rig.port.submit(2, &proposal()).unwrap();
    rig.remove_live_inputs(1);
    drop(reviewer);
    let before = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
    let held = rig.driver.supervisor_mut().broker_mut().advance_hosted_learned(
        before.actor_revision, before.position).unwrap();
    assert!(held.accepted().is_none());
    let incident = rig.driver.supervisor().broker().learned_host_stop_incident().unwrap().clone();
    let work = rig.driver.supervisor().broker().hosted_learned_generation().unwrap();
    assert!(matches!(rig.port.poll(&queued), Knowledge::Pending { .. }));
    // Capture is also a supervision boundary: its refusal must publish the
    // actual lower-level stop and release the old job before returning.
    assert_eq!(rig.driver.capture_hosted_learned_checkpoint(1, work.actor_revision).unwrap_err(), Error::WrongState);
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert!(matches!(rig.port.poll(&queued), Knowledge::Known {
        value: ActorOutcome::CancelledBeforeDispatch, ..
    }));
    assert_eq!(rig.port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
    let mut absent = snapshot();
    absent.complete = false;
    let event = rig.driver.step(ElapsedTick(1), None, &absent, None).unwrap();
    let DriverEvent::HostedStop { sweep } = event else { panic!("original learned stop was not serviced"); };
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 0);
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert!(rig.driver.stop_progress().unwrap().quiesced());
    assert!(rig.driver.supervisor().broker().human_review_required());
    assert_eq!(rig.driver.supervisor().broker().learned_host_stop_incident(), Some(&incident));
    assert_eq!(rig.driver.supervisor().broker().hosted_learned_generation().unwrap(), work);
    assert_eq!(rig.driver.endpoint_mut().deliver(&delayed), Err(Error::Stale));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known {
        value: ActorOutcome::ConfirmedNotExecuted, ..
    }));
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}
