//! An opted-in failed service retains the native acknowledged owner/source cut.
//! Retention is independent evidence custody, never another publication attempt.
use super::*;
use crate::workflow::actor_service::series::{self as service, Options};

const OCCUPIED_OUTPUT: &[u8] = b"independently retained file; must not be replaced";

fn fail_after_charged_dispatch(mode: &str, occupied_output: bool) -> String {
    let root = Root::new();
    let config = configuration(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let archive = root.path("source.bin");
    let selected = retained_selection(&root, &archive);
    let original_topology = fs::read(root.path("topology.json")).unwrap();
    let submitted = proposal_document(&config);
    let actor_peer = actor.clone();
    let peer = std::thread::spawn(move || {
        let response = exchange(&mut connect_actor(&actor_peer), &submitted);
        assert_eq!(
            field(request_knowledge(&response), "state").as_str(),
            Some("pending")
        );
    });
    let human = human(reviewer.clone(), ReviewDecision::Approve);
    let store = config.store.clone();
    let profile = config.profile.clone();
    let mut observed_dispatch = false;
    let result = serve_selected_ingress(
        config,
        &actor,
        &reviewer,
        None,
        false,
        None,
        Some(selected),
        None,
        Some(GENERATION),
        None,
        || {
            if !observed_dispatch
                && let Ok(state) = FileOversight::read_publication(&store, &profile)
                && state.executions == 0
                && state.control.ledger.charged == 7
            {
                observed_dispatch = true;
                match mode {
                    "missing" => fs::remove_file(root.path("inventory.json")).unwrap(),
                    "changed" => fs::write(root.path("topology.json"), graph(true)).unwrap(),
                    _ => unreachable!(),
                }
                // This file appears only after every preflight succeeded. The
                // later retention attempt must preserve this independent file.
                if occupied_output {
                    fs::write(&archive, OCCUPIED_OUTPUT).unwrap();
                }
            }
            ElapsedTick(1000)
        },
    );
    peer.join().unwrap();
    human.join().unwrap();
    assert!(observed_dispatch, "{mode}: {result:?}");
    let error = result.unwrap_err();
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(state.executions, 0, "{mode}: {error}");
    assert_eq!(state.payload, b"initial");
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(
        state.control.ledger.available
            + state.control.ledger.reserved
            + state.control.ledger.charged,
        profile.delivery.total
    );
    if state.control.ledger.charged == 0 {
        assert_eq!(
            state.control.ledger.stages[&1],
            ActionState::ConfirmedNotExecuted
        );
    } else {
        assert_eq!(state.control.ledger.charged, 7);
        assert!(matches!(
            state.control.ledger.stages[&1],
            ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown
        ));
    }
    assert!(state.stop.is_some());
    if occupied_output {
        assert_eq!(fs::read(&archive).unwrap(), OCCUPIED_OUTPUT);
        assert!(
            error.contains("topology checkpoint retention failed:"),
            "{error}"
        );
        assert!(error.contains("acknowledged work is not undone"), "{error}");
    } else {
        assert!(
            !error.contains("topology checkpoint retention failed:"),
            "{error}"
        );
        let saved = TopologyCheckpoint::read_trusted(
            &mut Cursor::new(fs::read(&archive).unwrap()),
            MAX_TOPOLOGY_CHECKPOINT_BYTES,
        )
        .unwrap();
        assert_eq!(saved.history_anchor().revision(), state.revision);
        assert_eq!(saved.minimum().control_sequence, state.control.sequence);
        assert_eq!(saved.minimum().authority_epoch, state.control.ledger.epoch);
        assert_eq!(saved.work().checks, 2);
        assert_eq!(
            saved.work().reserved_read_bytes,
            2 * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert_eq!(saved.last_operation(), 2);
        assert!(!saved.available());
        assert_eq!(saved.topology_bytes(), original_topology);
        assert_eq!(saved.inventory_bytes(), INVENTORY);
    }
    match mode {
        "missing" => assert!(!root.path("inventory.json").exists()),
        "changed" => assert_eq!(
            fs::read(root.path("topology.json")).unwrap(),
            graph(true).as_bytes()
        ),
        _ => unreachable!(),
    }
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
    error
        .split("; helper cleanup pending=")
        .next()
        .unwrap()
        .to_owned()
}

#[test]
fn tower_creation_retains_failed_source_custody_and_preserves_both_independent_errors() {
    for mode in ["missing", "changed"] {
        let original_failure = fail_after_charged_dispatch(mode, false);
        let failed_retention = fail_after_charged_dispatch(mode, true);
        // The same real native publication failure remains the leading cause;
        // retention neither masks it nor reports its own failure as execution.
        assert!(
            failed_retention.starts_with(&format!(
                "{original_failure}; topology checkpoint retention failed:"
            )),
            "original={original_failure}; combined={failed_retention}"
        );
    }
}

#[test]
fn topology_creation_refusal_before_native_custody_creates_no_checkpoint() {
    for scheduled in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let archive = root.path("source.bin");
        let selected = retained_selection(&root, &archive);
        fs::remove_file(root.path("inventory.json")).unwrap();
        let store = config.store.clone();
        let result = if scheduled {
            service::serve_tower_with_topology(
                config,
                &actor,
                &reviewer,
                None,
                Options {
                    requests: &[7, 8],
                    open: false,
                    credibility: None,
                },
                GENERATION,
                None,
                selected,
                || ElapsedTick(1000),
            )
        } else {
            serve_selected_ingress(
                config,
                &actor,
                &reviewer,
                None,
                false,
                None,
                Some(selected),
                None,
                Some(GENERATION),
                None,
                || ElapsedTick(1000),
            )
        };
        assert!(result.is_err(), "scheduled={scheduled}");
        assert!(!archive.exists());
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(7).exists());
        assert!(!reviewer.socket(8).exists());
    }
}
