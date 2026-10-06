//! Actual completed learned output, not caller-authored action payloads.
//! The original forecast is scored on message content through source-only intake.
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, FileHumanReviewer, decoder::learned::FileLearnedConfig,
    consistency::{FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver}};
use crate::action::consequence::delivery::persistent::requests::{FileRequestDisposition,
    actor::{FileActorSupervisor, LearnedTextProposal, LearnedTextRelease}};
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::Snapshot;
#[path = "../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn predictor(crossing: bool, stream: Option<StreamProfile>) -> FileConsistencyConfig {
    let (model, _, _) = recipe();
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let config = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: model.residual_contract(1).unwrap().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral,
            positive: if crossing { BinaryForecast::new(16_384, 49_152).unwrap() } else { neutral } },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap();
    match stream { Some(stream) => config.with_stream_messages(stream).unwrap(), None => config }
}
fn ready(root: &Directory, crossing: bool, stream: Option<StreamProfile>)
    -> (FileOversight, FileConsistencyObserver, FileLearnedConfig, FileOversightProfile)
{
    let mut profile = profile();
    let (mut host, config) = match stream {
        Some(stream) => {
            profile.delivery.initial_payload.clear(); profile.delivery.total = 1000;
            let (model, tokenizer, source) = recipe();
            let config = FileLearnedConfig::new_text_stream(model, tokenizer, source,
                LearnedDecoderBindingLimits::default(), stream).unwrap().with_required_sidecar().unwrap();
            let (host, _) = FileOversight::create_with_learned_text_stream(root.store(), profile.clone(), config.clone()).unwrap();
            (host, config)
        }
        None => {
            let config = config();
            let (host, _) = FileOversight::create_with_learned_text(root.store(), profile.clone(), config.clone()).unwrap();
            (host, config)
        }
    };
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let role = host.enable_action_consistency(host.revision(), predictor(crossing, stream)).unwrap();
    for _ in 0..3 { step(&mut host); }
    assert!(!host.learned_generation_inspection().unwrap().numerical.status.is_active());
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    (host, role, config, profile)
}
fn forecast(role: &FileConsistencyObserver, host: &mut FileOversight) {
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    role.forecast_hosted_request(host, revision, 71, n.actor_revision).unwrap().unwrap();
}
fn check_observed(host: &FileOversight, crossing: bool) {
    let observed = host.action_consistency_observation(1).unwrap();
    assert!(observed.event(), "the category must be the original generated aa content");
    assert_eq!(observed.crossed(), crossing);
    let evidence = host.action_consistency_snapshot().unwrap();
    assert_eq!(evidence.evidence.samples(), 1); assert_eq!(evidence.evidence.crossed(), crossing);
    assert!(evidence.pending_attempt.is_none());
    assert_eq!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::Admitted { .. }), !crossing);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn completed_generated_text_is_the_observed_category_and_retry_cannot_resample_it() {
    for crossing in [false, true] {
        let root = Directory::new(); let (mut host, role, config, profile) = ready(&root, crossing, None);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        forecast(&role, &mut host);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot());
        let ticket = port.submit(71, proposal()).unwrap();
        {
            let host = supervisor.host().unwrap(); check_observed(&host, crossing);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            if !crossing { assert_eq!(host.request_action(71).unwrap().spec().payload, b"aa"); }
        }
        let outcome = port.poll(&ticket);
        let retry = port.submit(71, proposal()).unwrap();
        assert_eq!(port.poll(&retry), outcome);
        assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
        drop(supervisor);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile, &config).unwrap();
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.crossed(), crossing);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn source_only_stream_predicts_message_content_not_transport_header_or_cumulative_frame() {
    for crossing in [false, true] {
        let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
        let (mut host, role, _, _) = ready(&root, crossing, Some(stream));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        forecast(&role, &mut host);
        let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot());
        let ticket = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        {
            let host = supervisor.host().unwrap(); check_observed(&host, crossing);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            if !crossing {
                let spec = host.request_action(71).unwrap().spec();
                assert!(!spec.payload.starts_with(b"aa"));
                let frame = ReleaseFrame::decode(&spec.payload).unwrap();
                assert_eq!(frame.message(), Some("aa")); assert!(frame.prior_messages().is_empty());
                assert_eq!(spec.units, spec.payload.len() as u64);
            }
            assert!(host.stream_snapshot().unwrap().published.visible().is_empty());
        }
        let outcome = port.poll(&ticket);
        let retry = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        assert_eq!(port.poll(&retry), outcome);
        assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
        assert!(port.submit(71, LearnedTextRelease::Finish, ElapsedTick(100)).is_err());
    }
}

#[test]
fn expired_forecast_cannot_be_renewed_by_source_only_submission_of_already_complete_output() {
    for tick in [10, 11] {
        let root = Directory::new(); let (mut host, role, _, _) = ready(&root, false, None);
        forecast(&role, &mut host); // created at 1, expires at 11
        host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot());
        let ticket = port.submit(71, proposal()).unwrap();
        let host = supervisor.host().unwrap();
        let evidence = host.action_consistency_snapshot().unwrap();
        if tick == 10 {
            assert_eq!(evidence.evidence.samples(), 1); assert!(!evidence.coverage_lost);
            assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::Admitted { .. }));
        } else {
            assert_eq!(evidence.evidence.samples(), 0); assert!(evidence.coverage_lost);
            assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
        }
        assert_eq!(host.inspect().executions, 0); drop(host);
        let outcome = port.poll(&ticket);
        let retry = port.submit(71, proposal()).unwrap(); assert_eq!(port.poll(&retry), outcome);
        assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence, evidence.evidence);
    }
}
