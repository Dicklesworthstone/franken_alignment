//! Actual child congress rounds and independent operator/human sockets. The
//! synthetic helper recovery fixture establishes plumbing, not model quality.
use super::operator::settings_document;
use super::series::{executed, finish, human_for, poll, stop_request};
use super::*;
use crate::workflow::actor_service::operator::Settings;
use crate::workflow::actor_service::series::{self as service, Options};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileConveneBasis, MAX_CONVENE_OPERATIONS, MAX_CONVENE_QUERIES,
};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::{ActionState, Scope};
use fa_reference::address_resolver::Address;
use fa_reference::round::Verdict;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::fs::OpenOptions;

const CHILD: &str = "workflow::actor_service::tests::tower_service::convening::held_helper_process";
const MEMBER: &str = "FA_HELD_HELPER_MEMBER";

fn held_config(root: &Directory) -> Config {
    let mut config = configured(root);
    config.programs = ["alpha", "beta"]
        .into_iter()
        .map(|member| {
            (
                member.to_owned(),
                HelperProgram::new(
                    std::env::current_exe().unwrap(),
                    root.0.clone(),
                    vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
                    BTreeMap::from([(OsString::from(MEMBER), OsString::from(member))]),
                )
                .unwrap(),
            )
        })
        .collect();
    config
}

/// The exact same executable/arguments/profile first abstains and later becomes
/// available. Only an explicit new round may consult that restored helper.
#[test]
fn held_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else {
        return;
    };
    let profile = Config::decode(include_bytes!(
        "../../../../../../fixtures/supervised_publication.json"
    ))
    .unwrap()
    .profile;
    let mut client =
        HelperClient::from_process_stdin(profile.committee.members()[&member].profile_at(0))
            .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let expected = format!("review this fixture: {member}");
            assert!(
                input
                    .actual_input()
                    .submitted_bytes()
                    .windows(expected.len())
                    .any(|bytes| bytes == expected.as_bytes())
            );
            let mut runs = OpenOptions::new()
                .create(true)
                .append(true)
                .open("held-helper-runs")
                .unwrap();
            let recorded = format!("{}:{member}\n", input.round());
            runs.write_all(recorded.as_bytes()).unwrap();
            let verdict = if Path::new("helpers-restored").exists() {
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

fn runs(path: &Path) -> BTreeMap<u64, usize> {
    let mut observed = BTreeMap::new();
    for line in fs::read_to_string(path).unwrap_or_default().lines() {
        let (round, _) = line.split_once(':').unwrap();
        *observed.entry(round.parse().unwrap()).or_default() += 1;
    }
    observed
}

fn selected_document(base: &str, selection: &str) -> String {
    base.replacen('{', &format!("{{\"convening\":{selection},"), 1)
}

fn settings(root: &Directory, config: &Config, operations: usize) -> Settings {
    let selection = format!(
        r#"{{"queries":1024,"operations":{operations},"authority":"convene_held_review"}}"#,
    );
    let mut settings = Settings::decode(
        selected_document(&settings_document(root, config, 0, 0), &selection).as_bytes(),
    )
    .unwrap();
    settings.exchange_limit = 1024;
    settings
}

fn connect_operator(settings: &Settings) -> UnixStream {
    wait(&settings.socket);
    let socket = UnixStream::connect(&settings.socket).unwrap();
    settings.supervisor.verify(&socket).unwrap();
    socket
}

fn request_address(scope: Scope, request: u64) -> Address {
    Address::parse(&format!(
        "fa://{}/attempt/request-{}-{}-{}-{}-{request}@{GENERATION}",
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority,
    ))
    .unwrap()
}

fn next_document(scope: Scope, request: u64, round: u64) -> String {
    let mut command = TowerRequest::new(
        format!("convene-plan-{round}"),
        request_address(scope, request),
        Verb::Next,
    );
    command.arguments = ReadArguments::ConveneNext { round };
    encode_request(&command).unwrap()
}

fn known<'a>(reply: &'a Json, kind: &str) -> &'a Json {
    let result = field(reply, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    let observed = field(field(result, "value"), "knowledge");
    assert_eq!(field(observed, "state").as_str(), Some("known"));
    field(observed, "value")
}

fn held_plan(socket: &mut UnixStream, scope: Scope, request: u64, round: u64) -> Json {
    let document = next_document(scope, request, round);
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let reply = raw_exchange(socket, &document);
        if field(known(&reply, "convene-next"), "can_convene").as_bool() == Some(true) {
            return reply;
        }
        pause(10);
    }
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn action(scope: Scope, plan: &Json) -> String {
    let action = field(known(plan, "convene-next"), "action");
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
    let encoded = encode_request(&command).unwrap();
    assert_eq!(&parse(encoded.as_bytes()), action);
    encoded
}

fn outcome<'a>(reply: &'a Json, expected: &str) -> &'a Json {
    let value = known(reply, "convene");
    assert_eq!(field(value, "outcome").as_str(), Some(expected));
    assert_eq!(
        field(value, "round_opened").as_bool(),
        Some(expected == "opened")
    );
    value
}

fn checked(root: &Directory, config: &Config) -> PublicationProfile {
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
        FilePublicationInputs,
        producer::{FilePublicationProducer, PublicationProducerProfile},
    };
    use fa_reference::full_input::{
        ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart,
    };
    let actual = ActualHelperInput::new(
        b"Q".to_vec(),
        InputProfileBinding {
            profile_id: 1,
            profile_bytes: Vec::new(),
            tokenizer_epoch: 1,
            policy_epoch: 0,
            model_epoch: 1,
        },
        vec![SubmittedPart {
            span: ByteSpan { start: 0, end: 1 },
            kind: PartKind::Question,
        }],
        Vec::new(),
    )
    .unwrap();
    let (_producer, _) = FilePublicationProducer::create(
        root.0.join("producer"),
        PublicationProducerProfile {
            source: 91,
            scope: config.profile.delivery.scope,
            feed: 41,
            clock_domain: CLOCK_DOMAIN,
            after: 0,
        },
        FilePublicationInputs::new(None, Some(actual)),
        ElapsedTick(1000),
    )
    .unwrap();
    PublicationProfile::decode(format!(
        r#"{{"schema":"fa.supervised-whole-input/1","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
        root.0.display(), scope(config),
    ).as_bytes()).unwrap()
}

#[test]
fn explicit_same_source_successor_preserves_both_keys_checked_publication_and_scheduled_rounds() {
    for with_checked in [false, true] {
        let root = Directory::new();
        let config = held_config(&root);
        evidence(&root, &config);
        let source = root.0.join("evidence.json");
        let original_source = fs::read(&source).unwrap();
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 500;
        let settings = settings(&root, &config, 1);
        let publication = with_checked.then(|| checked(&root, &config));
        let selected_scope = config.profile.delivery.scope;
        let first = proposal_document(&config, 7);
        let first_poll = get_document(&config);
        let second = super::series::document(&config, 8, 2, "7365636f6e64");
        let second_poll = poll(&config, 8);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let peer_store = store.clone();
        let peer_bootstrap = bootstrap.clone();
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let human_profile = reviewer.clone();
        let marker = root.0.join("helpers-restored");
        let child_log = root.0.join("held-helper-runs");
        let peer_log = child_log.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let mut operator = connect_operator(&operator_peer);
            assert_eq!(
                field(knowledge(&raw_exchange(&mut actor, &first)), "state").as_str(),
                Some("pending")
            );
            // Round 8 deliberately occupies the next scheduled request's old
            // preferred round ID. The original allocator must later choose 9.
            let plan = held_plan(&mut operator, selected_scope, 7, 8);
            let command = action(selected_scope, &plan);
            assert_eq!(runs(&peer_log), BTreeMap::from([(7, 2)]));
            fs::write(&marker, b"helper availability restored").unwrap();
            for _ in 0..3 {
                assert_eq!(
                    field(knowledge(&raw_exchange(&mut actor, &first)), "state").as_str(),
                    Some("pending")
                );
            }
            let actor_refusal = raw_exchange(&mut actor, &command);
            assert_eq!(
                field(field(&actor_refusal, "envelope"), "reason_code").as_str(),
                Some("CapabilityUnsupported")
            );
            assert_eq!(runs(&peer_log), BTreeMap::from([(7, 2)]));
            assert!(!human_profile.socket(7).exists());
            let queued = raw_exchange(&mut operator, &command);
            outcome(&queued, "pending");
            assert!(field(known(&queued, "convene"), "opened").is_null());
            drop(operator);
            wait(&human_profile.socket(7));
            let mut operator = connect_operator(&operator_peer);
            let opened = raw_exchange(&mut operator, &command);
            outcome(&opened, "opened");
            assert_eq!(runs(&peer_log), BTreeMap::from([(7, 2), (8, 2)]));
            assert_eq!(
                field(knowledge(&raw_exchange(&mut actor, &first_poll)), "state").as_str(),
                Some("pending")
            );
            let before_human =
                FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            assert_eq!(
                (
                    before_human.executions,
                    before_human.control.ledger.reserved
                ),
                (0, 0)
            );
            let human = human_for(human_profile.clone(), 7);
            assert!(executed(&finish(&mut actor, &first, &first_poll)));
            human.join().unwrap();
            assert_eq!(raw_exchange(&mut operator, &command), opened);
            let human = human_for(human_profile.clone(), 8);
            assert!(executed(&finish(&mut actor, &second, &second_poll)));
            human.join().unwrap();
            assert_eq!(raw_exchange(&mut operator, &command), opened);
            assert_eq!(runs(&peer_log), BTreeMap::from([(7, 2), (8, 2), (9, 2)]));
        });
        let result = service::serve_tower(
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
            Some(&settings),
            || ElapsedTick(1000),
        );
        peer.join().unwrap();
        assert!(result.is_ok(), "checked={with_checked}: {result:?}");
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(
            (
                after.executions,
                after.control.ledger.charged,
                after.control.ledger.reserved
            ),
            (2, 14, 0)
        );
        assert_eq!(after.payload, b"second");
        assert!(
            after
                .control
                .ledger
                .stages
                .values()
                .all(|stage| *stage == ActionState::Confirmed)
        );
        assert_eq!(fs::read(&source).unwrap(), original_source);
        assert_eq!(runs(&child_log), BTreeMap::from([(7, 2), (8, 2), (9, 2)]));
    }
}

#[test]
fn absent_convening_keeps_default_hold_cancellation_and_missing_source_never_relaunches() {
    for selected in [false, true] {
        let root = Directory::new();
        let config = held_config(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 500;
        let settings = selected.then(|| settings(&root, &config, 1));
        let selected_scope = config.profile.delivery.scope;
        let proposal = proposal_document(&config, 7);
        let poll = get_document(&config);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let source = root.0.join("evidence.json");
        let child_log = root.0.join("held-helper-runs");
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            if let Some(settings) = operator_peer {
                let mut operator = connect_operator(&settings);
                raw_exchange(&mut actor, &proposal);
                let plan = held_plan(&mut operator, selected_scope, 7, 8);
                fs::remove_file(&source).unwrap();
                let command = action(selected_scope, &plan);
                outcome(&raw_exchange(&mut operator, &command), "pending");
                let started = Instant::now();
                loop {
                    assert!(started.elapsed() < Duration::from_secs(10));
                    let response = raw_exchange(&mut operator, &command);
                    if field(known(&response, "convene"), "outcome").as_str() == Some("failed") {
                        outcome(&response, "failed");
                        break;
                    }
                    pause(1);
                }
            }
            let terminal = finish(&mut actor, &proposal, &poll);
            assert!(!executed(&terminal));
        });
        let result = serve_selected_ingress(
            config,
            &actor,
            &reviewer,
            None,
            false,
            None,
            None,
            None,
            Some(GENERATION),
            settings.as_ref(),
            || ElapsedTick(1000),
        );
        peer.join().unwrap();
        assert_eq!(result.is_err(), selected);
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(
            (
                after.executions,
                after.control.ledger.charged,
                after.control.ledger.reserved
            ),
            (0, 0, 0)
        );
        assert!(
            after
                .control
                .ledger
                .stages
                .values()
                .all(|stage| *stage == ActionState::Cancelled)
        );
        assert_eq!(runs(&child_log), BTreeMap::from([(7, 2)]));
        assert!(!reviewer.socket(7).exists());
    }
}

#[test]
fn failed_convened_capture_attempts_automatic_stop_drain_once_across_terminal_replies() {
    use std::cell::Cell;
    for scheduled in [false, true] {
        let root = Directory::new();
        let config = held_config(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 500;
        let settings = settings(&root, &config, 1);
        let selected_scope = config.profile.delivery.scope;
        let proposal = proposal_document(&config, 7);
        let poll = get_document(&config);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let peer_store = store.clone();
        let source = root.0.join("evidence.json");
        let source_clock = source.clone();
        let child_log = root.0.join("held-helper-runs");
        let stop_socket = crate::workflow::control::socket_path(&reviewer, 7);
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let mut operator = connect_operator(&operator_peer);
            raw_exchange(&mut actor, &proposal);
            let plan = held_plan(&mut operator, selected_scope, 7, 8);
            fs::remove_file(source).unwrap();
            let command = action(selected_scope, &plan);
            outcome(&raw_exchange(&mut operator, &command), "pending");
            let started = Instant::now();
            let failed = loop {
                assert!(started.elapsed() < Duration::from_secs(10));
                let response = raw_exchange(&mut operator, &command);
                if field(known(&response, "convene"), "outcome").as_str() == Some("failed") {
                    outcome(&response, "failed");
                    break response;
                }
                pause(1);
            };
            assert!(
                !stop_socket.exists(),
                "the automatic fallback retired its unused control cache"
            );
            assert!(
                operator_peer.socket.exists(),
                "original finite operator replies survive"
            );
            let canonical = super::operator::store_files(&peer_store);
            for _ in 0..3 {
                assert_eq!(raw_exchange(&mut operator, &command), failed);
            }
            assert_eq!(super::operator::store_files(&peer_store), canonical);
            assert!(!executed(&finish(&mut actor, &proposal, &poll)));
        });
        let drain_clocks = Cell::new(0);
        let time = || {
            if !source_clock.exists() {
                let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
                if state.stop.is_some() {
                    let calls = drain_clocks.get() + 1;
                    drain_clocks.set(calls);
                    assert_eq!(
                        calls, 1,
                        "terminal reply grace retried the failed native drain"
                    );
                    return ElapsedTick(999);
                }
            }
            ElapsedTick(1000)
        };
        let result = if scheduled {
            service::serve_tower(
                config,
                &actor,
                &reviewer,
                None,
                Options {
                    requests: &[7],
                    open: false,
                    credibility: None,
                },
                GENERATION,
                Some(&settings),
                time,
            )
        } else {
            serve_selected_ingress(
                config,
                &actor,
                &reviewer,
                None,
                false,
                None,
                None,
                None,
                Some(GENERATION),
                Some(&settings),
                time,
            )
        };
        peer.join().unwrap();
        assert!(result.is_err());
        assert_eq!(drain_clocks.get(), 1);
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert!(after.stop.is_some());
        assert_eq!(
            (
                after.executions,
                after.control.ledger.charged,
                after.control.ledger.reserved
            ),
            (0, 0, 0)
        );
        assert_eq!(runs(&child_log), BTreeMap::from([(7, 2)]));
        assert!(!reviewer.socket(7).exists());
    }
}

#[test]
fn held_wait_preserves_independent_stop_and_original_deadline_without_a_successor() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    for expired in [false, true] {
        let root = Directory::new();
        let config = held_config(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let settings = settings(&root, &config, 1);
        let selected_scope = config.profile.delivery.scope;
        let proposal = proposal_document(&config, 7);
        let poll = get_document(&config);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let source = root.0.join("evidence.json");
        let child_log = root.0.join("held-helper-runs");
        let actor_peer = actor.clone();
        let operator_peer = settings.clone();
        let stop_profile = reviewer.clone();
        let clock = Arc::new(AtomicU64::new(1000));
        let peer_clock = Arc::clone(&clock);
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let mut operator = connect_operator(&operator_peer);
            raw_exchange(&mut actor, &proposal);
            let plan = held_plan(&mut operator, selected_scope, 7, 8);
            assert!(
                field(known(&plan, "convene-next"), "action")
                    .as_object()
                    .is_some()
            );
            fs::remove_file(source).unwrap();
            if expired {
                peer_clock.store(1000 + actor_peer.runtime_ms, Ordering::SeqCst);
            } else {
                stop_request(&stop_profile, 7);
            }
            assert!(!executed(&finish(&mut actor, &proposal, &poll)));
        });
        let result = serve_selected_ingress(
            config,
            &actor,
            &reviewer,
            None,
            false,
            None,
            None,
            None,
            Some(GENERATION),
            Some(&settings),
            || ElapsedTick(clock.load(Ordering::SeqCst)),
        );
        peer.join().unwrap();
        assert_eq!(result.is_err(), expired, "{result:?}");
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert!(after.stop.is_some());
        assert_eq!(
            (
                after.executions,
                after.control.ledger.charged,
                after.control.ledger.reserved
            ),
            (0, 0, 0)
        );
        assert_eq!(runs(&child_log), BTreeMap::from([(7, 2)]));
    }
}

#[test]
fn convening_profile_is_closed_bounded_and_refuses_numerical_selection() {
    let root = Directory::new();
    let config = held_config(&root);
    let old = settings_document(&root, &config, 1, 0);
    for document in [old.clone(), selected_document(&old, "null")] {
        let settings = Settings::decode(document.as_bytes()).unwrap();
        assert!(settings.convening.is_none());
    }
    let selection = format!(
        r#"{{"queries":{MAX_CONVENE_QUERIES},"operations":{MAX_CONVENE_OPERATIONS},"authority":"convene_held_review"}}"#,
    );
    let chosen = Settings::decode(selected_document(&old, &selection).as_bytes()).unwrap();
    assert!(chosen.check_generated_operator().is_err());
    assert!(chosen.check_learned_operator().is_err());
    for invalid in [
        selection.replace(
            &format!("\"queries\":{MAX_CONVENE_QUERIES}"),
            "\"queries\":0",
        ),
        selection.replace(
            &format!("\"operations\":{MAX_CONVENE_OPERATIONS}"),
            "\"operations\":0",
        ),
        selection.replace(
            &MAX_CONVENE_QUERIES.to_string(),
            &(MAX_CONVENE_QUERIES + 1).to_string(),
        ),
        selection.replace(
            &format!("\"operations\":{MAX_CONVENE_OPERATIONS}"),
            &format!("\"operations\":{}", MAX_CONVENE_OPERATIONS + 1),
        ),
        selection.replace("convene_held_review", "approve_effect"),
        selection.replacen('{', "{\"renew_deadline\":true,", 1),
    ] {
        assert!(Settings::decode(selected_document(&old, &invalid).as_bytes()).is_err());
    }
    assert!(!config.store.exists());
    for scheduled in [false, true] {
        let root = Directory::new();
        let config = held_config(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let selected = settings(&root, &config, 1);
        let topology = crate::workflow::continuation::creation::topology::Selection::decode(
            br#"{"schema_version":"fa.mediated-create/1","topology_path":"/unread-convene-topology","inventory_path":"/unread-convene-inventory"}"#,
        ).unwrap();
        let result = if scheduled {
            service::serve_tower_with_topology(
                config,
                &actor,
                &reviewer,
                None,
                Options {
                    requests: &[7],
                    open: false,
                    credibility: None,
                },
                GENERATION,
                Some(&selected),
                topology,
                || panic!("unsupported topology convening sampled time"),
            )
        } else {
            serve_selected_ingress(
                config,
                &actor,
                &reviewer,
                None,
                false,
                None,
                None,
                Some(topology),
                Some(GENERATION),
                Some(&selected),
                || panic!("unsupported topology convening sampled time"),
            )
        };
        assert!(result.unwrap_err().contains("held-review convening"));
        assert!(!root.0.join("store").exists());
        assert!(!actor.socket.exists());
        assert!(!selected.socket.exists());
    }
}
