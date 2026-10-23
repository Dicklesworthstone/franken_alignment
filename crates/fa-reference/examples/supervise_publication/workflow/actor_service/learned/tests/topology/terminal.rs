//! The real terminal parser must retain the independently selected topology.
use super::super::super::{recovery::STOP_ACQUISITION_DEADLINE, stopped as terminal};
use super::*;
use std::{cell::Cell, collections::BTreeMap};

fn files(config: &Config) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&config.store)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn args(
    config: &Path,
    recipe: &Path,
    profile: &Path,
    phase: &str,
    anchor: &Path,
    output: Option<&Path>,
) -> Vec<String> {
    let mut args = vec![
        "create-learned-generated".into(),
        config.display().to_string(),
        recipe.display().to_string(),
        phase.into(),
        "801".into(),
        "--history-anchor".into(),
        anchor.display().to_string(),
        "--topology-profile".into(),
        profile.display().to_string(),
    ];
    if let Some(output) = output {
        args.extend([
            "--retain-history-anchor".into(),
            output.display().to_string(),
        ]);
    }
    args
}

fn command(
    args: &[String],
    clock: impl FnOnce() -> Result<ElapsedTick, JournalError>,
    elapsed: &mut impl FnMut() -> Duration,
) -> (Result<(), String>, String) {
    let (args, options) = anchors::take_options(args).unwrap();
    let mut output = Vec::new();
    let result = terminal::command_with(&args, options, clock, elapsed, &mut output);
    (result, String::from_utf8(output).unwrap())
}

#[test]
fn learned_topology_terminal_cli_keeps_all_predictor_custody_through_stop_and_drain() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, recipe_path, loaded) = selected(&root, mode, false);
        run_peer(&root, config, loaded, None, None);
        let config = configured(&root);
        let loaded = recipe::load(&recipe_path, &config, false).unwrap();
        let before = observed(&config, &loaded);
        let saved = checkpoint(&root, 1);
        assert_eq!(before.executions, 1);
        assert!(before.control.ledger.charged > 0);
        let original_anchor = root.0.join("history-1.bin");
        let original_archive = fs::read(&original_anchor).unwrap();
        let original_source = fs::read(root.0.join("source-1.bin")).unwrap();
        let profile = archived_selection(&root, &saved, 1, None);
        let config_path = root.0.join("terminal-config.json");
        let source = include_str!(concat!(
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
        fs::write(&config_path, source).unwrap();
        let admitted_config = Config::read(&config_path).unwrap();
        crate::config::assert_oversight_profiles_eq(&admitted_config.profile, &config.profile);
        assert_eq!(admitted_config.timing.runtime_ms, config.timing.runtime_ms);
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
        fs::remove_dir(root.0.join("peers")).unwrap();
        fs::write(
            config.store.join("delivery.pending"),
            b"topology terminal staged evidence",
        )
        .unwrap();
        let active = files(&config);
        let drain = args(
            &config_path,
            &recipe_path,
            &profile,
            "--drain-stopped",
            &original_anchor,
            None,
        );
        let (result, output) = command(
            &drain,
            || panic!("active topology cannot silently stop"),
            &mut || Duration::ZERO,
        );
        assert!(result.unwrap_err().contains("WrongState"));
        assert!(output.is_empty());
        assert_eq!(files(&config), active);
        let combined = args(
            &config_path,
            &recipe_path,
            &profile,
            "--recover-stop",
            &original_anchor,
            None,
        );
        let elapsed = Cell::new(Duration::ZERO);
        let (result, output) = command(
            &combined,
            || {
                elapsed.set(Duration::from_millis(config.timing.runtime_ms));
                Ok(ElapsedTick(1001))
            },
            &mut || elapsed.get(),
        );
        assert!(result.unwrap_err().contains(STOP_ACQUISITION_DEADLINE));
        assert!(output.is_empty());
        assert_eq!(files(&config), active);
        assert_eq!(observed(&config, &loaded), before);

        let admitted_anchor = root.0.join("topology-admitted.anchor");
        let admission = args(
            &config_path,
            &recipe_path,
            &profile,
            "--stop-admission",
            &original_anchor,
            Some(&admitted_anchor),
        );
        let (result, output) = command(
            &admission,
            || panic!("topology admission stop has no logical-clock dependency"),
            &mut || Duration::ZERO,
        );
        result.unwrap();
        assert!(output.contains("\"appended_events\":2"));
        assert!(output.contains("\"drain_attempted\":false"));
        let stopped = observed(&config, &loaded);
        assert_eq!(stopped.revision, before.revision + 2);
        assert_eq!(stopped.executions, before.executions);
        assert_eq!(
            stopped.control.ledger.charged,
            before.control.ledger.charged
        );
        assert_eq!(
            anchors::load(&admitted_anchor).unwrap().revision(),
            stopped.revision
        );
        let stopped_bytes = files(&config);
        // An archived source checkpoint binds its original cut, never a later
        // availability withdrawal. Every phase must preserve this distinction.
        for phase in ["--stop-admission", "--drain-stopped", "--recover-stop"] {
            let retry = args(
                &config_path,
                &recipe_path,
                &profile,
                phase,
                &admitted_anchor,
                None,
            );
            let (result, output) = command(
                &retry,
                || panic!("stale topology cut must refuse before time"),
                &mut || Duration::ZERO,
            );
            assert!(result.is_err());
            assert!(output.is_empty());
            assert_eq!(files(&config), stopped_bytes);
        }

        // Independently select the actual stopped availability; the exact old
        // source work and immutable graph remain required without any live read.
        fs::write(root.0.join("terminal-current.json"), saved.topology_bytes()).unwrap();
        fs::write(
            root.0.join("terminal-current-inventory.json"),
            saved.inventory_bytes(),
        )
        .unwrap();
        let current = pair(
            &root,
            "terminal-current.json",
            "terminal-current-inventory.json",
        );
        let initial = pair(&root, "initial.json", "initial-inventory.json");
        let next = pair(&root, "missing-next.json", "missing-next-inventory.json");
        let manual = root.0.join("terminal-topology.json");
        for available in [true, false] {
            fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{initial},"current":{current},"next":{next},"available":{available},"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
                saved.last_operation() + 1, saved.work().checks, saved.work().reserved_read_bytes,
                saved.last_operation())).unwrap();
            for phase in ["--stop-admission", "--drain-stopped", "--recover-stop"] {
                let retry = args(
                    &config_path,
                    &recipe_path,
                    &manual,
                    phase,
                    &admitted_anchor,
                    None,
                );
                let (result, output) = command(
                    &retry,
                    || panic!("guard refusal or completed retry cannot acquire time"),
                    &mut || Duration::ZERO,
                );
                assert_eq!(result.is_ok(), !available, "{mode:?}: {phase}: {result:?}");
                if available {
                    assert!(output.is_empty());
                } else if phase == "--stop-admission" {
                    assert!(output.contains("\"appended_events\":0"));
                } else {
                    assert!(output.contains("\"recovery\":\"already_drained\""));
                }
                assert_eq!(files(&config), stopped_bytes);
            }
        }
        let drained_anchor = root.0.join("topology-drained.anchor");
        let drain = args(
            &config_path,
            &recipe_path,
            &manual,
            "--drain-stopped",
            &admitted_anchor,
            Some(&drained_anchor),
        );
        let (result, _) = command(
            &drain,
            || panic!("drained topology retry has no clock"),
            &mut || Duration::ZERO,
        );
        result.unwrap();
        assert_eq!(files(&config), stopped_bytes);
        assert_eq!(
            fs::read(&admitted_anchor).unwrap(),
            fs::read(&drained_anchor).unwrap()
        );
        assert_eq!(observed(&config, &loaded), stopped);
        assert_eq!(fs::read(&original_anchor).unwrap(), original_archive);
        assert_eq!(
            fs::read(root.0.join("source-1.bin")).unwrap(),
            original_source
        );
        assert!(!root.0.join("peers").exists());
    }
}
