//! Actual ByteBpe/learned inference and ORIGINAL event replay, not guard-only models.
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, FileHumanReviewer, machine::Machine, journal, Event,
    consistency::{ConsistencyEvent, FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver},
    decoder::{DecoderEvent, learned::{FileLearnedConfig, LearnedEvent, LearnedStepIntent, bind_history}}};
use crate::action::consequence::delivery::persistent::{JournalError,
    requests::{FileRequestDisposition, actor::{FileActorSupervisor, LearnedTextProposal}}};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::{Error, Snapshot};
use std::rc::Rc;
#[path = "../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn predictor(required: bool) -> FileConsistencyConfig {
    let (model, _, _) = recipe(); let pair = BinaryForecast::new(32_768, 32_768).unwrap();
    let config = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: model.residual_contract(1).unwrap().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap();
    if required { config.with_pre_output_forecast().unwrap() } else { config }
}
fn setup(root: &Directory, required: bool) -> (FileOversight, FileConsistencyObserver, FileLearnedConfig) {
    let config = config();
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let role = host.enable_action_consistency(host.revision(), predictor(required)).unwrap();
    (host, role, config)
}
fn forecast(host: &mut FileOversight, role: &FileConsistencyObserver, request: u64) {
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    role.forecast_hosted_request(host, revision, request, n.actor_revision).unwrap().unwrap();
}
fn disk(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn read_original(host: &FileOversight, config: &FileLearnedConfig) -> Machine {
    let mut events = journal::decode(&host.profile, host.store.identity(), &disk(host)).unwrap();
    bind_history(&mut events, config).unwrap(); Machine::replay(&host.profile, &events).unwrap()
}
fn spec(host: &FileOversight, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: payload.to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}

#[test]
fn ordinary_and_cooperative_generation_require_the_same_acknowledged_prompt_forecast() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, role, config) = setup(&root, true); let (mut control, control_role, _) = setup(&other, false);
    step(&mut host); step(&mut control); // fixed prompt, no sampled token
    let n = host.learned_generation_inspection().unwrap().numerical; let bytes = disk(&host);
    assert_eq!(host.begin_learned_step(host.revision(), n.actor_revision, n.position), Err(Error::Incomplete.into()));
    assert_eq!(host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).err(), Some(Error::Incomplete.into()));
    assert_eq!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).err(), Some(Error::Incomplete.into()));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n); assert_eq!(disk(&host), bytes);
    let intent = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Begin(LearnedStepIntent {
        actor_revision: n.actor_revision, position: n.position,
    })));
    assert_eq!(read_original(&host, &config).apply(&intent).err(), Some(Error::Incomplete));
    let revision = host.revision();
    assert_eq!(role.forecast_hosted_action(&mut host, revision, 1, n.actor_revision).err(), Some(Error::Binding.into()));
    forecast(&mut host, &role, 71); forecast(&mut control, &control_role, 71);
    for _ in 0..2 { step(&mut host); step(&mut control); }
    assert_eq!(host.learned_generation_inspection().unwrap().numerical,
        control.learned_generation_inspection().unwrap().numerical);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    let replay = read_original(&host, &config);
    assert_eq!(replay.snapshot(host.events.len()), host.inspect());
    assert_eq!(replay.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
    // Only the declared requirement differs: the ORIGINAL numerical/witness
    // records must otherwise match independently generated legacy history.
    let mut normalized = host.events.clone();
    for event in &mut normalized {
        if let Event::Consistency(ConsistencyEvent::Enable(config)) = event { *config = Rc::new(predictor(false)); }
    }
    assert_eq!(journal::encode(&host.profile, host.store.identity(), &normalized).unwrap(),
        journal::encode(&host.profile, host.store.identity(), &control.events).unwrap());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn no_action_can_answer_the_forecast_before_complete_output_or_with_substitute_bytes() {
    let root = Directory::new(); let (mut host, role, _) = setup(&root, true);
    step(&mut host); forecast(&mut host, &role, 71);
    let early = spec(&host, b"aa"); let bytes = disk(&host);
    assert!(host.submit_request(host.revision(), 71, early.clone(), snapshot()).is_err());
    assert_eq!(disk(&host), bytes); assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    step(&mut host); step(&mut host);
    let bytes = disk(&host); let substitution = spec(&host, b"zz");
    assert_eq!(host.submit_request(host.revision(), 71, substitution, snapshot()).err(), Some(Error::Binding.into()));
    assert_eq!(host.submit_request(host.revision(), 72, early, snapshot()).err(), Some(Error::Binding.into()));
    assert_eq!(disk(&host), bytes); assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
    let outcome = port.poll(&ticket); let retry = port.submit(71, proposal()).unwrap();
    assert_eq!(port.poll(&retry), outcome);
    let host = supervisor.host().unwrap();
    assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::Admitted { .. }));
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn replay_rejects_a_legacy_sample_history_labeled_as_requiring_pre_output_prediction() {
    let root = Directory::new(); let (mut legacy, _, config) = setup(&root, false);
    step(&mut legacy); step(&mut legacy); // legal only for the legacy contract
    assert_eq!(legacy.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 1);
    let mut modified = legacy.events.clone();
    for event in &mut modified {
        if let Event::Consistency(ConsistencyEvent::Enable(config)) = event { *config = Rc::new(predictor(true)); }
    }
    assert_eq!(Machine::replay(&legacy.profile, &modified).err(), Some(Error::Incomplete));
    let replay = read_original(&legacy, &config);
    assert_eq!(replay.broker.hosted_learned_generation().unwrap(), legacy.learned_generation_inspection().unwrap().numerical);
    let good_root = Directory::new(); let (mut good, role, config) = setup(&good_root, true);
    step(&mut good); forecast(&mut good, &role, 71); step(&mut good);
    assert_eq!(read_original(&good, &config).snapshot(good.events.len()), good.inspect());
}

#[test]
fn expiry_or_lost_coverage_between_intent_and_completion_prevents_any_sampling() {
    for lost in [false, true] {
        let root = Directory::new(); let (mut host, role, config) = setup(&root, true);
        step(&mut host); forecast(&mut host, &role, 71);
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
        if lost { let revision = host.revision(); role.unavailable(&mut host, revision).unwrap(); }
        else { host.observe_time(host.revision(), ElapsedTick(11)).unwrap(); }
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
        assert!(host.complete_learned_step(host.revision(), n.actor_revision, n.position).is_err());
        assert!(host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).is_err());
        assert_eq!(host.learned_generation_inspection().unwrap(), before); assert_eq!(disk(&host), bytes);
        assert_eq!(before.numerical.work.sampling_attempts, 0); assert!(before.pending.is_some());
        drop(host);
        let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert!(recovered.action_consistency_snapshot().unwrap().coverage_lost);
        assert!(recovered.learned_generation_inspection().unwrap().paused);
        assert!(recovered.advance_learned_generation(recovered.revision(), n.actor_revision, n.position).is_err());
        assert_eq!(recovered.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 0);
    }
}

#[test]
fn optional_handle_operates_under_required_mode_but_dropping_it_cannot_disable_the_contract() {
    let root = Directory::new(); let (mut host, role, _) = setup(&root, true);
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    assert!(role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).is_err());
    step(&mut host); let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    let mut continuation = role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).unwrap().unwrap();
    let revision = host.revision(); continuation.advance(&mut host, revision).unwrap().unwrap();
    drop(continuation);
    step(&mut host); // the durable pending forecast, not local handle custody, is the requirement
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision(); let bytes = disk(&host);
    assert_eq!(role.forecast_hosted_request(&mut host, revision, 72, n.actor_revision).err(), Some(Error::WrongState.into()));
    assert_eq!(disk(&host), bytes); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn requirement_cannot_be_installed_after_sampling_or_on_a_nontext_owner() {
    for plain in [false, true] {
        let root = Directory::new();
        let mut host = if plain { FileOversight::create(root.store(), profile()).unwrap().0 }
            else { FileOversight::create_with_learned_text(root.store(), profile(), config()).unwrap().0 };
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        if !plain { step(&mut host); step(&mut host); }
        let bytes = disk(&host);
        assert!(host.enable_action_consistency(host.revision(), predictor(true)).is_err());
        assert!(!host.action_consistency_required()); assert_eq!(disk(&host), bytes);
        assert!(host.storage_failure().is_none());
    }
    let root = Directory::new(); let (mut positive, role, _) = setup(&root, true);
    step(&mut positive); forecast(&mut positive, &role, 71); step(&mut positive);
    assert_eq!(positive.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 1);
}

mod integration;
