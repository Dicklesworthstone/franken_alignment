//! A restored prompt boundary previously deadlocked between forecast and sampling.
//! These tests use actual fresh source audits, native review and two-key effects.
//! Tiny weights and neutral forecasts are controls, not calibration evidence.
use super::*;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    requests::FileRequestDisposition,
    observed::{FileHumanReviewer, helpers::learned::native::{
        NativeReviewLimits, NativeReviewMember, NativeReviewStatus,
    }},
}};
use fa_reference::action::consequence::oversight::{ReviewWindow,
    learned_source::LearnedAvailability,
    learned_host::sidecar::{LearnedSidecarRequest,
        workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarCongressBudget, SidecarIdentity,
        receiver::native::{SidecarDecisionBasis, SidecarProbeQuery}},
};
use std::collections::BTreeSet;

fn configured(model: &DecoderModel) -> FileLearnedConfig {
    recipe(model, source(model), Mode::Owned, true)
        .with_owned_forecast_restart_audit().unwrap()
}

fn prompt_checkpoint(host: &mut FileOversight) -> FileLearnedCheckpoint {
    assert!(step(host).sample().is_none());
    assert!(step(host).sample().is_none());
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((n.position, n.work.sampling_attempts), (2, 0));
    host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}

fn intent(host: &FileOversight, saved: &FileLearnedCheckpoint,
    audit: fa_reference::action::consequence::activation::tensor::kv::decoder::monitoring::LearnedDecoderAllowance)
    -> FileLearnedResetIntent
{
    let n = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control;
    FileLearnedResetIntent::for_recovery(saved.id(), FileResetRequest {
        operation: 900, expected_control_sequence: control.sequence,
        expected_actor_revision: n.actor_revision, expected_authority_epoch: control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![host.inspect().target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit }).unwrap()
}

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }
}

fn publish_reviewed(host: &mut FileOversight, reviewer: &FileHumanReviewer, last: &GenerationEvent) {
    let request = 71;
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
        else { panic!("the original generated action must be admitted"); };
    let source = last.audit().source();
    let n = host.learned_generation_inspection().unwrap().numerical;
    let sidecar = host.begin_learned_sidecar_plan(host.revision(), attempt, n.actor_revision,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1071, generation: 1, transform_id: 7 },
            priority: source.groups().collect(), budget: SidecarCongressBudget::default() }).unwrap();
    let input = host.current_learned_sidecar(&sidecar).unwrap().clone();
    let rows: BTreeSet<_> = source.groups().map(|group| group.row).collect();
    let members = input.views().iter().enumerate().map(|(index, (name, view))| {
        let queries = rows.iter().map(|row| {
            let (frame, heads, channels) = source.row_shape(*row).unwrap();
            SidecarProbeQuery { row: *row, probe: LinearProbe::new(1, 1, frame.profile,
                &vec![0.0; heads * channels], 0.0, 1.0).unwrap() }
        }).collect();
        (name.clone(), NativeReviewMember {
            evaluator: native_fixture::evaluator(view.actual_input().input_profile().clone(), b"allow"),
            queries, salt: vec![16 + index as u8; 32],
        })
    }).collect();
    let mut review = host.begin_native_sidecar_review(host.revision(), sidecar,
        LearnedWorkerRound { round: 101, evidence_root: [7; 32],
            window: ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) } },
        members, NativeReviewLimits::default(), snapshot()).unwrap();
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if review.status() != NativeReviewStatus::Running { break; }
        review.advance(host, review.revision(), ElapsedTick(2), snapshot()).unwrap();
    }
    assert_eq!(review.status(), NativeReviewStatus::Finished);
    assert!(review.records().values().all(|record| record.failure.is_none()));
    assert!(review.records().values().any(|record|
        record.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    assert!(host.publish_checked(host.revision(), attempt, Some(&input), snapshot(), ElapsedTick(2)).is_err(),
        "a new audit, quiet forecast and helper verdict still cannot publish without two keys");
    let request = host.request_human_approval(host.revision(), 1071, attempt, &input, ElapsedTick(30)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    let action = host.request_action(71).unwrap().clone();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), attempt, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), attempt).unwrap();
    assert_eq!(host.inspect().payload, b"OK");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn restored_prompt_audit_forecasts_before_sampling_then_reaches_native_and_human_publication() {
    let root = Directory::new();
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    let source = source(&model); let config = configured(&model);
    let expected_samples = original_samples(&model, source.clone());
    let (mut host, old_observer) = owner(&root, &config);
    let saved = prompt_checkpoint(&mut host);
    let before = host.learned_generation_inspection().unwrap().numerical;
    let intent = begin_reset(&mut host, &saved, source.policy.allowance());
    let required = requirements(&host, &config);
    let evidence = host.action_consistency_snapshot().unwrap().evidence;
    drop(host);
    let (mut host, roles) = recover(&root, &config, Mode::Owned, &required, &intent).unwrap();
    let receipt = host.learned_reset_result(900).unwrap().unwrap();
    let original = receipt.restart.kv().audit().unwrap();
    let fresh = receipt.restored_forecast_audit.as_ref().unwrap();
    assert!(receipt.control.restored);
    assert_eq!(receipt.resumed_stream, Some(22));
    assert!(original.monitoring().source().descriptor().layers().values().all(|layer| layer.stream == 21));
    assert!(fresh.monitoring().source().descriptor().layers().values().all(|layer| layer.stream == 22));
    assert!(fresh.monitoring().complete_quiet());
    assert_eq!((fresh.monitoring().first_position(), fresh.monitoring().end_position()), (0, 2));
    assert_eq!(fresh.monitoring().planned_rows(), 4);
    let restored = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(restored.work, before.work, "source auditing does not repeat inference or draw a token");
    assert_eq!(restored.sampled_draws, 0);
    assert_eq!(restored.availability, LearnedAvailability::Empty);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(usage.restart_attempts, 1);
    assert_eq!(usage.completed_audit.compression_encoded_bytes,
        (original.compression().encoded_bytes + fresh.compression().encoded_bytes) as u64);
    assert_eq!(usage.completed_audit.monitor_probe_coordinates,
        (original.monitoring().work().probe_coordinates + fresh.monitoring().work().probe_coordinates) as u64);
    assert_eq!(usage.reserved_audit.compression_source_values,
        2 * source.policy.allowance().preparation.compression.source_values as u64);
    assert_eq!(host.inspect().executions, 0);
    resume(&mut host);
    assert!(Mode::Owned.forecast(&mut host, &old_observer, 71).is_err());
    let forecast = Mode::Owned.forecast(&mut host, &roles.consistency_observer, 71).unwrap().unwrap();
    assert_eq!((forecast.frame.stream, forecast.frame.position, forecast.frame.sequence), (22, 1, 2));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
    let mut samples = Vec::new(); let mut last = None;
    while host.learned_generation_inspection().unwrap().numerical.status.is_active() {
        let event = step(&mut host);
        samples.push(event.sample().unwrap().clone()); last = Some(event);
    }
    assert_eq!(samples, expected_samples);
    submit_generated(&mut host, 71);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    publish_reviewed(&mut host, &roles.oversight.human, &last.unwrap());
    let replayed = FileOversight::read_publication_with_owned_learned_consistency(
        root.store(), &profile(), &config, config.required_owned_pre_output_forecast().unwrap()).unwrap();
    assert_eq!(replayed.publication, host.inspect());
    assert_eq!(replayed.consistency, host.learned_action_consistency_snapshot().unwrap());
    let revision = host.revision(); let bytes = root.bytes();
    assert!(host.learned_reset_result(900).unwrap().unwrap().restored_forecast_audit.is_some());
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
}

#[test]
fn legacy_successors_keep_the_prompt_boundary_deadlock_and_single_audit_contract() {
    let root = Directory::new();
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    let source = source(&model); let config = recipe(&model, source.clone(), Mode::Owned, true);
    let (mut host, observer) = owner(&root, &config); let saved = prompt_checkpoint(&mut host);
    let request = intent(&host, &saved, source.policy.allowance());
    let receipt = host.reset_learned_checkpoint(host.revision(), &saved,
        request.control().clone(), request.budget()).unwrap().unwrap();
    assert!(receipt.restored_forecast_audit.is_none());
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(usage.reserved_audit.compression_source_values,
        source.policy.allowance().preparation.compression.source_values as u64);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(n.availability, LearnedAvailability::Empty);
    assert_eq!(Mode::Owned.forecast(&mut host, &observer, 71).unwrap().err(), Some(Error::Incomplete));
    let revision = host.revision(); let bytes = root.bytes();
    assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
}

#[test]
fn reset_audit_recipe_refuses_raw_and_implicit_selection_and_binds_recovery() {
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    assert_eq!(recipe(&model, source(&model), Mode::Raw, true).with_owned_forecast_restart_audit(), Err(Error::Binding));
    assert_eq!(recipe(&model, source(&model), Mode::Owned, false).with_owned_forecast_restart_audit(), Err(Error::Binding));
    let old = recipe(&model, source(&model), Mode::Owned, true);
    let config = old.clone().with_owned_forecast_restart_audit().unwrap();
    assert_ne!(config, old);
    assert!(config.requires_owned_forecast_restart_audit());
    assert_eq!(config.clone().with_owned_forecast_restart_audit(), Err(Error::Duplicate));
    let root = Directory::new(); let (mut host, _) = owner(&root, &config);
    let saved = prompt_checkpoint(&mut host);
    let request = begin_reset(&mut host, &saved, source(&model).policy.allowance());
    let required = requirements(&host, &config); let bytes = root.bytes(); drop(host);
    assert!(recover(&root, &old, Mode::Owned, &required, &request).is_err());
    assert_eq!(root.bytes(), bytes);
    let (host, _) = recover(&root, &config, Mode::Owned, &required, &request).unwrap();
    assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
}

#[test]
fn pending_forecasts_and_answered_sample_history_never_become_fresh_statistical_credit() {
    let root = Directory::new();
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    let config = configured(&model); let (mut host, observer) = owner(&root, &config);
    let saved = prompt_checkpoint(&mut host);
    Mode::Owned.forecast(&mut host, &observer, 71).unwrap().unwrap();
    let request = intent(&host, &saved, source(&model).policy.allowance());
    let bytes = root.bytes(); let revision = host.revision();
    let forecast = host.learned_action_consistency_snapshot().unwrap();
    assert_eq!(host.begin_learned_reset(revision, &saved, request.control().clone(), request.budget()),
        Err(Error::Incomplete.into()));
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap(), forecast);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    assert_eq!(finish_samples(&mut host).len(), 3);
    submit_generated(&mut host, 71);
    assert!(host.pending_forecast_request().unwrap().is_none());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    let request = intent(&host, &saved, source(&model).policy.allowance());
    let bytes = root.bytes(); let revision = host.revision();
    let forecast = host.learned_action_consistency_snapshot().unwrap();
    assert_eq!(host.begin_learned_reset(revision, &saved, request.control().clone(), request.budget()),
        Err(Error::WrongState.into()));
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap(), forecast);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
}

#[test]
fn only_the_complete_original_prompt_can_supply_the_new_reset_checkpoint() {
    let root = Directory::new();
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    let config = configured(&model); let (mut host, observer) = owner(&root, &config);
    step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).is_err());
    assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 0);
    step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    Mode::Owned.forecast(&mut host, &observer, 71).unwrap().unwrap(); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.capture_learned_checkpoint(host.revision(), 2, n.actor_revision,
        host.inspect().control.ledger.epoch).is_err());
    assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
}

#[test]
fn failed_audit_keeps_both_reservations_and_cannot_publish_a_successor_source() {
    let root = Directory::new();
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    let config = configured(&model); let (mut host, observer) = owner(&root, &config);
    let saved = prompt_checkpoint(&mut host);
    let mut allowance = source(&model).policy.allowance(); allowance.monitoring.probe_coordinates = 0;
    let request = intent(&host, &saved, allowance);
    assert!(host.reset_learned_checkpoint(host.revision(), &saved,
        request.control().clone(), request.budget()).unwrap().is_err());
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(usage.restart_attempts, 1);
    assert_eq!(usage.reserved_audit.compression_source_values, 2 * allowance.preparation.compression.source_values as u64);
    assert!(host.learned_generation_inspection().unwrap().numerical.host_failure.is_some());
    assert_eq!(Mode::Owned.forecast(&mut host, &observer, 71).unwrap().err(), Some(Error::Incomplete));
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    assert_eq!(host.inspect().executions, 0);
    let bytes = root.bytes(); let revision = host.revision();
    assert!(host.reset_learned_checkpoint(revision, &saved,
        request.control().clone(), request.budget()).unwrap().is_err());
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), bytes);
}
