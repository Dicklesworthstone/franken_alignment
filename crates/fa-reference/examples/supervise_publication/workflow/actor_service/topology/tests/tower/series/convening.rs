//! Original mediated custody survives an explicit fresh congress and actual
//! publication. Synthetic helper recovery tests protocol behavior, not detection.
use super::*;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::address_resolver::Address;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::ffi::OsString;

const HELPER: &str =
    "workflow::actor_service::tests::tower_service::convening::held_helper_process";

fn held_configuration(root: &Root) -> Config {
    let mut config = configuration(root);
    config.programs = ["alpha", "beta"]
        .into_iter()
        .map(|member| {
            (
                member.to_owned(),
                HelperProgram::new(
                    std::env::current_exe().unwrap(),
                    root.0.clone(),
                    vec!["--exact".into(), HELPER.into(), "--nocapture".into()],
                    BTreeMap::from([(
                        OsString::from("FA_HELD_HELPER_MEMBER"),
                        OsString::from(member),
                    )]),
                )
                .unwrap(),
            )
        })
        .collect();
    config
}

fn selected_operator(root: &Root, config: &Config) -> Settings {
    let _ = operator_profile(root, config, 0, 0);
    let text = fs::read_to_string(root.path("operator.json")).unwrap().replacen(
        '{',
        r#"{"convening":{"queries":1024,"operations":1,"authority":"convene_held_review"},"#,
        1,
    );
    let mut selected = Settings::decode(text.as_bytes()).unwrap();
    selected.exchange_limit = 1024;
    selected
}

fn bypass_with_checks(checks: u64) -> String {
    graph(true)
        .replace("\"checks\":32", &format!("\"checks\":{checks}"))
        .replace(
            &format!("\"read_bytes\":{}", 32 * TOPOLOGY_REFRESH_READ_BYTES),
            &format!("\"read_bytes\":{}", checks * TOPOLOGY_REFRESH_READ_BYTES),
        )
}

fn helper_runs(path: &Path) -> BTreeMap<u64, usize> {
    let mut rounds = BTreeMap::new();
    for line in fs::read_to_string(path).unwrap_or_default().lines() {
        let (round, _) = line.split_once(':').unwrap();
        *rounds.entry(round.parse().unwrap()).or_default() += 1;
    }
    rounds
}

fn value<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let knowledge = field(native(response, kind), "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    field(knowledge, "value")
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn next_document(scope: Scope) -> String {
    let mut command = TowerRequest::new(
        "mediated-convene-plan",
        Address::parse(&format!(
            "fa://{}/attempt/request-{}-{}-{}-{}-7@{GENERATION}",
            scope.tenant, scope.run, scope.principal, scope.branch, scope.authority,
        ))
        .unwrap(),
        Verb::Next,
    );
    command.arguments = ReadArguments::ConveneNext { round: 8 };
    encode_request(&command).unwrap()
}

fn held_plan(socket: &mut UnixStream, scope: Scope) -> Json {
    let document = next_document(scope);
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let reply = exchange(socket, &document);
        if field(value(&reply, "convene-next"), "can_convene").as_bool() == Some(true) {
            return reply;
        }
        pause(10);
    }
}

fn action(scope: Scope, plan: &Json) -> String {
    let action = field(value(plan, "convene-next"), "action");
    let arguments = field(action, "arguments");
    let expected = field(arguments, "expected");
    let mut command = TowerRequest::new(
        field(action, "operation_id").as_str().unwrap(),
        Address::parse(field(action, "address").as_str().unwrap()).unwrap(),
        Verb::Convene,
    );
    command.idempotency_key = Some(
        field(action, "idempotency_key")
            .as_str()
            .unwrap()
            .to_owned(),
    );
    command.arguments = ReadArguments::Convene {
        round: number(field(arguments, "round")),
        expected: FileConveneBasis {
            scope,
            request: number(field(expected, "request")),
            request_generation: number(field(expected, "request_generation")),
            attempt: number(field(expected, "attempt")),
            held_round: number(field(expected, "held_round")),
            journal_revision: number(field(expected, "journal_revision")),
            control_sequence: number(field(expected, "control_sequence")),
            authority_epoch: number(field(expected, "authority_epoch")),
            input_revision: number(field(expected, "input_revision")),
            recorded_at: expected
                .get("recorded_at")
                .filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(number(tick))),
            deadline: ElapsedTick(number(field(expected, "deadline"))),
        },
    };
    let document = encode_request(&command).unwrap();
    assert_eq!(&parse(document.as_bytes()), action);
    document
}

fn outcome(response: &Json, expected: &str) {
    let receipt = value(response, "convene");
    assert_eq!(field(receipt, "outcome").as_str(), Some(expected));
    assert_eq!(
        field(receipt, "round_opened").as_bool(),
        Some(expected == "opened")
    );
}

fn saved_source(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

#[test]
fn mediated_convene_publishes_with_fresh_keys_and_one_source_across_checked_schedules() {
    for scheduled in [false, true] {
        for checked in [false, true] {
            let root = Root::new();
            let config = held_configuration(&root);
            evidence(&root, &config);
            let (mut actor, reviewer) = profiles(&root, &config);
            actor.reply_ms = 500;
            let operator = selected_operator(&root, &config);
            let archive = root.path("convened-source.bin");
            // Initial qualification, successor qualification, and one check
            // per actual publication. No new source allowance is issued.
            let checks = if scheduled { 4 } else { 3 };
            let selected = selected_checks(&root, &archive, checks);
            let publication = checked.then(|| publication(&root));
            let second_action = checked.then(|| {
                write_pair(&root, &action_for(&root, &config, 1, 1, b"first"));
                action_for(&root, &config, 2, 2, b"second")
            });
            let first = document(&config, 7, 1, "6669727374");
            let second = document(&config, 8, 2, "7365636f6e64");
            let first_poll = poll(&config, 7);
            let second_poll = poll(&config, 8);
            let selected_scope = config.profile.delivery.scope;
            let store = config.store.clone();
            let bootstrap = config.profile.clone();
            let peer_store = store.clone();
            let peer_bootstrap = bootstrap.clone();
            let actor_peer = actor.clone();
            let operator_peer = operator.clone();
            let reviewer_peer = reviewer.clone();
            let root_path = root.0.clone();
            let source_bytes = fs::read(root.path("evidence.json")).unwrap();
            let peer = std::thread::spawn(move || {
                let mut actor = connect_actor(&actor_peer);
                let mut operator = connect_operator(&operator_peer);
                exchange(&mut actor, &first);
                let plan = held_plan(&mut operator, selected_scope);
                let command = action(selected_scope, &plan);
                let log = root_path.join("held-helper-runs");
                assert_eq!(helper_runs(&log), BTreeMap::from([(7, 2)]));
                fs::write(root_path.join("helpers-restored"), b"available").unwrap();
                // Restored helpers and actor polls cannot select a new round.
                for _ in 0..3 {
                    assert_eq!(
                        field(request_knowledge(&exchange(&mut actor, &first)), "state").as_str(),
                        Some("pending")
                    );
                }
                assert_eq!(helper_runs(&log), BTreeMap::from([(7, 2)]));
                let refused = exchange(&mut actor, &command);
                assert_eq!(
                    field(field(&refused, "envelope"), "reason_code").as_str(),
                    Some("CapabilityUnsupported")
                );
                outcome(&exchange(&mut operator, &command), "pending");
                drop(operator);
                wait(&reviewer_peer.socket(7));
                let before = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
                assert_eq!((before.executions, before.control.ledger.charged), (0, 0));
                let mut operator = connect_operator(&operator_peer);
                let opened = exchange(&mut operator, &command);
                outcome(&opened, "opened");
                assert_eq!(helper_runs(&log), BTreeMap::from([(7, 2), (8, 2)]));
                let human = human_for(reviewer_peer.clone(), 7);
                assert_executed(&finish(&mut actor, &first, &first_poll));
                human.join().unwrap();
                assert_eq!(exchange(&mut operator, &command), opened);
                if scheduled {
                    wait(&crate::workflow::control::socket_path(&reviewer_peer, 8));
                    if let Some(action) = second_action {
                        let fixture = capture(&action, 1, &[0, 2, 4]);
                        let bytes = FilePublicationCapture::new(
                            2,
                            fixture.identity(),
                            &action,
                            fixture.inputs().clone(),
                        )
                        .unwrap()
                        .to_bytes()
                        .unwrap();
                        fs::write(root_path.join("original.bin"), &bytes).unwrap();
                        fs::write(root_path.join("current.bin"), &bytes).unwrap();
                    }
                    let human = human_for(reviewer_peer.clone(), 8);
                    assert_executed(&finish(&mut actor, &second, &second_poll));
                    human.join().unwrap();
                    // Successor 8 cannot collide with request 8's initial round.
                    assert_eq!(helper_runs(&log), BTreeMap::from([(7, 2), (8, 2), (9, 2)]));
                    assert_eq!(exchange(&mut operator, &command), opened);
                }
            });
            let result = if scheduled {
                service::serve_tower_with_topology(
                    config, &actor, &reviewer, publication.as_ref(),
                    Options { requests: &[7, 8], open: false, credibility: None },
                    GENERATION, Some(&operator), selected, || ElapsedTick(1000),
                )
            } else {
                serve_selected_ingress(
                    config, &actor, &reviewer, publication.as_ref(), false, None,
                    Some(selected), None, Some(GENERATION), Some(&operator),
                    || ElapsedTick(1000),
                )
            };
            peer.join().unwrap();
            assert!(result.is_ok(), "scheduled={scheduled} checked={checked}: {result:?}");
            let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
            let effects = if scheduled { 2 } else { 1 };
            assert_eq!((state.executions, state.control.ledger.charged), (effects, effects * 7));
            assert_eq!(state.control.ledger.reserved, 0);
            assert!(state.control.ledger.stages.values().all(|stage| *stage == ActionState::Confirmed));
            assert!(state.stop.is_none());
            let saved = saved_source(&archive);
            assert_eq!(saved.work().checks, checks);
            assert_eq!(saved.work().reserved_read_bytes, checks * TOPOLOGY_REFRESH_READ_BYTES);
            assert_eq!(saved.history_anchor().revision(), state.revision);
            assert_eq!(fs::read(root.path("evidence.json")).unwrap(), source_bytes);
            assert!(!actor.socket.exists() && !operator.socket.exists());
        }
    }
}

#[test]
fn mediated_convene_refuses_changed_missing_or_exhausted_topology_without_second_helpers() {
    for scheduled in [false, true] {
        for fault in ["bypass", "missing", "exhausted"] {
            let root = Root::new();
            let config = held_configuration(&root);
            evidence(&root, &config);
            let (mut actor, reviewer) = profiles(&root, &config);
            actor.reply_ms = 1000;
            let operator = selected_operator(&root, &config);
            let archive = root.path("failed-convene-source.bin");
            let checks = if fault == "exhausted" { 1 } else { 3 };
            let selected = selected_checks(&root, &archive, checks);
            let first = proposal_document(&config);
            let poll = request_document(&config, "get");
            let selected_scope = config.profile.delivery.scope;
            let store = config.store.clone();
            let bootstrap = config.profile.clone();
            let actor_peer = actor.clone();
            let operator_peer = operator.clone();
            let root_path = root.0.clone();
            let peer = std::thread::spawn(move || {
                let mut actor = connect_actor(&actor_peer);
                let mut operator = connect_operator(&operator_peer);
                exchange(&mut actor, &first);
                let plan = held_plan(&mut operator, selected_scope);
                let command = action(selected_scope, &plan);
                fs::write(root_path.join("helpers-restored"), b"available").unwrap();
                match fault {
                    "bypass" => fs::write(root_path.join("topology.json"), bypass_with_checks(checks)).unwrap(),
                    "missing" => fs::remove_file(root_path.join("inventory.json")).unwrap(),
                    "exhausted" => {}
                    _ => unreachable!(),
                }
                outcome(&exchange(&mut operator, &command), "pending");
                let started = Instant::now();
                let failed = loop {
                    assert!(started.elapsed() < Duration::from_secs(10));
                    let response = exchange(&mut operator, &command);
                    if field(value(&response, "convene"), "outcome").as_str() == Some("failed") {
                        break response;
                    }
                    pause(1);
                };
                outcome(&failed, "failed");
                assert!(field(value(&failed, "convene"), "opened").is_null());
                assert_eq!(exchange(&mut operator, &command), failed);
                assert_eq!(helper_runs(&root_path.join("held-helper-runs")), BTreeMap::from([(7, 2)]));
                let terminal = finish(&mut actor, &first, &poll);
                assert_ne!(request_knowledge(&terminal).get("value").and_then(Json::as_str), Some("executed"));
            });
            let result = if scheduled {
                service::serve_tower_with_topology(
                    config, &actor, &reviewer, None,
                    Options { requests: &[7], open: false, credibility: None },
                    GENERATION, Some(&operator), selected, || ElapsedTick(1000),
                )
            } else {
                serve_selected_ingress(
                    config, &actor, &reviewer, None, false, None, Some(selected),
                    None, Some(GENERATION), Some(&operator), || ElapsedTick(1000),
                )
            };
            peer.join().unwrap();
            assert!(result.is_err(), "{fault}: {result:?}");
            let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
            assert_eq!((state.executions, state.control.ledger.charged, state.control.ledger.reserved), (0, 0, 0));
            assert!(state.stop.is_some());
            let saved = saved_source(&archive);
            assert_eq!(saved.history_anchor().revision(), state.revision);
            assert_eq!(saved.work().checks, if fault == "exhausted" { 1 } else { 2 });
            assert!(!reviewer.socket(7).exists());
            assert!(!actor.socket.exists() && !operator.socket.exists());
        }
    }
}

#[test]
fn mediated_convened_publication_still_checks_final_topology_after_both_keys() {
    for changed in [false, true] {
        let root = Root::new();
        let config = held_configuration(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 500;
        let operator = selected_operator(&root, &config);
        let archive = root.path("final-convene-source.bin");
        let selected = selected_checks(&root, &archive, 3);
        let proposal = proposal_document(&config);
        let poll = request_document(&config, "get");
        let selected_scope = config.profile.delivery.scope;
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let actor_peer = actor.clone();
        let operator_peer = operator.clone();
        let reviewer_peer = reviewer.clone();
        let root_path = root.0.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect_actor(&actor_peer);
            let mut operator = connect_operator(&operator_peer);
            exchange(&mut actor, &proposal);
            let plan = held_plan(&mut operator, selected_scope);
            fs::write(root_path.join("helpers-restored"), b"available").unwrap();
            outcome(&exchange(&mut operator, &action(selected_scope, &plan)), "pending");
            let human = human_for(reviewer_peer, 7);
            if changed {
                // An unresolved final boundary is allowed to remain Unknown.
                // Do not poll it into an invented terminal acknowledgment.
                human.join().unwrap();
            } else {
                assert_executed(&finish(&mut actor, &proposal, &poll));
                human.join().unwrap();
            }
        });
        let mut changed_at_dispatch = false;
        let result = serve_selected_ingress(
            config, &actor, &reviewer, None, false, None, Some(selected),
            None, Some(GENERATION), Some(&operator), || {
                if changed && !changed_at_dispatch
                    && let Ok(state) = FileOversight::read_publication(&store, &bootstrap)
                    && state.control.ledger.charged == 7 && state.executions == 0
                {
                    fs::write(root.path("topology.json"), bypass_with_checks(3)).unwrap();
                    changed_at_dispatch = true;
                }
                ElapsedTick(1000)
            },
        );
        peer.join().unwrap();
        if !changed {
            assert!(result.is_ok(), "{result:?}");
        }
        assert_eq!(changed_at_dispatch, changed);
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(state.executions, u64::from(!changed));
        assert_eq!(state.control.ledger.reserved, 0);
        let stage = *state.control.ledger.stages.values().next().unwrap();
        if !changed {
            assert_eq!(state.control.ledger.charged, 7);
            assert_eq!(stage, ActionState::Confirmed);
        } else if state.control.ledger.charged == 0 {
            // Only original native proof of nonexecution can refund dispatch.
            assert_eq!(stage, ActionState::ConfirmedNotExecuted);
        } else {
            assert_eq!(state.control.ledger.charged, 7);
            assert!(matches!(
                stage,
                ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown
            ));
        }
        assert_eq!(
            state.control.ledger.available + state.control.ledger.charged,
            bootstrap.delivery.total
        );
        let saved = saved_source(&archive);
        assert_eq!(saved.work().checks, 3);
        assert_eq!(saved.history_anchor().revision(), state.revision);
    }
}
