//! Original learned generator, exact K/V forecasts, actor tickets and Store faults.
//! Synthetic model/probability parameters are causal controls, not calibration.
use super::*;
use crate::action::consequence::delivery::persistent::{JournalIo, observed::{
    FileHumanReviewer, journal, storage,
    consistency::{ConsistencyEvent, FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver,
        learned::owned::bind_owned_history},
    decoder::{DecoderEvent, learned::LearnedEvent},
    guarded::{FileGuardSet, FileRecoveryRequirements, FileRecoveryFloor, FileCredentialEpoch}},
    requests::actor::{FileActorSupervisor, LearnedTextProposal}};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use crate::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::oversight::{learned_source::LearnedEvidenceLimits,
    actor::{ActorOutcome, Knowledge}};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::Snapshot;
#[allow(dead_code)]
#[path = "../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
fn predictor(required: bool) -> FileLearnedConsistencyConfig {
    let (model, _, _) = recipe(); let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let raw = FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model.cache_profile().layers()[&1].values().profile(), weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap();
    let owned = FileLearnedConsistencyConfig::new(raw, 1, KvSide::Value,
        LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
        .unwrap().with_owned_generation().unwrap();
    if required { owned.with_pre_output_forecast().unwrap() } else { owned }
}
fn expected(host: &FileOversight, generation: &FileLearnedConfig, prediction: FileLearnedConsistencyConfig)
    -> FileOwnedPredictiveRequirements
{
    let control = host.inspect().control;
    FileOwnedPredictiveRequirements { oversight: FileRecoveryRequirements {
        guards: FileGuardSet { stream: generation.text_stream_profile(), decoder: None, decoder_stop: None,
            source: generation.required_policy_source(), identity: None, campaigns: None, credential: None },
        effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch },
    }, prediction, evaluation: None }
}
fn at_prompt(root: &Directory) -> (FileOversight, FileHumanReviewer, FileConsistencyObserver, FileLearnedConfig) {
    let generation = config();
    let (mut host, human) = FileOversight::create_with_learned_text(root.store(), profile(), generation.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let observer = host.enable_learned_action_consistency(host.revision(), predictor(true)).unwrap();
    step(&mut host);
    (host, human, observer, generation)
}
fn bytes(root: &Directory) -> Vec<u8> { std::fs::read(root.store().join(storage::CANONICAL)).unwrap() }
fn ready(run: &mut FileOwnedPredictiveRecovery) {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress().replayed_events;
        assert_eq!(run.advance(before, 1).unwrap().replayed_events, before + 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
}
fn forecast(host: &mut FileOversight, role: &FileConsistencyObserver, request: u64) {
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    role.forecast_owned_learned_request(host, revision, request, n.actor_revision)
        .unwrap().unwrap().prediction().unwrap();
}
fn independent(host: &FileOversight, generation: &FileLearnedConfig, prediction: &FileLearnedConsistencyConfig) {
    let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &canonical).unwrap();
    bind_owned_history(&mut events, generation, prediction).unwrap();
    let machine = Machine::replay(&host.profile, &events).unwrap();
    assert_eq!(machine.snapshot(events.len()), host.inspect());
    assert_eq!(machine.consistency_snapshot(host.revision()).unwrap(), host.action_consistency_snapshot().unwrap());
    assert_eq!(machine.broker.learned_consistency_work().unwrap(), host.learned_action_consistency_snapshot().unwrap().work);
    assert_eq!(machine.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
}

#[test]
fn bounded_original_replay_fences_once_then_the_new_observer_enables_real_generated_intake() {
    let root = Directory::new(); let (host, _, old_observer, generation) = at_prompt(&root);
    let required = expected(&host, &generation, predictor(true)); let prior = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let disk = bytes(&root); drop(host);
    assert!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(),
        &required.oversight, &generation).is_err());
    let mut run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).unwrap();
    let initial = run.progress();
    assert_eq!(run.advance(1, 1), Err(Error::Stale.into()));
    assert_eq!(run.advance(0, 0), Err(Error::InvalidInput.into()));
    assert_eq!(run.progress(), initial);
    assert!(FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).is_err());
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress().replayed_events;
        assert_eq!(run.advance(before, 1).unwrap().replayed_events, before + 1);
        assert_eq!(bytes(&root), disk);
    }
    let (mut host, roles) = run.finish().unwrap();
    assert_eq!(host.revision(), prior.revision + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.learned_generation_inspection().unwrap().paused); assert!(!host.clock_ready());
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    assert!(roles.oversight.identity_observer.is_none() && roles.oversight.policy_governor.is_none() && roles.evaluator.is_none());
    let revision = host.revision();
    assert_eq!(old_observer.forecast_owned_learned_request(&mut host, revision, 71, numerical.actor_revision).err(),
        Some(Error::Binding.into()));
    assert!(roles.consistency_observer.forecast_owned_learned_request(&mut host, revision, 71, numerical.actor_revision).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    let revision = host.revision();
    assert!(host.begin_learned_step(revision, numerical.actor_revision, numerical.position).is_err());
    assert_eq!(host.revision(), revision);
    forecast(&mut host, &roles.consistency_observer, 71); step(&mut host); step(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    let epoch = host.inspect().control.ledger.epoch;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let mut proposal = proposal(); proposal.expected_policy_epoch = epoch;
    let ticket = port.submit(71, proposal).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
    assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    independent(&supervisor.host().unwrap(), &generation, &required.prediction);
}

#[test]
fn abandonment_and_premature_finish_leave_the_original_lock_and_canonical_history_unchanged() {
    let root = Directory::new(); let (host, _, _, generation) = at_prompt(&root);
    let required = expected(&host, &generation, predictor(true)); let disk = bytes(&root); drop(host);
    let run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).unwrap();
    assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(bytes(&root), disk);
    let mut run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).unwrap();
    run.advance(0, 1).unwrap(); drop(run); assert_eq!(bytes(&root), disk);
    let (host, _) = FileOversight::open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).unwrap();
    assert_eq!(host.revision(), required.oversight.minimum.journal_revision + 1);
    independent(&host, &generation, &required.prediction);
}

#[test]
fn both_recipes_are_bound_before_replay_and_final_guard_failures_never_expose_ready() {
    let root = Directory::new(); let (host, _, _, generation) = at_prompt(&root);
    let required = expected(&host, &generation, predictor(true)); let disk = bytes(&root); drop(host);
    let mut weak = required.clone(); weak.prediction = predictor(false);
    assert!(FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &weak, &generation).is_err());
    let (model, tokenizer, mut source) = recipe(); source.prompt = "q".to_owned();
    let altered = FileLearnedConfig::new_text(model, tokenizer, source,
        crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits::default())
        .unwrap().with_required_sidecar().unwrap();
    assert!(FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &altered).is_err());
    assert_eq!(bytes(&root), disk);
    for field in 0..5 {
        let mut wrong = required.clone();
        match field {
            0 => wrong.oversight.minimum.journal_revision += 1,
            1 => wrong.oversight.minimum.control_sequence += 1,
            2 => wrong.oversight.minimum.authority_epoch += 1,
            3 => wrong.oversight.effective_policy = crate::action::consequence::gate::containment::session::policy::Policy::new(
                2, vec![crate::action::consequence::gate::containment::session::policy::Predicate::PayloadAtMost(1)]).unwrap(),
            _ => wrong.oversight.credential_epoch = Some(FileCredentialEpoch { generation: 1, revoked: false }),
        }
        let mut run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &wrong, &generation).unwrap();
        let error = run.advance(0, run.progress().total_events).unwrap_err();
        assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
        assert_eq!(run.advance(run.progress().replayed_events, 1), Err(error));
        assert!(run.finish().is_err()); assert_eq!(bytes(&root), disk);
    }
    let (host, _) = FileOversight::open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).unwrap();
    independent(&host, &generation, &required.prediction);
}

#[test]
fn pending_forecasts_and_intents_keep_spent_work_and_coverage_loss_under_new_roles() {
    for pending_step in [false, true] {
        let root = Directory::new(); let (mut host, _, role, generation) = at_prompt(&root);
        forecast(&mut host, &role, 71); let n = host.learned_generation_inspection().unwrap().numerical;
        if pending_step { host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap(); }
        let required = expected(&host, &generation, predictor(true));
        let before = host.learned_action_consistency_snapshot().unwrap(); drop(host);
        let (mut host, roles) = FileOversight::open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &required, &generation).unwrap();
        let after = host.learned_action_consistency_snapshot().unwrap();
        assert!(after.consistency.coverage_lost); assert_eq!(after.work, before.work);
        assert_eq!(after.retained_source_bytes, before.retained_source_bytes);
        assert_eq!(after.consistency.evidence, before.consistency.evidence);
        assert_eq!(after.consistency.pending_attempt, before.consistency.pending_attempt);
        assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
        assert_eq!(host.learned_generation_inspection().unwrap().pending.is_some(), pending_step);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
        let revision = host.revision(); let disk = bytes(&root);
        assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
        assert!(roles.consistency_observer.forecast_owned_learned_request(&mut host, revision, 72, n.actor_revision).is_err());
        assert_eq!(bytes(&root), disk); assert_eq!(host.revision(), revision);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn exact_anchor_accepts_a_real_successor_but_rejects_a_same_counter_history_fork() {
    let root = Directory::new(); let (mut host, _, _, generation) = at_prompt(&root);
    let prefix = host.history_anchor().unwrap(); host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let required = expected(&host, &generation, predictor(true)); let full = host.history_anchor().unwrap();
    let disk = bytes(&root); let mut events = host.events.clone();
    *events.last_mut().unwrap() = Event::Core(crate::action::consequence::delivery::persistent::Event::Time(ElapsedTick(3)));
    let fork = journal::encode(&host.profile, host.store.identity(), &events).unwrap(); drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
    assert!(FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation, &full).is_err());
    assert_eq!(bytes(&root), fork);
    std::fs::write(root.store().join(storage::CANONICAL), &disk).unwrap();
    let mut run = FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation, &prefix).unwrap();
    ready(&mut run); assert_eq!(bytes(&root), disk);
    let (host, _) = run.finish().unwrap(); independent(&host, &generation, &required.prediction);
}

#[test]
fn every_original_fence_fault_withholds_roles_and_reopens_without_additional_generation() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (host, _, _, generation) = at_prompt(&root);
        let required = expected(&host, &generation, predictor(true));
        let n = host.learned_generation_inspection().unwrap().numerical; drop(host);
        let mut run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &required, &generation).unwrap();
        ready(&mut run); run.inner.fail_once(barrier);
        assert!(matches!(run.finish(), Err(JournalError::Io(failure)) if failure.operation == barrier));
        let (host, roles) = FileOversight::open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &required, &generation).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
        assert!(roles.evaluator.is_none()); independent(&host, &generation, &required.prediction);
    }
}

#[test]
fn original_numerical_and_forecast_witness_corruption_are_sticky_replay_failures() {
    for numerical in [false, true] {
        let root = Directory::new(); let (mut host, _, observer, generation) = at_prompt(&root);
        forecast(&mut host, &observer, 71);
        let required = expected(&host, &generation, predictor(true)); let disk = bytes(&root);
        let mut events = host.events.clone();
        let witness = events.iter_mut().find_map(|event| match event {
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) if numerical => Some(witness),
            Event::Consistency(ConsistencyEvent::ForecastOwnedLearnedRequest(_, _, Some(witness))) if !numerical => Some(witness),
            _ => None,
        }).unwrap();
        let mut altered = witness.to_vec(); *altered.last_mut().unwrap() ^= 1; *witness = altered.into();
        let changed = journal::encode(&host.profile, host.store.identity(), &events).unwrap(); drop(host);
        std::fs::write(root.store().join(storage::CANONICAL), &changed).unwrap();
        let mut run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &required, &generation).unwrap();
        let error = run.advance(0, run.progress().total_events).unwrap_err();
        assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
        assert_eq!(run.advance(run.progress().replayed_events, 1), Err(error));
        assert!(run.finish().is_err()); assert_eq!(bytes(&root), changed);
        std::fs::write(root.store().join(storage::CANONICAL), &disk).unwrap();
        let mut run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &required, &generation).unwrap();
        ready(&mut run); assert_eq!(bytes(&root), disk);
    }
}

mod lifecycle;
mod pending_reset;
