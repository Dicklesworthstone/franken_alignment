//! Genuine original publication relationships through the explicit document
//! client, with causally altered replies stopped before output or a later send.

use super::super::super::request_history as validation;
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileRequestHistoryDisclosure, FileSupervisorRequestHistoryGrant, MAX_REQUEST_HISTORY_EVENTS,
    MAX_REQUEST_HISTORY_REVIEWS,
};

struct NativeHistory {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorRequestHistoryGrant,
    _storage: native::Directory,
}

impl NativeHistory {
    fn new(queries: usize) -> Self {
        let storage = native::Directory::new();
        let (actor, mut supervisor, reviewer, proposal) = native::create(&storage);
        native::observe(&mut supervisor);
        actor.submit(7, &proposal).unwrap();
        let keys = native::ready(&mut supervisor, &reviewer, 7);
        {
            let mut host = supervisor.host_mut().unwrap();
            native::oversight::dispatch(&mut host, &keys);
            let revision = host.revision();
            host.publish(revision, keys.automatic.attempt()).unwrap();
            let revision = host.revision();
            host.reconcile(revision, keys.automatic.attempt()).unwrap();
            assert_eq!(host.inspect().executions, 1);
        }
        let (port, grant) = supervisor
            .request_history_port(
                queries,
                MAX_REQUEST_HISTORY_EVENTS,
                MAX_REQUEST_HISTORY_REVIEWS,
                FileRequestHistoryDisclosure::RecordedRequestRelationships,
            )
            .unwrap();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_request_history(Some(port)).unwrap();
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

fn request(key: u64) -> TowerRequest {
    TowerRequest::new(
        "operator-history",
        Address::new(
            "1",
            Noun::Attempt,
            format!("request-3-2-4-5-{key}"),
            Some(GENERATION),
        )
        .unwrap(),
        Verb::Related,
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
        .replacen('{', "{\"request_history\":{\"queries\":2,\"max_history_events\":4096,\"max_reviews\":2048,\"disclosure\":\"recorded_request_relationships\"},", 1);
    let decoded = OperatorSettings::decode(selected.as_bytes()).unwrap();
    assert_eq!(
        (decoded.inspection_queries, decoded.explanation_queries),
        (0, 0)
    );
    assert!(decoded.journal_tail.is_none());
    assert!(decoded.review_explanation.is_none());
    assert!(decoded.resolution.is_none());
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
        .fold(value, |value, key| value.get(key).unwrap())
}

#[test]
fn original_request_relationships_require_sealed_identity_and_exact_explain_links() {
    let query = request(7);
    let mut native = NativeHistory::new(2);
    let original = native.exchange(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    let value = field(&parsed, &["result", "value"]);
    let rows = field(value, &["knowledge", "value", "reviews"])
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], &["round"]).as_str(), Some("107"));
    assert_eq!(
        field(value, &["knowledge", "value", "stage"]).as_str(),
        Some("confirmed")
    );
    validation::check_profile(
        &parsed,
        &query,
        native::profile().delivery.scope,
        Some((MAX_REQUEST_HISTORY_EVENTS, MAX_REQUEST_HISTORY_REVIEWS)),
    )
    .unwrap();
    let mut foreign = native::profile().delivery.scope;
    foreign.principal += 1;
    assert!(validation::check_profile(&parsed, &query, foreign, None).is_err());
    assert!(
        validation::check_profile(
            &parsed,
            &query,
            native::profile().delivery.scope,
            Some((1, 1))
        )
        .is_err()
    );
    for verb in [Verb::Explain, Verb::Get, Verb::WhyHeld, Verb::Situation] {
        let mut wrong = query.clone();
        wrong.verb = verb;
        assert!(validate_response(original.as_bytes(), &wrong).is_err());
    }
    let begin = field(&rows[0], &["begin_revision"]).as_str().unwrap();
    let completed = field(&rows[0], &["completed_revision"]).as_str().unwrap();
    for damaged in [
        original.replace(
            "fa.publication-request-history/1",
            "fa.publication-request-history/2",
        ),
        original.replace(
            "recorded_request_relationships",
            "policy_and_congress_trace",
        ),
        original.replace("\"historical\":true", "\"historical\":false"),
        original.replace(
            "\"enumeration_complete\":true",
            "\"enumeration_complete\":false",
        ),
        original.replace("\"request\":\"7\"", "\"request\":\"8\""),
        original.replace("\"principal\":\"2\"", "\"principal\":\"22\""),
        original.replace("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
        original.replace(
            "\"current_eligibility\":\"not_observed\"",
            "\"current_eligibility\":\"allowed\"",
        ),
        original.replace("\"admitted\":true", "\"admitted\":false"),
        original.replace("\"round\":\"107\"", "\"round\":\"0107\""),
        original.replace(
            &format!("\"begin_revision\":\"{begin}\""),
            &format!("\"begin_revision\":\"{completed}\""),
        ),
        original.replace("application_attempted", "applied_permission"),
        original.replace("related-explain-107", "related-explain-108"),
        original.replace("\"verb\":\"explain\"", "\"verb\":\"propose\""),
        original.replace(
            &format!("\"registration\":\"fa://1/run/3@{GENERATION}\""),
            "\"registration\":\"fa://1/run/3@999\"",
        ),
        original.replace(
            "\"query_address\":",
            "\"payload\":\"private\",\"query_address\":",
        ),
    ] {
        assert_ne!(damaged, original, "response mutation missed its target");
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    // The JSON shape itself cannot smuggle another link for the same round.
    let mut duplicate = value.clone();
    if let Json::Object(wrapper) = &mut duplicate {
        if let Some(Json::Object(knowledge)) = wrapper.get_mut("knowledge") {
            if let Some(Json::Object(report)) = knowledge.get_mut("value") {
                report.insert(
                    "reviews".into(),
                    Json::Array(vec![rows[0].clone(), rows[0].clone()]),
                );
            }
        }
    }
    assert!(!validation::matches(&duplicate, &query));
    let mut unqualified = query;
    unqualified.address.generation = None;
    let current = native.exchange(&unqualified);
    validate_response(current.as_bytes(), &unqualified).unwrap();
    assert!(current.contains(&format!("request-3-2-4-5-7@{GENERATION}")));
    assert_eq!(native.grant.remaining_queries(), 0);
}

#[test]
fn authenticated_client_preserves_known_unknown_and_exhausted_history_without_following_links() {
    let root = Directory::new();
    let profile = settings(&root);
    let expected = [request(7), request(999), request(7)];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, request)| root.document(&format!("history-{index}.json"), request))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeHistory::new(2);
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
            "client followed a review link without an explicit document"
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
fn altered_history_link_stops_before_output_and_the_next_explicit_request() {
    let root = Directory::new();
    let profile = settings(&root);
    let first = request(7);
    let documents = [
        root.document("first.json", &first),
        root.document("later.json", &request(999)),
    ];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeHistory::new(2);
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), first);
        let original = native.exchange(&first);
        let damaged = original.replace("related-explain-107", "related-explain-108");
        assert_ne!(damaged, original);
        socket.write_all(damaged.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "invalid history caused later work"
        );
        assert_eq!(native.grant.remaining_queries(), 1);
    });
    let mut output = Vec::new();
    let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
    assert!(error.contains("invalid or unbound Tower response"));
    assert!(output.is_empty());
    server.join().unwrap();
}
