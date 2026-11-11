//! Real original-owner plans, receipts and authenticated explicit documents.
//! Synthetic helper decisions establish only control behavior in these fixtures.

use super::*;
use fa_reference::action::{ActionState, ResolvedTarget};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, FileSupervisorJournalTailGrant, FileSupervisorNarrowGrant,
    MAX_NARROW_REPLAY_EVENTS,
};
use fa_reference::typestate_cx::PurposeTag;

struct NativeNarrow {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorNarrowGrant,
    _tail: FileSupervisorJournalTailGrant,
    _storage: native::Directory,
}

impl NativeNarrow {
    fn new() -> Self {
        let storage = native::Directory::new();
        let (mut host, reviewer) = FileOversight::create(storage.store(), native::profile()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let proposal = native::proposal(&native::oversight::spec(&host, b"original"));
        let (actor, mut supervisor) = host.into_actor_gateway();
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        let keys = native::ready(&mut supervisor, &reviewer, 7);
        {
            let mut host = supervisor.host_mut().unwrap();
            native::oversight::dispatch(&mut host, &keys);
            let revision = host.revision();
            host.reconcile(revision, keys.automatic.attempt()).unwrap();
        }
        native::observe(&mut supervisor);
        actor.submit(8, &proposal).unwrap();
        let _reserved = native::ready(&mut supervisor, &reviewer, 8);
        let (port, grant) = supervisor.narrow_port(12, 4, MAX_NARROW_REPLAY_EVENTS).unwrap();
        let (tail, tail_grant) = supervisor.journal_tail_port(2, 16).unwrap();
        let mut tower = PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_narrow(Some(port)).unwrap();
        tower.set_journal_tail(Some(tail)).unwrap();
        Self { tower, supervisor, grant, _tail: tail_grant, _storage: storage }
    }

    fn next(&self, operation: u64, retained_targets: Vec<ResolvedTarget>) -> TowerRequest {
        let mut request = TowerRequest::new("operator-narrow-plan", mount(), Verb::Next);
        request.arguments = ReadArguments::NarrowingNext { operation, retained_targets };
        request
    }

    fn command(&self, operation: u64, retained_targets: Vec<ResolvedTarget>) -> TowerRequest {
        let state = self.supervisor.host().unwrap().inspect();
        let mut request = TowerRequest::new(format!("narrow-authority-{operation}"), mount(), Verb::Narrow);
        request.idempotency_key = Some(operation.to_string());
        request.arguments = ReadArguments::Narrow {
            operation,
            expected: FileNarrowBasis {
                scope: native::profile().delivery.scope,
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
                recorded_at: state.control.ledger.elapsed,
            },
            retained_targets,
        };
        request
    }

    fn dispatch(&mut self, request: &TowerRequest) -> String {
        let access = TowerAccess::new(CallerScope {
            caller_tenant: native::profile().delivery.scope.tenant.to_string(),
            allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
            granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
        }, true, vec![PurposeTag::Permit], 0);
        self.tower.exchange(&access, encode_request(request).unwrap().as_bytes()).to_json().unwrap()
    }
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, name| value.get(name).unwrap())
}

fn target_json(target: ResolvedTarget) -> String {
    format!(r#"{{"adapter":"{}","object":"{}","contract_version":"{}","expected_version":"{}","generation":"{}"}}"#,
        target.adapter, target.object, target.contract_version, target.expected_version, target.generation)
}

fn settings(root: &Directory) -> PathBuf {
    let path = root.settings(credentials().pid(), 8);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original.replacen('{', &format!(
        "{{\"narrowing\":{{\"queries\":12,\"operations\":4,\"max_replay_events\":{MAX_NARROW_REPLAY_EVENTS},\"authority\":\"narrow_original_targets\"}},"
    ), 1);
    assert!(OperatorSettings::decode(selected.as_bytes()).unwrap().narrowing.is_some());
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap().verify(socket).unwrap();
}

#[test]
fn genuine_plans_require_exact_intersection_and_fully_bound_explicit_narrow_action() {
    let mut native = NativeNarrow::new();
    let target = native::profile().delivery.target;
    let query = native.next(901, vec![target]);
    let before = native.supervisor.host().unwrap().inspect();
    let original = native.dispatch(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    assert_eq!(field(&parsed, &["result", "kind"]).as_str(), Some("narrowing-next"));
    assert_eq!(native.supervisor.host().unwrap().inspect(), before);
    assert_eq!(native.grant.remaining_operations(), 4);
    let target = target_json(target);
    for damaged in [
        original.replace("fa.publication-narrowing-next/1", "fa.publication-narrowing-next/2"),
        original.replace("original_run_target_ceiling", "all_runs"),
        original.replace("single_connected_target_narrowing", "complete_action_ranking"),
        original.replace("live_narrowing_grant", "durable_global_operation"),
        original.replace("\"narrowing_operation\":\"901\"", "\"narrowing_operation\":\"902\""),
        original.replace("\"registration\":\"fa://1/run/3@11\"", "\"registration\":\"fa://1/run/3@12\""),
        original.replace("\"effective_targets\":[", "\"effective_targets\":[],\"extra_targets\":["),
        original.replace(&format!("\"effective_targets\":[{target}]"), "\"effective_targets\":[]"),
        original.replace("\"current_ceiling\":null", "\"current_ceiling\":[]"),
        original.replace("\"can_narrow\":true", "\"can_narrow\":false"),
        original.replace("\"block_reason\":null", "\"block_reason\":\"operation_limit\""),
        original.replace("narrow-authority-901", "narrow-authority-902"),
        original.replace("\"verb\":\"narrow\"", "\"verb\":\"fence\""),
        original.replace("\"idempotency_key\":\"901\"", "\"idempotency_key\":\"902\""),
        original.replace("\"cost_estimate\":{\"state\":\"unknown\"", "\"cost_estimate\":{\"state\":\"known\""),
        original.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        original.replace("\"status\":\"ok\"", "\"status\":\"error\""),
    ] {
        assert_ne!(damaged, original);
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    let mut unqualified = query.clone();
    unqualified.address.generation = None;
    validate_response(native.dispatch(&unqualified).as_bytes(), &unqualified).unwrap();
}

#[test]
fn exact_native_receipt_preserves_unknown_liability_and_cannot_claim_widening_or_epoch_reset() {
    let mut native = NativeNarrow::new();
    let command = native.command(902, Vec::new());
    let expected = match &command.arguments { ReadArguments::Narrow { expected, .. } => *expected, _ => unreachable!() };
    let before = native.supervisor.host().unwrap().inspect();
    let original = native.dispatch(&command);
    parse_response(original.as_bytes(), &command).unwrap();
    let after = native.supervisor.host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.sequence, before.control.sequence + 1);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(after.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Cancelled);
    assert_eq!((after.control.ledger.charged, after.control.ledger.reserved), (16, 0));
    assert_eq!(after.executions, 0);
    let target = target_json(native::profile().delivery.target);
    for damaged in [
        original.replace("fa.publication-narrowing/1", "fa.publication-narrowing/2"),
        original.replace("\"reset_attempted\":false", "\"reset_attempted\":true"),
        original.replace("\"publication_attempted\":false", "\"publication_attempted\":true"),
        original.replace("\"may_write_journal\":true", "\"may_write_journal\":false"),
        original.replace("\"clock_read\":false", "\"clock_read\":true"),
        original.replace("\"cancelled\":[\"2\"]", "\"cancelled\":[\"2\",\"2\"]"),
        original.replace("\"cancelled\":[\"2\"]", "\"cancelled\":[]"),
        original.replace("\"refunded_units\":\"16\"", "\"refunded_units\":\"016\""),
        original.replace("\"refunded_units\"", "\"charged_units\""),
        original.replace("\"effective_targets\":[]", &format!("\"effective_targets\":[{target}]")),
        original.replace("\"previous_ceiling\":null", "\"previous_ceiling\":false"),
        original.replace("\"recorded_at\":\"1\"", "\"recorded_at\":\"2\""),
        original.replace(&format!("\"authority_epoch\":\"{}\"", expected.authority_epoch),
            &format!("\"authority_epoch\":\"{}\"", expected.authority_epoch + 1)),
        original.replace(&format!("\"journal_revision\":\"{}\"", expected.journal_revision + 1),
            &format!("\"journal_revision\":\"{}\"", expected.journal_revision + 2)),
        original.replace(&format!("\"control_sequence\":\"{}\"", expected.control_sequence + 1),
            &format!("\"control_sequence\":\"{}\"", expected.control_sequence + 2)),
        original.replacen("\"knowledge\":", "\"error\":{\"code\":\"unavailable\"},\"knowledge\":", 1),
    ] {
        assert_ne!(damaged, original);
        assert!(validate_response(damaged.as_bytes(), &command).is_err());
    }
    let wider = native.command(903, vec![native::profile().delivery.target]);
    let receipt = native.dispatch(&wider);
    validate_response(receipt.as_bytes(), &wider).unwrap();
    let forged = receipt.replace("\"effective_targets\":[]", &format!("\"effective_targets\":[{target}]"));
    assert!(validate_response(forged.as_bytes(), &wider).is_err());
    let files = fs::read(native._storage.store().join("delivery.bin")).unwrap();
    assert_eq!(native.dispatch(&command), original);
    assert_eq!(fs::read(native._storage.store().join("delivery.bin")).unwrap(), files);

    let mut tail = TowerRequest::new("narrow-tail", mount(), Verb::Tail);
    tail.arguments = ReadArguments::JournalTail { after_revision: before.revision };
    let tail_reply = native.dispatch(&tail);
    validate_response(tail_reply.as_bytes(), &tail).unwrap();
    assert!(tail_reply.contains("\"kind\":\"narrow_authority\""));
    let falsely_linked = tail_reply.replace("\"attempt\":null", "\"attempt\":\"1\"");
    assert!(validate_response(falsely_linked.as_bytes(), &tail).is_err());
}

#[test]
fn authenticated_client_never_executes_a_plan_or_retries_without_explicit_documents() {
    for foreign_audience in [false, true] {
        let root = Directory::new();
        let profile = settings(&root);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let query = TowerRequest {
            arguments: ReadArguments::NarrowingNext { operation: 904, retained_targets: Vec::new() },
            ..TowerRequest::new("narrow-plan-only", mount(), Verb::Next)
        };
        let peer_query = query.clone();
        let server = std::thread::spawn(move || {
            let mut native = NativeNarrow::new();
            let before = native.supervisor.host().unwrap().inspect();
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), peer_query);
            let response = native.dispatch(&peer_query);
            let response = if foreign_audience {
                // Run spelling still matches, and both plan/action agree with
                // each other. The original configured principal must also bind.
                let changed = response.replace("\"principal\":\"2\"", "\"principal\":\"22\"");
                assert_ne!(changed, response);
                changed
            } else { response };
            socket.write_all(format!("{response}\n").as_bytes()).unwrap();
            assert_eq!(socket.read(&mut [0_u8]).unwrap(), 0, "unexpected automatically selected action");
            assert_eq!(native.supervisor.host().unwrap().inspect(), before);
            assert_eq!(native.grant.remaining_operations(), 4);
            response
        });
        let mut output = Vec::new();
        let result = operator_command(&args(&profile, &[root.document("plan.json", &query)]), &mut output);
        let response = server.join().unwrap();
        if foreign_audience {
            assert!(result.is_err());
            assert!(output.is_empty());
        } else {
            result.unwrap();
            assert_eq!(output, format!("{response}\n").as_bytes());
        }
    }
}

#[test]
fn authenticated_explicit_narrow_and_retry_emit_original_receipts_once_per_document() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeNarrow::new();
        let command = native.command(905, Vec::new());
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().inspect();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for _ in 0..2 {
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), command);
            let response = native.dispatch(&command);
            validate_response(response.as_bytes(), &command).unwrap();
            socket.write_all(format!("{response}\n").as_bytes()).unwrap();
            replies.push(response);
        }
        assert_eq!(socket.read(&mut [0_u8]).unwrap(), 0, "unexpected automatic request or retry");
        let after = native.supervisor.host().unwrap().inspect();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(native.grant.remaining_operations(), 3);
        assert_eq!(replies[0], replies[1]);
        replies
    });
    let command = decode_request(received.recv_timeout(Duration::from_secs(10)).unwrap().as_bytes()).unwrap();
    let documents = [root.document("narrow.json", &command), root.document("explicit-retry.json", &command)];
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(output, (server.join().unwrap().join("\n") + "\n").as_bytes());
}

#[test]
fn forged_widening_receipt_emits_nothing_and_sends_no_later_document() {
    let root = Directory::new();
    let profile = settings(&root);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeNarrow::new();
        let command = native.command(906, Vec::new());
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), command);
        let original = native.dispatch(&command);
        let forged = original.replace("\"effective_targets\":[]", &format!(
            "\"effective_targets\":[{}]", target_json(native::profile().delivery.target),
        ));
        assert_ne!(forged, original);
        socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
        assert_eq!(socket.read(&mut [0_u8]).unwrap(), 0, "a later request followed the rejected receipt");
        assert_eq!(native.grant.remaining_operations(), 3);
    });
    let command = decode_request(received.recv_timeout(Duration::from_secs(10)).unwrap().as_bytes()).unwrap();
    let documents = [root.document("narrow.json", &command), root.document("must-not-retry.json", &command)];
    let mut output = Vec::new();
    assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
    assert!(output.is_empty());
    server.join().unwrap();
}
