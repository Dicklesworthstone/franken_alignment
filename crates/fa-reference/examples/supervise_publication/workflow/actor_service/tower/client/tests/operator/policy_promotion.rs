//! Explicit authenticated client documents against independently provisioned
//! original campaigns. Profile declarations never approve or mint a policy key.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    governance::PolicyUpdate,
    requests::actor::{
        FilePolicyPromotionBasis, FileSupervisorPolicyPromotionGrant,
        MAX_POLICY_PROMOTION_REPLAY_EVENTS,
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::action::{ActionState, MAX_ATTEMPTS};
use fa_reference::typestate_cx::PurposeTag;

const CAMPAIGN: u64 = 801;

struct NativePromotion {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorPolicyPromotionGrant,
    _storage: native::Directory,
}

impl NativePromotion {
    fn new() -> Self {
        let storage = native::Directory::new();
        let (mut host, reviewer) =
            FileOversight::create(storage.store(), native::profile()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let governor = host
            .enable_policy_campaigns(
                host.revision(),
                ReplayLimits {
                    cases: 16,
                    input_bytes: 1_048_576,
                },
                8,
            )
            .unwrap();
        let proposal = native::proposal(&native::oversight::spec(&host, b"original"));
        let (actor, mut supervisor) = host.into_actor_gateway();
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        let unknown = native::ready(&mut supervisor, &reviewer, 7);
        {
            let mut host = supervisor.host_mut().unwrap();
            native::oversight::dispatch(&mut host, &unknown);
            let revision = host.revision();
            host.reconcile(revision, unknown.automatic.attempt())
                .unwrap();
        }
        native::observe(&mut supervisor);
        actor.submit(8, &proposal).unwrap();
        let _reserved = native::ready(&mut supervisor, &reviewer, 8);
        let key = {
            let mut host = supervisor.host_mut().unwrap();
            let cut = host.inspect();
            let update = PolicyUpdate::new(
                CAMPAIGN,
                cut.control.sequence,
                cut.control.ledger.epoch,
                Policy::new(2, vec![Predicate::PayloadAtMost(3)]).unwrap(),
            )
            .unwrap();
            let review = host.request_policy_campaign(cut.revision, &update).unwrap();
            let revision = host.revision();
            governor
                .approve(&mut host, revision, &review, false)
                .unwrap()
        };
        let (port, grant) = supervisor
            .policy_promotion_port(key, 12, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
            .unwrap();
        let mut tower =
            PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default())
                .unwrap();
        tower.set_policy_promotion(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            _storage: storage,
        }
    }

    fn command(&self, operation: u64) -> TowerRequest {
        let host = self.supervisor.host().unwrap();
        let cut = host.inspect();
        let mut request = TowerRequest::new(
            format!("promote-policy-{operation}"),
            mount(),
            Verb::Promote,
        );
        request.idempotency_key = Some(operation.to_string());
        request.arguments = ReadArguments::Promote {
            operation,
            expected: FilePolicyPromotionBasis {
                scope: native::profile().delivery.scope,
                campaign: CAMPAIGN,
                journal_revision: cut.revision,
                control_sequence: cut.control.sequence,
                authority_epoch: cut.control.ledger.epoch,
                policy_generation: host.current_policy().unwrap().generation(),
                recorded_at: cut.control.ledger.elapsed,
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
        self.tower
            .exchange(&access, encode_request(request).unwrap().as_bytes())
            .to_json()
            .unwrap()
    }

    fn image(&self) -> Vec<u8> {
        fs::read(self._storage.store().join("delivery.bin")).unwrap()
    }
}

fn next(operation: u64) -> TowerRequest {
    let mut request = TowerRequest::new("operator-policy-plan", mount(), Verb::Next);
    request.arguments = ReadArguments::PolicyPromotionNext {
        campaign: CAMPAIGN,
        operation,
    };
    request
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, name| value.get(name).unwrap())
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

#[test]
fn original_plan_requires_exact_campaign_predecessor_and_executable_promotion() {
    let mut native = NativePromotion::new();
    let query = next(901);
    let before = native.image();
    let original = native.dispatch(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    assert_eq!(
        field(&parsed, &["result", "kind"]).as_str(),
        Some("policy-promotion-next")
    );
    assert_eq!(native.image(), before);
    assert_eq!(native.grant.remaining_operations(), 1);
    for damaged in [
        original.replace(
            "fa.publication-policy-promotion-next/1",
            "fa.publication-policy-promotion-next/2",
        ),
        original.replace(
            "\"kind\":\"policy-promotion-next\"",
            "\"kind\":\"policy-replay\"",
        ),
        original.replace("\"kind\":\"policy-promotion-next\"", "\"kind\":\"refused\""),
        original.replace("original_approved_policy_campaign", "all_campaigns"),
        original.replace(
            "single_independently_approved_policy_campaign",
            "all_policy_changes",
        ),
        original.replace("live_policy_promotion_grant", "durable_global_operation"),
        original.replacen("\"campaign\":\"801\"", "\"campaign\":\"802\"", 1),
        original.replace(
            "\"promotion_operation\":\"901\"",
            "\"promotion_operation\":\"902\"",
        ),
        original.replace(
            "\"registration\":\"fa://1/run/3@11\"",
            "\"registration\":\"fa://1/run/3@12\"",
        ),
        original.replace(
            "\"candidate_policy_generation\":\"2\"",
            "\"candidate_policy_generation\":\"1\"",
        ),
        original.replace("\"can_promote\":true", "\"can_promote\":false"),
        original.replace(
            "\"block_reason\":null",
            "\"block_reason\":\"operation_limit\"",
        ),
        original.replace("promote-policy-901", "promote-policy-902"),
        original.replace("\"verb\":\"promote\"", "\"verb\":\"fence\""),
        original.replace("\"idempotency_key\":\"901\"", "\"idempotency_key\":\"902\""),
        original.replace(
            "\"cost_estimate\":{\"state\":\"unknown\"",
            "\"cost_estimate\":{\"state\":\"known\"",
        ),
        original.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        original.replace("\"policy_disclosed\":false", "\"policy_disclosed\":true"),
        original.replace("\"status\":\"ok\"", "\"status\":\"error\""),
        original.replace("\"retry_safe\":true", "\"retry_safe\":false"),
    ] {
        assert_ne!(damaged, original, "plan mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    let mut unqualified = query;
    unqualified.address.generation = None;
    validate_response(native.dispatch(&unqualified).as_bytes(), &unqualified).unwrap();
    native.tower.set_policy_promotion(None).unwrap();
    let unsupported = native.dispatch(&unqualified);
    validate_response(unsupported.as_bytes(), &unqualified).unwrap();
    assert!(unsupported.contains("\"kind\":\"refused\""));
    let denied = native
        .tower
        .exchange(
            &TowerAccess::new(
                CallerScope::full_tenant_access("1"),
                false,
                vec![PurposeTag::Permit],
                0,
            ),
            encode_request(&unqualified).unwrap().as_bytes(),
        )
        .to_json()
        .unwrap();
    validate_response(denied.as_bytes(), &unqualified).unwrap();
    assert!(denied.contains("\"state\":\"Withheld\""));
    let command = native.command(901);
    let refused = native.dispatch(&command);
    validate_response(refused.as_bytes(), &command).unwrap();
    assert!(refused.contains("\"kind\":\"refused\""));
    for changed in [
        refused.replace("\"retry_safe\":false", "\"retry_safe\":true"),
        refused.replace("\"retry_safe\":false", "\"retry_safe\":\"false\""),
    ] {
        assert_ne!(changed, refused);
        assert!(validate_response(changed.as_bytes(), &command).is_err());
    }
    let malformed_plan_refusal =
        unsupported.replace("\"retry_safe\":true", "\"retry_safe\":\"true\"");
    assert_ne!(malformed_plan_refusal, unsupported);
    assert!(validate_response(malformed_plan_refusal.as_bytes(), &unqualified).is_err());
}

#[test]
fn promotion_receipt_binds_epoch_policy_cancellations_and_historical_retry() {
    let mut native = NativePromotion::new();
    let command = native.command(902);
    let expected = match &command.arguments {
        ReadArguments::Promote { expected, .. } => *expected,
        _ => unreachable!(),
    };
    let before = native.supervisor.host().unwrap().inspect();
    let original = native.dispatch(&command);
    parse_response(original.as_bytes(), &command).unwrap();
    let after = native.supervisor.host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.sequence, before.control.sequence + 1);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 1);
    assert_eq!(after.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Cancelled);
    assert_eq!(
        (after.control.ledger.charged, after.control.ledger.reserved),
        (16, 0)
    );
    assert_eq!(after.executions, 0);
    for damaged in [
        original.replace(
            "fa.publication-policy-promotion/1",
            "fa.publication-policy-promotion/2",
        ),
        original.replace(
            "\"kind\":\"policy-promotion\"",
            "\"kind\":\"policy-replay\"",
        ),
        original.replace("\"kind\":\"policy-promotion\"", "\"kind\":\"refused\""),
        original.replace(
            "\"publication_attempted\":false",
            "\"publication_attempted\":true",
        ),
        original.replace("\"may_write_journal\":true", "\"may_write_journal\":false"),
        original.replace("\"automatic_retry\":false", "\"automatic_retry\":true"),
        original.replace("\"clock_read\":false", "\"clock_read\":true"),
        original.replace("\"cancelled\":[\"2\"]", "\"cancelled\":[\"2\",\"2\"]"),
        original.replace("\"cancelled\":[\"2\"]", "\"cancelled\":[]"),
        original.replace("\"cancelled\":[\"2\"]", "\"cancelled\":[\"0\"]"),
        original.replace("\"refunded_units\":\"16\"", "\"refunded_units\":\"016\""),
        original.replace("\"refunded_units\"", "\"charged_units\""),
        original.replace(
            "\"candidate_policy_generation\":\"2\"",
            "\"candidate_policy_generation\":\"3\"",
        ),
        original.replace("\"recorded_at\":\"1\"", "\"recorded_at\":\"2\""),
        original.replace(
            &format!("\"authority_epoch\":\"{}\"", expected.authority_epoch + 1),
            &format!("\"authority_epoch\":\"{}\"", expected.authority_epoch),
        ),
        original.replace(
            &format!("\"journal_revision\":\"{}\"", expected.journal_revision + 1),
            &format!("\"journal_revision\":\"{}\"", expected.journal_revision + 2),
        ),
        original.replace(
            &format!("\"control_sequence\":\"{}\"", expected.control_sequence + 1),
            &format!("\"control_sequence\":\"{}\"", expected.control_sequence + 2),
        ),
        original.replace("\"retry_safe\":false", "\"retry_safe\":true"),
        original.replacen(
            "\"knowledge\":",
            "\"error\":{\"code\":\"unavailable\"},\"knowledge\":",
            1,
        ),
    ] {
        assert_ne!(damaged, original, "receipt mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &command).is_err());
    }
    let saved = native.image();
    assert_eq!(native.dispatch(&command), original);
    assert_eq!(native.image(), saved);
    let blocked = next(902);
    let response = native.dispatch(&blocked);
    validate_response(response.as_bytes(), &blocked).unwrap();
    assert!(response.contains("\"block_reason\":\"operation_already_used\""));
    assert!(response.contains("\"action\":null"));
}

#[test]
fn authenticated_plan_only_never_follows_its_action_and_checks_full_configured_scope() {
    for foreign_audience in [false, true] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 8);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let query = next(903);
        let peer_query = query.clone();
        let server = std::thread::spawn(move || {
            let mut native = NativePromotion::new();
            let before = native.image();
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(
                decode_request(&read_document(&mut socket)).unwrap(),
                peer_query
            );
            let response = native.dispatch(&peer_query);
            let response = if foreign_audience {
                let changed = response.replace("\"principal\":\"2\"", "\"principal\":\"22\"");
                assert_ne!(changed, response);
                // The plan/action agree internally; the separate client
                // connection audience must still reject this foreign principal.
                validate_response(changed.as_bytes(), &peer_query).unwrap();
                changed
            } else {
                response
            };
            socket
                .write_all(format!("{response}\n").as_bytes())
                .unwrap();
            assert_eq!(
                socket.read(&mut [0_u8]).unwrap(),
                0,
                "unexpected automatic promotion"
            );
            assert_eq!(native.image(), before);
            assert_eq!(native.grant.remaining_operations(), 1);
            response
        });
        let mut output = Vec::new();
        let result = operator_command(
            &args(&profile, &[root.document("plan.json", &query)]),
            &mut output,
        );
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
fn explicit_operator_documents_promote_and_retry_the_independently_approved_campaign() {
    let root = Directory::new();
    // This existing connection profile has no promotion selection or governor.
    let profile = root.settings(credentials().pid(), 8);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativePromotion::new();
        let command = native.command(904);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().inspect();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for request in [next(904), command.clone(), command] {
            assert_eq!(
                decode_request(&read_document(&mut socket)).unwrap(),
                request
            );
            let response = native.dispatch(&request);
            validate_response(response.as_bytes(), &request).unwrap();
            socket
                .write_all(format!("{response}\n").as_bytes())
                .unwrap();
            replies.push(response);
        }
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "unexpected automatic request or retry"
        );
        let after = native.supervisor.host().unwrap().inspect();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(native.grant.remaining_operations(), 0);
        assert_eq!(replies[1], replies[2]);
        replies
    });
    let command = decode_request(
        received
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let documents = [
        root.document("plan.json", &next(904)),
        root.document("promote.json", &command),
        root.document("explicit-retry.json", &command),
    ];
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn forged_refund_acknowledgment_emits_nothing_and_prevents_later_documents() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativePromotion::new();
        let command = native.command(905);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let before = native.supervisor.host().unwrap().inspect();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(
            decode_request(&read_document(&mut socket)).unwrap(),
            command
        );
        let original = native.dispatch(&command);
        let forged = original.replace("\"cancelled\":[\"2\"]", "\"cancelled\":[]");
        assert_ne!(forged, original);
        socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "later request followed an invalid receipt"
        );
        // Client rejection cannot retract the already acknowledged transition.
        assert_eq!(
            native.supervisor.host().unwrap().revision(),
            before.revision + 1
        );
        assert_eq!(native.grant.remaining_operations(), 0);
    });
    let command = decode_request(
        received
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let documents = [
        root.document("promote.json", &command),
        root.document("must-not-retry.json", &command),
    ];
    let mut output = Vec::new();
    assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
    assert!(output.is_empty());
    server.join().unwrap();
}

#[test]
fn a_later_foreign_predecessor_is_rejected_before_connecting_or_sending_the_first_document() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let listener = UnixListener::bind(root.socket()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let native = NativePromotion::new();
    let mut command = native.command(906);
    let ReadArguments::Promote { expected, .. } = &mut command.arguments else {
        panic!("promotion");
    };
    expected.scope.principal += 1;
    let documents = [
        root.document("plan.json", &next(906)),
        root.document("foreign.json", &command),
    ];
    let before = native.image();
    let mut output = Vec::new();
    assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
    assert!(output.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(native.image(), before);
    assert_eq!(native.grant.remaining_operations(), 1);
}

#[test]
fn cancellation_array_parser_bound_covers_the_native_maximum_without_accepting_overflow() {
    let mut native = NativePromotion::new();
    let command = native.command(907);
    let original = native.dispatch(&command);
    // A syntax-size fixture, not evidence that this owner had these attempts.
    // The client establishes bounded structure, never remote ledger truth.
    let ids = (1..=MAX_ATTEMPTS)
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(",");
    let maximal = original.replace("\"cancelled\":[\"2\"]", &format!("\"cancelled\":[{ids}]"));
    validate_response(maximal.as_bytes(), &command).unwrap();
    let overflow = maximal.replace(
        &format!("\"cancelled\":[{ids}]"),
        &format!("\"cancelled\":[{ids},\"{}\"]", MAX_ATTEMPTS + 1),
    );
    assert_ne!(overflow, maximal);
    assert!(validate_response(overflow.as_bytes(), &command).is_err());
}
