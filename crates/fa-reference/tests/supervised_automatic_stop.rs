//! Real numerical inference, socket congress, one-shot sends and file recovery.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/supervised_driver.rs"]
#[allow(dead_code)]
mod control;
#[path = "support/hosted_decoder.rs"]
#[allow(dead_code)]
mod numerical;
use control::{Rig, proposal, snapshot};
use fa_reference::action::consequence::activation::monitor::{RefinementMonitor, MonitorOutcome};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder, MonitoredSampledStep};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderModel, DecoderProfile, DecoderShape};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::delivery::{DispatchEnvelope, PublicationEndpoint, FilePublicationLimits};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, ActorTicket, Knowledge};
use fa_reference::action::consequence::oversight::decoder_host::{HostedStopCause, HostedStopPolicy};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::human::HumanReviewer;
use fa_reference::action::consequence::oversight::supervised::{DriverEvent, DriverPhase};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn policy() -> HostedStopPolicy { HostedStopPolicy::new(1, 1, 900).unwrap() }
fn install(rig: &mut Rig, run: MonitoredSampledDecoder) {
    let broker = rig.driver.supervisor_mut().broker_mut();
    broker.own_sampled_decoder(run, DecoderBindingLimits::default()).unwrap();
    broker.enable_hosted_stop(policy()).unwrap();
    let state = broker.hosted_decoder().unwrap();
    let result = rig.driver.advance_hosted_forced(state.actor_revision, state.position, 0,
        numerical::compute(), || ElapsedTick(1));
    assert!(matches!(result.inference.unwrap(), MonitoredStep::Released(_)));
    result.synchronization.unwrap(); assert!(result.containment.is_none());
}
fn sample_alarm() -> MonitoredSampledDecoder {
    let p = DecoderProfile::new(numerical::model().profile().identity(), DecoderShape {
        vocabulary: 2, hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16,
    }, 1e-5, 10000.0).unwrap();
    let layers = numerical::control::numerical::fixture::zero_layers(&p);
    let model = DecoderModel::new(p, vec![0.0, 1.0, 1.0, 0.0], layers, vec![1.0; 2],
        vec![0.0, 0.0, 0.0, 1.0]).unwrap();
    let allowance = numerical::control::numerical::allowance();
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(), &[1.0, 0.0], 0.0, 0.5).unwrap();
    let monitors = BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap())]);
    // A singleton original sampling distribution still spends one real draw.
    let start = SamplingStart { policy: SamplingPolicy::new(1, 1, 2, 1.0, 1, 1.0).unwrap(), stream: 99, seed: 0 };
    MonitoredSampledDecoder::new(model, 7, 11, monitors, allowance, start).unwrap()
}
fn send(rig: &mut Rig, reviewer: Option<&HumanReviewer>) -> (ActorTicket, DispatchEnvelope) {
    let ticket = rig.accept(1); rig.start(1, 11); rig.finish_review([Verdict::Allow; 2]);
    let input = rig.inputs.as_ref().unwrap().clone();
    let automatic = rig.driver.supervisor_mut().authorize_request(1, Some(&input), &snapshot()).unwrap();
    let human = reviewer.map(|reviewer| {
        let request = rig.driver.request_human_approval(101, Some(&input), ElapsedTick(50)).unwrap();
        reviewer.approve(&request, ElapsedTick(1)).unwrap()
    });
    let message = rig.driver.supervisor_mut().dispatch_request(1, DispatchKeys {
        automatic: &automatic, human: human.as_ref(),
    }, Some(&input), &snapshot()).unwrap();
    (ticket, message)
}
fn held(step: &MonitoredSampledStep) {
    assert!(matches!(step, MonitoredSampledStep::Held(_)));
    assert_eq!(step.review().outcome(), MonitorOutcome::Alarm);
}

#[test]
fn configured_quiet_sampling_preserves_original_logits_and_both_socket_reviewed_publications() {
    for two in [false, true] {
        let (mut rig, reviewer) = Rig::new(two);
        install(&mut rig, numerical::quiet());
        let mut reference = numerical::quiet();
        reference.advance_forced(0, 0, numerical::compute()).unwrap();
        let expected = reference.advance_sampled(1, numerical::budget(6)).unwrap();
        let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
        let result = rig.driver.advance_hosted_sampled(before.actor_revision, before.position,
            numerical::budget(6), || ElapsedTick(1));
        result.synchronization.unwrap(); assert!(result.containment.is_none());
        let (MonitoredSampledStep::Released(actual), MonitoredSampledStep::Released(expected)) = (result.inference.unwrap(), expected)
            else { panic!("quiet source did not release its computed sample"); };
        assert_eq!(actual.choice(), expected.choice());
        assert_eq!(actual.reviewed().step().logits.as_ref(), expected.reviewed().step().logits.as_ref());
        rig.accept(1); rig.start(1, 11); rig.finish_review([Verdict::Allow; 2]);
        let human = reviewer.as_ref().map(|reviewer| {
            let request = rig.driver.request_human_approval(101, rig.inputs.as_ref(), ElapsedTick(50)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), human.as_ref()).unwrap(), DriverEvent::PublicationResolved { .. }));
        assert_eq!(rig.driver.endpoint().execution_count(), 1);
        assert_eq!(rig.driver.endpoint().payload(), b"publish");
        assert!(rig.driver.supervisor().broker().hosted_stop_incident().is_none());
    }
}

#[test]
fn inference_trip_cancels_active_and_queued_work_without_waiting_for_helper_reveals() {
    let (mut rig, _) = Rig::new(false); install(&mut rig, numerical::alarm());
    let ticket = rig.accept(1); rig.start(1, 11);
    let queued = rig.port.submit(2, &proposal()).unwrap();
    let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    let result = rig.driver.advance_hosted_forced(before.actor_revision, before.position, 1,
        numerical::compute(), || ElapsedTick(1));
    assert!(matches!(result.inference.unwrap(), MonitoredStep::Held(_))); result.synchronization.unwrap();
    let sweep = result.containment.unwrap().unwrap(); assert!(sweep.progress.drained());
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert!(rig.driver.stop_progress().unwrap().quiesced());
    for ticket in [&ticket, &queued] {
        assert!(matches!(rig.port.poll(ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
    assert_eq!(rig.port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(rig.port.submit(1, &proposal()).unwrap().request(), 1);
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
}

#[test]
fn post_inference_clock_failure_keeps_draw_and_charge_until_a_real_endpoint_barrier() {
    for executed_during_gap in [false, true] {
        let (mut rig, _) = Rig::new(false); install(&mut rig, sample_alarm());
        let (ticket, delayed) = send(&mut rig, None);
        let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
        let mut calls = 0;
        let result = rig.driver.advance_hosted_sampled(before.actor_revision, before.position,
            numerical::budget(2), || { calls += 1; ElapsedTick(if calls == 1 { 1 } else { 0 }) });
        held(&result.inference.unwrap()); result.synchronization.unwrap();
        assert_eq!(calls, 2); assert_eq!(result.containment, Some(Err(Error::Stale)));
        assert_eq!(rig.driver.phase(), DriverPhase::Idle);
        assert!(!rig.driver.stop_progress().unwrap().effects.endpoint_fenced);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
        let state = rig.driver.supervisor().broker().hosted_decoder().unwrap();
        assert_eq!(state.sampled_draws, 1); assert_eq!(state.status, MonitoringStatus::Held);
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
        if executed_during_gap { let _lost = rig.driver.endpoint_mut().deliver(&delayed).unwrap(); }
        let sweep = rig.driver.service_hosted_stop(|| ElapsedTick(2)).unwrap().unwrap();
        assert!(sweep.progress.drained());
        assert_eq!(sweep.progress.charged_units, if executed_during_gap { 16 } else { 0 });
        let expected = if executed_during_gap { ActorOutcome::Executed } else { ActorOutcome::ConfirmedNotExecuted };
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value, .. } if value == expected));
        assert_eq!(rig.driver.endpoint_mut().deliver(&delayed), Err(Error::Stale));
        let retry = rig.driver.advance_hosted_sampled(state.actor_revision, state.position,
            numerical::budget(2), || ElapsedTick(2));
        assert!(matches!(retry.inference, Err(Error::WrongState)));
        assert!(retry.containment.unwrap().unwrap().outcomes.is_empty());
        assert_eq!(rig.driver.supervisor().broker().hosted_decoder().unwrap(), state);
        assert_eq!(rig.driver.endpoint().execution_count(), u64::from(executed_during_gap));
    }
}

#[test]
fn ordinary_step_services_lower_level_trip_even_without_current_input_or_human_role() {
    let (mut rig, reviewer) = Rig::new(true); install(&mut rig, sample_alarm());
    let (ticket, delayed) = send(&mut rig, reviewer.as_ref()); drop(reviewer);
    let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    held(&rig.driver.supervisor_mut().broker_mut().advance_hosted_sampled(before.actor_revision,
        before.position, numerical::budget(2)).unwrap());
    let incident = rig.driver.supervisor().broker().hosted_stop_incident().unwrap().clone();
    assert_eq!(incident.cause(), HostedStopCause::Monitoring(MonitorOutcome::Alarm));
    let work = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    rig.inputs = None; rig.clients.clear();
    let mut absent = snapshot(); absent.complete = false;
    let event = rig.driver.step(ElapsedTick(1), None, &absent, None).unwrap();
    let DriverEvent::HostedStop { sweep } = event else { panic!("automatic stop not serviced"); };
    assert!(sweep.progress.drained()); assert_eq!(sweep.progress.charged_units, 0);
    assert!(rig.driver.supervisor().broker().human_review_required());
    assert_eq!(rig.driver.supervisor().broker().hosted_stop_incident(), Some(&incident));
    assert_eq!(rig.driver.supervisor().broker().hosted_decoder().unwrap(), work);
    assert_eq!(rig.driver.endpoint_mut().deliver(&delayed), Err(Error::Stale));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
}

#[test]
fn fresh_post_inference_retention_expiry_remains_unresolved_instead_of_refunded() {
    let (mut rig, _) = Rig::new(false); install(&mut rig, sample_alarm());
    let (ticket, delayed) = send(&mut rig, None);
    let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    let mut first = true;
    let result = rig.driver.advance_hosted_sampled(before.actor_revision, before.position,
        numerical::budget(2), || { if std::mem::take(&mut first) { ElapsedTick(1) } else { delayed.retained_until() } });
    held(&result.inference.unwrap());
    let sweep = result.containment.unwrap().unwrap();
    assert!(sweep.progress.endpoint_fenced); assert!(!sweep.progress.drained());
    assert_eq!(sweep.progress.unresolved, vec![1]); assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.stages[&1], ActionState::Unknown);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-auto-stop-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("automatic-stop fixture cleanup: {error}"); } }
}

#[test]
fn failed_file_fence_reconnects_the_same_stopped_owner_and_recovers_both_real_outcomes() {
    for executed in [false, true] {
        let root = Directory::new();
        let (endpoint, key) = PublicationEndpoint::create_file_publication(root.0.join("endpoint"), control::target(),
            b"old".to_vec(), 200, 16, FilePublicationLimits { mutations: 256, bytes: 2_097_152 }).unwrap();
        let (mut rig, reviewer) = Rig::with_endpoint(endpoint, true); install(&mut rig, sample_alarm());
        let (ticket, delayed) = send(&mut rig, reviewer.as_ref()); drop(reviewer);
        if executed { let _lost = rig.driver.endpoint_mut().deliver(&delayed).unwrap(); }
        fs::write(key.directory().join("publication.pending"), b"occupied staging file").unwrap();
        let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
        let result = rig.driver.advance_hosted_sampled(before.actor_revision, before.position,
            numerical::budget(2), || ElapsedTick(1));
        held(&result.inference.unwrap()); result.synchronization.unwrap();
        assert_eq!(result.containment, Some(Err(Error::Incomplete)));
        assert!(!rig.driver.stop_progress().unwrap().effects.endpoint_fenced);
        assert_eq!(rig.driver.supervisor().broker().inspect().ledger.charged, 16);
        let incident = rig.driver.supervisor().broker().hosted_stop_incident().unwrap().clone();
        let (offline, endpoint) = rig.driver.detach_endpoint(); drop(endpoint);
        let endpoint = key.reopen().unwrap();
        let mut driver = offline.reconnect(endpoint, ElapsedTick(1)).unwrap();
        let mut absent = snapshot(); absent.complete = false;
        let DriverEvent::HostedStop { sweep } = driver.step(ElapsedTick(1), None, &absent, None).unwrap()
            else { panic!("reconnected stop was not serviced"); };
        assert!(sweep.progress.drained()); assert_eq!(sweep.progress.charged_units, if executed { 16 } else { 0 });
        assert_eq!(driver.supervisor().broker().hosted_stop_incident(), Some(&incident));
        assert_eq!(driver.supervisor().broker().hosted_decoder().unwrap().sampled_draws, 1);
        assert_eq!(driver.endpoint().execution_count(), u64::from(executed));
        assert_eq!(driver.endpoint_mut().deliver(&delayed), Err(Error::Stale));
        let expected = if executed { ActorOutcome::Executed } else { ActorOutcome::ConfirmedNotExecuted };
        assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value, .. } if value == expected));
        assert_eq!(rig.port.submit(2, &proposal()).unwrap_err(), ActorError::Unavailable);
        assert!(driver.service_hosted_stop(|| ElapsedTick(1)).unwrap().unwrap().outcomes.is_empty());
    }
}

#[test]
fn ordinary_clock_refusal_does_not_trigger_policy_or_cancel_an_unrelated_healthy_review() {
    let (mut rig, _) = Rig::new(false); install(&mut rig, numerical::quiet());
    let ticket = rig.accept(1); rig.start(1, 11);
    let before = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    let result = rig.driver.advance_hosted_forced(before.actor_revision, before.position, 1,
        numerical::compute(), || ElapsedTick(0));
    assert!(matches!(result.inference, Err(Error::Stale))); result.synchronization.unwrap();
    assert!(result.containment.is_none());
    assert_eq!(rig.driver.phase(), DriverPhase::Reviewing { request: 1 });
    assert_eq!(rig.driver.supervisor().broker().hosted_decoder().unwrap(), before);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Pending { .. }));
    assert!(rig.driver.supervisor().broker().hosted_stop_incident().is_none());
    rig.finish_review([Verdict::Allow; 2]);
    assert!(matches!(rig.driver.step(ElapsedTick(1), rig.inputs.as_ref(), &snapshot(), None).unwrap(), DriverEvent::PublicationResolved { .. }));
}
