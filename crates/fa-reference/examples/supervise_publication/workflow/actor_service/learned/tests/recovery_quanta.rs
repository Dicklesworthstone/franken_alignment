//! Real loaded generation, original guarded replay and unchanged disk on veto.
//! These tests exercise ordinary, raw-residual and owned-K/V recovery. The
//! synthetic checkpoints are cancellation controls, not measured latency claims.
use super::*;
use super::super::prediction;
use fa_reference::action::consequence::delivery::persistent::observed::{
    consistency::FileConsistencySnapshot,
    decoder::learned::FileLearnedRecoveryStatus as RecoveryStatus,
};
use fa_reference::action::consequence::oversight::learned_host::HostedLearnedInspection;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Case {
    _root: Root,
    config: Config,
    loaded: Loaded,
    numerical: HostedLearnedInspection,
    consistency: Option<FileConsistencySnapshot>,
}

fn generated(mode: Option<bool>) -> Case {
    let root = Root::new();
    let mut config = configured(&root);
    let path = super::continuation::write(&root);
    if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer) = recovery::create_selected(&config, &loaded).unwrap();
    let mut forecast = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000), forecast.as_mut()).unwrap());
    let (numerical, consistency) = {
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.sampled_draws, 2);
        let consistency = mode.map(|_| host.action_consistency_snapshot().unwrap());
        (numerical, consistency)
    };
    drop(port);
    drop(forecast);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    Case { _root: root, config, loaded, numerical, consistency }
}

#[test]
fn loaded_recovery_yields_and_cancellation_preserves_canonical_and_staged_bytes_in_all_modes() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode);
        let canonical = case.config.store.join("delivery.bin");
        let staged = case.config.store.join("delivery.pending");
        let original = fs::read(&canonical).unwrap();
        fs::write(&staged, b"uncommitted diagnostic bytes").unwrap();
        // Before opening, partway through history, and after full verification.
        // The last cut must still precede staged-file cleanup and fencing.
        for cut in [None, Some(2), Some(usize::MAX)] {
            let result = recovery::open_selected_with_checkpoints(&case.config, &case.loaded, &mut |view| {
                assert_eq!(fs::read(&canonical).unwrap(), original);
                assert_eq!(fs::read(&staged).unwrap(), b"uncommitted diagnostic bytes");
                let cancel = match (cut, view) {
                    (None, None) => true,
                    (Some(n), Some(p)) => p.replayed_events == n
                        || (n == usize::MAX && p.status == RecoveryStatus::Ready),
                    _ => false,
                };
                if cancel { Err("test recovery veto".into()) } else { Ok(()) }
            });
            assert_eq!(result.err().as_deref(), Some("test recovery veto"));
            assert_eq!(fs::read(&canonical).unwrap(), original);
            assert_eq!(fs::read(&staged).unwrap(), b"uncommitted diagnostic bytes");
        }
        // Same store/recipe, now permitted: exact original numerical replay,
        // one fence and the original role inventory, not a substitute owner.
        let mut views = Vec::new();
        let (host, _human, observer) = recovery::open_selected_with_checkpoints(
            &case.config, &case.loaded, &mut |view| { views.push(view); Ok(()) }).unwrap();
        assert_eq!(views[0], None);
        let progress: Vec<_> = views.into_iter().flatten().collect();
        let ready = progress.last().unwrap();
        assert_eq!(ready.status, RecoveryStatus::Ready);
        assert_eq!(ready.replayed_events, ready.total_events);
        assert_eq!(progress.len(), ready.total_events + 1);
        for (index, view) in progress.iter().enumerate() { assert_eq!(view.replayed_events, index); }
        assert_eq!(host.revision(), ready.total_events as u64 + 1);
        let inspection = host.learned_generation_inspection().unwrap();
        assert!(inspection.paused);
        assert_eq!(inspection.numerical, case.numerical);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(observer.is_some(), mode.is_some());
        if let Some(old) = case.consistency {
            let current = host.action_consistency_snapshot().unwrap();
            assert_eq!(current.evidence, old.evidence);
            assert_eq!(current.pending_attempt, old.pending_attempt);
            assert!(current.coverage_lost); // No repair/refund of an unanswered forecast.
        }
        assert!(!staged.exists());
        assert_ne!(fs::read(&canonical).unwrap(), original);
    }
}

#[test]
fn checkpoint_unwind_releases_original_recovery_lock_without_cleanup_or_fence() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode);
        let canonical = case.config.store.join("delivery.bin");
        let original = fs::read(&canonical).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = recovery::open_selected_with_checkpoints(&case.config, &case.loaded, &mut |view| {
                if view.is_some_and(|p| p.replayed_events == 1) { panic!("checkpoint unwind control"); }
                Ok(())
            });
        }));
        assert!(result.is_err());
        assert_eq!(fs::read(&canonical).unwrap(), original);
        let (host, _, _) = recovery::open_selected(&case.config, &case.loaded).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, case.numerical);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn cooperative_replay_does_not_skip_original_external_floor_checks() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode);
        let canonical = case.config.store.join("delivery.bin");
        let original = fs::read(&canonical).unwrap();
        let floor = case.loaded.floor;
        case.loaded.floor.journal_revision = u64::MAX;
        let mut ready = false;
        let result = recovery::open_selected_with_checkpoints(&case.config, &case.loaded, &mut |view| {
            ready |= view.is_some_and(|p| p.status == RecoveryStatus::Ready);
            Ok(())
        });
        assert!(result.is_err());
        assert!(!ready);
        assert_eq!(fs::read(&canonical).unwrap(), original);
        case.loaded.floor = floor;
        let (host, _, _) = recovery::open_selected(&case.config, &case.loaded).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, case.numerical);
    }
}
