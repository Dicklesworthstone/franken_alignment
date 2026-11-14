//! Governance cannot take source/archive paths or mutate a source-free receipt
//! service. These controls retain the same genuine published prefix as the live
//! creation and recovered-governor workflows in the parent module.
use super::*;
use crate::workflow::actor_service::topology::recovery as mediated_recovery;

fn selected_governor(root: &Directory, config: &Config) -> governor::Settings {
    governor::Settings::decode(governor_document(root, config,
        std::process::id().checked_add(10_000).unwrap()).as_bytes()).unwrap()
}

fn governor_alias(root: &Directory, governor: &governor::Settings) -> PathBuf {
    let alias = root.0.join("source-custody-alias");
    std::os::unix::fs::symlink(governor.socket.parent().unwrap(), &alias).unwrap();
    alias.join(governor.socket.file_name().unwrap())
}

fn no_endpoints(actor: &Profile, reviewer: &PeerProfile,
    operator: &operator::Settings, governor: &governor::Settings)
{
    for path in [actor.socket.clone(), operator.socket.clone(), governor.socket.clone(),
        reviewer.socket(7), reviewer.socket(8),
        crate::workflow::control::socket_path(reviewer, 7),
        crate::workflow::control::socket_path(reviewer, 8)] {
        assert!(!path.exists(), "retained service endpoint {}", path.display());
    }
}

#[test]
fn governed_mediated_creation_rejects_normalized_source_and_archive_aliases_before_time() {
    for source_alias in [false, true] {
        let root = Directory::new();
        let config = mediated_config(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let operator = operator_settings(&root, &config);
        let governor = selected_governor(&root, &config);
        let alias = governor_alias(&root, &governor);
        let graph = if source_alias { alias.clone() } else { root.0.join("missing-topology.json") };
        let output = if source_alias { root.0.join("source-1.bin") } else { alias.clone() };
        let topology = CreationSelection::decode(format!(
            r#"{{"schema_version":"fa.mediated-create/2","topology_path":{:?},"inventory_path":{:?},"retain_checkpoint":{:?}}}"#,
            graph.to_str().unwrap(), root.0.join("missing-inventory.json").to_str().unwrap(),
            output.to_str().unwrap()).as_bytes()).unwrap();
        let store = config.store.clone();
        let error = service::serve_tower_governed_with_topology(config, &actor, &reviewer,
            None, &[7, 8], GENERATION, &operator, &governor, topology,
            || panic!("aliased mediated governor custody reached time")).unwrap_err();
        assert!(error.contains("source or retained checkpoint custody"), "{error}");
        assert!(!store.exists());
        assert!(!graph.exists());
        assert!(!output.exists());
        assert!(!root.0.join("missing-inventory.json").exists());
        no_endpoints(&actor, &reviewer, &operator, &governor);
    }
}

#[test]
fn governed_mediated_recovery_rejects_missing_next_source_and_archive_aliases_before_fence() {
    let root = Directory::new();
    let (config, recovery, saved) = seed(&root, false, false);
    let (actor, reviewer) = profiles(&root, &config);
    let operator = operator_settings(&root, &config);
    let governor = selected_governor(&root, &config);
    let alias = governor_alias(&root, &governor);
    let canonical = config.store.join("delivery.bin");
    let canonical_bytes = fs::read(&canonical).unwrap();
    let source_bytes = fs::read(root.0.join("source-1.bin")).unwrap();
    let history_bytes = fs::read(root.0.join("before.anchor")).unwrap();
    let staging = config.store.join("delivery.pending");
    fs::write(&staging, b"independent staging before governed recovery").unwrap();
    for source_alias in [false, true] {
        let graph = if source_alias { alias.clone() } else { root.0.join("missing-next.json") };
        let output = if source_alias { root.0.join("source-2.bin") } else { alias.clone() };
        let topology = RecoverySelection::decode(format!(
            r#"{{"schema_version":"fa.mediated-submit/2","initial":{{"topology_path":{:?},"inventory_path":{:?}}},"source_checkpoint":{:?},"next":{{"topology_path":{:?},"inventory_path":{:?}}},"operation":{},"retain_checkpoint":{:?}}}"#,
            root.0.join("initial-topology.json").to_str().unwrap(),
            root.0.join("initial-inventory.json").to_str().unwrap(),
            root.0.join("source-1.bin").to_str().unwrap(), graph.to_str().unwrap(),
            root.0.join("missing-next-inventory.json").to_str().unwrap(),
            saved.last_operation() + 1, output.to_str().unwrap()).as_bytes()).unwrap();
        let error = mediated_recovery::serve_sequence_governed(mediated_config(&root),
            &actor, &reviewer, None, None, &recovery, &topology, GENERATION,
            &operator, &governor, &[7, 8],
            || panic!("aliased recovered governor custody reached time")).unwrap_err();
        assert!(error.contains("source or retained checkpoint custody"), "{error}");
        assert_eq!(fs::read(&canonical).unwrap(), canonical_bytes);
        assert_eq!(fs::read(&staging).unwrap(), b"independent staging before governed recovery");
        assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), source_bytes);
        assert_eq!(fs::read(root.0.join("before.anchor")).unwrap(), history_bytes);
        assert!(!graph.exists());
        assert!(!output.exists());
        assert!(!root.0.join("after.anchor").exists());
        no_endpoints(&actor, &reviewer, &operator, &governor);
    }
}

#[test]
fn all_historical_governed_mediated_schedules_refuse_without_source_work_and_retain_the_recovered_pair() {
    for checked_mode in [false, true] {
        let root = Directory::new();
        let (mut config, recovery, original) = seed(&root, checked_mode, true);
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(before.executions, 2);
        let source_bytes = fs::read(root.0.join("source-1.bin")).unwrap();
        let history_bytes = fs::read(root.0.join("before.anchor")).unwrap();
        let topology = next_selection(&root, &original);
        let publication = checked_mode.then(|| PublicationProfile::decode(
            checked::profile_document(&root).as_bytes()).unwrap());
        let (actor, reviewer) = profiles(&root, &config);
        let operator = operator_settings(&root, &config);
        let governor = selected_governor(&root, &config);
        for name in ["topology-2.json", "inventory-2.json", "evidence.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        if checked_mode {
            for name in ["original.bin", "current.bin"] {
                fs::remove_file(root.0.join(name)).unwrap();
            }
        }
        config.programs.clear();
        let profile = config.profile.clone();
        let store = config.store.clone();
        let mut calls = 0;
        let error = mediated_recovery::serve_sequence_governed(config, &actor, &reviewer,
            publication.as_ref(), None, &recovery, &topology, GENERATION,
            &operator, &governor, &[7, 8], || {
                calls += 1;
                assert!(!governor.socket.exists(), "historical schedule activated a governor");
                ElapsedTick(1001)
            }).unwrap_err();
        assert!(calls > 0, "the original guarded replay must actually run");
        assert!(error.contains("requires a new scheduled request"), "{error}");
        let after = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(after.revision, before.revision + 3,
            "only original Fence/Time/Sweep may precede the refusal");
        assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 2);
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.target, before.target);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.control.ledger.available, before.control.ledger.available);
        assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
        assert_eq!(after.control.ledger.stages, before.control.ledger.stages);
        assert!(after.stop.is_none());
        let retained = read_checkpoint(&root, "source-2.bin");
        assert_eq!(retained.history_anchor(), &read_anchor(&root, "after.anchor"));
        assert_eq!(retained.history_anchor().revision(), after.revision);
        assert_eq!(retained.minimum().authority_epoch, after.control.ledger.epoch);
        assert_eq!(retained.work(), original.work());
        assert_eq!(retained.last_operation(), original.last_operation());
        assert_eq!(retained.topology_bytes(), original.topology_bytes());
        assert_eq!(retained.inventory_bytes(), original.inventory_bytes());
        assert!(!retained.available());
        assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), source_bytes);
        assert_eq!(fs::read(root.0.join("before.anchor")).unwrap(), history_bytes);
        for name in ["topology-2.json", "inventory-2.json", "evidence.json", "original.bin", "current.bin"] {
            assert!(!root.0.join(name).exists(), "source-free refusal created {name}");
        }
        no_endpoints(&actor, &reviewer, &operator, &governor);
    }
}
