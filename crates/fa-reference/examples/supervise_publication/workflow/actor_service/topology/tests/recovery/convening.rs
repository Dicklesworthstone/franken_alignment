//! An explicit held successor after complete guarded recovery retains both
//! independent archives and the original cumulative topology work.
use super::*;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::address_resolver::Address;
use fa_reference::round::Verdict;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::fs::OpenOptions;

const HELD_CHILD: &str =
    "workflow::actor_service::topology::tests::recovery::convening::recovered_held_helper_process";

#[test]
fn recovered_held_helper_process() {
    let Ok(member) = std::env::var("FA_RECOVERY_CONVENE_MEMBER") else {
        return;
    };
    let epoch: u64 = std::env::var("FA_RECOVERY_CONVENE_EPOCH").unwrap().parse().unwrap();
    let profile = Config::decode(TEMPLATE.as_bytes()).unwrap().profile;
    let mut client =
        HelperClient::from_process_stdin(profile.committee.members()[&member].profile_at(epoch))
            .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let expected = format!("review this fixture: {member}");
            assert!(input.actual_input().submitted_bytes().windows(expected.len())
                .any(|bytes| bytes == expected.as_bytes()));
            let mut log = OpenOptions::new().create(true).append(true)
                .open("recovered-helper-runs").unwrap();
            writeln!(log, "{}:{member}", input.round()).unwrap();
            let verdict = if Path::new("recovered-helpers-restored").exists() {
                Verdict::Allow
            } else {
                Verdict::Abstain
            };
            client.respond(verdict, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        pause(1);
    }
}

fn configuration_with_held_helpers(root: &Root) -> Config {
    // The original recovery fixture independently supplies epoch three after
    // Fence, source rebind and initial topology qualification.
    helper_epoch(root, 3);
    let text = fs::read_to_string(root.path("config.json")).unwrap()
        .replace(CHILD, HELD_CHILD)
        .replace("FA_MEDIATED_SUBMIT_MEMBER", "FA_RECOVERY_CONVENE_MEMBER")
        .replace("FA_MEDIATED_SUBMIT_EPOCH", "FA_RECOVERY_CONVENE_EPOCH");
    fs::write(root.path("config.json"), &text).unwrap();
    Config::decode(text.as_bytes()).unwrap()
}

fn selected_operator(root: &Root, config: &Config) -> Settings {
    operator(root, config);
    let text = fs::read_to_string(root.path("operator.json")).unwrap()
        .replacen('{', r#"{"convening":{"queries":1024,"operations":1,"authority":"convene_held_review"},"#, 1)
        .replace("\"exchange_limit\":64", "\"exchange_limit\":1024");
    fs::write(root.path("operator.json"), &text).unwrap();
    Settings::decode(text.as_bytes()).unwrap()
}

fn known<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let observed = field(native(response, kind), "knowledge");
    assert_eq!(field(observed, "state").as_str(), Some("known"));
    field(observed, "value")
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn next(scope: Scope) -> String {
    let mut request = TowerRequest::new(
        "recovered-held-next",
        Address::parse(&format!(
            "fa://{}/attempt/request-{}-{}-{}-{}-8@{GENERATION}",
            scope.tenant, scope.run, scope.principal, scope.branch, scope.authority,
        )).unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::ConveneNext { round: 9 };
    encode_request(&request).unwrap()
}

fn action(scope: Scope, plan: &Json) -> String {
    let action = field(known(plan, "convene-next"), "action");
    let arguments = field(action, "arguments");
    let expected = field(arguments, "expected");
    let mut request = TowerRequest::new(
        field(action, "operation_id").as_str().unwrap(),
        Address::parse(field(action, "address").as_str().unwrap()).unwrap(),
        Verb::Convene,
    );
    request.idempotency_key = Some(field(action, "idempotency_key").as_str().unwrap().to_owned());
    request.arguments = ReadArguments::Convene {
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
            recorded_at: expected.get("recorded_at").filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(number(tick))),
            deadline: ElapsedTick(number(field(expected, "deadline"))),
        },
    };
    let encoded = encode_request(&request).unwrap();
    assert_eq!(&parse(encoded.as_bytes()), action);
    encoded
}

fn runs(path: &Path) -> BTreeMap<u64, usize> {
    let mut rounds = BTreeMap::new();
    for line in fs::read_to_string(path).unwrap_or_default().lines() {
        let (round, _) = line.split_once(':').unwrap();
        *rounds.entry(round.parse().unwrap()).or_default() += 1;
    }
    rounds
}

#[test]
fn recovered_plain_and_checked_convening_preserves_original_work_and_both_custody_exports() {
    for checked in [false, true] {
        for bypass in [false, true] {
            let root = Root::new();
            let (_, reviewer, before) = seed(&root, checked, 32);
            let config = configuration_with_held_helpers(&root);
            let actor = actor_profile(&root, 8);
            next_graph(&root, 2, 32);
            if checked {
                second_captures(&root, &config);
            }
            let settings = selected_operator(&root, &config);
            let submit = proposal(&config, 8, 3, 2);
            let poll = request_document(&config, 8, "get");
            let scope = config.profile.delivery.scope;
            let root_path = root.0.clone();
            let actor_peer = actor.clone();
            let operator_peer = settings.clone();
            let human_profile = reviewer.clone();
            let store = config.store.clone();
            let profile = config.profile.clone();
            let peer = std::thread::spawn(move || {
                let mut actor = connect(&actor_peer.socket);
                actor_peer.supervisor.verify(&actor).unwrap();
                let mut operator = connect(&operator_peer.socket);
                operator_peer.supervisor.verify(&operator).unwrap();
                assert_eq!(field(knowledge(&exchange(&mut actor, &submit)), "state").as_str(), Some("pending"));
                let document = next(scope);
                let started = Instant::now();
                let plan = loop {
                    assert!(started.elapsed() < Duration::from_secs(10));
                    let observed = exchange(&mut operator, &document);
                    if field(known(&observed, "convene-next"), "can_convene").as_bool() == Some(true) {
                        break observed;
                    }
                    pause(1);
                };
                assert_eq!(runs(&root_path.join("recovered-helper-runs")), BTreeMap::from([(8, 2)]));
                assert!(!human_profile.socket(8).exists());
                fs::write(root_path.join("recovered-helpers-restored"), b"available").unwrap();
                if bypass {
                    let graph = fs::read_to_string(root_path.join("topology.json")).unwrap();
                    let altered = graph.replace(
                        r#""edges":["#,
                        r#""edges":[{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103},"#,
                    );
                    assert_ne!(altered, graph);
                    fs::write(root_path.join("topology.json"), altered).unwrap();
                }
                let command = action(scope, &plan);
                assert_eq!(field(known(&exchange(&mut operator, &command), "convene"), "outcome").as_str(), Some("pending"));
                if bypass {
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(10));
                        let observed = exchange(&mut operator, &command);
                        if field(known(&observed, "convene"), "outcome").as_str() == Some("failed") {
                            assert_eq!(field(known(&observed, "convene"), "round_opened").as_bool(), Some(false));
                            assert_eq!(exchange(&mut operator, &command), observed);
                            break;
                        }
                        pause(1);
                    }
                    assert_eq!(runs(&root_path.join("recovered-helper-runs")), BTreeMap::from([(8, 2)]));
                } else {
                    drop(operator);
                    wait(&human_profile.socket(8));
                    let mut operator = connect(&operator_peer.socket);
                    operator_peer.supervisor.verify(&operator).unwrap();
                    let opened = exchange(&mut operator, &command);
                    assert_eq!(field(known(&opened, "convene"), "outcome").as_str(), Some("opened"));
                    let held = FileOversight::read_publication(&store, &profile).unwrap();
                    assert_eq!((held.executions, held.control.ledger.charged, held.control.ledger.reserved), (1, 7, 0));
                    let human = review(human_profile, 8, None, None);
                    assert_eq!(field(knowledge(&terminal(&mut actor, &poll)), "value").as_str(), Some("executed"));
                    human.join().unwrap();
                    assert_eq!(exchange(&mut operator, &command), opened);
                    assert_eq!(runs(&root_path.join("recovered-helper-runs")), BTreeMap::from([(8, 2), (9, 2)]));
                }
            });
            let result = command_with_clock(&arguments(&root, checked, true, true), None, || ElapsedTick(1001));
            peer.join().unwrap();
            assert_eq!(result.is_ok(), !bypass, "checked={checked}, bypass={bypass}: {result:?}");
            let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
            assert_eq!(state.executions, if bypass { 1 } else { 2 });
            assert_eq!(state.control.ledger.charged, if bypass { 7 } else { 13 });
            assert_eq!(state.control.ledger.reserved, 0);
            let after = read_checkpoint(&root, "second-checkpoint.bin");
            let anchor = FileHistoryAnchor::read_trusted(
                &mut Cursor::new(fs::read(root.path("second.anchor")).unwrap()),
                MAX_HISTORY_ANCHOR_BYTES,
            ).unwrap();
            assert_eq!(after.history_anchor(), &anchor);
            assert_eq!(anchor.revision(), state.revision);
            assert_eq!(after.minimum().control_sequence, state.control.sequence);
            assert_eq!(after.minimum().authority_epoch, state.control.ledger.epoch);
            assert_eq!(after.work().checks, before.work().checks + if bypass { 2 } else { 3 });
            assert_eq!(after.work().reserved_read_bytes,
                after.work().checks * TOPOLOGY_REFRESH_READ_BYTES);
            assert!(after.last_operation() > before.last_operation());
            assert_eq!(after.expected().graph().spec().generation, 2);
            assert_eq!(after.available(), !bypass);
        }
    }
}
