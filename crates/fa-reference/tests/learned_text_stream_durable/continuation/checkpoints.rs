//! Original stream rewind within one unchanged confirmed publication boundary.
//! Synthetic tiny-model histories exercise state/authority conservation only;
//! the supervisor-declared incident frontier is not an authenticated detector.
use super::*;
use fa_reference::action::consequence::{
    activation::tensor::kv::{
        decoder::monitoring::restart::KvRestartBudget,
        model::MAX_MODEL_KV_VALUES,
    },
    delivery::persistent::{
        governance::PolicyUpdate,
        observed::{
            containment::FileResetRequest,
            decoder::learned::checkpoint::FileLearnedStreamCheckpoint,
        },
    },
    gate::ReviewBinding,
    oversight::learned_source::LearnedAvailability,
};

fn restart_profile() -> FileOversightProfile {
    let mut result = profile();
    let original = &result.delivery.actor;
    let mut declared = original.profile();
    declared.grade = RestartGrade::FunctionalRestart;
    result.delivery.actor = ActorState::new(declared, original.tokens().to_vec(),
        original.cache().to_vec(), original.sampler().to_vec(), original.next_position()).unwrap();
    result
}

fn restart_owner(root: &Directory, recipe: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(
        root.store(), restart_profile(), recipe.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}

fn reopen_restart(root: &Directory, recipe: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    FileOversight::open_with_learned_text_stream(root.store(), restart_profile(), recipe).unwrap()
}

fn restart_budget() -> KvRestartBudget {
    let model = finite_model();
    KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: finite_text(&model, 6, false).policy.allowance() }
}

fn capture_stream(host: &mut FileOversight, checkpoint: u64) -> FileLearnedStreamCheckpoint {
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_stream_checkpoint(host.revision(), checkpoint,
        numerical.actor_revision, host.inspect().control.ledger.epoch).unwrap()
}

fn reset_instruction(host: &FileOversight, operation: u64) -> FileResetRequest {
    let current = host.inspect();
    FileResetRequest { operation,
        expected_control_sequence: current.control.sequence,
        expected_actor_revision: host.learned_generation_inspection().unwrap().numerical.actor_revision,
        expected_authority_epoch: current.control.ledger.epoch,
        binding: ReviewBinding { round: operation, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![current.target] }
}

fn incident_frontier(host: &mut FileOversight, operation: u64) -> u64 {
    // A real governance transition establishes a later original control
    // sequence. It does not authenticate when a model incident first occurred;
    // the new API deliberately treats that frontier as a supervisor declaration.
    let before = host.inspect();
    let generation = host.current_policy().unwrap().generation() + 1;
    let same_predicates = Policy::new(generation,
        vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap();
    let update = PolicyUpdate::new(operation, before.control.sequence,
        before.control.ledger.epoch, same_predicates).unwrap();
    host.replace_policy(host.revision(), &update).unwrap();
    assert!(host.inspect().control.sequence > before.control.sequence);
    host.inspect().control.sequence
}

#[test]
fn stream_reset_restores_original_numerics_but_requires_fresh_observation_and_both_keys() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let (mut host, reviewer) = restart_owner(&root, &recipe);
    step(&mut host);
    let saved = capture_stream(&mut host, 1);
    let original = host.learned_generation_inspection().unwrap().numerical;
    step(&mut host);
    let incident = incident_frontier(&mut host, 800);
    generate(&mut host);
    let old_action = message(&mut host, 1);
    let (inputs, automatic, offer) = prepared(&mut host, 1, &old_action);
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &offer).unwrap();
    let before = host.learned_generation_inspection().unwrap().numerical;
    let ledger = host.inspect().control.ledger;
    let control = reset_instruction(&host, 900); let revision = host.revision();
    let reset = host.reset_learned_stream_checkpoint(revision, &saved,
        control, restart_budget(), incident).unwrap().unwrap();
    assert_eq!(host.revision(), revision + 2);
    assert!(reset.control.restored);
    assert_eq!(reset.control.incident_count, 1);
    assert_eq!(reset.control.cancelled, vec![1]);
    let restored = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(restored.position, original.position);
    assert_eq!(restored.sampled_draws, original.sampled_draws);
    assert_eq!(restored.work, original.work);
    assert_eq!(restored.cumulative_work, before.cumulative_work);
    assert!(restored.cumulative_telemetry.source_check_values > before.cumulative_telemetry.source_check_values);
    assert_eq!(restored.availability, LearnedAvailability::Empty);
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    assert!(host.inspect().control.ledger.epoch > ledger.epoch);
    assert_eq!(host.inspect().control.ledger.charged, ledger.charged);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    let files = root.files();
    assert!(host.dispatch(host.revision(), &automatic, &human, &old_action, &inputs, snapshot()).is_err());
    assert!(host.propose_learned_text_stream_message(host.revision(), 2,
        ElapsedTick(100), Vec::new(), snapshot()).is_err());
    assert_eq!(root.files(), files);
    generate(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"A");
    let fresh = message(&mut host, 2);
    let inputs = dispatched(&mut host, &reviewer, 2, &fresh);
    publish(&mut host, 2, &inputs, 2); host.reconcile(host.revision(), 2).unwrap();
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(stream.publication.executions, 1);
    assert_eq!(stream.publication.control.ledger.charged, fresh.spec().units);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(FileOversight::read_stream_publication_with_learned_generation(
        root.store(), &restart_profile(), &recipe).unwrap(), stream);
    drop(host);
    let (host, _) = reopen_restart(&root, &recipe);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
}

#[test]
fn a_later_message_rewind_keeps_exact_confirmed_prefix_target_offset_and_spent_rights() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let (mut host, reviewer) = restart_owner(&root, &recipe);
    generate(&mut host);
    let first_charge = confirmed_message(&mut host, &reviewer, 71, "A", 2);
    continue_message(&mut host, 71, 72);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.capture_learned_stream_checkpoint(host.revision(), 1,
        n.actor_revision, host.inspect().control.ledger.epoch).is_err());
    step(&mut host);
    let saved = capture_stream(&mut host, 1);
    assert_eq!(saved.info().position, 4);
    assert_eq!(saved.info().sampled_draws, 3);
    generate(&mut host);
    let incident = incident_frontier(&mut host, 800);
    let before = host.stream_snapshot().unwrap();
    let control = reset_instruction(&host, 900);
    let reset = host.reset_learned_stream_checkpoint(host.revision(), &saved,
        control, restart_budget(), incident).unwrap().unwrap();
    assert!(reset.control.restored);
    let after = host.stream_snapshot().unwrap();
    assert_eq!(after.confirmed, before.confirmed);
    assert_eq!(after.published, before.published);
    assert_eq!(after.publication.target, before.publication.target);
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(after.publication.executions, 1);
    assert_eq!(after.publication.control.ledger.charged, first_charge);
    assert_eq!(after.pending, None);
    generate(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"B");
    let second_charge = confirmed_message(&mut host, &reviewer, 72, "B", 3);
    let finished = host.stream_snapshot().unwrap();
    assert_eq!(finished.confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(finished.publication.executions, 2);
    assert_eq!(finished.publication.control.ledger.charged, first_charge + second_charge);
    let files = root.files();
    assert_eq!(FileOversight::read_stream_publication_with_learned_generation(
        root.store(), &restart_profile(), &recipe).unwrap(), finished);
    assert_eq!(root.files(), files);
}

#[test]
fn pending_unknown_published_confirmed_and_finished_cuts_cannot_rewind_the_audience() {
    for cut in 0..4 {
        let recipe = finite_recipe(6, false, true); let root = Directory::new();
        let (mut host, reviewer) = restart_owner(&root, &recipe);
        step(&mut host); let saved = capture_stream(&mut host, 1);
        let incident = incident_frontier(&mut host, 800);
        generate(&mut host);
        let action = message(&mut host, 1);
        let inputs = dispatched(&mut host, &reviewer, 1, &action);
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        if cut >= 1 { publish(&mut host, 1, &inputs, 2); }
        if cut >= 2 { host.reconcile(host.revision(), 1).unwrap(); }
        if cut == 3 {
            let close = closing(&mut host, 2);
            let inputs = dispatched(&mut host, &reviewer, 2, &close);
            publish(&mut host, 2, &inputs, 3); host.reconcile(host.revision(), 2).unwrap();
            assert!(host.stream_snapshot().unwrap().confirmed.finished());
        }
        let files = root.files(); let stream = host.stream_snapshot().unwrap();
        let numerical = host.learned_generation_inspection().unwrap();
        let usage = host.learned_recovery_usage().unwrap();
        let control = reset_instruction(&host, 900);
        assert!(host.begin_learned_stream_reset(host.revision(), &saved,
            control.clone(), restart_budget(), incident).is_err());
        assert!(host.reset_learned_stream_checkpoint(host.revision(), &saved,
            control, restart_budget(), incident).is_err());
        assert_eq!(root.files(), files);
        assert_eq!(host.stream_snapshot().unwrap(), stream);
        assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(host.pending_learned_reset().unwrap(), None);
        if cut < 2 {
            assert_eq!(stream.pending, Some(1));
            assert_eq!(stream.publication.control.ledger.charged, action.spec().units);
            assert_eq!(stream.publication.control.ledger.stages[&1], ActionState::Unknown);
        }
    }
}

#[test]
fn stream_capture_and_reset_require_preincident_order_and_the_original_owner_kind() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let foreign_root = Directory::new();
    let (mut host, _) = restart_owner(&root, &recipe);
    let (mut foreign, _) = restart_owner(&foreign_root, &recipe);
    step(&mut host); step(&mut foreign);
    let saved = capture_stream(&mut host, 1);
    let other = capture_stream(&mut foreign, 1);
    let info = *saved.info();
    assert!(host.learned_checkpoint(1).is_err());
    assert!(host.capture_learned_checkpoint(host.revision(), 1,
        info.actor_revision, info.authority_epoch).is_err());
    let incident = incident_frontier(&mut host, 800);
    let files = root.files(); let usage = host.learned_recovery_usage().unwrap();
    let control = reset_instruction(&host, 900);
    for invalid in [0, info.control_sequence, host.inspect().control.sequence + 1] {
        assert!(host.begin_learned_stream_reset(host.revision(), &saved,
            control.clone(), restart_budget(), invalid).is_err());
        assert_eq!(root.files(), files);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    }
    assert!(host.check_learned_stream_checkpoint(&other).is_err());
    assert!(host.reset_learned_stream_checkpoint(host.revision(), &other,
        control.clone(), restart_budget(), incident).is_err());
    assert_eq!(root.files(), files);
    assert!(host.reset_learned_stream_checkpoint(host.revision(), &saved,
        control, restart_budget(), incident).unwrap().unwrap().control.restored);
}

#[test]
fn exact_stream_reset_retries_survive_reopen_without_new_audit_incident_or_replacement() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let (mut host, _) = restart_owner(&root, &recipe);
    step(&mut host); let saved = capture_stream(&mut host, 1); step(&mut host);
    let incident = incident_frontier(&mut host, 800);
    let control = reset_instruction(&host, 900); let revision = host.revision();
    let budget = restart_budget();
    let result = host.reset_learned_stream_checkpoint(revision, &saved,
        control.clone(), budget, incident).unwrap().unwrap();
    let files = root.files(); let usage = host.learned_recovery_usage().unwrap();
    for retry_revision in [revision, host.revision()] {
        assert_eq!(host.reset_learned_stream_checkpoint(retry_revision, &saved,
            control.clone(), budget, incident).unwrap().unwrap().control, result.control);
    }
    assert_eq!(host.complete_learned_stream_reset(0, 900).unwrap().unwrap().control, result.control);
    assert!(host.complete_learned_reset(host.revision(), 900).is_err());
    assert!(host.reset_learned_stream_checkpoint(host.revision(), &saved,
        control.clone(), budget, incident + 1).is_err());
    let changed = KvRestartBudget { cache_values: budget.cache_values - 1, ..budget };
    assert!(host.reset_learned_stream_checkpoint(host.revision(), &saved,
        control.clone(), changed, incident).is_err());
    assert_eq!(root.files(), files);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    drop(host);
    let (mut host, _) = reopen_restart(&root, &recipe);
    assert!(host.check_learned_stream_checkpoint(&saved).is_err());
    let reacquired = host.learned_stream_checkpoint(1).unwrap();
    let files = root.files(); let revision = host.revision();
    assert_eq!(host.reset_learned_stream_checkpoint(0, &reacquired,
        control, budget, incident).unwrap().unwrap().control, result.control);
    assert_eq!(root.files(), files);
    assert_eq!(host.revision(), revision);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert!(host.learned_generation_inspection().unwrap().paused);
}

#[test]
fn an_interrupted_stream_intent_cannot_be_completed_under_a_new_recovery_epoch() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let (mut host, _) = restart_owner(&root, &recipe);
    step(&mut host); let saved = capture_stream(&mut host, 1);
    let incident = incident_frontier(&mut host, 800);
    let control = reset_instruction(&host, 900);
    host.begin_learned_stream_reset(host.revision(), &saved,
        control, restart_budget(), incident).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    let files = root.files();
    assert!(host.complete_learned_reset(host.revision(), 900).is_err());
    assert_eq!(root.files(), files); drop(host);
    let (mut host, _) = reopen_restart(&root, &recipe);
    assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 0);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let files = root.files();
    assert!(host.complete_learned_stream_reset(host.revision(), 900).is_err());
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(root.files(), files);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn a_consumed_later_message_request_cannot_be_rebound_after_stream_rewind() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let (mut host, reviewer) = restart_owner(&root, &recipe); generate(&mut host);
    confirmed_message(&mut host, &reviewer, 71, "A", 2);
    continue_message(&mut host, 71, 72); step(&mut host);
    let saved = capture_stream(&mut host, 1);
    let incident = incident_frontier(&mut host, 800);
    generate(&mut host);
    let _ = request(&mut host, 72, Some("B"));
    let files = root.files(); let stream = host.stream_snapshot().unwrap();
    let usage = host.learned_recovery_usage().unwrap();
    let control = reset_instruction(&host, 900);
    assert!(host.begin_learned_stream_reset(host.revision(), &saved,
        control, restart_budget(), incident).is_err());
    assert_eq!(root.files(), files);
    assert_eq!(host.stream_snapshot().unwrap(), stream);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
}

#[test]
fn stream_resets_conserve_incident_count_and_original_repetition_escalation() {
    let recipe = finite_recipe(6, false, true); let root = Directory::new();
    let (mut host, _) = restart_owner(&root, &recipe);
    step(&mut host); let saved = capture_stream(&mut host, 1);
    let incident = incident_frontier(&mut host, 800);
    for index in 0..3 {
        if index != 0 { step(&mut host); }
        let control = reset_instruction(&host, 900 + index);
        let result = host.reset_learned_stream_checkpoint(host.revision(), &saved,
            control, restart_budget(), incident).unwrap().unwrap();
        assert_eq!(result.control.incident_count, index + 1);
        assert_eq!(result.control.restored, index < 2);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, index + 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, (index + 1) as usize);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
    }
    assert!(host.inspect().control.suspended);
    let files = root.files(); let usage = host.learned_recovery_usage().unwrap();
    let control = reset_instruction(&host, 903);
    assert!(host.reset_learned_stream_checkpoint(host.revision(), &saved,
        control, restart_budget(), incident).is_err());
    assert_eq!(root.files(), files);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
}
