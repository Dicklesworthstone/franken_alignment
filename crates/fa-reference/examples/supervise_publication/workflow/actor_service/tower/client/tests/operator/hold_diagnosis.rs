//! Original recorded conditions through the authenticated operator client.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorHoldDiagnosisGrant;

struct NativeHold {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorHoldDiagnosisGrant,
    _storage: native::Directory,
}

impl NativeHold {
    fn new(queries: usize) -> Self {
        let storage = native::Directory::new();
        let (actor, mut supervisor, _reviewer, proposal) = native::create(&storage);
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        let (port, grant) = supervisor.hold_diagnosis_port(queries).unwrap();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_hold_diagnosis(Some(port)).unwrap();
        Self { tower, supervisor, grant, _storage: storage }
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
        let response = self.tower.exchange(&access, encode_request(request).unwrap().as_bytes());
        assert_eq!(self.supervisor.host().unwrap().inspect(), before);
        assert_eq!(fs::read(self._storage.store().join("delivery.bin")).unwrap(), bytes);
        response.to_json().unwrap()
    }
}

fn request(key: u64) -> TowerRequest {
    TowerRequest::new(
        "operator-hold",
        Address::new("1", Noun::Attempt, format!("request-3-2-4-5-{key}"), Some(GENERATION))
            .unwrap(),
        Verb::WhyHeld,
    )
}

fn settings(root: &Directory) -> PathBuf {
    let path = root.settings(credentials().pid(), 4);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":2", "\"explanation_queries\":0")
        .replace("\"explanation_replay_events\":1024", "\"explanation_replay_events\":0")
        .replace("policy_and_selected_witnesses", "none")
        .replacen(
            '{',
            "{\"hold_diagnosis\":{\"queries\":2,\"disclosure\":\"recorded_publication_state\"},",
            1,
        );
    let settings = OperatorSettings::decode(selected.as_bytes()).unwrap();
    assert_eq!((settings.inspection_queries, settings.explanation_queries), (0, 0));
    assert!(settings.policy_replay.is_none());
    assert!(settings.review_explanation.is_none());
    assert_eq!(settings.hold_diagnosis.unwrap().queries, 2);
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
    path.iter().fold(value, |value, key| value.get(key).unwrap())
}

#[test]
fn genuine_hold_reply_requires_exact_request_registration_basis_and_unknown_live_conditions() {
    let query = request(7);
    let mut native = NativeHold::new(2);
    let original = native.exchange(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    assert_eq!(field(&parsed, &["result", "kind"]).as_str(), Some("hold-diagnosis"));
    assert_eq!(
        field(&parsed, &["result", "value", "knowledge", "basis", "request_generation"])
            .as_str(),
        Some("0")
    );
    assert_eq!(native.grant.remaining_queries(), 1);
    for verb in [Verb::Explain, Verb::Get, Verb::Next, Verb::Situation] {
        let mut wrong = query.clone();
        wrong.verb = verb;
        assert!(validate_response(original.as_bytes(), &wrong).is_err());
    }
    let mut wrong = query.clone();
    wrong.arguments = ReadArguments::Next { max_cost: 1 };
    assert!(validate_response(original.as_bytes(), &wrong).is_err());
    for damaged in [
        original.replace("fa.publication-hold-diagnosis/1", "fa.publication-hold-diagnosis/2"),
        original.replace("\"kind\":\"hold-diagnosis\"", "\"kind\":\"explanation\""),
        original.replace("\"request\":\"7\"", "\"request\":\"8\""),
        original.replace("\"request\":\"7\"", "\"request\":\"07\""),
        original.replace("\"principal\":\"2\"", "\"principal\":\"22\""),
        original.replace("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
        original.replacen("\"request_generation\":\"0\"", "\"request_generation\":\"1\"", 1),
        original.replace(
            &format!("\"registration\":\"{}\"", mount().canonical_string()),
            &format!("\"registration\":\"fa://1/run/3@{}\"", GENERATION + 1),
        ),
        original.replace("\"effect_authority\":false", "\"effect_authority\":true"),
        original.replace("\"clock_read\":false", "\"clock_read\":true"),
        original.replace("\"live_evidence_read\":false", "\"live_evidence_read\":true"),
        original.replace("\"current_eligibility\":\"not_observed\"", "\"current_eligibility\":\"known\""),
        original.replace("recorded_conditions_only", "complete_dispatch_predicates"),
        original.replace("\"condition\":\"final_publication_eligibility\"", "\"condition\":\"current_clock\""),
        original.replace(
            "\"state\":\"unknown\",\"reason\":\"controller_unavailable\"",
            "\"state\":\"known\",\"value\":true",
        ),
    ] {
        assert_ne!(damaged, original, "fixture mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    // An omitted registration selector remains a valid explicit read; its reply
    // still names the original nonzero mount, never a native clock coordinate.
    let mut current_mount = query;
    current_mount.address.generation = None;
    let current_mount_reply = native.exchange(&current_mount);
    validate_response(current_mount_reply.as_bytes(), &current_mount).unwrap();
    assert_eq!(native.grant.remaining_queries(), 0);
}

#[test]
fn authenticated_client_preserves_native_known_unknown_and_exhausted_diagnoses() {
    let root = Directory::new();
    let profile = settings(&root);
    let expected = [request(7), request(999), request(7)];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, request)| root.document(&format!("hold-{index}.json"), request))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeHold::new(2);
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
        assert_eq!(socket.read(&mut [0_u8]).unwrap(), 0, "unexpected automatic hold retry");
        assert_eq!(native.grant.remaining_queries(), 0);
        replies
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(output, (server.join().unwrap().join("\n") + "\n").as_bytes());
}

#[test]
fn changed_native_request_stops_before_emitting_or_sending_the_next_explicit_diagnosis() {
    let root = Directory::new();
    let profile = settings(&root);
    let first = request(7);
    let documents = [
        root.document("first.json", &first),
        root.document("later.json", &request(999)),
    ];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeHold::new(2);
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), first);
        let original = native.exchange(&first);
        let damaged = original.replacen("\"request\":\"7\"", "\"request\":\"8\"", 1);
        assert_ne!(damaged, original);
        socket.write_all(damaged.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "unbound diagnosis caused another request"
        );
        assert_eq!(native.grant.remaining_queries(), 1);
    });
    let mut output = Vec::new();
    let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
    assert!(error.contains("invalid or unbound Tower response"));
    assert!(output.is_empty());
    server.join().unwrap();
}
