//! Two real effects through one mediated owner; each later effect still buys
//! its own source check, fresh independent review and one-shot publication keys.
#[path = "series/cancellation.rs"]
mod cancellation;

use super::*;
use crate::workflow::actor_service::series::{self as service, Options};

fn document(config: &Config, request: u64, version: u64, payload: &str) -> String {
    proposal_document(config)
        .replace("\"request\":\"7\"", &format!("\"request\":\"{request}\""))
        .replace(
            "\"idempotency_key\":\"7\"",
            &format!("\"idempotency_key\":\"{request}\""),
        )
        .replace(
            &format!(
                "\"expected_version\":\"{}\"",
                config.profile.delivery.target.expected_version
            ),
            &format!("\"expected_version\":\"{version}\""),
        )
        .replace("6e6574776f726b", payload)
}

fn poll(config: &Config, request: u64) -> String {
    request_document(config, "get").replace("-7@", &format!("-{request}@"))
}

fn finish(socket: &mut UnixStream, proposal: &str, poll: &str) -> Json {
    let response = exchange(socket, proposal);
    let observed = request_knowledge(&response);
    match field(observed, "state").as_str() {
        Some("pending") => terminal(socket, poll),
        _ => response,
    }
}

fn assert_executed(response: &Json) {
    let observed = request_knowledge(response);
    assert_eq!(field(observed, "state").as_str(), Some("known"));
    assert_eq!(field(observed, "value").as_str(), Some("executed"));
}

fn human_for(profile: PeerProfile, request: u64) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = profile
            .connect_client_waiting(request, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

fn selected_checks(root: &Root, output: &Path, checks: u64) -> Selection {
    let selected = retained_selection(root, output);
    let text = graph(false)
        .replace("\"checks\":32", &format!("\"checks\":{checks}"))
        .replace(
            &format!("\"read_bytes\":{}", 32 * TOPOLOGY_REFRESH_READ_BYTES),
            &format!("\"read_bytes\":{}", checks * TOPOLOGY_REFRESH_READ_BYTES),
        );
    fs::write(root.path("topology.json"), text).unwrap();
    selected
}

fn action_for(
    root: &Root,
    config: &Config,
    attempt: u64,
    version: u64,
    payload: &[u8],
) -> FrozenAction {
    let mut profile = config.profile.clone();
    profile.delivery.target.expected_version = version;
    let target = profile.delivery.target;
    let (mut host, _) =
        FileOversight::create(root.path(&format!("capture-owner-{attempt}")), profile).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000))
        .unwrap();
    host.propose(
        host.revision(),
        attempt,
        ActionSpec {
            version: VERSION,
            scope: config.profile.delivery.scope,
            target: Some(target),
            payload: payload.to_vec(),
            required_witnesses: Vec::new(),
            policy_epoch: 0,
            deadline: ElapsedTick(100000),
            units: 7,
        },
        snapshot(),
    )
    .unwrap()
}

fn write_pair(root: &Root, action: &FrozenAction) {
    let original = capture(action, 1, &[0, 2, 4]);
    write_capture(root, "original.bin", &original);
    write_capture(root, "current.bin", &original);
}

#[test]
fn topology_tower_schedule_publishes_two_versions_with_one_cumulative_source_and_operator() {
    for checked in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.connections = 1;
        actor.candidates = 1;
        let operator = operator_profile(&root, &config, 3, 0);
        let archive = root.path("sequence-source.bin");
        let selected = selected_checks(&root, &archive, 3);
        let publication = checked.then(|| publication(&root));
        let second_action = checked.then(|| {
            write_pair(&root, &action_for(&root, &config, 1, 1, b"first"));
            action_for(&root, &config, 2, 2, b"second")
        });
        let first = document(&config, 7, 1, "6669727374");
        let second = document(&config, 8, 2, "7365636f6e64");
        let first_poll = poll(&config, 7);
        let second_poll = poll(&config, 8);
        let situation = situation_document(&config);
        let actor_peer = actor.clone();
        let operator_peer = operator.clone();
        let next_control = crate::workflow::control::socket_path(&reviewer, 8);
        let root_path = root.0.clone();
        let peer = std::thread::spawn(move || {
            let mut operator = connect_operator(&operator_peer);
            counts(&exchange(&mut operator, &situation), &[("attempts", "0")]);
            let mut actor = connect_actor(&actor_peer);
            assert_executed(&finish(&mut actor, &first, &first_poll));
            wait(&next_control);
            counts(
                &exchange(&mut operator, &situation),
                &[
                    ("attempts", "1"),
                    ("executions", "1"),
                    ("charged_units", "7"),
                ],
            );
            if let Some(action) = second_action {
                let fixture = capture(&action, 1, &[0, 2, 4]);
                let original = FilePublicationCapture::new(
                    2,
                    fixture.identity(),
                    &action,
                    fixture.inputs().clone(),
                )
                .unwrap()
                .to_bytes()
                .unwrap();
                fs::write(root_path.join("original.bin"), &original).unwrap();
                fs::write(root_path.join("current.bin"), original).unwrap();
            }
            assert_executed(&finish(&mut actor, &second, &second_poll));
            // A retry and an operator read cannot renew topology/source custody.
            for name in ["topology.json", "inventory.json", "evidence.json"] {
                fs::remove_file(root_path.join(name)).unwrap();
            }
            assert_executed(&exchange(&mut actor, &first));
            assert_executed(&exchange(&mut actor, &second));
            counts(
                &exchange(&mut operator, &situation),
                &[
                    ("attempts", "2"),
                    ("executions", "2"),
                    ("charged_units", "14"),
                ],
            );
            let exhausted = exchange(&mut operator, &situation);
            assert_eq!(
                field(field(&exhausted, "envelope"), "reason_code").as_str(),
                Some("BudgetExhausted")
            );
        });
        let first_human = human_for(reviewer.clone(), 7);
        let second_human = human_for(reviewer.clone(), 8);
        let result = service::serve_tower_with_topology(
            config,
            &actor,
            &reviewer,
            publication.as_ref(),
            Options {
                requests: &[7, 8],
                open: false,
                credibility: None,
            },
            GENERATION,
            Some(&operator),
            selected,
            || ElapsedTick(1000),
        );
        peer.join().unwrap();
        first_human.join().unwrap();
        second_human.join().unwrap();
        assert!(result.is_ok(), "checked={checked}: {result:?}");
        let config = configuration(&root);
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(
            (state.executions, state.payload.as_slice()),
            (2, b"second".as_slice())
        );
        assert_eq!(state.target.expected_version, 3);
        assert_eq!(
            (state.control.ledger.charged, state.control.ledger.reserved),
            (14, 0)
        );
        assert_eq!(
            state.control.ledger.available,
            config.profile.delivery.total - 14
        );
        assert_eq!(state.control.ledger.stages.len(), 2);
        assert!(state.stop.is_none());
        let saved = TopologyCheckpoint::read_trusted(
            &mut Cursor::new(fs::read(archive).unwrap()),
            MAX_TOPOLOGY_CHECKPOINT_BYTES,
        )
        .unwrap();
        assert_eq!(saved.history_anchor().revision(), state.revision);
        assert_eq!(saved.work().checks, 3);
        assert_eq!(
            saved.work().reserved_read_bytes,
            3 * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert!(!actor.socket.exists());
        assert!(!operator.socket.exists());
        assert!(!reviewer.socket(7).exists());
        assert!(!reviewer.socket(8).exists());
    }
}

#[test]
fn topology_tower_schedule_rechecks_second_dispatch_and_preserves_the_first_real_effect() {
    for mode in ["unchanged", "missing", "changed"] {
        let root = Root::new();
        let config = configuration(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let archive = root.path("source.bin");
        let selected = selected_checks(&root, &archive, 3);
        let original_topology = fs::read(root.path("topology.json")).unwrap();
        let first = document(&config, 7, 1, "6669727374");
        let second = document(&config, 8, 2, "7365636f6e64");
        let first_poll = poll(&config, 7);
        let actor_peer = actor.clone();
        let next_control = crate::workflow::control::socket_path(&reviewer, 8);
        let peer = std::thread::spawn(move || {
            let mut socket = connect_actor(&actor_peer);
            assert_executed(&finish(&mut socket, &first, &first_poll));
            wait(&next_control);
            assert_eq!(
                field(request_knowledge(&exchange(&mut socket, &second)), "state").as_str(),
                Some("pending")
            );
        });
        let first_human = human_for(reviewer.clone(), 7);
        let second_human = human_for(reviewer.clone(), 8);
        let store = config.store.clone();
        let profile = config.profile.clone();
        let mut observed_second_dispatch = false;
        let result = service::serve_tower_with_topology(
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
            || {
                if !observed_second_dispatch
                    && let Ok(state) = FileOversight::read_publication(&store, &profile)
                    && state.executions == 1
                    && state.control.ledger.charged == 14
                {
                    observed_second_dispatch = true;
                    match mode {
                        "missing" => fs::remove_file(root.path("inventory.json")).unwrap(),
                        "changed" => fs::write(root.path("topology.json"), graph(true)).unwrap(),
                        "unchanged" => {}
                        _ => unreachable!(),
                    }
                }
                ElapsedTick(1000)
            },
        );
        peer.join().unwrap();
        first_human.join().unwrap();
        second_human.join().unwrap();
        assert!(observed_second_dispatch, "{mode}: {result:?}");
        let state = FileOversight::read_publication(&store, &profile).unwrap();
        let succeeds = mode == "unchanged";
        assert_eq!(
            state.executions,
            if succeeds { 2 } else { 1 },
            "{mode}: {result:?}"
        );
        assert_eq!(
            state.payload,
            if succeeds {
                b"second".to_vec()
            } else {
                b"first".to_vec()
            }
        );
        assert_eq!(
            state.control.ledger.available
                + state.control.ledger.reserved
                + state.control.ledger.charged,
            profile.delivery.total
        );
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.stages.len(), 2);
        assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
        if succeeds {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(state.control.ledger.charged, 14);
        } else if state.control.ledger.charged == 7 {
            assert!(result.is_err(), "{mode}: {result:?}");
            assert_eq!(
                state.control.ledger.stages[&2],
                ActionState::ConfirmedNotExecuted
            );
        } else {
            assert!(result.is_err(), "{mode}: {result:?}");
            assert_eq!(state.control.ledger.charged, 14);
            assert!(matches!(
                state.control.ledger.stages[&2],
                ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown
            ));
        }
        let saved = TopologyCheckpoint::read_trusted(
            &mut Cursor::new(fs::read(&archive).unwrap()),
            MAX_TOPOLOGY_CHECKPOINT_BYTES,
        )
        .unwrap();
        assert_eq!(saved.history_anchor().revision(), state.revision);
        assert_eq!(saved.minimum().control_sequence, state.control.sequence);
        assert_eq!(saved.minimum().authority_epoch, state.control.ledger.epoch);
        assert_eq!(saved.work().checks, 3);
        assert_eq!(
            saved.work().reserved_read_bytes,
            3 * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert_eq!(saved.last_operation(), 3);
        assert_eq!(saved.available(), succeeds);
        assert_eq!(saved.topology_bytes(), original_topology);
        assert_eq!(saved.inventory_bytes(), INVENTORY);
    }
}

#[test]
fn topology_tower_handoff_does_not_replenish_the_source_check_allowance() {
    let root = Root::new();
    let config = configuration(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let archive = root.path("source.bin");
    let selected = selected_checks(&root, &archive, 2);
    let original_topology = fs::read(root.path("topology.json")).unwrap();
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let first_poll = poll(&config, 7);
    let actor_peer = actor.clone();
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let peer = std::thread::spawn(move || {
        let mut socket = connect_actor(&actor_peer);
        assert_executed(&finish(&mut socket, &first, &first_poll));
        wait(&next_control);
        assert_eq!(
            field(request_knowledge(&exchange(&mut socket, &second)), "state").as_str(),
            Some("pending")
        );
    });
    let first_human = human_for(reviewer.clone(), 7);
    let second_human = human_for(reviewer.clone(), 8);
    let store = config.store.clone();
    let profile = config.profile.clone();
    let result = service::serve_tower_with_topology(
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
    );
    peer.join().unwrap();
    first_human.join().unwrap();
    second_human.join().unwrap();
    assert!(result.is_err(), "{result:?}");
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(state.executions, 1, "{result:?}");
    assert_eq!(state.payload, b"first");
    assert_eq!(state.control.ledger.stages.len(), 2);
    assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
    assert_ne!(state.control.ledger.stages[&2], ActionState::Confirmed);
    assert_eq!(
        state.control.ledger.available
            + state.control.ledger.reserved
            + state.control.ledger.charged,
        profile.delivery.total
    );
    assert!(state.control.ledger.charged >= 7);
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
    assert_eq!(saved.last_operation(), 3);
    assert!(!saved.available());
    assert_eq!(saved.topology_bytes(), original_topology);
    assert_eq!(saved.inventory_bytes(), INVENTORY);
}

#[test]
fn topology_tower_stop_remains_independent_of_an_incomplete_second_frame() {
    let root = Root::new();
    let config = configuration(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let archive = root.path("source.bin");
    let selected = selected_checks(&root, &archive, 3);
    let first = document(&config, 7, 1, "6669727374");
    let first_poll = poll(&config, 7);
    let actor_peer = actor.clone();
    let stop_peer = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect_actor(&actor_peer);
        assert_executed(&finish(&mut socket, &first, &first_poll));
        wait(&crate::workflow::control::socket_path(&stop_peer, 8));
        socket.write_all(b"{\"schema_version\":").unwrap();
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_peer, 8, &mut output).unwrap();
        assert_eq!(
            field(&parse(&output), "status").as_str(),
            Some("stopped_drained")
        );
    });
    let human = human_for(reviewer.clone(), 7);
    let store = config.store.clone();
    let profile = config.profile.clone();
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
    .unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"first");
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert!(state.control.suspended);
    assert_eq!(state.stop.unwrap().request().operation, 8);
    assert!(!reviewer.socket(8).exists());
    assert!(!actor.socket.exists());
    let saved = TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(archive).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap();
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work().checks, 2);
    assert_eq!(
        saved.work().reserved_read_bytes,
        2 * TOPOLOGY_REFRESH_READ_BYTES
    );
}

#[test]
fn topology_tower_schedule_rejects_later_checkpoint_socket_aliases_before_setup() {
    for kind in ["human", "stop", "operator"] {
        for aliased in [false, true] {
            let root = Root::new();
            let config = configuration(&root);
            let (actor, reviewer) = profiles(&root, &config);
            let operator = operator_profile(&root, &config, 3, 0);
            let endpoint = match kind {
                "human" => reviewer.socket(8),
                "stop" => crate::workflow::control::socket_path(&reviewer, 8),
                "operator" => operator.socket.clone(),
                _ => unreachable!(),
            };
            let output = if aliased {
                let alias = root.path("endpoint-alias");
                std::os::unix::fs::symlink(endpoint.parent().unwrap(), &alias).unwrap();
                alias.join(endpoint.file_name().unwrap())
            } else {
                endpoint
            };
            let selected = retained_selection(&root, &output);
            fs::remove_file(root.path("topology.json")).unwrap();
            fs::remove_file(root.path("inventory.json")).unwrap();
            let store = config.store.clone();
            let result = service::serve_tower_with_topology(
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
                Some(&operator),
                selected,
                || panic!("late checkpoint/socket alias entered clock/source/setup"),
            );
            assert!(result.is_err(), "{kind}, aliased={aliased}");
            assert!(!store.exists());
            assert!(!actor.socket.exists());
            assert!(!operator.socket.exists());
            assert!(!reviewer.socket(8).exists());
            assert!(!output.exists());
        }
    }
}
