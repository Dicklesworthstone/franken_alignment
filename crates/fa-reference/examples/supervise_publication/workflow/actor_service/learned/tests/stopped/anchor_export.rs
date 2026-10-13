//! The actual operator exports only a native acknowledged terminal cut.
//! These tests use the original model, guards, replay and file writer. They do
//! not qualify independent custodian authentication or storage crash behavior.
use super::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn select_output(case: &mut Case, output: &Path) -> Result<(), String> {
    let args = vec!["create-learned-generated".into(), "config".into(), "recipe".into(),
        "--recover-stop".into(), "801".into(), "--history-anchor".into(),
        case.anchor.to_str().unwrap().into(), "--retain-history-anchor".into(),
        output.to_str().unwrap().into()];
    let (remaining, options) = custody::take_options(&args)?;
    assert_eq!(remaining.len(), 5);
    options.admit(&case.config, &mut case.loaded, true)
}

#[test]
fn stopped_export_captures_the_exact_private_cut_and_reuses_it_in_every_mode() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode, Some(RecoveryReserve::terminal()));
        let output = case.root.0.join("stopped.anchor");
        select_output(&mut case, &output).unwrap();
        let reads = case.config.source.status().read_attempts;
        let clocks = Cell::new(0); let mut stdout = Vec::new();
        command_stop::run(&case.config, &case.loaded, 801, || {
            clocks.set(clocks.get() + 1); ElapsedTick(1001)
        }, &mut stdout).unwrap();
        assert_eq!(clocks.get(), 1); report(stdout, "advanced", case.before.revision + 3);
        let state = observed(&case); let canonical = files(&case.config.store);
        let anchor = custody::load(&output).unwrap();
        assert_eq!(anchor.revision(), state.revision);
        assert_eq!(anchor.retained_bytes(), canonical["delivery.bin"].len());
        #[cfg(unix)]
        assert_eq!(fs::metadata(&output).unwrap().permissions().mode() & 0o077, 0);
        // Native capture is at the same confirmed cut, not a reopened live owner.
        let original = recovery::stop_selected_with_anchor_with_elapsed(
            &case.config, &case.loaded, 801, || panic!("drained capture reads no clock"),
            &mut || Duration::ZERO).unwrap();
        assert_eq!(original.history_anchor(), &anchor);
        assert_eq!(original.stopped().snapshot(), &state);
        assert_eq!(files(&case.config.store), canonical);
        let second = case.root.0.join("stopped-again.anchor");
        case.loaded.recovery_anchor = Some(anchor);
        case.loaded.anchor_output = Some(second.clone());
        let mut stdout = Vec::new();
        command_stop::run(&case.config, &case.loaded, 801,
            || panic!("export retry cannot repeat stopping"), &mut stdout).unwrap();
        report(stdout, "already_drained", state.revision);
        assert_eq!(fs::read(&second).unwrap(), fs::read(&output).unwrap());
        assert_eq!(files(&case.config.store), canonical); assert_eq!(observed(&case), state);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert_eq!(case.config.source.status().read_attempts, reads);
        assert_eq!(fs::read_dir(case.root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn actual_stop_cli_retains_and_reuses_an_anchor_without_live_assets() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode, Some(RecoveryReserve::terminal()));
        let config = write_config(&case.root); let output = case.root.0.join("cli-stop.anchor");
        fs::remove_dir(case.root.0.join("peers")).unwrap();
        let missing = case.root.0.join("missing-credibility");
        let args = vec!["create-learned-generated".into(), config.to_str().unwrap().into(),
            case.recipe.to_str().unwrap().into(), "--recover-stop".into(), "801".into(),
            "--history-anchor".into(), case.anchor.to_str().unwrap().into(),
            "--retain-history-anchor".into(), output.to_str().unwrap().into(),
            "--credibility-activation".into(), missing.to_str().unwrap().into()];
        crate::command(args).unwrap();
        let state = observed(&case); let canonical = files(&case.config.store);
        assert_eq!(state.revision, case.before.revision + 3);
        assert_eq!(custody::load(&output).unwrap().revision(), state.revision);
        let second = case.root.0.join("cli-stop-again.anchor");
        crate::command(vec!["create-learned-generated".into(), config.to_str().unwrap().into(),
            case.recipe.to_str().unwrap().into(), "--recover-stop".into(), "801".into(),
            "--history-anchor".into(), output.to_str().unwrap().into(),
            "--retain-history-anchor".into(), second.to_str().unwrap().into()]).unwrap();
        assert_eq!(fs::read(&second).unwrap(), fs::read(&output).unwrap());
        assert_eq!(files(&case.config.store), canonical); assert_eq!(observed(&case), state);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert!(!missing.exists()); assert!(!case.root.0.join("peers").exists());
    }
}

#[test]
fn a_late_output_collision_preserves_the_stop_report_and_allows_retention_only_retry() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode, Some(RecoveryReserve::terminal()));
        let output = case.root.0.join("racing.anchor"); select_output(&mut case, &output).unwrap();
        let mut stdout = Vec::new();
        let error = command_stop::run(&case.config, &case.loaded, 801, || {
            fs::write(&output, b"independent winner").unwrap(); ElapsedTick(1001)
        }, &mut stdout).unwrap_err();
        assert!(error.contains("native stop acknowledged"));
        assert!(error.contains("history anchor retention failed"));
        assert!(error.contains("no effect retry was sent"));
        report(stdout, "advanced", case.before.revision + 3);
        assert_eq!(fs::read(&output).unwrap(), b"independent winner");
        let state = observed(&case); let canonical = files(&case.config.store);
        let corrected = case.root.0.join("corrected.anchor");
        select_output(&mut case, &corrected).unwrap(); let mut stdout = Vec::new();
        command_stop::run(&case.config, &case.loaded, 801,
            || panic!("retention failure must not repeat a drained stop"), &mut stdout).unwrap();
        report(stdout, "already_drained", state.revision);
        assert_eq!(custody::load(&corrected).unwrap().revision(), state.revision);
        assert_eq!(files(&case.config.store), canonical); assert_eq!(observed(&case), state);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    }
}

#[test]
fn stop_export_rejects_unsafe_output_custody_before_any_terminal_mutation() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode, Some(RecoveryReserve::terminal()));
        let original = files(&case.config.store);
        let inside = case.config.store.join("not-independent.anchor");
        let error = select_output(&mut case, &inside).unwrap_err();
        assert!(error.contains("outside the authority store")); assert!(!inside.exists());
        let existing = case.anchor.clone();
        let error = select_output(&mut case, &existing).unwrap_err();
        assert!(error.contains("must be a new file"));
        assert_eq!(fs::read(&existing).unwrap(), case.archive);
        let absent_parent = case.root.0.join("absent-parent").join("stopped.anchor");
        assert!(select_output(&mut case, &absent_parent).is_err());
        assert!(!absent_parent.exists()); assert!(case.loaded.anchor_output.is_none());
        assert_eq!(files(&case.config.store), original); assert_eq!(observed(&case), case.before);
        let valid = case.root.0.join("allowed.anchor"); select_output(&mut case, &valid).unwrap();
        command_stop::run(&case.config, &case.loaded, 801,
            || ElapsedTick(1001), &mut Vec::new()).unwrap();
        assert_eq!(custody::load(&valid).unwrap().revision(), case.before.revision + 3);
    }
}

#[test]
fn report_loss_preserves_successful_export_and_does_not_hide_a_simultaneous_export_failure() {
    for mode in [None, Some(false), Some(true)] {
        for collide in [false, true] {
            let mut case = generated(mode, Some(RecoveryReserve::terminal()));
            let output = case.root.0.join("report-loss.anchor"); select_output(&mut case, &output).unwrap();
            let mut stdout = FailedOutput { writes: 0 };
            let error = command_stop::run(&case.config, &case.loaded, 801, || {
                if collide { fs::write(&output, b"existing archive").unwrap(); }
                ElapsedTick(1001)
            }, &mut stdout).unwrap_err();
            assert_eq!(stdout.writes, 1); assert!(error.contains("output failed"));
            assert!(error.contains("native stop acknowledged"));
            assert_eq!(error.contains("history anchor retention failed"), collide);
            let state = observed(&case); assert_eq!(state.revision, case.before.revision + 3);
            if collide { assert_eq!(fs::read(&output).unwrap(), b"existing archive"); }
            else { assert_eq!(custody::load(&output).unwrap().revision(), state.revision); }
            let canonical = files(&case.config.store); let retry = case.root.0.join("report-loss-retry.anchor");
            select_output(&mut case, &retry).unwrap();
            command_stop::run(&case.config, &case.loaded, 801,
                || panic!("lost report never repeats the native stop"), &mut Vec::new()).unwrap();
            assert_eq!(files(&case.config.store), canonical);
            assert_eq!(custody::load(&retry).unwrap().revision(), state.revision);
            assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        }
    }
}

#[test]
fn anchored_stop_capture_preserves_every_original_replay_deadline_boundary() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode, Some(RecoveryReserve::terminal()));
        case.config.timing.runtime_ms = 50;
        let output = case.root.0.join("not-yet-exported.anchor"); select_output(&mut case, &output).unwrap();
        let total = usize::try_from(case.before.revision).unwrap(); let original = files(&case.config.store);
        let limit = Duration::from_millis(50); let before = limit.checked_sub(Duration::from_nanos(1)).unwrap();
        for expire_at in 0..=total + 1 {
            let mut calls = 0;
            let result = recovery::stop_selected_with_anchor_with_elapsed(
                &case.config, &case.loaded, 801, || panic!("cancelled capture has no terminal clock"),
                &mut || { let cut = calls; calls += 1; if cut == expire_at { limit } else { before } });
            assert_eq!(result.err().as_deref(), Some(EXPIRED), "mode {mode:?}, cut {expire_at}");
            assert_eq!(calls, expire_at + 1); assert_eq!(files(&case.config.store), original);
            assert!(!output.exists()); assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        }
        let clocks = Cell::new(0); let mut calls = 0;
        let history = recovery::stop_selected_with_anchor_with_elapsed(&case.config, &case.loaded, 801,
            || { clocks.set(clocks.get() + 1); ElapsedTick(1001) },
            &mut || { calls += 1; before }).unwrap();
        assert_eq!(calls, total + 2); assert_eq!(clocks.get(), 1);
        assert_eq!(history.stopped().snapshot().revision, case.before.revision + 3);
        assert_eq!(history.history_anchor().revision(), case.before.revision + 3);
        assert_eq!(history.history_anchor().retained_bytes(), fs::read(case.config.store.join("delivery.bin")).unwrap().len());
        assert!(!output.exists(), "native capture alone is not an export acknowledgment");
    }
}

#[test]
fn invalid_stop_intent_or_independent_guard_cannot_export_a_candidate_anchor() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode, Some(RecoveryReserve::terminal()));
        let output = case.root.0.join("guarded.anchor"); select_output(&mut case, &output).unwrap();
        let original = files(&case.config.store); let floor = case.loaded.floor;
        let mut stdout = Vec::new();
        assert_eq!(command_stop::run(&case.config, &case.loaded, 0,
            || panic!("zero operation must not read time"), &mut stdout).unwrap_err(), "stop operation must be nonzero");
        case.loaded.floor.journal_revision = case.before.revision + 1;
        let error = command_stop::run(&case.config, &case.loaded, 801,
            || panic!("stale guard must not read time"), &mut stdout).unwrap_err();
        assert!(error.contains(&debug(JournalError::Contract(Error::Stale))));
        case.loaded.floor = floor;
        let mut foreign = generated(mode, Some(RecoveryReserve::terminal()));
        let own = case.loaded.recovery_anchor.take();
        case.loaded.recovery_anchor = foreign.loaded.recovery_anchor.take();
        let error = command_stop::run(&case.config, &case.loaded, 801,
            || panic!("foreign anchor must not read time"), &mut stdout).unwrap_err();
        assert!(error.contains(&debug(JournalError::Contract(Error::Binding))));
        assert!(stdout.is_empty()); assert!(!output.exists()); assert_eq!(files(&case.config.store), original);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        case.loaded.recovery_anchor = own;
        command_stop::run(&case.config, &case.loaded, 801, || ElapsedTick(1001), &mut stdout).unwrap();
        assert_eq!(custody::load(&output).unwrap().revision(), case.before.revision + 3);
    }
}
