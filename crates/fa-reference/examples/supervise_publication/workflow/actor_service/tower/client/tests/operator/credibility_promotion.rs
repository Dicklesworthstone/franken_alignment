//! Real native qualifications behind the existing authenticated document client.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileCredibilityPromotionBasis, FileSupervisorCredibilityPromotionGrant,
    MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::joint_credibility::JointReplayBudget;
use fa_reference::tower::publication::PublicationLimits;
use fa_reference::typestate_cx::PurposeTag;

#[path = "../../../../../../../../tests/support/file_credibility_promotion.rs"]
mod credibility;

struct NativePromotion {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorCredibilityPromotionGrant,
    storage: credibility::Directory,
}

impl NativePromotion {
    fn new() -> Self {
        let (storage, mut host, _human, evaluator) = credibility::ready(0, JointReplayBudget::default());
        credibility::calibrate(&mut host, &evaluator);
        let (actor, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.credibility_promotion_port(
            12, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
        ).unwrap();
        let mut tower = PublicationTower::new_control_operator(
            mount(), actor, PublicationLimits { max_response_bytes: 65_536 },
        ).unwrap();
        tower.set_credibility_promotion(Some(port)).unwrap();
        Self { tower, supervisor, grant, storage }
    }

    fn command(&self, operation: u64) -> TowerRequest {
        let host = self.supervisor.host().unwrap();
        let state = host.inspect();
        let report = host.credibility_report().unwrap();
        let mut request = TowerRequest::new(format!("promote-credibility-{operation}"), mount(), Verb::Promote);
        request.idempotency_key = Some(operation.to_string());
        request.arguments = ReadArguments::PromoteCredibility {
            operation,
            expected: FileCredibilityPromotionBasis {
                scope: credibility::profile(0).delivery.scope,
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
                evaluation_revision: report.revision,
                congress_generation: 1,
                policy_generation: report.policy_generation,
                recorded_at: state.control.ledger.elapsed,
            },
        };
        request
    }

    fn dispatch(&mut self, request: &TowerRequest) -> String {
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: credibility::profile(0).delivery.scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            }, true, vec![PurposeTag::Permit], 0,
        );
        self.tower.exchange(&access, encode_request(request).unwrap().as_bytes()).to_json().unwrap()
    }
}

fn next(operation: u64) -> TowerRequest {
    let mut request = TowerRequest::new("credible-plan", mount(), Verb::Next);
    request.arguments = ReadArguments::CredibilityPromotionNext { operation };
    request
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap().verify(socket).unwrap();
}

#[test]
fn plan_and_receipt_bind_the_exact_native_qualification_coordinates() {
    let mut native = NativePromotion::new();
    let query = next(901);
    let before = native.storage.bytes();
    let plan = native.dispatch(&query);
    validate_response(plan.as_bytes(), &query).unwrap();
    assert!(plan.contains("\"can_promote\":true"));
    assert_eq!(native.storage.bytes(), before);
    for changed in [
        plan.replace("single_independently_evaluated_credibility", "all_weight_changes"),
        plan.replace("live_credibility_promotion_grant", "durable_global_operation"),
        plan.replace("\"kind\":\"credibility_promotion\"", "\"kind\":\"policy_promotion\""),
        plan.replace("\"promotion_operation\":\"901\"", "\"promotion_operation\":\"902\""),
        plan.replace("\"kind\":\"credibility-promotion-next\"", "\"kind\":\"refused\""),
        plan.replace("\"credibility_disclosed\":false", "\"credibility_disclosed\":true"),
    ] {
        assert_ne!(changed, plan);
        assert!(validate_response(changed.as_bytes(), &query).is_err());
    }
    let command = native.command(901);
    let expected = match &command.arguments {
        ReadArguments::PromoteCredibility { expected, .. } => *expected,
        _ => unreachable!(),
    };
    let receipt = native.dispatch(&command);
    validate_response(receipt.as_bytes(), &command).unwrap();
    assert!(receipt.contains("\"kind\":\"credibility-promotion\""));
    for changed in [
        receipt.replace("fa.publication-credibility-promotion/1", "fa.publication-credibility-promotion/2"),
        receipt.replace("\"kind\":\"credibility-promotion\"", "\"kind\":\"refused\""),
        receipt.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        receipt.replace("\"automatic_retry\":false", "\"automatic_retry\":true"),
        receipt.replace("\"retry_safe\":false", "\"retry_safe\":true"),
        receipt.replace("\"publication_attempted\":false", "\"publication_attempted\":true"),
        receipt.replace("\"congress_generation\":\"2\"", "\"congress_generation\":\"3\""),
        receipt.replace(&format!("\"evaluation_revision\":\"{}\"", expected.evaluation_revision + 1),
            &format!("\"evaluation_revision\":\"{}\"", expected.evaluation_revision)),
        receipt.replace(&format!("\"authority_epoch\":\"{}\"", expected.authority_epoch + 1),
            &format!("\"authority_epoch\":\"{}\"", expected.authority_epoch + 2)),
        receipt.replace("\"refunded_units\"", "\"forgiven_charges\""),
        receipt.replace("\"policy_generation\":\"1\"", "\"policy_generation\":\"2\""),
    ] {
        assert_ne!(changed, receipt);
        assert!(validate_response(changed.as_bytes(), &command).is_err());
    }
    let committed = native.storage.bytes();
    assert_eq!(native.dispatch(&command), receipt);
    assert_eq!(native.storage.bytes(), committed);
    assert_eq!(native.grant.remaining_operations(), 0);
}

#[test]
fn authenticated_explicit_documents_promote_once_and_preserve_exact_retry() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativePromotion::new();
        let command = native.command(902);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().inspect();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for request in [next(902), command.clone(), command] {
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), request);
            let response = native.dispatch(&request);
            validate_response(response.as_bytes(), &request).unwrap();
            socket.write_all(format!("{response}\n").as_bytes()).unwrap();
            replies.push(response);
        }
        assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0, "unexpected automatic follow-up");
        let after = native.supervisor.host().unwrap().inspect();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.executions, before.executions);
        assert_eq!(native.grant.remaining_operations(), 0);
        assert_eq!(replies[1], replies[2]);
        replies
    });
    let command = decode_request(received.recv_timeout(Duration::from_secs(10)).unwrap().as_bytes()).unwrap();
    let documents = [root.document("next.json", &next(902)),
        root.document("promote.json", &command), root.document("explicit-retry.json", &command)];
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(output, (server.join().unwrap().join("\n") + "\n").as_bytes());
}

#[test]
fn a_foreign_but_internally_consistent_plan_is_rejected_by_the_connection_audience() {
    for foreign in [false, true] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 8);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let request = next(903);
        let peer_request = request.clone();
        let server = std::thread::spawn(move || {
            let mut native = NativePromotion::new();
            let saved = native.storage.bytes();
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), peer_request);
            let response = native.dispatch(&peer_request);
            let response = if foreign {
                let changed = response.replace("\"principal\":\"2\"", "\"principal\":\"22\"");
                assert_ne!(changed, response);
                validate_response(changed.as_bytes(), &peer_request).unwrap();
                changed
            } else { response };
            socket.write_all(format!("{response}\n").as_bytes()).unwrap();
            assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0, "plan caused automatic promotion");
            assert_eq!(native.storage.bytes(), saved);
            assert_eq!(native.grant.remaining_operations(), 1);
            response
        });
        let mut output = Vec::new();
        let result = operator_command(&args(&profile, &[root.document("plan.json", &request)]), &mut output);
        let response = server.join().unwrap();
        if foreign {
            assert!(result.is_err());
            assert!(output.is_empty());
        } else {
            result.unwrap();
            assert_eq!(output, format!("{response}\n").as_bytes());
        }
    }
}

#[test]
fn an_invalid_receipt_stops_output_and_later_documents_without_undoing_promotion() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativePromotion::new();
        let command = native.command(904);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().revision();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), command);
        let original = native.dispatch(&command);
        let forged = original.replace("\"congress_generation\":\"2\"", "\"congress_generation\":\"3\"");
        assert_ne!(forged, original);
        socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
        assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0, "invalid receipt was followed by another document");
        assert_eq!(native.supervisor.host().unwrap().revision(), before + 1);
        assert_eq!(native.grant.remaining_operations(), 0);
    });
    let command = decode_request(received.recv_timeout(Duration::from_secs(10)).unwrap().as_bytes()).unwrap();
    let documents = [root.document("promote.json", &command), root.document("do-not-retry.json", &command)];
    let mut output = Vec::new();
    assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
    assert!(output.is_empty());
    server.join().unwrap();
}
