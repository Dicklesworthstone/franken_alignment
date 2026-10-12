//! Independent receipt replay time budget over actual anchored learned histories.
//! Synthetic elapsed values select exact cancellation cuts; they do not measure
//! event latency, interrupt one reducer operation, or authenticate anchor custody.
use super::*;
use super::super::{anchors as custody, prediction};
use fa_reference::action::consequence::delivery::persistent::observed::{
    consistency::{FileConsistencySnapshot, learned::FileLearnedConsistencySnapshot},
    guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
};
use fa_reference::action::consequence::oversight::learned_host::HostedLearnedInspection;
use std::{collections::BTreeMap, path::Path};

const EXPIRED: &str = "learned receipt recovery replay deadline reached";

struct Case {
    root: Root,
    config: Config,
    loaded: Loaded,
    revision: u64,
    numerical: HostedLearnedInspection,
    consistency: Option<FileConsistencySnapshot>,
    learned: Option<FileLearnedConsistencySnapshot>,
    archive: Vec<u8>,
}

fn generated(mode: Option<bool>) -> Case {
    let root = Root::new(); let mut config = configured(&root);
    let path = fixture::write(&root, b"allow");
    if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
    evidence(&root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer) = recovery::create_selected(&config, &loaded).unwrap();
    let mut prediction = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
    let (revision, numerical, consistency, learned, archive) = {
        let host = driver.supervisor().host().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4); assert_eq!(numerical.sampled_draws, 2);
        assert!(numerical.work.accepted_decoder.scalar_products().unwrap() > 0);
        assert_eq!(host.inspect().executions, 0);
        let consistency = mode.map(|_| host.action_consistency_snapshot().unwrap());
        let learned = (mode == Some(true)).then(|| host.learned_action_consistency_snapshot().unwrap());
        let mut archive = Vec::new();
        host.history_anchor().unwrap().write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES).unwrap();
        (host.revision(), numerical, consistency, learned, archive)
    };
    drop(port); drop(prediction); assert_eq!(cleanup(driver, 1000, 1), 0);
    let anchor_path = root.0.join("independent-receipt.anchor");
    fs::write(&anchor_path, &archive).unwrap();
    loaded.recovery_anchor = Some(custody::load(&anchor_path).unwrap());
    // Neither the independent replay timer nor original numerical recovery may
    // acquire a fresh policy observation or start native helpers.
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    fs::remove_file(root.0.join("native-roster.json")).unwrap();
    config.timing.runtime_ms = 50;
    fs::write(config.store.join("delivery.pending"), b"retained receipt replay staging").unwrap();
    Case { root, config, loaded, revision, numerical, consistency, learned, archive }
}

fn files(store: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(store).unwrap().map(|entry| {
        let entry = entry.unwrap(); assert!(entry.file_type().unwrap().is_file());
        (entry.file_name().into_string().unwrap(), fs::read(entry.path()).unwrap())
    }).collect()
}

fn retained(case: &Case, host: &FileOversight, observer: bool) {
    let inspection = host.learned_generation_inspection().unwrap();
    assert_eq!(inspection.numerical, case.numerical);
    assert!(inspection.paused); assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(observer, case.consistency.is_some());
    if let Some(old) = &case.consistency {
        let current = host.action_consistency_snapshot().unwrap();
        assert_eq!(current.evidence, old.evidence);
        assert_eq!(current.pending_attempt, old.pending_attempt);
        assert!(current.coverage_lost);
        assert_eq!(current.evidence.samples(), 0);
    }
    if let Some(old) = &case.learned {
        let current = host.learned_action_consistency_snapshot().unwrap();
        assert_eq!(current.work, old.work);
        assert_eq!(current.retained_source_bytes, old.retained_source_bytes);
        assert!(current.work.probe_coordinates > 0 && current.retained_source_bytes > 0);
    }
    assert_eq!(fs::read(case.root.0.join("independent-receipt.anchor")).unwrap(), case.archive);
}

#[test]
fn expired_receipt_replay_refuses_before_an_existing_original_writer_lock() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode);
        let reads = case.config.source.status().read_attempts;
        let (held, _human, _observer) = recovery::open_selected(&case.config, &case.loaded).unwrap();
        let revision = held.revision();
        fs::write(case.config.store.join("delivery.pending"), b"held writer staging").unwrap();
        let original = files(&case.config.store); let mut calls = 0;
        let result = recovery::open_receipt_with_elapsed(&case.config, &case.loaded, &mut || {
            calls += 1;
            Duration::from_millis(case.config.timing.runtime_ms)
        });
        assert_eq!(result.err().as_deref(), Some(EXPIRED)); assert_eq!(calls, 1);
        assert_eq!(held.revision(), revision); assert_eq!(files(&case.config.store), original);
        assert_eq!(case.config.source.status().read_attempts, reads);
        drop(held);
        // The same independently loaded anchored history succeeds with time
        // available, releases original staging, and publishes exactly one fence.
        let mut calls = 0;
        let (host, _human, observer) = recovery::open_receipt_with_elapsed(
            &case.config, &case.loaded, &mut || { calls += 1; Duration::ZERO }).unwrap();
        assert_eq!(calls, usize::try_from(revision).unwrap() + 2);
        assert_eq!(host.revision(), revision + 1);
        assert!(!case.config.store.join("delivery.pending").exists());
        retained(&case, &host, observer.is_some());
        assert_eq!(case.config.source.status().read_attempts, reads);
    }
}

#[test]
fn receipt_wall_deadline_cancels_every_original_event_and_ready_cut_without_fence_or_refund() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode);
        let total = usize::try_from(case.revision).unwrap(); assert!(total > 4);
        let original = files(&case.config.store);
        let reads = case.config.source.status().read_attempts;
        let limit = Duration::from_millis(case.config.timing.runtime_ms);
        let before = limit.checked_sub(Duration::from_nanos(1)).unwrap();
        // Call 0 is before open; calls 1..=N+1 precede original events 0..N,
        // with N being Ready before the sole cleanup/fence/role boundary.
        for expire_at in 0..=total + 1 {
            let mut calls = 0;
            let result = recovery::open_receipt_with_elapsed(&case.config, &case.loaded, &mut || {
                assert_eq!(files(&case.config.store), original);
                let cut = calls; calls += 1;
                if cut == expire_at { limit } else { before }
            });
            assert_eq!(result.err().as_deref(), Some(EXPIRED), "mode {mode:?}, cut {expire_at}");
            assert_eq!(calls, expire_at + 1);
            assert_eq!(files(&case.config.store), original);
            assert_eq!(case.config.source.status().read_attempts, reads);
            assert_eq!(fs::read(case.root.0.join("independent-receipt.anchor")).unwrap(), case.archive);
            // A second attempt reaches its first replay checkpoint under a new
            // original lock, proving cancellation dropped the previous cursor.
            let mut retry_calls = 0;
            let retry = recovery::open_receipt_with_elapsed(&case.config, &case.loaded, &mut || {
                retry_calls += 1;
                if retry_calls == 1 { Duration::ZERO } else { limit }
            });
            assert_eq!(retry.err().as_deref(), Some(EXPIRED)); assert_eq!(retry_calls, 2);
            assert_eq!(files(&case.config.store), original);
        }
        let mut calls = 0;
        let (host, _human, observer) = recovery::open_receipt_with_elapsed(
            &case.config, &case.loaded, &mut || {
                calls += 1; assert_eq!(files(&case.config.store), original); before
            }).unwrap();
        assert_eq!(calls, total + 2);
        assert_eq!(host.revision(), case.revision + 1);
        assert!(!case.config.store.join("delivery.pending").exists());
        assert_ne!(fs::read(case.config.store.join("delivery.bin")).unwrap(), original["delivery.bin"]);
        retained(&case, &host, observer.is_some());
        assert_eq!(case.config.source.status().read_attempts, reads);
    }
}
