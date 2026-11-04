//! Real command repair of a reset/Stop cut whose paired exports were lost.
use super::*;
use super::native_terminal::stopped_case;
use std::cell::Cell;

fn arguments(root: &Root, case: &SourceCase) -> Vec<String> {
    let mut args = super::arguments(root, case);
    args[3] = "--recover-stopped-stream-reset".into();
    args.insert(5, "900".into());
    args
}

fn confirmed(bytes: &[u8]) {
    let value = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(value.get("schema_version").and_then(Json::as_str),
        Some("fa.admission-stop/1"));
    assert_eq!(value.get("stop_acknowledged").and_then(Json::as_bool), Some(true));
    assert_eq!(value.get("drain_attempted").and_then(Json::as_bool), Some(false));
    assert_eq!(value.get("operation").and_then(Json::as_str), Some("900"));
    assert!(std::str::from_utf8(bytes).unwrap().contains("\"appended_events\":0"));
}

#[test]
fn terminal_command_repairs_lost_custody_and_retries_the_new_pair_without_new_events() {
    for completed in [false, true] {
        let root = Root::new();
        let case = stopped_case(&root, completed);
        let args = arguments(&root, &case);
        let canonical = disk(&case.case);
        let state = snapshot(&case.case);
        let old_anchor = fs::read(&args[7]).unwrap();
        let old_source = fs::read(root.0.join("source-1.bin")).unwrap();
        let staging = case.case.config.store.join("delivery.pending");
        fs::write(&staging, b"confirmed terminal retry may clean unrelated staging").unwrap();
        remove_live_inputs(&root);
        let mut out = Vec::new();
        reset::terminal::command(&args, None, || ElapsedTick(1000), &mut out).unwrap();
        confirmed(&out);
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(snapshot(&case.case), state);
        assert!(!staging.exists(), "original native confirmation owns staging cleanup");
        assert_eq!(fs::read(&args[7]).unwrap(), old_anchor);
        assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), old_source);
        let saved = checkpoint(&root.0.join("source-reset.bin"));
        unchanged_source(&case.saved, &saved);
        assert_eq!(saved.history_anchor().revision(), state.revision);
        assert_eq!(saved.history_anchor(), &custody::load(Path::new(&args[9])).unwrap());

        // The executable selector must preserve the terminal grammar and accept
        // the newly retained pair without recording another terminal event.
        let next_profile = profile(&root, &saved, "source-reset.bin", Some("source-terminal-retry.bin"));
        let mut retry = args.clone();
        retry[7] = args[9].clone();
        retry[9] = root.0.join("terminal-retry.anchor").display().to_string();
        retry[11] = next_profile.display().to_string();
        crate::command_with_output(retry.clone(), &mut Vec::new()).unwrap();
        assert_eq!(disk(&case.case), canonical);
        let next = checkpoint(&root.0.join("source-terminal-retry.bin"));
        unchanged_source(&saved, &next);
        assert_eq!(next.history_anchor(), &custody::load(Path::new(&retry[9])).unwrap());
        assert_eq!(next.history_anchor(), saved.history_anchor());
    }
}

#[test]
fn missing_stop_or_changed_stop_reset_and_frontier_cannot_confirm_or_clean() {
    for completed in [false, true] {
        let root = Root::new();
        let case = prepared(&root, false);
        if completed {
            let (host, _roles) = native_ready(&case).finish_pending_stream_reset().unwrap();
            drop(host);
        }
        let canonical = disk(&case.case);
        let staging = case.case.config.store.join("delivery.pending");
        fs::write(&staging, b"no acknowledged Stop to confirm").unwrap();
        let mut out = Vec::new();
        assert!(reset::terminal::run(&case.case.config, &case.case.loaded, &case.case.request,
            900, &deadline(), &mut || ElapsedTick(1000), &mut out).is_err());
        assert!(out.is_empty());
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"no acknowledged Stop to confirm");
        assert!(!case.case.loaded.anchor_output.as_ref().unwrap().exists());
        assert!(!root.0.join("source-reset.bin").exists());
    }
    let root = Root::new();
    let case = stopped_case(&root, true);
    let canonical = disk(&case.case);
    let staging = case.case.config.store.join("delivery.pending");
    fs::write(&staging, b"exact original terminal expectation required").unwrap();
    for changed in 0..5 {
        let mut request = case.case.request.clone();
        let mut stop = 900;
        let mut control = request.intent.control().clone();
        let mut budget = request.intent.budget();
        let mut checkpoint = request.intent.checkpoint();
        match changed {
            0 => stop = 901,
            1 => request.first_incident_sequence += 1,
            2 => control.binding.evidence_root[0] ^= 1,
            3 => budget.cache_values -= 1,
            _ => checkpoint += 1,
        }
        request.intent = FileLearnedResetIntent::for_recovery(checkpoint, control, budget).unwrap();
        let mut out = Vec::new();
        assert!(reset::terminal::run(&case.case.config, &case.case.loaded, &request,
            stop, &deadline(), &mut || ElapsedTick(1000), &mut out).is_err());
        assert!(out.is_empty());
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"exact original terminal expectation required");
        assert!(!case.case.loaded.anchor_output.as_ref().unwrap().exists());
        assert!(!root.0.join("source-reset.bin").exists());
    }
}

#[test]
fn terminal_repair_attempts_both_exports_after_a_create_new_race() {
    for occupied_history in [false, true] {
        let root = Root::new();
        let case = stopped_case(&root, true);
        let canonical = disk(&case.case);
        let history = case.case.loaded.anchor_output.as_ref().unwrap().clone();
        let source = root.0.join("source-reset.bin");
        let occupied = if occupied_history { &history } else { &source };
        let mut first = true;
        let mut out = Vec::new();
        let error = reset::terminal::run(&case.case.config, &case.case.loaded,
            &case.case.request, 900, &deadline(), &mut || {
                if first {
                    fs::write(occupied, b"independent output appeared after admission").unwrap();
                    first = false;
                }
                ElapsedTick(1000)
            }, &mut out).unwrap_err();
        confirmed(&out);
        assert!(error.contains("confirmed at revision"), "{error}");
        assert!(error.contains("retention failed"), "{error}");
        assert_eq!(fs::read(occupied).unwrap(), b"independent output appeared after admission");
        assert_eq!(disk(&case.case), canonical);
        if occupied_history {
            let retained = checkpoint(&source);
            unchanged_source(&case.saved, &retained);
            assert_eq!(retained.history_anchor().revision(), snapshot(&case.case).revision);
        } else {
            assert_eq!(custody::load(&history).unwrap().revision(), snapshot(&case.case).revision);
        }
    }
}

#[test]
fn terminal_repair_deadline_vetoes_confirmation_but_late_output_preserves_both_archives() {
    let root = Root::new();
    let case = stopped_case(&root, true);
    let canonical = disk(&case.case);
    let staging = case.case.config.store.join("delivery.pending");
    fs::write(&staging, b"terminal Ready is still vetoable").unwrap();
    let total = usize::try_from(snapshot(&case.case).revision).unwrap();
    for expire_at in [0, total / 2, total + 1] {
        let limit = deadline();
        let mut calls = 0;
        let mut out = Vec::new();
        let error = reset::terminal::run(&case.case.config, &case.case.loaded,
            &case.case.request, 900, &limit, &mut || {
                let at = calls; calls += 1;
                if at == expire_at { limit.logical } else { ElapsedTick(1000) }
            }, &mut out).unwrap_err();
        assert!(error.contains("deadline reached"), "{error}");
        assert_eq!(calls, expire_at + 1);
        assert!(out.is_empty());
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"terminal Ready is still vetoable");
        assert!(!case.case.loaded.anchor_output.as_ref().unwrap().exists());
        assert!(!root.0.join("source-reset.bin").exists());
    }

    struct Late<'a> { bytes: Vec<u8>, expired: &'a Cell<bool> }
    impl Write for Late<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            self.expired.set(true);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let expired = Cell::new(false);
    let mut out = Late { bytes: Vec::new(), expired: &expired };
    let limit = deadline();
    let error = reset::terminal::run(&case.case.config, &case.case.loaded,
        &case.case.request, 900, &limit, &mut || {
            if expired.get() { limit.logical } else { ElapsedTick(1000) }
        }, &mut out).unwrap_err();
    assert!(error.contains("deadline reached during acknowledged output"), "{error}");
    confirmed(&out.bytes);
    assert_eq!(disk(&case.case), canonical);
    assert!(!staging.exists());
    assert!(case.case.loaded.anchor_output.as_ref().unwrap().exists());
    let saved = checkpoint(&root.0.join("source-reset.bin"));
    unchanged_source(&case.saved, &saved);
    assert_eq!(saved.history_anchor(),
        &custody::load(case.case.loaded.anchor_output.as_ref().unwrap()).unwrap());
}

#[test]
fn terminal_repair_grammar_rejects_live_modes_and_noncanonical_operations_before_io() {
    let valid: Vec<String> = [
        "create-learned-generated", "/missing/config", "/missing/recipe",
        "--recover-stopped-stream-reset", "/held/reset.json", "900",
        "--history-anchor", "/held/input.anchor",
        "--retain-history-anchor", "/held/new.anchor",
        "--topology-profile", "/held/source.json",
    ].into_iter().map(str::to_owned).collect();
    let mut invalid = Vec::new();
    for operation in ["0", "0900", "-1", "18446744073709551616", "--resume"] {
        let mut args = valid.clone(); args[5] = operation.into(); invalid.push(args);
    }
    for flag in ["--stop-profile", "--resume", "--credibility-activation",
        "--continue-generation", "--complete-stream-reset"] {
        let mut args = valid.clone(); args.extend([flag.into(), "/held/incompatible".into()]);
        invalid.push(args);
    }
    let mut duplicate = valid.clone();
    duplicate[10] = "--history-anchor".into();
    invalid.push(duplicate);
    let mut missing = valid.clone(); missing.truncate(10); invalid.push(missing);
    for args in invalid {
        let error = reset::terminal::command(&args, None,
            || panic!("invalid terminal grammar reached clock"), &mut Vec::new()).unwrap_err();
        assert!(!error.contains("No such file"), "{error}");
    }
    assert!(reset::terminal::command(&valid, Some(Path::new("/held/credibility")),
        || panic!("credibility reached terminal clock"), &mut Vec::new()).is_err());
}
