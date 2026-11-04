//! The real guarded replay returns data at full capacity, never another owner.
//! Numerical/native fixtures test enforcement and parity, not model quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedRecoveryStatus,
    guarded::{
        FileGuardSet, FileRecoveryRequirements,
        predictive::{FilePredictiveRequirements, owned::FileOwnedPredictiveRequirements},
    },
};
use fa_reference::action::consequence::oversight::actor::UnknownReason;
use std::{collections::BTreeMap, path::Path};

fn files(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), fs::read(path).unwrap())
        })
        .collect()
}

// Each branch invokes its original public guarded opener and consumed finalizer.
// The callback is test-only: exercise a changed on-disk cut after complete replay.
fn read(
    config: &Config,
    loaded: &Loaded,
    request: u64,
    after: Option<u64>,
    replay: bool,
    before_finish: impl FnOnce(),
) -> Result<Knowledge<ActorOutcome>, JournalError> {
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet {
            stream: Some(loaded.stream),
            decoder: None,
            decoder_stop: None,
            source: Some(config.source_policy),
            identity: None,
            campaigns: None,
            credential: None,
        },
        effective_policy: config.profile.delivery.policy.clone(),
        credential_epoch: None,
        minimum: loaded.floor,
    };
    let anchor = loaded.recovery_anchor.as_ref().unwrap();
    macro_rules! finish {
        ($opened:expr) => {{
            let mut run = $opened?.require_recovery_reserve(RecoveryReserve::terminal())?;
            if replay {
                while run.progress().status == FileLearnedRecoveryStatus::Replaying {
                    run.advance(run.progress().replayed_events, 3)?;
                }
                assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
            }
            before_finish();
            run.finish_receipt(request, after)
        }};
    }
    if let Some(prediction) = loaded.generation.required_pre_output_forecast() {
        let expected = FilePredictiveRequirements {
            oversight: expected,
            prediction: prediction.clone(),
            evaluation: None,
        };
        finish!(
            FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
                &config.store,
                config.profile.clone(),
                &expected,
                &loaded.generation,
                anchor
            )
        )
    } else if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
        let expected = FileOwnedPredictiveRequirements {
            oversight: expected,
            prediction: prediction.clone(),
            evaluation: None,
        };
        finish!(
            FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
                &config.store,
                config.profile.clone(),
                &expected,
                &loaded.generation,
                anchor
            )
        )
    } else {
        finish!(
            FileOversight::begin_open_guarded_anchored_with_learned_generation(
                &config.store,
                config.profile.clone(),
                &expected,
                &loaded.generation,
                anchor
            )
        )
    }
}

#[test]
fn readonly_receipts_preserve_denied_and_native_confirmed_results_across_repeated_reads() {
    for mode in [None, Some(false), Some(true)] {
        for receipt_case in ReceiptCase::ALL {
            let allowed = receipt_case.permitted();
            let root = Root::new();
            let (mut config, path, mut loaded, reviewer, mut driver, mut wire, deadline) =
                generated(&root, mode);
            receipt_case.observe(&root, &config);
            receipt_case.submit_release(
                &mut driver,
                &mut wire,
                &mut config,
                &loaded,
                &deadline,
                LearnedTextRelease::Message,
            );
            if allowed {
                let (returned, history) =
                    native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
                driver = returned;
                let human = native::approve(&mut driver, &mut config, &reviewer, 1);
                native::publish(&mut driver, &mut config, &human);
                drop(history);
            }
            let expected = response(&mut wire, 1).result.unwrap();
            assert!(matches!(&expected, Knowledge::Known { value, .. }
                if *value == receipt_case.outcome()));
            loaded.recovery_anchor = Some(
                driver
                    .supervisor()
                    .host()
                    .unwrap()
                    .history_anchor()
                    .unwrap(),
            );
            assert_eq!(cleanup(driver, 1000, 1), 0);
            remove_live_assets(&root);
            // Load without native helpers, retaining the independent anchor.
            let mut loaded_again = recipe::load(&path, &config, false).unwrap();
            loaded_again.recovery_anchor = loaded.recovery_anchor;
            fs::write(
                config.store.join("delivery.pending"),
                b"never authoritative",
            )
            .unwrap();
            let original = files(&config.store);
            let reads = config.source.status().read_attempts;
            for _ in 0..3 {
                assert_eq!(
                    read(&config, &loaded_again, 1, None, true, || {}).unwrap(),
                    expected
                );
                assert_eq!(files(&config.store), original);
                assert_eq!(config.source.status().read_attempts, reads);
            }
        }
    }
}

#[test]
fn readonly_receipts_work_after_both_ordinary_and_terminal_event_capacity_are_exhausted() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let mut bounded = configured(&root);
        bounded.profile.delivery.limits.events = 256;
        let (mut config, _, mut loaded, _, mut driver, mut wire, deadline) =
            generated_with_config(&root, mode, bounded);
        evidence(&root, &config, 2, false, false);
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            loaded.recovery_anchor = Some(host.history_anchor().unwrap());
            while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(1000)).unwrap();
            }
            while host.revision() < config.profile.delivery.limits.events as u64 {
                let revision = host.revision();
                host.fence(revision).unwrap();
            }
            assert_eq!(host.revision(), 256);
            assert_eq!(
                host.journal_capacity().unwrap().ordinary_remaining().events,
                0
            );
        }
        let expected = response(&mut wire, 1).result.unwrap();
        assert_eq!(cleanup(driver, 1000, 1), 0);
        fs::write(
            config.store.join("delivery.pending"),
            b"full journal staging",
        )
        .unwrap();
        let original = files(&config.store);
        assert!(
            recovery::open_selected(&config, &loaded).is_err(),
            "a writable owner needs a fence slot"
        );
        assert_eq!(files(&config.store), original);
        for _ in 0..3 {
            assert_eq!(
                read(&config, &loaded, 1, None, true, || {}).unwrap(),
                expected
            );
            assert_eq!(files(&config.store), original);
        }
    }
}

#[test]
fn readonly_receipts_never_reconcile_published_output_or_cancel_pending_work() {
    // Original public wire outcomes provide the oracle, not a second projection.
    for phase in 0..4 {
        let root = Root::new();
        let (mut config, _, mut loaded, reviewer, mut driver, mut wire, deadline) =
            generated(&root, None);
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        if phase == 1 {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            host.cancel_request(revision, 1).unwrap();
        } else if phase >= 2 {
            let (returned, history) =
                native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
            driver = returned;
            let human = native::approve(&mut driver, &mut config, &reviewer, 1);
            assert!(matches!(
                driver
                    .step_computed_from_policy_file(
                        &mut config.source,
                        || ElapsedTick(1000),
                        Some(&human)
                    )
                    .result
                    .unwrap(),
                FileDriverEvent::Dispatched { .. }
            ));
            if phase == 3 {
                assert!(matches!(
                    driver
                        .step_computed_from_policy_file(
                            &mut config.source,
                            || ElapsedTick(1000),
                            None
                        )
                        .result
                        .unwrap(),
                    FileDriverEvent::PublicationChecked { .. }
                ));
                assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
                assert!(
                    driver
                        .supervisor()
                        .host()
                        .unwrap()
                        .request_resolution(1)
                        .unwrap()
                        .is_none()
                );
            }
            drop(history);
        }
        let expected = response(&mut wire, 1).result.unwrap();
        match phase {
            0 => assert!(matches!(expected, Knowledge::Pending { .. })),
            1 => assert!(matches!(
                expected,
                Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    ..
                }
            )),
            _ => assert!(matches!(
                expected,
                Knowledge::Unknown {
                    reason: UnknownReason::OutcomeUnknown
                }
            )),
        }
        loaded.recovery_anchor = Some(
            driver
                .supervisor()
                .host()
                .unwrap()
                .history_anchor()
                .unwrap(),
        );
        assert_eq!(cleanup(driver, 1000, 1), 0);
        let original = files(&config.store);
        assert_eq!(
            read(&config, &loaded, 1, None, true, || {}).unwrap(),
            expected
        );
        assert_eq!(files(&config.store), original);
    }
}

#[test]
fn readonly_receipts_reject_incomplete_wrong_selection_floor_and_changed_canonical_cut() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (mut config, _, mut loaded, _, mut driver, mut wire, deadline) = generated(&root, mode);
        evidence(&root, &config, 2, false, false);
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &deadline,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        loaded.recovery_anchor = Some(
            driver
                .supervisor()
                .host()
                .unwrap()
                .history_anchor()
                .unwrap(),
        );
        assert!(matches!(
            read(&config, &loaded, 1, None, true, || {}),
            Err(JournalError::Busy)
        ));
        let expected = response(&mut wire, 1).result.unwrap();
        assert_eq!(cleanup(driver, 1000, 1), 0);
        fs::write(
            config.store.join("delivery.pending"),
            b"preserve invalid reads",
        )
        .unwrap();
        let original = files(&config.store);
        assert_eq!(
            read(&config, &loaded, 1, None, false, || {}).err(),
            Some(Error::Incomplete.into())
        );
        for (request, after) in [(0, None), (2, None), (1, Some(1)), (1, Some(9))] {
            assert!(read(&config, &loaded, request, after, true, || {}).is_err());
            assert_eq!(files(&config.store), original);
        }
        let floor = loaded.floor;
        loaded.floor.journal_revision = loaded.recovery_anchor.as_ref().unwrap().revision() + 1;
        assert!(read(&config, &loaded, 1, None, true, || {}).is_err());
        loaded.floor = floor;
        assert_eq!(files(&config.store), original);
        let canonical = config.store.join("delivery.bin");
        assert_eq!(
            read(&config, &loaded, 1, None, true, || {
                fs::write(&canonical, b"changed after ready").unwrap();
            })
            .err(),
            Some(Error::Binding.into())
        );
        assert_eq!(
            fs::read(config.store.join("delivery.pending")).unwrap(),
            b"preserve invalid reads"
        );
        fs::write(&canonical, &original[&canonical]).unwrap();
        assert_eq!(
            read(&config, &loaded, 1, None, true, || {}).unwrap(),
            expected
        );
        assert_eq!(files(&config.store), original);
    }
}
