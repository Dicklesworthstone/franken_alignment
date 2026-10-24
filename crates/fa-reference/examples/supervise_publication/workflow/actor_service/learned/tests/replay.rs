//! Original learned cursors and real journal files under cooperative callbacks.
//! These bounded fixture histories exercise replay cancellation, not wall-clock
//! latency, process isolation, forecast calibration or unbounded journal sizes.
use super::*;
use super::super::prediction;
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot,
    observed::{
        decoder::learned::{FileLearnedInspection, FileLearnedRecoveryProgress, FileLearnedRecoveryStatus},
        consistency::{FileConsistencySnapshot, learned::FileLearnedConsistencySnapshot},
        guarded::{
            FileGuardSet, FileOversightRoles, FileRecoveryRequirements,
            learned_recovery::FileGuardedLearnedRecovery,
            predictive::{FilePredictiveRequirements, FilePredictiveRoles,
                learned::FilePredictiveLearnedRecovery,
                owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements}},
        },
    },
};
use std::{collections::BTreeMap, path::Path};

// This test-only adapter drives each original cursor in ONE full-history batch.
// The production path below always enters recovery::open_selected_with_checkpoints.
trait BatchCursor: Sized {
    type Output;
    fn progress(&self) -> FileLearnedRecoveryProgress;
    fn replay_all(&mut self) -> Result<FileLearnedRecoveryProgress, JournalError>;
    fn finish(self) -> Result<Self::Output, JournalError>;
}
impl BatchCursor for FileGuardedLearnedRecovery {
    type Output = (FileOversight, FileOversightRoles);
    fn progress(&self) -> FileLearnedRecoveryProgress { Self::progress(self) }
    fn replay_all(&mut self) -> Result<FileLearnedRecoveryProgress, JournalError> {
        let progress = self.progress();
        self.advance(progress.replayed_events, progress.total_events)
    }
    fn finish(self) -> Result<Self::Output, JournalError> { Self::finish(self) }
}
impl BatchCursor for FilePredictiveLearnedRecovery {
    type Output = (FileOversight, FilePredictiveRoles);
    fn progress(&self) -> FileLearnedRecoveryProgress { Self::progress(self) }
    fn replay_all(&mut self) -> Result<FileLearnedRecoveryProgress, JournalError> {
        let progress = self.progress();
        self.advance(progress.replayed_events, progress.total_events)
    }
    fn finish(self) -> Result<Self::Output, JournalError> { Self::finish(self) }
}
impl BatchCursor for FileOwnedPredictiveRecovery {
    type Output = (FileOversight, FilePredictiveRoles);
    fn progress(&self) -> FileLearnedRecoveryProgress { Self::progress(self) }
    fn replay_all(&mut self) -> Result<FileLearnedRecoveryProgress, JournalError> {
        let progress = self.progress();
        self.advance(progress.replayed_events, progress.total_events)
    }
    fn finish(self) -> Result<Self::Output, JournalError> { Self::finish(self) }
}

type Observed = (FileDeliverySnapshot, FileLearnedInspection,
    Option<FileConsistencySnapshot>, Option<FileLearnedConsistencySnapshot>);

fn observed(host: &FileOversight) -> Observed {
    let numerical = host.learned_generation_inspection().unwrap();
    assert_eq!(numerical.numerical.position, 4);
    assert_eq!(numerical.numerical.sampled_draws, 2);
    assert!(numerical.numerical.work.accepted_decoder.scalar_products().unwrap() > 0);
    assert!(numerical.paused);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
    let consistency = host.action_consistency_required()
        .then(|| host.action_consistency_snapshot().unwrap());
    let learned = host.owned_learned_action_consistency_required()
        .then(|| host.learned_action_consistency_snapshot().unwrap());
    if let Some(snapshot) = &consistency {
        // The generated but unsubmitted original message remains unanswered.
        // Recovery cannot recreate its lost coverage as a permitting forecast.
        assert!(snapshot.coverage_lost);
        assert_eq!(snapshot.evidence.samples(), 0);
        assert!(snapshot.pending_attempt.is_some());
    }
    if let Some(snapshot) = &learned {
        assert!(snapshot.work.probe_coordinates > 0);
        assert!(snapshot.retained_source_bytes > 0);
    }
    (host.inspect(), numerical, consistency, learned)
}

fn files(store: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(store).unwrap().map(|entry| {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        (entry.file_name().into_string().unwrap(), fs::read(entry.path()).unwrap())
    }).collect()
}

fn requirements(config: &Config, loaded: &Loaded, wrong_floor: bool) -> FileRecoveryRequirements {
    let mut expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(loaded.stream), decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(),
        credential_epoch: None, minimum: loaded.floor,
    };
    if wrong_floor { expected.minimum.journal_revision = u64::MAX; }
    expected
}

fn prepared(root: &Root, owned: Option<bool>) -> (Config, Loaded, Loaded) {
    let mut config = configured(root);
    let path = fixture::write(root, b"allow");
    if let Some(owned) = owned { super::forecast::install(root, &path, owned, 60_000); }
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer, _identity) = recovery::create_selected(&config, &loaded).unwrap();
    let mut prediction = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let wire = ActorWire::new(port);
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
    {
        let host = driver.supervisor().host().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4);
        assert_eq!(numerical.sampled_draws, 2);
        assert_eq!(host.inspect().executions, 0);
        if owned.is_some() { assert!(host.action_consistency_deadline().unwrap().is_some()); }
    }
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(wire);
    drop(prediction);
    fs::write(config.store.join("delivery.pending"), b"unacknowledged staging evidence").unwrap();
    let mut wrong = recipe::load(&path, &config, false).unwrap();
    wrong.floor.journal_revision = u64::MAX;
    (config, loaded, wrong)
}

/// Each original cursor provides the independent batch oracle. All cooperative
/// calls use the production selection, guard admission and checkpoint entry.
fn exercise<C: BatchCursor>(config: &Config, loaded: &Loaded, wrong: &Loaded,
    begin: impl Fn(bool) -> C, inspect: impl Fn(&C::Output) -> Observed) {
    let store = &config.store;
    let original = files(store);
    let first = begin(false);
    let start = first.progress();
    assert_eq!(start.replayed_events, 0);
    assert_eq!(start.status, FileLearnedRecoveryStatus::Replaying);
    assert!(start.total_events > 4);
    let total = start.total_events;
    // The pre-open checkpoint must precede even lock acquisition. This original
    // cursor holds the writer lock, but cancellation wins without trying it.
    let refusal = "cancelled before opening the original recovery".to_owned();
    let mut pre_open = 0;
    let result = recovery::open_selected_with_checkpoints(config, loaded, &mut |view| {
        assert_eq!(view, None);
        pre_open += 1;
        Err(refusal.clone())
    });
    assert_eq!(result.err(), Some(refusal));
    assert_eq!(pre_open, 1);
    assert_eq!(files(store), original);
    drop(first);

    // Fixed denominator: N original events plus the final pre-finish boundary,
    // and the separate pre-open refusal above, in each of the three modes.
    for stop_at in 0..=total {
        let refusal = format!("supervisor cancelled at original replay cut {stop_at}");
        let mut pre_open = 0;
        let mut seen = Vec::new();
        let result = recovery::open_selected_with_checkpoints(config, loaded, &mut |view| {
            assert_eq!(files(store), original);
            let Some(progress) = view else {
                pre_open += 1;
                assert_eq!(pre_open, 1);
                assert!(seen.is_empty());
                return Ok(());
            };
            assert_eq!(pre_open, 1);
            assert_eq!(progress.total_events, total);
            assert_eq!(progress.replayed_events, seen.len());
            assert_eq!(progress.status, if progress.replayed_events == total {
                FileLearnedRecoveryStatus::Ready
            } else { FileLearnedRecoveryStatus::Replaying });
            seen.push(progress.replayed_events);
            if progress.replayed_events == stop_at { Err(refusal.clone()) } else { Ok(()) }
        });
        assert_eq!(result.err(), Some(refusal));
        assert_eq!(pre_open, 1);
        assert_eq!(seen, (0..=stop_at).collect::<Vec<_>>());
        assert_eq!(files(store), original);
        // An error returned no owner/roles and dropped the original writer lock.
        let retry = begin(false);
        assert_eq!(retry.progress(), start);
        drop(retry);
    }

    // A real final guard mismatch remains its original error, including after
    // all numerical witnesses executed. It cannot become callback success,
    // Ready, staging cleanup, or a second attempt at the failed event.
    let mut refused = begin(true);
    let error = refused.replay_all().unwrap_err();
    assert_eq!(error, JournalError::Contract(Error::Stale));
    drop(refused);
    let mut pre_open = 0;
    let mut seen = Vec::new();
    assert_eq!(recovery::open_selected_with_checkpoints(config, wrong, &mut |view| {
        assert_eq!(files(store), original);
        if let Some(progress) = view {
            assert_eq!(progress.total_events, total);
            assert_eq!(progress.replayed_events, seen.len());
            assert_eq!(progress.status, FileLearnedRecoveryStatus::Replaying);
            seen.push(progress.replayed_events);
        } else { pre_open += 1; }
        Ok(())
    }).err(), Some(debug(error)));
    assert_eq!(pre_open, 1);
    assert_eq!(seen, (0..total).collect::<Vec<_>>());
    assert_eq!(files(store), original);

    // Independent schedule oracle: execute the ORIGINAL cursor in one
    // whole-history quantum, and preserve the resulting exact fence bytes.
    let mut batch = begin(false);
    assert_eq!(batch.replay_all().unwrap().status, FileLearnedRecoveryStatus::Ready);
    assert_eq!(files(store), original);
    let batch = batch.finish().unwrap();
    let expected = inspect(&batch);
    assert_eq!(expected.0.revision, total as u64 + 1);
    let expected_files = files(store);
    assert!(!expected_files.contains_key("delivery.pending"));
    assert_ne!(expected_files["delivery.bin"], original["delivery.bin"]);
    drop(batch);
    // Restore this isolated test fixture's original acknowledged image and
    // staging, then compare the new schedule at the very same store identity.
    fs::write(store.join("delivery.bin"), &original["delivery.bin"]).unwrap();
    fs::write(store.join("delivery.pending"), &original["delivery.pending"]).unwrap();
    assert_eq!(files(store), original);
    let mut pre_open = 0;
    let mut seen = Vec::new();
    let cooperative = recovery::open_selected_with_checkpoints(config, loaded, &mut |view| {
        assert_eq!(files(store), original);
        if let Some(progress) = view {
            assert_eq!(pre_open, 1);
            assert_eq!(progress.total_events, total);
            assert_eq!(progress.replayed_events, seen.len());
            assert_eq!(progress.status, if progress.replayed_events == total {
                FileLearnedRecoveryStatus::Ready
            } else { FileLearnedRecoveryStatus::Replaying });
            seen.push(progress.replayed_events);
        } else {
            pre_open += 1;
            assert_eq!(pre_open, 1);
            assert!(seen.is_empty());
        }
        Ok(())
    }).unwrap();
    assert_eq!(pre_open, 1);
    assert_eq!(seen, (0..=total).collect::<Vec<_>>());
    assert_eq!(cooperative.2.is_some(), cooperative.0.action_consistency_required());
    assert_eq!(observed(&cooperative.0), expected);
    assert_eq!(files(store), expected_files);
    drop(cooperative);
}

#[test]
fn all_original_recovery_modes_cancel_at_every_cut_and_match_the_permitted_batch_fence() {
    for owned in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, loaded, wrong) = prepared(&root, owned);
        match owned {
            None => exercise(&config, &loaded, &wrong, |wrong| {
                FileOversight::begin_open_guarded_with_learned_generation(
                    &config.store, config.profile.clone(), &requirements(&config, &loaded, wrong),
                    &loaded.generation).unwrap()
                    .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
            }, |(host, _roles)| observed(host)),
            Some(false) => exercise(&config, &loaded, &wrong, |wrong| {
                let expected = FilePredictiveRequirements {
                    oversight: requirements(&config, &loaded, wrong),
                    prediction: loaded.generation.required_pre_output_forecast().unwrap().clone(),
                    evaluation: None,
                };
                FileOversight::begin_open_predictive_guarded_with_learned_generation(
                    &config.store, config.profile.clone(), &expected, &loaded.generation).unwrap()
                    .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
            }, |(host, _roles)| observed(host)),
            Some(true) => exercise(&config, &loaded, &wrong, |wrong| {
                let expected = FileOwnedPredictiveRequirements {
                    oversight: requirements(&config, &loaded, wrong),
                    prediction: loaded.generation.required_owned_pre_output_forecast().unwrap().clone(),
                    evaluation: None,
                };
                FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
                    &config.store, config.profile.clone(), &expected, &loaded.generation).unwrap()
                    .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
            }, |(host, _roles)| observed(host)),
        }
    }
}
