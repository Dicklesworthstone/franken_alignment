//! One independently cancelled effect must not become a domain Stop or renew
//! the surviving schedule's source, actor, operator or cancellation custody.
use super::*;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileHoldDiagnosisBasis, MAX_CANCELLATION_REPLAY_EVENTS,
};
use fa_reference::address_resolver::Address;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};

fn cancellation_next(scope: Scope, request: u64, operation: u64) -> TowerRequest {
    let address = Address::parse(&format!(
        "fa://{}/attempt/request-{}-{}-{}-{}-{request}@{GENERATION}",
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority,
    ))
    .unwrap();
    let mut query = TowerRequest::new(format!("cancel-plan-{operation}"), address, Verb::Next);
    query.arguments = ReadArguments::CancellationNext { operation };
    query
}

fn numeric(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

/// Follow the exact explicit native action, retaining every predecessor field
/// and checking the complete document before sending it through the socket.
fn cancellation_document(scope: Scope, response: &Json) -> String {
    let value = field(known(response, "cancellation-next"), "value");
    assert_eq!(field(value, "stage").as_str(), Some("authorized"));
    assert_eq!(field(value, "can_cancel").as_bool(), Some(true));
    let action = field(value, "action");
    let arguments = field(action, "arguments");
    assert_eq!(
        field(arguments, "action").as_str(),
        Some("cancel_undispatched")
    );
    let expected = field(arguments, "expected");
    let operation = numeric(field(arguments, "operation"));
    let mut request = TowerRequest::new(
        field(action, "operation_id").as_str().unwrap().to_owned(),
        Address::parse(field(action, "address").as_str().unwrap()).unwrap(),
        Verb::Resolve,
    );
    request.idempotency_key = Some(
        field(action, "idempotency_key")
            .as_str()
            .unwrap()
            .to_owned(),
    );
    request.arguments = ReadArguments::CancelUndispatched {
        operation,
        expected: FileHoldDiagnosisBasis {
            scope,
            request: numeric(field(expected, "request")),
            request_generation: numeric(field(expected, "request_generation")),
            attempt: numeric(field(expected, "attempt")),
            journal_revision: numeric(field(expected, "journal_revision")),
            control_sequence: numeric(field(expected, "control_sequence")),
            recorded_at: expected
                .get("recorded_at")
                .filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(numeric(tick))),
        },
    };
    let document = encode_request(&request).unwrap();
    assert_eq!(&parse(document.as_bytes()), action);
    document
}

#[test]
fn scoped_operator_cancellation_reaps_first_request_and_publishes_the_next_without_a_fence() {
    for checked in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.connections = 1;
        actor.candidates = 1;
        let scope = config.profile.delivery.scope;
        operator_profile(&root, &config, 3, 0);
        let text = fs::read_to_string(root.path("operator.json")).unwrap();
        let selected_cancellation = format!(
            r#"{{"cancellation":{{"queries":2,"operations":1,"max_replay_events":{MAX_CANCELLATION_REPLAY_EVENTS},"authority":"cancel_undispatched_requests"}},"#
        );
        let text = text.replacen('{', &selected_cancellation, 1);
        let mut operator = Settings::decode(text.as_bytes()).unwrap();
        operator.connection_limit = 2;
        let archive = root.path("cancelled-sequence-source.bin");
        // One initial cut and one final cut for the only published request.
        let selected = selected_checks(&root, &archive, 2);
        let publication = checked.then(|| publication(&root));
        let second_action = checked.then(|| {
            write_pair(&root, &action_for(&root, &config, 1, 1, b"first"));
            action_for(&root, &config, 2, 1, b"second")
        });
        let first = document(&config, 7, 1, "6669727374");
        // Cancelling request 7 does not advance the external target version.
        let second = document(&config, 8, 1, "7365636f6e64");
        let first_poll = poll(&config, 7);
        let second_poll = poll(&config, 8);
        let situation = situation_document(&config);
        let first_human_socket = reviewer.socket(7);
        let second_control = crate::workflow::control::socket_path(&reviewer, 8);
        let actor_peer = actor.clone();
        let operator_peer = operator.clone();
        let root_path = root.0.clone();
        let (offered, offered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();

        // Only the second request receives a human decision. Keep its original
        // offer pending while inspecting the retained operator grant after handoff.
        let human_profile = reviewer.clone();
        let human = std::thread::spawn(move || {
            let mut client = human_profile
                .connect_client_waiting(8, Duration::from_secs(10))
                .unwrap();
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(10));
                match client.step().unwrap() {
                    ReviewClientProgress::NeedsDecision => {
                        offered.send(()).unwrap();
                        release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                        client.respond(ReviewDecision::Approve).unwrap();
                    }
                    ReviewClientProgress::Complete => return,
                    _ => pause(1),
                }
            }
        });
        let peer = std::thread::spawn(move || {
            let mut operator = connect_operator(&operator_peer);
            counts(&exchange(&mut operator, &situation), &[("attempts", "0")]);
            let mut actor = connect_actor(&actor_peer);
            let submitted = exchange(&mut actor, &first);
            assert_eq!(
                field(request_knowledge(&submitted), "state").as_str(),
                Some("pending")
            );
            // This endpoint is created only after automatic authorization.
            // No reviewer connects, so the exact predecessor stays unchanged.
            wait(&first_human_socket);
            let plan = exchange(
                &mut operator,
                &encode_request(&cancellation_next(scope, 7, 901)).unwrap(),
            );
            let command = cancellation_document(scope, &plan);
            let cancelled = exchange(&mut operator, &command);
            let result = field(known(&cancelled, "cancellation"), "value");
            assert_eq!(field(result, "before_stage").as_str(), Some("authorized"));
            assert_eq!(field(result, "stage").as_str(), Some("cancelled"));
            assert_eq!(field(result, "refunded_units").as_str(), Some("7"));
            let observed = terminal(&mut actor, &first_poll);
            assert_eq!(
                field(request_knowledge(&observed), "value").as_str(),
                Some("cancelled")
            );
            wait(&second_control);
            counts(
                &exchange(&mut operator, &situation),
                &[
                    ("attempts", "1"),
                    ("executions", "0"),
                    ("charged_units", "0"),
                    ("reserved_units", "0"),
                ],
            );
            assert!(!first_human_socket.exists());
            drop(operator);
            let mut operator = connect_operator(&operator_peer);
            // Reconnection and request handoff retain the original receipt.
            assert_eq!(exchange(&mut operator, &command), cancelled);
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
                fs::write(root_path.join("current.bin"), bytes).unwrap();
            }
            let submitted = exchange(&mut actor, &second);
            assert_eq!(
                field(request_knowledge(&submitted), "state").as_str(),
                Some("pending")
            );
            offered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            let blocked = exchange(
                &mut operator,
                &encode_request(&cancellation_next(scope, 8, 902)).unwrap(),
            );
            let plan = field(known(&blocked, "cancellation-next"), "value");
            assert_eq!(field(plan, "stage").as_str(), Some("authorized"));
            assert_eq!(field(plan, "can_cancel").as_bool(), Some(false));
            assert_eq!(
                field(plan, "block_reason").as_str(),
                Some("operation_limit")
            );
            assert!(field(plan, "action").is_null());
            // A historical retry cannot cancel the currently active second job.
            assert_eq!(exchange(&mut operator, &command), cancelled);
            release.send(()).unwrap();
            assert_executed(&terminal(&mut actor, &second_poll));
            counts(
                &exchange(&mut operator, &situation),
                &[
                    ("attempts", "2"),
                    ("executions", "1"),
                    ("charged_units", "7"),
                    ("reserved_units", "0"),
                ],
            );
            let exhausted = exchange(
                &mut operator,
                &encode_request(&cancellation_next(scope, 8, 903)).unwrap(),
            );
            assert_eq!(
                field(field(&exhausted, "envelope"), "reason_code").as_str(),
                Some("BudgetExhausted")
            );
        });
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
        human.join().unwrap();
        assert!(result.is_ok(), "checked={checked}: {result:?}");
        let config = configuration(&root);
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(
            (state.executions, state.payload.as_slice()),
            (1, b"second".as_slice())
        );
        assert_eq!(state.target.expected_version, 2);
        assert_eq!(state.control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(state.control.ledger.stages[&2], ActionState::Confirmed);
        assert_eq!(
            (state.control.ledger.charged, state.control.ledger.reserved),
            (7, 0)
        );
        assert_eq!(
            state.control.ledger.available,
            config.profile.delivery.total - 7
        );
        assert!(state.stop.is_none());
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
        assert!(!actor.socket.exists());
        assert!(!operator.socket.exists());
        assert!(!reviewer.socket(8).exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 8).exists());
    }
}
