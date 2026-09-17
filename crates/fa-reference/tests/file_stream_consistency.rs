//! Message-aware prediction through the original wire, approval and publication path.
#![cfg(unix)]
#[path = "support/file_stream_wire.rs"] mod fixture;
#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod data;
use fixture::{Directory, profile, stream, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::stream::StreamView;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::*;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::*;
use fa_reference::action::consequence::oversight::actor::{Knowledge, ActorOutcome, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, encode_command};
use fa_reference::action::consequence::oversight::consistency::ConsistencyEventDomain;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};

fn capture() -> CaptureProfile { CaptureProfile { tenant: 1, model: 2, model_generation: 1, tap: 4, layout_generation: 5 } }
fn config(owned: bool, messages: bool, alpha: u64) -> FileConsistencyConfig {
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    let mut config = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 7, probe_generation: 1,
        profile: if owned { numerical::model().residual_contract(1).unwrap().profile() } else { capture() },
        weights: if owned { vec![1.0, 0.0] } else { vec![1.0] }, bias: 0.0, threshold: 1.5,
        forecast: ForecastRegistration { domain: 8, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, alpha).unwrap(), stream: if owned { 5 } else { 7 },
        max_predictions: 16, max_prediction_age_ticks: 10 }).unwrap();
    if owned { config = config.with_hosted_residual(1).unwrap(); }
    if messages { config = config.with_stream_messages(stream()).unwrap(); }
    config
}
fn guards(owned: bool) -> FileGuardSet {
    FileGuardSet { stream: Some(stream()), decoder: owned.then(|| FileDecoderConfig::new(
        numerical::profile(), data::weights(), data::monitor(100.0), data::sampling(), 5,
        DecoderBindingLimits::default()).unwrap()), decoder_stop: None,
        source: None, identity: None, campaigns: None, credential: None }
}
fn create(root: &Directory, owned: bool, messages: bool, alpha: u64) -> (FileOversight, FilePredictiveRoles) {
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), profile(), &guards(owned),
        None, config(owned, messages, alpha), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); (h, roles)
}
fn forecast(h: &mut FileOversight, role: &FileConsistencyObserver, request: u64, sequence: u64) {
    let frame = SourceFrame::capture(FrameIdentity { profile: capture(), stream: 7, sequence, position: 0 }, &[-1.0]).unwrap();
    let rev = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    role.forecast_request(h, rev, request, actor, &frame).unwrap().unwrap();
}
fn expected(h: &FileOversight, owned: bool, messages: bool, alpha: u64) -> FilePredictiveRequirements {
    let c = h.inspect().control;
    FilePredictiveRequirements { oversight: FileRecoveryRequirements { guards: guards(owned),
        effective_policy: h.current_policy().unwrap().clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: h.revision(), control_sequence: c.sequence, authority_epoch: c.ledger.epoch } },
        prediction: config(owned, messages, alpha), evaluation: None }
}
fn publish(h: &mut FileOversight, human: &fa_reference::action::consequence::delivery::persistent::observed::FileHumanReviewer,
    request: u64, attempt: u64) {
    let keys = fixture::approve(h, human, request, attempt); fixture::ordinary::dispatch(h, &keys);
    let now = h.inspect().control.ledger.elapsed.unwrap();
    let result = h.publish_checked(h.revision(), attempt, Some(&keys.inputs), snapshot(), now).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: attempt + 1 });
    assert_eq!(h.reconcile(h.revision(), attempt).unwrap(), Reconciliation::Resolved(result.outcome));
}

#[test]
fn wire_messages_and_explicit_finish_count_only_new_content_and_exact_retries_count_once() {
    let root = Directory::new(); let (h, roles) = create(&root, false, true, 100);
    let (port, mut supervisor) = h.into_stream_actor_gateway().unwrap(); let mut wire = ActorWire::new(port);
    let mut commands = Vec::new();
    for (attempt, message, event) in [(1, Some("risk: α"), true), (2, Some("ordinary β"), false), (3, None, false)] {
        let request = 9000 + attempt;
        { let mut h = supervisor.host_mut().unwrap(); forecast(&mut h, &roles.consistency_observer, request, attempt); }
        let command = fixture::command(&supervisor.host().unwrap(), request, message); fixture::admit(&mut supervisor);
        assert!(matches!(wire.exchange(&encode_command(&command).unwrap()).result, Ok(Knowledge::Pending { .. })));
        {
            let mut h = supervisor.host_mut().unwrap();
            let observed = h.action_consistency_observation(attempt).unwrap();
            assert_eq!(observed.event(), event);
            assert_eq!(observed.event_domain(), ConsistencyEventDomain::StreamMessagePrefix(stream()));
            let keys = fixture::approve(&mut h, &roles.oversight.human, request, attempt);
            fixture::ordinary::dispatch(&mut h, &keys);
            let rev = h.revision();
            let result = h.publish_checked(rev, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
            assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: attempt + 1 });
        }
        let poll = encode_command(&Command::Poll { request }).unwrap();
        assert_eq!(wire.exchange(&poll).result, Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        { let mut h = supervisor.host_mut().unwrap(); let rev = h.revision(); h.reconcile(rev, attempt).unwrap(); }
        assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        commands.push(command);
    }
    let before = supervisor.host().unwrap().action_consistency_snapshot().unwrap();
    for command in commands { assert!(wire.exchange(&encode_command(&command).unwrap()).result.is_ok()); }
    let h = supervisor.host().unwrap(); assert_eq!(h.action_consistency_snapshot().unwrap(), before);
    assert_eq!(before.evidence.samples(), 3); assert!(!before.evidence.crossed());
    let view = h.stream_snapshot().unwrap().confirmed;
    assert!(view.finished()); assert_eq!(view.messages().collect::<Vec<_>>(), vec!["risk: α", "ordinary β"]);
}

#[test]
fn message_crossing_seals_a_prior_dispatch_but_raw_header_mode_remains_unchanged() {
    for messages in [false, true] {
        let root = Directory::new(); let (mut h, roles) = create(&root, false, messages, 4);
        forecast(&mut h, &roles.consistency_observer, 9000, 1);
        let spec = h.stream_message_spec("risk: first", ElapsedTick(100)).unwrap();
        h.submit_request(h.revision(), 9000, spec, snapshot()).unwrap();
        let keys = fixture::approve(&mut h, &roles.oversight.human, 9000, 1); fixture::ordinary::dispatch(&mut h, &keys);
        let charged = h.inspect().control.ledger.charged; assert!(charged > 0);
        forecast(&mut h, &roles.consistency_observer, 9001, 2);
        // A second well-framed proposed message is observable even though the
        // original stream gate refuses admission while the first is outstanding.
        let mut next = keys.action.spec().clone();
        next.payload = StreamView::empty(stream()).encode_message("risk: second").unwrap(); next.units = next.payload.len() as u64;
        let status = h.submit_request(h.revision(), 9001, next.clone(), snapshot()).unwrap();
        assert!(matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)));
        assert_eq!(h.action_consistency_observation(2).unwrap().event(), messages);
        assert_eq!(h.action_consistency_snapshot().unwrap().evidence.crossed(), messages);
        let before = h.action_consistency_snapshot().unwrap();
        assert_eq!(h.submit_request(0, 9001, next, snapshot()).unwrap(), status);
        assert_eq!(h.action_consistency_snapshot().unwrap(), before);
        let result = h.publish_checked(h.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        assert_eq!(matches!(result.outcome, EndpointOutcome::Executed { .. }), !messages);
        assert_eq!(h.inspect().control.ledger.charged, charged);
        assert_eq!(h.reconcile(h.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
        assert_eq!(h.inspect().control.ledger.charged, if messages { 0 } else { charged });
    }
}

#[test]
fn recovery_pins_message_semantics_and_owned_activation_before_fresh_numerical_continuation() {
    let root = Directory::new(); let (mut h, roles) = create(&root, true, true, 100);
    let n = h.decoder_inspection().unwrap().numerical;
    h.advance_decoder_forced(h.revision(), n.actor_revision, n.position, 0, numerical::budget()).unwrap().unwrap();
    let rev = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    roles.consistency_observer.forecast_hosted_request(&mut h, rev, 9000, actor).unwrap().unwrap();
    let spec = h.stream_message_spec("risk: first", ElapsedTick(100)).unwrap(); h.submit_request(h.revision(), 9000, spec, snapshot()).unwrap();
    publish(&mut h, &roles.oversight.human, 9000, 1);
    let required = expected(&h, true, true, 100); let evidence = h.action_consistency_snapshot().unwrap().evidence; drop(h);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"preserve on mismatch").unwrap();
    let mut wrong = required.clone(); wrong.prediction = config(true, false, 100);
    assert!(FileOversight::open_predictive_guarded(root.store(), profile(), &wrong).is_err());
    assert_eq!(std::fs::read(&pending).unwrap(), b"preserve on mismatch");
    let (mut h, fresh) = FileOversight::open_predictive_guarded(root.store(), profile(), &required).unwrap();
    assert_eq!(h.action_consistency_snapshot().unwrap().evidence, evidence); assert!(h.decoder_inspection().unwrap().paused);
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap(); let n = h.decoder_inspection().unwrap().numerical;
    h.resume_decoder(h.revision(), n.actor_revision, n.position).unwrap();
    let n = h.decoder_inspection().unwrap().numerical;
    h.advance_decoder_sampled(h.revision(), n.actor_revision, n.position, SampleBudget {
        decoder: numerical::budget(), sampling: SamplingBudget { vocabulary: 2 } }).unwrap().unwrap();
    let rev = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    assert!(roles.consistency_observer.forecast_hosted_request(&mut h, rev, 9001, actor).is_err());
    fresh.consistency_observer.forecast_hosted_request(&mut h, rev, 9001, actor).unwrap().unwrap();
    let spec = h.stream_message_spec("ordinary", ElapsedTick(100)).unwrap(); h.submit_request(h.revision(), 9001, spec, snapshot()).unwrap();
    assert!(h.action_consistency_observation(1).unwrap().event()); assert!(!h.action_consistency_observation(2).unwrap().event());
    publish(&mut h, &fresh.oversight.human, 9001, 2);
    assert_eq!(FileOversight::read_predictive_consistency(root.store(), &profile(), &required).unwrap().consistency.evidence.samples(), 2);
}

#[test]
fn malformed_or_unacknowledged_release_never_becomes_a_negative_sample_on_recovery() {
    for malformed in [false, true] {
        let root = Directory::new(); let (mut h, roles) = create(&root, false, true, 100);
        forecast(&mut h, &roles.consistency_observer, 9000, 1);
        let mut spec = h.stream_message_spec("ordinary", ElapsedTick(100)).unwrap();
        if malformed { spec.payload.push(0); spec.units = spec.payload.len() as u64; }
        else { std::fs::write(root.store().join("delivery.pending"), b"prevent acknowledgment").unwrap(); }
        let required = expected(&h, false, true, 100);
        let result = h.submit_request(h.revision(), 9000, spec, snapshot());
        if malformed { assert!(matches!(result.unwrap().disposition, FileRequestDisposition::NotAdmitted(_))); }
        else { assert!(result.is_err()); assert_eq!(h.action_consistency_snapshot(), Err(JournalError::Unavailable)); }
        let disk = FileOversight::read_predictive_consistency(root.store(), &profile(), &required).unwrap();
        assert_eq!(disk.consistency.evidence.samples(), 0); assert_eq!(disk.consistency.pending_attempt, Some(1));
        assert_eq!(disk.consistency.coverage_lost, malformed); drop(h);
        let (mut h, fresh) = FileOversight::open_predictive_guarded(root.store(), profile(), &required).unwrap();
        assert!(h.action_consistency_snapshot().unwrap().coverage_lost);
        h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
        let source = SourceFrame::capture(FrameIdentity { profile: capture(), stream: 7, sequence: 2, position: 0 }, &[-1.0]).unwrap();
        let rev = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
        assert!(fresh.consistency_observer.forecast_request(&mut h, rev, 9001, actor, &source).is_err());
        assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    }
}

#[test]
fn mismatched_message_profile_refuses_atomic_bootstrap_without_creating_storage() {
    let root = Directory::new(); let mut wrong = guards(false); wrong.stream = None;
    assert!(FileOversight::create_predictive_guarded(root.store(), profile(), &wrong, None, config(false, true, 100), None).is_err());
    assert!(!root.store().exists());
    let root = Directory::new(); let wrong = config(false, false, 100).with_stream_messages(
        fa_reference::action::consequence::delivery::stream::StreamProfile::new(7, 2, 4, 1024, 4096).unwrap()).unwrap();
    assert!(FileOversight::create_predictive_guarded(root.store(), profile(), &guards(false), None, wrong, None).is_err());
    assert!(!root.store().exists());
    let (h, _) = create(&root, false, true, 100); assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 0);
}
