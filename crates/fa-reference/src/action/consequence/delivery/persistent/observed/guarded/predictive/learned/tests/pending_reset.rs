//! The original reset plus one original fence under the pinned predictor contract.
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
fn recovery(root: &Directory, required: &FilePredictiveRequirements, config: &FileLearnedConfig)
    -> FilePredictiveLearnedRecovery
{
    let mut run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), required, config).unwrap();
    ready(&mut run); run
}

#[test]
fn completes_the_original_reset_atomically_and_exact_retry_does_not_repeat_it() {
    let root = Directory::new(); let (mut host, old_observer, config) = at_prompt(&root);
    let intent = pending(&mut host); let required = expected(&host, &config);
    let cut = host.revision(); let before = bytes(&root);
    let mut events = journal::decode(&host.profile, host.store.identity(), &before).unwrap();
    bind_history(&mut events, &config).unwrap();
    let mut control = Machine::replay(&host.profile, &events).unwrap();
    control.prepare_learned_reset(900).unwrap();
    control.apply(&Event::Core(crate::action::consequence::delivery::persistent::Event::Fence)).unwrap();
    let expected_snapshot = control.snapshot(cut as usize + 2);
    let expected_numerical = control.broker.hosted_learned_generation().unwrap();
    let expected_usage = control.broker.hosted_learned_recovery_usage().unwrap();
    drop(host);
    let run = recovery(&root, &required, &config); assert_eq!(bytes(&root), before);
    let (mut host, roles) = run.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.inspect(), expected_snapshot);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, expected_numerical);
    assert_eq!(host.learned_recovery_usage().unwrap(), expected_usage);
    assert_eq!(host.revision(), cut + 2); assert!(!host.clock_ready());
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
    assert_eq!(host.learned_reset_result(900).unwrap().unwrap().resumed_stream, Some(22));
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    let revision = host.revision();
    assert_eq!(old_observer.forecast_hosted_request(&mut host, revision, 71,
        expected_numerical.actor_revision).err(), Some(Error::Binding.into()));
    assert!(roles.consistency_observer.forecast_hosted_request(&mut host, revision, 71,
        expected_numerical.actor_revision).is_err(), "custody is not fresh source evidence");
    independent(&host, &config); drop(host);
    let (host, _) = recovery(&root, &required, &config).finish_pending_reset(&intent).unwrap();
    assert_eq!(host.revision(), cut + 3);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap(), expected_usage);
    independent(&host, &config);
}

#[test]
fn pending_forecast_is_not_erased_by_reset_or_returned_observer_custody() {
    let root = Directory::new(); let (mut host, observer, config) = at_prompt(&root);
    begin_forecast(&mut host, &observer);
    let evidence = host.action_consistency_snapshot().unwrap();
    let intent = pending(&mut host); let required = expected(&host, &config); drop(host);
    let (mut host, roles) = recovery(&root, &required, &config).finish_pending_reset(&intent).unwrap();
    let after = host.action_consistency_snapshot().unwrap();
    assert!(after.coverage_lost); assert_eq!(after.pending_attempt, evidence.pending_attempt);
    assert_eq!(after.evidence, evidence.evidence); assert_eq!(after.evidence.samples(), 0);
    assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    let revision = host.revision(); let disk = bytes(&root);
    assert!(roles.consistency_observer.forecast_hosted_request(&mut host, revision, 72, n.actor_revision).is_err());
    assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
    assert_eq!(host.revision(), revision); assert_eq!(bytes(&root), disk);
    assert_eq!(host.inspect().executions, 0); independent(&host, &config);
}

#[test]
fn premature_changed_and_already_fenced_intents_cannot_mutate_canonical_history() {
    let root = Directory::new(); let (mut host, _, config) = at_prompt(&root);
    let intent = pending(&mut host); let required = expected(&host, &config);
    let disk = bytes(&root); drop(host);
    let run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &required, &config).unwrap();
    assert_eq!(run.finish_pending_reset(&intent).err(), Some(Error::Incomplete.into()));
    assert_eq!(bytes(&root), disk);
    let mut control = intent.control().clone(); control.binding.evidence_root[0] ^= 1;
    let changed = FileLearnedResetIntent::for_recovery(1, control, intent.budget()).unwrap();
    assert_eq!(recovery(&root, &required, &config).finish_pending_reset(&changed).err(), Some(Error::Binding.into()));
    assert_eq!(bytes(&root), disk);
    let (host, _) = recovery(&root, &required, &config).finish().unwrap();
    assert!(host.pending_learned_reset().unwrap().unwrap().interrupted); drop(host);
    let disk = bytes(&root);
    assert_eq!(recovery(&root, &required, &config).finish_pending_reset(&intent).err(), Some(Error::WrongState.into()));
    assert_eq!(bytes(&root), disk);
}

#[test]
fn every_reset_completion_write_fault_withholds_roles_and_exact_retry_counts_once() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, _, config) = at_prompt(&root);
        let intent = pending(&mut host); let required = expected(&host, &config);
        let cut = host.revision(); let before = bytes(&root); drop(host);
        let run = recovery(&root, &required, &config); run.inner.fail_once(barrier);
        assert!(matches!(run.finish_pending_reset(&intent), Err(JournalError::Io(failure)) if failure.operation == barrier));
        if barrier != JournalIo::DirectorySync { assert_eq!(bytes(&root), before); }
        let (host, _) = recovery(&root, &required, &config).finish_pending_reset(&intent).unwrap();
        assert_eq!(host.revision(), cut + if barrier == JournalIo::DirectorySync { 3 } else { 2 });
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert!(!host.clock_ready()); assert!(host.pending_learned_reset().unwrap().is_none());
        independent(&host, &config);
    }
}
