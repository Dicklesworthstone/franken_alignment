//! Original generated text, likelihood observations, journal replay and storage.
//! Synthetic fixtures prove neither learned-code fidelity nor calibration.
use super::{FileConsistencyConfig, ProgressiveForecastPolicy, precision, parameters};
use crate::action::consequence::activation::consistency::Prediction;
use crate::action::consequence::activation::probe::ProbeOutcome;
use crate::action::consequence::delivery::persistent::{JournalError, JournalIo,
    observed::{FileOversight, FileOversightProfile, FileHumanReviewer, Event, journal, storage,
        consistency::FileConsistencyObserver, decoder::{DecoderEvent, learned::{FileLearnedConfig, LearnedEvent, bind_history}},
        machine::Machine},
    requests::actor::{FileActorSupervisor, LearnedTextProposal, LearnedTextRelease}};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::{learned_source::LearnedEvidenceLimits,
    decoder_monitoring::LearnedDecoderBindingLimits, consistency::ConsistencyStopPolicy,
    actor::{ActorOutcome, Knowledge}};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::{Error, Snapshot};
#[path = "../../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn exact(policy: Option<ProgressiveForecastPolicy>, threshold: f32, stream: Option<StreamProfile>, stop: bool) -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    let mut p = parameters(); p.profile = model.residual_contract(1).unwrap().profile(); p.threshold = threshold;
    let mut forecast = FileConsistencyConfig::new(p).unwrap().with_hosted_residual(1).unwrap();
    if let Some(stream) = stream { forecast = forecast.with_stream_messages(stream).unwrap(); }
    forecast = forecast.with_pre_output_forecast().unwrap();
    if let Some(policy) = policy { forecast = forecast.with_progressive_forecast(policy).unwrap(); }
    if stop { forecast = forecast.with_terminal_stop(ConsistencyStopPolicy::new(9, 1, 900).unwrap()).unwrap(); }
    let config = match stream {
        None => FileLearnedConfig::new_text(model, tokenizer, source, LearnedDecoderBindingLimits::default()).unwrap(),
        Some(stream) => FileLearnedConfig::new_text_stream(model, tokenizer, source,
            LearnedDecoderBindingLimits::default(), stream).unwrap(),
    };
    config.with_required_sidecar().unwrap().with_required_pre_output_forecast(forecast).unwrap()
}
fn ready(root: &Directory, config: &FileLearnedConfig, stream: bool) -> (FileOversight, FileConsistencyObserver) {
    let mut p = profile();
    if stream { p.delivery.initial_payload.clear(); p.delivery.total = 1000; }
    let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast(root.store(), p, config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.work.sampling_attempts, 0);
    assert!(host.begin_learned_step(host.revision(), numerical.actor_revision, numerical.position).is_err());
    (host, observer)
}
fn predict(host: &mut FileOversight, observer: &FileConsistencyObserver) -> Result<Result<Prediction, Error>, JournalError> {
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    observer.forecast_hosted_request(host, revision, 71, n.actor_revision)
}
fn bytes(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn cold(host: &FileOversight, config: &FileLearnedConfig) -> Vec<Event> {
    let mut events = journal::decode(&host.profile, host.store.identity(), &bytes(host)).unwrap();
    bind_history(&mut events, config).unwrap(); let original = Machine::replay(&host.profile, &events).unwrap();
    assert_eq!(original.snapshot(events.len()), host.inspect());
    assert_eq!(original.consistency_snapshot(host.revision()).unwrap(), host.action_consistency_snapshot().unwrap());
    assert_eq!(original.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
    events
}

#[test]
fn owned_prompt_uses_coarse_certificate_then_original_generation_and_one_shot_actor_observation() {
    let a = Directory::new(); let b = Directory::new();
    let selected = exact(Some(precision()), 0.0, None, false); let baseline = exact(None, 0.0, None, false);
    let (mut adaptive, observer) = ready(&a, &selected, false); let (mut original, reference) = ready(&b, &baseline, false);
    let n = adaptive.learned_generation_inspection().unwrap().numerical;
    let actual = predict(&mut adaptive, &observer).unwrap().unwrap();
    let expected = predict(&mut original, &reference).unwrap().unwrap();
    assert_eq!(actual.forecast(), expected.forecast());
    assert_eq!(actual.observation().outcome(), expected.observation().outcome());
    assert_eq!(actual.observation().mantissa_bits(), 0); assert_eq!(expected.observation().mantissa_bits(), 23);
    assert!(actual.encoded_bytes() < expected.encoded_bytes());
    assert_eq!(adaptive.learned_generation_inspection().unwrap().numerical, n, "forecast is not a sampler draw");
    for _ in 0..2 {
        step(&mut adaptive); step(&mut original);
        assert_eq!(adaptive.learned_generation_inspection().unwrap().numerical, original.learned_generation_inspection().unwrap().numerical);
    }
    let witnesses = |events: Vec<Event>| events.into_iter().filter_map(|event| match event {
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) => Some(witness), _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(witnesses(cold(&adaptive, &selected)), witnesses(cold(&original, &baseline)));
    for (host, config) in [(adaptive, &selected), (original, &baseline)] {
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
        assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
        let retry = port.submit(71, proposal()).unwrap(); assert_eq!(port.poll(&retry), port.poll(&ticket));
        assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
        port.cancel(&ticket).unwrap();
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        let host = supervisor.host().unwrap(); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100); cold(&host, config);
    }
}

#[test]
fn original_prediction_failure_closes_coverage_without_sampling_and_cannot_retry_with_more_precision() {
    for limited_bytes in [false, true] {
        let root = Directory::new();
        let policy = if limited_bytes { ProgressiveForecastPolicy::new(0, 4, 23, 1).unwrap() }
            else { ProgressiveForecastPolicy::new(0, 4, 0, 1024).unwrap() };
        let config = exact(Some(policy), 1.0, None, true);
        let (mut host, observer) = ready(&root, &config, false);
        let failure = if limited_bytes { Error::Limit } else { Error::Incomplete };
        assert_eq!(predict(&mut host, &observer).unwrap(), Err(failure));
        let state = host.action_consistency_snapshot().unwrap();
        assert!(state.coverage_lost); assert_eq!(state.evidence.samples(), 0);
        assert!(host.inspect().stop.is_some()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 0);
        assert!(!matches!(predict(&mut host, &observer), Ok(Ok(_))), "a failed job cannot issue a replacement forecast");
        let n = host.learned_generation_inspection().unwrap().numerical;
        assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
        cold(&host, &config);
    }
    // Same ambiguous source, full admitted precision: original equality forecast.
    let root = Directory::new(); let config = exact(Some(precision()), 1.0, None, true);
    let (mut host, observer) = ready(&root, &config, false);
    let actual = predict(&mut host, &observer).unwrap().unwrap();
    assert_eq!(actual.observation().outcome(), ProbeOutcome::AtThreshold);
    assert_eq!(actual.observation().mantissa_bits(), 23); assert!(actual.encoded_bytes() > 86);
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost); step(&mut host);
}

#[test]
fn recovery_rejects_precision_policy_substitution_without_cleaning_or_rearming_pending_forecasts() {
    let root = Directory::new(); let config = exact(Some(precision()), 0.0, None, false);
    let (mut host, observer) = ready(&root, &config, false); predict(&mut host, &observer).unwrap().unwrap();
    step(&mut host); let before = bytes(&host); drop(host);
    for other in [None, Some(ProgressiveForecastPolicy::new(1, 4, 23, 1024).unwrap()),
        Some(ProgressiveForecastPolicy::new(0, 2, 23, 1024).unwrap()),
        Some(ProgressiveForecastPolicy::new(0, 4, 22, 1024).unwrap()),
        Some(ProgressiveForecastPolicy::new(0, 4, 23, 1024).unwrap())] {
        assert!(FileOversight::open_with_learned_generation(root.store(), profile(), &exact(other, 0.0, None, false)).is_err());
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
    }
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    assert!(recovered.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(recovered.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    assert!(predict(&mut recovered, &observer).is_err(), "old observer cannot adopt recovered owner");
}

#[test]
fn all_forecast_write_barriers_withhold_predictions_and_cold_recovery_preserves_the_selected_mode() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = exact(Some(precision()), 0.0, None, false);
        let (mut host, observer) = ready(&root, &config, false); let before = host.inspect();
        host.store.fail_once(barrier);
        assert!(matches!(predict(&mut host, &observer), Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some());
        drop(host);
        let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert!(recovered.learned_generation_inspection().unwrap().paused);
        assert_eq!(recovered.machine.consistency.as_deref(), config.required_pre_output_forecast());
        assert_eq!(recovered.inspect().executions, 0);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 0);
    }
}

#[test]
fn stream_mode_observes_generated_message_once_and_still_refuses_unconfirmed_finish() {
    let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let config = exact(Some(precision()), 0.0, Some(stream), false);
    let (mut host, observer) = ready(&root, &config, true);
    let result = predict(&mut host, &observer).unwrap().unwrap(); assert_eq!(result.observation().mantissa_bits(), 0);
    step(&mut host); step(&mut host);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    let ticket = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
    observe(&mut supervisor, snapshot());
    assert!(port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).is_err());
    let retry = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert_eq!(port.poll(&retry), port.poll(&ticket));
    let host = supervisor.host().unwrap();
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert!(host.stream_snapshot().unwrap().published.visible().is_empty());
    assert_eq!(host.inspect().executions, 0); cold(&host, &config);
}
