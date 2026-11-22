//! Real native Message/Finish, independent operator custody and child congresses.
//! Synthetic helper availability tests explicit continuation, not detector quality.
use super::*;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::round::Verdict;
use std::{collections::BTreeMap, ffi::OsString, fs, fs::OpenOptions, path::Path};

const CHILD: &str = "workflow::actor_service::native::sequence::tests::operator_tests::convening_tests::native_held_helper_process";
const MEMBER: &str = "FA_NATIVE_HELD_MEMBER";

fn hold_helpers(config: &mut Config, root: &Root) {
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
}

/// The original fixed process recipe abstains until the independently selected
/// round is made available. Neither a new source nor actor polling selects it.
#[test]
fn native_held_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else {
        return;
    };
    let profile = Config::decode(include_bytes!(
        "../../../../../../../../fixtures/supervised_publication.json"
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
            OpenOptions::new()
                .create(true)
                .append(true)
                .open("native-held-runs")
                .unwrap()
                .write_all(format!("{}:{member}\n", input.round()).as_bytes())
                .unwrap();
            let verdict = if Path::new(&format!("allow-native-{}", input.round())).exists() {
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
    let mut result = BTreeMap::new();
    for line in fs::read_to_string(path).unwrap_or_default().lines() {
        let (round, _) = line.split_once(':').unwrap();
        *result.entry(round.parse().unwrap()).or_default() += 1;
    }
    result
}

fn chosen(root: &Root, actor: &Profile, operations: usize) -> operator::Selection {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let settings = Settings::decode(format!(
        r#"{{"schema":"fa.tower-operator-service/1",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "socket":"{}/operator.sock",
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "operator":{{"uid":{},"gid":{},"pid":{}}},
        "candidate_limit":8,"connection_limit":8,"exchange_limit":1024,
        "runtime_ms":15000,"poll_ms":1,"inspection_queries":8,
        "explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none",
        "convening":{{"queries":1024,"operations":{operations},"authority":"convene_held_review"}},
        "fence":{{"max_replay_events":{MAX_FENCE_REPLAY_EVENTS},"numerical_replay_events":{MAX_FENCE_REPLAY_EVENTS},"authority":"stop_original_admission"}}}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        root.0.display(), identity.uid(), identity.gid(), identity.pid().unwrap(),
        identity.uid(), identity.gid(), identity.pid().unwrap(),
    ).as_bytes()).unwrap();
    operator::Selection::new(settings, 41).unwrap()
}

fn request_address(scope: Scope, request: u64) -> Address {
    Address::parse(&format!(
        "fa://{}/attempt/request-{}-{}-{}-{}-{request}@41",
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority
    ))
    .unwrap()
}

fn plan_request(scope: Scope, request: u64, round: u64) -> TowerRequest {
    let mut command = TowerRequest::new(
        format!("native-held-plan-{round}"),
        request_address(scope, request),
        Verb::Next,
    );
    command.arguments = ReadArguments::ConveneNext { round };
    command
}

fn value<'a>(reply: &'a Json, kind: &str) -> Option<&'a Json> {
    assert_eq!(field(reply, &["result", "kind"]).as_str(), Some(kind));
    let knowledge = field(reply, &["result", "value", "knowledge"]);
    (field(knowledge, &["state"]).as_str() == Some("known")).then(|| field(knowledge, &["value"]))
}

fn held_plan(socket: &mut UnixStream, settings: &Settings, request: u64, round: u64) -> Json {
    let command = plan_request(settings.scope, request, round);
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let reply = exchange(socket, &command, settings.response_bytes);
        if value(&reply, "convene-next")
            .is_some_and(|plan| field(plan, &["can_convene"]).as_bool() == Some(true))
        {
            return reply;
        }
        pause(10);
    }
}

fn convene(scope: Scope, plan: &Json) -> TowerRequest {
    let action = field(value(plan, "convene-next").unwrap(), &["action"]);
    let args = field(action, &["arguments"]);
    let expected = field(args, &["expected"]);
    let mut command = TowerRequest::new(
        field(action, &["operation_id"]).as_str().unwrap(),
        Address::parse(field(action, &["address"]).as_str().unwrap()).unwrap(),
        Verb::Convene,
    );
    command.idempotency_key = Some(field(action, &["idempotency_key"]).as_str().unwrap().into());
    command.arguments = ReadArguments::Convene {
        round: number(args, &["round"]),
        expected: FileConveneBasis {
            scope,
            request: number(expected, &["request"]),
            request_generation: number(expected, &["request_generation"]),
            attempt: number(expected, &["attempt"]),
            held_round: number(expected, &["held_round"]),
            journal_revision: number(expected, &["journal_revision"]),
            control_sequence: number(expected, &["control_sequence"]),
            authority_epoch: number(expected, &["authority_epoch"]),
            input_revision: number(expected, &["input_revision"]),
            recorded_at: field(expected, &["recorded_at"])
                .as_str()
                .map(|tick| ElapsedTick(tick.parse().unwrap())),
            deadline: ElapsedTick(number(expected, &["deadline"])),
        },
    };
    let encoded = encode_request(&command).unwrap();
    assert_eq!(
        &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap(),
        action
    );
    command
}

fn outcome(reply: &Json, expected: &str) {
    let receipt = value(reply, "convene").unwrap();
    assert_eq!(field(receipt, &["outcome"]).as_str(), Some(expected));
    assert_eq!(
        field(receipt, &["round_opened"]).as_bool(),
        Some(expected == "opened")
    );
}

#[test]
fn generated_convening_admission_preserves_recorded_refusals() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, true);
    let selected = chosen(&root, &entries[0].actor, 1);
    selected
        .check(&config, &entries[0].actor, &reviewer)
        .unwrap();
    selected.settings.check_learned_operator().unwrap();
    assert!(selected.check_recorded().is_err());
    // The convening selection alone must still choose the native control mount;
    // it is not permitted merely because this fixture also selects Fence.
    let mut only = selected.settings.clone();
    only.fence = None;
    only.inspection_queries = 0;
    let only = operator::Selection::new(only, 41).unwrap();
    only.check(&config, &entries[0].actor, &reviewer).unwrap();
    assert!(only.check_recorded().is_err());
    let mut plain = only.settings.clone();
    plain.convening = None;
    plain.inspection_queries = 1;
    operator::Selection::new(plain, 41)
        .unwrap()
        .check_recorded()
        .unwrap();
    assert!(!config.store.exists());
    assert!(!selected.settings.socket.exists());
}

#[test]
fn resident_native_held_messages_and_finish_need_explicit_rounds_and_fresh_human_keys() {
    for reject_finish in [false, true] {
        let root = Root::new();
        let (mut config, mut entries, reviewer) = selected(&root, true);
        hold_helpers(&mut config, &root);
        for entry in &mut entries {
            entry.actor.reply_ms = 500;
        }
        let reader = Reader::new(&config, &entries[0]);
        let mut selection = chosen(&root, &entries[0].actor, 3);
        // Exercise the generated control mount with ONLY the new grant.
        selection.settings.fence = None;
        selection.settings.inspection_queries = 0;
        let settings = selection.settings.clone();
        let store = reader.store.clone();
        let profile = reader.profile.clone();
        let recipe_path = root.0.join("recipe.json");
        let root_path = root.0.clone();
        let child_log = root.0.join("native-held-runs");
        let peer_log = child_log.clone();
        let human_profile = reviewer.clone();
        let (offered, offers) = mpsc::channel();
        let (release, decisions) = mpsc::channel();
        let human = held_reviews(reviewer.clone(), REQUESTS.to_vec(), offered, decisions);
        let observer = thread::spawn(move || {
            // Decoder assets contain Rc custody; construct this independent
            // read-only verifier inside its thread instead of moving the owner.
            let assets = recipe::load(
                &recipe_path,
                profile.delivery.scope.tenant,
                profile.delivery.limits.bytes,
            )
            .unwrap();
            let read = Reader {
                store,
                profile,
                decoder: assets.decoder,
                tokenizer: assets.tokenizer,
                stream: assets.stream,
            };
            let mut socket = connect(&settings);
            let inode = fs::metadata(&settings.socket).unwrap().ino();
            let mut expected_runs = BTreeMap::new();
            let mut first_receipt = None;
            for (index, request_id) in REQUESTS.into_iter().enumerate() {
                let initial_round = 91 + 2 * index as u64;
                let next_round = initial_round + 1;
                let plan = held_plan(&mut socket, &settings, request_id, next_round);
                let command = convene(settings.scope, &plan);
                let generation = if index == 0 { 7 } else { 8 };
                let before = read.progress(generation);
                expected_runs.insert(initial_round, 2);
                assert_eq!(runs(&peer_log), expected_runs);
                assert_eq!(before.publication.executions, index as u64);
                assert_eq!(before.publication.control.ledger.reserved, 0);
                assert!(!human_profile.socket(request_id).exists());
                fs::write(
                    root_path.join(format!("allow-native-{next_round}")),
                    b"available",
                )
                .unwrap();
                // Source/helper availability and repeated actor polls are not an
                // implicit request for another round. The original cut persists.
                for _ in 0..3 {
                    let unchanged = exchange(
                        &mut socket,
                        &plan_request(settings.scope, request_id, next_round),
                        settings.response_bytes,
                    );
                    assert_eq!(convene(settings.scope, &unchanged), command);
                    assert_eq!(runs(&peer_log), expected_runs);
                }
                outcome(
                    &exchange(&mut socket, &command, settings.response_bytes),
                    "pending",
                );
                let (human_request, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
                assert_eq!(human_request, request_id);
                expected_runs.insert(next_round, 2);
                assert_eq!(runs(&peer_log), expected_runs);
                let after = read.progress(generation);
                assert_eq!(
                    after.numerical.numerical, before.numerical.numerical,
                    "new review must not repeat prompt inference or sampling"
                );
                assert_eq!(after.text.command(), before.text.command());
                assert_eq!(
                    after.text.generation_revision(),
                    before.text.generation_revision()
                );
                assert_eq!(after.text.finish(), before.text.finish());
                assert_eq!(
                    after.text.bytes(),
                    before.text.bytes(),
                    "the original generated output remains frozen"
                );
                assert_eq!(after.publication.executions, index as u64);
                assert_eq!(after.publication.control.ledger.reserved, 0);
                let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
                assert_eq!(frame.is_finish(), index == 2);
                assert_eq!(frame.prior_messages(), vec!["A"; index.min(2)].as_slice());
                assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
                let opened = exchange(&mut socket, &command, settings.response_bytes);
                outcome(&opened, "opened");
                assert_eq!(fs::metadata(&settings.socket).unwrap().ino(), inode);
                if let Some((old_command, receipt)) = &first_receipt {
                    assert_eq!(
                        exchange(&mut socket, old_command, settings.response_bytes),
                        *receipt
                    );
                } else {
                    first_receipt = Some((command.clone(), opened));
                }
                if index == 1 {
                    drop(socket);
                    socket = connect(&settings);
                }
                release
                    .send(Some(if reject_finish && index == 2 {
                        ReviewDecision::Reject
                    } else {
                        ReviewDecision::Approve
                    }))
                    .unwrap();
            }
        });
        let mut output = Output::new(&entries);
        let actors = output.profiles.clone();
        let result = run_with_operator(
            config,
            entries,
            &reviewer,
            None,
            (None, Some(&selection)),
            || ElapsedTick(1000),
            &mut output,
        );
        observer.join().unwrap();
        let packets = human.join().unwrap();
        if reject_finish {
            assert!(
                result
                    .unwrap_err()
                    .contains("2 of 3 confirmed Executed releases")
            );
        } else {
            result.unwrap();
        }
        let replies = output.join();
        assert_eq!(replies.len(), 3);
        assert!(executed(&replies[0]) && executed(&replies[1]));
        assert_eq!(executed(&replies[2]), !reject_finish);
        let after = reader.stream();
        assert_eq!(
            after.publication.executions,
            if reject_finish { 2 } else { 3 }
        );
        assert_eq!(after.publication.payload, b"AA");
        assert_eq!(after.publication.control.ledger.reserved, 0);
        let paid = packets
            .iter()
            .take(if reject_finish { 2 } else { 3 })
            .map(|packet| packet.action().spec().units)
            .sum::<u64>();
        assert_eq!(after.publication.control.ledger.charged, paid);
        assert_eq!(
            runs(&child_log),
            BTreeMap::from([(91, 2), (92, 2), (93, 2), (94, 2), (95, 2), (96, 2)])
        );
        if !reject_finish {
            complete(
                &reader,
                &replies,
                &std::array::from_fn(|i| Some(packets[i].clone())),
            );
        }
        closed(&actors, &reviewer);
        assert!(!selection.settings.socket.exists());
    }
}

#[test]
fn resident_native_held_successor_allowance_is_not_renewed_by_release_or_reconnect() {
    let root = Root::new();
    let (mut config, mut entries, reviewer) = selected(&root, true);
    hold_helpers(&mut config, &root);
    for entry in &mut entries {
        entry.actor.reply_ms = 1000;
    }
    let reader = Reader::new(&config, &entries[0]);
    let selection = chosen(&root, &entries[0].actor, 1);
    let settings = selection.settings.clone();
    let actor_profile = entries[0].actor.clone();
    let marker = root.0.join("allow-native-92");
    let child_log = root.0.join("native-held-runs");
    let peer_log = child_log.clone();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(reviewer.clone(), vec![91], offered, decisions);
    let observer = thread::spawn(move || {
        let mut socket = connect(&settings);
        let plan = held_plan(&mut socket, &settings, 91, 92);
        let command = convene(settings.scope, &plan);
        fs::write(marker, b"available").unwrap();
        outcome(
            &exchange(&mut socket, &command, settings.response_bytes),
            "pending",
        );
        offers.recv_timeout(Duration::from_secs(20)).unwrap();
        let opened = exchange(&mut socket, &command, settings.response_bytes);
        outcome(&opened, "opened");
        release.send(Some(ReviewDecision::Approve)).unwrap();
        drop(socket);
        socket = connect(&settings);
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            let next = exchange(
                &mut socket,
                &plan_request(settings.scope, 92, 94),
                settings.response_bytes,
            );
            if let Some(plan) = value(&next, "convene-next") {
                if field(plan, &["block_reason"]).as_str() == Some("operation_limit") {
                    assert_eq!(field(plan, &["can_convene"]).as_bool(), Some(false));
                    break;
                }
            }
            pause(10);
        }
        assert_eq!(runs(&peer_log), BTreeMap::from([(91, 2), (92, 2), (93, 2)]));
        assert_eq!(
            exchange(&mut socket, &command, settings.response_bytes),
            opened
        );
        let situation = exchange(
            &mut socket,
            &request(&actor_profile, "held-budget", Verb::Situation),
            settings.response_bytes,
        );
        known(&situation);
        assert_eq!(summary(&situation, "executions"), 1);
        let stop = fence(&actor_profile, &situation);
        let acknowledged = exchange(&mut socket, &stop, settings.response_bytes);
        assert_eq!(
            field(&acknowledged, &["result", "kind"]).as_str(),
            Some("fence")
        );
    });
    let mut output = Output::new(&entries);
    let result = run_with_operator(
        config,
        entries,
        &reviewer,
        None,
        (None, Some(&selection)),
        || ElapsedTick(1000),
        &mut output,
    );
    observer.join().unwrap();
    human.join().unwrap();
    assert!(
        result
            .unwrap_err()
            .contains("1 of 3 confirmed Executed releases")
    );
    let replies = output.join();
    assert_eq!(replies.len(), 2);
    assert!(executed(&replies[0]));
    assert!(!executed(&replies[1]));
    let after = reader.publication();
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"A");
    assert_eq!(after.control.ledger.reserved, 0);
    assert!(after.stop.is_some());
    assert_eq!(
        runs(&child_log),
        BTreeMap::from([(91, 2), (92, 2), (93, 2)])
    );
}

#[test]
fn single_native_hold_keeps_default_cancellation_without_a_convening_grant() {
    let root = Root::new();
    let (mut config, mut entries, reviewer) = selected(&root, true);
    hold_helpers(&mut config, &root);
    let mut entry = entries.remove(0);
    entry.actor.reply_ms = 500;
    let reader = Reader::new(&config, &entry);
    // A restored future helper and ordinary actor polls do not select its round.
    fs::write(root.0.join("allow-native-92"), b"available").unwrap();
    let selection = selection(&root, &entry.actor, 1, false);
    let mut output = Output::new(std::slice::from_ref(&entry));
    let result = crate::workflow::actor_service::native::serve_lifecycle_selected(
        config,
        &entry.actor,
        &reviewer,
        entry.inputs,
        (false, None, None, finish::Intent::Existing),
        Some(&selection),
        || ElapsedTick(1000),
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    let replies = output.join();
    assert_eq!(replies.len(), 1);
    assert!(matches!(
        &replies[0].result,
        Ok(Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        })
    ));
    assert_eq!(
        runs(&root.0.join("native-held-runs")),
        BTreeMap::from([(91, 2)])
    );
    let after = reader.progress(7);
    assert_eq!(after.text.bytes().unwrap(), b"A");
    assert_eq!(after.text.generation_revision(), 4);
    assert_eq!(after.publication.executions, 0);
    assert_eq!(after.publication.control.ledger.charged, 0);
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert!(after.publication.stop.is_none());
    assert!(!reviewer.socket(entry.actor.request).exists());
}

#[test]
fn single_native_held_continuation_preserves_source_failure_revocation_and_original_stop() {
    for mode in ["source", "revoked", "stop"] {
        let root = Root::new();
        let (mut config, mut entries, reviewer) = selected(&root, true);
        hold_helpers(&mut config, &root);
        let mut entry = entries.remove(0);
        entry.actor.reply_ms = 1000;
        let reader = Reader::new(&config, &entry);
        let selection = chosen(&root, &entry.actor, 1);
        let settings = selection.settings.clone();
        let source = config.source.path().to_path_buf();
        let marker = root.0.join("allow-native-92");
        let child_log = root.0.join("native-held-runs");
        let peer_log = child_log.clone();
        let peer_reviewer = reviewer.clone();
        let observer = thread::spawn(move || {
            let mut socket = connect(&settings);
            let plan = held_plan(&mut socket, &settings, 91, 92);
            assert_eq!(runs(&peer_log), BTreeMap::from([(91, 2)]));
            assert!(!peer_reviewer.socket(91).exists());
            fs::write(marker, b"available").unwrap();
            match mode {
                "source" => {
                    fs::remove_file(source).unwrap();
                    let command = convene(settings.scope, &plan);
                    outcome(
                        &exchange(&mut socket, &command, settings.response_bytes),
                        "pending",
                    );
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(10));
                        let reply = exchange(&mut socket, &command, settings.response_bytes);
                        if field(value(&reply, "convene").unwrap(), &["outcome"]).as_str()
                            == Some("failed")
                        {
                            outcome(&reply, "failed");
                            assert_eq!(
                                exchange(&mut socket, &command, settings.response_bytes),
                                reply
                            );
                            break;
                        }
                        pause(1);
                    }
                }
                "revoked" => {
                    // A malformed original operator connection retires its
                    // custodian. The held loop cannot replace or renew it.
                    socket.write_all(b"{").unwrap();
                    socket.shutdown(std::net::Shutdown::Write).unwrap();
                    let mut byte = [0; 1];
                    assert!(matches!(socket.read(&mut byte), Ok(0) | Err(_)));
                    let started = Instant::now();
                    while settings.socket.exists() {
                        assert!(started.elapsed() < Duration::from_secs(10));
                        pause(1);
                    }
                }
                "stop" => {
                    // The original separately authenticated Stop endpoint is
                    // still serviced while the completed generated release waits.
                    let receipt = stopping(peer_reviewer).join().unwrap();
                    assert!(receipt.acknowledged() && receipt.drained());
                }
                _ => unreachable!(),
            }
            assert_eq!(runs(&peer_log), BTreeMap::from([(91, 2)]));
        });
        let mut output = Output::new(std::slice::from_ref(&entry));
        let result = crate::workflow::actor_service::native::serve_lifecycle_selected(
            config,
            &entry.actor,
            &reviewer,
            entry.inputs,
            (false, None, None, finish::Intent::Existing),
            Some(&selection),
            || ElapsedTick(1000),
            &mut output,
        );
        observer.join().unwrap();
        assert_eq!(result.is_err(), mode == "source", "{mode}: {result:?}");
        let replies = output.join();
        assert_eq!(replies.len(), 1);
        assert!(!executed(&replies[0]));
        let after = reader.progress(7);
        assert_eq!(after.text.bytes().unwrap(), b"A");
        assert_eq!(after.text.generation_revision(), 4);
        assert_eq!(after.numerical.numerical.position, 4);
        assert_eq!(after.publication.executions, 0);
        assert_eq!(after.publication.control.ledger.charged, 0);
        assert_eq!(after.publication.control.ledger.reserved, 0);
        assert_eq!(after.publication.stop.is_some(), mode != "revoked");
        assert_eq!(runs(&child_log), BTreeMap::from([(91, 2)]));
        assert!(!reviewer.socket(entry.actor.request).exists());
        assert!(!selection.settings.socket.exists());
    }
}
