//! The ordinary prepared prompt retains its exact topology/history pair and
//! cumulative source work. Recovery still requires a fresh current source.
use super::*;
use crate::workflow::actor_service::learned::topology;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
    mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
};
use std::io::Cursor;

#[path = "mediated/control.rs"]
mod control;

fn selected(root: &Root) -> Case {
    // Reuse the original independently provisioned graph and inventory, then
    // select the ordinary numerical recipe under functional_restart.
    let (_, path, _) = mediated_topology::selected(root, None, false);
    let case = super::selected(root, false);
    assert_eq!(path, case.recipe);
    case
}

fn preparation_args(root: &Root, case: &Case) -> Vec<String> {
    let mut args = case.args();
    args.extend([
        "--topology-profile".into(),
        root.0.join("create-topology.json").display().to_string(),
    ]);
    args
}

fn checkpoint(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

fn prepare(root: &Root, case: &Case) -> TopologyCheckpoint {
    let mut output = Vec::new();
    prepare::command(
        &preparation_args(root, case),
        None,
        || ElapsedTick(1000),
        &mut output,
    )
    .unwrap();
    let report = report(&output);
    prepared(case, &report);
    assert_eq!(
        report
            .get("topology_checkpoint_retained")
            .and_then(Json::as_bool),
        Some(true)
    );
    let saved = checkpoint(&root.0.join("source-1.bin"));
    assert_eq!(
        saved.history_anchor(),
        &custody::load(&case.anchor).unwrap()
    );
    assert_eq!(saved.work().checks, 1);
    assert_eq!(
        saved.work().reserved_read_bytes,
        TOPOLOGY_REFRESH_READ_BYTES
    );
    assert!(saved.available());
    saved
}

fn recovery_profile(root: &Root, input: &Path, output: Option<&Path>, label: &str) -> PathBuf {
    let saved = checkpoint(input);
    let profile = root.0.join(format!("{label}-topology.json"));
    let retained = output.map_or_else(String::new, |path| {
        format!(r#", "retain_checkpoint":"{}""#, path.display())
    });
    fs::write(
        &profile,
        format!(
            r#"{{"schema_version":"fa.mediated-submit/2",
        "initial":{{"topology_path":"{}","inventory_path":"{}"}},
        "next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":{},
        "source_checkpoint":"{}"{retained}}}"#,
            root.0.join("initial.json").display(),
            root.0.join("initial-inventory.json").display(),
            root.0.join("live-2.json").display(),
            root.0.join("inventory-2.json").display(),
            saved.last_operation() + 1,
            input.display()
        ),
    )
    .unwrap();
    profile
}

fn next_source(root: &Root) {
    let graph = fs::read_to_string(root.0.join("initial.json"))
        .unwrap()
        .replacen("\"generation\":1", "\"generation\":2", 1)
        .replacen(
            "\"inventory_generation\":1",
            "\"inventory_generation\":2",
            1,
        )
        .replacen(
            "\"complete_inventory_generation\":1",
            "\"complete_inventory_generation\":2",
            1,
        );
    fs::write(root.0.join("live-2.json"), graph).unwrap();
    fs::copy(
        root.0.join("initial-inventory.json"),
        root.0.join("inventory-2.json"),
    )
    .unwrap();
}

#[test]
fn ordinary_mediated_preparation_continues_with_original_work_and_fresh_source_and_human_keys() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let case = selected(&root);
        let helper = root.0.join("helper-weights.safetensors");
        let weights = fs::read(&helper).unwrap();
        fs::remove_file(&helper).unwrap();
        let original = prepare(&root, &case);
        let input_source = fs::read(root.0.join("source-1.bin")).unwrap();
        let input_history = fs::read(&case.anchor).unwrap();
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        fs::write(helper, weights).unwrap();
        next_source(&root);
        let source = root.0.join("continued.topology");
        let retained = root.0.join("continued.anchor");
        let profile = recovery_profile(
            &root,
            &root.0.join("source-1.bin"),
            Some(&source),
            "continue",
        );
        let (peer_path, peers) = functional_creation::peers_file(&root, &case.config);
        let args = vec![
            "create-learned-generated".into(),
            case.config_path.display().to_string(),
            case.recipe.display().to_string(),
            peer_path.display().to_string(),
            "--continue-generation".into(),
            "--history-anchor".into(),
            case.anchor.display().to_string(),
            "--retain-history-anchor".into(),
            retained.display().to_string(),
            "--topology-profile".into(),
            profile.display().to_string(),
        ];
        let mut human = None;
        let result =
            crate::workflow::actor_service::learned::command_with_clock(&args, None, || {
                if human.is_none() && peers.socket(1).exists() {
                    human = Some(peer_intake::human(peers.clone(), 1, decision));
                }
                ElapsedTick(1001)
            });
        let packet = human
            .expect("prepared topology is not an independent human key")
            .join()
            .unwrap();
        original_packet(&packet);
        assert_eq!(
            result.is_ok(),
            decision == ReviewDecision::Approve,
            "{result:?}"
        );
        let state = view(&case);
        assert_eq!(
            state.executions,
            u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(
            state.control.ledger.charged,
            if decision == ReviewDecision::Approve {
                packet.action().spec().units
            } else {
                0
            }
        );
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(
            state.control.ledger.available + state.control.ledger.charged,
            case.config.profile.delivery.total
        );
        let saved = checkpoint(&source);
        assert_eq!(saved.history_anchor(), &custody::load(&retained).unwrap());
        assert_eq!(saved.history_anchor().revision(), state.revision);
        assert_eq!(
            saved.work().checks,
            original.work().checks + 1 + u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(
            saved.work().reserved_read_bytes,
            saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert_eq!(
            saved.expected().source_limits(),
            original.expected().source_limits()
        );
        assert_eq!(saved.expected().graph().spec().generation, 2);
        assert!(saved.last_operation() > original.last_operation());
        assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), input_source);
        assert_eq!(fs::read(&case.anchor).unwrap(), input_history);
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&retained).unwrap());
        let historical = recovery_profile(&root, &source, None, "historical");
        topology::admit(
            &case.config,
            &mut loaded,
            Some(&historical),
            topology::Mode::Historical,
        )
        .unwrap();
        let (host, _, forecast, _) = recovery::open_selected(&case.config, &loaded).unwrap();
        assert!(forecast.is_none());
        numerical(&host, 4, 2);
    }
}

#[test]
fn ordinary_mediated_live_checkpoint_capture_still_requires_current_source_and_human_approval() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let case = selected(&root);
        let (peer_path, peers) = functional_creation::peers_file(&root, &case.config);
        let args = vec![
            "create-learned-generated".into(),
            case.config_path.display().to_string(),
            case.recipe.display().to_string(),
            peer_path.display().to_string(),
            "--checkpoint-before-generation".into(),
            case.selection.display().to_string(),
            "--retain-history-anchor".into(),
            case.anchor.display().to_string(),
            "--topology-profile".into(),
            root.0.join("create-topology.json").display().to_string(),
        ];
        let mut human = None;
        let result =
            crate::workflow::actor_service::learned::command_with_clock(&args, None, || {
                if human.is_none() && peers.socket(1).exists() {
                    human = Some(peer_intake::human(peers.clone(), 1, decision));
                }
                ElapsedTick(1000)
            });
        let packet = human
            .expect("live capture cannot replace the independent human key")
            .join()
            .unwrap();
        original_packet(&packet);
        assert_eq!(
            result.is_ok(),
            decision == ReviewDecision::Approve,
            "{result:?}"
        );
        let state = view(&case);
        assert_eq!(
            state.executions,
            u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(
            state.control.ledger.charged,
            if decision == ReviewDecision::Approve {
                packet.action().spec().units
            } else {
                0
            }
        );
        assert_eq!(state.control.ledger.reserved, 0);
        let source = root.0.join("source-1.bin");
        let saved = checkpoint(&source);
        assert_eq!(
            saved.history_anchor(),
            &custody::load(&case.anchor).unwrap()
        );
        assert_eq!(saved.history_anchor().revision(), state.revision);
        assert_eq!(
            saved.work().checks,
            1 + u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(
            saved.work().reserved_read_bytes,
            saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
        );
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
        let historical = recovery_profile(&root, &source, None, "historical");
        topology::admit(
            &case.config,
            &mut loaded,
            Some(&historical),
            topology::Mode::Historical,
        )
        .unwrap();
        let (host, _, forecast, _) = recovery::open_selected(&case.config, &loaded).unwrap();
        assert!(forecast.is_none());
        numerical(&host, 4, 2);
    }
}

#[test]
fn ordinary_mediated_preparation_cannot_reuse_retained_source_after_current_input_loss() {
    let root = Root::new();
    let case = selected(&root);
    let original = prepare(&root, &case);
    next_source(&root);
    let source = root.0.join("failed.topology");
    let retained = root.0.join("failed.anchor");
    let profile = recovery_profile(
        &root,
        &root.0.join("source-1.bin"),
        Some(&source),
        "missing-source",
    );
    let (peer_path, peers) = functional_creation::peers_file(&root, &case.config);
    let args = vec![
        "create-learned-generated".into(),
        case.config_path.display().to_string(),
        case.recipe.display().to_string(),
        peer_path.display().to_string(),
        "--continue-generation".into(),
        "--history-anchor".into(),
        case.anchor.display().to_string(),
        "--retain-history-anchor".into(),
        retained.display().to_string(),
        "--topology-profile".into(),
        profile.display().to_string(),
    ];
    let mut removed = false;
    let result = crate::workflow::actor_service::learned::command_with_clock(&args, None, || {
        assert!(
            !peers.socket(1).exists(),
            "source loss must precede human review"
        );
        // Let the exact recovery Fence and fresh source rebind succeed first.
        // Loss must be observed by the first real current-source check, not by
        // an unrelated startup profile or asset refusal.
        if !removed && view(&case).control.ledger.epoch == 3 {
            fs::remove_file(root.0.join("inventory-2.json")).unwrap();
            removed = true;
        }
        ElapsedTick(1001)
    });
    assert!(
        removed,
        "source loss was injected after the original rebind"
    );
    assert!(result.is_err());
    let state = view(&case);
    assert!(state.stop.is_some());
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 0);
    let saved = checkpoint(&source);
    assert_eq!(saved.history_anchor(), &custody::load(&retained).unwrap());
    assert_eq!(saved.work().checks, original.work().checks + 1);
    assert_eq!(
        saved.work().reserved_read_bytes,
        saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
    );
    assert!(!saved.available());
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&retained).unwrap());
    let historical = recovery_profile(&root, &source, None, "historical");
    topology::admit(
        &case.config,
        &mut loaded,
        Some(&historical),
        topology::Mode::Historical,
    )
    .unwrap();
    let (host, _, forecast, _) = recovery::open_selected(&case.config, &loaded).unwrap();
    assert!(forecast.is_none());
    numerical(&host, 2, 0);
}

#[test]
fn ordinary_mediated_preparation_requires_separate_retained_history_and_source_outputs() {
    let root = Root::new();
    let case = selected(&root);
    let path = root.0.join("create-topology.json");
    let original = fs::read_to_string(&path).unwrap();
    // Use the admitted v1 no-output form, rather than an invalid v2 document.
    let without_output = format!(
        r#"{{"schema_version":"fa.mediated-create/1","topology_path":"{}","inventory_path":"{}"}}"#,
        root.0.join("live-1.json").display(),
        root.0.join("inventory-1.json").display()
    );
    for document in [
        without_output,
        original.replace(
            root.0.join("source-1.bin").to_str().unwrap(),
            case.anchor.to_str().unwrap(),
        ),
    ] {
        fs::write(&path, document).unwrap();
        let error = prepare::command(
            &preparation_args(&root, &case),
            None,
            || panic!("missing or aliased custody must refuse before time and creation"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(
            error.contains("new retained topology checkpoint")
                || error.contains("distinct new files"),
            "{error}"
        );
        assert!(!case.config.store.exists());
        assert!(!case.anchor.exists());
        assert!(!root.0.join("source-1.bin").exists());
    }
    fs::write(path, original).unwrap();
    prepare(&root, &case);
}
