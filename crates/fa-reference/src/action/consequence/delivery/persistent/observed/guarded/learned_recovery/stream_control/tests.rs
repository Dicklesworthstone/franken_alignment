//! Ordinary prompt containment uses the original numerical owner and journal.
//! Synthetic weights exercise custody/accounting; they do not qualify a model.
use super::*;
use crate::{Error, Snapshot};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::{
        decoder::monitoring::restart::KvRestartBudget,
        experiment::KvSide,
        model::MAX_MODEL_KV_VALUES,
    },
};
use crate::action::consequence::delivery::{
    persistent::{
        JournalIo, RecoveryReserve,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            containment::FileResetRequest,
            consistency::{
                FileConsistencyConfig, FileConsistencyParameters,
                learned::FileLearnedConsistencyConfig,
            },
            decoder::learned::{
                FileLearnedConfig, FileLearnedRecoveryStatus, FileLearnedStreamControlReceipt,
                FileLearnedStreamNarrowControl,
            },
            guarded::{
                FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
                anchored::FileHistoryAnchor,
            },
            journal, storage,
        },
        requests::actor::{FileActorSupervisor, LearnedTextProposal},
    },
    stream::StreamProfile,
};
use crate::action::consequence::gate::ReviewBinding;
use crate::action::consequence::oversight::{
    decoder_monitoring::LearnedDecoderBindingLimits,
    learned_source::LearnedEvidenceLimits,
};

#[path = "../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn stream() -> StreamProfile {
    StreamProfile::new(1, 1, 4, 16, 64).unwrap()
}

fn delivery_profile() -> FileOversightProfile {
    let mut profile = profile();
    profile.delivery.initial_payload.clear();
    profile
}

fn generation(prompt: &str) -> FileLearnedConfig {
    let (model, tokenizer, mut source) = recipe();
    source.prompt = prompt.into();
    FileLearnedConfig::new_text_stream(
        model, tokenizer, source, LearnedDecoderBindingLimits::default(), stream(),
    )
    .unwrap()
    .with_required_sidecar()
    .unwrap()
}

fn guards() -> FileGuardSet {
    FileGuardSet {
        stream: Some(stream()),
        decoder: None,
        decoder_stop: None,
        source: None,
        identity: None,
        campaigns: None,
        credential: None,
    }
}

fn expected(host: &FileOversight) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements {
        guards: guards(),
        effective_policy: delivery_profile().delivery.policy,
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: host.revision(),
            control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch,
        },
    }
}

fn bytes(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join(storage::CANONICAL)).unwrap()
}

fn prepared_at(
    root: &Directory,
    prompt: &str,
    steps: usize,
) -> (FileOversight, FileLearnedConfig, FileLearnedStreamNarrowRequest) {
    let generation = generation(prompt);
    let (mut host, _) = FileOversight::create_guarded_with_learned_generation_reserved(
        root.store(), delivery_profile(), &guards(), None,
        generation.clone(), RecoveryReserve::terminal(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..steps {
        step(&mut host);
    }
    let n = host.learned_generation_inspection().unwrap().numerical;
    let saved = host.capture_learned_stream_checkpoint(
        host.revision(), 1, n.actor_revision, host.inspect().control.ledger.epoch,
    )
    .unwrap();
    let control = host.inspect().control;
    let target = host.inspect().target;
    let request = FileLearnedStreamNarrowRequest::new(
        host.revision(), *saved.info(),
        FileLearnedStreamNarrowControl {
            operation: 800,
            expected_control_sequence: control.sequence,
            expected_actor_revision: n.actor_revision,
            expected_authority_epoch: control.ledger.epoch,
            binding: ReviewBinding {
                round: 800, reducer_generation: 1, evidence_root: [8; 32],
            },
            retained_targets: vec![
                target,
                ResolvedTarget { expected_version: target.expected_version + 1, ..target },
            ],
        },
    )
    .unwrap();
    (host, generation, request)
}

fn prepared(root: &Directory) -> (FileOversight, FileLearnedConfig, FileLearnedStreamNarrowRequest) {
    prepared_at(root, "p", 1)
}

fn begin(
    root: &Directory,
    generation: &FileLearnedConfig,
    required: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
    profile: FileOversightProfile,
) -> FileGuardedLearnedRecovery {
    FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile, required, generation, anchor,
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap()
}

fn ready(mut run: FileGuardedLearnedRecovery) -> FileGuardedLearnedRecovery {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress().replayed_events;
        assert_eq!(run.advance(before, 1).unwrap().replayed_events, before + 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    run
}

fn recovery(
    root: &Directory,
    generation: &FileLearnedConfig,
    required: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> FileGuardedLearnedRecovery {
    ready(begin(root, generation, required, anchor, delivery_profile()))
}

#[test]
fn ordinary_live_and_offline_narrow_preserve_work_and_retry_one_original_event() {
    for live in [false, true] {
        let root = Directory::new();
        let (mut host, generation, request) = prepared(&root);
        let required = expected(&host);
        let anchor = host.history_anchor().unwrap();
        let before = host.inspect();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let usage = host.learned_recovery_usage().unwrap();
        let audience = host.stream_snapshot().unwrap();
        assert!(!host.action_consistency_required());
        let live_receipt = if live {
            let receipt = host.narrow_learned_stream_checkpoint(&request).unwrap();
            let canonical = bytes(&root);
            assert_eq!(host.narrow_learned_stream_checkpoint(&request).unwrap(), receipt);
            assert_eq!(bytes(&root), canonical);
            let current = host.stream_snapshot().unwrap();
            assert_eq!(current.confirmed_target, audience.confirmed_target);
            assert_eq!(current.confirmed, audience.confirmed);
            assert_eq!(current.published, audience.published);
            Some(receipt)
        } else {
            None
        };
        drop(host);
        let result = recovery(&root, &generation, &required, &anchor)
            .finish_narrowed_stream_checkpoint(&request).unwrap();
        assert_eq!(result.appended_events(), usize::from(!live));
        assert_eq!(result.snapshot().revision, before.revision + 1);
        assert_eq!(result.snapshot().control.sequence, before.control.sequence + 1);
        assert_eq!(result.snapshot().control.ledger.epoch, before.control.ledger.epoch + 1);
        assert_eq!(result.snapshot().control.ledger.charged, before.control.ledger.charged);
        assert_eq!(result.snapshot().executions, before.executions);
        assert_eq!(result.numerical(), &numerical);
        assert_eq!(result.recovery_usage(), usage);
        let FileLearnedStreamControlReceipt::Narrowed { request: original, receipt } = result.receipt()
        else { panic!("original narrow receipt"); };
        assert_eq!(original, &request);
        assert_eq!(receipt.incident_count, 0);
        assert_eq!(receipt.refunded_units, 0);
        assert!(receipt.cancelled.is_empty());
        assert!(receipt.ceiling.contains(before.target));
        assert!(!receipt.ceiling.contains(ResolvedTarget { object: before.target.object + 1, ..before.target }));
        if let Some(live_receipt) = live_receipt {
            assert_eq!(receipt, &live_receipt);
        }
        let canonical = bytes(&root);
        let again = recovery(&root, &generation, &required, &anchor)
            .finish_narrowed_stream_checkpoint(&request).unwrap();
        assert_eq!(again.appended_events(), 0);
        assert_eq!(again.receipt(), result.receipt());
        assert_eq!(again.snapshot(), result.snapshot());
        assert_eq!(again.numerical(), result.numerical());
        assert_eq!(again.recovery_usage(), result.recovery_usage());
        assert_eq!(bytes(&root), canonical);
    }
}

#[test]
fn ordinary_prompt_narrowing_supplies_the_real_reset_frontier_and_resumes_generation() {
    let root = Directory::new();
    let (host, generation, request) = prepared(&root);
    let required = expected(&host);
    let anchor = host.history_anchor().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let initial_work = numerical.cumulative_work;
    drop(host);
    let narrowed = recovery(&root, &generation, &required, &anchor)
        .finish_narrowed_stream_checkpoint(&request).unwrap();
    let FileLearnedStreamControlReceipt::Narrowed { receipt, .. } = narrowed.receipt()
    else { panic!("native consequence"); };
    let first = receipt.sequence;
    assert!(first > request.checkpoint().control_sequence);
    let state = narrowed.snapshot();
    let intent = FileLearnedResetIntent::for_recovery(
        request.checkpoint().checkpoint,
        FileResetRequest {
            operation: 900,
            expected_control_sequence: state.control.sequence,
            expected_actor_revision: narrowed.numerical().actor_revision,
            expected_authority_epoch: state.control.ledger.epoch,
            binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [9; 32] },
            retained_targets: request.control().retained_targets.clone(),
        },
        KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit: recipe().2.policy.allowance() },
    )
    .unwrap();
    let pending = recovery(&root, &generation, &required, narrowed.history_anchor())
        .finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(pending.appended_events(), 1);
    assert_eq!(pending.numerical(), narrowed.numerical());
    let (mut host, _) = recovery(&root, &generation, &required, pending.history_anchor())
        .finish_pending_stream_reset(&intent, first).unwrap();
    let restored = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((restored.position, restored.sampled_draws), (numerical.position, 0));
    assert_eq!(restored.cumulative_work, initial_work);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert!(host.inspect().control.ledger.epoch > state.control.ledger.epoch);
    assert_eq!(host.inspect().executions, 0);
    assert!(!host.action_consistency_required());
    assert!(!host.clock_ready());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), restored.actor_revision, restored.position).unwrap();
    step(&mut host);
    step(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    assert_eq!(host.inspect().executions, 0, "generated text still lacks both publication keys");
}

#[test]
fn changed_checkpoint_predecessor_or_audience_refuses_before_any_canonical_change() {
    for field in 0..13 {
        let root = Directory::new();
        let (mut host, generation, request) = prepared(&root);
        let required = expected(&host);
        let anchor = host.history_anchor().unwrap();
        let before = host.inspect();
        let canonical = bytes(&root);
        let mut revision = request.revision();
        let mut checkpoint = request.checkpoint();
        let mut control = request.control().clone();
        match field {
            0 => revision += 1,
            1 => checkpoint.actor_revision += 1,
            2 => checkpoint.authority_epoch += 1,
            3 => checkpoint.control_sequence += 1,
            4 => checkpoint.stream += 1,
            5 => checkpoint.position += 1,
            6 => checkpoint.sampled_draws += 1,
            7 => checkpoint.checkpoint += 1,
            8 => control.expected_control_sequence += 1,
            9 => control.expected_actor_revision += 1,
            10 => control.expected_authority_epoch += 1,
            11 => control.retained_targets[0].object += 1,
            _ => control.retained_targets[0].generation += 1,
        }
        let changed = FileLearnedStreamNarrowRequest::new(revision, checkpoint, control).unwrap();
        assert!(host.narrow_learned_stream_checkpoint(&changed).is_err(), "field {field}");
        assert_eq!(host.inspect(), before);
        assert_eq!(bytes(&root), canonical);
        drop(host);
        assert!(recovery(&root, &generation, &required, &anchor)
            .finish_narrowed_stream_checkpoint(&changed).is_err(), "field {field}");
        assert_eq!(bytes(&root), canonical);
        let permitted = recovery(&root, &generation, &required, &anchor)
            .finish_narrowed_stream_checkpoint(&request).unwrap();
        assert_eq!(permitted.appended_events(), 1);
        let canonical = bytes(&root);
        assert!(recovery(&root, &generation, &required, &anchor)
            .finish_narrowed_stream_checkpoint(&changed).is_err());
        assert_eq!(bytes(&root), canonical);
    }
}

#[test]
fn ordinary_narrowing_requires_the_complete_unsampled_prompt_and_no_pending_step() {
    for (prompt, steps, pending, permitted) in [
        ("pq", 1, false, false),
        ("p", 2, false, false),
        ("p", 1, true, false),
        ("pq", 2, false, true),
    ] {
        let root = Directory::new();
        let (mut host, generation, request) = prepared_at(&root, prompt, steps);
        if pending {
            let n = host.learned_generation_inspection().unwrap().numerical;
            host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
        }
        let request = FileLearnedStreamNarrowRequest::new(
            host.revision(), request.checkpoint(), request.control().clone(),
        ).unwrap();
        let required = expected(&host);
        let anchor = host.history_anchor().unwrap();
        let canonical = bytes(&root);
        let before = host.inspect();
        if !permitted {
            assert!(host.narrow_learned_stream_checkpoint(&request).is_err());
            assert_eq!(host.inspect(), before);
            assert_eq!(bytes(&root), canonical);
        }
        drop(host);
        let result = recovery(&root, &generation, &required, &anchor)
            .finish_narrowed_stream_checkpoint(&request);
        assert_eq!(result.is_ok(), permitted, "{prompt:?} {steps} {pending}: {result:?}");
        if !permitted {
            assert_eq!(bytes(&root), canonical);
        } else {
            assert_eq!(result.unwrap().appended_events(), 1);
        }
    }
}

fn predictor() -> FileConsistencyConfig {
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let (model, _, _) = recipe();
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91,
        probe_generation: 1,
        profile: model.cache_profile().layers()[&1].values().profile(),
        weights: vec![1.0, 0.0],
        bias: 0.0,
        threshold: 0.0,
        forecast: ForecastRegistration {
            domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral,
        },
        alpha: ErrorBudget::new(1, 2).unwrap(),
        stream: 21,
        max_predictions: 8,
        max_prediction_age_ticks: 50,
    })
    .unwrap()
    .with_stream_messages(stream())
    .unwrap()
}

#[test]
fn installed_raw_optional_and_legacy_owned_predictors_cannot_borrow_ordinary_admission() {
    for selection in 0..4 {
        let root = Directory::new();
        let (mut host, generation, request) = prepared(&root);
        if selection < 2 {
            let raw = if selection == 0 { predictor() } else {
                predictor().with_pre_output_forecast().unwrap()
            };
            host.enable_action_consistency(host.revision(), raw).unwrap();
        } else {
            let mut owned = FileLearnedConsistencyConfig::new(
                predictor(), 1, KvSide::Value, LearnedMonitorBudget::default(),
                LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES,
            ).unwrap().with_owned_generation().unwrap();
            if selection == 3 {
                owned = owned.with_pre_output_forecast().unwrap();
            }
            host.enable_learned_action_consistency(host.revision(), owned).unwrap();
        }
        assert!(host.action_consistency_required());
        let request = FileLearnedStreamNarrowRequest::new(
            host.revision(), request.checkpoint(), request.control().clone(),
        ).unwrap();
        let required = expected(&host);
        let anchor = host.history_anchor().unwrap();
        let canonical = bytes(&root);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(host.narrow_learned_stream_checkpoint(&request).err(), Some(Error::Binding.into()));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(bytes(&root), canonical);
        drop(host);
        assert!(FileOversight::begin_open_guarded_anchored_with_learned_generation(
            root.store(), delivery_profile(), &required, &generation, &anchor,
        ).is_err(), "installed predictor {selection} cannot be omitted");
        assert_eq!(bytes(&root), canonical);
    }
    let root = Directory::new();
    let (mut host, _, request) = prepared(&root);
    assert!(!host.action_consistency_required());
    assert!(host.narrow_learned_stream_checkpoint(&request).is_ok());
}

#[test]
fn all_storage_acknowledgment_failures_preserve_the_exact_original_narrow_retry() {
    for live in [false, true] {
        for barrier in [
            JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync,
        ] {
            let root = Directory::new();
            let (mut host, generation, request) = prepared(&root);
            let required = expected(&host);
            let anchor = host.history_anchor().unwrap();
            let before = host.inspect();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let usage = host.learned_recovery_usage().unwrap();
            let canonical = bytes(&root);
            if live {
                host.store.fail_once(barrier);
                assert!(matches!(host.narrow_learned_stream_checkpoint(&request),
                    Err(JournalError::Io(failure)) if failure.operation == barrier));
                assert_eq!(host.narrow_learned_stream_checkpoint(&request).err(), Some(JournalError::Unavailable));
                drop(host);
            } else {
                drop(host);
                let run = recovery(&root, &generation, &required, &anchor);
                run.fail_once(barrier);
                assert!(matches!(run.finish_narrowed_stream_checkpoint(&request),
                    Err(JournalError::Io(failure)) if failure.operation == barrier));
            }
            if barrier != JournalIo::DirectorySync {
                assert_eq!(bytes(&root), canonical);
            }
            let result = recovery(&root, &generation, &required, &anchor)
                .finish_narrowed_stream_checkpoint(&request).unwrap();
            assert_eq!(result.appended_events(), usize::from(barrier != JournalIo::DirectorySync));
            assert_eq!(result.snapshot().revision, before.revision + 1);
            assert_eq!(result.numerical(), &numerical);
            assert_eq!(result.recovery_usage(), usage);
            let canonical = bytes(&root);
            let again = recovery(&root, &generation, &required, &anchor)
                .finish_narrowed_stream_checkpoint(&request).unwrap();
            assert_eq!(again.appended_events(), 0);
            assert_eq!(again.receipt(), result.receipt());
            assert_eq!(bytes(&root), canonical);
        }
    }
}

#[test]
fn incomplete_replay_stale_independent_floor_and_terminal_only_capacity_cannot_narrow() {
    let root = Directory::new();
    let (host, generation, request) = prepared(&root);
    let required = expected(&host);
    let anchor = host.history_anchor().unwrap();
    let canonical = bytes(&root);
    drop(host);
    assert!(begin(&root, &generation, &required, &anchor, delivery_profile())
        .finish_narrowed_stream_checkpoint(&request).is_err());
    assert_eq!(bytes(&root), canonical);
    let mut wrong = required.clone();
    wrong.minimum.authority_epoch += 1;
    let mut run = begin(&root, &generation, &wrong, &anchor, delivery_profile());
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        if run.advance(run.progress().replayed_events, 1).is_err() {
            break;
        }
    }
    assert!(run.finish_narrowed_stream_checkpoint(&request).is_err());
    assert_eq!(bytes(&root), canonical);
    let good = recovery(&root, &generation, &required, &anchor)
        .finish_narrowed_stream_checkpoint(&request).unwrap();
    assert_eq!(good.appended_events(), 1);

    for ordinary in [0, 1] {
        let root = Directory::new();
        let (host, generation, request) = prepared(&root);
        let required = expected(&host);
        let events = host.events.clone();
        let mut profile = delivery_profile();
        profile.delivery.limits.events = events.len() + ordinary + RecoveryReserve::terminal().events;
        let canonical = journal::encode(&profile, host.store.identity(), &events).unwrap();
        let anchor = FileHistoryAnchor::from_acknowledged(events.len(), canonical.clone());
        drop(host);
        std::fs::write(root.store().join(storage::CANONICAL), &canonical).unwrap();
        let result = ready(begin(&root, &generation, &required, &anchor, profile))
            .finish_narrowed_stream_checkpoint(&request);
        if ordinary == 0 {
            assert_eq!(result.err(), Some(Error::Limit.into()));
            assert_eq!(bytes(&root), canonical);
        } else {
            assert_eq!(result.unwrap().appended_events(), 1);
        }
    }
}
