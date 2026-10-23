//! Failed live work must preserve healthy, acknowledged source custody.
//! Native storage poison remains unexportable; errors never imply permission.
use super::*;
use super::super::super::stopped as terminal_command;

const REFUSAL: &str = "initial learned topology was not certified by the native checker";

fn config_path(root: &Root, config: &Config) -> PathBuf {
    let path = root.0.join("failed-service-config.json");
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))
        .replace("/operator/publication-store", config.store.to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("\"initial_payload_hex\": \"696e697469616c\"", "\"initial_payload_hex\": \"\"")
        .replace("\"events\": 256", "\"events\": 1024")
        .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 60000");
    fs::write(&path, text).unwrap();
    crate::config::assert_oversight_profiles_eq(&Config::read(&path).unwrap().profile, &config.profile);
    path
}

fn terminal_args(config: &Path, recipe: &Path, profile: &Path, phase: &str,
    operation: u64, input: &Path, anchor: &Path, source: &Path) -> Vec<String>
{
    vec!["create-learned-generated".into(), config.display().to_string(),
        recipe.display().to_string(), phase.into(), operation.to_string(),
        "--history-anchor".into(), input.display().to_string(),
        "--topology-profile".into(), profile.display().to_string(),
        "--retain-history-anchor".into(), anchor.display().to_string(),
        "--retain-topology-checkpoint".into(), source.display().to_string()]
}

#[test]
fn learned_failed_service_retains_healthy_source_work_and_supports_offline_terminal_retry() {
    for mode in [None, Some(false), Some(true)] {
        for missing_source in [false, true] {
            let root = Root::new();
            let (config, recipe_path, loaded) = selected(&root, mode, !missing_source);
            let peers = fixture::peers(&root, &config);
            let store = config.store.clone();
            let original_topology = fs::read(root.0.join("live-1.json")).unwrap();
            let original_inventory = fs::read(root.0.join("inventory-1.json")).unwrap();
            let mut removed = false;
            let result = run(config, loaded, &peers, false, || {
                if missing_source && !removed && store.join("delivery.bin").exists() {
                    // Intake has admitted the original documents and output.
                    // The original source's next real read observes this loss.
                    fs::remove_file(root.0.join("inventory-1.json")).unwrap();
                    removed = true;
                }
                ElapsedTick(1000)
            }).unwrap();
            assert_eq!(removed, missing_source);
            assert_eq!(result.failure.as_deref(), Some(REFUSAL));
            assert_eq!(result.cleanup_pending, 0);
            assert!(!matches!(result.response.result,
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
            let config = configured(&root);
            let loaded = recipe::load(&recipe_path, &config, false).unwrap();
            let state = observed(&config, &loaded);
            assert_eq!(state.executions, 0);
            assert!(state.payload.is_empty());
            assert!(state.control.ledger.stages.is_empty());
            assert_eq!(state.control.ledger.reserved, 0);
            assert_eq!(state.control.ledger.charged, 0);
            let operation = state.stop.as_ref().expect("original failure applies native Stop").request().operation;
            assert_eq!(operation, loaded.request);
            let saved = checkpoint(&root, 1);
            let original_anchor = root.0.join("history-1.bin");
            assert_eq!(saved.history_anchor(), &anchors::load(&original_anchor).unwrap());
            assert_eq!(saved.minimum().journal_revision, state.revision);
            assert_eq!(saved.minimum().control_sequence, state.control.sequence);
            assert_eq!(saved.minimum().authority_epoch, state.control.ledger.epoch);
            assert_eq!(saved.work().checks, 1);
            assert_eq!(saved.work().reserved_read_bytes, TOPOLOGY_REFRESH_READ_BYTES);
            assert_eq!(saved.last_operation(), 1);
            // refresh_checked withdraws both a non-Verified cut and missing
            // inventory through the original native update with next: None.
            // This happens before Stop; it is not an invented recovery Fence.
            assert!(!saved.available(),
                "the native source withdraws uncertified topology and missing inventory");
            assert_eq!(saved.topology_bytes(), original_topology.as_slice());
            assert_eq!(saved.inventory_bytes(), original_inventory.as_slice());
            let source_bytes = fs::read(root.0.join("source-1.bin")).unwrap();
            let anchor_bytes = fs::read(&original_anchor).unwrap();
            let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
            let profile = archived_selection(&root, &saved, 1, None);
            let config_path = config_path(&root, &config);
            for name in ["live-1.json", "evidence.json", "native-roster.json",
                "helper-model.json", "helper-weights.safetensors", "helper-monitor.json",
                "helper-tokenizer.bbpe", "helper-sampling.json"] {
                fs::remove_file(root.0.join(name)).unwrap();
            }
            if !missing_source { fs::remove_file(root.0.join("inventory-1.json")).unwrap(); }
            assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
            fs::remove_dir(root.0.join("peers")).unwrap();
            for (index, phase) in ["--stop-admission", "--drain-stopped", "--recover-stop"].iter().enumerate() {
                let anchor = root.0.join(format!("failed-service-{index}.anchor"));
                let source = root.0.join(format!("failed-service-{index}.source"));
                let args = terminal_args(&config_path, &recipe_path, &profile, phase,
                    operation, &original_anchor, &anchor, &source);
                let (args, options) = anchors::take_options(&args).unwrap();
                let mut output = Vec::new();
                terminal_command::command_with(&args, options,
                    || panic!("already drained failure retry cannot acquire time"),
                    &mut || Duration::ZERO, &mut output).unwrap();
                let report = String::from_utf8(output).unwrap();
                assert!(report.contains("\"drained\":true"));
                let retained = TopologyCheckpoint::read_trusted(
                    &mut Cursor::new(fs::read(&source).unwrap()), MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
                assert_eq!(retained, saved);
                assert_eq!(retained.history_anchor(), &anchors::load(&anchor).unwrap());
                assert_eq!(fs::read(&source).unwrap(), source_bytes);
                assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
                assert_eq!(observed(&config, &loaded), state);
            }
            assert_eq!(fs::read(original_anchor).unwrap(), anchor_bytes);
            assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), source_bytes);
        }
    }
}

#[test]
fn learned_failed_service_checkpoint_collision_preserves_original_error_and_independent_anchor() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, recipe_path, loaded) = selected(&root, mode, true);
        let peers = fixture::peers(&root, &config);
        let store = config.store.clone();
        let source = root.0.join("source-1.bin");
        let mut collided = false;
        let result = run(config, loaded, &peers, false, || {
            if !collided && store.join("delivery.bin").exists() {
                fs::write(&source, b"independent competing archive").unwrap();
                collided = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        assert!(collided);
        let failure = result.failure.unwrap();
        assert!(failure.starts_with(REFUSAL), "{failure}");
        assert!(failure.contains("topology checkpoint retention failed"), "{failure}");
        assert!(failure.contains("do not resend an effect"), "{failure}");
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(fs::read(&source).unwrap(), b"independent competing archive");
        let config = configured(&root);
        let loaded = recipe::load(&recipe_path, &config, false).unwrap();
        let state = observed(&config, &loaded);
        assert_eq!(state.executions, 0);
        assert!(state.stop.is_some());
        assert_eq!(anchors::load(&root.0.join("history-1.bin")).unwrap().revision(), state.revision);
    }
}

#[test]
fn learned_failed_service_poisoned_owner_cannot_export_either_checkpoint() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, recipe_path, loaded) = selected(&root, mode, false);
        let peers = fixture::peers(&root, &config);
        let store = config.store.clone();
        let mut original = None;
        let result = run(config, loaded, &peers, false, || {
            if original.is_none() && store.join("delivery.bin").exists() {
                original = Some(fs::read(store.join("delivery.bin")).unwrap());
                // The actual next native observe_time transaction fails at the
                // stage creation barrier, latching the original owner's fault.
                fs::create_dir(store.join("delivery.pending")).unwrap();
            }
            ElapsedTick(1000)
        }).unwrap();
        let failure = result.failure.unwrap();
        assert!(failure.starts_with("Io("), "{failure}");
        assert!(failure.contains("operation: Stage"), "{failure}");
        assert!(failure.contains("kind: AlreadyExists"), "{failure}");
        assert!(failure.contains("stop/drain: Unavailable"), "{failure}");
        assert!(failure.contains("topology checkpoint retention failed: Unavailable"), "{failure}");
        assert!(failure.contains("history anchor retention failed: Unavailable"), "{failure}");
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), original.unwrap());
        assert!(store.join("delivery.pending").is_dir());
        assert!(!root.0.join("source-1.bin").exists());
        assert!(!root.0.join("history-1.bin").exists());
        let config = configured(&root);
        let loaded = recipe::load(&recipe_path, &config, false).unwrap();
        let state = observed(&config, &loaded);
        assert_eq!(state.executions, 0);
        assert!(state.stop.is_none());
        assert!(state.control.ledger.stages.is_empty());
    }
}
