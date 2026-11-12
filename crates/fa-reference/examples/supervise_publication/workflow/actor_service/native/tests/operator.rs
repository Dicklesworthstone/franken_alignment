//! Authenticated supervision of actual native inference and publication.
//! Deterministic model weights and helper fixtures establish protocol behavior.
use super::super::operator as native_operator;
use super::*;
use crate::workflow::actor_service::operator::{Service, Settings};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, MAX_FENCE_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::io::Read;

const REGISTRATION: u64 = 41;
const STOP_OPERATION: u64 = 1201;

fn settings(root: &Root, actor: &Profile, queries: usize, fence: bool) -> Settings {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let selected_fence = if fence {
        format!(
            r#", "fence":{{"max_replay_events":{MAX_FENCE_REPLAY_EVENTS},"numerical_replay_events":{MAX_FENCE_REPLAY_EVENTS},"authority":"stop_original_admission"}}"#
        )
    } else {
        String::new()
    };
    Settings::decode(
        format!(
            r#"{{"schema":"fa.tower-operator-service/1",
      "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
      "socket":"{}/operator.sock",
      "supervisor":{{"uid":{},"gid":{},"pid":{}}},
      "operator":{{"uid":{},"gid":{},"pid":{}}},
      "candidate_limit":8,"connection_limit":8,"exchange_limit":32,
      "runtime_ms":15000,"poll_ms":1,"inspection_queries":{queries},
      "explanation_queries":0,"explanation_replay_events":0,
      "explanation_disclosure":"none"{selected_fence}}}"#,
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
    .unwrap()
}

fn selected(
    root: &Root,
    actor: &Profile,
    queries: usize,
    fence: bool,
) -> native_operator::Selection {
    native_operator::Selection::new(settings(root, actor, queries, fence), REGISTRATION).unwrap()
}

fn mount(actor: &Profile) -> Address {
    Address::new(
        actor.scope.tenant.to_string(),
        Noun::Run,
        actor.scope.run.to_string(),
        Some(REGISTRATION),
    )
    .unwrap()
}
fn request(actor: &Profile, id: &str, verb: Verb) -> TowerRequest {
    TowerRequest::new(id, mount(actor), verb)
}
fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| &value.as_object().unwrap()[*key])
}
fn number(value: &Json, path: &[&str]) -> u64 {
    field(value, path).as_str().unwrap().parse().unwrap()
}
fn summary(value: &Json, name: &str) -> u64 {
    number(
        value,
        &["result", "value", "knowledge", "value", "summary", name],
    )
}
fn known(value: &Json) {
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
fn connect(settings: &Settings) -> UnixStream {
    let started = Instant::now();
    while !settings.socket.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "operator socket never appeared"
        );
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
fn exchange(socket: &mut UnixStream, request: &TowerRequest, limit: usize) -> Json {
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
fn fence(actor: &Profile, basis: FileFenceBasis) -> TowerRequest {
    let mut request = request(actor, "native-fence", Verb::Fence);
    request.idempotency_key = Some(STOP_OPERATION.to_string());
    request.arguments = ReadArguments::Fence {
        operation: STOP_OPERATION,
        expected: basis,
    };
    request
}
fn basis(actor: &Profile, situation: &Json) -> FileFenceBasis {
    FileFenceBasis {
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
    }
}

fn held_reviewer(
    reviewer: PeerProfile,
    offered: mpsc::Sender<ReviewPacket>,
    release: mpsc::Receiver<Option<ReviewDecision>>,
) -> std::thread::JoinHandle<ReviewPacket> {
    std::thread::spawn(move || {
        let started = Instant::now();
        while !reviewer.socket(91).exists() {
            assert!(started.elapsed() < Duration::from_secs(20));
            pause(1);
        }
        let mut client = reviewer
            .connect_client_waiting(91, Duration::from_secs(20))
            .unwrap();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(25));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    let current = client.packet().unwrap().clone();
                    offered.send(current.clone()).unwrap();
                    packet = Some(current);
                    match release.recv_timeout(Duration::from_secs(20)).unwrap() {
                        Some(decision) => client.respond(decision).unwrap(),
                        None => return packet.unwrap(),
                    }
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => pause(1),
            }
        }
    })
}

#[test]
fn native_operator_observes_real_review_and_can_explicitly_fence_before_publication() {
    for stop in [false, true] {
        let root = Root::new();
        let config = configured(&root);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 1000;
        let native = inputs(&root, &config);
        let selection = selected(&root, &actor, 2, stop);
        let operator_settings = selection.settings.clone();
        let operator_actor = actor.clone();
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let decoder = native.decoder.clone();
        let tokenizer = native.tokenizer.clone();
        let (source_sender, source_receiver) = mpsc::channel();
        let (actor_sender, actor_receiver) = mpsc::channel();
        let (offered, offer_receiver) = mpsc::channel();
        let (release, release_receiver) = mpsc::channel();
        let client = actor_thread(actor.clone(), actor_receiver);
        let human = held_reviewer(reviewer.clone(), offered, release_receiver);
        let operator = std::thread::spawn(move || {
            let document = source_receiver
                .recv_timeout(Duration::from_secs(20))
                .unwrap();
            let mut socket = connect(&operator_settings);
            let initial = exchange(
                &mut socket,
                &request(&operator_actor, "before-actor", Verb::Situation),
                operator_settings.response_bytes,
            );
            known(&initial);
            assert_eq!(summary(&initial, "attempts"), 0);
            assert_eq!(summary(&initial, "executions"), 0);
            // Closing an independently authenticated observer is no Stop.
            drop(socket);
            let mut socket = connect(&operator_settings);
            actor_sender.send(document).unwrap();
            let packet = offer_receiver
                .recv_timeout(Duration::from_secs(20))
                .unwrap();
            let pending = exchange(
                &mut socket,
                &request(&operator_actor, "during-human", Verb::Situation),
                operator_settings.response_bytes,
            );
            known(&pending);
            assert_eq!(summary(&pending, "attempts"), 1);
            assert_eq!(summary(&pending, "pending_attempts"), 1);
            assert_eq!(summary(&pending, "executions"), 0);
            assert_eq!(
                ReleaseFrame::decode(&packet.action().spec().payload)
                    .unwrap()
                    .message(),
                Some("A")
            );
            if stop {
                let command = fence(&operator_actor, basis(&operator_actor, &pending));
                let acknowledged =
                    exchange(&mut socket, &command, operator_settings.response_bytes);
                assert_eq!(
                    field(&acknowledged, &["result", "kind"]).as_str(),
                    Some("fence")
                );
                assert_eq!(
                    field(&acknowledged, &["result", "value", "knowledge", "state"]).as_str(),
                    Some("known")
                );
                drop(socket);
                let mut socket = connect(&operator_settings);
                assert_eq!(
                    exchange(&mut socket, &command, operator_settings.response_bytes),
                    acknowledged
                );
                release.send(None).unwrap();
            } else {
                let exhausted = exchange(
                    &mut socket,
                    &request(&operator_actor, "same-spent-grant", Verb::Situation),
                    operator_settings.response_bytes,
                );
                assert_eq!(
                    field(&exhausted, &["envelope", "reason_code"]).as_str(),
                    Some("BudgetExhausted")
                );
                let mut ordinary = request(&operator_actor, "no-operator-submit", Verb::Propose);
                ordinary.idempotency_key = Some("92".to_owned());
                ordinary.arguments = ReadArguments::Propose {
                    request: 92,
                    proposal: fa_reference::action::consequence::oversight::actor::ActorProposal {
                        target: packet.action().spec().target.unwrap(),
                        payload: b"operator supplied replacement".to_vec(),
                        units: 29,
                        deadline: packet.action().spec().deadline,
                        expected_policy_epoch: packet.action().spec().policy_epoch,
                    },
                };
                let refused = exchange(&mut socket, &ordinary, operator_settings.response_bytes);
                assert_eq!(
                    field(&refused, &["envelope", "reason_code"]).as_str(),
                    Some("CapabilityUnsupported")
                );
                release.send(Some(ReviewDecision::Approve)).unwrap();
            }
        });
        let mut output = ReferenceOutput {
            bytes: Vec::new(),
            sender: Some(source_sender),
        };
        let result = serve_lifecycle_selected(
            config,
            &actor,
            &reviewer,
            native,
            (false, None, None, finish::Intent::Existing),
            Some(&selection),
            || ElapsedTick(1000),
            &mut output,
        );
        assert!(result.is_ok(), "{result:?}");
        operator.join().unwrap();
        human.join().unwrap();
        let response = client.join().unwrap();
        let after =
            FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7)
                .unwrap();
        assert_eq!(after.text.generation_revision(), 4);
        assert_eq!(after.numerical.numerical.position, 4);
        assert_eq!(after.text.bytes().unwrap(), b"A");
        assert_eq!(after.publication.executions, if stop { 0 } else { 1 });
        assert_eq!(after.publication.control.ledger.reserved, 0);
        if stop {
            assert!(after.publication.stop.is_some());
            assert_eq!(after.publication.control.ledger.charged, 0);
            assert!(after.publication.payload.is_empty());
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    ..
                })
            ));
        } else {
            assert!(after.publication.stop.is_none());
            assert_eq!(after.publication.payload, b"A");
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
        }
        assert!(!selection.settings.socket.exists());
    }
}

#[test]
fn native_generation_checkpoint_requires_an_authenticated_explicit_fence() {
    for authorized in [false, true] {
        let root = Root::new();
        let mut config = configured(&root);
        let (actor, reviewer_profile) = profiles(&root, &config);
        let native = inputs(&root, &config);
        let selected_bootstrap = bootstrap::selection(&config, &native, None).unwrap();
        let (host, reviewer) =
            bootstrap::prepare(&config, &native, selected_bootstrap, false).unwrap();
        let (port, mut driver) = host.into_generated_text_supervised_driver().unwrap();
        let limit = Deadline {
            logical: ElapsedTick(16000),
            started: Instant::now(),
            wall: Duration::from_secs(15),
        };
        refresh(&mut driver, &mut config, &mut || ElapsedTick(1000)).unwrap();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let numerical = host.decoder_inspection().unwrap().numerical;
            let command = FileTextGenerationCommand::new(
                native.generation,
                numerical.actor_revision,
                numerical.position,
                native.request.clone(),
            )
            .unwrap();
            host.begin_decoder_text(host.revision(), command).unwrap();
            host.advance_decoder_text(host.revision(), native.generation, 0)
                .unwrap();
        }
        let before = driver.supervisor().host().unwrap().inspect();
        let numerical = driver
            .supervisor()
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap()
            .numerical;
        assert_eq!(numerical.position, 1);
        let source_work = config.source.status();
        let mut chosen = settings(&root, &actor, 0, true);
        if !authorized {
            chosen.operator = PeerPolicy::new(
                chosen.operator.uid(),
                chosen.operator.gid(),
                Some(chosen.operator.pid().unwrap().checked_add(1).unwrap()),
            )
            .unwrap();
        }
        let selection = native_operator::Selection::new(chosen, REGISTRATION).unwrap();
        selection.check(&config, &actor, &reviewer_profile).unwrap();
        let service = selection.start(&actor, port, &driver, &limit).unwrap();
        let mut control = Control::new(&config, actor.request, None).unwrap();
        control.install_operator(Some(service));
        let command = fence(
            &actor,
            FileFenceBasis {
                scope: actor.scope,
                journal_revision: before.revision,
                control_sequence: before.control.sequence,
                authority_epoch: before.control.ledger.epoch,
            },
        );
        // Source failure cannot invalidate stop-only custody or demand inference.
        std::fs::write(root.0.join("evidence.json"), b"unavailable source").unwrap();
        let settings = selection.settings.clone();
        let client = std::thread::spawn(move || {
            // A connected/disconnected peer sends no mutation, even if authorized.
            drop(connect(&settings));
            let mut socket = connect(&settings);
            let mut bytes = encode_request(&command).unwrap().into_bytes();
            bytes.push(b'\n');
            if authorized {
                return Some(exchange(&mut socket, &command, settings.response_bytes));
            }
            let _ = socket.write_all(&bytes);
            let mut byte = [0; 1];
            assert!(matches!(socket.read(&mut byte), Ok(0) | Err(_)));
            None
        });
        let started = Instant::now();
        let mut reply_only: Option<Service> = None;
        let mut stopped = false;
        while !client.is_finished() {
            if let Some(service) = &mut reply_only {
                service.checkpoint();
            } else if control
                .checkpoint(&mut driver, &reviewer, &limit, &mut || ElapsedTick(1000))
                .unwrap()
            {
                stopped = true;
                // Match the real lifecycle's terminal reply grace: the same
                // spent grant drains its receipt after forward work has ended.
                reply_only = control.take_operator();
            }
            assert!(started.elapsed() < Duration::from_secs(20));
            pause(1);
        }
        let response = client.join().unwrap();
        if !stopped {
            stopped = control
                .checkpoint(&mut driver, &reviewer, &limit, &mut || ElapsedTick(1000))
                .unwrap();
        }
        assert_eq!(stopped, authorized);
        let after = driver.supervisor().host().unwrap().inspect();
        assert_eq!(after.stop.is_some(), authorized);
        assert_eq!(after.executions, 0);
        assert_eq!(after.control.ledger.charged, 0);
        assert_eq!(
            driver
                .supervisor()
                .host()
                .unwrap()
                .decoder_inspection()
                .unwrap()
                .numerical,
            numerical
        );
        assert_eq!(
            driver
                .supervisor()
                .host()
                .unwrap()
                .decoder_text_progress(7)
                .unwrap()
                .generation_revision(),
            1
        );
        assert_eq!(config.source.status(), source_work);
        if let Some(response) = response {
            assert_eq!(
                field(&response, &["result", "kind"]).as_str(),
                Some("fence")
            );
            assert_eq!(
                field(&response, &["result", "value", "knowledge", "state"]).as_str(),
                Some("known")
            );
        } else {
            assert_eq!(after, before);
        }
        if let Some(mut service) = reply_only.or_else(|| control.take_operator()) {
            service.retire();
        }
        assert_eq!(
            cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms),
            0
        );
    }
}

#[test]
fn native_operator_preflight_preserves_roles_paths_and_independent_numerical_work() {
    for collision in 0..5 {
        let root = Root::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let native = inputs(&root, &config);
        let mut chosen = settings(&root, &actor, 1, false);
        chosen.socket = match collision {
            0 => actor.socket.clone(),
            1 => reviewer.socket(actor.request),
            2 => crate::workflow::control::socket_path(&reviewer, actor.request),
            3 => config.source.path().to_owned(),
            _ => config.store.join("operator.sock"),
        };
        let selected = native_operator::Selection::new(chosen, REGISTRATION).unwrap();
        let store = config.store.clone();
        let source = std::fs::read(config.source.path()).unwrap();
        let result = serve_lifecycle_selected(
            config,
            &actor,
            &reviewer,
            native,
            (false, None, None, finish::Intent::Existing),
            Some(&selected),
            || panic!("invalid selection reached the clock"),
            &mut Vec::new(),
        );
        assert!(result.is_err());
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert_eq!(std::fs::read(root.0.join("evidence.json")).unwrap(), source);
    }
    let root = Root::new();
    let config = configured(&root);
    let (actor, _) = profiles(&root, &config);
    let mut chosen = settings(&root, &actor, 0, true);
    chosen.fence.as_mut().unwrap().numerical_replay_events = None;
    assert!(native_operator::Selection::new(chosen, REGISTRATION).is_err());
    let args: Vec<String> = [
        "serve-create",
        "config",
        "actor",
        "reviewer",
        "--operator-profile",
        "operator",
        "--native-text",
        "recipe",
        "--tower-generation",
        "41",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let (remaining, selected) = native_operator::take_option(&args).unwrap();
    assert_eq!(
        remaining,
        [
            "serve-create",
            "config",
            "actor",
            "reviewer",
            "--native-text",
            "recipe"
        ]
    );
    assert_eq!(selected.unwrap().1, REGISTRATION);
    for suffix in [
        ["--requests", "91,92"],
        ["--precheck-profile", "precheck"],
        ["--tower-generation", "42"],
        ["--after-message", "90"],
    ] {
        let mut invalid = args.clone();
        invalid.extend(suffix.into_iter().map(str::to_owned));
        assert!(native_operator::take_option(&invalid).is_err());
    }
}

struct BrokenOutput;
impl Write for BrokenOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "fixture reference output failed",
        ))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn recorded_native_operator_observes_receipts_without_source_helpers_or_implicit_stop() {
    for mode in 0..3 {
        let broken_output = mode == 1;
        let mutation_selected = mode == 2;
        let root = Root::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let native = inputs(&root, &config);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let decoder = native.decoder.clone();
        let tokenizer = native.tokenizer.clone();
        let (sender, receiver) = mpsc::channel();
        let client = actor_thread(actor.clone(), receiver);
        let human = reviewing(
            reviewer.clone(),
            store.clone(),
            bootstrap.clone(),
            ReviewDecision::Approve,
        );
        let mut first = ReferenceOutput {
            bytes: Vec::new(),
            sender: Some(sender),
        };
        serve(
            config,
            &actor,
            &reviewer,
            native,
            || ElapsedTick(1000),
            &mut first,
        )
        .unwrap();
        human.join().unwrap();
        assert!(matches!(
            client.join().unwrap().result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        let before =
            FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7)
                .unwrap();
        assert_eq!(before.publication.executions, 1);
        assert!(before.publication.stop.is_none());

        let mut config = configured(&root);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 1000;
        let native = inputs(&root, &config);
        config.programs.clear();
        std::fs::remove_file(config.source.path()).unwrap();
        let selection = selected(&root, &actor, 1, mutation_selected);
        if mutation_selected {
            let error = serve_lifecycle_selected(
                config,
                &actor,
                &reviewer,
                native,
                (true, None, None, finish::Intent::Existing),
                Some(&selection),
                || ElapsedTick(20000),
                &mut Vec::new(),
            )
            .unwrap_err();
            assert!(error.contains(
                "recorded native receipt recovery accepts only observation operator grants"
            ));
            assert!(!selection.settings.socket.exists());
            assert!(!actor.socket.exists());
            assert!(!reviewer.socket(actor.request).exists());
            assert!(!crate::workflow::control::socket_path(&reviewer, actor.request).exists());
            let after = FileOversight::read_decoder_text_progress(
                &store, &bootstrap, &decoder, &tokenizer, 7,
            )
            .unwrap();
            assert_eq!(after.numerical.numerical, before.numerical.numerical);
            assert_eq!(
                after.text.generation_revision(),
                before.text.generation_revision()
            );
            assert_eq!(after.publication.executions, 1);
            assert_eq!(after.publication.payload, b"A");
            assert_eq!(
                after.publication.control.ledger.charged,
                before.publication.control.ledger.charged
            );
            assert!(after.publication.stop.is_none());
            continue;
        }
        let observer_settings = selection.settings.clone();
        let observer_actor = actor.clone();
        let observer_reviewer = reviewer.clone();
        let (source_sender, source_receiver) = mpsc::channel();
        let (actor_sender, actor_receiver) = mpsc::channel();
        let client = (!broken_output).then(|| actor_thread(actor.clone(), actor_receiver));
        let original_document = first.bytes;
        let observer = std::thread::spawn(move || {
            let document = if broken_output {
                None
            } else {
                let document = source_receiver
                    .recv_timeout(Duration::from_secs(20))
                    .unwrap();
                assert_eq!(document, original_document);
                Some(document)
            };
            let mut socket = connect(&observer_settings);
            let historical = exchange(
                &mut socket,
                &request(&observer_actor, "recorded-native", Verb::Situation),
                observer_settings.response_bytes,
            );
            known(&historical);
            assert_eq!(summary(&historical, "executions"), 1);
            assert_eq!(summary(&historical, "terminal_attempts"), 1);
            assert_eq!(
                field(
                    &historical,
                    &[
                        "result",
                        "value",
                        "knowledge",
                        "value",
                        "summary",
                        "stop_recorded"
                    ]
                )
                .as_bool(),
                Some(false)
            );
            assert!(!observer_reviewer.socket(observer_actor.request).exists());
            assert!(
                !crate::workflow::control::socket_path(&observer_reviewer, observer_actor.request)
                    .exists()
            );
            if let Some(document) = document {
                actor_sender.send(document).unwrap();
            }
        });
        let result = if broken_output {
            serve_lifecycle_selected(
                config,
                &actor,
                &reviewer,
                native,
                (true, None, None, finish::Intent::Existing),
                Some(&selection),
                || ElapsedTick(20000),
                &mut BrokenOutput,
            )
        } else {
            let mut output = ReferenceOutput {
                bytes: Vec::new(),
                sender: Some(source_sender),
            };
            serve_lifecycle_selected(
                config,
                &actor,
                &reviewer,
                native,
                (true, None, None, finish::Intent::Existing),
                Some(&selection),
                || ElapsedTick(20000),
                &mut output,
            )
        };
        if broken_output {
            assert!(
                result
                    .unwrap_err()
                    .contains("source reference output failed")
            );
        } else {
            result.unwrap();
            assert!(matches!(
                client.unwrap().join().unwrap().result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
        }
        observer.join().unwrap();
        let after =
            FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7)
                .unwrap();
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(
            after.text.generation_revision(),
            before.text.generation_revision()
        );
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(after.publication.executions, 1);
        assert_eq!(after.publication.payload, b"A");
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert!(after.publication.stop.is_none());
        assert!(!selection.settings.socket.exists());
    }
}

#[test]
fn native_reference_output_failure_stops_once_without_repeating_cleanup_during_operator_grace() {
    let mut outcomes = Vec::new();
    for with_operator in [false, true] {
        let root = Root::new();
        let config = configured(&root);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.reply_ms = 30;
        let native = inputs(&root, &config);
        let selection = selected(&root, &actor, 1, false);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let decoder = native.decoder.clone();
        let tokenizer = native.tokenizer.clone();
        let error = if with_operator {
            serve_lifecycle_selected(
                config,
                &actor,
                &reviewer,
                native,
                (false, None, None, finish::Intent::Existing),
                Some(&selection),
                || ElapsedTick(1000),
                &mut BrokenOutput,
            )
        } else {
            serve(
                config,
                &actor,
                &reviewer,
                native,
                || ElapsedTick(1000),
                &mut BrokenOutput,
            )
        }
        .unwrap_err();
        assert!(error.contains("source reference output failed"));
        let after =
            FileOversight::read_decoder_text_progress(&store, &bootstrap, &decoder, &tokenizer, 7)
                .unwrap();
        assert_eq!(after.text.generation_revision(), 4);
        assert_eq!(after.numerical.numerical.position, 4);
        assert!(after.publication.stop.is_some());
        assert_eq!(after.publication.executions, 0);
        assert_eq!(after.publication.control.ledger.charged, 0);
        assert_eq!(after.publication.control.ledger.reserved, 0);
        assert!(after.publication.payload.is_empty());
        assert!(!selection.settings.socket.exists());
        outcomes.push((
            after.publication,
            std::fs::read(store.join("delivery.bin")).unwrap(),
        ));
    }
    assert_eq!(
        outcomes[0], outcomes[1],
        "operator reply-only grace must preserve the original single Stop/sweep"
    );
}
