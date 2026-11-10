//! Complete native admission-stop acknowledgments over authenticated sockets.

use super::*;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, FileSupervisorFenceGrant, MAX_FENCE_REPLAY_EVENTS,
};
use fa_reference::typestate_cx::PurposeTag;

struct NativeFence {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorFenceGrant,
    _storage: native::Directory,
}

impl NativeFence {
    fn new() -> Self {
        let storage = native::Directory::new();
        let (mut host, reviewer) = FileOversight::create(storage.store(), native::profile()).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let proposal = native::proposal(&native::oversight::spec(&host, b"original"));
        let (actor, mut supervisor) = host.into_actor_gateway();
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        let keys = native::ready(&mut supervisor, &reviewer, 7);
        native::oversight::dispatch(&mut supervisor.host_mut().unwrap(), &keys);
        native::observe(&mut supervisor);
        actor.submit(8, &proposal).unwrap();
        let (port, grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
        let mut tower = PublicationTower::new_control_operator(
            mount(), actor, PublicationLimits::default(),
        ).unwrap();
        tower.set_fence(Some(port)).unwrap();
        Self { tower, supervisor, grant, _storage: storage }
    }

    fn command(&self, operation: u64) -> TowerRequest {
        let state = self.supervisor.host().unwrap().inspect();
        let mut request = TowerRequest::new("operator-fence", mount(), Verb::Fence);
        request.idempotency_key = Some(operation.to_string());
        request.arguments = ReadArguments::Fence {
            operation,
            expected: FileFenceBasis {
                scope: native::profile().delivery.scope,
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
            },
        };
        request
    }

    fn dispatch(&mut self, request: &TowerRequest) -> String {
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
        self.tower.exchange(&access, encode_request(request).unwrap().as_bytes())
            .to_json().unwrap()
    }
}

fn settings(root: &Directory) -> PathBuf {
    let path = root.settings(credentials().pid(), 8);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":2", "\"explanation_queries\":0")
        .replace("\"explanation_replay_events\":1024", "\"explanation_replay_events\":0")
        .replace("policy_and_selected_witnesses", "none")
        .replacen('{', &format!(
            "{{\"fence\":{{\"max_replay_events\":{MAX_FENCE_REPLAY_EVENTS},\"authority\":\"stop_original_admission\"}},"
        ), 1);
    let decoded = OperatorSettings::decode(selected.as_bytes()).unwrap();
    assert!(decoded.fence.is_some());
    assert!(decoded.resolution.is_none());
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap().verify(socket).unwrap();
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, name| value.get(name).unwrap())
}

#[test]
fn genuine_stop_acknowledgment_requires_complete_original_coordinates_and_no_drain_claim() {
    let mut native = NativeFence::new();
    let command = native.command(901);
    let expected = match &command.arguments {
        ReadArguments::Fence { expected, .. } => *expected,
        _ => unreachable!(),
    };
    let before = native.supervisor.host().unwrap().inspect();
    let original = native.dispatch(&command);
    let parsed = parse_response(original.as_bytes(), &command).unwrap();
    assert_eq!(field(&parsed, &["result", "kind"]).as_str(), Some("fence"));
    assert_eq!(field(&parsed, &["result", "value", "knowledge", "value", "progress", "charged_units"]).as_str(), Some("16"));
    assert_eq!(field(&parsed, &["result", "value", "knowledge", "value", "progress", "endpoint_fenced"]).as_bool(), Some(false));
    let after = native.supervisor.host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Cancelled);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.executions, 0);

    let impossible_epoch = original
        .replace(
            &format!("\"authority_epoch\":\"{}\"", expected.authority_epoch + 1),
            &format!("\"authority_epoch\":\"{}\"", expected.authority_epoch + 2),
        )
        .replace(
            &format!("\"revocation_floor\":\"{}\"", expected.authority_epoch + 1),
            &format!("\"revocation_floor\":\"{}\"", expected.authority_epoch + 2),
        );
    for damaged in [
        original.replace("fa.publication-fence/1", "fa.publication-fence/2"),
        original.replace("original_domain_admission", "all_authority_domains"),
        original.replace("\"fence_operation\":\"901\"", "\"fence_operation\":\"902\""),
        original.replace("\"registration\":\"fa://1/run/3@11\"", "\"registration\":\"fa://1/run/3@12\""),
        original.replace("\"principal\":\"2\"", "\"principal\":\"22\""),
        original.replace("\"historical\":true", "\"historical\":false"),
        original.replace("\"clock_read\":false", "\"clock_read\":true"),
        original.replace("\"live_evidence_read\":false", "\"live_evidence_read\":true"),
        original.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        original.replace("\"payload_disclosed\":false", "\"payload_disclosed\":true"),
        original.replace("\"may_write_journal\":true", "\"may_write_journal\":false"),
        original.replace("\"publication_attempted\":false", "\"publication_attempted\":true"),
        original.replace("\"drain_attempted\":false", "\"drain_attempted\":true"),
        original.replace("\"endpoint_fenced\":false", "\"endpoint_fenced\":true"),
        original.replace("\"drained\":false", "\"drained\":true"),
        original.replace("\"reserved_units\":\"0\"", "\"reserved_units\":\"1\""),
        original.replace("\"charged_units\":\"16\"", "\"charged_units\":\"016\""),
        original.replace("\"cancelled\":[\"2\"]", "\"cancelled\":[\"1\"]"),
        original.replace("\"unresolved\":[\"1\"]", "\"unresolved\":[\"1\",\"1\"]"),
        original.replace("\"irrecoverable\":[]", "\"irrecoverable\":[\"3\"]"),
        original.replace("\"actor_halted\":{\"state\":\"unknown\"", "\"actor_halted\":{\"state\":\"known\""),
        original.replace("\"fleet_fenced\":{\"state\":\"unknown\"", "\"fleet_fenced\":{\"state\":\"known\""),
        original.replacen("\"knowledge\":", "\"error\":{\"code\":\"unavailable\"},\"knowledge\":", 1),
        original.replace(
            &format!("\"journal_revision\":\"{}\"", expected.journal_revision + 1),
            &format!("\"journal_revision\":\"{}\"", expected.journal_revision + 2),
        ),
        impossible_epoch,
    ] {
        assert_ne!(damaged, original);
        assert!(validate_response(damaged.as_bytes(), &command).is_err());
    }
    let mut changed = command.clone();
    if let ReadArguments::Fence { expected, .. } = &mut changed.arguments {
        expected.authority_epoch += 1;
    }
    assert!(validate_response(original.as_bytes(), &changed).is_err());

    // Later native drain is independent. The same grant still returns the
    // original historical admission-stop cut, not a synthesized live status.
    {
        let mut host = native.supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.progress_stop(revision, ElapsedTick(2)).unwrap().progress.drained());
    }
    let current = native.supervisor.host().unwrap().inspect();
    let bytes = fs::read(native._storage.store().join("delivery.bin")).unwrap();
    let repeated = native.dispatch(&command);
    assert_eq!(repeated, original);
    validate_response(repeated.as_bytes(), &command).unwrap();
    assert_eq!(native.supervisor.host().unwrap().inspect(), current);
    assert_eq!(fs::read(native._storage.store().join("delivery.bin")).unwrap(), bytes);
    assert_eq!(native.grant.remaining_operations(), 0);
}

#[test]
fn authenticated_client_forwards_only_explicit_fence_and_exact_retry_documents() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeFence::new();
        let command = native.command(910);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().inspect();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for _ in 0..2 {
            let request = decode_request(&read_document(&mut socket)).unwrap();
            assert_eq!(request, command);
            let response = native.dispatch(&request);
            validate_response(response.as_bytes(), &request).unwrap();
            let current = native.supervisor.host().unwrap().inspect();
            assert_eq!(current.revision, before.revision + 1);
            assert_eq!(current.control.ledger.charged, before.control.ledger.charged);
            assert_eq!(current.executions, 0);
            socket.write_all(response.as_bytes()).unwrap();
            socket.write_all(b"\n").unwrap();
            socket.flush().unwrap();
            replies.push(response);
        }
        assert_eq!(socket.read(&mut [0_u8]).unwrap(), 0, "unexpected automatic request or retry");
        assert_eq!(native.grant.remaining_operations(), 0);
        assert_eq!(replies[0], replies[1]);
        replies
    });
    let document = received.recv_timeout(Duration::from_secs(10)).unwrap();
    let command = decode_request(document.as_bytes()).unwrap();
    let documents = [
        root.document("fence.json", &command),
        root.document("same-explicit-fence.json", &command),
    ];
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(output, (server.join().unwrap().join("\n") + "\n").as_bytes());
}

#[test]
fn damaged_stop_acknowledgment_emits_nothing_and_sends_no_later_request_or_retry() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeFence::new();
        let command = native.command(920);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let request = decode_request(&read_document(&mut socket)).unwrap();
        assert_eq!(request, command);
        let original = native.dispatch(&request);
        let stopped = native.supervisor.host().unwrap().inspect();
        let bytes = fs::read(native._storage.store().join("delivery.bin")).unwrap();
        assert!(stopped.stop.is_some());
        let damaged = original.replace("\"endpoint_fenced\":false", "\"endpoint_fenced\":true");
        assert_ne!(damaged, original);
        socket.write_all(damaged.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(socket.read(&mut [0_u8]).unwrap(), 0, "invalid acknowledgment caused another request");
        // Rejecting the unbound reply cannot undo the acknowledged native Stop.
        assert_eq!(native.supervisor.host().unwrap().inspect(), stopped);
        assert_eq!(fs::read(native._storage.store().join("delivery.bin")).unwrap(), bytes);
        assert_eq!(native.grant.remaining_operations(), 0);
        assert_eq!(native.dispatch(&command), original);
    });
    let document = received.recv_timeout(Duration::from_secs(10)).unwrap();
    let command = decode_request(document.as_bytes()).unwrap();
    let documents = [
        root.document("fence.json", &command),
        root.document("must-not-send.json", &command),
    ];
    let mut output = Vec::new();
    let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
    assert!(error.contains("invalid or unbound Tower response"));
    assert!(output.is_empty());
    server.join().unwrap();
}
