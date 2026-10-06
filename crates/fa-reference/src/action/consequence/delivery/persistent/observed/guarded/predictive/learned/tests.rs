//! Real learned generation, original guard checks and canonical recovery writes.
//! Synthetic model/forecast parameters are controls, not calibration evidence.
use super::*;
use crate::action::consequence::delivery::persistent::{JournalIo, observed::{
    FileHumanReviewer, journal, storage, machine::Machine,
    decoder::learned::bind_history,
    consistency::{FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver},
    guarded::{FileGuardSet, FileRecoveryRequirements, FileRecoveryFloor}},
    requests::{FileRequestDisposition, actor::{FileActorSupervisor, LearnedTextProposal}}};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::oversight::{learned_source::LearnedEvidenceLimits,
    actor::{ActorOutcome, Knowledge}};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::Snapshot;
#[path = "../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
fn predictor() -> FileConsistencyConfig {
    let (model, _, _) = recipe(); let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model.residual_contract(1).unwrap().profile(), weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap().with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap()
}
fn pinned() -> FileLearnedConfig { config().with_required_pre_output_forecast(predictor()).unwrap() }
fn expected(host: &FileOversight, config: &FileLearnedConfig) -> FilePredictiveRequirements {
    let control = host.inspect().control;
    FilePredictiveRequirements { oversight: FileRecoveryRequirements {
        guards: FileGuardSet { stream: config.text_stream_profile(), decoder: None, decoder_stop: None,
            source: config.required_policy_source(), identity: None, campaigns: None, credential: None },
        effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch },
    }, prediction: config.required_pre_output_forecast().unwrap().clone(), evaluation: None }
}
fn at_prompt(root: &Directory) -> (FileOversight, FileConsistencyObserver, FileLearnedConfig) {
    let config = pinned();
    let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    (host, observer, config)
}
fn bytes(root: &Directory) -> Vec<u8> { std::fs::read(root.store().join(storage::CANONICAL)).unwrap() }
fn ready(run: &mut FilePredictiveLearnedRecovery) {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress().replayed_events;
        let progress = run.advance(before, 1).unwrap();
        assert_eq!(progress.replayed_events, before + 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
}
fn begin_forecast(host: &mut FileOversight, observer: &FileConsistencyObserver) {
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    observer.begin_pre_output_request(host, revision, 71, n.actor_revision, n.position).unwrap().unwrap();
}
fn independent(host: &FileOversight, config: &FileLearnedConfig) {
    let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &canonical).unwrap();
    bind_history(&mut events, config).unwrap();
    let machine = Machine::replay(&host.profile, &events).unwrap();
    assert_eq!(machine.snapshot(events.len()), host.inspect());
    assert_eq!(machine.consistency_snapshot(host.revision()).unwrap(), host.action_consistency_snapshot().unwrap());
    assert_eq!(machine.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
}

#[test]
fn cooperative_recovery_yields_then_fences_once_and_recovers_a_usable_original_observer() {
    let root = Directory::new(); let (host, old_observer, config) = at_prompt(&root);
    let expected = expected(&host, &config); let prior = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let disk = bytes(&root); drop(host);
    // The old base profile does not silently drop a predictive requirement.
    assert!(FileOversight::open_guarded_with_learned_generation(root.store(), profile(),
        &expected.oversight, &config).is_err());
    assert_eq!(bytes(&root), disk);
    let mut run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    assert_eq!(run.progress().replayed_events, 0);
    let initial = run.progress();
    assert_eq!(run.advance(1, 1), Err(Error::Stale.into()));
    assert_eq!(run.advance(0, 0), Err(Error::InvalidInput.into()));
    assert_eq!(run.progress(), initial);
    assert!(FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).is_err(), "one original exclusive writer");
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress().replayed_events; run.advance(before, 1).unwrap();
        assert_eq!(bytes(&root), disk, "partial replay is never a storage operation");
    }
    let (mut host, roles) = run.finish().unwrap();
    assert_eq!(host.revision(), prior.revision + 1); assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(!host.clock_ready()); assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    assert!(roles.oversight.identity_observer.is_none() && roles.oversight.policy_governor.is_none() && roles.evaluator.is_none());
    let revision = host.revision();
    assert_eq!(old_observer.forecast_hosted_request(&mut host, revision, 71, numerical.actor_revision).err(), Some(Error::Binding.into()));
    assert!(roles.consistency_observer.forecast_hosted_request(&mut host, revision, 71, numerical.actor_revision).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    let revision = host.revision();
    assert!(host.begin_learned_step(revision, numerical.actor_revision, numerical.position).is_err());
    assert_eq!(host.revision(), revision, "new observer custody is not a forecast");
    begin_forecast(&mut host, &roles.consistency_observer); step(&mut host); step(&mut host);
    let epoch = host.inspect().control.ledger.epoch;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let mut proposal = proposal(); proposal.expected_policy_epoch = epoch;
    let ticket = port.submit(71, proposal).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
    assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    independent(&supervisor.host().unwrap(), &config);
}

#[test]
fn dropped_or_premature_recovery_never_releases_roles_or_replaces_canonical_bytes() {
    let root = Directory::new(); let (host, _, config) = at_prompt(&root);
    let expected = expected(&host, &config); let disk = bytes(&root); drop(host);
    let run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(bytes(&root), disk);
    let mut run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    run.advance(0, 1).unwrap(); drop(run); assert_eq!(bytes(&root), disk);
    let (host, _) = FileOversight::open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    assert_eq!(host.revision(), expected.oversight.minimum.journal_revision + 1);
}

#[test]
fn mismatched_recipe_predictor_and_final_guard_floors_cannot_be_changed_after_replay() {
    let root = Directory::new(); let (host, _, config) = at_prompt(&root);
    let expected = expected(&host, &config); let disk = bytes(&root); drop(host);
    let weak = fixture::config();
    assert!(FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &weak).is_err());
    let mut wrong = expected.clone();
    wrong.prediction = wrong.prediction.clone().with_terminal_stop(
        crate::action::consequence::oversight::consistency::ConsistencyStopPolicy::new(9, 1, 900).unwrap()).unwrap();
    assert!(FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &wrong, &config).is_err());
    let changed = weak.with_required_pre_output_forecast(wrong.prediction.clone()).unwrap();
    assert!(FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &wrong, &changed).is_err(), "matching caller arguments cannot relabel disk recipe");
    for field in 0..4 {
        let mut wrong = expected.clone();
        match field {
            0 => wrong.oversight.minimum.journal_revision += 1,
            1 => wrong.oversight.minimum.control_sequence += 1,
            2 => wrong.oversight.minimum.authority_epoch += 1,
            _ => wrong.oversight.effective_policy = crate::action::consequence::gate::containment::session::policy::Policy::new(
                2, vec![crate::action::consequence::gate::containment::session::policy::Predicate::PayloadAtMost(1)]).unwrap(),
        }
        let mut run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
            root.store(), profile(), &wrong, &config).unwrap();
        let failure = run.advance(0, run.progress().total_events).unwrap_err();
        assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
        assert_eq!(run.advance(run.progress().replayed_events, 1), Err(failure.clone()));
        assert!(run.finish().is_err()); assert_eq!(bytes(&root), disk);
    }
    let (host, _) = FileOversight::open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    independent(&host, &config);
}

#[test]
fn returned_observer_cannot_rearm_an_unanswered_forecast_or_complete_its_pending_sample() {
    for pending_step in [false, true] {
        let root = Directory::new(); let (mut host, observer, config) = at_prompt(&root);
        begin_forecast(&mut host, &observer);
        let n = host.learned_generation_inspection().unwrap().numerical;
        if pending_step { host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap(); }
        let expected = expected(&host, &config); let evidence = host.action_consistency_snapshot().unwrap(); drop(host);
        let (mut host, roles) = FileOversight::open_predictive_guarded_with_learned_generation(
            root.store(), profile(), &expected, &config).unwrap();
        let recovered = host.action_consistency_snapshot().unwrap();
        assert!(recovered.coverage_lost); assert_eq!(recovered.pending_attempt, evidence.pending_attempt);
        assert_eq!(recovered.evidence, evidence.evidence); assert_eq!(recovered.evidence.samples(), 0);
        assert_eq!(host.learned_generation_inspection().unwrap().pending.is_some(), pending_step);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
        let revision = host.revision(); let disk = bytes(&root);
        assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
        assert!(roles.consistency_observer.forecast_hosted_request(&mut host, revision, 72, n.actor_revision).is_err());
        assert_eq!(host.revision(), revision); assert_eq!(bytes(&root), disk);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn exact_anchor_accepts_real_successors_but_rejects_a_divergent_same_counter_cut() {
    let root = Directory::new(); let (mut host, _, config) = at_prompt(&root);
    let anchor = host.history_anchor().unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let expected = expected(&host, &config); let disk = bytes(&root);
    let fork_anchor = host.history_anchor().unwrap();
    let mut fork = host.events.clone();
    *fork.last_mut().unwrap() = Event::Core(crate::action::consequence::delivery::persistent::Event::Time(ElapsedTick(3)));
    let fork_bytes = journal::encode(&host.profile, host.store.identity(), &fork).unwrap(); drop(host);
    // The original anchor compares actual canonical bytes, not matching counts.
    std::fs::write(root.store().join(storage::CANONICAL), &fork_bytes).unwrap();
    assert!(FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &config, &fork_anchor).is_err());
    assert_eq!(bytes(&root), fork_bytes);
    std::fs::write(root.store().join(storage::CANONICAL), &disk).unwrap();
    let mut run = FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &config, &anchor).unwrap();
    ready(&mut run); assert_eq!(bytes(&root), disk);
    let (host, _) = run.finish().unwrap(); independent(&host, &config);
}

#[test]
fn every_recovery_fence_storage_fault_withholds_all_roles_and_preserves_numerical_history() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (host, _, config) = at_prompt(&root);
        let expected = expected(&host, &config); let n = host.learned_generation_inspection().unwrap().numerical;
        drop(host);
        let mut run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
            root.store(), profile(), &expected, &config).unwrap();
        ready(&mut run); run.inner.fail_once(barrier);
        assert!(matches!(run.finish(), Err(JournalError::Io(failure)) if failure.operation == barrier));
        let (host, roles) = FileOversight::open_predictive_guarded_with_learned_generation(
            root.store(), profile(), &expected, &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
        assert!(roles.evaluator.is_none()); independent(&host, &config);
    }
}

mod lifecycle;
