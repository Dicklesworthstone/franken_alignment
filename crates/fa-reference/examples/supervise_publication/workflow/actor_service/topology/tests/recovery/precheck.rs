//! Recovered disclosure uses the original post-Fence owner and cumulative source.
//! Real sockets and synthetic reviewers exercise enforcement, not model quality.
use super::*;
use crate::workflow::actor_service::precheck::Settings as PrecheckSettings;
use fa_reference::action::consequence::delivery::persistent::{
    observed::reviewer::peer::control::StopClientProgress,
    requests::actor::{
        FilePrecheckOutcome,
        precheck_wire::{
            PrecheckRequest, PrecheckWireError, PrecheckWireResponse, client::PrecheckPeerClient,
        },
    },
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;

fn settings(root: &Root, version: u8, queries: usize, epoch: u64) -> PrecheckSettings {
    let disclosure = if version == 3 {
        "full"
    } else {
        "exact_predicates"
    };
    let cache = if version == 1 {
        ""
    } else {
        ",\"cache_entries\":4,\"cache_bytes\":262144"
    };
    let leakage = if version == 3 {
        ",\"leakage\":{\"policy_structure\":\"disclosed\",\"policy_literals\":\"disclosed\",\"source_values\":\"withheld\",\"helper_feedback\":\"withheld\",\"consequences\":\"withheld\",\"timing\":\"observable\",\"query_usage\":\"observable\"}"
    } else {
        ""
    };
    let text = format!(
        r#"{{"schema":"fa.actor-precheck-service/{version}","policy_disclosure":"{disclosure}","socket":"{}","expected_policy_epoch":{epoch},"disclosure_id":81,"query_limit":{queries},"exchange_limit":8,"candidate_limit":8,"connection_limit":4,"runtime_ms":15000{cache}{leakage}}}"#,
        root.path("actors/precheck.sock").display(),
    );
    fs::write(root.path("precheck.json"), &text).unwrap();
    PrecheckSettings::decode(text.as_bytes()).unwrap()
}

fn candidate(config: &Config) -> ActorProposal {
    let mut target = config.profile.delivery.target;
    target.expected_version = 2;
    ActorProposal {
        target,
        payload: b"second".to_vec(),
        units: 6,
        deadline: ElapsedTick(100000),
        expected_policy_epoch: 3,
    }
}

fn query(
    actor: &Profile,
    settings: &PrecheckSettings,
    proposal: &ActorProposal,
) -> PrecheckWireResponse {
    let mut client = PrecheckPeerClient::connect(
        connect(settings.socket_path()),
        actor.supervisor,
        PrecheckRequest {
            query: 7,
            proposal: proposal.clone(),
        },
        proposal.expected_policy_epoch,
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "missing recovered precheck response"
        );
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() {
            return response;
        }
        assert!(
            !report.status.closed(),
            "closed without native precheck response"
        );
        pause(1);
    }
}

fn passed(response: &PrecheckWireResponse) {
    assert_eq!(response.query, Some(7));
    let Knowledge::Known { value, basis } = response.result.as_ref().unwrap() else {
        panic!("missing exact-policy observation");
    };
    assert_eq!(*value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!(
        (basis.query, basis.disclosure, basis.policy_epoch),
        (7, 81, 3)
    );
    assert_eq!(basis.observed_at, ElapsedTick(1001));
    assert!(
        String::from_utf8(response.encode().unwrap())
            .unwrap()
            .contains("\"review_required\":true")
    );
}

fn args(root: &Root, checked: bool, operator: bool) -> Vec<String> {
    let mut args = arguments(root, checked, true, operator);
    args.extend([
        "--precheck-profile".into(),
        root.path("precheck.json").display().to_string(),
    ]);
    args
}

fn retained(
    root: &Root,
    before: &TopologyCheckpoint,
    revision: u64,
    added_checks: u64,
) -> TopologyCheckpoint {
    let saved = read_checkpoint(root, "second-checkpoint.bin");
    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(root.path("second.anchor")).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    assert_eq!(saved.history_anchor(), &anchor);
    assert_eq!(anchor.revision(), revision);
    assert_eq!(saved.work().checks, before.work().checks + added_checks);
    assert_eq!(
        saved.work().reserved_read_bytes,
        before.work().reserved_read_bytes + added_checks * TOPOLOGY_REFRESH_READ_BYTES
    );
    assert_eq!(read_checkpoint(root, "first-checkpoint.bin"), *before);
    saved
}

#[test]
fn all_recovered_disclosures_keep_one_finite_grant_and_publish_with_both_keys() {
    for checked in [false, true] {
        for version in [1, 2, 3] {
            let root = Root::new();
            let (_, reviewer, before) = seed(&root, checked, 32);
            let config = helper_epoch(&root, 3);
            let actor = actor_profile(&root, 8);
            next_graph(&root, 2, 32);
            if checked {
                second_captures(&root, &config);
            }
            let operator = operator(&root, &config);
            let selected = settings(&root, version, 2, 3);
            let proposed = candidate(&config);
            let submit = proposal(&config, 8, 3, 2);
            let poll = request_document(&config, 8, "get");
            let situation = situation_document(&config);
            let store = config.store.clone();
            let bootstrap = config.profile.clone();
            let review_socket = reviewer.socket(8);
            let actor_peer = actor.clone();
            let operator_peer = operator.clone();
            let (offered_send, offered_receive) = mpsc::channel();
            let (release_send, release_receive) = mpsc::channel();
            let human = review(
                reviewer.clone(),
                8,
                Some(offered_send),
                Some(release_receive),
            );
            let peer = std::thread::spawn(move || {
                passed(&query(&actor_peer, &selected, &proposed));
                let unchanged = FileOversight::read_publication(&store, &bootstrap).unwrap();
                assert_eq!(unchanged.executions, 1);
                assert_eq!(unchanged.control.ledger.stages.len(), 1);
                assert_eq!(unchanged.control.ledger.epoch, 3);
                assert!(!review_socket.exists(), "precheck acquired the human key");
                passed(&query(&actor_peer, &selected, &proposed));
                assert_eq!(
                    query(&actor_peer, &selected, &proposed).result,
                    Err(PrecheckWireError::Capacity)
                );
                assert_eq!(
                    FileOversight::read_publication(&store, &bootstrap).unwrap(),
                    unchanged
                );
                let mut operator = connect(&operator_peer.socket);
                operator_peer.supervisor.verify(&operator).unwrap();
                counts(
                    &exchange(&mut operator, &situation),
                    &[
                        ("attempts", "1"),
                        ("executions", "1"),
                        ("charged_units", "7"),
                    ],
                );
                let mut socket = connect(&actor_peer.socket);
                actor_peer.supervisor.verify(&socket).unwrap();
                assert_eq!(
                    field(knowledge(&exchange(&mut socket, &submit)), "state").as_str(),
                    Some("pending")
                );
                offered_receive
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                assert_eq!(
                    FileOversight::read_publication(&store, &bootstrap)
                        .unwrap()
                        .executions,
                    1
                );
                assert!(
                    !selected.socket_path().exists(),
                    "disclosure survived into effect review"
                );
                release_send.send(()).unwrap();
                assert_eq!(
                    field(knowledge(&terminal(&mut socket, &poll)), "value").as_str(),
                    Some("executed")
                );
            });
            let result =
                command_with_clock(&args(&root, checked, true), None, || ElapsedTick(1001));
            peer.join().unwrap();
            human.join().unwrap();
            assert!(
                result.is_ok(),
                "checked={checked}, disclosure={version}: {result:?}"
            );
            let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
            assert_eq!(state.executions, 2);
            assert_eq!(state.payload, b"second");
            assert_eq!(
                (state.control.ledger.charged, state.control.ledger.reserved),
                (13, 0)
            );
            assert_eq!(
                state.control.ledger.available + state.control.ledger.charged,
                config.profile.delivery.total
            );
            assert!(state.stop.is_none());
            let saved = retained(&root, &before, state.revision, 2);
            assert!(saved.available());
            assert_eq!(saved.expected().graph().spec().generation, 2);
            assert_eq!(saved.expected().graph().spec().inventory_generation, 2);
            assert!(saved.last_operation() > before.last_operation());
            assert!(!actor.socket.exists());
            assert!(!operator.socket.exists());
            assert!(!root.path("actors/precheck.sock").exists());
        }
    }
}

#[test]
fn recovered_precheck_pass_cannot_replace_fresh_actor_evidence() {
    let root = Root::new();
    let (_, reviewer, before) = seed(&root, false, 32);
    let config = helper_epoch(&root, 3);
    let actor = actor_profile(&root, 8);
    next_graph(&root, 2, 32);
    let selected = settings(&root, 2, 2, 3);
    let proposed = candidate(&config);
    let submit = proposal(&config, 8, 3, 2);
    let evidence_path = root.path("evidence.json");
    let changed = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 2,
            scope: config.profile.delivery.scope,
        },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"blocked".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|name| {
                (
                    name.to_owned(),
                    format!("review this fixture: {name}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap()
    .encode();
    let actor_peer = actor.clone();
    let peer = std::thread::spawn(move || {
        passed(&query(&actor_peer, &selected, &proposed));
        fs::write(evidence_path, changed).unwrap();
        let response = exchange(&mut connect(&actor_peer.socket), &submit);
        assert_eq!(field(knowledge(&response), "state").as_str(), Some("known"));
        assert_eq!(
            field(knowledge(&response), "value").as_str(),
            Some("denied")
        );
    });
    let result = command_with_clock(&args(&root, false, false), None, || ElapsedTick(1001));
    peer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"network");
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(state.control.ledger.stages[&2], ActionState::Denied);
    assert!(!reviewer.socket(8).exists());
    retained(&root, &before, state.revision, 1);
}

#[test]
fn recovered_precheck_keeps_final_topology_and_checked_witness_gates_after_dispatch() {
    for fault in ["missing-inventory", "bypass", "changed-witness"] {
        let checked = fault == "changed-witness";
        let root = Root::new();
        let (_, reviewer, before) = seed(&root, checked, 32);
        let config = helper_epoch(&root, 3);
        let actor = actor_profile(&root, 8);
        next_graph(&root, 2, 32);
        let action = checked.then(|| second_captures(&root, &config));
        let selected = settings(&root, 1, 2, 3);
        let proposed = candidate(&config);
        let submit = proposal(&config, 8, 3, 2);
        let actor_peer = actor.clone();
        let peer = std::thread::spawn(move || {
            passed(&query(&actor_peer, &selected, &proposed));
            let response = exchange(&mut connect(&actor_peer.socket), &submit);
            assert_eq!(
                field(knowledge(&response), "state").as_str(),
                Some("pending")
            );
        });
        let human = review(reviewer.clone(), 8, None, None);
        let mut changed = false;
        let result = command_with_clock(&args(&root, checked, false), None, || {
            if !changed
                && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
                && state.executions == 1
                && state.control.ledger.charged == 13
            {
                changed = true;
                match fault {
                    "missing-inventory" => fs::remove_file(root.path("inventory.json")).unwrap(),
                    "bypass" => {
                        let text = graph(true)
                            .replacen("\"generation\":1,", "\"generation\":2,", 1)
                            .replace("\"inventory_generation\":1", "\"inventory_generation\":2")
                            .replace(
                                "\"complete_inventory_generation\":1",
                                "\"complete_inventory_generation\":2",
                            );
                        fs::write(root.path("topology.json"), text).unwrap();
                    }
                    _ => write_capture(
                        &root,
                        "current.bin",
                        &second_capture(action.as_ref().unwrap(), &[0, 1, 2, 4]),
                    ),
                }
            }
            ElapsedTick(1001)
        });
        peer.join().unwrap();
        human.join().unwrap();
        assert!(changed, "{fault}: {result:?}");
        if !checked {
            assert!(result.is_err(), "{fault}: {result:?}");
        }
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 1, "{fault}: {result:?}");
        assert_eq!(state.payload, b"network");
        assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
        assert_eq!(state.control.ledger.reserved, 0);
        if state.control.ledger.charged == 7 {
            assert_eq!(
                state.control.ledger.stages[&2],
                ActionState::ConfirmedNotExecuted
            );
        } else {
            assert_eq!(state.control.ledger.charged, 13);
            assert!(matches!(
                state.control.ledger.stages[&2],
                ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown
            ));
        }
        assert_eq!(
            state.control.ledger.available + state.control.ledger.charged,
            config.profile.delivery.total
        );
        let saved = retained(&root, &before, state.revision, 2);
        if !checked {
            assert!(!saved.available());
        }
        assert!(!actor.socket.exists());
        assert!(!root.path("actors/precheck.sock").exists());
    }
}

#[test]
fn partial_recovered_precheck_and_actor_frames_yield_to_independent_stop() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, true, 32);
    let actor = actor_profile(&root, 8);
    next_graph(&root, 2, 32);
    let selected = settings(&root, 1, 2, 3);
    let proposed = candidate(&config);
    let evidence_path = root.path("evidence.json");
    let inventory_path = root.path("inventory.json");
    let submit = proposal(&config, 8, 3, 2);
    let actor_peer = actor.clone();
    let stop_peer = reviewer.clone();
    let peer = std::thread::spawn(move || {
        passed(&query(&actor_peer, &selected, &proposed));
        fs::remove_file(evidence_path).unwrap();
        fs::remove_file(inventory_path).unwrap();
        let mut partial = connect(selected.socket_path());
        partial.write_all(b"{").unwrap();
        let mut actor = connect(&actor_peer.socket);
        actor
            .write_all(&submit.as_bytes()[..submit.len() / 2])
            .unwrap();
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_peer, 8, &mut output).unwrap();
        assert_eq!(
            field(&parse(&output), "status").as_str(),
            Some("stopped_drained")
        );
    });
    let result = command_with_clock(&args(&root, true, false), None, || ElapsedTick(1001));
    peer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(state.stop.unwrap().request().operation, 8);
    retained(&root, &before, state.revision, 1);
    assert!(!root.path("evidence.json").exists());
    assert!(!root.path("inventory.json").exists());
    assert!(!root.path("actors/precheck.sock").exists());
    assert!(!actor.socket.exists());
}

#[test]
fn replay_stop_precedes_recovered_disclosure_and_needs_no_live_source() {
    for checked in [false, true] {
        let root = Root::new();
        let (config, reviewer, before) = seed(&root, checked, 32);
        let actor = actor_profile(&root, 8);
        settings(&root, 1, 2, 3);
        for name in [
            "topology.json",
            "inventory.json",
            "evidence.json",
            "original.bin",
            "current.bin",
        ] {
            let path = root.path(name);
            if path.exists() {
                fs::remove_file(path).unwrap();
            }
        }
        let mut calls = 0;
        let mut peer = None;
        let error = command_with_clock(&args(&root, checked, false), None, || {
            calls += 1;
            if calls == 2 {
                // Queue the authenticated peer after the independent listener
                // exists, before the first cooperative replay checkpoint.
                let mut client = reviewer.connect_stop_client(8).unwrap();
                peer = Some(std::thread::spawn(move || {
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(10));
                        match client.step().unwrap() {
                            StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                            StopClientProgress::Complete => return client.receipt().unwrap(),
                            _ => pause(1),
                        }
                    }
                }));
            }
            ElapsedTick(1001)
        })
        .unwrap_err();
        assert!(
            error.contains("native admission Stop acknowledged"),
            "{error}"
        );
        assert!(peer.unwrap().join().unwrap().acknowledged());
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.revision, before.history_anchor().revision() + 2);
        assert_eq!(state.executions, 1);
        assert_eq!(
            (state.control.ledger.charged, state.control.ledger.reserved),
            (7, 0)
        );
        assert_eq!(state.stop.unwrap().request().operation, 8);
        let saved = retained(&root, &before, state.revision, 0);
        assert_eq!(saved.last_operation(), before.last_operation());
        assert_eq!(saved.topology_bytes(), before.topology_bytes());
        assert_eq!(saved.inventory_bytes(), before.inventory_bytes());
        assert!(!saved.available());
        assert!(!root.path("actors/precheck.sock").exists());
        assert!(!actor.socket.exists());
    }
}

#[test]
fn recovered_disclosure_requires_the_explicit_post_fence_policy_epoch() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, false, 32);
    let actor = actor_profile(&root, 8);
    next_graph(&root, 2, 32);
    settings(&root, 1, 2, 0);
    fs::remove_file(root.path("evidence.json")).unwrap();
    let error =
        command_with_clock(&args(&root, false, false), None, || ElapsedTick(1001)).unwrap_err();
    assert!(
        error.contains("precheck expected policy epoch does not match the original owner"),
        "{error}"
    );
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.stages.len(), 1);
    // A failed live startup still follows the ordinary native Stop cleanup;
    // the selected disclosure epoch is never rewritten to make it succeed.
    assert_eq!(state.stop.unwrap().request().operation, 8);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (7, 0)
    );
    retained(&root, &before, state.revision, 1);
    assert!(!root.path("actors/precheck.sock").exists());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn recorded_recovery_refuses_a_new_disclosure_without_loading_live_assets() {
    for checked in [false, true] {
        let root = Root::new();
        let (config, reviewer, before) = seed(&root, checked, 32);
        let actor = actor_profile(&root, 7);
        settings(&root, 1, 2, 2);
        for name in [
            "topology.json",
            "inventory.json",
            "evidence.json",
            "original.bin",
            "current.bin",
        ] {
            let path = root.path(name);
            if path.exists() {
                fs::remove_file(path).unwrap();
            }
        }
        let error = command_with_clock(&args(&root, checked, false), None, || ElapsedTick(1001))
            .unwrap_err();
        assert!(error.contains("recorded requests are observation-only and cannot acquire a new precheck disclosure"), "{error}");
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 1);
        assert_eq!(state.payload, b"network");
        assert_eq!(state.control.ledger.stages.len(), 1);
        assert_eq!(
            (state.control.ledger.charged, state.control.ledger.reserved),
            (7, 0)
        );
        let saved = retained(&root, &before, state.revision, 0);
        assert_eq!(saved.last_operation(), before.last_operation());
        assert_eq!(saved.topology_bytes(), before.topology_bytes());
        assert_eq!(saved.inventory_bytes(), before.inventory_bytes());
        assert!(!saved.available());
        assert!(!root.path("actors/precheck.sock").exists());
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(7).exists());
    }
}

#[test]
fn recovered_precheck_rejects_archive_endpoint_source_and_store_aliases_before_fence() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, false, 32);
    let actor = actor_profile(&root, 8);
    let config_text = fs::read_to_string(root.path("config.json")).unwrap();
    let actor_text = fs::read_to_string(root.path("actor.json")).unwrap();
    let journal = fs::read(config.store.join("delivery.bin")).unwrap();
    fs::write(
        config.store.join("delivery.pending"),
        b"untouched recovery staging",
    )
    .unwrap();
    std::os::unix::fs::symlink(&root.0, root.path("via")).unwrap();
    for alias in [false, true] {
        for destination in [
            "history",
            "checkpoint",
            "actor",
            "reviewer",
            "stop",
            "operator",
            "topology",
            "inventory",
            "evidence",
            "store",
        ] {
            fs::write(root.path("config.json"), &config_text).unwrap();
            fs::write(root.path("actor.json"), &actor_text).unwrap();
            guard_profile(&root, &before, "second.anchor");
            continuation_profile(&root, Some("second-checkpoint.bin"));
            let operator = operator(&root, &config);
            let selected = settings(&root, 1, 2, 3);
            let selected_path = if alias {
                root.path("via/actors/precheck.sock")
            } else {
                selected.socket_path().to_path_buf()
            };
            match destination {
                "history" => guard_profile(
                    &root,
                    &before,
                    if alias {
                        "via/actors/precheck.sock"
                    } else {
                        "actors/precheck.sock"
                    },
                ),
                "checkpoint" => continuation_profile(
                    &root,
                    Some(if alias {
                        "via/actors/precheck.sock"
                    } else {
                        "actors/precheck.sock"
                    }),
                ),
                "actor" | "operator" => {
                    let (file, original) = if destination == "actor" {
                        ("actor.json", &actor.socket)
                    } else {
                        ("operator.json", &operator.socket)
                    };
                    let text = fs::read_to_string(root.path(file)).unwrap().replace(
                        &original.display().to_string(),
                        &selected_path.display().to_string(),
                    );
                    fs::write(root.path(file), text).unwrap();
                }
                "topology" | "inventory" => {
                    let original = root.path(&format!("{destination}.json"));
                    let text = fs::read_to_string(root.path("continuation.json"))
                        .unwrap()
                        .replace(
                            &original.display().to_string(),
                            &selected_path.display().to_string(),
                        );
                    fs::write(root.path("continuation.json"), text).unwrap();
                }
                "evidence" => {
                    let text = config_text.replace(
                        &config.source.path().display().to_string(),
                        &selected_path.display().to_string(),
                    );
                    fs::write(root.path("config.json"), text).unwrap();
                }
                _ => {
                    let collision = match destination {
                        "reviewer" => reviewer.socket(8),
                        "stop" => crate::workflow::control::socket_path(&reviewer, 8),
                        _ => config.store.join("precheck.sock"),
                    };
                    let collision = if alias {
                        root.path("via")
                            .join(collision.strip_prefix(&root.0).unwrap())
                    } else {
                        collision
                    };
                    let text = fs::read_to_string(root.path("precheck.json"))
                        .unwrap()
                        .replace(
                            &selected.socket_path().display().to_string(),
                            &collision.display().to_string(),
                        );
                    fs::write(root.path("precheck.json"), text).unwrap();
                }
            }
            let error = command_with_clock(&args(&root, false, true), None, || {
                panic!("{destination}, alias={alias} crossed preflight into time/recovery")
            })
            .unwrap_err();
            assert!(
                error.contains("distinct") || error.contains("outside the authority store"),
                "{destination}, alias={alias}: {error}"
            );
            assert_eq!(
                fs::read(config.store.join("delivery.bin")).unwrap(),
                journal
            );
            assert_eq!(
                fs::read(config.store.join("delivery.pending")).unwrap(),
                b"untouched recovery staging"
            );
            assert!(!root.path("second.anchor").exists());
            assert!(!root.path("second-checkpoint.bin").exists());
            assert!(!root.path("actors/precheck.sock").exists());
            assert!(!actor.socket.exists());
        }
    }
}
