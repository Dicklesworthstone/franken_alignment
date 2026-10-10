//! Reset recovery preserves original owned K/V source and lifetime custody.
use super::*;
use crate::action::consequence::activation::tensor::kv::{model::MAX_MODEL_KV_VALUES,
    decoder::monitoring::restart::KvRestartBudget};
use crate::action::consequence::delivery::persistent::observed::{containment::FileResetRequest,
    decoder::learned::checkpoint::FileLearnedResetIntent};
use crate::action::consequence::gate::ReviewBinding;

fn pending(host: &mut FileOversight) -> FileLearnedResetIntent {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let saved = host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    let control = host.inspect().control;
    let intent = FileLearnedResetIntent::for_recovery(1, FileResetRequest {
        operation: 900, expected_control_sequence: control.sequence,
        expected_actor_revision: n.actor_revision, expected_authority_epoch: control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![host.inspect().target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() }).unwrap();
    host.begin_learned_reset(host.revision(), &saved, intent.control().clone(), intent.budget()).unwrap();
    intent
}
fn recovery(root: &Directory, required: &FileOwnedPredictiveRequirements, generation: &FileLearnedConfig)
    -> FileOwnedPredictiveRecovery
{
    let mut run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), required, generation).unwrap();
    ready(&mut run); run
}
fn pinned_at_prompt(root: &Directory) -> (FileOversight, FileConsistencyObserver, FileLearnedConfig) {
    let generation = config().with_required_owned_pre_output_forecast(predictor(true)).unwrap();
    let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast(
        root.store(), profile(), generation.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    (host, observer, generation)
}

#[test]
fn original_owned_reset_recovers_both_historical_and_recipe_pinned_predictors_once() {
    for recipe_pinned in [false, true] {
        let root = Directory::new();
        let (mut host, old_observer, generation) = if recipe_pinned { pinned_at_prompt(&root) }
            else { let (host, _, observer, generation) = at_prompt(&root); (host, observer, generation) };
        let intent = pending(&mut host); let required = expected(&host, &generation, predictor(true));
        let cut = host.revision(); let before = bytes(&root);
        let mut events = journal::decode(&host.profile, host.store.identity(), &before).unwrap();
        bind_owned_history(&mut events, &generation, &required.prediction).unwrap();
        let mut control = Machine::replay(&host.profile, &events).unwrap();
        control.prepare_learned_reset(900).unwrap();
        control.apply(&Event::Core(crate::action::consequence::delivery::persistent::Event::Fence)).unwrap();
        let expected_snapshot = control.snapshot(cut as usize + 2);
        let expected_numerical = control.broker.hosted_learned_generation().unwrap();
        let expected_usage = control.broker.hosted_learned_recovery_usage().unwrap();
        drop(host);
        let run = recovery(&root, &required, &generation); assert_eq!(bytes(&root), before);
        let (mut host, roles) = run.finish_pending_reset(&intent).unwrap();
        assert_eq!(host.inspect(), expected_snapshot);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, expected_numerical);
        assert_eq!(host.learned_recovery_usage().unwrap(), expected_usage);
        assert_eq!(host.revision(), cut + 2); assert!(!host.clock_ready());
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert_eq!(host.learned_reset_result(900).unwrap().unwrap().resumed_stream, Some(22));
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        let revision = host.revision();
        assert_eq!(old_observer.forecast_owned_learned_request(&mut host, revision, 71,
            expected_numerical.actor_revision).err(), Some(Error::Binding.into()));
        assert!(roles.consistency_observer.forecast_owned_learned_request(&mut host, revision, 71,
            expected_numerical.actor_revision).is_err());
        independent(&host, &generation, &required.prediction); drop(host);
        let (host, _) = recovery(&root, &required, &generation).finish_pending_reset(&intent).unwrap();
        assert_eq!(host.revision(), cut + 3); assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap(), expected_usage);
        independent(&host, &generation, &required.prediction);
    }
}

#[test]
fn pending_owned_forecast_loses_coverage_without_refunding_work_or_retained_source() {
    let root = Directory::new(); let (mut host, observer, generation) = pinned_at_prompt(&root);
    forecast(&mut host, &observer, 71);
    let before = host.learned_action_consistency_snapshot().unwrap();
    let intent = pending(&mut host); let required = expected(&host, &generation, predictor(true)); drop(host);
    let (mut host, roles) = recovery(&root, &required, &generation).finish_pending_reset(&intent).unwrap();
    let after = host.learned_action_consistency_snapshot().unwrap();
    assert!(after.consistency.coverage_lost);
    assert_eq!(after.consistency.pending_attempt, before.consistency.pending_attempt);
    assert_eq!(after.consistency.evidence, before.consistency.evidence);
    assert_eq!(after.work, before.work); assert_eq!(after.retained_source_bytes, before.retained_source_bytes);
    assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
    assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    let revision = host.revision(); let disk = bytes(&root);
    assert!(roles.consistency_observer.forecast_owned_learned_request(&mut host, revision, 72, n.actor_revision).is_err());
    assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
    assert_eq!(host.revision(), revision); assert_eq!(bytes(&root), disk);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, before.work);
    independent(&host, &generation, &required.prediction);
}

#[test]
fn owned_reset_refuses_premature_changed_or_previously_fenced_completion() {
    let root = Directory::new(); let (mut host, _, generation) = pinned_at_prompt(&root);
    let intent = pending(&mut host); let required = expected(&host, &generation, predictor(true));
    let disk = bytes(&root); drop(host);
    let run = FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).unwrap();
    assert_eq!(run.finish_pending_reset(&intent).err(), Some(Error::Incomplete.into()));
    assert_eq!(bytes(&root), disk);
    let mut control = intent.control().clone(); control.binding.evidence_root[0] ^= 1;
    let changed = FileLearnedResetIntent::for_recovery(1, control, intent.budget()).unwrap();
    assert_eq!(recovery(&root, &required, &generation).finish_pending_reset(&changed).err(), Some(Error::Binding.into()));
    assert_eq!(bytes(&root), disk);
    let (host, _) = recovery(&root, &required, &generation).finish().unwrap();
    assert!(host.pending_learned_reset().unwrap().unwrap().interrupted); drop(host);
    let disk = bytes(&root);
    assert_eq!(recovery(&root, &required, &generation).finish_pending_reset(&intent).err(), Some(Error::WrongState.into()));
    assert_eq!(bytes(&root), disk);
}

#[test]
fn owned_reset_preserves_atomic_completion_through_every_storage_failure() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, _, generation) = pinned_at_prompt(&root);
        let intent = pending(&mut host); let required = expected(&host, &generation, predictor(true));
        let cut = host.revision(); let before = bytes(&root); drop(host);
        let run = recovery(&root, &required, &generation); run.inner.fail_once(barrier);
        assert!(matches!(run.finish_pending_reset(&intent), Err(JournalError::Io(failure)) if failure.operation == barrier));
        if barrier != JournalIo::DirectorySync { assert_eq!(bytes(&root), before); }
        let (host, _) = recovery(&root, &required, &generation).finish_pending_reset(&intent).unwrap();
        assert_eq!(host.revision(), cut + if barrier == JournalIo::DirectorySync { 3 } else { 2 });
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert!(!host.clock_ready()); assert!(host.pending_learned_reset().unwrap().is_none());
        independent(&host, &generation, &required.prediction);
    }
}
