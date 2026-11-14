//! Admission controls for the newly composed governor/topology selection.
use super::*;

#[test]
fn mediated_governor_rejects_normalized_checkpoint_socket_alias_before_binding() {
    for alias_directory in [false, true] {
        let root = Root::new();
        let (config, paths, entries) = selected_mediated(&root);
        selections(&root, &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap());
        let mut args = inputs(&root, &config, &paths, &entries, None);
        let profile = root.0.join("create-topology.json");
        let destination = if alias_directory {
            let directory = root.0.join("peers-alias");
            std::os::unix::fs::symlink(root.0.join("peers"), &directory).unwrap();
            directory.join("governor.sock")
        } else { root.0.join("peers/governor.sock") };
        let original = fs::read_to_string(&profile).unwrap();
        let changed = original.replace(root.0.join("source-1.bin").to_str().unwrap(),
            destination.to_str().unwrap());
        assert_ne!(changed, original);
        fs::write(&profile, changed).unwrap();
        args.extend(["--topology-profile".into(), profile.display().to_string()]);
        let source = fs::read(root.0.join("live-1.json")).unwrap();
        let inventory = fs::read(root.0.join("inventory-1.json")).unwrap();
        let error = resident::command_with_clock(&args, None,
            || panic!("aliased source custody reached live time")).unwrap_err();
        assert!(error.contains("retained checkpoint custody"), "{error}");
        assert!(!config.store.exists());
        assert!(!root.0.join("final.anchor").exists());
        assert!(!root.0.join("source-1.bin").exists());
        assert_eq!(fs::read(root.0.join("live-1.json")).unwrap(), source);
        assert_eq!(fs::read(root.0.join("inventory-1.json")).unwrap(), inventory);
        quiescent(&root);
    }
}

#[test]
fn mediated_governor_still_rejects_raw_and_owned_forecast_in_later_recipe_before_assets() {
    for owned in [false, true] {
        let root = Root::new();
        let (config, paths, entries) = selected_mediated(&root);
        selections(&root, &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap());
        let mut args = inputs(&root, &config, &paths, &entries, None);
        args.extend(["--topology-profile".into(),
            root.0.join("create-topology.json").display().to_string()]);
        super::super::super::super::forecast::install(&root, &paths[1], owned, 60_000);
        for name in ["weights.safetensors", "native-roster.json", "evidence.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let error = resident::command_with_clock(&args, None,
            || panic!("unsupported governed predictor reached live time")).unwrap_err();
        assert!(error.contains("nonpredictive"), "{error}");
        assert!(!config.store.exists());
        assert!(!root.0.join("final.anchor").exists());
        assert!(!root.0.join("source-1.bin").exists());
        quiescent(&root);
    }
}

#[test]
fn mediated_governor_refuses_a_different_valid_anchor_without_touching_original_history() {
    let root = Root::new();
    let (config, paths, entries) = selected_mediated(&root);
    seed(&root, &paths[0]);
    let source_bytes = fs::read(root.0.join("source-1.bin")).unwrap();
    let history_bytes = fs::read(root.0.join("history-1.bin")).unwrap();
    // Produce another genuinely acknowledged anchor using the original guarded
    // replay. Its Fence successor is not the source checkpoint's retained cut.
    let mut reader = Reader::new(&root, &paths[0]);
    reader.loaded.expected_campaigns = Some(requirement());
    reader.loaded.recovery_anchor = Some(anchors::load(&root.0.join("history-1.bin")).unwrap());
    let historical = archived(&root, &root.0.join("source-1.bin"), 99, None);
    topology::admit(&reader.config, &mut reader.loaded, Some(&historical),
        topology::Mode::Historical).unwrap();
    let wrong = root.0.join("different-valid.anchor");
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    assert_ne!(host.history_anchor().unwrap(), anchors::load(&root.0.join("history-1.bin")).unwrap());
    anchors::export_acknowledged(&host.history_anchor().unwrap(), &wrong).unwrap();
    drop(host);
    let canonical = config.store.join("delivery.bin");
    let before = fs::read(&canonical).unwrap();
    let staging = config.store.join("delivery.pending");
    fs::write(&staging, b"independent unacknowledged staging").unwrap();
    recovery_recipes(&paths[1..]);
    next_source(&root);
    selections(&root, &entries[0].actor,
        std::process::id().checked_add(10_000).unwrap());
    let mut args = recovered_args(inputs(&root, &config, &paths[1..], &entries[1..], None), &root);
    let at = args.iter().position(|arg| arg == "--history-anchor").unwrap();
    args[at + 1] = wrong.display().to_string();
    let output = root.0.join("source-2.bin");
    let profile = archived(&root, &root.0.join("source-1.bin"), 2, Some(&output));
    args.extend(["--topology-profile".into(), profile.display().to_string()]);
    let error = resident::command_with_clock(&args, None,
        || panic!("mixed valid source/history cuts reached live time")).unwrap_err();
    assert!(error.contains("different cuts"), "{error}");
    assert_eq!(fs::read(&canonical).unwrap(), before);
    assert_eq!(fs::read(&staging).unwrap(), b"independent unacknowledged staging");
    assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), source_bytes);
    assert_eq!(fs::read(root.0.join("history-1.bin")).unwrap(), history_bytes);
    assert!(!output.exists());
    assert!(!root.0.join("final.anchor").exists());
    quiescent(&root);
}

#[test]
fn mediated_governor_refuses_valid_manual_source_minima_before_recovery_mutation() {
    let root = Root::new();
    let (config, paths, entries) = selected_mediated(&root);
    seed(&root, &paths[0]);
    let saved = mediated_fixture::checkpoint(&root, 1);
    assert!(saved.available());
    let canonical = config.store.join("delivery.bin");
    let before = fs::read(&canonical).unwrap();
    let source_bytes = fs::read(root.0.join("source-1.bin")).unwrap();
    let history_bytes = fs::read(root.0.join("history-1.bin")).unwrap();
    let staging = config.store.join("delivery.pending");
    fs::write(&staging, b"unacknowledged staging before governed recovery").unwrap();
    recovery_recipes(&paths[1..]);
    next_source(&root);
    let pair = |graph: &str, inventory: &str| format!(
        r#"{{"topology_path":{},"inventory_path":{}}}"#,
        quote(root.0.join(graph).to_str().unwrap()),
        quote(root.0.join(inventory).to_str().unwrap()));
    let profile = root.0.join("manual-governed-topology.json");
    fs::write(&profile, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{},"current":{},"next":{},"available":{},"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
        pair("initial.json", "initial-inventory.json"),
        pair("live-1.json", "inventory-1.json"),
        pair("live-2.json", "inventory-2.json"),
        saved.available(), saved.last_operation() + 1, saved.work().checks,
        saved.work().reserved_read_bytes, saved.last_operation())).unwrap();
    // This is a valid ordinary v1 selection with actual spent work and current
    // availability. Governed recovery additionally requires the exact v2 pair,
    // as exercised positively by the anchored resident workflow in the parent.
    let mut selected = recipe::load(&paths[1], &config, false).unwrap();
    selected.recovery_anchor = Some(anchors::load(&root.0.join("history-1.bin")).unwrap());
    topology::admit(&config, &mut selected, Some(&profile), topology::Mode::Live).unwrap();
    assert!(topology::history(&selected).unwrap().unwrap().checkpoint().is_none());
    selections(&root, &entries[0].actor,
        std::process::id().checked_add(10_000).unwrap());
    let mut args = recovered_args(inputs(&root, &config, &paths[1..], &entries[1..], None), &root);
    args.extend(["--topology-profile".into(), profile.display().to_string()]);
    let error = resident::command_with_clock(&args, None,
        || panic!("manual governed topology reached live recovery time")).unwrap_err();
    assert!(error.contains("requires an exact v2 source checkpoint"), "{error}");
    assert_eq!(fs::read(&canonical).unwrap(), before);
    assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged staging before governed recovery");
    assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), source_bytes);
    assert_eq!(fs::read(root.0.join("history-1.bin")).unwrap(), history_bytes);
    assert!(!root.0.join("source-2.bin").exists());
    assert!(!root.0.join("final.anchor").exists());
    quiescent(&root);
}
