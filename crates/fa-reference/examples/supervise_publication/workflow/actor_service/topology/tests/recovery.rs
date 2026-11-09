//! Actual Tower restart across independently retained original custody. Synthetic
//! helpers/captures exercise enforcement; they do not establish evaluator quality.

#[path = "recovery/sequence.rs"]
mod sequence;

use super::*;
use crate::workflow::actor_service::{operator::Settings, tower::command_with_clock};
use fa_reference::ReadWitness;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
    guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
};
use fa_reference::strict_json::{self, Json, Limits};
use std::io::{Cursor, Read, Write};
use std::sync::mpsc;

const GENERATION: u64 = 11;
const CHILD: &str = "workflow::continuation::guarded::mediated::tests::synthetic_helper_process";

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value.get(name).unwrap()
}

fn parse(bytes: &[u8]) -> Json {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: 16 * 1024,
            max_depth: 16,
            max_items: 2048,
            max_string_bytes: 8192,
        },
    )
    .unwrap()
}

fn connect(path: &Path) -> UnixStream {
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{}",
            path.display()
        );
        match UnixStream::connect(path) {
            Ok(socket) => return socket,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                pause(1)
            }
            Err(error) => panic!("connect {}: {error}", path.display()),
        }
    }
}

fn exchange(socket: &mut UnixStream, document: &str) -> Json {
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket.write_all(document.as_bytes()).unwrap();
    socket.write_all(b"\n").unwrap();
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        assert!(bytes.len() < 16 * 1024);
        socket.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            return parse(&bytes);
        }
        bytes.push(byte[0]);
    }
}

fn native<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let result = field(response, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    field(result, "value")
}

fn knowledge(response: &Json) -> &Json {
    field(native(response, "publication-request"), "knowledge")
}

fn terminal(socket: &mut UnixStream, poll: &str) -> Json {
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let response = exchange(socket, poll);
        match field(knowledge(&response), "state").as_str() {
            Some("pending") => pause(1),
            Some("unknown")
                if knowledge(&response).get("reason").and_then(Json::as_str)
                    == Some("outcome_unknown") =>
            {
                pause(1)
            }
            _ => return response,
        }
    }
}

fn proposal(config: &Config, request: u64, epoch: u64, version: u64) -> String {
    let scope = config.profile.delivery.scope;
    let target = config.profile.delivery.target;
    let (payload, units) = if request == 7 {
        ("6e6574776f726b", 7)
    } else {
        ("7365636f6e64", 6)
    };
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"recovered-submit-{request}","address":"fa://{}/run/{}@{GENERATION}","verb":"propose","arguments":{{"request":"{request}","proposal":{{"target":{{"adapter":"{}","object":"{}","contract_version":"{}","expected_version":"{version}","generation":"{}"}},"payload_hex":"{payload}","units":"{units}","deadline":"100000","expected_policy_epoch":"{epoch}"}}}},"idempotency_key":"{request}"}}"#,
        scope.tenant,
        scope.run,
        target.adapter,
        target.object,
        target.contract_version,
        target.generation,
    )
}

fn request_document(config: &Config, request: u64, verb: &str) -> String {
    let scope = config.profile.delivery.scope;
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"recovered-{verb}-{request}","address":"fa://{}/attempt/request-{}-{}-{}-{}-{request}@{GENERATION}","verb":"{verb}","arguments":null,"idempotency_key":null}}"#,
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority,
    )
}

fn situation_document(config: &Config) -> String {
    let scope = config.profile.delivery.scope;
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"recovered-situation","address":"fa://{}/run/{}@{GENERATION}","verb":"situation","arguments":null,"idempotency_key":null}}"#,
        scope.tenant, scope.run,
    )
}

fn counts(response: &Json, expected: &[(&str, &str)]) {
    let known = field(native(response, "situation"), "knowledge");
    assert_eq!(field(known, "state").as_str(), Some("known"));
    assert_eq!(
        field(field(known, "basis"), "origin").as_str(),
        Some("acknowledged_owner")
    );
    let summary = field(field(known, "value"), "summary");
    for &(name, expected) in expected {
        assert_eq!(field(summary, name).as_str(), Some(expected), "{name}");
    }
}

fn operator(root: &Root, config: &Config) -> Settings {
    let (socket, _) = UnixStream::pair().unwrap();
    let id = PeerCredentials::observe(&socket).unwrap();
    let scope = config.profile.delivery.scope;
    let text = format!(
        r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"operator":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"connection_limit":4,"exchange_limit":64,"runtime_ms":15000,"poll_ms":1,"inspection_queries":3,"explanation_queries":1,"explanation_replay_events":1024,"explanation_disclosure":"policy_and_selected_witnesses"}}"#,
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        root.path("actors/operator.sock").display(),
        id.uid(),
        id.gid(),
        id.pid(),
        id.uid(),
        id.gid(),
        id.pid(),
    );
    fs::write(root.path("operator.json"), &text).unwrap();
    Settings::decode(text.as_bytes()).unwrap()
}

fn actor_profile(root: &Root, request: u64) -> Profile {
    let text = fs::read_to_string(root.path("actor.json"))
        .unwrap()
        .replace("\"request\":7", &format!("\"request\":{request}"))
        .replace("\"reply_ms\":200", "\"reply_ms\":1000");
    fs::write(root.path("actor.json"), &text).unwrap();
    Profile::decode(text.as_bytes()).unwrap()
}

fn helper_epoch(root: &Root, epoch: u64) -> Config {
    let mut text = fs::read_to_string(root.path("config.json"))
        .unwrap()
        .replace("tests::synthetic_helper_process", CHILD)
        .replace("FA_EXAMPLE_HELPER_MEMBER", "FA_MEDIATED_SUBMIT_MEMBER");
    for member in ["alpha", "beta"] {
        text = text.replace(
            &format!(r#""FA_MEDIATED_SUBMIT_MEMBER":"{member}""#),
            &format!(
                r#""FA_MEDIATED_SUBMIT_MEMBER":"{member}","FA_MEDIATED_SUBMIT_EPOCH":"{epoch}""#
            ),
        );
    }
    fs::write(root.path("config.json"), &text).unwrap();
    Config::decode(text.as_bytes()).unwrap()
}

fn next_graph(root: &Root, generation: u64, checks: u64) {
    let text = graph(false)
        .replacen(
            "\"generation\":1,",
            &format!("\"generation\":{generation},"),
            1,
        )
        .replace(
            "\"inventory_generation\":1",
            &format!("\"inventory_generation\":{generation}"),
        )
        .replace(
            "\"complete_inventory_generation\":1",
            &format!("\"complete_inventory_generation\":{generation}"),
        )
        .replace("\"checks\":32", &format!("\"checks\":{checks}"))
        .replace(
            &format!("\"read_bytes\":{}", 32 * TOPOLOGY_REFRESH_READ_BYTES),
            &format!("\"read_bytes\":{}", checks * TOPOLOGY_REFRESH_READ_BYTES),
        );
    fs::write(root.path("topology.json"), text).unwrap();
    fs::write(root.path("inventory.json"), INVENTORY).unwrap();
}

fn read_checkpoint(root: &Root, name: &str) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(root.path(name)).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

fn guard_profile(root: &Root, checkpoint: &TopologyCheckpoint, retain: &str) {
    let mut bytes = Vec::new();
    checkpoint
        .history_anchor()
        .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
        .unwrap();
    fs::write(root.path("first.anchor"), bytes).unwrap();
    let floor = checkpoint.minimum();
    let text = format!(
        r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":"{}","retain_history_anchor":"{}"}}"#,
        floor.journal_revision,
        floor.control_sequence,
        floor.authority_epoch,
        root.path("first.anchor").display(),
        root.path(retain).display(),
    );
    fs::write(root.path("guarded.json"), text).unwrap();
}

fn continuation_profile(root: &Root, retain: Option<&str>) {
    continuation_from(root, "first-checkpoint.bin", retain);
}

fn continuation_from(root: &Root, saved: &str, retain: Option<&str>) {
    let output = retain
        .map(|name| format!(r#", "retain_checkpoint":"{}""#, root.path(name).display()))
        .unwrap_or_default();
    let text = format!(
        r#"{{"schema_version":"fa.mediated-submit/2","initial":{{"topology_path":"{}","inventory_path":"{}"}},"source_checkpoint":"{}","next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":100{output}}}"#,
        root.path("initial-topology.json").display(),
        root.path("initial-inventory.json").display(),
        root.path(saved).display(),
        root.path("topology.json").display(),
        root.path("inventory.json").display(),
    );
    fs::write(root.path("continuation.json"), text).unwrap();
}

fn arguments(root: &Root, checked: bool, open: bool, operator: bool) -> Vec<String> {
    let mode = match (open, checked) {
        (false, false) => "serve-create",
        (false, true) => "serve-create-checked",
        (true, false) => "serve-open",
        (true, true) => "serve-open-checked",
    };
    let mut args = vec![
        mode.into(),
        root.path("config.json").display().to_string(),
        root.path("actor.json").display().to_string(),
        root.path("reviewer.json").display().to_string(),
    ];
    if checked {
        args.push(root.path("witness.json").display().to_string());
    }
    args.extend([
        "--tower-generation".into(),
        GENERATION.to_string(),
        "--topology-profile".into(),
        root.path(if open {
            "continuation.json"
        } else {
            "topology-profile.json"
        })
        .display()
        .to_string(),
    ]);
    if open {
        args.extend([
            "--guarded".into(),
            root.path("guarded.json").display().to_string(),
        ]);
    }
    if operator {
        args.extend([
            "--operator-profile".into(),
            root.path("operator.json").display().to_string(),
        ]);
    }
    args
}

fn review(
    profile: PeerProfile,
    request: u64,
    offered: Option<mpsc::Sender<()>>,
    release: Option<mpsc::Receiver<()>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = profile
            .connect_client_waiting(request, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        let mut seen = false;
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(!seen);
                    seen = true;
                    if let Some(offered) = &offered {
                        offered.send(()).unwrap();
                    }
                    if let Some(release) = &release {
                        release.recv_timeout(Duration::from_secs(10)).unwrap();
                    }
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

fn seed(root: &Root, checked: bool, checks: u64) -> (Config, PeerProfile, TopologyCheckpoint) {
    let config = configuration(root);
    let (_, reviewer) = profiles(root, &config);
    let actor = actor_profile(root, 7);
    if checked {
        prepare(root, &config, ElapsedTick(100000));
    } else {
        evidence(root, &config);
    }
    next_graph(root, 1, checks);
    fs::copy(
        root.path("topology.json"),
        root.path("initial-topology.json"),
    )
    .unwrap();
    fs::copy(
        root.path("inventory.json"),
        root.path("initial-inventory.json"),
    )
    .unwrap();
    fs::write(root.path("topology-profile.json"), format!(
        r#"{{"schema_version":"fa.mediated-create/2","topology_path":"{}","inventory_path":"{}","retain_checkpoint":"{}"}}"#,
        root.path("topology.json").display(), root.path("inventory.json").display(),
        root.path("first-checkpoint.bin").display(),
    )).unwrap();
    let submit = proposal(&config, 7, 0, 1);
    let poll = request_document(&config, 7, "get");
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&actor.socket);
        actor.supervisor.verify(&socket).unwrap();
        assert_eq!(
            field(knowledge(&exchange(&mut socket, &submit)), "state").as_str(),
            Some("pending")
        );
        assert_eq!(
            field(knowledge(&terminal(&mut socket, &poll)), "value").as_str(),
            Some("executed")
        );
    });
    let human = review(reviewer.clone(), 7, None, None);
    let result = command_with_clock(&arguments(root, checked, false, false), None, || {
        ElapsedTick(1000)
    });
    peer.join().unwrap();
    human.join().unwrap();
    assert!(result.is_ok(), "seed: {result:?}");
    let saved = read_checkpoint(root, "first-checkpoint.bin");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.charged, 7);
    assert_eq!(saved.work().checks, 2);
    assert_eq!(saved.history_anchor().revision(), state.revision);
    guard_profile(root, &saved, "second.anchor");
    continuation_profile(root, Some("second-checkpoint.bin"));
    (config, reviewer, saved)
}

fn second_captures(root: &Root, config: &Config) -> FrozenAction {
    let mut target = config.profile.delivery.target;
    target.expected_version = 2;
    let action = FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: config.profile.delivery.scope,
        target: Some(target),
        payload: b"second".to_vec(),
        required_witnesses: vec![ReadWitness::Exact {
            key: 7,
            value: Some(b"ok".to_vec()),
        }],
        policy_epoch: 3,
        deadline: ElapsedTick(100000),
        units: 6,
    })
    .unwrap();
    let packet = second_capture(&action, &[0, 2, 4]);
    write_capture(root, "original.bin", &packet);
    write_capture(root, "current.bin", &packet);
    action
}

fn second_capture(action: &FrozenAction, keys: &[u64]) -> FilePublicationCapture {
    // Only two native attempts exist, although external keys are seven/eight.
    let key = ProjectionKey {
        source: 40,
        branch: action.spec().scope.branch,
        projection: 7,
        source_epoch: 1,
    };
    let close = TrustedClosingMarker {
        key,
        final_sequence: 1,
        marker_generation: 1,
    };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(key, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(
        3,
        3,
        20,
        AdapterDomainInput::new(
            DomainProjection::new(40, 1, key),
            DomainClosure::Closed(close),
        ),
        keys.iter()
            .map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap())
            .collect(),
        &frontiers,
    )
    .unwrap();
    FilePublicationCapture::new(
        2,
        FileCaptureIdentity {
            source: 91,
            generation: 3,
        },
        action,
        FilePublicationInputs::new(Some(input), None),
    )
    .unwrap()
}

#[test]
fn tower_reopens_the_original_mediated_owner_for_a_second_plain_or_checked_effect() {
    for checked in [false, true] {
        let root = Root::new();
        let (_, reviewer, before) = seed(&root, checked, 32);
        let config = helper_epoch(&root, 3);
        let actor = actor_profile(&root, 8);
        next_graph(&root, 2, 32);
        if checked {
            second_captures(&root, &config);
        }
        let operator = operator(&root, &config);
        let submit = proposal(&config, 8, 3, 2);
        let poll = request_document(&config, 8, "get");
        let explain = request_document(&config, 8, "explain");
        let situation = situation_document(&config);
        let (offered_send, offered_receive) = mpsc::channel();
        let (release_send, release_receive) = mpsc::channel();
        let human = review(
            reviewer.clone(),
            8,
            Some(offered_send),
            Some(release_receive),
        );
        let source = root.path("evidence.json");
        let actor_peer = actor.clone();
        let operator_peer = operator.clone();
        let peer = std::thread::spawn(move || {
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
            let mut actor = connect(&actor_peer.socket);
            actor_peer.supervisor.verify(&actor).unwrap();
            assert_eq!(
                field(knowledge(&exchange(&mut actor, &submit)), "state").as_str(),
                Some("pending")
            );
            offered_receive
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
            counts(
                &exchange(&mut operator, &situation),
                &[
                    ("attempts", "2"),
                    ("pending_attempts", "1"),
                    ("executions", "1"),
                    ("charged_units", "7"),
                ],
            );
            let explained = exchange(&mut operator, &explain);
            let observed = field(native(&explained, "explanation"), "knowledge");
            assert_eq!(field(observed, "state").as_str(), Some("known"));
            assert_eq!(
                field(field(observed, "basis"), "request").as_str(),
                Some("8")
            );
            assert_eq!(
                field(field(observed, "value"), "ending_state").as_str(),
                Some("reviewing")
            );
            assert_eq!(
                field(native(&explained, "explanation"), "effect_authority").as_bool(),
                Some(false)
            );
            release_send.send(()).unwrap();
            assert_eq!(
                field(knowledge(&terminal(&mut actor, &poll)), "value").as_str(),
                Some("executed")
            );
            fs::remove_file(source).unwrap();
            counts(
                &exchange(&mut operator, &situation),
                &[
                    ("executions", "2"),
                    ("charged_units", "13"),
                    ("reserved_units", "0"),
                ],
            );
        });
        let result = command_with_clock(&arguments(&root, checked, true, true), None, || {
            ElapsedTick(1001)
        });
        peer.join().unwrap();
        human.join().unwrap();
        assert!(result.is_ok(), "checked={checked}: {result:?}");
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
        let after = read_checkpoint(&root, "second-checkpoint.bin");
        assert_eq!(after.history_anchor().revision(), state.revision);
        let anchor = FileHistoryAnchor::read_trusted(
            &mut Cursor::new(fs::read(root.path("second.anchor")).unwrap()),
            MAX_HISTORY_ANCHOR_BYTES,
        )
        .unwrap();
        assert_eq!(after.history_anchor(), &anchor);
        assert_eq!(after.expected().graph().spec().generation, 2);
        assert_eq!(after.expected().graph().spec().inventory_generation, 2);
        assert_eq!(after.work().checks, before.work().checks + 2);
        assert_eq!(
            after.work().reserved_read_bytes,
            before.work().reserved_read_bytes + 2 * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert!(after.last_operation() > before.last_operation());
        assert!(!actor.socket.exists());
        assert!(!operator.socket.exists());
        assert!(!reviewer.socket(8).exists());

        // Even intact old custody cannot explain work performed since its cut.
        let journal = fs::read(config.store.join("delivery.bin")).unwrap();
        guard_profile(&root, &before, "stale.anchor");
        continuation_profile(&root, Some("stale-checkpoint.bin"));
        let error = command_with_clock(&arguments(&root, checked, true, false), None, || {
            ElapsedTick(1002)
        })
        .unwrap_err();
        // The canonical history already names graph two. The original native
        // graph requirement rejects this older cut before the extent check.
        assert!(
            error.contains("guarded submission recovery is unconfirmed")
                && error.contains("Binding"),
            "{error}"
        );
        assert_eq!(
            fs::read(config.store.join("delivery.bin")).unwrap(),
            journal
        );
        assert!(!root.path("stale.anchor").exists());
        assert!(!root.path("stale-checkpoint.bin").exists());
    }
}

#[test]
fn tower_recovery_cannot_reset_spent_source_work_or_rebind_an_old_inventory() {
    for exhausted in [false, true] {
        let root = Root::new();
        let checks = if exhausted { 2 } else { 32 };
        let (config, reviewer, before) = seed(&root, false, checks);
        let actor = actor_profile(&root, 8);
        next_graph(&root, 2, checks);
        if !exhausted {
            let graph = fs::read_to_string(root.path("topology.json"))
                .unwrap()
                .replace("\"inventory_generation\":2", "\"inventory_generation\":1")
                .replace(
                    "\"complete_inventory_generation\":2",
                    "\"complete_inventory_generation\":1",
                );
            fs::write(root.path("topology.json"), graph).unwrap();
        }
        fs::remove_file(root.path("evidence.json")).unwrap();
        let error = command_with_clock(&arguments(&root, false, true, false), None, || {
            ElapsedTick(1001)
        })
        .unwrap_err();
        let expected = if exhausted {
            "topology budget must cover initial admission and final publication"
        } else {
            "next graph and inventory generations must both advance"
        };
        assert!(error.contains(expected), "{error}");
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 1);
        assert_eq!(state.payload, b"network");
        assert_eq!(state.control.ledger.stages.len(), 1);
        assert_eq!(
            (state.control.ledger.charged, state.control.ledger.reserved),
            (7, 0)
        );
        assert_eq!(read_checkpoint(&root, "first-checkpoint.bin"), before);
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(8).exists());
        let after = read_checkpoint(&root, "second-checkpoint.bin");
        assert_eq!(after.work(), before.work());
        assert_eq!(after.last_operation(), before.last_operation());
        assert!(!after.available());
        assert_eq!(after.history_anchor().revision(), state.revision);
    }
}

#[test]
fn independent_stop_preempts_partial_actor_and_operator_frames_after_mediated_recovery() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, true, 32);
    let actor = actor_profile(&root, 8);
    let operator = operator(&root, &config);
    next_graph(&root, 2, 32);
    for name in ["evidence.json", "original.bin", "current.bin"] {
        fs::remove_file(root.path(name)).unwrap();
    }
    let situation = situation_document(&config);
    let submit = proposal(&config, 8, 3, 2);
    let actor_peer = actor.clone();
    let operator_peer = operator.clone();
    let stop_peer = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut operator = connect(&operator_peer.socket);
        operator_peer.supervisor.verify(&operator).unwrap();
        counts(
            &exchange(&mut operator, &situation),
            &[("attempts", "1"), ("executions", "1")],
        );
        let mut actor = connect(&actor_peer.socket);
        actor_peer.supervisor.verify(&actor).unwrap();
        actor
            .write_all(&submit.as_bytes()[..submit.len() / 2])
            .unwrap();
        operator
            .write_all(&situation.as_bytes()[..situation.len() / 2])
            .unwrap();
        wait(&crate::workflow::control::socket_path(&stop_peer, 8));
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_peer, 8, &mut output).unwrap();
        assert_eq!(
            field(&parse(&output), "status").as_str(),
            Some("stopped_drained")
        );
    });
    let result = command_with_clock(&arguments(&root, true, true, true), None, || {
        ElapsedTick(1001)
    });
    peer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"network");
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(state.stop.unwrap().request().operation, 8);
    assert!(state.control.suspended);
    let saved = read_checkpoint(&root, "second-checkpoint.bin");
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work().checks, before.work().checks + 1);
    assert!(!actor.socket.exists());
    assert!(!operator.socket.exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn recovered_checked_effect_still_requires_the_original_final_witness_cut_after_dispatch() {
    let root = Root::new();
    let (_, reviewer, _) = seed(&root, true, 32);
    let config = helper_epoch(&root, 3);
    let actor = actor_profile(&root, 8);
    next_graph(&root, 2, 32);
    let action = second_captures(&root, &config);
    let submit = proposal(&config, 8, 3, 2);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile.socket);
        profile.supervisor.verify(&socket).unwrap();
        assert_eq!(
            field(knowledge(&exchange(&mut socket, &submit)), "state").as_str(),
            Some("pending")
        );
    });
    let human = review(reviewer.clone(), 8, None, None);
    let mut changed = false;
    let result = command_with_clock(&arguments(&root, true, true, false), None, || {
        if !changed
            && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
            && state.control.ledger.charged == 13
            && state.executions == 1
        {
            write_capture(
                &root,
                "current.bin",
                &second_capture(&action, &[0, 1, 2, 4]),
            );
            changed = true;
        }
        ElapsedTick(1001)
    });
    peer.join().unwrap();
    human.join().unwrap();
    assert!(changed, "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1, "{result:?}");
    assert_eq!(state.payload, b"network");
    assert_eq!(state.control.ledger.stages.len(), 2);
    assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(
        state.control.ledger.available + state.control.ledger.charged,
        config.profile.delivery.total
    );
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
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn repeated_recorded_recovery_keeps_original_receipt_and_source_work_without_live_assets() {
    let root = Root::new();
    let (config, reviewer, mut before) = seed(&root, true, 32);
    for name in [
        "topology.json",
        "inventory.json",
        "evidence.json",
        "original.bin",
        "current.bin",
    ] {
        fs::remove_file(root.path(name)).unwrap();
    }
    // Keep the original declared helper contracts while making actual process
    // launch impossible. A recorded request must never enter that boundary.
    let text = fs::read_to_string(root.path("config.json"))
        .unwrap()
        .replace(
            &format!(
                "{:?}",
                std::env::current_exe().unwrap().display().to_string()
            ),
            &format!("{:?}", root.path("absent-helper").display().to_string()),
        );
    fs::write(root.path("config.json"), text).unwrap();
    let credibility = root.path("absent-credibility");
    let mut original_receipt = None;
    for (source, output, anchor, tick) in [
        (
            "first-checkpoint.bin",
            "second-checkpoint.bin",
            "second.anchor",
            1000000,
        ),
        (
            "second-checkpoint.bin",
            "third-checkpoint.bin",
            "third.anchor",
            1000001,
        ),
    ] {
        guard_profile(&root, &before, anchor);
        continuation_from(&root, source, Some(output));
        let actor = actor_profile(&root, 7);
        let submit = proposal(&config, 7, 0, 1);
        let poll = request_document(&config, 7, "get");
        let profile = actor.clone();
        let peer = std::thread::spawn(move || {
            let mut socket = connect(&profile.socket);
            profile.supervisor.verify(&socket).unwrap();
            let response = exchange(&mut socket, &submit);
            if field(knowledge(&response), "state").as_str() == Some("pending") {
                terminal(&mut socket, &poll)
            } else {
                response
            }
        });
        let result = command_with_clock(
            &arguments(&root, true, true, false),
            Some(&credibility),
            || ElapsedTick(tick),
        );
        let response = peer.join().unwrap();
        assert!(result.is_ok(), "{source}: {result:?}");
        assert_eq!(field(knowledge(&response), "state").as_str(), Some("known"));
        assert_eq!(
            field(knowledge(&response), "value").as_str(),
            Some("executed")
        );
        if let Some(receipt) = &original_receipt {
            assert_eq!(knowledge(&response), receipt);
        } else {
            original_receipt = Some(knowledge(&response).clone());
        }
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 1);
        assert_eq!(state.payload, b"network");
        assert_eq!(state.control.ledger.stages.len(), 1);
        assert_eq!(
            (state.control.ledger.charged, state.control.ledger.reserved),
            (7, 0)
        );
        let after = read_checkpoint(&root, output);
        assert_eq!(after.history_anchor().revision(), state.revision);
        assert!(after.history_anchor().revision() > before.history_anchor().revision());
        assert_eq!(after.work(), before.work());
        assert_eq!(after.last_operation(), before.last_operation());
        assert_eq!(after.topology_bytes(), before.topology_bytes());
        assert_eq!(after.inventory_bytes(), before.inventory_bytes());
        assert!(!after.available());
        let retained = FileHistoryAnchor::read_trusted(
            &mut Cursor::new(fs::read(root.path(anchor)).unwrap()),
            MAX_HISTORY_ANCHOR_BYTES,
        )
        .unwrap();
        assert_eq!(after.history_anchor(), &retained);
        for name in [
            "topology.json",
            "inventory.json",
            "evidence.json",
            "original.bin",
            "current.bin",
            "absent-helper",
            "absent-credibility",
        ] {
            assert!(!root.path(name).exists(), "{name}");
        }
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(7).exists());
        before = after;
    }
}

#[test]
fn recovered_unresolved_dispatch_cannot_admit_another_request_or_refund_its_charge() {
    let root = Root::new();
    let (_, reviewer, _) = seed(&root, false, 32);
    let config = helper_epoch(&root, 3);
    let actor = actor_profile(&root, 8);
    next_graph(&root, 2, 32);
    let submit = proposal(&config, 8, 3, 2);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile.socket);
        profile.supervisor.verify(&socket).unwrap();
        assert_eq!(
            field(knowledge(&exchange(&mut socket, &submit)), "state").as_str(),
            Some("pending")
        );
    });
    let human = review(reviewer.clone(), 8, None, None);
    let mut expired = false;
    let result = command_with_clock(&arguments(&root, false, true, false), None, || {
        if !expired
            && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
            && state.control.ledger.charged == 13
            && state.executions == 1
        {
            expired = true;
        }
        // This real timeout is after both original keys and charged dispatch,
        // before publication. Native endpoint retention has also expired, so
        // the native Stop sweep cannot manufacture a nonexecution receipt.
        ElapsedTick(if expired { 5_000_000 } else { 1001 })
    });
    peer.join().unwrap();
    human.join().unwrap();
    assert!(expired && result.is_err(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"network");
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (13, 0)
    );
    assert!(matches!(
        state.control.ledger.stages[&2],
        ActionState::Unknown | ActionState::IrrecoverablyUnknown
    ));
    let saved = read_checkpoint(&root, "second-checkpoint.bin");
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work().checks, 3);

    guard_profile(&root, &saved, "third.anchor");
    continuation_from(&root, "second-checkpoint.bin", Some("third-checkpoint.bin"));
    let text = fs::read_to_string(root.path("continuation.json"))
        .unwrap()
        .replace("\"operation\":100", "\"operation\":200");
    fs::write(root.path("continuation.json"), text).unwrap();
    let text = fs::read_to_string(root.path("actor.json"))
        .unwrap()
        .replace("\"request\":8", "\"request\":9");
    fs::write(root.path("actor.json"), text).unwrap();
    for name in ["topology.json", "inventory.json", "evidence.json"] {
        fs::remove_file(root.path(name)).unwrap();
    }
    let error = command_with_clock(&arguments(&root, false, true, false), None, || {
        ElapsedTick(5_000_001)
    })
    .unwrap_err();
    assert!(error.contains("unresolved prior deliveries"), "{error}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.stages.len(), 2);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (13, 0)
    );
    assert_eq!(
        state.control.ledger.available + state.control.ledger.charged,
        config.profile.delivery.total
    );
    let after = read_checkpoint(&root, "third-checkpoint.bin");
    assert_eq!(after.history_anchor().revision(), state.revision);
    assert_eq!(after.work(), saved.work());
    assert_eq!(after.last_operation(), saved.last_operation());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(9).exists());
}

#[test]
fn recorded_recovery_still_services_stop_without_exporting_an_obsolete_source_cut() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, true, 32);
    let actor = actor_profile(&root, 7);
    for name in [
        "topology.json",
        "inventory.json",
        "evidence.json",
        "original.bin",
        "current.bin",
    ] {
        fs::remove_file(root.path(name)).unwrap();
    }
    let submit = proposal(&config, 7, 0, 1);
    let profile = actor.clone();
    let stop_peer = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile.socket);
        profile.supervisor.verify(&socket).unwrap();
        socket
            .write_all(&submit.as_bytes()[..submit.len() / 2])
            .unwrap();
        wait(&crate::workflow::control::socket_path(&stop_peer, 7));
        let mut response = Vec::new();
        crate::peers::stop::request(&stop_peer, 7, &mut response).unwrap();
        assert_eq!(
            field(&parse(&response), "status").as_str(),
            Some("stopped_drained")
        );
    });
    let error = command_with_clock(&arguments(&root, true, true, false), None, || {
        ElapsedTick(1000000)
    })
    .unwrap_err();
    peer.join().unwrap();
    assert!(
        error.contains("source checkpoint retention refused"),
        "{error}"
    );
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"network");
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(state.stop.unwrap().request().operation, 7);
    assert!(state.control.suspended);
    let history = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(root.path("second.anchor")).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    assert_eq!(history.revision(), state.revision);
    assert_eq!(read_checkpoint(&root, "first-checkpoint.bin"), before);
    assert!(!root.path("second-checkpoint.bin").exists());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
}
