//! Real native terminal writes survive missing exports. Repair consumes only
//! the old independently retained pair and a closed original terminal suffix.
use super::*;

const REPAIR: &str = "--recover-terminal-checkpoint";

fn repairing(mut args: Vec<String>) -> Vec<String> {
    args.push(REPAIR.into());
    args
}

fn archive_paths(case: &Case, name: &str) -> (PathBuf, PathBuf) {
    (case.root.0.join(format!("{name}.anchor")), case.root.0.join(format!("{name}.source")))
}

#[test]
fn learned_terminal_repair_recovers_either_or_both_failed_exports_without_repeating_work() {
    for (mode, occupy_history, occupy_source) in [
        (None, true, true), (Some(false), true, false), (Some(true), false, true),
    ] {
        let case = dispatched(mode);
        let original = case.root.0.join("history-1.bin");
        let original_anchor = fs::read(&original).unwrap();
        let original_source = fs::read(case.root.0.join("source-1.bin")).unwrap();
        let (failed_anchor, failed_source) = archive_paths(&case, "interrupted");
        let combined = retaining(args(&case.config_path, &case.recipe, &case.profile,
            "--recover-stop", &original, Some(&failed_anchor)), &failed_source);
        let (result, report) = command(&combined, || {
            if occupy_history { fs::write(&failed_anchor, b"other history custody").unwrap(); }
            if occupy_source { fs::write(&failed_source, b"other source custody").unwrap(); }
            Ok(ElapsedTick(1001))
        }, &mut || Duration::ZERO);
        let error = result.unwrap_err();
        assert!(error.contains("acknowledged"));
        assert!(report.contains("\"status\":\"stopped_drained\""));
        if occupy_history { assert!(error.contains("history anchor retention failed")); }
        if occupy_source { assert!(error.contains("topology checkpoint retention failed")); }
        let state = observed(&case.config, &case.loaded);
        assert_eq!(state.revision, case.before.revision + 3);
        assert_eq!(state.executions, 0);
        assert_eq!(state.control.ledger.charged, 0);
        let complete = files(&case.config);

        for (index, phase) in ["--recover-stop", "--drain-stopped", "--stop-admission"].iter().enumerate() {
            let (anchor, source) = archive_paths(&case, &format!("repaired-{index}"));
            let retry = retaining(args(&case.config_path, &case.recipe, &case.profile,
                phase, &original, Some(&anchor)), &source);
            // Changing destinations alone still cannot relax exact-cut recovery.
            let (result, report) = command(&retry,
                || panic!("stale ordinary recovery cannot obtain time"), &mut || Duration::ZERO);
            assert!(result.unwrap_err().contains("journal advanced beyond the source checkpoint"));
            assert!(report.is_empty());
            assert_eq!(files(&case.config), complete);
            assert!(!anchor.exists());
            assert!(!source.exists());

            let (result, report) = command(&repairing(retry),
                || panic!("acknowledged completed repair must not acquire time"), &mut || Duration::ZERO);
            result.unwrap();
            assert!(report.contains(if *phase == "--stop-admission" {
                "\"appended_events\":0"
            } else { "\"recovery\":\"already_drained\"" }));
            paired(&case, &source, &anchor, &state);
            assert_eq!(files(&case.config), complete);
        }
        // The public command dispatcher must reach the same closed parser.
        let retry = repairing(args(&case.config_path, &case.recipe, &case.profile,
            "--stop-admission", &original, None));
        super::super::super::super::super::command(&retry, None).unwrap();
        assert_eq!(files(&case.config), complete);
        assert_eq!(fs::read(&original).unwrap(), original_anchor);
        assert_eq!(fs::read(case.root.0.join("source-1.bin")).unwrap(), original_source);
        if occupy_history { assert_eq!(fs::read(&failed_anchor).unwrap(), b"other history custody"); }
        if occupy_source { assert_eq!(fs::read(&failed_source).unwrap(), b"other source custody"); }
        for name in ["live-1.json", "inventory-1.json", "missing-next.json",
            "missing-next-inventory.json", "evidence.json", "native-roster.json",
            "helper-model.json", "helper-weights.safetensors"] {
            assert!(!case.root.0.join(name).exists());
        }
    }
}

#[test]
fn learned_terminal_repair_carries_interrupted_admission_into_the_original_later_drain() {
    for mode in [None, Some(false), Some(true)] {
        let case = dispatched(mode);
        let original = case.root.0.join("history-1.bin");
        let (lost_anchor, lost_source) = archive_paths(&case, "lost-admission");
        let admission = repairing(retaining(args(&case.config_path, &case.recipe, &case.profile,
            "--stop-admission", &original, Some(&lost_anchor)), &lost_source));
        let boundaries = Cell::new(0);
        let (result, report) = command(&admission,
            || panic!("admission must remain independent of settlement time"), &mut || {
                boundaries.set(boundaries.get() + 1);
                // The fifth input checkpoint follows both output admissions.
                // A competing custodian wins create_new before the native stop.
                if boundaries.get() == 5 {
                    fs::write(&lost_anchor, b"occupied after admission").unwrap();
                    fs::write(&lost_source, b"occupied after admission").unwrap();
                }
                Duration::ZERO
            });
        let error = result.unwrap_err();
        assert!(error.contains("history anchor retention failed"));
        assert!(error.contains("topology checkpoint retention failed"));
        assert!(report.contains("\"appended_events\":2"));
        assert!(report.contains("\"drain_attempted\":false"));
        let admitted = observed(&case.config, &case.loaded);
        assert_eq!(admitted.revision, case.before.revision + 2);
        assert_eq!(admitted.control.ledger.charged, case.before.control.ledger.charged);
        let stopped = files(&case.config);

        let (repaired_anchor, repaired_source) = archive_paths(&case, "repaired-admission");
        let retry = repairing(retaining(args(&case.config_path, &case.recipe, &case.profile,
            "--stop-admission", &original, Some(&repaired_anchor)), &repaired_source));
        let (result, report) = command(&retry,
            || panic!("admission repair must not acquire time"), &mut || Duration::ZERO);
        result.unwrap();
        assert!(report.contains("\"appended_events\":0"));
        paired(&case, &repaired_source, &repaired_anchor, &admitted);
        assert_eq!(files(&case.config), stopped);

        let (drained_anchor, drained_source) = archive_paths(&case, "drained");
        let drain = repairing(retaining(args(&case.config_path, &case.recipe, &case.profile,
            "--drain-stopped", &original, Some(&drained_anchor)), &drained_source));
        let mut wrong = drain.clone(); wrong[4] = "802".into();
        let (result, report) = command(&wrong,
            || panic!("a repair cannot replace the original stop identity"), &mut || Duration::ZERO);
        assert!(result.is_err());
        assert!(report.is_empty());
        assert_eq!(files(&case.config), stopped);
        let (result, report) = command(&drain, || Err(JournalError::Unavailable),
            &mut || Duration::ZERO);
        assert!(result.unwrap_err().contains("Unavailable"));
        assert!(report.is_empty());
        assert_eq!(files(&case.config), stopped);
        assert!(!drained_anchor.exists());
        assert!(!drained_source.exists());

        let calls = Cell::new(0);
        let (result, report) = command(&drain, || {
            calls.set(calls.get() + 1); Ok(ElapsedTick(1001))
        }, &mut || Duration::ZERO);
        result.unwrap();
        assert_eq!(calls.get(), 1);
        assert!(report.contains("\"status\":\"stopped_drained\""));
        let drained = observed(&case.config, &case.loaded);
        assert_eq!(drained.revision, admitted.revision + 1);
        assert_eq!(drained.executions, 0);
        assert_eq!(drained.control.ledger.charged, 0);
        paired(&case, &drained_source, &drained_anchor, &drained);
        assert_eq!(fs::read(&lost_anchor).unwrap(), b"occupied after admission");
        assert_eq!(fs::read(&lost_source).unwrap(), b"occupied after admission");
    }
}

#[test]
fn learned_terminal_repair_is_explicit_v2_only_and_keeps_separate_anchor_binding() {
    let case = dispatched(None);
    let anchor = case.root.0.join("history-1.bin");
    let base = args(&case.config_path, &case.recipe, &case.profile,
        "--stop-admission", &anchor, None);
    let original = files(&case.config);
    let mut no_topology = base.clone();
    let at = no_topology.iter().position(|arg| arg == "--topology-profile").unwrap();
    no_topology.drain(at..at + 2);
    let mut no_anchor = base.clone();
    let at = no_anchor.iter().position(|arg| arg == "--history-anchor").unwrap();
    no_anchor.drain(at..at + 2);
    let mut misplaced = base.clone(); misplaced.insert(3, REPAIR.into());
    let mut extra_value = repairing(base.clone()); extra_value.push("true".into());
    for invalid in [repairing(no_topology), repairing(no_anchor),
        repairing(repairing(base.clone())), misplaced, extra_value] {
        let (result, report) = command(&invalid,
            || panic!("invalid repair selection cannot acquire time"), &mut || Duration::ZERO);
        assert!(result.is_err(), "{invalid:?}");
        assert!(report.is_empty());
        assert_eq!(files(&case.config), original);
    }
    fs::write(case.root.0.join("manual-current.json"), case.saved.topology_bytes()).unwrap();
    fs::write(case.root.0.join("manual-inventory.json"), case.saved.inventory_bytes()).unwrap();
    let initial = pair(&case.root, "initial.json", "initial-inventory.json");
    let current = pair(&case.root, "manual-current.json", "manual-inventory.json");
    let next = pair(&case.root, "missing-next.json", "missing-next-inventory.json");
    let manual = case.root.0.join("manual-repair.json");
    fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{initial},"current":{current},"next":{next},"available":true,"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
        case.saved.last_operation() + 1, case.saved.work().checks,
        case.saved.work().reserved_read_bytes, case.saved.last_operation())).unwrap();
    let manual_args = repairing(args(&case.config_path, &case.recipe, &manual,
        "--stop-admission", &anchor, None));
    let (result, report) = command(&manual_args,
        || panic!("manual work minima cannot repair exact source custody"), &mut || Duration::ZERO);
    assert!(result.unwrap_err().contains("exact v2 source checkpoint"));
    assert!(report.is_empty());
    assert_eq!(files(&case.config), original);

    let newer_anchor = case.root.0.join("admitted.anchor");
    let valid = repairing(args(&case.config_path, &case.recipe, &case.profile,
        "--stop-admission", &anchor, Some(&newer_anchor)));
    command(&valid, || panic!("valid exact-cut admission needs no clock"),
        &mut || Duration::ZERO).0.unwrap();
    let stopped = files(&case.config);
    let mixed = repairing(args(&case.config_path, &case.recipe, &case.profile,
        "--stop-admission", &newer_anchor, None));
    let (result, report) = command(&mixed,
        || panic!("repair cannot mix independently selected cuts"), &mut || Duration::ZERO);
    assert!(result.unwrap_err().contains("different cuts"));
    assert!(report.is_empty());
    assert_eq!(files(&case.config), stopped);
}

#[test]
fn learned_terminal_repair_rejects_native_nonterminal_work_without_cleaning_staging() {
    let mut case = dispatched(Some(true));
    case.loaded.recovery_anchor = Some(case.saved.history_anchor().clone());
    custody::admit(&case.config, &mut case.loaded, Some(&case.profile), custody::Mode::Historical).unwrap();
    let (mut host, _, _, _identity) = recovery::open_selected(&case.config, &case.loaded).unwrap();
    let revision = host.revision();
    host.observe_time(revision, ElapsedTick(1001)).unwrap();
    drop(host);
    fs::write(case.config.store.join("delivery.pending"), b"nonterminal suffix staged evidence").unwrap();
    let original = files(&case.config);
    let (anchor, source) = archive_paths(&case, "refused");
    let repair = repairing(retaining(args(&case.config_path, &case.recipe, &case.profile,
        "--recover-stop", &case.root.0.join("history-1.bin"), Some(&anchor)), &source));
    let (result, report) = command(&repair,
        || panic!("unretained native work must refuse before terminal time"), &mut || Duration::ZERO);
    assert!(result.is_err());
    assert!(report.is_empty());
    assert_eq!(files(&case.config), original);
    assert!(!anchor.exists());
    assert!(!source.exists());
}
