//! Real retained source custody across the original receipt-only settlement.
//! No helper, live source refresh, new sampled token or effect retry is admitted.
use super::*;
use crate::workflow::actor_service::learned::recovery::readonly as query;
use fa_reference::action::consequence::oversight::actor::UnknownReason;
use std::{
    cell::Cell,
    io::{self, Write},
};

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

fn dispatched(mode: Option<bool>, published: bool) -> Case {
    let root = Root::new();
    let (mut config, recipe, loaded) = selected(&root, mode, false);
    let deadline = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    let custody::Started {
        custody: (host, reviewer, observer, _identity),
        source,
        ..
    } = custody::start(
        &config,
        &loaded,
        false,
        &mut control,
        &deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let mut prediction = observer.map(|observer| prediction::Session::new(1, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    custody::attach(&mut driver, source).unwrap();
    custody::certify(&mut driver, &deadline, &mut || ElapsedTick(1000)).unwrap();
    assert!(
        generate_selected(
            &mut driver,
            &reviewer,
            &mut config,
            &mut control,
            &deadline,
            &mut || ElapsedTick(1000),
            prediction.as_mut()
        )
        .unwrap()
    );
    submit(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        &deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let (mut driver, review) =
        native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    assert!(matches!(
        driver
            .step_computed_from_policy_file(&mut config.source, || ElapsedTick(1000), Some(&human))
            .result
            .unwrap(),
        FileDriverEvent::Dispatched { .. }
    ));
    if published {
        assert!(matches!(
            driver
                .step_computed_from_policy_file(&mut config.source, || ElapsedTick(1000), None)
                .result
                .unwrap(),
            FileDriverEvent::PublicationChecked { .. }
        ));
    }
    assert!(matches!(
        response(&mut wire, 1).result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        })
    ));
    let host = driver.supervisor().host().unwrap();
    let before = host.inspect();
    assert_eq!(before.executions, u64::from(published));
    assert!(before.control.ledger.charged > 0);
    let saved = driver
        .topology()
        .unwrap()
        .checkpoint(
            &host,
            &fs::read(root.0.join("live-1.json")).unwrap(),
            &fs::read(root.0.join("inventory-1.json")).unwrap(),
        )
        .unwrap();
    assert!(saved.available());
    assert!(saved.work().checks > 0);
    anchors::export_acknowledged(saved.history_anchor(), &root.0.join("history-1.bin")).unwrap();
    let mut bytes = Vec::new();
    saved
        .write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES)
        .unwrap();
    fs::write(root.0.join("source-1.bin"), bytes).unwrap();
    drop(host);
    drop(review);
    drop(wire);
    drop(prediction);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let profile = archived_selection(&root, &saved, 1, None);
    let loaded = recipe::load(&recipe, &config, false).unwrap();
    assert_eq!(observed(&config, &loaded), before);
    let config_path = root.0.join("receipt-config.json");
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    ))
    .replace(
        "/operator/publication-store",
        config.store.to_str().unwrap(),
    )
    .replace(
        "/operator/evidence.json",
        root.0.join("evidence.json").to_str().unwrap(),
    )
    .replace(
        "\"initial_payload_hex\": \"696e697469616c\"",
        "\"initial_payload_hex\": \"\"",
    )
    .replace("\"events\": 256", "\"events\": 1024")
    .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 60000");
    fs::write(&config_path, text).unwrap();
    crate::config::assert_oversight_profiles_eq(
        &Config::read(&config_path).unwrap().profile,
        &config.profile,
    );
    for name in [
        "live-1.json",
        "inventory-1.json",
        "evidence.json",
        "native-roster.json",
        "helper-model.json",
        "helper-weights.safetensors",
        "helper-monitor.json",
        "helper-tokenizer.bbpe",
        "helper-sampling.json",
    ] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::write(
        config.store.join("delivery.pending"),
        b"receipt custody staging",
    )
    .unwrap();
    Case {
        root,
        config,
        config_path,
        recipe,
        loaded,
        saved,
        before,
        profile,
    }
}

fn args(case: &Case, profile: &Path, input: &Path, output: &Path, source: &Path) -> Vec<String> {
    vec![
        "create-learned-generated".into(),
        case.config_path.display().to_string(),
        case.recipe.display().to_string(),
        "--resume".into(),
        "--reconcile-receipts".into(),
        "--history-anchor".into(),
        input.display().to_string(),
        "--retain-history-anchor".into(),
        output.display().to_string(),
        "--topology-profile".into(),
        profile.display().to_string(),
        "--retain-topology-checkpoint".into(),
        source.display().to_string(),
    ]
}

fn readonly_args(case: &Case, profile: &Path, anchor: &Path) -> Vec<String> {
    vec![
        "create-learned-generated".into(),
        case.config_path.display().to_string(),
        case.recipe.display().to_string(),
        "--resume".into(),
        "--receipts-only".into(),
        "--history-anchor".into(),
        anchor.display().to_string(),
        "--topology-profile".into(),
        profile.display().to_string(),
    ]
}

fn source_file(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

fn pair_at(case: &Case, source: &Path, anchor: &Path, revision: u64) -> TopologyCheckpoint {
    let saved = source_file(source);
    assert_eq!(saved.history_anchor(), &anchors::load(anchor).unwrap());
    assert_eq!(saved.minimum().journal_revision, revision);
    assert_eq!(saved.history_anchor().revision(), revision);
    assert!(!saved.available());
    assert_eq!(saved.work(), case.saved.work());
    assert_eq!(saved.last_operation(), case.saved.last_operation());
    assert_eq!(saved.topology_bytes(), case.saved.topology_bytes());
    assert_eq!(saved.inventory_bytes(), case.saved.inventory_bytes());
    assert_eq!(saved.expected(), case.saved.expected());
    assert!(saved.minimum().control_sequence >= case.saved.minimum().control_sequence);
    assert!(saved.minimum().authority_epoch >= case.saved.minimum().authority_epoch);
    saved
}

fn profile_for(case: &Case, saved: &TopologyCheckpoint, source: &Path, name: &str) -> PathBuf {
    let path = case.root.0.join(name);
    let initial = pair(&case.root, "initial.json", "initial-inventory.json");
    let next = pair(
        &case.root,
        "missing-next.json",
        "missing-next-inventory.json",
    );
    fs::write(&path, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{initial},"next":{next},"operation":{},"source_checkpoint":"{}"}}"#,
        saved.last_operation() + 1, source.display())).unwrap();
    path
}

fn assert_no_live_inputs(case: &Case) {
    for name in [
        "live-1.json",
        "inventory-1.json",
        "missing-next.json",
        "missing-next-inventory.json",
        "native-roster.json",
        "evidence.json",
    ] {
        assert!(!case.root.0.join(name).exists(), "{name}");
    }
}

#[test]
fn retained_topology_receipts_support_subsequent_exact_recovery_in_every_forecast_mode() {
    for mode in [None, Some(false), Some(true)] {
        for published in [false, true] {
            let case = dispatched(mode, published);
            let input = case.root.0.join("history-1.bin");
            let original_history = fs::read(&input).unwrap();
            let original_source = fs::read(case.root.0.join("source-1.bin")).unwrap();
            let output = case.root.0.join("settled.anchor");
            let source = case.root.0.join("settled.source");
            let calls = Cell::new(0);
            let mut bytes = Vec::new();
            let result = query::command_reconciled_with_elapsed(
                &args(&case, &case.profile, &input, &output, &source),
                None,
                &mut bytes,
                || {
                    calls.set(calls.get() + 1);
                    ElapsedTick(1001)
                },
                &mut || Duration::ZERO,
            );
            assert_eq!(result.is_ok(), published, "{result:?}");
            assert_eq!(calls.get(), 1);
            let state = observed(&case.config, &case.loaded);
            assert_eq!(state.revision, case.before.revision + 3);
            assert_eq!(state.executions, u64::from(published));
            assert_eq!(state.payload, case.before.payload);
            assert_eq!(
                state.control.ledger.charged,
                case.before.control.ledger.charged
            );
            assert_eq!(
                state.control.ledger.available,
                case.before.control.ledger.available
            );
            let saved = pair_at(&case, &source, &output, state.revision);
            let profile = profile_for(&case, &saved, &source, "settled-profile.json");
            let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
            let mut read = Vec::new();
            assert_eq!(
                query::command_with_elapsed(
                    &readonly_args(&case, &profile, &output),
                    None,
                    &mut read,
                    &mut || Duration::ZERO
                )
                .is_ok(),
                published
            );
            assert_eq!(read, bytes);
            assert_eq!(
                fs::read(case.config.store.join("delivery.bin")).unwrap(),
                canonical
            );
            // The original exact-extent guard still rejects the earlier pair.
            let mut stale = Vec::new();
            assert!(
                query::command_with_elapsed(
                    &readonly_args(&case, &case.profile, &input),
                    None,
                    &mut stale,
                    &mut || Duration::ZERO
                )
                .is_err()
            );
            assert!(stale.is_empty());
            assert_eq!(
                fs::read(case.config.store.join("delivery.bin")).unwrap(),
                canonical
            );
            // A correctly retained successor pair permits the next original
            // receipt transaction without restoring source work or resending.
            let next_output = case.root.0.join("settled-again.anchor");
            let next_source = case.root.0.join("settled-again.source");
            let mut repeated = Vec::new();
            assert_eq!(
                query::command_reconciled_with_elapsed(
                    &args(&case, &profile, &output, &next_output, &next_source),
                    None,
                    &mut repeated,
                    || ElapsedTick(1002),
                    &mut || Duration::ZERO
                )
                .is_ok(),
                published
            );
            assert_eq!(repeated, bytes);
            let next = observed(&case.config, &case.loaded);
            assert_eq!(next.revision, state.revision + 3);
            assert_eq!(next.executions, state.executions);
            assert_eq!(next.control.ledger.charged, state.control.ledger.charged);
            pair_at(&case, &next_source, &next_output, next.revision);
            assert_eq!(fs::read(input).unwrap(), original_history);
            assert_eq!(
                fs::read(case.root.0.join("source-1.bin")).unwrap(),
                original_source
            );
            assert_no_live_inputs(&case);
        }
    }
}

fn remove_option(mut args: Vec<String>, flag: &str) -> Vec<String> {
    let index = args.iter().position(|arg| arg == flag).unwrap();
    args.drain(index..index + 2);
    args
}
fn set_option(mut args: Vec<String>, flag: &str, value: &Path) -> Vec<String> {
    let index = args.iter().position(|arg| arg == flag).unwrap();
    args[index + 1] = value.display().to_string();
    args
}

#[test]
fn retained_topology_receipts_refuse_unpaired_mixed_manual_and_unsafe_custody_before_clock() {
    let case = dispatched(None, true);
    let input = case.root.0.join("history-1.bin");
    let output = case.root.0.join("retained.anchor");
    let source = case.root.0.join("retained.source");
    let base = args(&case, &case.profile, &input, &output, &source);
    let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let pending = fs::read(case.config.store.join("delivery.pending")).unwrap();
    let occupied = case.root.0.join("occupied");
    fs::write(&occupied, b"independently retained evidence").unwrap();
    let alias = case.root.0.join("alias");
    std::os::unix::fs::symlink(&case.root.0, &alias).unwrap();
    let mut invalid = vec![
        remove_option(base.clone(), "--history-anchor"),
        remove_option(base.clone(), "--retain-history-anchor"),
        remove_option(base.clone(), "--topology-profile"),
        remove_option(base.clone(), "--retain-topology-checkpoint"),
        set_option(base.clone(), "--retain-topology-checkpoint", &output),
        set_option(
            base.clone(),
            "--retain-topology-checkpoint",
            &alias.join("retained.anchor"),
        ),
        set_option(
            base.clone(),
            "--retain-topology-checkpoint",
            &case.config.store.join("source.bin"),
        ),
        set_option(
            base.clone(),
            "--retain-history-anchor",
            &case.config.store.join("history.bin"),
        ),
        set_option(base.clone(), "--retain-topology-checkpoint", &occupied),
        set_option(base.clone(), "--retain-history-anchor", &occupied),
        set_option(
            base.clone(),
            "--retain-topology-checkpoint",
            Path::new("relative.bin"),
        ),
    ];
    let mut duplicate = base.clone();
    duplicate.extend([
        "--retain-topology-checkpoint".into(),
        case.root.0.join("other.source").display().to_string(),
    ]);
    invalid.push(duplicate);
    let mut wrong_kind = base.clone();
    wrong_kind.extend(["--finish-after-message".into(), "9".into()]);
    invalid.push(wrong_kind);
    let foreign = dispatched(None, true);
    invalid.push(set_option(
        base.clone(),
        "--history-anchor",
        &foreign.root.0.join("history-1.bin"),
    ));
    let manual = case.root.0.join("manual-topology.json");
    let initial = pair(&case.root, "initial.json", "initial-inventory.json");
    let next = pair(
        &case.root,
        "missing-next.json",
        "missing-next-inventory.json",
    );
    fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{initial},"current":{initial},"next":{next},"available":true,"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
        case.saved.last_operation() + 1, case.saved.work().checks,
        case.saved.work().reserved_read_bytes, case.saved.last_operation())).unwrap();
    invalid.push(set_option(base.clone(), "--topology-profile", &manual));
    for command in invalid {
        let mut bytes = Vec::new();
        assert!(
            query::command_reconciled_with_elapsed(
                &command,
                None,
                &mut bytes,
                || panic!("invalid custody reached the settlement clock"),
                &mut || Duration::ZERO
            )
            .is_err()
        );
        assert!(bytes.is_empty());
        assert_eq!(
            fs::read(case.config.store.join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(
            fs::read(case.config.store.join("delivery.pending")).unwrap(),
            pending
        );
        assert!(!output.exists());
        assert!(!source.exists());
    }
    let readonly: Vec<_> = base
        .iter()
        .map(|arg| {
            if arg == "--reconcile-receipts" {
                "--receipts-only".to_owned()
            } else {
                arg.clone()
            }
        })
        .collect();
    assert!(
        query::command_with_elapsed(&readonly, None, &mut Vec::new(), &mut || panic!(
            "read-only export mixture reached configuration intake"
        ))
        .is_err()
    );
    assert_eq!(
        fs::read(case.config.store.join("delivery.bin")).unwrap(),
        canonical
    );
    assert_eq!(
        fs::read(&occupied).unwrap(),
        b"independently retained evidence"
    );
    query::command_reconciled_with_elapsed(
        &base,
        None,
        &mut Vec::new(),
        || ElapsedTick(1001),
        &mut || Duration::ZERO,
    )
    .unwrap();
    pair_at(&case, &source, &output, case.before.revision + 3);
}

struct Report {
    bytes: Vec<u8>,
    broken: bool,
}
impl Write for Report {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.broken {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn retained_topology_receipts_attempt_both_archives_before_late_failures() {
    for failure in [
        "history",
        "source",
        "both",
        "deadline",
        "stdout",
        "source-and-stdout",
    ] {
        let case = dispatched(Some(true), true);
        let input = case.root.0.join("history-1.bin");
        let old_source = fs::read(case.root.0.join("source-1.bin")).unwrap();
        let old_history = fs::read(&input).unwrap();
        let output = case.root.0.join("retained.anchor");
        let source = case.root.0.join("retained.source");
        let history_fails = matches!(failure, "history" | "both");
        let source_fails = matches!(failure, "source" | "both" | "source-and-stdout");
        let called = Cell::new(false);
        let mut report = Report {
            bytes: Vec::new(),
            broken: matches!(failure, "stdout" | "source-and-stdout"),
        };
        let error = query::command_reconciled_with_elapsed(
            &args(&case, &case.profile, &input, &output, &source),
            None,
            &mut report,
            || {
                assert!(!called.replace(true));
                if history_fails {
                    fs::write(&output, b"raced history output").unwrap();
                }
                if source_fails {
                    fs::write(&source, b"raced source output").unwrap();
                }
                ElapsedTick(1001)
            },
            &mut || {
                if failure == "deadline" && called.get() {
                    Duration::from_secs(3600)
                } else {
                    Duration::ZERO
                }
            },
        )
        .unwrap_err();
        assert!(called.get());
        assert!(error.contains("reconciliation acknowledged"), "{error}");
        if history_fails {
            assert!(error.contains("history anchor retention failed"), "{error}");
            assert_eq!(fs::read(&output).unwrap(), b"raced history output");
        } else {
            assert_eq!(
                anchors::load(&output).unwrap().revision(),
                case.before.revision + 3
            );
        }
        if source_fails {
            assert!(
                error.contains("topology checkpoint retention failed"),
                "{error}"
            );
            assert_eq!(fs::read(&source).unwrap(), b"raced source output");
        } else {
            let saved = source_file(&source);
            assert_eq!(saved.history_anchor().revision(), case.before.revision + 3);
            assert_eq!(saved.work(), case.saved.work());
            assert_eq!(saved.last_operation(), case.saved.last_operation());
        }
        if failure == "deadline" {
            assert!(error.contains("deadline expired"));
            assert!(report.bytes.is_empty());
        } else if report.broken {
            assert!(error.contains("original response output failed"));
            assert!(report.bytes.is_empty());
        } else {
            assert!(
                !report.bytes.is_empty(),
                "archive failure must still report original outcome"
            );
        }
        let state = observed(&case.config, &case.loaded);
        assert_eq!(state.revision, case.before.revision + 3);
        assert_eq!(state.executions, 1);
        assert_eq!(
            state.control.ledger.charged,
            case.before.control.ledger.charged
        );
        assert_eq!(state.payload, case.before.payload);
        assert_eq!(fs::read(&input).unwrap(), old_history);
        assert_eq!(
            fs::read(case.root.0.join("source-1.bin")).unwrap(),
            old_source
        );
        if !history_fails && !source_fails {
            pair_at(&case, &source, &output, state.revision);
        }
        assert_no_live_inputs(&case);
    }
}

#[path = "receipts/repair.rs"]
mod repair;
