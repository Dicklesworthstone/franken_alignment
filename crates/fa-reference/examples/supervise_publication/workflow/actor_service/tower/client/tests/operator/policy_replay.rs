//! Carry original native policy reports through the actual operator client.
//! Damaging a real response must still stop the explicit document sequence.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorPolicyReplayGrant;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::tower::publication::{
    MAX_PUBLICATION_RESPONSE_BYTES, MIN_PUBLICATION_RESPONSE_BYTES,
};
use fa_reference::typestate_cx::PurposeTag;

struct NativeReplay {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorPolicyReplayGrant,
    cases: usize,
    _storage: native::Directory,
}

impl NativeReplay {
    fn new() -> Self {
        Self::with_cases(1, MIN_PUBLICATION_RESPONSE_BYTES)
    }

    fn with_cases(cases: usize, response_bytes: usize) -> Self {
        let storage = native::Directory::new();
        let (actor, mut supervisor, _reviewer, proposal) = native::create(&storage);
        for index in 0..cases {
            let request = 7 + index as u64;
            native::observe(&mut supervisor);
            let ticket = actor.submit(request, &proposal).unwrap();
            assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request });
        }
        let (port, grant) = supervisor
            .policy_replay_port(
                1,
                ReplayLimits {
                    cases: 16,
                    input_bytes: 1_048_576,
                },
            )
            .unwrap();
        let mut tower = PublicationTower::new(
            mount(),
            actor,
            PublicationLimits {
                max_response_bytes: response_bytes,
            },
        )
        .unwrap();
        tower.set_policy_replay(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            cases,
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
                assert_eq!(value.report().cases().len(), self.cases);
                assert_eq!(value.report().newly_blocked().len(), self.cases);
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
    replay_request_with_candidate(
        campaign,
        Policy::new(2, vec![Predicate::PayloadAtMost(1)]).unwrap(),
    )
}

fn replay_request_with_candidate(campaign: u64, candidate: Policy) -> TowerRequest {
    let mut request = TowerRequest::new("operator-policy", mount(), Verb::PolicyReplay);
    request.arguments = ReadArguments::PolicyReplay {
        campaign,
        expected_policy_generation: native::profile().delivery.policy.generation(),
        candidate,
    };
    request.idempotency_key = Some(campaign.to_string());
    request
}

fn traced_candidate() -> Policy {
    let mut nodes = vec![Predicate::PayloadAtMost(1); 127];
    nodes.push(Predicate::All((0..127).collect()));
    Policy::new(2, nodes).unwrap()
}

fn selected_replay_settings(root: &Directory, response_bytes: usize) -> PathBuf {
    let path = replay_settings(root);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original.replacen('{', &format!("{{\"response_bytes\":{response_bytes},"), 1);
    assert_eq!(
        OperatorSettings::decode(selected.as_bytes())
            .unwrap()
            .response_bytes,
        response_bytes,
    );
    fs::write(&path, selected).unwrap();
    path
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
                    value
                        .get("knowledge")
                        .unwrap()
                        .get("state")
                        .unwrap()
                        .as_str(),
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
        let damaged = original.replace("\"kind\":\"policy-replay\"", "\"kind\":\"situation\"");
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

#[test]
fn operator_client_preserves_complete_large_literals_traces_and_cached_retries() {
    for (cases, candidate) in [
        (
            1,
            Policy::new(2, vec![Predicate::PayloadIs(vec![0xaa; 20_000])]).unwrap(),
        ),
        (6, traced_candidate()),
    ] {
        let root = Directory::new();
        let profile = selected_replay_settings(&root, MAX_PUBLICATION_RESPONSE_BYTES);
        let first = replay_request_with_candidate(71, candidate);
        let mut retry = first.clone();
        retry.operation_id = "operator-large-retry".into();
        let expected = [first, retry];
        let documents: Vec<_> = expected
            .iter()
            .enumerate()
            .map(|(index, request)| root.document(&format!("large-{index}.json"), request))
            .collect();
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut native = NativeReplay::with_cases(cases, MAX_PUBLICATION_RESPONSE_BYTES);
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            let mut replies = Vec::new();
            let mut retained = None;
            let mut original_knowledge = None;
            for expected in expected {
                let request = decode_request(&read_document(&mut socket)).unwrap();
                assert_eq!(request, expected);
                let response = native.exchange(&request);
                assert!(response.len() > MIN_PUBLICATION_RESPONSE_BYTES);
                let parsed = parse_response(response.as_bytes(), &request).unwrap();
                let knowledge = parsed
                    .get("result")
                    .unwrap()
                    .get("value")
                    .unwrap()
                    .get("knowledge")
                    .unwrap();
                assert_eq!(knowledge.get("state").unwrap().as_str(), Some("known"));
                let report = knowledge.get("value").unwrap();
                let rows = report.get("cases").unwrap().as_array().unwrap();
                assert_eq!(rows.len(), cases);
                assert_eq!(
                    report.get("case_count").unwrap().as_str().unwrap(),
                    cases.to_string()
                );
                if cases == 6 {
                    assert_eq!(
                        strict_json::parse(
                            response.as_bytes(),
                            Limits {
                                max_bytes: MAX_REPLY_BYTES,
                                max_depth: 16,
                                max_items: 4096,
                                max_string_bytes: MAX_REQUEST_STRING_BYTES,
                            }
                        )
                        .unwrap_err()
                        .kind,
                        strict_json::ErrorKind::ItemLimit,
                    );
                    for row in rows {
                        assert_eq!(
                            row.get("candidate")
                                .unwrap()
                                .get("trace")
                                .unwrap()
                                .as_array()
                                .unwrap()
                                .len(),
                            128
                        );
                    }
                } else {
                    let nodes = report
                        .get("candidate_policy")
                        .unwrap()
                        .get("nodes")
                        .unwrap()
                        .as_array()
                        .unwrap();
                    assert_eq!(
                        nodes[0].get("value_hex").unwrap().as_str().unwrap(),
                        "aa".repeat(20_000)
                    );
                }
                if let Some(original) = &original_knowledge {
                    assert_eq!(knowledge, original);
                    assert_eq!(Some(native.grant.retained_bytes()), retained);
                } else {
                    original_knowledge = Some(knowledge.clone());
                    retained = Some(native.grant.retained_bytes());
                }
                assert_eq!(native.grant.remaining_queries(), 0);
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
            replies
        });
        let mut output = Vec::new();
        operator_command(&args(&profile, &documents), &mut output).unwrap();
        assert_eq!(
            output,
            (server.join().unwrap().join("\n") + "\n").as_bytes()
        );
    }
}

#[test]
fn larger_policy_parser_keeps_other_verbs_bounded_and_rejects_damaged_reports() {
    let request = replay_request_with_candidate(71, traced_candidate());
    let mut native = NativeReplay::with_cases(6, MAX_PUBLICATION_RESPONSE_BYTES);
    let response = native.exchange(&request);
    validate_response(response.as_bytes(), &request).unwrap();
    let retained = native.grant.retained_bytes();

    // Relabeling this genuine large document would pass the generic envelope
    // and kind checks; unrelated operations still reject its parser work.
    for (verb, kind) in [
        (Verb::Situation, "situation"),
        (Verb::Explain, "explanation"),
    ] {
        let mut relabeled_request = request.clone();
        relabeled_request.verb = verb;
        let relabeled = response.replace(
            "\"kind\":\"policy-replay\"",
            &format!("\"kind\":\"{kind}\""),
        );
        assert_ne!(relabeled, response);
        assert!(validate_response(relabeled.as_bytes(), &relabeled_request).is_err());
    }
    for damaged in [
        response.replace("operator-policy", "wrong-operation"),
        response.replace("\"idempotency_key\":\"71\"", "\"idempotency_key\":\"72\""),
        response.replacen("\"envelope\":{", "\"envelope\":{\"extra\":null,", 1),
        response.replacen(
            "\"retry_safe\":true",
            "\"retry_safe\":true,\"retry_safe\":false",
            1,
        ),
        format!("{response}null"),
    ] {
        assert_ne!(damaged, response);
        assert!(validate_response(damaged.as_bytes(), &request).is_err());
    }
    // An attacker cannot use the larger allowance to remove the finite item,
    // nesting, string, or complete-response byte bounds.
    let too_many = response.replacen(
        "\"value\":{",
        &format!(
            "\"value\":{{\"items\":[{}],",
            vec!["0"; MAX_POLICY_REPLAY_REPLY_ITEMS + 1].join(",")
        ),
        1,
    );
    assert!(too_many.len() < MAX_REPLY_BYTES);
    let too_deep = response.replacen(
        "\"value\":{",
        &format!(
            "\"value\":{{\"nested\":{}0{},",
            "[".repeat(17),
            "]".repeat(17)
        ),
        1,
    );
    let too_long = response.replacen(
        "\"value\":{",
        &format!(
            "\"value\":{{\"literal\":\"{}\",",
            "x".repeat(MAX_REQUEST_STRING_BYTES + 1)
        ),
        1,
    );
    let too_large = format!("{response}{}", " ".repeat(MAX_REPLY_BYTES));
    for damaged in [too_many, too_deep, too_long, too_large] {
        assert!(validate_response(damaged.as_bytes(), &request).is_err());
    }
    assert_eq!(native.grant.remaining_queries(), 0);
    assert_eq!(native.grant.retained_bytes(), retained);
}
