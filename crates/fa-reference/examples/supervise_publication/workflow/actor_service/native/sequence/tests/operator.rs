//! One actual native owner and one separately authenticated finite operator.
//! Synthetic model/helper fixtures test custody and workflow, not model quality.
use super::*;
use crate::workflow::actor_service::operator::Settings;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, MAX_FENCE_REPLAY_EVENTS,
};
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::{io::Read, os::unix::fs::MetadataExt, sync::mpsc};

#[path = "operator/early.rs"]
mod early_tests;

#[path = "operator/combined_precheck.rs"]
mod combined_precheck_tests;

pub(super) fn selection(
    root: &Root,
    actor: &Profile,
    queries: usize,
    fence: bool,
) -> operator::Selection {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let fence = if fence {
        format!(
            r#", "fence":{{"max_replay_events":{MAX_FENCE_REPLAY_EVENTS},"numerical_replay_events":{MAX_FENCE_REPLAY_EVENTS},"authority":"stop_original_admission"}}"#
        )
    } else {
        String::new()
    };
    let settings = Settings::decode(
        format!(
            r#"{{"schema":"fa.tower-operator-service/1",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "socket":"{}/operator.sock",
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "operator":{{"uid":{},"gid":{},"pid":{}}},
        "candidate_limit":8,"connection_limit":8,"exchange_limit":32,
        "runtime_ms":15000,"poll_ms":1,"inspection_queries":{queries},
        "explanation_queries":0,"explanation_replay_events":0,
        "explanation_disclosure":"none"{fence}}}"#,
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            root.0.display(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
        )
        .as_bytes(),
    )
    .unwrap();
    operator::Selection::new(settings, 41).unwrap()
}

pub(super) fn request(actor: &Profile, id: &str, verb: Verb) -> TowerRequest {
    TowerRequest::new(
        id,
        Address::new(
            actor.scope.tenant.to_string(),
            Noun::Run,
            actor.scope.run.to_string(),
            Some(41),
        )
        .unwrap(),
        verb,
    )
}

pub(super) fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| &value.as_object().unwrap()[*key])
}

fn number(value: &Json, path: &[&str]) -> u64 {
    field(value, path).as_str().unwrap().parse().unwrap()
}

pub(super) fn summary(value: &Json, name: &str) -> u64 {
    number(
        value,
        &["result", "value", "knowledge", "value", "summary", name],
    )
}

pub(super) fn known(value: &Json) {
    assert_eq!(
        field(value, &["result", "kind"]).as_str(),
        Some("situation")
    );
    assert_eq!(
        field(value, &["result", "value", "knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(
        field(value, &["result", "value", "knowledge", "basis", "origin"]).as_str(),
        Some("acknowledged_owner")
    );
    assert_eq!(
        field(value, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
}

pub(super) fn connect(settings: &Settings) -> UnixStream {
    let started = Instant::now();
    while !settings.socket.exists() {
        assert!(started.elapsed() < Duration::from_secs(20));
        pause(1);
    }
    let socket = UnixStream::connect(&settings.socket).unwrap();
    settings.supervisor.verify(&socket).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(20)))
        .unwrap();
    socket
}

pub(super) fn exchange(socket: &mut UnixStream, request: &TowerRequest, limit: usize) -> Json {
    let mut bytes = encode_request(request).unwrap().into_bytes();
    bytes.push(b'\n');
    socket.write_all(&bytes).unwrap();
    let mut response = Vec::new();
    loop {
        let mut byte = [0; 1];
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        assert!(response.len() <= limit + 1);
        if byte[0] == b'\n' {
            break;
        }
    }
    let text = std::str::from_utf8(&response).unwrap();
    for private in [
        "review this fixture",
        "\"payload\"",
        "\"logits\"",
        "\"token_ids\"",
    ] {
        assert!(!text.contains(private), "operator leaked {private}: {text}");
    }
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}

fn fence(actor: &Profile, situation: &Json) -> TowerRequest {
    let mut command = request(actor, "resident-native-fence", Verb::Fence);
    command.idempotency_key = Some("1201".into());
    command.arguments = ReadArguments::Fence {
        operation: 1201,
        expected: FileFenceBasis {
            scope: actor.scope,
            journal_revision: number(
                situation,
                &["result", "value", "knowledge", "basis", "journal_revision"],
            ),
            control_sequence: number(
                situation,
                &["result", "value", "knowledge", "basis", "control_sequence"],
            ),
            authority_epoch: summary(situation, "policy_epoch"),
        },
    };
    command
}

/// An independent human withholds each key until the operator has inspected
/// that live boundary. No native permission is supplied by a fixture shortcut.
pub(super) fn held_reviews(
    reviewer: PeerProfile,
    requests: Vec<u64>,
    offered: mpsc::Sender<(u64, ReviewPacket)>,
    decisions: mpsc::Receiver<Option<ReviewDecision>>,
) -> JoinHandle<Vec<ReviewPacket>> {
    thread::spawn(move || {
        let mut packets = Vec::new();
        for request in requests {
            let started = Instant::now();
            while !reviewer.socket(request).exists() {
                assert!(started.elapsed() < Duration::from_secs(20));
                pause(1);
            }
            let mut client = reviewer
                .connect_client_waiting(request, Duration::from_secs(20))
                .unwrap();
            loop {
                assert!(started.elapsed() < Duration::from_secs(25));
                match client.step().unwrap() {
                    ReviewClientProgress::NeedsDecision => {
                        let packet = client.packet().unwrap().clone();
                        offered.send((request, packet.clone())).unwrap();
                        packets.push(packet);
                        match decisions.recv_timeout(Duration::from_secs(20)).unwrap() {
                            Some(decision) => client.respond(decision).unwrap(),
                            None => return packets,
                        }
                    }
                    ReviewClientProgress::Complete => break,
                    _ => pause(1),
                }
            }
        }
        packets
    })
}

#[test]
fn resident_native_operator_keeps_one_spent_grant_and_session_across_real_releases() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, true);
    let reader = Reader::new(&config, &entries[0]);
    let chosen = selection(&root, &entries[0].actor, 1, false);
    let settings = chosen.settings.clone();
    let actor = entries[0].actor.clone();
    let store = reader.store.clone();
    let profile = reader.profile.clone();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(reviewer.clone(), REQUESTS.to_vec(), offered, decisions);
    let observer = thread::spawn(move || {
        let mut socket = connect(&settings);
        let inode = std::fs::metadata(&settings.socket).unwrap().ino();
        for (index, selected_request) in REQUESTS.into_iter().enumerate() {
            let (request_id, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(request_id, selected_request);
            assert_eq!(std::fs::metadata(&settings.socket).unwrap().ino(), inode);
            assert_eq!(
                FileOversight::read_publication(&store, &profile)
                    .unwrap()
                    .executions,
                index as u64,
                "operator observation is not the missing human key"
            );
            let response = exchange(
                &mut socket,
                &request(&actor, &format!("entry-{index}"), Verb::Situation),
                settings.response_bytes,
            );
            if index == 0 {
                known(&response);
                assert_eq!(summary(&response, "executions"), 0);
                assert_eq!(summary(&response, "pending_attempts"), 1);
                // An operator request cannot turn the branded native owner
                // into an ordinary supplied-payload publication endpoint.
                let mut proposal = request(&actor, "operator-is-not-actor", Verb::Propose);
                proposal.idempotency_key = Some("1000".into());
                proposal.arguments = ReadArguments::Propose {
                    request: 1000,
                    proposal: ActorProposal {
                        target: packet.action().spec().target.unwrap(),
                        payload: b"operator replacement".to_vec(),
                        units: 20,
                        deadline: packet.action().spec().deadline,
                        expected_policy_epoch: packet.action().spec().policy_epoch,
                    },
                };
                let refused = exchange(&mut socket, &proposal, settings.response_bytes);
                assert_eq!(
                    field(&refused, &["envelope", "reason_code"]).as_str(),
                    Some("CapabilityUnsupported")
                );
            } else {
                assert_eq!(
                    field(&response, &["envelope", "reason_code"]).as_str(),
                    Some("BudgetExhausted"),
                    "request handoff must not issue another inspection grant"
                );
            }
            if index == 1 {
                // A healthy reconnect retains the same native allowance.
                drop(socket);
                socket = connect(&settings);
            }
            release.send(Some(ReviewDecision::Approve)).unwrap();
        }
    });
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let result = run_with_operator(
        config,
        entries,
        &reviewer,
        None,
        (None, Some(&chosen)),
        || ElapsedTick(1000),
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    observer.join().unwrap();
    let packets = human.join().unwrap();
    assert_eq!(packets.len(), 3);
    let packets = std::array::from_fn(|index| Some(packets[index].clone()));
    complete(&reader, &output.join(), &packets);
    assert!(!chosen.settings.socket.exists());
    closed(&actor_profiles, &reviewer);
}

#[test]
fn resident_native_operator_fault_is_not_fence_and_later_fence_preserves_the_prefix() {
    for stop in [false, true] {
        let root = Root::new();
        let (config, mut entries, reviewer) = selected(&root, false);
        for entry in &mut entries {
            entry.actor.reply_ms = 500;
        }
        let reader = Reader::new(&config, &entries[0]);
        let chosen = selection(&root, &entries[0].actor, 2, stop);
        let settings = chosen.settings.clone();
        let actor = entries[0].actor.clone();
        let store = reader.store.clone();
        let profile = reader.profile.clone();
        let (offered, offers) = mpsc::channel();
        let (release, decisions) = mpsc::channel();
        let human = held_reviews(reviewer.clone(), REQUESTS.to_vec(), offered, decisions);
        let observer = thread::spawn(move || {
            let mut socket = connect(&settings);
            for (index, selected_request) in REQUESTS.into_iter().enumerate() {
                let (request_id, _) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
                assert_eq!(request_id, selected_request);
                if index == 0 || (stop && index == 1) {
                    let situation = exchange(
                        &mut socket,
                        &request(&actor, &format!("original-{index}"), Verb::Situation),
                        settings.response_bytes,
                    );
                    known(&situation);
                    assert_eq!(summary(&situation, "executions"), index as u64);
                    if stop && index == 1 {
                        let command = fence(&actor, &situation);
                        let acknowledged = exchange(&mut socket, &command, settings.response_bytes);
                        assert_eq!(
                            field(&acknowledged, &["result", "kind"]).as_str(),
                            Some("fence")
                        );
                        assert_eq!(
                            field(&acknowledged, &["result", "value", "knowledge", "state"])
                                .as_str(),
                            Some("known")
                        );
                        // One frame is processed per checkpoint. A second
                        // complete round trip must pass the original
                        // LiveControl Stop/drain checkpoint before it can be
                        // serviced; the first Fence reply alone is not drain
                        // evidence and can race that initial cleanup.
                        assert_eq!(
                            exchange(&mut socket, &command, settings.response_bytes),
                            acknowledged
                        );
                        let settled = FileOversight::read_publication(&store, &profile).unwrap();
                        drop(socket);
                        socket = connect(&settings);
                        assert_eq!(
                            exchange(&mut socket, &command, settings.response_bytes),
                            acknowledged
                        );
                        assert_eq!(
                            FileOversight::read_publication(&store, &profile).unwrap(),
                            settled
                        );
                        release.send(None).unwrap();
                        return;
                    }
                    if !stop {
                        socket.write_all(b"{").unwrap();
                        socket.shutdown(std::net::Shutdown::Write).unwrap();
                        let mut byte = [0; 1];
                        assert!(matches!(socket.read(&mut byte), Ok(0) | Err(_)));
                        let retired = Instant::now();
                        while settings.socket.exists() {
                            assert!(retired.elapsed() < Duration::from_secs(20));
                            pause(1);
                        }
                    }
                }
                release.send(Some(ReviewDecision::Approve)).unwrap();
            }
        });
        let mut output = Output::new(&entries);
        let actor_profiles = output.profiles.clone();
        let result = run_with_operator(
            config,
            entries,
            &reviewer,
            None,
            (None, Some(&chosen)),
            || ElapsedTick(1000),
            &mut output,
        );
        observer.join().unwrap();
        let packets = human.join().unwrap();
        let replies = output.join();
        let after = reader.publication();
        if stop {
            assert!(
                result
                    .unwrap_err()
                    .contains("1 of 3 confirmed Executed releases")
            );
            assert_eq!(packets.len(), 2);
            assert_eq!(replies.len(), 2);
            assert!(executed(&replies[0]));
            assert!(!executed(&replies[1]));
            assert_eq!(after.executions, 1);
            assert_eq!(after.payload, b"A");
            assert_eq!(
                after.control.ledger.charged,
                packets[0].action().spec().units
            );
            assert_eq!(after.control.ledger.reserved, 0);
            assert!(after.stop.is_some());
            assert_eq!(reader.progress(8).numerical.numerical.position, 8);
            assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
            reader.no_generation(9);
        } else {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(packets.len(), 3);
            assert!(replies.iter().all(executed));
            assert_eq!(after.executions, 3);
            assert_eq!(after.payload, b"AAA");
            assert!(after.stop.is_none());
        }
        assert!(!chosen.settings.socket.exists());
        closed(&actor_profiles, &reviewer);
    }
}

#[test]
fn resident_native_actor_transport_failure_preserves_independent_operator_reply_grace() {
    use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;

    let root = Root::new();
    let (config, mut entries, reviewer_profile) = selected(&root, false);
    let entry = entries.remove(0);
    let mut actor = entry.actor;
    actor.reply_ms = 1000;
    actor.candidates = 1;
    // This actor channel's one actually offered peer fails kernel admission.
    // The separately selected operator keeps its original correct identity.
    actor.actor = PeerPolicy::new(
        actor.actor.uid(),
        actor.actor.gid(),
        Some(actor.actor.pid().unwrap().checked_add(1).unwrap()),
    )
    .unwrap();
    let chosen = selection(&root, &actor, 1, false);
    chosen.check(&config, &actor, &reviewer_profile).unwrap();
    let bootstrap = bootstrap::selection(&config, &entry.inputs, None).unwrap();
    let (host, reviewer) = bootstrap::prepare(&config, &entry.inputs, bootstrap, false).unwrap();
    let (port, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let before = driver.supervisor().host().unwrap().inspect();
    let source = config.source.status();
    let deadline = Deadline {
        logical: ElapsedTick(16000),
        started: Instant::now(),
        wall: Duration::from_secs(15),
    };
    let service = chosen
        .start(&actor, port.clone(), &driver, &deadline)
        .unwrap();
    let mut control = Control::new(&config, actor.request, None).unwrap();
    control.install_operator(Some(service));
    let socket = BoundSocket::bind(&actor.socket, None).unwrap();
    actor.secure_socket().unwrap();
    let inbox = FileActorInbox::new(
        actor.actor,
        ActorWire::new(port),
        actor.channels(),
        actor.connections,
    )
    .unwrap();
    let pool = FileActorPool::for_requests(vec![(actor.request, inbox)]).unwrap();
    let mut intake = Intake {
        socket,
        pool,
        profile: actor.clone(),
        attempted: 0,
    };
    let settings = chosen.settings.clone();
    let observer = thread::spawn(move || {
        let mut rejected = UnixStream::connect(&actor.socket).unwrap();
        rejected
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let mut byte = [0; 1];
        assert_eq!(rejected.read(&mut byte).unwrap(), 0);
        // Request disclosure only after observing the actual actor rejection.
        let mut operator = connect(&settings);
        let response = exchange(
            &mut operator,
            &request(&actor, "after-actor-rejection", Verb::Situation),
            settings.response_bytes,
        );
        known(&response);
        assert_eq!(summary(&response, "attempts"), 0);
        assert_eq!(summary(&response, "executions"), 0);
    });
    let error = reply_grace(
        &mut driver,
        &mut intake,
        (&mut control, &reviewer, &deadline),
        true,
        &mut || ElapsedTick(1000),
    )
    .unwrap_err();
    assert!(error.contains("CredentialsRejected"), "{error}");
    observer.join().unwrap();
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(config.source.status(), source);
    assert!(!control.native_cleanup_started());
    control.take_operator().unwrap().retire();
    intake.pool.revoke_all();
    drop(intake);
    assert_eq!(
        cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms),
        0
    );
}
