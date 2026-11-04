//! Real native capture/initiation, independent guarded replay and storage faults.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{
    decoder::learned::{FileLearnedRecoveryStatus, FileLearnedStreamCheckpointRequest,
        FileLearnedStreamControlReceipt},
    guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        anchored::FileHistoryAnchor, learned_recovery::FileGuardedLearnedRecovery},
};

fn expected(host: &FileOversight) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(stream_profile()), decoder: None,
            decoder_stop: None, source: None, identity: None, campaigns: None, credential: None },
        effective_policy: host.current_policy().unwrap().clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch },
    }
}
fn selection(host: &FileOversight) -> FileLearnedStreamCheckpointRequest {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control;
    FileLearnedStreamCheckpointRequest::new(host.revision(), checkpoint::FileLearnedCheckpointInfo {
        checkpoint: 1, actor_revision: n.actor_revision, authority_epoch: control.ledger.epoch,
        control_sequence: control.sequence,
        stream: host.machine.broker.hosted_learned_observation().unwrap().stream(),
        position: n.position, sampled_draws: n.sampled_draws,
    }).unwrap()
}
fn ready(root: &Directory, config: &FileLearnedConfig,
    required: &FileRecoveryRequirements, anchor: &FileHistoryAnchor) -> FileGuardedLearnedRecovery
{
    ready_with_profile(root, config, required, anchor, stream_delivery_profile())
}
fn ready_with_profile(root: &Directory, config: &FileLearnedConfig,
    required: &FileRecoveryRequirements, anchor: &FileHistoryAnchor,
    profile: FileOversightProfile) -> FileGuardedLearnedRecovery
{
    let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), profile, required, config, anchor).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    run
}
fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

#[test]
fn exact_guarded_capture_preserves_numerical_authority_and_confirms_zero_event_retries() {
    let root = Directory::new(); let config = stream_config();
    let host = start(&root, &config);
    let request = selection(&host); let required = expected(&host);
    let anchor = host.history_anchor().unwrap(); let before = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    drop(host);
    let result = ready(&root, &config, &required, &anchor).finish_captured_stream_checkpoint(request).unwrap();
    assert_eq!(result.receipt(), &FileLearnedStreamControlReceipt::Captured(request.checkpoint()));
    assert_eq!(result.appended_events(), 1);
    assert_eq!(result.snapshot().revision, before.revision + 1);
    assert_eq!(result.snapshot().control, before.control);
    assert_eq!(result.snapshot().executions, 0);
    assert_eq!(result.numerical(), &numerical);
    assert_eq!(result.recovery_usage().checkpoints, 1);
    assert_eq!(result.recovery_usage().restart_attempts, 0);
    let after = canonical(&root);
    let repeated = ready(&root, &config, &required, &anchor).finish_captured_stream_checkpoint(request).unwrap();
    assert_eq!(repeated.appended_events(), 0);
    assert_eq!(repeated.receipt(), result.receipt());
    assert_eq!(repeated.history_anchor(), result.history_anchor());
    assert_eq!(repeated.recovery_usage(), result.recovery_usage());
    assert_eq!(canonical(&root), after);
    // The same coordinates at a different journal event cannot relabel capture.
    let changed = FileLearnedStreamCheckpointRequest::new(request.revision() + 1, request.checkpoint()).unwrap();
    assert_eq!(ready(&root, &config, &required, &anchor).finish_captured_stream_checkpoint(changed).err(),
        Some(Error::Binding.into()));
    assert_eq!(canonical(&root), after);
}

#[test]
fn independent_begin_remains_completable_without_a_fence_and_never_repeats_a_reset() {
    let root = Directory::new(); let config = stream_config();
    let (host, saved, first) = prepared(&root, &config);
    let intent = FileLearnedResetIntent::for_recovery(saved.id(), instruction(&host, 900), budget(&host)).unwrap();
    let required = expected(&host); let anchor = host.history_anchor().unwrap();
    let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap().numerical;
    drop(host);
    let begun = ready(&root, &config, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(begun.appended_events(), 1);
    assert_eq!(begun.snapshot().revision, before.revision + 1);
    assert_eq!(begun.snapshot().control, before.control);
    assert_eq!(begun.numerical(), &numerical);
    assert_eq!(begun.recovery_usage().restart_attempts, 0);
    let pending = canonical(&root);
    let repeated = ready(&root, &config, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(repeated.appended_events(), 0);
    assert_eq!(repeated.history_anchor(), begun.history_anchor());
    assert_eq!(canonical(&root), pending);
    let (host, _) = ready(&root, &config, &required, &anchor).finish_pending_stream_reset(&intent, first).unwrap();
    assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
    assert_eq!(host.revision(), before.revision + 3);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, saved.info().position);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.cumulative_work, numerical.cumulative_work);
    assert!(host.learned_generation_inspection().unwrap().paused);
    no_publication(&host);
    drop(host);
    let completed = canonical(&root);
    let old = ready(&root, &config, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    assert_eq!(old.appended_events(), 0);
    assert!(matches!(old.receipt(), FileLearnedStreamControlReceipt::ResetBegun { already_completed: true, .. }));
    assert_eq!(old.recovery_usage().restart_attempts, 1);
    assert_eq!(canonical(&root), completed);
}

#[test]
fn capture_requires_every_independent_coordinate_before_any_write() {
    for field in 0..8 {
        let root = Directory::new(); let config = stream_config(); let host = start(&root, &config);
        let request = selection(&host); let required = expected(&host); let anchor = host.history_anchor().unwrap();
        let before = bytes(&host); drop(host);
        let mut revision = request.revision(); let mut info = request.checkpoint();
        match field {
            0 => revision += 1, 1 => info.actor_revision += 1, 2 => info.authority_epoch += 1,
            3 => info.control_sequence += 1, 4 => info.stream += 1, 5 => info.position += 1,
            6 => info.sampled_draws += 1, _ => info.checkpoint = 0,
        }
        match FileLearnedStreamCheckpointRequest::new(revision, info) {
            Ok(changed) => assert!(ready(&root, &config, &required, &anchor).finish_captured_stream_checkpoint(changed).is_err()),
            Err(error) => assert_eq!(error, Error::InvalidInput),
        }
        assert_eq!(canonical(&root), before);
        assert_eq!(ready(&root, &config, &required, &anchor).finish_captured_stream_checkpoint(request)
            .unwrap().appended_events(), 1);
    }
}

#[test]
fn changed_and_interrupted_reset_instructions_never_rebase_or_repeat_initiation() {
    let root = Directory::new(); let config = stream_config();
    let (host, saved, first) = prepared(&root, &config);
    let intent = FileLearnedResetIntent::for_recovery(saved.id(), instruction(&host, 900), budget(&host)).unwrap();
    let required = expected(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    ready(&root, &config, &required, &anchor).finish_begun_stream_reset(&intent, first).unwrap();
    let before = canonical(&root);
    let mut changed = intent.control().clone(); changed.binding.evidence_root[0] ^= 1;
    let changed = FileLearnedResetIntent::for_recovery(saved.id(), changed, intent.budget()).unwrap();
    assert_eq!(ready(&root, &config, &required, &anchor).finish_begun_stream_reset(&changed, first).err(),
        Some(Error::Binding.into()));
    assert_eq!(ready(&root, &config, &required, &anchor).finish_begun_stream_reset(&intent, first + 1).err(),
        Some(Error::Binding.into()));
    assert_eq!(canonical(&root), before);
    let (host, _) = ready(&root, &config, &required, &anchor).finish().unwrap();
    assert!(host.pending_learned_reset().unwrap().unwrap().interrupted); drop(host);
    let fenced = canonical(&root);
    assert_eq!(ready(&root, &config, &required, &anchor).finish_begun_stream_reset(&intent, first).err(),
        Some(Error::WrongState.into()));
    assert_eq!(canonical(&root), fenced);
}

#[test]
fn all_original_storage_barriers_withhold_receipts_and_exact_retry_counts_one_event() {
    for capture_operation in [true, false] {
        for barrier in BARRIERS {
            let root = Directory::new(); let config = stream_config();
            let (host, intent, first) = if capture_operation {
                (start(&root, &config), None, 0)
            } else {
                let (host, saved, first) = prepared(&root, &config);
                let intent = FileLearnedResetIntent::for_recovery(saved.id(), instruction(&host, 900), budget(&host)).unwrap();
                (host, Some(intent), first)
            };
            let capture = selection(&host); let required = expected(&host); let anchor = host.history_anchor().unwrap();
            let before = bytes(&host); let revision = host.revision();
            let numerical = host.learned_generation_inspection().unwrap().numerical; drop(host);
            let run = ready(&root, &config, &required, &anchor); run.fail_once(barrier);
            let result = match &intent {
                Some(intent) => run.finish_begun_stream_reset(intent, first),
                None => run.finish_captured_stream_checkpoint(capture),
            };
            assert!(matches!(result, Err(JournalError::Io(failure)) if failure.operation == barrier));
            if barrier != JournalIo::DirectorySync { assert_eq!(canonical(&root), before); }
            let run = ready(&root, &config, &required, &anchor);
            let retained = match &intent {
                Some(intent) => run.finish_begun_stream_reset(intent, first),
                None => run.finish_captured_stream_checkpoint(capture),
            }.unwrap();
            assert_eq!(retained.appended_events(), usize::from(barrier != JournalIo::DirectorySync));
            assert_eq!(retained.snapshot().revision, revision + 1);
            assert_eq!(retained.numerical(), &numerical);
            assert_eq!(retained.recovery_usage().restart_attempts, 0);
            assert_eq!(retained.recovery_usage().checkpoints, 1);
        }
    }
}

#[test]
fn incomplete_or_changed_canonical_history_and_wrong_policy_never_capture() {
    let root = Directory::new(); let config = stream_config(); let host = start(&root, &config);
    let request = selection(&host); let required = expected(&host); let anchor = host.history_anchor().unwrap();
    let before = bytes(&host); drop(host);
    let run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), stream_delivery_profile(), &required, &config, &anchor).unwrap();
    assert_eq!(run.finish_captured_stream_checkpoint(request).err(), Some(Error::Incomplete.into()));
    assert_eq!(canonical(&root), before);
    let run = ready(&root, &config, &required, &anchor);
    let mut changed = before.clone(); changed[0] ^= 1;
    std::fs::write(root.store().join("delivery.bin"), &changed).unwrap();
    assert_eq!(run.finish_captured_stream_checkpoint(request).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&root), changed);
    std::fs::write(root.store().join("delivery.bin"), &before).unwrap();
    let mut wrong = required.clone();
    wrong.effective_policy = Policy::new(required.effective_policy.generation() + 1,
        vec![Predicate::ExactValue { key: 7, value: b"other".to_vec() }]).unwrap();
    let mut run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(), stream_delivery_profile(), &wrong, &config, &anchor).unwrap();
    let result = loop {
        match run.advance(run.progress().replayed_events, 1) {
            Ok(progress) if progress.status == FileLearnedRecoveryStatus::Ready => break Ok(()),
            Ok(_) => {}, Err(error) => break Err(error),
        }
    };
    assert!(result.is_err());
    assert!(run.finish_captured_stream_checkpoint(request).is_err());
    assert_eq!(canonical(&root), before);
}

#[test]
fn capture_and_begin_preserve_the_original_ordinary_capacity_and_terminal_reserve() {
    for capturing in [true, false] {
        for ordinary in [0, 1, 2] {
            let root = Directory::new(); let config = stream_config();
            let (mut host, _) = FileOversight::create_with_learned_text_stream_with_reserve(
                root.store(), stream_delivery_profile(), config.clone(), RecoveryReserve::terminal()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
            let request = selection(&host);
            let begun = if capturing { None } else {
                let saved = capture(&mut host); step(&mut host); let first = frontier(&mut host, 800);
                Some((FileLearnedResetIntent::for_recovery(saved.id(), instruction(&host, 900), budget(&host)).unwrap(), first))
            };
            let required = expected(&host); let history = events(&host, &config);
            let mut profile = stream_delivery_profile();
            profile.delivery.limits.events = history.len() + ordinary + RecoveryReserve::terminal().events;
            // As in the existing native capacity tests, encode the COMPLETE
            // bootstrap under this independently selected exact logical bound.
            let original = journal::encode(&profile, host.store.identity(), &history).unwrap();
            let anchor = FileHistoryAnchor::from_acknowledged(history.len(), original.clone());
            drop(host); std::fs::write(root.store().join("delivery.bin"), &original).unwrap();
            let run = ready_with_profile(&root, &config, &required, &anchor, profile);
            let result = match begun {
                Some((intent, first)) => run.finish_begun_stream_reset(&intent, first),
                None => run.finish_captured_stream_checkpoint(request),
            };
            let needed = if capturing { 1 } else { 2 };
            if ordinary < needed {
                assert_eq!(result.err(), Some(Error::Limit.into()));
                assert_eq!(canonical(&root), original);
            } else {
                assert_eq!(result.unwrap().appended_events(), 1);
            }
        }
    }
}
