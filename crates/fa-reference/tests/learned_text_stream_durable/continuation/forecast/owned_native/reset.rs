//! A fresh restored-prefix audit feeds the ORIGINAL cumulative stream predictor.
//! These tiny-model regressions do not qualify calibration or reset secrecy.
use super::*;
use fa_reference::action::consequence::{
    activation::tensor::kv::{decoder::monitoring::restart::KvRestartBudget, model::MAX_MODEL_KV_VALUES},
    delivery::persistent::{FileDeliverySnapshot, RecoveryReserve, observed::{
        containment::FileResetRequest,
        decoder::learned::{FileLearnedRecoveryStatus, FileLearnedStreamCheckpointRequest,
            FileLearnedStreamControlReceipt, FileLearnedStreamNarrowControl, FileLearnedStreamNarrowRequest,
            checkpoint::{FileLearnedCheckpointInfo, FileLearnedResetIntent, FileLearnedStreamCheckpoint}},
        guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            anchored::FileHistoryAnchor,
            predictive::owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements}},
    }},
    gate::ReviewBinding,
    oversight::learned_source::LearnedAvailability,
};

fn restart_profile() -> FileOversightProfile {
    let mut value = profile();
    let actor = &value.delivery.actor;
    let mut selected = actor.profile(); selected.grade = RestartGrade::FunctionalRestart;
    value.delivery.actor = ActorState::new(selected, actor.tokens().to_vec(), actor.cache().to_vec(),
        actor.sampler().to_vec(), actor.next_position()).unwrap();
    value
}

fn recipe() -> FileLearnedConfig {
    pinned(true).with_owned_forecast_stream_restart_audit().unwrap()
}

fn start(root: &Directory, recipe: &FileLearnedConfig)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver)
{
    let (mut host, human, observer) = FileOversight::create_with_pre_output_forecast_with_reserve(
        root.store(), restart_profile(), recipe.clone(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human, observer)
}

fn capture(host: &mut FileOversight) -> FileLearnedStreamCheckpoint {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_stream_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}

fn selection(host: &FileOversight) -> FileLearnedStreamCheckpointRequest {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control;
    FileLearnedStreamCheckpointRequest::new(host.revision(), FileLearnedCheckpointInfo {
        checkpoint: 1, actor_revision: n.actor_revision, authority_epoch: control.ledger.epoch,
        control_sequence: control.sequence, stream: 21, position: n.position,
        sampled_draws: n.sampled_draws,
    }).unwrap()
}

fn retained(target: ResolvedTarget) -> Vec<ResolvedTarget> {
    // Target identity includes endpoint version. Explicitly retain the exact
    // A, B and Finish envelopes; no later reset may widen this selection.
    (0..3).map(|offset| ResolvedTarget { expected_version: target.expected_version + offset, ..target }).collect()
}

fn narrow_at(snapshot: &FileDeliverySnapshot, checkpoint: FileLearnedCheckpointInfo)
    -> FileLearnedStreamNarrowRequest
{
    FileLearnedStreamNarrowRequest::new(snapshot.revision, checkpoint, FileLearnedStreamNarrowControl {
        operation: 800, expected_control_sequence: snapshot.control.sequence,
        expected_actor_revision: checkpoint.actor_revision, expected_authority_epoch: snapshot.control.ledger.epoch,
        binding: ReviewBinding { round: 800, reducer_generation: 1, evidence_root: [7; 32] },
        retained_targets: retained(snapshot.target),
    }).unwrap()
}

fn incident(host: &mut FileOversight) -> u64 {
    let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap().numerical;
    let policy = host.current_policy().unwrap().clone(); let usage = host.learned_recovery_usage().unwrap();
    let request = narrow_at(&before, *host.learned_stream_checkpoint(1).unwrap().info());
    let receipt = host.narrow_learned_stream_checkpoint(&request).unwrap();
    assert!(receipt.sequence > request.checkpoint().control_sequence);
    assert_eq!(receipt.sequence, before.control.sequence + 1);
    assert_eq!(receipt.revocation_floor, before.control.ledger.epoch + 1);
    assert_eq!(receipt.incident_count, 0);
    assert_eq!(host.current_policy().unwrap(), &policy);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.inspect().executions, 0);
    receipt.sequence
}

fn later_narrow(host: &FileOversight, saved: &FileLearnedStreamCheckpoint) -> FileLearnedStreamNarrowRequest {
    let base = narrow_at(&host.inspect(), *saved.info()); let mut control = base.control().clone();
    control.operation = 801; control.binding.round = 801;
    control.expected_actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    FileLearnedStreamNarrowRequest::new(base.revision(), base.checkpoint(), control).unwrap()
}

fn instruction_at(snapshot: &FileDeliverySnapshot, actor_revision: u64) -> FileLearnedResetIntent {
    let control = &snapshot.control;
    let budget = KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: finite_text(&finite_model(), 4, false).policy.allowance() };
    FileLearnedResetIntent::for_recovery(1, FileResetRequest { operation: 900,
        expected_control_sequence: control.sequence,
        expected_actor_revision: actor_revision,
        expected_authority_epoch: control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: retained(snapshot.target) }, budget).unwrap()
}

fn instruction(host: &FileOversight) -> FileLearnedResetIntent {
    instruction_at(&host.inspect(), host.learned_generation_inspection().unwrap().numerical.actor_revision)
}

fn requirements(host: &FileOversight, recipe: &FileLearnedConfig) -> FileOwnedPredictiveRequirements {
    let control = host.inspect().control;
    FileOwnedPredictiveRequirements {
        oversight: FileRecoveryRequirements {
            guards: FileGuardSet { stream: recipe.text_stream_profile(), decoder: None,
                decoder_stop: None, source: None, identity: None, campaigns: None, credential: None },
            effective_policy: host.current_policy().unwrap().clone(), credential_epoch: None,
            minimum: FileRecoveryFloor { journal_revision: host.revision(),
                control_sequence: control.sequence, authority_epoch: control.ledger.epoch },
        }, prediction: recipe.required_owned_pre_output_forecast().unwrap().clone(), evaluation: None,
    }
}

fn begin(root: &Directory, recipe: &FileLearnedConfig, required: &FileOwnedPredictiveRequirements,
    anchor: &FileHistoryAnchor) -> Result<FileOwnedPredictiveRecovery, JournalError>
{
    FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
        root.store(), restart_profile(), required, recipe, anchor)?
        .require_recovery_reserve(RecoveryReserve::terminal())
}

fn ready(root: &Directory, recipe: &FileLearnedConfig, required: &FileOwnedPredictiveRequirements,
    anchor: &FileHistoryAnchor) -> FileOwnedPredictiveRecovery
{
    let mut recovery = begin(root, recipe, required, anchor).unwrap();
    while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
        recovery.advance(recovery.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(recovery.progress().status, FileLearnedRecoveryStatus::Ready);
    recovery
}

#[test]
fn prompt_reset_then_fresh_forecast_preserves_two_native_reviewed_messages_and_finish() {
    let root = Directory::new(); let recipe = recipe();
    let (mut host, _, old_observer) = start(&root, &recipe);
    step(&mut host); let saved = capture(&mut host);
    let first = incident(&mut host); let intent = instruction(&host);
    let before = host.learned_generation_inspection().unwrap().numerical;
    let audience = host.stream_snapshot().unwrap();
    let evidence = host.action_consistency_snapshot().unwrap().evidence;
    host.begin_learned_stream_reset(host.revision(), &saved, intent.control().clone(), intent.budget(), first).unwrap();
    let required = requirements(&host, &recipe); let anchor = host.history_anchor().unwrap();
    let revision = host.revision(); drop(host);
    let (mut host, roles) = ready(&root, &recipe, &required, &anchor)
        .finish_pending_stream_reset(&intent, first).unwrap();
    assert_eq!(host.revision(), revision + 2);
    let result = host.learned_reset_result(900).unwrap().unwrap();
    assert!(result.control.restored); assert_eq!(result.resumed_stream, Some(22));
    let original = result.restart.kv().audit().unwrap();
    let fresh = result.restored_forecast_audit.as_ref().unwrap();
    assert!(original.monitoring().source().descriptor().layers().values().all(|row| row.stream == 21));
    assert!(fresh.monitoring().source().descriptor().layers().values().all(|row| row.stream == 22));
    assert!(fresh.monitoring().complete_quiet());
    assert_eq!((fresh.monitoring().first_position(), fresh.monitoring().end_position()), (0, 1));
    let restored = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(restored.work, before.work); assert_eq!(restored.cumulative_work, before.cumulative_work);
    assert_eq!(restored.sampled_draws, 0); assert_eq!(restored.availability, LearnedAvailability::Empty);
    let current = host.stream_snapshot().unwrap();
    assert_eq!(current.confirmed, audience.confirmed); assert_eq!(current.published, audience.published);
    assert_eq!(current.publication.target, audience.publication.target);
    assert_eq!(current.publication.control.ledger.charged, audience.publication.control.ledger.charged);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(usage.restart_attempts, 1);
    assert_eq!(usage.completed_audit.compression_encoded_bytes,
        (original.compression().encoded_bytes + fresh.compression().encoded_bytes) as u64);
    assert_eq!(usage.reserved_audit.compression_source_values,
        2 * intent.budget().audit.preparation.compression.source_values as u64);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert!(!host.clock_ready()); assert!(host.learned_generation_inspection().unwrap().paused);
    resume(&mut host, 1); no_sample(&mut host, &root);
    assert_eq!(forecast(&mut host, &old_observer, true, 71).err(), Some(Error::Binding.into()));
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    let forecast = roles.consistency_observer.forecast_owned_learned_request(
        &mut host, revision, 71, n.actor_revision).unwrap().unwrap();
    assert_eq!(forecast.prediction().unwrap().observation().frame().stream, 22);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
    generate(&mut host);
    let (attempt, action, input, automatic, human) = reviewed(&mut host, &roles.oversight.human, 71, Some("A"));
    assert!(host.publish_checked(host.revision(), attempt, Some(&input), snapshot(), ElapsedTick(1)).is_err());
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    publish(&mut host, attempt, &input, 1); host.reconcile(host.revision(), attempt).unwrap();
    let first_charge = action.spec().units;
    continue_message(&mut host, 71, 72); no_sample(&mut host, &root);
    assert_eq!(generate_with_forecast(&mut host, &roles.consistency_observer, true, 72), 3);
    let second_charge = effect(&mut host, &roles.oversight.human, 72, Some("B"));
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.sampled_draws, 4);
    let model = finite_model(); let mut source = finite_text(&model, 4, false);
    source.stop_tokens = BTreeSet::from([OTHER_CONTROL]);
    let mut control = model.observed_learned_text_generation(tokenizer(&model), source).unwrap();
    control.run_to_stop().unwrap();
    assert_eq!(numerical.work, control.work());
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().evidence().tokens(),
        control.accepted_tokens());
    assert_eq!(super::forecast(&mut host, &roles.consistency_observer, true, 73).unwrap().unwrap(), 5);
    let finish_charge = effect(&mut host, &roles.oversight.human, 73, None);
    let completed = host.stream_snapshot().unwrap();
    assert_eq!(completed.confirmed, completed.published); assert!(completed.confirmed.finished());
    assert_eq!(completed.confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(host.inspect().executions, 3);
    assert_eq!(host.inspect().control.ledger.charged, first_charge + second_charge + finish_charge);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 3);
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(FileOversight::read_stream_publication_with_learned_generation(
        root.store(), &restart_profile(), &recipe).unwrap(), completed);
    let checked = FileOversight::read_publication_with_owned_learned_consistency(root.store(),
        &restart_profile(), &recipe, recipe.required_owned_pre_output_forecast().unwrap()).unwrap();
    assert_eq!(checked.publication, host.inspect());
    assert_eq!(checked.consistency, host.learned_action_consistency_snapshot().unwrap());
}

#[test]
fn offline_owned_capture_and_begin_keep_exact_custody_and_zero_event_retries() {
    let root = Directory::new(); let recipe = recipe(); let (mut host, _, _) = start(&root, &recipe);
    step(&mut host); let request = selection(&host);
    let required = requirements(&host, &recipe); let anchor = host.history_anchor().unwrap();
    let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap().numerical;
    drop(host);
    let captured = ready(&root, &recipe, &required, &anchor).finish_captured_stream_checkpoint(request).unwrap();
    assert_eq!(captured.appended_events(), 1);
    assert_eq!(captured.receipt(), &FileLearnedStreamControlReceipt::Captured(request.checkpoint()));
    assert_eq!(captured.snapshot().revision, before.revision + 1);
    assert_eq!(captured.snapshot().control, before.control);
    assert_eq!(captured.numerical(), &numerical);
    let bytes = root.files();
    let repeated = ready(&root, &recipe, &required, &anchor).finish_captured_stream_checkpoint(request).unwrap();
    assert_eq!(repeated.appended_events(), 0); assert_eq!(repeated.history_anchor(), captured.history_anchor());
    assert_eq!(root.files(), bytes);
    let wrong = FileLearnedStreamCheckpointRequest::new(request.revision() + 1, request.checkpoint()).unwrap();
    assert_eq!(ready(&root, &recipe, &required, &anchor).finish_captured_stream_checkpoint(wrong).err(),
        Some(Error::Binding.into()));
    assert_eq!(root.files(), bytes);
    // Capture -> real narrowing -> begin are all immutable offline results.
    // No intermediate recovery Fence can erase the original prompt source.
    let narrow = narrow_at(captured.snapshot(), request.checkpoint());
    let narrowed = ready(&root, &recipe, &required, &anchor).finish_narrowed_stream_checkpoint(&narrow).unwrap();
    let first = match narrowed.receipt() {
        FileLearnedStreamControlReceipt::Narrowed { request, receipt } => {
            assert_eq!(request, &narrow); assert_eq!(receipt.incident_count, 0); receipt.sequence
        }
        _ => panic!("original narrowing receipt"),
    };
    assert_eq!(narrowed.appended_events(), 1); assert_eq!(narrowed.numerical(), &numerical);
    let bytes = root.files();
    let repeated = ready(&root, &recipe, &required, &anchor).finish_narrowed_stream_checkpoint(&narrow).unwrap();
    assert_eq!(repeated.appended_events(), 0); assert_eq!(repeated.receipt(), narrowed.receipt());
    assert_eq!(root.files(), bytes);
    let intent = instruction_at(narrowed.snapshot(), numerical.actor_revision);
    let before = narrowed.snapshot().clone();
    let begun = ready(&root, &recipe, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(begun.appended_events(), 1); assert_eq!(begun.snapshot().revision, before.revision + 1);
    assert_eq!(begun.snapshot().control, before.control); assert_eq!(begun.numerical(), &numerical);
    assert_eq!(begun.recovery_usage().restart_attempts, 0);
    let bytes = root.files();
    let repeated = ready(&root, &recipe, &required, &anchor).finish_narrowed_stream_checkpoint(&narrow).unwrap();
    assert_eq!(repeated.appended_events(), 0); assert_eq!(repeated.receipt(), narrowed.receipt());
    assert_eq!(repeated.recovery_usage(), narrowed.recovery_usage()); assert_eq!(root.files(), bytes);
    let retry = ready(&root, &recipe, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(retry.appended_events(), 0); assert_eq!(retry.history_anchor(), begun.history_anchor());
    assert_eq!(root.files(), bytes);
    assert_eq!(ready(&root, &recipe, &required, &anchor).finish_pending_reset(&intent).err(),
        Some(Error::Binding.into()));
    assert_eq!(ready(&root, &recipe, &required, &anchor).finish_pending_stream_reset(&intent, first + 1).err(),
        Some(Error::Binding.into()));
    assert_eq!(root.files(), bytes);
    let (mut host, roles) = ready(&root, &recipe, &required, &anchor).finish_pending_stream_reset(&intent, first).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    let usage = host.learned_recovery_usage().unwrap(); assert_eq!(usage.restart_attempts, 1);
    resume(&mut host, 1); generate_with_forecast(&mut host, &roles.consistency_observer, true, 71);
    effect(&mut host, &roles.oversight.human, 71, Some("A")); drop(host);
    // Historical acknowledgment recovery cannot rerun either audit, cancel a
    // now-published message or count that origin's likelihood evidence twice.
    let bytes = root.files();
    let repeated = ready(&root, &recipe, &required, &anchor).finish_narrowed_stream_checkpoint(&narrow).unwrap();
    assert_eq!(repeated.appended_events(), 0); assert_eq!(repeated.receipt(), narrowed.receipt());
    assert_eq!(repeated.recovery_usage(), usage); assert_eq!(root.files(), bytes);
    let retry = ready(&root, &recipe, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(retry.appended_events(), 0); assert_eq!(retry.recovery_usage(), usage);
    assert!(matches!(retry.receipt(), FileLearnedStreamControlReceipt::ResetBegun { already_completed: true, .. }));
    assert_eq!(root.files(), bytes);
    let (host, _) = ready(&root, &recipe, &required, &anchor).finish_pending_stream_reset(&intent, first).unwrap();
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
}

#[test]
fn stream_restart_recipe_is_explicit_owned_and_independently_bound_before_replay() {
    let legacy = pinned(true); let selected = recipe();
    assert!(!legacy.requires_owned_forecast_stream_restart_audit());
    assert!(selected.requires_owned_forecast_stream_restart_audit());
    assert!(selected.requires_owned_forecast_restart_audit());
    assert!(selected.follows_forecast_reset_successors());
    assert_ne!(selected, legacy);
    assert_eq!(selected.clone().with_owned_forecast_stream_restart_audit().err(), Some(Error::Duplicate));
    assert_eq!(pinned(false).with_owned_forecast_stream_restart_audit().err(), Some(Error::Binding));
    assert_eq!(finite_recipe(4, false, false).with_owned_forecast_stream_restart_audit().err(), Some(Error::Binding));
    assert_eq!(legacy.clone().with_forecast_reset_successors().err(), Some(Error::Binding));
    assert_eq!(legacy.clone().with_owned_forecast_restart_audit().err(), Some(Error::Binding));
    let model = finite_model();
    let nonstream = FileLearnedConfig::new_text(model.clone(), tokenizer(&model), finite_text(&model, 4, false),
        LearnedDecoderBindingLimits::default()).unwrap();
    assert_eq!(nonstream.with_owned_forecast_stream_restart_audit().err(), Some(Error::Binding));
    for owned in [false, true] {
        let root = Directory::new(); let config = pinned(owned);
        let (mut host, _, _) = start(&root, &config); step(&mut host);
        let before = root.files(); let n = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(host.capture_learned_stream_checkpoint(host.revision(), 1, n.actor_revision,
            host.inspect().control.ledger.epoch).err(), Some(Error::Incomplete.into()));
        assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 0); assert_eq!(root.files(), before);
    }
    let root = Directory::new(); let (mut host, _, _) = start(&root, &selected); step(&mut host);
    let request = selection(&host); let required = requirements(&host, &selected);
    let anchor = host.history_anchor().unwrap(); let bytes = root.files(); drop(host);
    assert!(begin(&root, &legacy, &required, &anchor).is_err()); assert_eq!(root.files(), bytes);
    assert_eq!(ready(&root, &selected, &required, &anchor).finish_captured_stream_checkpoint(request)
        .unwrap().appended_events(), 1);
}

#[test]
fn pending_forecast_and_answered_sampling_cannot_be_reset_or_captured_again() {
    let root = Directory::new(); let recipe = recipe(); let (mut host, _, observer) = start(&root, &recipe);
    step(&mut host); let saved = capture(&mut host); let first = incident(&mut host);
    forecast(&mut host, &observer, true, 71).unwrap().unwrap();
    let intent = instruction(&host); let before = root.files(); let revision = host.revision();
    let prediction = host.learned_action_consistency_snapshot().unwrap();
    let narrow = later_narrow(&host, &saved);
    assert_eq!(host.narrow_learned_stream_checkpoint(&narrow).err(), Some(Error::Incomplete.into()));
    assert_eq!(host.begin_learned_stream_reset(revision, &saved, intent.control().clone(), intent.budget(), first),
        Err(Error::Incomplete.into()));
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(host.capture_learned_stream_checkpoint(revision, 2, n.actor_revision,
        host.inspect().control.ledger.epoch).err(), Some(Error::Incomplete.into()));
    assert_eq!(host.revision(), revision); assert_eq!(root.files(), before);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap(), prediction);
    generate(&mut host); super::super::super::request(&mut host, 71, Some("A"));
    assert!(host.pending_forecast_request().unwrap().is_none());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    let intent = instruction(&host); let revision = host.revision(); let bytes = root.files();
    let prediction = host.learned_action_consistency_snapshot().unwrap();
    let narrow = later_narrow(&host, &saved);
    assert_eq!(host.narrow_learned_stream_checkpoint(&narrow).err(), Some(Error::WrongState.into()));
    assert_eq!(host.begin_learned_stream_reset(revision, &saved, intent.control().clone(), intent.budget(), first),
        Err(Error::WrongState.into()));
    assert_eq!(host.revision(), revision); assert_eq!(root.files(), bytes);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap(), prediction);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn only_the_complete_unsampled_original_prompt_can_be_captured() {
    let model = finite_model(); let mut source = finite_text(&model, 4, false);
    source.prompt = "PPP".to_owned(); // two original tokens after PP merges
    let base = FileLearnedConfig::new_text_stream(model.clone(), tokenizer(&model), source,
        LearnedDecoderBindingLimits::default(), stream()).unwrap().with_required_sidecar().unwrap();
    let recipe = Forecast::new(true).pin(base).unwrap().with_forecast_text_stream_continuation().unwrap()
        .with_owned_forecast_stream_restart_audit().unwrap();
    let root = Directory::new(); let (mut host, _, observer) = start(&root, &recipe); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(n.position, 1); assert_eq!(n.status, GenerationStatus::Prefilling);
    let before = root.files();
    assert_eq!(host.capture_learned_stream_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).err(), Some(Error::WrongState.into()));
    assert_eq!(root.files(), before); assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 0);
    step(&mut host); let saved = capture(&mut host);
    assert_eq!((saved.info().position, saved.info().sampled_draws), (2, 0));
    forecast(&mut host, &observer, true, 71).unwrap().unwrap(); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical; let before = root.files();
    assert!(host.capture_learned_stream_checkpoint(host.revision(), 2, n.actor_revision,
        host.inspect().control.ledger.epoch).is_err());
    assert_eq!(root.files(), before); assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
}

#[test]
fn exhausted_forecast_acquisition_cannot_gain_fresh_coverage_or_work_through_reset() {
    for exhausted in [false, true] {
        let Forecast::Learned(original) = Forecast::new(true) else { panic!(); };
        let lifetime = if exhausted { LearnedMonitorBudget { probe_coordinates: 0,
            ..LearnedMonitorBudget::default() } } else { LearnedMonitorBudget::default() };
        let predictor = FileLearnedConsistencyConfig::new(original.consistency().clone(), 1, KvSide::Value,
            LearnedMonitorBudget::default(), lifetime, MAX_CHECKED_KV_BYTES).unwrap()
            .with_owned_generation().unwrap().with_pre_output_forecast().unwrap();
        let recipe = finite_recipe(4, false, false).with_required_sidecar().unwrap()
            .with_required_owned_pre_output_forecast(predictor).unwrap()
            .with_forecast_text_stream_continuation().unwrap()
            .with_owned_forecast_stream_restart_audit().unwrap();
        let root = Directory::new(); let (mut host, _, observer) = start(&root, &recipe);
        step(&mut host); let saved = capture(&mut host); let first = incident(&mut host);
        if exhausted {
            assert!(forecast(&mut host, &observer, true, 71).unwrap().is_err());
            let prediction = host.learned_action_consistency_snapshot().unwrap();
            assert!(prediction.consistency.coverage_lost); assert!(prediction.retained_source_bytes > 0);
            assert!(host.pending_forecast_request().unwrap().is_none());
            let intent = instruction(&host); let before = root.files();
            assert_eq!(host.begin_learned_stream_reset(host.revision(), &saved,
                intent.control().clone(), intent.budget(), first), Err(Error::Incomplete.into()));
            assert_eq!(host.learned_action_consistency_snapshot().unwrap(), prediction);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
            assert_eq!(root.files(), before);
        } else {
            let intent = instruction(&host);
            let restored = host.reset_learned_stream_checkpoint(host.revision(), &saved,
                intent.control().clone(), intent.budget(), first).unwrap().unwrap();
            assert!(restored.control.restored);
            assert_eq!(forecast(&mut host, &observer, true, 71).unwrap().unwrap(), 1);
            assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
        }
    }
}

#[test]
fn fresh_stream_audit_failure_keeps_both_charges_and_never_exposes_a_forecast_source() {
    let root = Directory::new(); let recipe = recipe(); let (mut host, _, observer) = start(&root, &recipe);
    step(&mut host); let saved = capture(&mut host); let first = incident(&mut host);
    let intent = instruction(&host); let mut budget = intent.budget(); budget.audit.monitoring.probe_coordinates = 0;
    assert!(host.reset_learned_stream_checkpoint(host.revision(), &saved,
        intent.control().clone(), budget, first).unwrap().is_err());
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(usage.restart_attempts, 1);
    assert_eq!(usage.reserved_audit.compression_source_values, 2 * budget.audit.preparation.compression.source_values as u64);
    assert!(host.learned_generation_inspection().unwrap().numerical.host_failure.is_some());
    let before = root.files(); let revision = host.revision();
    assert!(host.reset_learned_stream_checkpoint(revision, &saved,
        intent.control().clone(), budget, first).unwrap().is_err());
    assert_eq!(host.learned_recovery_usage().unwrap(), usage); assert_eq!(host.revision(), revision);
    assert_eq!(root.files(), before);
    assert!(forecast(&mut host, &observer, true, 71).unwrap().is_err());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn exact_guard_policy_predictor_and_replay_cuts_are_required_for_offline_control() {
    let root = Directory::new(); let recipe = recipe(); let (mut host, _, _) = start(&root, &recipe);
    step(&mut host); let saved = capture(&mut host); let first = incident(&mut host);
    let intent = instruction(&host); let required = requirements(&host, &recipe);
    let anchor = host.history_anchor().unwrap(); let bytes = root.files(); drop(host);
    assert_eq!(begin(&root, &recipe, &required, &anchor).unwrap().finish_begun_stream_reset(&intent, first).err(),
        Some(Error::Incomplete.into()));
    assert_eq!(root.files(), bytes);
    for field in 0..5 {
        let mut wrong = required.clone();
        match field {
            0 => wrong.oversight.effective_policy = Policy::new(2,
                vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            1 => wrong.oversight.guards.stream = None,
            2 => wrong.oversight.minimum.authority_epoch += 1,
            3 => wrong.oversight.minimum.journal_revision += 1,
            _ => {
                let prior = wrong.prediction.clone();
                wrong.prediction = FileLearnedConsistencyConfig::new(prior.consistency().clone(), 1,
                    KvSide::Key, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
                    MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
                    .with_pre_output_forecast().unwrap();
            }
        }
        match begin(&root, &recipe, &wrong, &anchor) {
            Err(_) => {},
            Ok(mut recovery) => {
                let mut refused = false;
                while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
                    if recovery.advance(recovery.progress().replayed_events, 1).is_err() {
                        refused = true; break;
                    }
                }
                assert!(refused);
                assert!(recovery.finish_begun_stream_reset(&intent, first).is_err());
            }
        }
        assert_eq!(root.files(), bytes);
    }
    let begun = ready(&root, &recipe, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(begun.appended_events(), 1); assert_eq!(begun.recovery_usage().restart_attempts, 0);
    let (host, _) = ready(&root, &recipe, &required, &anchor).finish_pending_stream_reset(&intent, first).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_reset_result(900).unwrap().unwrap().position, saved.info().position);
}
