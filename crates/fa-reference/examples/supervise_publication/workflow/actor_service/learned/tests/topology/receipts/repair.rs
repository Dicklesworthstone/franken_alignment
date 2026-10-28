//! Recover only the original source cut and immutable selected receipt after
//! independent exports fail. Historical Sweep does not name its trigger request.
use super::*;

const REPAIR: &str = "--recover-receipt-checkpoint";

fn repair_args(
    case: &Case,
    profile: &Path,
    input: &Path,
    output: &Path,
    source: &Path,
) -> Vec<String> {
    let mut command = args(case, profile, input, output, source);
    command.push(REPAIR.into());
    command
}

fn canonical(case: &Case) -> Vec<u8> {
    fs::read(case.config.store.join("delivery.bin")).unwrap()
}

fn assert_effects(case: &Case, current: &FileDeliverySnapshot) {
    assert_eq!(current.executions, case.before.executions);
    assert_eq!(current.payload, case.before.payload);
    assert_eq!(
        current.control.ledger.charged,
        case.before.control.ledger.charged
    );
    assert_eq!(
        current.control.ledger.available,
        case.before.control.ledger.available
    );
}

#[test]
fn lost_receipt_exports_recover_from_original_pair_in_every_forecast_mode_without_another_sweep() {
    for mode in [None, Some(false), Some(true)] {
        for failure in ["history", "source", "both"] {
            let published = failure != "both";
            let case = dispatched(mode, published);
            let input = case.root.0.join("history-1.bin");
            let old_history = fs::read(&input).unwrap();
            let old_source = fs::read(case.root.0.join("source-1.bin")).unwrap();
            let lost_history = case.root.0.join("lost.anchor");
            let lost_source = case.root.0.join("lost.source");
            let calls = Cell::new(0);
            let mut original_response = Vec::new();
            let error = query::command_reconciled_with_elapsed(
                &args(&case, &case.profile, &input, &lost_history, &lost_source),
                None,
                &mut original_response,
                || {
                    calls.set(calls.get() + 1);
                    if failure != "source" {
                        fs::write(&lost_history, b"raced history").unwrap();
                    }
                    if failure != "history" {
                        fs::write(&lost_source, b"raced source").unwrap();
                    }
                    ElapsedTick(1001)
                },
                &mut || Duration::ZERO,
            )
            .unwrap_err();
            assert!(error.contains("reconciliation acknowledged"), "{error}");
            assert_eq!(calls.get(), 1);
            assert!(!original_response.is_empty());
            let after = observed(&case.config, &case.loaded);
            assert_eq!(after.revision, case.before.revision + 3);
            assert_effects(&case, &after);
            let journal = canonical(&case);
            let lost_history_bytes = fs::read(&lost_history).unwrap();
            let lost_source_bytes = fs::read(&lost_source).unwrap();
            // Both ordinary historical consumers retain their exact-extent law.
            let mut stale = Vec::new();
            assert!(
                query::command_with_elapsed(
                    &readonly_args(&case, &case.profile, &input),
                    None,
                    &mut stale,
                    &mut || Duration::ZERO,
                )
                .is_err()
            );
            assert!(stale.is_empty());
            assert!(
                query::command_reconciled_with_elapsed(
                    &args(
                        &case,
                        &case.profile,
                        &input,
                        &case.root.0.join("strict.anchor"),
                        &case.root.0.join("strict.source")
                    ),
                    None,
                    &mut stale,
                    || panic!("ordinary stale custody cannot reach a clock"),
                    &mut || Duration::ZERO,
                )
                .is_err()
            );
            assert!(stale.is_empty());
            for attempt in [1, 2] {
                let output = case.root.0.join(format!("recovered-{attempt}.anchor"));
                let source = case.root.0.join(format!("recovered-{attempt}.source"));
                fs::write(
                    case.config.store.join("delivery.pending"),
                    b"confirmed cleanup",
                )
                .unwrap();
                let mut recovered = Vec::new();
                let result = query::command_reconciled_with_elapsed(
                    &repair_args(&case, &case.profile, &input, &output, &source),
                    None,
                    &mut recovered,
                    || panic!("visible Fence/Time/Sweep must never read a new clock"),
                    &mut || Duration::ZERO,
                );
                assert_eq!(result.is_ok(), published, "{result:?}");
                assert_eq!(recovered, original_response);
                assert_eq!(canonical(&case), journal);
                assert_eq!(observed(&case.config, &case.loaded), after);
                assert!(!case.config.store.join("delivery.pending").exists());
                let saved = pair_at(&case, &source, &output, after.revision);
                let profile =
                    profile_for(&case, &saved, &source, &format!("recovered-{attempt}.json"));
                let mut passive = Vec::new();
                assert_eq!(
                    query::command_with_elapsed(
                        &readonly_args(&case, &profile, &output),
                        None,
                        &mut passive,
                        &mut || Duration::ZERO,
                    )
                    .is_ok(),
                    published
                );
                assert_eq!(passive, recovered);
            }
            assert_eq!(fs::read(&input).unwrap(), old_history);
            assert_eq!(
                fs::read(case.root.0.join("source-1.bin")).unwrap(),
                old_source
            );
            assert_eq!(fs::read(lost_history).unwrap(), lost_history_bytes);
            assert_eq!(fs::read(lost_source).unwrap(), lost_source_bytes);
            assert_no_live_inputs(&case);
        }
    }
}

#[test]
fn unchanged_receipt_cut_executes_original_transaction_once_and_successor_cut_is_independent() {
    for mode in [None, Some(false), Some(true)] {
        let case = dispatched(mode, true);
        let mut input = case.root.0.join("history-1.bin");
        let mut profile = case.profile.clone();
        let mut expected = case.before.revision;
        let mut first_response = None;
        for attempt in [1, 2] {
            let output = case.root.0.join(format!("current-{attempt}.anchor"));
            let source = case.root.0.join(format!("current-{attempt}.source"));
            let calls = Cell::new(0);
            let mut response = Vec::new();
            query::command_reconciled_with_elapsed(
                &repair_args(&case, &profile, &input, &output, &source),
                None,
                &mut response,
                || {
                    calls.set(calls.get() + 1);
                    ElapsedTick(1000 + attempt)
                },
                &mut || Duration::ZERO,
            )
            .unwrap();
            assert_eq!(calls.get(), 1);
            if let Some(first) = &first_response {
                assert_eq!(&response, first);
            } else {
                first_response = Some(response);
            }
            expected += 3;
            let state = observed(&case.config, &case.loaded);
            assert_eq!(state.revision, expected);
            assert_effects(&case, &state);
            let saved = pair_at(&case, &source, &output, expected);
            profile = profile_for(&case, &saved, &source, &format!("current-{attempt}.json"));
            input = output;
        }
        assert_no_live_inputs(&case);
    }
}

#[test]
fn repair_option_never_fills_a_missing_path_or_finish_value_and_rejects_mixed_modes_before_config()
{
    let case = dispatched(None, true);
    let input = case.root.0.join("history-1.bin");
    let output = case.root.0.join("grammar.anchor");
    let source = case.root.0.join("grammar.source");
    let base = repair_args(&case, &case.profile, &input, &output, &source);
    let journal = canonical(&case);
    let pending = fs::read(case.config.store.join("delivery.pending")).unwrap();
    let mut invalid = Vec::new();
    for flag in [
        "--history-anchor",
        "--retain-history-anchor",
        "--topology-profile",
        "--retain-topology-checkpoint",
    ] {
        invalid.push(remove_option(base.clone(), flag));
        let mut value_slot = base.clone();
        value_slot.pop();
        let at = value_slot.iter().position(|arg| arg == flag).unwrap() + 1;
        value_slot.insert(at, REPAIR.into());
        invalid.push(value_slot);
    }
    // Stripping a different path-valued option must not repair the original
    // missing history or finish value either.
    let mut path_as_value = remove_option(base.clone(), "--retain-topology-checkpoint");
    let at = path_as_value
        .iter()
        .position(|arg| arg == "--history-anchor")
        .unwrap()
        + 1;
    path_as_value.splice(
        at..at,
        [
            "--retain-topology-checkpoint".into(),
            source.display().to_string(),
        ],
    );
    invalid.push(path_as_value);
    let mut finish_as_value = remove_option(base.clone(), "--retain-topology-checkpoint");
    finish_as_value.extend([
        "--finish-after-message".into(),
        "--retain-topology-checkpoint".into(),
        source.display().to_string(),
        "71".into(),
    ]);
    invalid.push(finish_as_value);
    let mut duplicate = base.clone();
    duplicate.push(REPAIR.into());
    invalid.push(duplicate);
    let mut early = base.clone();
    early.pop();
    early.insert(3, REPAIR.into());
    invalid.push(early);
    let mut create = base.clone();
    create[3] = "--create".into();
    invalid.push(create);
    let mut passive = base.clone();
    passive[4] = "--receipts-only".into();
    invalid.push(passive.clone());
    let mut mixed = base.clone();
    mixed.push("--receipts-only".into());
    invalid.push(mixed);
    let mut finish = base.clone();
    finish.pop();
    finish.extend(["--finish-after-message".into(), REPAIR.into(), "1".into()]);
    invalid.push(finish);
    let mut mode_as_value = base.clone();
    mode_as_value.remove(4);
    mode_as_value.extend([
        "--finish-after-message".into(),
        "--reconcile-receipts".into(),
        "1".into(),
    ]);
    invalid.push(mode_as_value);
    for flag in [
        "--continue-generation",
        "--recover-stop",
        "--unrecognized-recovery",
    ] {
        let mut mixed = base.clone();
        mixed.push(flag.into());
        invalid.push(mixed);
    }
    for command in invalid {
        assert!(
            query::command_reconciled_with_elapsed(
                &command,
                None,
                &mut Vec::new(),
                || panic!("malformed repair read a clock"),
                &mut || panic!("malformed repair reached configuration intake"),
            )
            .is_err(),
            "{command:?}"
        );
    }
    assert!(
        query::command_with_elapsed(&passive, None, &mut Vec::new(), &mut || panic!(
            "passive repair mixture reached configuration intake"
        ))
        .is_err()
    );
    assert_eq!(canonical(&case), journal);
    assert_eq!(
        fs::read(case.config.store.join("delivery.pending")).unwrap(),
        pending
    );
    assert!(!output.exists());
    assert!(!source.exists());
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

#[test]
fn repair_preserves_independent_custody_selection_and_all_numeric_floors_before_cleanup() {
    let case = dispatched(None, true);
    let input = case.root.0.join("history-1.bin");
    let output = case.root.0.join("guarded.anchor");
    let source = case.root.0.join("guarded.source");
    let base = repair_args(&case, &case.profile, &input, &output, &source);
    let journal = canonical(&case);
    let pending = fs::read(case.config.store.join("delivery.pending")).unwrap();
    let occupied = case.root.0.join("occupied.source");
    fs::write(&occupied, b"independent custody").unwrap();
    let alias = case.root.0.join("alias");
    std::os::unix::fs::symlink(&case.root.0, &alias).unwrap();
    let foreign = dispatched(None, true);
    let manual = case.root.0.join("manual.json");
    let initial = pair(&case.root, "initial.json", "initial-inventory.json");
    let next = pair(
        &case.root,
        "missing-next.json",
        "missing-next-inventory.json",
    );
    fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{initial},"current":{initial},"next":{next},"available":true,"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
        case.saved.last_operation() + 1, case.saved.work().checks,
        case.saved.work().reserved_read_bytes, case.saved.last_operation())).unwrap();
    let mut invalid = vec![
        set_option(base.clone(), "--retain-topology-checkpoint", &output),
        set_option(
            base.clone(),
            "--retain-topology-checkpoint",
            &alias.join("guarded.anchor"),
        ),
        set_option(base.clone(), "--retain-topology-checkpoint", &occupied),
        set_option(base.clone(), "--retain-history-anchor", &occupied),
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
        set_option(
            base.clone(),
            "--retain-topology-checkpoint",
            Path::new("relative.source"),
        ),
        set_option(
            base.clone(),
            "--history-anchor",
            &foreign.root.0.join("history-1.bin"),
        ),
        set_option(base.clone(), "--topology-profile", &manual),
    ];
    let mut wrong_finish = base.clone();
    wrong_finish.extend(["--finish-after-message".into(), "9".into()]);
    invalid.push(wrong_finish);
    let refused = |command: &[String]| {
        let mut bytes = Vec::new();
        assert!(
            query::command_reconciled_with_elapsed(
                command,
                None,
                &mut bytes,
                || panic!("invalid repair selection read a clock"),
                &mut || Duration::ZERO
            )
            .is_err()
        );
        assert!(bytes.is_empty());
        assert_eq!(canonical(&case), journal);
        assert_eq!(
            fs::read(case.config.store.join("delivery.pending")).unwrap(),
            pending
        );
        assert!(!output.exists());
        assert!(!source.exists());
    };
    for command in invalid {
        refused(&command);
    }
    let original_recipe = fs::read_to_string(&case.recipe).unwrap();
    for floor in ["journal_revision", "control_sequence", "authority_epoch"] {
        let old = format!("\"{floor}\":0");
        assert!(original_recipe.contains(&old));
        let new = format!("\"{floor}\":{}", u64::MAX);
        fs::write(&case.recipe, original_recipe.replacen(&old, &new, 1)).unwrap();
        refused(&base);
    }
    fs::write(&case.recipe, original_recipe).unwrap();
    assert_eq!(fs::read(&occupied).unwrap(), b"independent custody");
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

#[test]
fn repair_rejects_partial_or_unrelated_real_suffixes_without_cleanup() {
    for suffix in ["fence", "fence-time", "extra-fence"] {
        let case = dispatched(None, true);
        let input = case.root.0.join("history-1.bin");
        if suffix == "extra-fence" {
            query::command_reconciled_with_elapsed(
                &args(
                    &case,
                    &case.profile,
                    &input,
                    &case.root.0.join("settled.anchor"),
                    &case.root.0.join("settled.source"),
                ),
                None,
                &mut Vec::new(),
                || ElapsedTick(1001),
                &mut || Duration::ZERO,
            )
            .unwrap();
        }
        let (mut host, _) = FileOversight::open_with_learned_generation(
            &case.config.store,
            case.config.profile.clone(),
            &case.loaded.generation,
        )
        .unwrap();
        if suffix == "fence-time" {
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(1001)).unwrap();
        }
        drop(host);
        let journal = canonical(&case);
        fs::write(
            case.config.store.join("delivery.pending"),
            b"unrelated source evidence",
        )
        .unwrap();
        let output = case.root.0.join("invalid.anchor");
        let source = case.root.0.join("invalid.source");
        let mut response = Vec::new();
        assert!(
            query::command_reconciled_with_elapsed(
                &repair_args(&case, &case.profile, &input, &output, &source),
                None,
                &mut response,
                || panic!("unsupported suffix reached a clock"),
                &mut || Duration::ZERO,
            )
            .is_err(),
            "{suffix}"
        );
        assert!(response.is_empty());
        assert_eq!(canonical(&case), journal);
        assert_eq!(
            fs::read(case.config.store.join("delivery.pending")).unwrap(),
            b"unrelated source evidence"
        );
        assert!(!output.exists());
        assert!(!source.exists());
    }
}

#[test]
fn confirmed_repair_attempts_both_exports_before_deadline_or_stdout_failure_and_can_retry() {
    let case = dispatched(Some(true), true);
    let input = case.root.0.join("history-1.bin");
    let mut original_response = Vec::new();
    query::command_reconciled_with_elapsed(
        &args(
            &case,
            &case.profile,
            &input,
            &case.root.0.join("settled.anchor"),
            &case.root.0.join("settled.source"),
        ),
        None,
        &mut original_response,
        || ElapsedTick(1001),
        &mut || Duration::ZERO,
    )
    .unwrap();
    let journal = canonical(&case);
    let state = observed(&case.config, &case.loaded);
    for failure in [
        "history",
        "source",
        "both",
        "deadline",
        "stdout",
        "source-and-stdout",
    ] {
        let output = case.root.0.join(format!("{failure}.anchor"));
        let source = case.root.0.join(format!("{failure}.source"));
        let history_fails = matches!(failure, "history" | "both");
        let source_fails = matches!(failure, "source" | "both" | "source-and-stdout");
        let checkpoints = Cell::new(0);
        let mut report = Report {
            bytes: Vec::new(),
            broken: matches!(failure, "stdout" | "source-and-stdout"),
        };
        let error = query::command_reconciled_with_elapsed(
            &repair_args(&case, &case.profile, &input, &output, &source),
            None,
            &mut report,
            || panic!("already visible triplet cannot read time, including on export failure"),
            &mut || {
                let index = checkpoints.get();
                checkpoints.set(index + 1);
                // Configuration's elapsed gate is first. The next is pre-open,
                // after create-new output admission and before original replay.
                if index == 1 {
                    if history_fails {
                        fs::write(&output, b"raced history output").unwrap();
                    }
                    if source_fails {
                        fs::write(&source, b"raced source output").unwrap();
                    }
                }
                if failure == "deadline" && output.exists() && source.exists() {
                    Duration::from_secs(3600)
                } else {
                    Duration::ZERO
                }
            },
        )
        .unwrap_err();
        assert!(
            error.contains("receipt checkpoint recovery confirmed"),
            "{error}"
        );
        assert!(error.contains("no effect retry was sent"), "{error}");
        if history_fails {
            assert!(error.contains("history anchor retention failed"), "{error}");
            assert_eq!(fs::read(&output).unwrap(), b"raced history output");
        } else {
            assert_eq!(anchors::load(&output).unwrap().revision(), state.revision);
        }
        if source_fails {
            assert!(
                error.contains("topology checkpoint retention failed"),
                "{error}"
            );
            assert_eq!(fs::read(&source).unwrap(), b"raced source output");
        } else {
            let saved = source_file(&source);
            assert_eq!(saved.history_anchor().revision(), state.revision);
            assert_eq!(saved.work(), case.saved.work());
            assert_eq!(saved.last_operation(), case.saved.last_operation());
        }
        if failure == "deadline" {
            assert!(error.contains("deadline expired"), "{error}");
            assert!(report.bytes.is_empty());
        } else if report.broken {
            assert!(error.contains("original response output failed"), "{error}");
            assert!(report.bytes.is_empty());
        } else {
            assert_eq!(report.bytes, original_response);
        }
        assert_eq!(canonical(&case), journal);
        assert_eq!(observed(&case.config, &case.loaded), state);
    }
    let output = case.root.0.join("retry.anchor");
    let source = case.root.0.join("retry.source");
    let mut response = Vec::new();
    query::command_reconciled_with_elapsed(
        &repair_args(&case, &case.profile, &input, &output, &source),
        None,
        &mut response,
        || panic!("retrying exports cannot read time"),
        &mut || Duration::ZERO,
    )
    .unwrap();
    assert_eq!(response, original_response);
    pair_at(&case, &source, &output, state.revision);
    assert_eq!(canonical(&case), journal);
    assert_eq!(observed(&case.config, &case.loaded), state);
    assert_no_live_inputs(&case);
}

#[test]
fn repair_preserves_the_exact_finished_stream_predecessor_after_real_actor_and_human_reviews() {
    let root = Root::new();
    let (config, mut recipe_path, loaded) = selected(&root, None, false);
    run_peer(&root, config, loaded, None, None);
    let mut saved = checkpoint(&root, 1);
    // Reuse the original complete numerical continuation fixture: two messages
    // and their explicit finish all cross authenticated actor and human peers.
    for generation in [2, 3] {
        let config = configured(&root);
        let (next_path, mut loaded) = native::next(&root, &config, generation);
        recipe_path = next_path;
        loaded.recovery_anchor =
            Some(anchors::load(&root.0.join(format!("history-{}.bin", generation - 1))).unwrap());
        loaded.anchor_output = Some(root.0.join(format!("history-{generation}.bin")));
        fs::write(
            root.0.join(format!("live-{generation}.json")),
            graph(generation, false),
        )
        .unwrap();
        fs::write(
            root.0.join(format!("inventory-{generation}.json")),
            INVENTORY,
        )
        .unwrap();
        let profile = archived_selection(&root, &saved, generation - 1, Some(generation));
        custody::admit(&config, &mut loaded, Some(&profile), custody::Mode::Live).unwrap();
        run_peer(
            &root,
            config,
            loaded,
            (generation == 2).then_some(1),
            (generation == 3).then_some(2),
        );
        saved = checkpoint(&root, generation);
    }
    let config = configured(&root);
    let loaded = recipe::load(&recipe_path, &config, false).unwrap();
    let before = observed(&config, &loaded);
    assert_eq!(before.executions, 3);
    assert_eq!(before.payload, b"AA");
    let profile = archived_selection(&root, &saved, 3, None);
    let config_path = root.0.join("finish-repair-config.json");
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
    for generation in [1, 2, 3] {
        fs::remove_file(root.0.join(format!("live-{generation}.json"))).unwrap();
        fs::remove_file(root.0.join(format!("inventory-{generation}.json"))).unwrap();
    }
    for name in [
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
    let case = Case {
        root,
        config,
        config_path,
        recipe: recipe_path,
        loaded,
        saved,
        before,
        profile,
    };
    let input = case.root.0.join("history-3.bin");
    let journal = canonical(&case);
    fs::write(
        case.config.store.join("delivery.pending"),
        b"finish custody staging",
    )
    .unwrap();
    for predecessor in [None, Some(1)] {
        let mut invalid = repair_args(
            &case,
            &case.profile,
            &input,
            &case.root.0.join("wrong-finish.anchor"),
            &case.root.0.join("wrong-finish.source"),
        );
        if let Some(previous) = predecessor {
            invalid.extend(["--finish-after-message".into(), previous.to_string()]);
        }
        assert!(
            query::command_reconciled_with_elapsed(
                &invalid,
                None,
                &mut Vec::new(),
                || panic!("wrong finish predecessor reached time"),
                &mut || Duration::ZERO
            )
            .is_err()
        );
        assert_eq!(canonical(&case), journal);
        assert_eq!(
            fs::read(case.config.store.join("delivery.pending")).unwrap(),
            b"finish custody staging"
        );
    }
    let mut original_response = None;
    let mut acknowledged = None;
    for attempt in [1, 2] {
        let output = case.root.0.join(format!("finish-{attempt}.anchor"));
        let source = case.root.0.join(format!("finish-{attempt}.source"));
        let mut command = repair_args(&case, &case.profile, &input, &output, &source);
        command.extend(["--finish-after-message".into(), "2".into()]);
        let calls = Cell::new(0);
        let mut response = Vec::new();
        query::command_reconciled_with_elapsed(
            &command,
            None,
            &mut response,
            || {
                assert_eq!(attempt, 1, "visible finish triplet cannot read time");
                calls.set(calls.get() + 1);
                ElapsedTick(1001)
            },
            &mut || Duration::ZERO,
        )
        .unwrap();
        assert_eq!(calls.get(), usize::from(attempt == 1));
        let state = observed(&case.config, &case.loaded);
        assert_eq!(state.revision, case.before.revision + 3);
        assert_effects(&case, &state);
        pair_at(&case, &source, &output, state.revision);
        if let Some(original) = &original_response {
            assert_eq!(&response, original);
        } else {
            original_response = Some(response);
        }
        if let Some(original) = &acknowledged {
            assert_eq!(&canonical(&case), original);
        } else {
            acknowledged = Some(canonical(&case));
        }
    }
    assert_no_live_inputs(&case);
}
