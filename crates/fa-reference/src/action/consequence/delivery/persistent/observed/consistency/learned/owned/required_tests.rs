//! Actual generated text, original learned audits and canonical replay.
//! The synthetic predictor is a control, not calibration or detector evidence.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use crate::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::delivery::persistent::{JournalIo,
    requests::{FileRequestDisposition, actor::{FileActorSupervisor, LearnedTextProposal}}};
use crate::action::consequence::delivery::persistent::observed::{
    consistency::{FileConsistencyConfig, FileConsistencyParameters},
    decoder::{DecoderEvent, learned::{LearnedEvent, LearnedStepIntent}}};
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::Snapshot;
#[allow(dead_code)]
#[path = "../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];

fn base_predictor(pair: BinaryForecast) -> FileConsistencyConfig {
    let (model, _, _) = recipe(); let tensor = model.cache_profile().layers()[&1].values();
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: tensor.profile(), weights: vec![1.0, 0.0],
        bias: 0.0, threshold: 0.0, forecast: ForecastRegistration {
            domain: 71, generation: 1, policy_generation: 1, event_prefix: b"aa".to_vec(),
            negative: pair, at_threshold: pair, positive: pair,
        }, alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap()
}
fn configure(base: FileConsistencyConfig, required: bool, bytes: usize) -> FileLearnedConsistencyConfig {
    let config = FileLearnedConsistencyConfig::new(base, 1, KvSide::Value,
        LearnedMonitorBudget { encoded_bytes: bytes, ..LearnedMonitorBudget::default() },
        LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap();
    if required { config.with_pre_output_forecast().unwrap() } else { config }
}
fn predictor(required: bool) -> FileLearnedConsistencyConfig {
    configure(base_predictor(BinaryForecast::new(32_768, 32_768).unwrap()), required, MAX_CHECKED_KV_BYTES)
}
fn setup(root: &Directory, required: bool) -> (FileOversight, FileConsistencyObserver, FileLearnedConfig) {
    let generation = config();
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), generation.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let observer = host.enable_learned_action_consistency(host.revision(), predictor(required)).unwrap();
    (host, observer, generation)
}
fn forecast(host: &mut FileOversight, role: &FileConsistencyObserver, request: u64)
    -> Result<Result<LearnedForecastReport, Error>, JournalError>
{
    let n = host.learned_generation_inspection()?.numerical; let revision = host.revision();
    role.forecast_owned_learned_request(host, revision, request, n.actor_revision)
}
fn disk(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn replay(host: &FileOversight, generation: &FileLearnedConfig, predictor: &FileLearnedConsistencyConfig) -> Machine {
    let mut events = journal::decode(&host.profile, host.store.identity(), &disk(host)).unwrap();
    bind_owned_history(&mut events, generation, predictor).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}
fn spec(host: &FileOversight, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: payload.to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}

#[test]
fn timing_is_a_distinct_exact_owned_configuration_not_a_relabelled_raw_predictor() {
    let optional = predictor(false); let required = optional.clone().with_pre_output_forecast().unwrap();
    assert_eq!(&optional.encoded()[..8], b"FALCPRD\x02");
    assert_eq!(&required.encoded()[..8], b"FALCPRD\x03");
    assert_eq!(&required.encoded()[8..], &optional.encoded()[8..]);
    assert!(!optional.requires_pre_output_forecast()); assert!(required.requires_pre_output_forecast());
    assert!(required.uses_owned_generation());
    assert!(!required.consistency().requires_pre_output_forecast());
    assert_eq!(super::super::config::FileLearnedConsistencyConfig::decode(required.encoded()).unwrap(), required);
    assert_eq!(required.clone().with_pre_output_forecast(), Err(Error::Duplicate));
    assert_eq!(required.clone().with_owned_generation(), Err(Error::Duplicate));
    let supplied = FileLearnedConsistencyConfig::new(optional.consistency().clone(), 1, KvSide::Value,
        LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES).unwrap();
    assert_eq!(supplied.with_pre_output_forecast(), Err(Error::Binding));
    for end in 0..required.encoded().len() {
        assert!(super::super::config::FileLearnedConsistencyConfig::decode(&required.encoded()[..end]).is_err());
    }
    let mut extra = required.encoded().to_vec(); extra.push(0);
    assert!(super::super::config::FileLearnedConsistencyConfig::decode(&extra).is_err());
    for version in [0, 4, 255] {
        let mut bytes = required.encoded().to_vec(); bytes[7] = version;
        assert_eq!(super::super::config::FileLearnedConsistencyConfig::decode(&bytes), Err(Error::Binding));
    }
}

#[test]
fn ordinary_cached_and_cooperative_admission_need_a_forecast_without_changing_numerical_history() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, role, generation) = setup(&root, true);
    let (mut control, other_role, _) = setup(&other, false);
    step(&mut host); step(&mut control);
    let n = host.learned_generation_inspection().unwrap().numerical; let before = disk(&host);
    assert_eq!(host.begin_learned_step(host.revision(), n.actor_revision, n.position), Err(Error::Incomplete.into()));
    assert_eq!(host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).err(), Some(Error::Incomplete.into()));
    assert_eq!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).err(), Some(Error::Incomplete.into()));
    assert_eq!(disk(&host), before); assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    let intent = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Begin(LearnedStepIntent {
        actor_revision: n.actor_revision, position: n.position,
    })));
    assert_eq!(replay(&host, &generation, &predictor(true)).apply(&intent).err(), Some(Error::Incomplete));
    let report = forecast(&mut host, &role, 71).unwrap().unwrap(); report.prediction().unwrap();
    forecast(&mut control, &other_role, 71).unwrap().unwrap().prediction().unwrap();
    assert_eq!(report.prediction().unwrap().observation().frame().sequence, 1);
    for _ in 0..2 { step(&mut host); step(&mut control); }
    assert_eq!(host.learned_generation_inspection().unwrap().numerical,
        control.learned_generation_inspection().unwrap().numerical);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    assert_eq!(host.learned_action_consistency_snapshot().unwrap(), control.learned_action_consistency_snapshot().unwrap());
    let reconstructed = replay(&host, &generation, &predictor(true));
    assert_eq!(reconstructed.snapshot(host.events.len()), host.inspect());
    // Equality includes the ORIGINAL numerical result/witness bytes. Only the
    // requirement domain is normalized, not predictions or sampled outcomes.
    let mut normalized = host.events.clone();
    for event in &mut normalized {
        if let Event::Consistency(ConsistencyEvent::EnableLearned(binding)) = event {
            *binding = super::super::Configuration::new(predictor(false));
        }
    }
    assert_eq!(journal::encode(&host.profile, host.store.identity(), &normalized).unwrap(),
        journal::encode(&host.profile, host.store.identity(), &control.events).unwrap());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn only_a_prompt_keyed_forecast_can_be_answered_by_the_actual_completed_message_once() {
    let root = Directory::new(); let (mut host, role, generation) = setup(&root, true);
    let before = disk(&host); assert!(forecast(&mut host, &role, 71).is_err()); assert_eq!(disk(&host), before);
    step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical; let before = disk(&host);
    let revision = host.revision();
    assert_eq!(role.forecast_owned_learned_action(&mut host, revision, 1, n.actor_revision).err(), Some(Error::Binding.into()));
    let revision = host.revision();
    assert!(role.forecast_hosted_request(&mut host, revision, 71, n.actor_revision).is_err());
    assert_eq!(disk(&host), before);
    let invalid = Event::Consistency(ConsistencyEvent::ForecastOwnedLearned(1, n.actor_revision, None));
    assert_eq!(replay(&host, &generation, &predictor(true)).apply(&invalid).err(), Some(Error::Binding));
    forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap();
    let early = spec(&host, b"aa"); let before = disk(&host);
    assert!(host.submit_request(host.revision(), 71, early.clone(), snapshot()).is_err());
    assert_eq!(disk(&host), before); assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    step(&mut host); step(&mut host);
    let wrong = spec(&host, b"zz"); let before = disk(&host);
    assert_eq!(host.submit_request(host.revision(), 71, wrong, snapshot()).err(), Some(Error::Binding.into()));
    assert_eq!(host.submit_request(host.revision(), 72, early, snapshot()).err(), Some(Error::Binding.into()));
    assert!(forecast(&mut host, &role, 72).is_err()); assert_eq!(disk(&host), before);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
    let outcome = port.poll(&ticket); let retry = port.submit(71, proposal()).unwrap();
    assert_eq!(port.poll(&retry), outcome);
    let host = supervisor.host().unwrap();
    assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::Admitted { .. }));
    assert_eq!(host.learned_action_consistency_observation(1).unwrap().prediction().observation().frame().sequence, 1);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn reconstruction_cannot_accept_legacy_sample_first_history_under_the_required_contract() {
    let root = Directory::new(); let (mut host, _, generation) = setup(&root, false);
    step(&mut host); step(&mut host);
    let mut events = host.events.clone();
    for event in &mut events {
        if let Event::Consistency(ConsistencyEvent::EnableLearned(binding)) = event {
            *binding = super::super::Configuration::new(predictor(true));
        }
    }
    assert_eq!(Machine::replay(&host.profile, &events).err(), Some(Error::Incomplete));
    assert_eq!(replay(&host, &generation, &predictor(false)).snapshot(host.events.len()), host.inspect());
    let good = Directory::new(); let (mut host, role, generation) = setup(&good, true);
    step(&mut host); forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap(); step(&mut host);
    assert_eq!(replay(&host, &generation, &predictor(true)).snapshot(host.events.len()), host.inspect());
}

#[test]
fn expiry_and_coverage_loss_between_begin_and_complete_leave_the_original_intent_pending() {
    for lost in [false, true] {
        let root = Directory::new(); let (mut host, role, generation) = setup(&root, true);
        step(&mut host); forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
        if lost { let revision = host.revision(); role.unavailable(&mut host, revision).unwrap(); }
        else { host.observe_time(host.revision(), ElapsedTick(11)).unwrap(); }
        let before = disk(&host); let state = host.learned_generation_inspection().unwrap();
        assert!(host.complete_learned_step(host.revision(), n.actor_revision, n.position).is_err());
        assert!(host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).is_err());
        assert_eq!(host.learned_generation_inspection().unwrap(), state); assert_eq!(disk(&host), before);
        assert!(state.pending.is_some()); assert_eq!(state.numerical.work.sampling_attempts, 0);
        drop(host);
        let (mut host, _, _) = FileOversight::open_with_owned_learned_consistency(
            root.store(), profile(), &generation, &predictor(true)).unwrap();
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
        assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 0);
    }
}

#[test]
fn required_contract_refuses_missing_owner_late_attachment_and_stream_mismatch_atomically() {
    for kind in 0..3 {
        let root = Directory::new();
        let mut host = if kind == 0 { FileOversight::create(root.store(), profile()).unwrap().0 }
            else { FileOversight::create_with_learned_text(root.store(), profile(), config()).unwrap().0 };
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        if kind == 1 { step(&mut host); step(&mut host); }
        let settings = if kind == 2 {
            let stream = crate::action::consequence::delivery::stream::StreamProfile::new(61, 1, 8, 32, 128).unwrap();
            configure(base_predictor(BinaryForecast::new(32_768, 32_768).unwrap()).with_stream_messages(stream).unwrap(),
                true, MAX_CHECKED_KV_BYTES)
        } else { predictor(true) };
        let before = disk(&host);
        assert!(host.enable_learned_action_consistency(host.revision(), settings).is_err());
        assert_eq!(disk(&host), before); assert!(!host.action_consistency_required());
        assert!(host.storage_failure().is_none());
    }
    let root = Directory::new(); let (mut host, role, _) = setup(&root, true);
    step(&mut host); forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap(); step(&mut host);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 1);
}

#[test]
fn numerical_budget_refusal_does_not_unlock_generation_and_recovery_cannot_downgrade_the_requirement() {
    let root = Directory::new(); let generation = config();
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), generation.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let settings = configure(base_predictor(BinaryForecast::new(32_768, 32_768).unwrap()), true, 0);
    let role = host.enable_learned_action_consistency(host.revision(), settings.clone()).unwrap(); step(&mut host);
    let report = forecast(&mut host, &role, 71).unwrap().unwrap();
    assert_eq!(report.prediction().unwrap_err(), Error::Limit);
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
    let n = host.learned_generation_inspection().unwrap().numerical; let before = disk(&host);
    assert!(host.begin_learned_step(host.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(disk(&host), before);
    let spent = host.learned_action_consistency_snapshot().unwrap(); drop(host);
    let downgraded = configure(settings.consistency().clone(), false, 0);
    assert!(FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &downgraded).is_err());
    let (mut host, _, role) = FileOversight::open_with_owned_learned_consistency(root.store(), profile(), &generation, &settings).unwrap();
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, spent.work);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().retained_source_bytes, spent.retained_source_bytes);
    assert!(forecast(&mut host, &role, 71).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 0);
}

#[test]
fn forecast_and_both_numerical_writes_never_return_an_unacknowledged_sample() {
    for phase in 0..3 {
        for barrier in BARRIERS {
            let root = Directory::new(); let (mut host, role, generation) = setup(&root, true);
            step(&mut host);
            if phase > 0 { forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap(); }
            let n = host.learned_generation_inspection().unwrap().numerical;
            if phase == 2 { host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap(); }
            let before = host.inspect(); host.store.fail_once(barrier);
            let result = match phase {
                0 => forecast(&mut host, &role, 71).map(|_| ()),
                1 => host.begin_learned_step(host.revision(), n.actor_revision, n.position),
                _ => host.complete_learned_step(host.revision(), n.actor_revision, n.position).map(|_| ()),
            };
            assert!(matches!(result, Err(JournalError::Io(ref error)) if error.operation == barrier));
            assert_eq!(host.inspect(), before);
            assert!(host.learned_generation_inspection().is_err());
            let read = FileOversight::read_publication_with_owned_learned_consistency(
                root.store(), &profile(), &generation, &predictor(true)).unwrap();
            let visible = barrier == JournalIo::DirectorySync;
            assert_eq!(read.pending_request, (phase > 0 || visible).then_some(71));
            assert_eq!(read.consistency.consistency.evidence.samples(), 0);
            assert_eq!(read.publication.executions, 0);
            drop(host);
            let (mut host, _, _) = FileOversight::open_with_owned_learned_consistency(
                root.store(), profile(), &generation, &predictor(true)).unwrap();
            let restored = host.learned_generation_inspection().unwrap();
            assert!(restored.paused); assert!(!host.clock_ready());
            assert_eq!(restored.numerical.work.sampling_attempts, u64::from(phase == 2 && visible));
            assert_eq!(restored.pending.is_some(), (phase == 1 && visible) || (phase == 2 && !visible));
            assert_eq!(host.action_consistency_snapshot().unwrap().coverage_lost, phase > 0 || visible);
            assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, read.consistency.work);
            assert!(host.advance_learned_generation(host.revision(), restored.numerical.actor_revision,
                restored.numerical.position).is_err());
            assert_eq!(host.inspect().control.ledger.available, 100);
        }
    }
}
