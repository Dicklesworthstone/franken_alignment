//! Real original generator/predictor/journal controls, not imported model verdicts.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use crate::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::delivery::persistent::{JournalError, JournalIo,
    observed::{FileOversight, FileOversightProfile, FileHumanReviewer, Event, Machine, journal, storage,
        consistency::{FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver, ConsistencyEvent},
        decoder::{DecoderEvent, learned::{LearnedEvent, bind_history}}},
    requests::actor::{FileActorSupervisor, LearnedTextProposal}};
use crate::action::consequence::oversight::{learned_source::LearnedEvidenceLimits,
    actor::{ActorProposal, ActorOutcome, Knowledge}};
use crate::Snapshot;
#[allow(dead_code)]
#[path = "../../text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
fn parameters() -> FileConsistencyParameters {
    let (model, _, _) = recipe(); let pair = BinaryForecast::new(32768, 32768).unwrap();
    FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model.cache_profile().layers()[&1].values().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10 }
}
fn predictor_with(parameters: FileConsistencyParameters, required: bool) -> FileLearnedConsistencyConfig {
    let predictor = FileLearnedConsistencyConfig::new(FileConsistencyConfig::new(parameters).unwrap(),
        1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
        MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap();
    if required { predictor.with_pre_output_forecast().unwrap() } else { predictor }
}
fn predictor() -> FileLearnedConsistencyConfig { predictor_with(parameters(), true) }
fn pinned() -> FileLearnedConfig { config().with_required_owned_pre_output_forecast(predictor()).unwrap() }
fn forecast(host: &mut FileOversight, observer: &FileConsistencyObserver) {
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    observer.forecast_owned_learned_request(host, revision, 71, n.actor_revision).unwrap().unwrap().prediction().unwrap();
}
fn bytes(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn replay(host: &FileOversight, generation: &FileLearnedConfig) -> Machine {
    let mut events = journal::decode(&host.profile, host.store.identity(), &bytes(host)).unwrap();
    bind_history(&mut events, generation).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}

#[test]
fn first_image_installs_both_requirements_and_matches_original_explicit_generation() {
    let root = Directory::new(); let other = Directory::new(); let generation = pinned();
    let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), generation.clone()).unwrap();
    assert_eq!(host.revision(), 1);
    assert!(matches!(host.events.as_slice(), [Event::Decoder(DecoderEvent::Learned(LearnedEvent::Enable(_)))]));
    assert_eq!(host.machine.learned_consistency.as_deref(), Some(&predictor()));
    assert!(host.owned_learned_action_consistency_required());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.admitted_tokens, 0);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    assert!(!host.clock_ready());
    let (mut control, _) = FileOversight::create_with_learned_text(other.store(), profile(), config()).unwrap();
    let control_role = control.enable_learned_action_consistency(control.revision(), predictor()).unwrap();
    for h in [&mut host, &mut control] {
        h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); step(h);
        let n = h.learned_generation_inspection().unwrap().numerical; let before = bytes(h);
        assert!(h.prepare_learned_step_intent(h.revision(), n.actor_revision, n.position).is_err());
        assert!(h.begin_learned_step(h.revision(), n.actor_revision, n.position).is_err());
        assert_eq!(bytes(h), before);
    }
    forecast(&mut host, &observer); forecast(&mut control, &control_role);
    for _ in 0..2 { step(&mut host); step(&mut control); }
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, control.learned_generation_inspection().unwrap().numerical);
    assert_eq!(host.actor_snapshot().unwrap(), control.actor_snapshot().unwrap());
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, control.learned_action_consistency_snapshot().unwrap().work);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    let reconstructed = replay(&host, &generation);
    assert_eq!(reconstructed.snapshot(host.events.len()), host.inspect());
    assert_eq!(reconstructed.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn pins_cannot_mix_source_modes_or_omit_the_required_sidecar_or_exact_stream() {
    assert_eq!(config().with_required_owned_pre_output_forecast(predictor_with(parameters(), false)), Err(Error::Binding));
    let (model, tokenizer, source) = recipe();
    let no_sidecar = FileLearnedConfig::new_text(model.clone(), tokenizer, source,
        crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits::default()).unwrap();
    assert_eq!(no_sidecar.with_required_owned_pre_output_forecast(predictor()), Err(Error::Binding));
    let mut raw_parameters = parameters(); raw_parameters.profile = model.residual_contract(1).unwrap().profile();
    let raw = FileConsistencyConfig::new(raw_parameters).unwrap().with_hosted_residual(1).unwrap()
        .with_pre_output_forecast().unwrap();
    assert_eq!(pinned().with_required_owned_pre_output_forecast(predictor()), Err(Error::Duplicate));
    assert_eq!(pinned().with_required_pre_output_forecast(raw.clone()), Err(Error::Duplicate));
    assert_eq!(config().with_required_pre_output_forecast(raw).unwrap()
        .with_required_owned_pre_output_forecast(predictor()), Err(Error::Duplicate));
    let stream = crate::action::consequence::delivery::stream::StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let p = FileLearnedConsistencyConfig::new(predictor().consistency().clone().with_stream_messages(stream).unwrap(),
        1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
        .unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap();
    assert_eq!(config().with_required_owned_pre_output_forecast(p), Err(Error::Binding));
    let original = config(); let frozen = original.clone().with_required_owned_pre_output_forecast(predictor()).unwrap();
    assert_ne!(frozen, original);
    assert_eq!(&frozen.bytes[..8], DOMAIN);
    assert_eq!(&frozen.bytes[16..24], b"FALKPRD\x01");
    assert_eq!(frozen.required_owned_pre_output_forecast(), Some(&predictor()));
    assert!(frozen.required_pre_output_forecast().is_none());
    assert_eq!(frozen.clone().with_required_computed_identity().unwrap().required_owned_pre_output_forecast(), Some(&predictor()));
}

#[test]
fn missing_or_incompatible_predictor_cannot_create_or_attach_a_weaker_numerical_owner() {
    let root = Directory::new();
    assert!(FileOversight::create_with_pre_output_forecast(root.store(), profile(), config()).is_err());
    assert!(!root.store().exists());
    let mut wrong = parameters(); wrong.stream += 1;
    let wrong = config().with_required_owned_pre_output_forecast(predictor_with(wrong, true)).unwrap();
    assert!(FileOversight::create_with_pre_output_forecast(root.store(), profile(), wrong.clone()).is_err());
    assert!(!root.store().exists(), "the original bootstrap validates both contracts before storage");
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let before = bytes(&host); let state = host.inspect();
    assert!(host.enable_learned_generation_with_pre_output_forecast(host.revision(), wrong).is_err());
    assert!(host.enable_learned_generation_with_pre_output_forecast(host.revision() + 1, pinned()).is_err());
    assert_eq!(bytes(&host), before); assert_eq!(host.inspect(), state);
    assert!(!host.learned_generation_required()); assert!(!host.action_consistency_required());
    assert!(host.storage_failure().is_none());
    host.enable_learned_generation_with_pre_output_forecast(host.revision(), pinned()).unwrap();
    assert!(host.owned_learned_action_consistency_required());
    let before = bytes(&host);
    assert!(host.enable_learned_generation_with_pre_output_forecast(host.revision(), pinned()).is_err());
    assert_eq!(bytes(&host), before, "duplicate attachment cannot reissue observer custody");
}

#[test]
fn recovery_binds_the_nested_pin_and_never_rearms_an_unanswered_forecast() {
    let root = Directory::new(); let generation = pinned();
    let (mut host, _, old) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), generation.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host); forecast(&mut host, &old);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let before = bytes(&host); let costs = host.learned_action_consistency_snapshot().unwrap();
    let mut duplicate = host.events.clone();
    duplicate.push(Event::Consistency(ConsistencyEvent::EnableLearned(
        crate::action::consequence::delivery::persistent::observed::consistency::learned::Configuration::new(predictor()))));
    assert!(crate::action::consequence::delivery::persistent::observed::consistency::learned::owned::bind_owned_history(
        &mut duplicate, &generation, &predictor()).is_err());
    drop(host);
    assert!(FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &config(), &predictor()).is_err());
    let weak = predictor_with(parameters(), false);
    assert!(FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &weak).is_err());
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
    let (mut host, _, observer) = FileOversight::open_with_owned_learned_consistency(
        root.store(), profile(), &generation, &predictor()).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
    let recovered = host.learned_action_consistency_snapshot().unwrap();
    assert_eq!(recovered.work, costs.work); assert_eq!(recovered.retained_source_bytes, costs.retained_source_bytes);
    assert!(recovered.consistency.coverage_lost); assert_eq!(recovered.consistency.evidence, costs.consistency.evidence);
    let revision = host.revision();
    assert_eq!(old.forecast_owned_learned_request(&mut host, revision, 72, n.actor_revision).err(), Some(Error::Binding.into()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    let revision = host.revision(); let before = bytes(&host);
    assert!(host.complete_learned_step(revision, n.actor_revision, n.position).is_err());
    assert!(observer.forecast_owned_learned_request(&mut host, revision, 72, n.actor_revision).is_err());
    assert_eq!(bytes(&host), before); assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    assert!(host.learned_generation_inspection().unwrap().pending.is_some());
}

#[test]
fn all_attachment_barriers_leave_either_no_generator_or_the_complete_pinned_contract() {
    for barrier in BARRIERS {
        let root = Directory::new(); let generation = pinned();
        let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
        let prior = host.inspect(); host.store.fail_once(barrier);
        assert!(matches!(host.enable_learned_generation_with_pre_output_forecast(host.revision(), generation.clone()),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), prior); assert!(host.storage_failure().is_some());
        let disk = bytes(&host); let visible = barrier == JournalIo::DirectorySync;
        let mut events = journal::decode(&host.profile, host.store.identity(), &disk).unwrap();
        assert_eq!(events.len(), usize::from(visible));
        if visible {
            bind_history(&mut events, &generation).unwrap();
            let machine = Machine::replay(&host.profile, &events).unwrap();
            assert_eq!(machine.learned_contract(), Some(&generation));
            assert_eq!(machine.learned_consistency.as_deref(), Some(&predictor()));
            assert_eq!(machine.broker.hosted_learned_generation().unwrap().work.admitted_tokens, 0);
        } else { assert!(bind_history(&mut events, &generation).is_err()); }
        drop(host);
        if visible {
            let (host, _, _) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &predictor()).unwrap();
            assert!(host.learned_generation_inspection().unwrap().paused);
            assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        } else {
            let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
            assert!(!host.learned_generation_required());
            assert!(!host.action_consistency_required());
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn original_actor_gateway_survives_attachment_and_consumes_only_one_forecast() {
    let root = Directory::new(); let generation = pinned();
    let (host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let (port, mut supervisor) = host.into_actor_gateway();
    let observer = {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.enable_learned_generation_with_pre_output_forecast(revision, generation.clone()).unwrap()
    };
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1)).unwrap(); step(&mut host); forecast(&mut host, &observer);
        step(&mut host); step(&mut host);
    }
    observe(&mut supervisor, snapshot()); let p = proposal();
    let command = ActorProposal { target: p.target, payload: b"aa".to_vec(),
        expected_policy_epoch: p.expected_policy_epoch, deadline: p.deadline, units: p.units };
    let ticket = port.submit(71, &command).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
    let retry = port.submit(71, &command).unwrap(); assert_eq!(port.poll(&ticket), port.poll(&retry));
    port.cancel(&ticket).unwrap();
    assert!(matches!(port.poll(&retry), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    let host = supervisor.host().unwrap();
    assert_eq!(host.retained_requests(), 1); assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(replay(&host, &generation).snapshot(host.events.len()), host.inspect());
}
