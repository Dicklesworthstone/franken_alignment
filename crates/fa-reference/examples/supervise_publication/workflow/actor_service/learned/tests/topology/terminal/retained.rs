//! Carry original source expenditure through offline terminal cuts, including
//! a real dispatched liability and failures of independently retained outputs.
use super::*;
use std::io::{self, Write};

struct Case {
    root: Root,
    config: Config,
    config_path: PathBuf,
    recipe: PathBuf,
    loaded: Loaded,
    saved: TopologyCheckpoint,
    before: FileDeliverySnapshot,
    profile: PathBuf,
}

fn dispatched(mode: Option<bool>) -> Case {
    let root = Root::new();
    let (mut config, recipe, loaded) = selected(&root, mode, false);
    let deadline = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    let custody::Started { custody: (host, reviewer, observer), source, .. } =
        custody::start(&config, &loaded, false, &mut control, &deadline,
            &mut || ElapsedTick(1000)).unwrap();
    let mut prediction = observer.map(|observer| prediction::Session::new(1, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    custody::attach(&mut driver, source).unwrap();
    custody::certify(&mut driver, &deadline, &mut || ElapsedTick(1000)).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline,
        &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, review) = native::review_current(
        &mut config, &loaded, &reviewer, driver, &deadline);
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), Some(&human)).result.unwrap(),
        FileDriverEvent::Dispatched { .. }));
    let host = driver.supervisor().host().unwrap();
    let before = host.inspect();
    assert_eq!(before.executions, 0);
    assert!(before.control.ledger.charged > 0);
    assert!(before.stop.is_none());
    let saved = driver.topology().unwrap().checkpoint(&host,
        &fs::read(root.0.join("live-1.json")).unwrap(),
        &fs::read(root.0.join("inventory-1.json")).unwrap()).unwrap();
    assert!(saved.available());
    assert!(saved.work().checks > 0);
    anchors::export_acknowledged(saved.history_anchor(), &root.0.join("history-1.bin")).unwrap();
    let mut bytes = Vec::new();
    saved.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    fs::write(root.0.join("source-1.bin"), bytes).unwrap();
    drop(host);
    drop(review);
    drop(wire);
    drop(prediction);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let profile = archived_selection(&root, &saved, 1, None);
    let loaded = recipe::load(&recipe, &config, false).unwrap();
    assert_eq!(observed(&config, &loaded), before);
    let config_path = root.0.join("terminal-config.json");
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))
        .replace("/operator/publication-store", config.store.to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("\"initial_payload_hex\": \"696e697469616c\"", "\"initial_payload_hex\": \"\"")
        .replace("\"events\": 256", "\"events\": 1024")
        .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 60000");
    fs::write(&config_path, text).unwrap();
    crate::config::assert_oversight_profiles_eq(
        &Config::read(&config_path).unwrap().profile, &config.profile);
    for name in ["live-1.json", "inventory-1.json", "evidence.json", "native-roster.json",
        "helper-model.json", "helper-weights.safetensors", "helper-monitor.json",
        "helper-tokenizer.bbpe", "helper-sampling.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::write(config.store.join("delivery.pending"), b"offline terminal source custody").unwrap();
    Case { root, config, config_path, recipe, loaded, saved, before, profile }
}

fn retaining(mut args: Vec<String>, output: &Path) -> Vec<String> {
    args.extend(["--retain-topology-checkpoint".into(), output.display().to_string()]);
    args
}

fn read_saved(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}

fn paired(case: &Case, source: &Path, anchor: &Path,
    state: &FileDeliverySnapshot) -> TopologyCheckpoint {
    let saved = read_saved(source);
    assert_eq!(saved.history_anchor(), &anchors::load(anchor).unwrap());
    assert_eq!(saved.minimum().journal_revision, state.revision);
    assert_eq!(saved.minimum().control_sequence, state.control.sequence);
    assert_eq!(saved.minimum().authority_epoch, state.control.ledger.epoch);
    assert!(!saved.available());
    assert_eq!(saved.work(), case.saved.work());
    assert_eq!(saved.last_operation(), case.saved.last_operation());
    assert_eq!(saved.topology_bytes(), case.saved.topology_bytes());
    assert_eq!(saved.inventory_bytes(), case.saved.inventory_bytes());
    assert_eq!(saved.expected(), case.saved.expected());
    saved
}

fn selection(case: &Case, saved: &TopologyCheckpoint, source: &Path, name: &str) -> PathBuf {
    let path = case.root.0.join(name);
    let initial = pair(&case.root, "initial.json", "initial-inventory.json");
    let next = pair(&case.root, "missing-next.json", "missing-next-inventory.json");
    fs::write(&path, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{initial},"next":{next},"operation":{},"source_checkpoint":"{}"}}"#,
        saved.last_operation() + 1, source.display())).unwrap();
    path
}

#[test]
fn learned_terminal_checkpoint_carries_real_sent_work_through_admission_drain_and_retries() {
    for mode in [None, Some(false), Some(true)] {
        let case = dispatched(mode);
        let original_anchor = case.root.0.join("history-1.bin");
        let original_source = case.root.0.join("source-1.bin");
        let original_archive = fs::read(&original_anchor).unwrap();
        let original_bytes = fs::read(&original_source).unwrap();
        let admitted_anchor = case.root.0.join("admitted.anchor");
        let admitted_source = case.root.0.join("admitted.source");
        let admission = retaining(args(&case.config_path, &case.recipe, &case.profile,
            "--stop-admission", &original_anchor, Some(&admitted_anchor)), &admitted_source);
        let (result, output) = command(&admission,
            || panic!("admission and checkpoint capture must not acquire settlement time"),
            &mut || Duration::ZERO);
        result.unwrap();
        assert!(output.contains("\"appended_events\":2"));
        assert!(output.contains("\"drain_attempted\":false"));
        assert!(output.contains("\"drained\":false"));
        let admitted = observed(&case.config, &case.loaded);
        assert_eq!(admitted.revision, case.before.revision + 2);
        assert_eq!(admitted.executions, 0);
        assert_eq!(admitted.control.ledger.charged, case.before.control.ledger.charged);
        let saved = paired(&case, &admitted_source, &admitted_anchor, &admitted);
        let admitted_profile = selection(&case, &saved, &admitted_source, "admitted-profile.json");
        let stopped = files(&case.config);

        // A later native clock failure does not discard the usable stopped pair.
        let drained_anchor = case.root.0.join("drained.anchor");
        let drained_source = case.root.0.join("drained.source");
        let drain = retaining(args(&case.config_path, &case.recipe, &admitted_profile,
            "--drain-stopped", &admitted_anchor, Some(&drained_anchor)), &drained_source);
        let (result, output) = command(&drain, || Err(JournalError::Unavailable),
            &mut || Duration::ZERO);
        assert!(result.unwrap_err().contains("Unavailable"));
        assert!(output.is_empty());
        assert!(!drained_anchor.exists());
        assert!(!drained_source.exists());
        assert_eq!(files(&case.config), stopped);
        assert_eq!(observed(&case.config, &case.loaded), admitted);

        // Old source accounting cannot name the stopped anchor, nor the larger
        // canonical extent even if paired with its own correctly framed anchor.
        for anchor in [&admitted_anchor, &original_anchor] {
            let stale = args(&case.config_path, &case.recipe, &case.profile,
                "--drain-stopped", anchor, None);
            let (result, output) = command(&stale,
                || panic!("stale source custody must refuse before clock acquisition"),
                &mut || Duration::ZERO);
            assert!(result.is_err());
            assert!(output.is_empty());
            assert_eq!(files(&case.config), stopped);
        }

        let calls = Cell::new(0);
        let (result, output) = command(&drain, || {
            calls.set(calls.get() + 1);
            Ok(ElapsedTick(1001))
        }, &mut || Duration::ZERO);
        result.unwrap();
        assert_eq!(calls.get(), 1);
        assert!(output.contains("\"status\":\"stopped_drained\""));
        let drained = observed(&case.config, &case.loaded);
        assert_eq!(drained.revision, admitted.revision + 1);
        assert_eq!(drained.executions, 0);
        assert_eq!(drained.control.ledger.charged, 0);
        let saved = paired(&case, &drained_source, &drained_anchor, &drained);
        let drained_profile = selection(&case, &saved, &drained_source, "drained-profile.json");
        let complete = files(&case.config);
        for (index, phase) in ["--stop-admission", "--drain-stopped", "--recover-stop"].iter().enumerate() {
            let anchor = case.root.0.join(format!("retry-{index}.anchor"));
            let source = case.root.0.join(format!("retry-{index}.source"));
            let retry = retaining(args(&case.config_path, &case.recipe, &drained_profile,
                phase, &drained_anchor, Some(&anchor)), &source);
            let (result, output) = command(&retry,
                || panic!("completed terminal checkpoint retry must not acquire time"),
                &mut || Duration::ZERO);
            result.unwrap();
            assert!(output.contains(if *phase == "--stop-admission" {
                "\"appended_events\":0"
            } else { "\"recovery\":\"already_drained\"" }));
            assert_eq!(files(&case.config), complete);
            paired(&case, &source, &anchor, &drained);
            assert_eq!(fs::read(source).unwrap(), fs::read(&drained_source).unwrap());
        }
        assert_eq!(fs::read(original_anchor).unwrap(), original_archive);
        assert_eq!(fs::read(original_source).unwrap(), original_bytes);
        assert!(!case.root.0.join("live-1.json").exists());
        assert!(!case.root.0.join("inventory-1.json").exists());
        assert!(!case.root.0.join("missing-next.json").exists());
        assert!(!case.root.0.join("native-roster.json").exists());
    }
}

#[test]
fn learned_terminal_checkpoint_refuses_unpaired_manual_or_unsafe_outputs_before_stop() {
    let case = dispatched(None);
    let anchor = case.root.0.join("history-1.bin");
    let output = case.root.0.join("retained.anchor");
    let source = case.root.0.join("retained.source");
    let original = files(&case.config);
    let base = args(&case.config_path, &case.recipe, &case.profile,
        "--stop-admission", &anchor, Some(&output));
    let missing_pair = retaining(args(&case.config_path, &case.recipe, &case.profile,
        "--stop-admission", &anchor, None), &source);
    let mut missing_topology = base.clone();
    let at = missing_topology.iter().position(|arg| arg == "--topology-profile").unwrap();
    missing_topology.drain(at..at + 2);
    let mut missing_path = base.clone();
    missing_path.push("--retain-topology-checkpoint".into());
    let duplicate = retaining(retaining(base.clone(), &source), &case.root.0.join("other.source"));
    let occupied = case.root.0.join("occupied.source");
    fs::write(&occupied, b"independent previous custody").unwrap();
    let alias = case.root.0.join("alias");
    std::os::unix::fs::symlink(&case.root.0, &alias).unwrap();
    let unsafe_commands = [
        missing_pair,
        retaining(missing_topology, &source),
        missing_path,
        duplicate,
        retaining(base.clone(), &occupied),
        retaining(base.clone(), &case.config.store.join("source.bin")),
        retaining(base.clone(), &output),
        retaining(base.clone(), &alias.join("retained.anchor")),
        retaining(base.clone(), Path::new("relative-source.bin")),
    ];
    for command_args in unsafe_commands {
        let (result, report) = command(&command_args,
            || panic!("output admission failure cannot reach Stop or logical time"),
            &mut || Duration::ZERO);
        assert!(result.is_err(), "{command_args:?}");
        assert!(report.is_empty());
        assert_eq!(files(&case.config), original);
        assert!(!output.exists());
        assert!(!source.exists());
    }
    assert_eq!(fs::read(&occupied).unwrap(), b"independent previous custody");

    // The live continuation field is not reinterpreted by the terminal flag.
    let live_profile = case.root.0.join("live-retention-profile.json");
    let original_profile = fs::read_to_string(&case.profile).unwrap();
    fs::write(&live_profile, format!(r#"{},"retain_checkpoint":"{}"}}"#,
        original_profile.trim_end_matches('}'), case.root.0.join("live.source").display())).unwrap();
    let live_export = retaining(args(&case.config_path, &case.recipe, &live_profile,
        "--stop-admission", &anchor, Some(&output)), &source);
    let (result, report) = command(&live_export,
        || panic!("live output selection cannot replace terminal custody"),
        &mut || Duration::ZERO);
    assert!(result.is_err());
    assert!(report.is_empty());
    assert_eq!(files(&case.config), original);
    assert!(!case.root.0.join("live.source").exists());

    // Independently specified v1 work remains a recovery minimum; it cannot be
    // promoted to exact source expenditure by requesting a v2 output archive.
    fs::write(case.root.0.join("manual-current.json"), case.saved.topology_bytes()).unwrap();
    fs::write(case.root.0.join("manual-inventory.json"), case.saved.inventory_bytes()).unwrap();
    let initial = pair(&case.root, "initial.json", "initial-inventory.json");
    let current = pair(&case.root, "manual-current.json", "manual-inventory.json");
    let next = pair(&case.root, "missing-next.json", "missing-next-inventory.json");
    let manual = case.root.0.join("manual-profile.json");
    fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{initial},"current":{current},"next":{next},"available":true,"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
        case.saved.last_operation() + 1, case.saved.work().checks,
        case.saved.work().reserved_read_bytes, case.saved.last_operation())).unwrap();
    let manual_export = retaining(args(&case.config_path, &case.recipe, &manual,
        "--stop-admission", &anchor, Some(&output)), &source);
    let (result, report) = command(&manual_export,
        || panic!("manual source minimum cannot acquire exact output custody"),
        &mut || Duration::ZERO);
    assert!(result.is_err());
    assert!(report.is_empty());
    assert_eq!(files(&case.config), original);
    assert!(!output.exists());
    assert!(!source.exists());

    // The near-identical retained-v2 command succeeds without changing any guard.
    let (result, report) = command(&retaining(base, &source),
        || panic!("valid admission checkpoint remains clock independent"),
        &mut || Duration::ZERO);
    result.unwrap();
    assert!(report.contains("\"drain_attempted\":false"));
    let state = observed(&case.config, &case.loaded);
    paired(&case, &source, &output, &state);
}

#[test]
fn learned_terminal_checkpoint_export_races_preserve_native_receipt_and_each_independent_archive() {
    for (occupy_history, occupy_source) in [(true, false), (false, true), (true, true)] {
        let case = dispatched(None);
        let anchor = case.root.0.join("history-1.bin");
        let output = case.root.0.join("retained.anchor");
        let source = case.root.0.join("retained.source");
        let combined = retaining(args(&case.config_path, &case.recipe, &case.profile,
            "--recover-stop", &anchor, Some(&output)), &source);
        let (result, report) = command(&combined, || {
            if occupy_history { fs::write(&output, b"racing history custodian").unwrap(); }
            if occupy_source { fs::write(&source, b"racing source custodian").unwrap(); }
            Ok(ElapsedTick(1001))
        }, &mut || Duration::ZERO);
        let error = result.unwrap_err();
        assert!(report.contains("\"status\":\"stopped_drained\""));
        assert!(error.contains("acknowledged"));
        if occupy_history { assert!(error.contains("history anchor retention failed")); }
        if occupy_source { assert!(error.contains("topology checkpoint retention failed")); }
        let state = observed(&case.config, &case.loaded);
        assert_eq!(state.revision, case.before.revision + 3);
        assert_eq!(state.executions, 0);
        assert_eq!(state.control.ledger.charged, 0);
        assert_eq!(state.stop.as_ref().unwrap().request().operation, 801);
        if occupy_history {
            assert_eq!(fs::read(&output).unwrap(), b"racing history custodian");
        } else {
            assert_eq!(anchors::load(&output).unwrap().revision(), state.revision);
        }
        if occupy_source {
            assert_eq!(fs::read(&source).unwrap(), b"racing source custodian");
        } else {
            let saved = read_saved(&source);
            assert_eq!(saved.minimum().journal_revision, state.revision);
            assert_eq!(saved.work(), case.saved.work());
            assert!(!saved.available());
        }
        assert_eq!(read_saved(&case.root.0.join("source-1.bin")), case.saved);
    }
}

#[test]
fn learned_terminal_checkpoint_report_failure_keeps_both_acknowledged_archives() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "report unavailable"))
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let case = dispatched(Some(true));
    let anchor = case.root.0.join("history-1.bin");
    let output = case.root.0.join("retained.anchor");
    let source = case.root.0.join("retained.source");
    let combined = retaining(args(&case.config_path, &case.recipe, &case.profile,
        "--recover-stop", &anchor, Some(&output)), &source);
    let (args, options) = anchors::take_options(&combined).unwrap();
    assert!(terminal::command_with(&args, options, || Ok(ElapsedTick(1001)),
        &mut || Duration::ZERO, &mut Broken).is_err());
    let state = observed(&case.config, &case.loaded);
    assert_eq!(state.revision, case.before.revision + 3);
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 0);
    paired(&case, &source, &output, &state);
}
