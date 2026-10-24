//! Pre-fence selection on the actual locked replay; never a second owner.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedRecoveryStatus, guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
};
use std::collections::BTreeMap;
use std::path::Path;

fn files(store: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(store)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn anchor(root: &Root, loaded: &mut Loaded, driver: &FileSupervisedDriver) -> Vec<u8> {
    let host = driver.supervisor().host().unwrap();
    let original = host.history_anchor().unwrap();
    let mut bytes = Vec::new();
    original
        .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
        .unwrap();
    fs::write(root.0.join("external.anchor"), &bytes).unwrap();
    loaded.recovery_anchor = Some(original);
    bytes
}

#[test]
fn invalid_message_receipt_retries_leave_staging_anchor_and_recovery_capacity_untouched() {
    for (mode, receipt_case) in [None, Some(false), Some(true)]
        .into_iter()
        .flat_map(|mode| ReceiptCase::REFUSALS.map(|receipt_case| (mode, receipt_case)))
    {
        let root = Root::new();
        let (mut config, path, mut loaded, _, mut driver, mut wire, deadline) =
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
        receipt_case.assert_admission(&driver.supervisor().host().unwrap(), 1);
        let expected = response(&mut wire, 1).result.unwrap();
        let saved_anchor = anchor(&root, &mut loaded, &driver);
        assert_eq!(cleanup(driver, 1000, 1), 0);
        remove_live_assets(&root);
        fs::write(
            config.store.join("delivery.pending"),
            b"uncommitted diagnostic evidence",
        )
        .unwrap();
        let before = files(&config.store);
        for (request, after) in [(0, None), (99, None), (1, Some(9)), (99, Some(1))] {
            let current = configured(&root);
            let mut selected = recipe::load(&path, &current, false).unwrap();
            selected.request = request;
            selected.recovery_anchor =
                Some(super::super::super::anchors::load(&root.0.join("external.anchor")).unwrap());
            let no_clock =
                || panic!("invalid receipt selection must not call the settlement clock");
            let result = match after {
                None => recovery::resume(current, selected, no_clock),
                Some(after) => recovery::resume_finish(current, selected, after, no_clock),
            };
            assert!(
                result.is_err(),
                "mode {mode:?}, request {request}, after {after:?}"
            );
            assert_eq!(files(&config.store), before);
            assert_eq!(
                fs::read(root.0.join("external.anchor")).unwrap(),
                saved_anchor
            );
        }
        // Same exact independently anchored history, corrected selection. The
        // original denied ticket is now recoverable; no successful effect exists.
        let result =
            recovery::resume(config, loaded, || panic!("terminal denial needs no clock")).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        let Knowledge::Known { value: old, .. } = expected else {
            panic!("expected terminal denial");
        };
        match result.response.result {
            Ok(Knowledge::Known { value, .. }) => assert_eq!(value, old),
            other => panic!("lost original denial: {other:?}"),
        }
        assert!(!root.0.join("store/delivery.pending").exists());
        assert_eq!(
            fs::read(root.0.join("external.anchor")).unwrap(),
            saved_anchor
        );
    }
}

#[test]
fn selected_receipt_keeps_ready_deadline_and_full_guard_floor_before_one_fence() {
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
        let (revision, numerical, original) = {
            let host = driver.supervisor().host().unwrap();
            (
                host.revision(),
                host.learned_generation_inspection().unwrap().numerical,
                host.learned_text_stream_message_request(1).unwrap(),
            )
        };
        anchor(&root, &mut loaded, &driver);
        assert_eq!(cleanup(driver, 1000, 1), 0);
        remove_live_assets(&root);
        fs::write(config.store.join("delivery.pending"), b"still pending").unwrap();
        let before = files(&config.store);
        let limit = Duration::from_millis(config.timing.runtime_ms);
        // Pre-open and Ready are the two irreversible-admission edges; existing
        // shared-timer tests additionally cover every intermediate replay event.
        for expire_at in [0, revision as usize + 1] {
            let mut calls = 0;
            let result =
                recovery::open_selected_receipt_with_elapsed(&config, &loaded, None, &mut || {
                    let cut = calls;
                    calls += 1;
                    if cut == expire_at {
                        limit
                    } else {
                        Duration::ZERO
                    }
                });
            assert_eq!(
                result.err().as_deref(),
                Some("learned receipt recovery replay deadline reached")
            );
            assert_eq!(calls, expire_at + 1);
            assert_eq!(files(&config.store), before);
        }
        let floor = loaded.floor;
        loaded.floor.journal_revision = revision + 1;
        assert!(
            recovery::open_selected_receipt_with_elapsed(&config, &loaded, None, &mut || {
                Duration::ZERO
            })
            .is_err()
        );
        assert_eq!(files(&config.store), before);
        loaded.floor = floor;
        let mut calls = 0;
        let ((host, _, observer, _identity), selected) =
            recovery::open_selected_receipt_with_elapsed(&config, &loaded, None, &mut || {
                calls += 1;
                Duration::ZERO
            })
            .unwrap();
        assert_eq!(calls, revision as usize + 2);
        assert_eq!(host.revision(), revision + 1);
        assert_eq!(selected, original);
        assert_eq!(
            host.learned_text_stream_message_request(1).unwrap(),
            original
        );
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(observer.is_some(), mode.is_some());
        assert_eq!(host.inspect().executions, 0);
        assert!(!config.store.join("delivery.pending").exists());
    }
}

#[test]
fn base_receipt_descriptor_requires_full_replay_and_cannot_bypass_canonical_recheck() {
    for changed in [false, true] {
        let root = Root::new();
        let (mut config, _, loaded, _, mut driver, mut wire, deadline) = generated(&root, None);
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
        let original = driver
            .supervisor()
            .host()
            .unwrap()
            .learned_text_stream_message_request(1)
            .unwrap();
        assert_eq!(cleanup(driver, 1000, 1), 0);
        fs::write(config.store.join("delivery.pending"), b"retained staging").unwrap();
        let before = files(&config.store);
        let mut run = FileOversight::begin_open_with_learned_generation(
            &config.store,
            config.profile.clone(),
            &loaded.generation,
        )
        .unwrap();
        let total = run.progress().total_events;
        while run.progress().replayed_events < total {
            assert_eq!(
                run.learned_text_stream_message_request(1).err(),
                Some(JournalError::Contract(Error::Incomplete))
            );
            assert_eq!(
                run.learned_text_stream_finish_request(1, 2).err(),
                Some(JournalError::Contract(Error::Incomplete))
            );
            run.advance(run.progress().replayed_events, 1).unwrap();
        }
        assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
        assert_eq!(
            run.learned_text_stream_message_request(1).unwrap(),
            original
        );
        assert_eq!(files(&config.store), before);
        if changed {
            let mut canonical = before["delivery.bin"].clone();
            canonical.push(0);
            fs::write(config.store.join("delivery.bin"), canonical).unwrap();
            let changed_files = files(&config.store);
            assert_eq!(
                run.finish().err(),
                Some(JournalError::Contract(Error::Binding))
            );
            assert_eq!(files(&config.store), changed_files);
        } else {
            let (host, _) = run.finish().unwrap();
            assert_eq!(host.revision(), total as u64 + 1);
            assert_eq!(
                host.learned_text_stream_message_request(1).unwrap(),
                original
            );
            assert!(!config.store.join("delivery.pending").exists());
        }
    }
}

#[test]
fn finish_receipt_selection_preserves_predecessor_and_refused_or_executed_outcome() {
    for mode in [None, Some(false), Some(true)] {
        for receipt_case in ReceiptCase::ALL {
            let allowed = receipt_case.permitted();
            let root = Root::new();
            let (mut config, path, loaded, reviewer, mut driver, mut wire, deadline) =
                generated(&root, mode);
            submit(
                &mut driver,
                &mut wire,
                &mut config,
                &loaded,
                &deadline,
                &mut || ElapsedTick(1000),
            )
            .unwrap();
            let (mut driver, history) =
                native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
            let human = native::approve(&mut driver, &mut config, &reviewer, 1);
            native::publish(&mut driver, &mut config, &human);
            assert!(executed(&mut wire, 1));
            assert_eq!(cleanup(driver, 1000, 1), 0);
            drop(history);
            // A newly provisioned request, never a mutation of the durable source.
            let next_path = root.0.join("finish-recipe.json");
            let next = fs::read_to_string(path)
                .unwrap()
                .replacen("\"request\":1", "\"request\":2", 1)
                .replacen("\"object_id\":1001", "\"object_id\":1002", 1)
                .replacen("\"round_ids\":[1]", "\"round_ids\":[2]", 1);
            fs::write(&next_path, next).unwrap();
            let mut last = recipe::load(&next_path, &config, true).unwrap();
            assert_eq!(last.generation, loaded.generation);
            let (host, reviewer, observer, _identity) =
                recovery::open_selected(&config, &last).unwrap();
            let mut prediction = observer.map(|role| prediction::Session::new(2, role));
            let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
            let mut driver = FileSupervisedDriver::new(supervisor);
            let mut wire = ActorWire::new(port);
            let mut control = Control::new(&config, 2, None).unwrap();
            assert!(
                super::super::super::finish::prepare(
                    &mut driver,
                    &reviewer,
                    &mut config,
                    &last,
                    1,
                    &mut control,
                    &deadline,
                    &mut || ElapsedTick(1000)
                )
                .unwrap()
            );
            if let Some(prediction) = &mut prediction {
                prediction
                    .finish_request(&mut driver.supervisor_mut().host_mut().unwrap())
                    .unwrap();
            }
            receipt_case.observe(&root, &config);
            receipt_case.submit_release(
                &mut driver,
                &mut wire,
                &mut config,
                &last,
                &deadline,
                LearnedTextRelease::Finish,
            );
            let mut history = None;
            if allowed {
                let reviewed = review::run(
                    driver,
                    &mut config,
                    &last,
                    &reviewer,
                    &mut control,
                    &deadline,
                    &mut || ElapsedTick(1000),
                );
                assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
                driver = reviewed.driver;
                history = reviewed.history;
                assert_eq!(
                    history.as_ref().unwrap().status(),
                    NativeReviewStatus::Finished
                );
                let human = native::approve(&mut driver, &mut config, &reviewer, 2);
                native::publish(&mut driver, &mut config, &human);
            }
            let expected = response(&mut wire, 2).result.unwrap();
            let original = driver
                .supervisor()
                .host()
                .unwrap()
                .learned_text_stream_finish_request(1, 2)
                .unwrap();
            assert!(original.message.is_none());
            assert_eq!(original.deadline, deadline.logical);
            let saved_anchor = anchor(&root, &mut last, &driver);
            assert_eq!(cleanup(driver, 1000, 1), 0);
            drop(history);
            remove_live_assets(&root);
            fs::write(
                config.store.join("delivery.pending"),
                b"finish diagnostic staging",
            )
            .unwrap();
            let before = files(&config.store);
            for after in [None, Some(2), Some(99)] {
                let current = configured(&root);
                let mut selected = recipe::load(&next_path, &current, false).unwrap();
                selected.recovery_anchor = Some(
                    super::super::super::anchors::load(&root.0.join("external.anchor")).unwrap(),
                );
                let no_clock = || panic!("wrong finish selection cannot observe settlement time");
                let result = match after {
                    None => recovery::resume(current, selected, no_clock),
                    Some(after) => recovery::resume_finish(current, selected, after, no_clock),
                };
                assert!(
                    result.is_err(),
                    "mode {mode:?}, allowed {allowed}, after {after:?}"
                );
                assert_eq!(files(&config.store), before);
                assert_eq!(
                    fs::read(root.0.join("external.anchor")).unwrap(),
                    saved_anchor
                );
            }
            let result = recovery::resume_finish(config, last, 1, || {
                panic!("terminal finish recovery cannot repeat inference, review or time")
            })
            .unwrap();
            assert!(result.failure.is_none(), "{:?}", result.failure);
            let Knowledge::Known { value: old, .. } = expected else {
                panic!("expected terminal finish");
            };
            match result.response.result {
                Ok(Knowledge::Known { value, .. }) => {
                    assert_eq!(value, old);
                    assert_eq!(matches!(value, ActorOutcome::Executed), allowed);
                }
                other => panic!("lost original finish: {other:?}"),
            }
            assert!(!root.0.join("store/delivery.pending").exists());
            assert_eq!(
                fs::read(root.0.join("external.anchor")).unwrap(),
                saved_anchor
            );
        }
    }
}
