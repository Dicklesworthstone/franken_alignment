//! Tower bytes use the original mediated owner and the original final effect cut.
//! Complete fixture graphs and synthetic helper votes are not deployment proof.

#[path = "tower/failure_custody.rs"]
mod failure_custody;
#[path = "tower/series.rs"]
mod series;

use super::*;
use crate::workflow::actor_service::operator::Settings;
use crate::workflow::actor_service::serve_selected_ingress;
use crate::workflow::actor_service::tower::command_with_clock as tower_command;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::{
    MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::io::{Cursor, Read, Write};
use std::sync::mpsc;

const GENERATION: u64 = 11;

fn proposal_document(config: &Config) -> String {
    let scope = config.profile.delivery.scope;
    let target = config.profile.delivery.target;
    format!(
        concat!(
            "{{\"schema_version\":\"fa.tower_request/1\",\"operation_id\":\"topology-submit\",",
            "\"address\":\"fa://{}/run/{}@{}\",\"verb\":\"propose\",",
            "\"arguments\":{{\"request\":\"7\",\"proposal\":{{\"target\":{{",
            "\"adapter\":\"{}\",\"object\":\"{}\",\"contract_version\":\"{}\",",
            "\"expected_version\":\"{}\",\"generation\":\"{}\"}},",
            "\"payload_hex\":\"6e6574776f726b\",\"units\":\"7\",\"deadline\":\"100000\",",
            "\"expected_policy_epoch\":\"0\"}}}},\"idempotency_key\":\"7\"}}"
        ),
        scope.tenant,
        scope.run,
        GENERATION,
        target.adapter,
        target.object,
        target.contract_version,
        target.expected_version,
        target.generation,
    )
}

fn request_document(config: &Config, verb: &str) -> String {
    let scope = config.profile.delivery.scope;
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"topology-{verb}","address":"fa://{}/attempt/request-{}-{}-{}-{}-7@{GENERATION}","verb":"{verb}","arguments":null,"idempotency_key":null}}"#,
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority,
    )
}

fn situation_document(config: &Config) -> String {
    let scope = config.profile.delivery.scope;
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"topology-situation","address":"fa://{}/run/{}@{GENERATION}","verb":"situation","arguments":null,"idempotency_key":null}}"#,
        scope.tenant, scope.run,
    )
}

fn operator_profile(
    root: &Root,
    config: &Config,
    inspection: usize,
    explanation: usize,
) -> Settings {
    let (socket, _) = UnixStream::pair().unwrap();
    let id = PeerCredentials::observe(&socket).unwrap();
    let scope = config.profile.delivery.scope;
    let (replay, disclosure) = if explanation == 0 {
        (0, "none")
    } else {
        (1024, "policy_and_selected_witnesses")
    };
    let text = format!(
        r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"operator":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"connection_limit":4,"exchange_limit":64,"runtime_ms":15000,"poll_ms":1,"inspection_queries":{inspection},"explanation_queries":{explanation},"explanation_replay_events":{replay},"explanation_disclosure":"{disclosure}"}}"#,
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

fn retained_selection(root: &Root, output: &Path) -> Selection {
    selection(root, false);
    let text = format!(
        r#"{{"schema_version":"fa.mediated-create/2","topology_path":"{}","inventory_path":"{}","retain_checkpoint":"{}"}}"#,
        root.path("topology.json").display(),
        root.path("inventory.json").display(),
        output.display(),
    );
    fs::write(root.path("topology-profile.json"), &text).unwrap();
    Selection::decode(text.as_bytes()).unwrap()
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

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value.get(name).unwrap()
}

fn native<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let result = field(response, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    field(result, "value")
}

fn request_knowledge(response: &Json) -> &Json {
    field(native(response, "publication-request"), "knowledge")
}

fn known<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let observed = field(native(response, kind), "knowledge");
    assert_eq!(field(observed, "state").as_str(), Some("known"));
    assert_eq!(
        field(field(observed, "basis"), "origin").as_str(),
        Some("acknowledged_owner")
    );
    observed
}

fn counts(response: &Json, expected: &[(&str, &str)]) {
    let summary = field(field(known(response, "situation"), "value"), "summary");
    for &(name, value) in expected {
        assert_eq!(field(summary, name).as_str(), Some(value), "{name}");
    }
}

fn connect_waiting(path: &Path) -> UnixStream {
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

fn connect_actor(profile: &Profile) -> UnixStream {
    let socket = connect_waiting(&profile.socket);
    profile.supervisor.verify(&socket).unwrap();
    socket
}

fn connect_operator(profile: &Settings) -> UnixStream {
    let socket = connect_waiting(&profile.socket);
    profile.supervisor.verify(&socket).unwrap();
    socket
}

fn exchange(socket: &mut UnixStream, document: &str) -> Json {
    // Test transport only; every original service/parser/credential check still runs.
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket.write_all(document.as_bytes()).unwrap();
    socket.write_all(b"\n").unwrap();
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        assert!(response.len() < 16 * 1024);
        socket.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            return parse(&response);
        }
        response.push(byte[0]);
    }
}

fn terminal(socket: &mut UnixStream, poll: &str) -> Json {
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let response = exchange(socket, poll);
        let observed = request_knowledge(&response);
        match field(observed, "state").as_str() {
            Some("pending") => {}
            Some("unknown")
                if observed.get("reason").and_then(Json::as_str) == Some("outcome_unknown") => {}
            _ => return response,
        }
        pause(1);
    }
}

fn gated_human(
    profile: PeerProfile,
    decision: ReviewDecision,
    offered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = profile
            .connect_client_waiting(7, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        let mut seen = false;
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(!seen);
                    seen = true;
                    offered.send(()).unwrap();
                    release.recv_timeout(Duration::from_secs(10)).unwrap();
                    client.respond(decision).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

#[test]
fn tower_topology_commands_require_both_original_keys_and_keep_operator_reads_observational() {
    for checked in [false, true] {
        for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
            let approved = decision == ReviewDecision::Approve;
            let root = Root::new();
            let config = configuration(&root);
            let (mut actor_profile, reviewer) = profiles(&root, &config);
            actor_profile.reply_ms = 1000;
            let actor_text = fs::read_to_string(root.path("actor.json"))
                .unwrap()
                .replace("\"reply_ms\":200", "\"reply_ms\":1000");
            fs::write(root.path("actor.json"), actor_text).unwrap();
            if checked {
                prepare(&root, &config, ElapsedTick(100000));
            } else {
                evidence(&root, &config);
            }
            let archive = root.path("topology-checkpoint.bin");
            retained_selection(&root, &archive);
            let operator = operator_profile(&root, &config, 3, 1);
            let submitted = proposal_document(&config);
            let poll = request_document(&config, "get");
            let explanation = request_document(&config, "explain");
            let situation = situation_document(&config);
            let source = root.path("evidence.json");
            let profile = actor_profile.clone();
            let operator_peer = operator.clone();
            let (offered_send, offered_receive) = mpsc::channel();
            let (release_send, release_receive) = mpsc::channel();
            let human = gated_human(reviewer.clone(), decision, offered_send, release_receive);
            let peer = std::thread::spawn(move || {
                let mut operator = connect_operator(&operator_peer);
                counts(
                    &exchange(&mut operator, &situation),
                    &[("attempts", "0"), ("executions", "0")],
                );
                let mut actor = connect_actor(&profile);
                let response = exchange(&mut actor, &submitted);
                assert_eq!(
                    field(request_knowledge(&response), "state").as_str(),
                    Some("pending")
                );
                offered_receive
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                counts(
                    &exchange(&mut operator, &situation),
                    &[
                        ("attempts", "1"),
                        ("pending_attempts", "1"),
                        ("reserved_units", "0"),
                        ("charged_units", "0"),
                        ("executions", "0"),
                    ],
                );
                let explained = exchange(&mut operator, &explanation);
                let observed = known(&explained, "explanation");
                assert_eq!(
                    field(field(observed, "basis"), "request").as_str(),
                    Some("7")
                );
                assert_eq!(
                    field(field(observed, "value"), "ending_state").as_str(),
                    Some("reviewing")
                );
                assert_eq!(
                    field(native(&explained, "explanation"), "current_eligibility").as_str(),
                    Some("not_observed")
                );
                assert_eq!(
                    field(native(&explained, "explanation"), "effect_authority").as_bool(),
                    Some(false)
                );
                release_send.send(()).unwrap();
                let response = terminal(&mut actor, &poll);
                let observed = request_knowledge(&response);
                assert_eq!(field(observed, "state").as_str(), Some("known"));
                assert_eq!(
                    field(observed, "value").as_str() == Some("executed"),
                    approved
                );
                // All source-dependent publication work is complete. These
                // original operator reads do not refresh policy or topology.
                fs::remove_file(&source).unwrap();
                counts(
                    &exchange(&mut operator, &situation),
                    &[
                        ("executions", if approved { "1" } else { "0" }),
                        ("charged_units", if approved { "7" } else { "0" }),
                        ("reserved_units", "0"),
                        ("terminal_attempts", "1"),
                    ],
                );
                assert!(!source.exists());
            });
            let mut args = vec![
                if checked {
                    "serve-create-checked".into()
                } else {
                    "serve-create".into()
                },
                root.path("config.json").display().to_string(),
                root.path("actor.json").display().to_string(),
                root.path("reviewer.json").display().to_string(),
            ];
            if checked {
                args.push(root.path("witness.json").display().to_string());
            }
            args.extend([
                "--operator-profile".into(),
                root.path("operator.json").display().to_string(),
                "--topology-profile".into(),
                root.path("topology-profile.json").display().to_string(),
                "--tower-generation".into(),
                GENERATION.to_string(),
            ]);
            let result = tower_command(&args, None, || ElapsedTick(1000));
            peer.join().unwrap();
            human.join().unwrap();
            assert!(
                result.is_ok(),
                "checked={checked}, decision={decision:?}: {result:?}"
            );
            let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
            assert_eq!(state.executions, u64::from(approved));
            assert_eq!(
                state.payload,
                if approved {
                    b"network".to_vec()
                } else {
                    b"initial".to_vec()
                }
            );
            assert_eq!(state.control.ledger.charged, if approved { 7 } else { 0 });
            assert_eq!(state.control.ledger.reserved, 0);
            assert_eq!(
                state.control.ledger.available,
                config.profile.delivery.total - state.control.ledger.charged
            );
            assert!(state.stop.is_none());
            let saved = TopologyCheckpoint::read_trusted(
                &mut Cursor::new(fs::read(&archive).unwrap()),
                MAX_TOPOLOGY_CHECKPOINT_BYTES,
            )
            .unwrap();
            assert_eq!(saved.history_anchor().revision(), state.revision);
            assert_eq!(
                saved.expected().graph().spec().scope,
                config.profile.delivery.scope
            );
            assert_eq!(saved.topology_bytes(), graph(false).as_bytes());
            assert_eq!(saved.inventory_bytes(), INVENTORY);
            assert_eq!(
                saved.work().checks,
                if approved { 2 } else { 1 },
                "operator reads spent native topology work"
            );
            assert!(!actor_profile.socket.exists());
            assert!(!operator.socket.exists());
            assert!(!reviewer.socket(7).exists());
        }
    }
}

#[test]
fn tower_final_publication_rechecks_original_topology_and_witnesses_after_charged_dispatch() {
    for mode in [
        "unrelated",
        "phantom",
        "missing-witness",
        "missing-topology",
        "changed-topology",
        "both-missing",
    ] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor_profile, reviewer) = profiles(&root, &config);
        let (publication, action) = prepare(&root, &config, ElapsedTick(100000));
        let selected = selection(&root, false);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let submitted = proposal_document(&config);
        let profile = actor_profile.clone();
        let peer = std::thread::spawn(move || {
            let response = exchange(&mut connect_actor(&profile), &submitted);
            assert_eq!(
                field(request_knowledge(&response), "state").as_str(),
                Some("pending")
            );
        });
        let human = human(reviewer.clone(), ReviewDecision::Approve);
        let mut changed = false;
        let result = serve_selected_ingress(
            config,
            &actor_profile,
            &reviewer,
            Some(&publication),
            false,
            None,
            Some(selected),
            None,
            Some(GENERATION),
            None,
            || {
                if !changed
                    && let Ok(state) = FileOversight::read_publication(&store, &bootstrap)
                    && state.control.ledger.charged != 0
                    && state.executions == 0
                {
                    match mode {
                        "unrelated" => write_capture(
                            &root,
                            "current.bin",
                            &capture(&action, 2, &[0, 2, 4, 99]),
                        ),
                        "phantom" => {
                            write_capture(&root, "current.bin", &capture(&action, 2, &[0, 1, 2, 4]))
                        }
                        "missing-witness" => fs::remove_file(root.path("current.bin")).unwrap(),
                        "missing-topology" => fs::remove_file(root.path("inventory.json")).unwrap(),
                        "changed-topology" => {
                            fs::write(root.path("topology.json"), graph(true)).unwrap()
                        }
                        "both-missing" => {
                            fs::remove_file(root.path("current.bin")).unwrap();
                            fs::remove_file(root.path("inventory.json")).unwrap();
                        }
                        _ => unreachable!(),
                    }
                    changed = true;
                }
                ElapsedTick(1000)
            },
        );
        peer.join().unwrap();
        human.join().unwrap();
        assert!(changed, "{mode}: {result:?}");
        if mode == "unrelated" {
            assert!(result.is_ok(), "{result:?}");
        }
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(
            state.executions,
            u64::from(mode == "unrelated"),
            "{mode}: {result:?}"
        );
        assert_eq!(
            state.payload,
            if mode == "unrelated" {
                b"network".to_vec()
            } else {
                b"initial".to_vec()
            }
        );
        assert_eq!(
            state.control.ledger.available
                + state.control.ledger.reserved
                + state.control.ledger.charged,
            bootstrap.delivery.total
        );
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.stages.len(), 1, "{mode}: {result:?}");
        let stage = *state.control.ledger.stages.values().next().unwrap();
        if mode == "unrelated" {
            assert_eq!(state.control.ledger.charged, 7);
            assert_eq!(stage, ActionState::Confirmed);
        } else if state.control.ledger.charged == 0 {
            // The hook already observed charged dispatch. Native trusted
            // nonexecution is the only subsequent refund path; neither an
            // unknown result nor a missing source may erase that liability.
            assert_eq!(
                stage,
                ActionState::ConfirmedNotExecuted,
                "{mode}: {result:?}"
            );
        } else {
            // Missing observations alone never justify refunding an unresolved
            // dispatch. Only the original endpoint's receipt may release it.
            assert_eq!(state.control.ledger.charged, 7);
            assert!(
                matches!(
                    stage,
                    ActionState::Dispatching
                        | ActionState::Unknown
                        | ActionState::IrrecoverablyUnknown
                ),
                "{mode}: {result:?}"
            );
        }
        assert!(!actor_profile.socket.exists());
        assert!(!reviewer.socket(7).exists());
    }
}

#[test]
fn initial_bypass_cannot_expose_a_tower_actor_or_operator_to_an_admitted_effect() {
    let root = Root::new();
    let mut config = configuration(&root);
    config.programs.clear();
    let (actor, reviewer) = profiles(&root, &config);
    let publication = publication(&root);
    let selected = selection(&root, true);
    let operator = operator_profile(&root, &config, 1, 0);
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let error = serve_selected_ingress(
        config,
        &actor,
        &reviewer,
        Some(&publication),
        false,
        None,
        Some(selected),
        None,
        Some(GENERATION),
        Some(&operator),
        || ElapsedTick(1000),
    )
    .unwrap_err();
    assert!(error.contains("initial live-service topology was not certified"));
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 0);
    assert!(state.control.ledger.stages.is_empty());
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (0, 0)
    );
    assert!(!root.path("evidence.json").exists());
    assert!(!actor.socket.exists());
    assert!(!operator.socket.exists());
    assert!(!reviewer.socket(7).exists());
}

#[test]
fn independent_stop_preempts_partial_tower_frames_before_mediated_source_admission() {
    let root = Root::new();
    let mut config = configuration(&root);
    config.programs.clear();
    let (actor, reviewer) = profiles(&root, &config);
    let selected = selection(&root, false);
    let operator = operator_profile(&root, &config, 1, 0);
    let situation = situation_document(&config);
    let submitted = proposal_document(&config);
    let profile = actor.clone();
    let operator_peer = operator.clone();
    let stop_peer = reviewer.clone();
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_peer);
        counts(&exchange(&mut operator, &situation), &[("attempts", "0")]);
        let mut actor = connect_actor(&profile);
        actor
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        actor
            .write_all(&submitted.as_bytes()[..submitted.len() / 2])
            .unwrap();
        operator
            .write_all(&situation.as_bytes()[..situation.len() / 2])
            .unwrap();
        wait(&crate::workflow::control::socket_path(&stop_peer, 7));
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_peer, 7, &mut output).unwrap();
        assert_eq!(
            field(&parse(&output), "status").as_str(),
            Some("stopped_drained")
        );
    });
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
        Some(&operator),
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 0);
    assert!(state.control.ledger.stages.is_empty());
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (0, 0)
    );
    assert_eq!(state.control.ledger.available, bootstrap.delivery.total);
    assert_eq!(state.stop.unwrap().request().operation, 7);
    assert!(state.control.suspended);
    assert!(!root.path("evidence.json").exists());
    assert!(!actor.socket.exists());
    assert!(!operator.socket.exists());
    assert!(!reviewer.socket(7).exists());
}

#[test]
fn tower_topology_cannot_downgrade_stronger_witness_profiles_before_time_or_source() {
    for waiting in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let schema = if waiting {
            "fa.supervised-witnesses/4"
        } else {
            "fa.supervised-witnesses/2"
        };
        let extra = if waiting { ",\"max_retries\":2" } else { "" };
        let text = publication_text(&root).replace("fa.supervised-witnesses/1", schema)
            .replace("\"source\":91", &format!(r#""source":91{extra},"feed":{{"path":"{}","source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#,
                root.path("missing-feed.bin").display()));
        let publication = PublicationProfile::decode(text.as_bytes()).unwrap();
        let selected = selection(&root, false);
        let operator = operator_profile(&root, &config, 1, 0);
        fs::remove_file(root.path("topology.json")).unwrap();
        let store = config.store.clone();
        let error = serve_selected_ingress(
            config,
            &actor,
            &reviewer,
            Some(&publication),
            false,
            None,
            Some(selected),
            None,
            Some(GENERATION),
            Some(&operator),
            || panic!("unsupported topology witness composition entered the clock"),
        )
        .unwrap_err();
        assert!(error.contains("not downgraded"));
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert!(!operator.socket.exists());
        assert!(!root.path("evidence.json").exists());
    }
}

#[test]
fn topology_checkpoint_aliasing_operator_socket_refuses_before_time_or_store() {
    for aliased_parent in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let operator = operator_profile(&root, &config, 1, 0);
        let output = if aliased_parent {
            let alias = root.path("aliased-actors");
            std::os::unix::fs::symlink(root.path("actors"), &alias).unwrap();
            alias.join("operator.sock")
        } else {
            operator.socket.clone()
        };
        let selected = retained_selection(&root, &output);
        fs::remove_file(root.path("topology.json")).unwrap();
        fs::remove_file(root.path("inventory.json")).unwrap();
        let store = config.store.clone();
        let error = serve_selected_ingress(
            config,
            &actor,
            &reviewer,
            None,
            false,
            None,
            Some(selected),
            None,
            Some(GENERATION),
            Some(&operator),
            || panic!("checkpoint/socket alias entered the clock or source boundary"),
        )
        .unwrap_err();
        assert_eq!(
            error,
            "topology checkpoint and operator socket must be distinct"
        );
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert!(!operator.socket.exists());
        assert!(!output.exists());
        assert!(!root.path("evidence.json").exists());
        assert!(!reviewer.socket(7).exists());
    }
}

#[test]
fn topology_checkpoint_cannot_replace_retired_actor_human_or_stop_endpoints() {
    for endpoint in ["actor", "human", "stop"] {
        for aliased_parent in [false, true] {
            let root = Root::new();
            let config = configuration(&root);
            let (actor, reviewer) = profiles(&root, &config);
            let selected_socket = match endpoint {
                "actor" => actor.socket.clone(),
                "human" => reviewer.socket(7),
                "stop" => crate::workflow::control::socket_path(&reviewer, 7),
                _ => unreachable!(),
            };
            let output = if aliased_parent {
                let alias = root.path("aliased-endpoints");
                std::os::unix::fs::symlink(selected_socket.parent().unwrap(), &alias).unwrap();
                alias.join(selected_socket.file_name().unwrap())
            } else {
                selected_socket
            };
            let selected = retained_selection(&root, &output);
            fs::remove_file(root.path("topology.json")).unwrap();
            fs::remove_file(root.path("inventory.json")).unwrap();
            let store = config.store.clone();
            let error = serve_selected_ingress(
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
                || panic!("checkpoint/socket alias entered the clock or source boundary"),
            )
            .unwrap_err();
            assert_eq!(
                error,
                "topology checkpoint and actor, human-review or Stop sockets must be distinct",
                "{endpoint}, aliased_parent={aliased_parent}",
            );
            assert!(!store.exists());
            assert!(!actor.socket.exists());
            assert!(!reviewer.socket(7).exists());
            assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
            assert!(!output.exists());
            assert!(!root.path("evidence.json").exists());
        }
    }
}
