//! The CLI validates original cancellation replies before output or sending a
//! later explicitly supplied request. Suggested actions never execute themselves.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileSupervisorCancellationGrant, MAX_CANCELLATION_REPLAY_EVENTS,
};
use fa_reference::tower::publication::PublicationResult;
use fa_reference::typestate_cx::PurposeTag;

struct NativeCancellation {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorCancellationGrant,
    _storage: native::Directory,
}

impl NativeCancellation {
    fn new(dispatched: bool, queries: usize, operations: usize) -> Self {
        let storage = native::Directory::new();
        let (actor, mut supervisor, reviewer, proposal) = native::create(&storage);
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        if dispatched {
            let keys = native::ready(&mut supervisor, &reviewer, 7);
            let mut host = supervisor.host_mut().unwrap();
            native::oversight::dispatch(&mut host, &keys);
            let revision = host.revision();
            host.reconcile(revision, keys.automatic.attempt()).unwrap();
        }
        let (port, grant) = supervisor
            .cancellation_port(queries, operations, MAX_CANCELLATION_REPLAY_EVENTS)
            .unwrap();
        let mut tower =
            PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default())
                .unwrap();
        tower.set_cancellation(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            _storage: storage,
        }
    }

    fn dispatch(
        &mut self,
        request: &TowerRequest,
    ) -> fa_reference::tower::publication::PublicationResponse {
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: native::profile().delivery.scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            vec![PurposeTag::Permit],
            0,
        );
        self.tower
            .exchange(&access, encode_request(request).unwrap().as_bytes())
    }

    fn action(&mut self, operation: u64) -> TowerRequest {
        let response = self.dispatch(&next(operation));
        let PublicationResult::CancellationNext(linked) = response.result else {
            panic!("missing native undispatched cancellation plan");
        };
        linked.action().unwrap().unwrap()
    }
}

fn next(operation: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "operator-cancellation-next",
        Address::new("1", Noun::Attempt, "request-3-2-4-5-7", Some(GENERATION)).unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::CancellationNext { operation };
    request
}

fn settings(root: &Directory) -> PathBuf {
    let path = root.settings(credentials().pid(), 8);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":2", "\"explanation_queries\":0")
        .replace("\"explanation_replay_events\":1024", "\"explanation_replay_events\":0")
        .replace("policy_and_selected_witnesses", "none")
        .replacen(
            '{',
            "{\"cancellation\":{\"queries\":4,\"operations\":2,\"max_replay_events\":4096,\"authority\":\"cancel_undispatched_requests\"},",
            1,
        );
    let decoded = OperatorSettings::decode(selected.as_bytes()).unwrap();
    assert_eq!(
        (decoded.inspection_queries, decoded.explanation_queries),
        (0, 0)
    );
    assert_eq!(decoded.cancellation.unwrap().operations, 2);
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, name| value.get(name).unwrap())
}

#[test]
fn cancellation_replies_bind_action_discriminator_full_predecessor_and_native_refund() {
    let query = next(901);
    let mut native = NativeCancellation::new(false, 2, 1);
    let before = native.supervisor.host().unwrap().inspect();
    let response = native.dispatch(&query);
    let original = response.to_json().unwrap();
    validate_response(original.as_bytes(), &query).unwrap();
    let PublicationResult::CancellationNext(linked) = response.result else {
        panic!("missing plan");
    };
    let command = linked.action().unwrap().unwrap();
    let predecessor = match &command.arguments {
        ReadArguments::CancelUndispatched {
            operation: 901,
            expected,
        } => *expected,
        _ => panic!("native plan was not explicit undispatched cancellation"),
    };
    assert_eq!(native.supervisor.host().unwrap().inspect(), before);
    for damaged in [
        original.replace(
            "fa.publication-cancellation-next/1",
            "fa.publication-cancellation-next/2",
        ),
        original.replace(
            "\"kind\":\"cancellation-next\"",
            "\"kind\":\"resolution-next\"",
        ),
        original.replace(
            "\"cancellation_operation\":\"901\"",
            "\"cancellation_operation\":\"902\"",
        ),
        original.replace(
            "\"action\":\"cancel_undispatched\"",
            "\"action\":\"reconcile\"",
        ),
        original.replace("\"action\":\"cancel_undispatched\",", ""),
        original.replacen("\"verb\":\"resolve\"", "\"verb\":\"hold\"", 1),
        original.replacen(
            "\"idempotency_key\":\"901\"",
            "\"idempotency_key\":\"902\"",
            1,
        ),
        original.replace("\"principal\":\"2\"", "\"principal\":\"22\""),
        original.replace("\"clock_read\":false", "\"clock_read\":true"),
        original.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        original.replace("\"can_cancel\":true", "\"can_cancel\":false"),
        original.replace(
            "single_connected_undispatched_cancellation",
            "complete_ranked_actions",
        ),
        original.replacen(
            "\"cost_estimate\":{\"state\":\"unknown\",\"reason\":\"controller_unavailable\"}",
            "\"cost_estimate\":{\"state\":\"known\",\"value\":\"0\"}",
            1,
        ),
        original.replacen(
            &format!("\"journal_revision\":\"{}\"", predecessor.journal_revision),
            &format!(
                "\"journal_revision\":\"{}\"",
                predecessor.journal_revision + 1
            ),
            1,
        ),
    ] {
        assert_ne!(damaged, original, "fixture mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    let mut legacy = query.clone();
    legacy.arguments = ReadArguments::ResolutionNext { operation: 901 };
    assert!(validate_response(original.as_bytes(), &legacy).is_err());

    let original = native.dispatch(&command).to_json().unwrap();
    let parsed = parse_response(original.as_bytes(), &command).unwrap();
    let settled = native.supervisor.host().unwrap().inspect();
    let refund = before.control.ledger.reserved - settled.control.ledger.reserved;
    assert_eq!(
        field(
            &parsed,
            &["result", "value", "knowledge", "value", "refunded_units"]
        )
        .as_str(),
        Some(refund.to_string().as_str())
    );
    assert_eq!(settled.executions, 0);
    assert_eq!(
        settled.control.ledger.charged,
        before.control.ledger.charged
    );
    assert!(settled.stop.is_none());
    for damaged in [
        original.replace("\"may_write_journal\":true", "\"may_write_journal\":false"),
        original.replace(
            "\"publication_attempted\":false",
            "\"publication_attempted\":true",
        ),
        original.replace("\"seal_attempted\":false", "\"seal_attempted\":true"),
        original.replace(
            "\"cancellation_operation\":\"901\"",
            "\"cancellation_operation\":\"902\"",
        ),
        original.replace("\"kind\":\"cancellation\"", "\"kind\":\"resolution\""),
        original.replace(
            "\"stage\":\"cancelled\"",
            "\"stage\":\"confirmed_not_executed\"",
        ),
        original.replace("\"recorded_at\":\"1\"", "\"recorded_at\":\"2\""),
        original.replace("\"principal\":\"2\"", "\"principal\":\"22\""),
        original.replace(
            "\"before_stage\":\"reviewing\"",
            "\"before_stage\":\"unknown\"",
        ),
        original.replace(
            &format!("\"refunded_units\":\"{refund}\""),
            "\"refunded_units\":\"-1\"",
        ),
        original.replace(
            &format!("\"refunded_units\":\"{refund}\""),
            "\"refunded_units\":0",
        ),
        original.replace(
            &format!("\"refunded_units\":\"{refund}\""),
            "\"refunded_units\":\"00\"",
        ),
        original.replace(
            &format!("\"refunded_units\":\"{refund}\""),
            "\"refunded_units\":\"18446744073709551616\"",
        ),
        original.replace(
            &format!("\"journal_revision\":\"{}\"", settled.revision),
            &format!("\"journal_revision\":\"{}\"", settled.revision + 1),
        ),
    ] {
        assert_ne!(damaged, original, "fixture mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &command).is_err());
    }
    assert_eq!(native.dispatch(&command).to_json().unwrap(), original);
    assert_eq!(native.supervisor.host().unwrap().inspect(), settled);
    assert_eq!(native.grant.remaining_operations(), 0);

    let mut sent = NativeCancellation::new(true, 1, 1);
    let state = sent.supervisor.host().unwrap().inspect();
    let response = sent.dispatch(&query).to_json().unwrap();
    let parsed = parse_response(response.as_bytes(), &query).unwrap();
    assert_eq!(
        field(&parsed, &["result", "value", "knowledge", "value", "stage"]).as_str(),
        Some("unknown")
    );
    assert_eq!(
        field(
            &parsed,
            &["result", "value", "knowledge", "value", "block_reason"]
        )
        .as_str(),
        Some("already_dispatched")
    );
    assert!(
        field(
            &parsed,
            &["result", "value", "knowledge", "value", "action"]
        )
        .is_null()
    );
    assert_eq!(sent.supervisor.host().unwrap().inspect(), state);
    assert_eq!(sent.grant.remaining_operations(), 1);
}

#[test]
fn authenticated_client_sends_only_explicit_cancellation_and_exact_retry() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeCancellation::new(false, 2, 1);
        let command = native.action(910);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().inspect();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let expected = [next(910), command.clone(), command];
        let mut replies = Vec::new();
        for (index, expected) in expected.into_iter().enumerate() {
            let request = decode_request(&read_document(&mut socket)).unwrap();
            assert_eq!(request, expected);
            let response = native.dispatch(&request).to_json().unwrap();
            validate_response(response.as_bytes(), &request).unwrap();
            let current = native.supervisor.host().unwrap().inspect();
            if index == 0 {
                assert_eq!(current, before);
            } else {
                assert_eq!(current.revision, before.revision + 1);
                assert_eq!(current.executions, 0);
                assert_eq!(
                    current.control.ledger.charged,
                    before.control.ledger.charged
                );
                assert!(current.stop.is_none());
            }
            socket.write_all(response.as_bytes()).unwrap();
            socket.write_all(b"\n").unwrap();
            socket.flush().unwrap();
            replies.push(response);
        }
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "unexpected automatic request or retry"
        );
        assert_eq!(native.grant.remaining_queries(), 0);
        assert_eq!(native.grant.remaining_operations(), 0);
        assert_eq!(replies[1], replies[2]);
        replies
    });
    let document = received.recv_timeout(Duration::from_secs(10)).unwrap();
    let command = decode_request(document.as_bytes()).unwrap();
    let documents = [
        root.document("next.json", &next(910)),
        root.document("cancel.json", &command),
        root.document("same-explicit-operation.json", &command),
    ];
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn malformed_cancellation_plan_stops_before_output_or_the_explicit_mutation() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeCancellation::new(false, 2, 1);
        let command = native.action(920);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().inspect();
        let bytes = fs::read(native._storage.store().join("delivery.bin")).unwrap();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let request = decode_request(&read_document(&mut socket)).unwrap();
        assert_eq!(request, next(920));
        let original = native.dispatch(&request).to_json().unwrap();
        let damaged = original.replace(
            "\"action\":\"cancel_undispatched\"",
            "\"action\":\"reconcile\"",
        );
        assert_ne!(damaged, original);
        socket.write_all(damaged.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "malformed plan caused a mutation request"
        );
        assert_eq!(native.supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            fs::read(native._storage.store().join("delivery.bin")).unwrap(),
            bytes
        );
        assert_eq!(native.grant.remaining_operations(), 1);
    });
    let document = received.recv_timeout(Duration::from_secs(10)).unwrap();
    let command = decode_request(document.as_bytes()).unwrap();
    let documents = [
        root.document("bad-plan.json", &next(920)),
        root.document("must-not-run.json", &command),
    ];
    let mut output = Vec::new();
    let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
    assert!(error.contains("invalid or unbound Tower response"));
    assert!(output.is_empty());
    server.join().unwrap();
}
