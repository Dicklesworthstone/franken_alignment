//! Carry original native policy reports through the actual operator client.
//! Damaging a real response must still stop the explicit document sequence.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorPolicyReplayGrant;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::typestate_cx::PurposeTag;

struct NativeReplay {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorPolicyReplayGrant,
    _storage: native::Directory,
}

impl NativeReplay {
    fn new() -> Self {
        let storage = native::Directory::new();
        let (actor, mut supervisor, _reviewer, proposal) = native::create(&storage);
        native::observe(&mut supervisor);
        let ticket = actor.submit(7, &proposal).unwrap();
        assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request: 7 });
        let (port, grant) = supervisor
            .policy_replay_port(
                1,
                ReplayLimits {
                    cases: 16,
                    input_bytes: 1_048_576,
                },
            )
            .unwrap();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_policy_replay(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            _storage: storage,
        }
    }

    fn exchange(&mut self, request: &TowerRequest) -> String {
        let before = self.supervisor.host().unwrap().inspect();
        let scope = native::profile().delivery.scope;
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            vec![PurposeTag::Experiment],
            0,
        );
        let response = self
            .tower
            .exchange(&access, encode_request(request).unwrap().as_bytes());
        let PublicationResult::PolicyReplay(linked) = &response.result else {
            panic!("expected the original policy replay result: {response:?}");
        };
        match linked.native() {
            Ok(ActorKnowledge::Known { value, .. }) => {
                assert_eq!(value.report().cases().len(), 1);
                assert_eq!(value.report().newly_blocked().len(), 1);
                assert!(!value.report().requires_shadow());
            }
            Err(fa_reference::action::consequence::oversight::actor::ActorError::Capacity) => {}
            other => panic!("unexpected native replay observation: {other:?}"),
        }
        assert_eq!(self.supervisor.host().unwrap().inspect(), before);
        assert_eq!(self.grant.remaining_queries(), 0);
        response.to_json().unwrap()
    }
}

fn replay_request(campaign: u64) -> TowerRequest {
    let mut request = TowerRequest::new("operator-policy", mount(), Verb::PolicyReplay);
    request.arguments = ReadArguments::PolicyReplay {
        campaign,
        expected_policy_generation: native::profile().delivery.policy.generation(),
        candidate: Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap(),
    };
    request.idempotency_key = Some(campaign.to_string());
    request
}

fn replay_settings(root: &Directory) -> PathBuf {
    let path = root.settings(credentials().pid(), 4);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":2", "\"explanation_queries\":0")
        .replace(
            "\"explanation_replay_events\":1024",
            "\"explanation_replay_events\":0",
        )
        .replace("policy_and_selected_witnesses", "none")
        .replacen('{', "{\"policy_replay\":{\"queries\":1,\"cases\":16,\"input_bytes\":1048576,\"disclosure\":\"policy_and_case_traces\"},", 1);
    let settings = OperatorSettings::decode(selected.as_bytes()).unwrap();
    assert_eq!(
        (settings.inspection_queries, settings.explanation_queries),
        (0, 0)
    );
    assert_eq!(settings.policy_replay.as_ref().unwrap().queries, 1);
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let expected = credentials();
    PeerPolicy::new(expected.uid(), expected.gid(), Some(expected.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

#[test]
fn genuine_policy_report_requires_its_verb_and_the_complete_correlated_envelope() {
    let query = replay_request(71);
    let mut native = NativeReplay::new();
    let reply = native.exchange(&query);
    let parsed = parse_response(reply.as_bytes(), &query).unwrap();
    assert_eq!(
        parsed.get("result").unwrap().get("kind").unwrap().as_str(),
        Some("policy-replay")
    );
    for verb in [
        Verb::Get,
        Verb::Propose,
        Verb::Situation,
        Verb::Explain,
        Verb::Precheck,
        Verb::Capabilities,
        Verb::Replay,
    ] {
        let mut wrong = query.clone();
        wrong.verb = verb;
        assert!(
            validate_response(reply.as_bytes(), &wrong).is_err(),
            "accepted {verb:?}"
        );
    }
    for damaged in [
        reply.replace("operator-policy", "other-policy"),
        reply.replace("fa.tower_response/1", "fa.tower_response/2"),
        reply.replace("@11", "@12"),
        reply.replace("\"idempotency_key\":\"71\"", "\"idempotency_key\":\"72\""),
        reply.replacen("\"envelope\":{", "\"envelope\":{\"operator\":true,", 1),
        reply.replacen(
            "\"retry_safe\":true",
            "\"retry_safe\":true,\"retry_safe\":false",
            1,
        ),
        reply.replace("\"kind\":\"policy-replay\"", "\"kind\":\"explanation\""),
        format!("{reply}null"),
    ] {
        assert_ne!(damaged, reply);
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
}

#[test]
fn operator_client_emits_original_known_retry_and_exhaustion_without_extra_work() {
    let root = Directory::new();
    let profile = replay_settings(&root);
    let first = replay_request(71);
    let mut retry = first.clone();
    retry.operation_id = "operator-policy-retry".into();
    let mut exhausted = replay_request(72);
    exhausted.operation_id = "operator-policy-exhausted".into();
    let expected = [first, retry, exhausted];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, request)| root.document(&format!("policy-{index}.json"), request))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeReplay::new();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for (index, expected) in expected.into_iter().enumerate() {
            let request = decode_request(&read_document(&mut socket)).unwrap();
            assert_eq!(request, expected);
            let response = native.exchange(&request);
            let parsed = strict_json::parse(response.as_bytes(), Limits::default()).unwrap();
            let value = parsed.get("result").unwrap().get("value").unwrap();
            if index < 2 {
                assert_eq!(
                    value.get("knowledge").unwrap().get("state").unwrap().as_str(),
                    Some("known")
                );
            } else {
                assert_eq!(
                    value.get("error").unwrap().get("code").unwrap().as_str(),
                    Some("capacity")
                );
            }
            socket.write_all(response.as_bytes()).unwrap();
            socket.write_all(b"\n").unwrap();
            socket.flush().unwrap();
            replies.push(response);
        }
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "unexpected automatic replay"
        );
        assert_eq!(native.grant.remaining_queries(), 0);
        replies
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    let original = server.join().unwrap().join("\n") + "\n";
    assert_eq!(output, original.as_bytes());
}

#[test]
fn damaged_policy_reply_is_not_emitted_and_stops_before_the_next_campaign() {
    let root = Directory::new();
    let profile = replay_settings(&root);
    let first = replay_request(71);
    let documents = [
        root.document("first.json", &first),
        root.document("later.json", &replay_request(72)),
    ];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeReplay::new();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), first);
        let original = native.exchange(&first);
        let damaged =
            original.replace("\"kind\":\"policy-replay\"", "\"kind\":\"situation\"");
        assert_ne!(damaged, original);
        socket.write_all(damaged.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "invalid report triggered another campaign"
        );
        assert_eq!(native.grant.remaining_queries(), 0);
    });
    let mut output = Vec::new();
    let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
    assert!(error.contains("invalid or unbound Tower response"));
    assert!(output.is_empty());
    server.join().unwrap();
}
