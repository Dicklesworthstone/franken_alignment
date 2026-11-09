//! Original completed congress reports through the authenticated document client.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorReviewExplanationGrant;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::tower::publication::MAX_PUBLICATION_RESPONSE_BYTES;

const ROUND: u64 = 107;

struct NativeReview {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorReviewExplanationGrant,
    _storage: native::Directory,
}

impl NativeReview {
    fn new(queries: usize) -> Self {
        Self::with_policy(queries, native::profile().delivery.policy)
    }

    fn with_policy(queries: usize, policy: Policy) -> Self {
        let storage = native::Directory::new();
        let mut profile = native::profile();
        profile.delivery.policy = policy;
        let (mut host, _reviewer) = FileOversight::create(storage.store(), profile).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let proposal = native::proposal(&native::oversight::spec(&host, b"published"));
        let (actor, mut supervisor) = host.into_actor_gateway();
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        {
            let mut host = supervisor.host_mut().unwrap();
            let attempt = native::attempt(&host, 7);
            let action = host.request_action(7).unwrap().clone();
            let inputs = native::oversight::inputs(&action, b"PRIVATE-REVIEW-INPUT");
            native::oversight::review_existing(&mut host, attempt, ROUND, &inputs);
        }
        let (port, grant) = supervisor.review_explanation_port(queries).unwrap();
        let mut tower = PublicationTower::new(
            mount(),
            actor,
            PublicationLimits {
                max_response_bytes: MAX_PUBLICATION_RESPONSE_BYTES,
            },
        )
        .unwrap();
        tower.set_review_explanation(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            _storage: storage,
        }
    }

    fn exchange(&mut self, request: &TowerRequest) -> String {
        let before = self.supervisor.host().unwrap().inspect();
        let bytes = fs::read(self._storage.store().join("delivery.bin")).unwrap();
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: native::profile().delivery.scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            Vec::new(),
            0,
        );
        let response = self
            .tower
            .exchange(&access, encode_request(request).unwrap().as_bytes());
        assert_eq!(self.supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            fs::read(self._storage.store().join("delivery.bin")).unwrap(),
            bytes
        );
        response.to_json().unwrap()
    }
}

fn request(round: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "operator-review",
        Address::new("1", Noun::Attempt, "request-3-2-4-5-7", Some(GENERATION)).unwrap(),
        Verb::Explain,
    );
    request.arguments = ReadArguments::ReviewExplanation { round };
    request
}

fn settings(root: &Directory) -> PathBuf {
    let path = root.settings(credentials().pid(), 4);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":2", "\"explanation_queries\":0")
        .replace("\"explanation_replay_events\":1024", "\"explanation_replay_events\":0")
        .replace("policy_and_selected_witnesses", "none")
        .replacen('{', &format!(
            "{{\"response_bytes\":{MAX_PUBLICATION_RESPONSE_BYTES},\"review_explanation\":{{\"queries\":2,\"max_replay_events\":1024,\"disclosure\":\"policy_and_congress_trace\"}},"
        ), 1);
    let settings = OperatorSettings::decode(selected.as_bytes()).unwrap();
    assert_eq!(
        (settings.inspection_queries, settings.explanation_queries),
        (0, 0)
    );
    assert!(settings.policy_replay.is_none());
    assert_eq!(settings.review_explanation.unwrap().queries, 2);
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let observed = credentials();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, field| value.get(field).unwrap())
}

#[test]
fn genuine_completed_review_requires_its_exact_selected_round_scope_and_disclosure_shape() {
    let query = request(ROUND);
    let mut native = NativeReview::new(2);
    let original = native.exchange(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    assert_eq!(
        field(&parsed, &["result", "kind"]).as_str(),
        Some("review-explanation")
    );
    assert_eq!(
        field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(native.grant.remaining_queries(), 1);
    for arguments in [
        ReadArguments::None,
        ReadArguments::ReviewExplanation { round: ROUND + 1 },
    ] {
        let mut wrong = query.clone();
        wrong.arguments = arguments;
        assert!(validate_response(original.as_bytes(), &wrong).is_err());
    }
    for verb in [
        Verb::Get,
        Verb::WhyHeld,
        Verb::Situation,
        Verb::PolicyReplay,
    ] {
        let mut wrong = query.clone();
        wrong.verb = verb;
        assert!(validate_response(original.as_bytes(), &wrong).is_err());
    }
    for damaged in [
        original.replace(
            "fa.publication-review-explanation/1",
            "fa.publication-review-explanation/2",
        ),
        original.replace(
            "\"kind\":\"review-explanation\"",
            "\"kind\":\"explanation\"",
        ),
        original.replacen("\"round\":\"107\"", "\"round\":\"108\"", 1),
        original.replace("\"round\":\"107\"", "\"round\":\"0107\""),
        original.replace("\"request\":\"7\"", "\"request\":\"8\""),
        original.replace("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
        original.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        original.replace(
            "\"helper_inputs_disclosed\":false",
            "\"helper_inputs_disclosed\":true",
        ),
        original.replace("historical_completed_review", "current_review_authority"),
        original.replace(
            "\"current_eligibility\":\"not_observed\"",
            "\"current_eligibility\":\"allowed\"",
        ),
        original.replace("\"historical\":true", "\"historical\":true,\"extra\":null"),
    ] {
        assert_ne!(damaged, original);
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    let missing = request(ROUND + 1);
    let unknown = native.exchange(&missing);
    let parsed = parse_response(unknown.as_bytes(), &missing).unwrap();
    assert_eq!(
        field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert!(validate_response(unknown.as_bytes(), &query).is_err());
    assert_eq!(native.grant.remaining_queries(), 0);
    assert!(!original.contains("PRIVATE-REVIEW-INPUT"));
}

#[test]
fn authenticated_operator_client_preserves_known_unknown_and_exhausted_review_responses() {
    let root = Directory::new();
    let profile = settings(&root);
    let expected = [request(ROUND), request(ROUND + 1), request(ROUND)];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, request)| root.document(&format!("review-{index}.json"), request))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeReview::new(2);
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for (index, expected) in expected.into_iter().enumerate() {
            let request = decode_request(&read_document(&mut socket)).unwrap();
            assert_eq!(request, expected);
            let response = native.exchange(&request);
            let parsed = parse_response(response.as_bytes(), &request).unwrap();
            if index < 2 {
                assert_eq!(
                    field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
                    Some(if index == 0 { "known" } else { "unknown" })
                );
            } else {
                assert_eq!(
                    field(&parsed, &["result", "value", "error", "code"]).as_str(),
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
            "unexpected automatic review retry"
        );
        assert_eq!(native.grant.remaining_queries(), 0);
        replies
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn mismatched_round_stops_before_emitting_or_requesting_the_next_review() {
    let root = Directory::new();
    let profile = settings(&root);
    let first = request(ROUND);
    let documents = [
        root.document("first.json", &first),
        root.document("later.json", &request(ROUND + 1)),
    ];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeReview::new(2);
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), first);
        let original = native.exchange(&first);
        let damaged = original.replacen("\"round\":\"107\"", "\"round\":\"108\"", 1);
        assert_ne!(damaged, original);
        socket.write_all(damaged.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "mismatched review triggered a later request"
        );
        assert_eq!(native.grant.remaining_queries(), 1);
    });
    let mut output = Vec::new();
    let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
    assert!(error.contains("invalid or unbound Tower response"));
    assert!(output.is_empty());
    server.join().unwrap();
}

#[test]
fn actual_review_with_large_retained_policy_crosses_the_selected_complete_response_bound() {
    let root = Directory::new();
    let profile = settings(&root);
    let query = request(ROUND);
    let documents = [root.document("large.json", &query)];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let policy = Policy::new(
            1,
            vec![
                Predicate::PayloadIs(vec![0xaa; 20_000]),
                Predicate::PayloadAtMost(128),
                Predicate::Any(vec![0, 1]),
            ],
        )
        .unwrap();
        let mut native = NativeReview::with_policy(1, policy);
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
        let reply = native.exchange(&query);
        assert!(reply.len() > PublicationLimits::default().max_response_bytes);
        let parsed = parse_response(reply.as_bytes(), &query).unwrap();
        assert_eq!(
            field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
            Some("known")
        );
        let nodes = field(
            &parsed,
            &["result", "value", "knowledge", "value", "policy", "nodes"],
        )
        .as_array()
        .unwrap();
        assert_eq!(
            nodes[0].get("value_hex").and_then(Json::as_str),
            Some("aa".repeat(20_000).as_str())
        );
        socket.write_all(reply.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(socket.read(&mut [0_u8]).unwrap(), 0);
        assert_eq!(native.grant.remaining_queries(), 0);
        reply
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
}
