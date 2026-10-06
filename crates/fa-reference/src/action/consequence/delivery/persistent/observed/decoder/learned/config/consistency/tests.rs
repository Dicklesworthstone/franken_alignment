//! Independent recipe binding, actual original generation and canonical files.
//! Synthetic weights/probabilities are controls, not calibration evidence.
use super::*;
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::delivery::persistent::observed::{Event, journal, storage,
    machine::Machine, decoder::{DecoderEvent, learned::{bind_history, Configuration, LearnedEvent}},
    consistency::{ConsistencyEvent, FileConsistencyParameters}};
use crate::action::consequence::delivery::persistent::requests::actor::{FileActorSupervisor, LearnedTextProposal};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::{learned_source::LearnedEvidenceLimits,
    consistency::ConsistencyStopPolicy, decoder_monitoring::LearnedDecoderBindingLimits};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::Snapshot;
#[path = "../text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn parameters() -> FileConsistencyParameters {
    let (model, _, _) = recipe();
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model.residual_contract(1).unwrap().profile(), weights: vec![1.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 10 }
}
fn required(p: FileConsistencyParameters, stream: Option<StreamProfile>) -> FileConsistencyConfig {
    let forecast = FileConsistencyConfig::new(p).unwrap().with_hosted_residual(1).unwrap();
    let forecast = match stream {
        Some(stream) => forecast.with_stream_messages(stream).unwrap(), None => forecast,
    };
    forecast.with_pre_output_forecast().unwrap()
}
fn predictor() -> FileConsistencyConfig { required(parameters(), None) }
fn pinned() -> FileLearnedConfig { config().with_required_pre_output_forecast(predictor()).unwrap() }
fn disk(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn decoded(host: &FileOversight, expected: &FileLearnedConfig) -> Vec<Event> {
    let mut events = journal::decode(&host.profile, host.store.identity(), &disk(host)).unwrap();
    bind_history(&mut events, expected).unwrap(); events
}
fn independent(host: &FileOversight, expected: &FileLearnedConfig) {
    let events = decoded(host, expected);
    let cold = Machine::replay(&host.profile, &events).unwrap();
    assert_eq!(cold.snapshot(events.len()), host.inspect());
    assert_eq!(cold.consistency_snapshot(host.revision()).unwrap(), host.action_consistency_snapshot().unwrap());
    assert_eq!(cold.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
}
fn begin(host: &mut FileOversight, observer: &FileConsistencyObserver)
    -> crate::action::consequence::delivery::persistent::observed::consistency::pre_output::FilePreOutputForecast
{
    let n = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    observer.begin_pre_output_request(host, revision, 71, n.actor_revision, n.position).unwrap().unwrap()
}

#[test]
fn recipe_wraps_exact_original_predictor_bytes_and_preserves_the_legacy_recipe() {
    let legacy = config(); let before = legacy.bytes().clone(); let forecast = predictor();
    assert!(legacy.required_pre_output_forecast().is_none());
    let exact = legacy.clone().with_required_pre_output_forecast(forecast.clone()).unwrap();
    let mut golden = b"FALBOOT\x01".to_vec(); golden.extend_from_slice(&0_u64.to_be_bytes());
    golden.extend_from_slice(b"FALPRED\x01");
    golden.extend_from_slice(&(before.len() as u64).to_be_bytes()); golden.extend_from_slice(&before);
    golden.extend_from_slice(&(forecast.encoded().len() as u64).to_be_bytes()); golden.extend_from_slice(forecast.encoded());
    assert_eq!(exact.bytes().as_ref(), golden);
    assert_eq!(exact.required_pre_output_forecast(), Some(&forecast));
    assert_eq!(legacy.bytes(), &before); assert_ne!(exact, legacy);
    assert_eq!(exact.clone().with_required_pre_output_forecast(forecast), Err(Error::Duplicate));
    for change in 0..6 {
        let mut p = parameters();
        match change {
            0 => p.weights[0] = 0.5, 1 => p.forecast.positive = BinaryForecast::new(16_384, 49_152).unwrap(),
            2 => p.max_prediction_age_ticks += 1, 3 => p.alpha = ErrorBudget::new(1, 4).unwrap(),
            4 => p.max_predictions -= 1, _ => p.forecast.event_prefix = b"different".to_vec(),
        }
        let other = legacy.clone().with_required_pre_output_forecast(required(p, None)).unwrap();
        assert_ne!(exact, other, "every original calibration and timing field is pinned");
    }
}

#[test]
fn first_canonical_image_enforces_forecasting_before_any_request_or_continuation() {
    let root = Directory::new(); let config = pinned();
    let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), config.clone()).unwrap();
    assert_eq!(host.revision(), 1); assert!(!host.clock_ready()); assert!(host.action_consistency_required());
    assert_eq!(host.machine.consistency.as_deref(), config.required_pre_output_forecast());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.cumulative_work.admitted_tokens, 0);
    let events = decoded(&host, &config);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], Event::Decoder(DecoderEvent::Learned(LearnedEvent::Enable(_)))));
    assert!(!events.iter().any(|event| matches!(event, Event::Consistency(ConsistencyEvent::Enable(_)))));
    independent(&host, &config);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical; let before = disk(&host);
    assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(disk(&host), before); assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    let mut forecast = begin(&mut host, &observer);
    while forecast.numerical().status.is_active() {
        let revision = host.revision(); forecast.advance(&mut host, revision).unwrap().unwrap();
    }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
    let retry = port.submit(71, proposal()).unwrap(); assert_eq!(port.poll(&ticket), port.poll(&retry));
    let host = supervisor.host().unwrap();
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    independent(&host, &config);
}

#[test]
fn pinned_bootstrap_matches_separate_original_installation_and_every_numerical_witness() {
    let a = Directory::new(); let b = Directory::new(); let legacy = config(); let exact = pinned();
    let (mut pinned, _, observer) = FileOversight::create_with_pre_output_forecast(a.store(), profile(), exact.clone()).unwrap();
    let (mut separate, _) = FileOversight::create_with_learned_text(b.store(), profile(), legacy.clone()).unwrap();
    let original = separate.enable_action_consistency(separate.revision(), predictor()).unwrap();
    for host in [&mut pinned, &mut separate] {
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(host);
    }
    let mut p = begin(&mut pinned, &observer); let mut q = begin(&mut separate, &original);
    assert_eq!(p.prediction(), q.prediction());
    while p.numerical().status.is_active() {
        let revision = pinned.revision(); p.advance(&mut pinned, revision).unwrap().unwrap();
        let revision = separate.revision(); q.advance(&mut separate, revision).unwrap().unwrap();
        assert_eq!(p.numerical(), q.numerical());
    }
    let witnesses = |events: Vec<Event>| events.into_iter().filter_map(|event| {
        match event { Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) => Some(witness), _ => None }
    }).collect::<Vec<_>>();
    assert_eq!(witnesses(decoded(&pinned, &exact)), witnesses(decoded(&separate, &legacy)));
    assert_eq!(pinned.inspect().control, separate.inspect().control);
    independent(&pinned, &exact); independent(&separate, &legacy);
}

#[test]
fn weaker_or_wrong_domain_configs_refuse_and_original_binding_runs_before_storage_creation() {
    let root = Directory::new();
    assert!(FileOversight::create_with_pre_output_forecast(root.store(), profile(), config()).is_err());
    assert!(!root.store().exists());
    let weak = FileConsistencyConfig::new(parameters()).unwrap().with_hosted_residual(1).unwrap();
    assert_eq!(config().with_required_pre_output_forecast(weak), Err(Error::Binding));
    let (model, tokenizer, source) = recipe();
    let no_sidecar = FileLearnedConfig::new_text(model, tokenizer, source, LearnedDecoderBindingLimits::default()).unwrap();
    assert_eq!(no_sidecar.with_required_pre_output_forecast(predictor()), Err(Error::Binding));
    let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    assert_eq!(config().with_required_pre_output_forecast(required(parameters(), Some(stream))), Err(Error::Binding));
    for change in 0..4 {
        let root = Directory::new(); let mut p = parameters();
        match change { 0 => p.profile.model_generation += 1, 1 => { p.weights.pop(); },
            2 => p.stream += 1, _ => p.forecast.policy_generation += 1 }
        let exact = config().with_required_pre_output_forecast(required(p, None)).unwrap();
        assert!(FileOversight::create_with_pre_output_forecast(root.store(), profile(), exact).is_err());
        assert!(!root.store().exists(), "invalid source never creates a partial durable owner");
    }
    let (host, _, _) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), pinned()).unwrap();
    assert!(host.action_consistency_required()); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn independent_recovery_rejects_downgrade_and_same_label_predictor_substitution_without_writing() {
    let root = Directory::new(); let exact = pinned();
    let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), exact.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let mut pending = begin(&mut host, &observer); let revision = host.revision();
    pending.advance(&mut host, revision).unwrap().unwrap();
    let before = disk(&host); drop(host);
    let mut variants = vec![config()];
    let mut changed = parameters(); changed.forecast.positive = BinaryForecast::new(16_384, 49_152).unwrap();
    variants.push(config().with_required_pre_output_forecast(required(changed, None)).unwrap());
    let mut changed = parameters(); changed.max_prediction_age_ticks += 1;
    variants.push(config().with_required_pre_output_forecast(required(changed, None)).unwrap());
    for variant in variants {
        assert!(FileOversight::open_with_learned_generation(root.store(), profile(), &variant).is_err());
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
    }
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &exact).unwrap();
    let n = recovered.learned_generation_inspection().unwrap();
    assert!(n.paused); assert!(recovered.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(recovered.machine.consistency.as_deref(), exact.required_pre_output_forecast());
    let revision = recovered.revision();
    assert_eq!(observer.forecast_hosted_request(&mut recovered, revision, 72, n.numerical.actor_revision).err(),
        Some(Error::Binding.into()));
    assert!(pending.advance(&mut recovered, revision).is_err());
    assert_eq!(recovered.inspect().executions, 0); assert_eq!(recovered.inspect().control.ledger.available, 100);
}

#[test]
fn runtime_less_disk_bytes_and_changed_bootstrap_cannot_supply_a_weaker_predictor() {
    let root = Directory::new(); let exact = pinned();
    let (host, _, _) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), exact.clone()).unwrap();
    let mut raw = journal::decode(&host.profile, host.store.identity(), &disk(&host)).unwrap();
    assert!(Machine::replay(&host.profile, &raw).is_err(), "encoded recipe alone is not a runtime trust root");
    let mut missing = Vec::new();
    assert!(bind_history(&mut missing, &exact).is_err());
    raw[0] = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Enable(Configuration::new(config()))));
    assert_eq!(bind_history(&mut raw, &exact), Err(Error::Binding));
    let original = disk(&host); let mut host = host;
    assert!(host.enable_action_consistency(host.revision(), predictor()).is_err());
    assert_eq!(disk(&host), original, "a second predictor cannot replace the pinned one");
    independent(&host, &exact);
}

#[test]
fn terminal_stop_policy_is_pinned_and_original_pair_constructor_cannot_omit_the_requirement() {
    let stop = ConsistencyStopPolicy::new(9, 1, 900).unwrap();
    let forecast = predictor().with_terminal_stop(stop).unwrap();
    let exact = config().with_required_pre_output_forecast(forecast.clone()).unwrap();
    assert_ne!(exact, pinned());
    let root = Directory::new();
    // Even without issuing an observer, a legacy creation API must install the
    // complete supplied recipe. It is not a downgrade path or role-reissue API.
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), exact.clone()).unwrap();
    assert_eq!(host.machine.consistency.as_deref(), Some(&forecast));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.begin_learned_step(host.revision(), n.actor_revision, n.position).is_err());
    assert!(host.enable_action_consistency(host.revision(), predictor()).is_err());
    independent(&host, &exact);
}

mod provisioned;
