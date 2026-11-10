//! Original native residual refinement through the authenticated document client.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileSupervisorReviewExplanationGrant, MAX_REVIEW_EXPLANATION_EVENTS,
};
use fa_reference::tower::publication::MAX_PUBLICATION_RESPONSE_BYTES;

#[path = "../../../../../../../../tests/support/learned_refinement_explanation.rs"]
mod source;

const ROUND: u64 = source::REFINEMENT_ROUND;

struct NativeRefinement {
    tower: PublicationTower,
    source: source::Prepared,
    grant: FileSupervisorReviewExplanationGrant,
    storage: source::Directory,
}

impl NativeRefinement {
    fn new(queries: usize) -> Self {
        let storage = source::Directory::new();
        let source = source::prepared(&storage);
        let (port, grant) = source
            .supervisor
            .review_explanation_port_with_numerical_budget(
                queries,
                MAX_REVIEW_EXPLANATION_EVENTS,
                queries * MAX_REVIEW_EXPLANATION_EVENTS,
            )
            .unwrap();
        let mut tower = PublicationTower::new_learned_observer(
            mount(),
            source.actor.clone(),
            PublicationLimits {
                max_response_bytes: MAX_PUBLICATION_RESPONSE_BYTES,
            },
        )
        .unwrap();
        tower.set_review_explanation(Some(port)).unwrap();
        Self {
            tower,
            source,
            grant,
            storage,
        }
    }

    fn exchange(&mut self, request: &TowerRequest) -> String {
        let before = self.source.supervisor.host().unwrap().inspect();
        let numerical = self
            .source
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap();
        let bytes = source::image(&self.storage);
        let access = TowerAccess::new(CallerScope::full_tenant_access("1"), true, Vec::new(), 0);
        let response = self
            .tower
            .exchange(&access, encode_request(request).unwrap().as_bytes())
            .to_json()
            .unwrap();
        assert_eq!(self.source.supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            self.source
                .supervisor
                .host()
                .unwrap()
                .learned_generation_inspection()
                .unwrap(),
            numerical
        );
        assert_eq!(source::image(&self.storage), bytes);
        response
    }

    fn publish(&mut self) {
        source::publish(&mut self.source);
    }
}

fn request(round: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "operator-refinement",
        Address::new(
            "1",
            Noun::Attempt,
            format!("request-3-2-4-5-{}", source::REQUEST),
            Some(GENERATION),
        )
        .unwrap(),
        Verb::Explain,
    );
    request.arguments = ReadArguments::ReviewExplanation { round };
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
            &format!(
                "{{\"response_bytes\":{MAX_PUBLICATION_RESPONSE_BYTES},\"review_explanation\":{{\"queries\":4,\"max_replay_events\":{MAX_REVIEW_EXPLANATION_EVENTS},\"numerical_replay_events\":{},\"disclosure\":\"policy_and_congress_trace\"}},",
                4 * MAX_REVIEW_EXPLANATION_EVENTS,
            ),
            1,
        );
    OperatorSettings::decode(selected.as_bytes()).unwrap();
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

fn body(value: &Json) -> &Json {
    field(value, &["result", "value", "knowledge", "value"])
}

fn replace(original: &str, from: &str, to: &str) -> String {
    assert!(original.contains(from), "missing mutation target: {from}");
    assert_ne!(from, to);
    original.replacen(from, to, 1)
}

#[test]
fn genuine_refinement_requires_exact_input_lineage_and_observes_later_independent_publication() {
    let query = request(ROUND);
    let mut native = NativeRefinement::new(3);
    let original = native.exchange(&query);
    let parsed = parse_response(original.as_bytes(), &query).unwrap();
    assert_eq!(
        field(&parsed, &["result", "kind"]).as_str(),
        Some("refinement-explanation")
    );
    assert_eq!(
        field(body(&parsed), &["application", "state"]).as_str(),
        Some("not_attempted")
    );
    assert_eq!(
        body(&parsed).get("input_revision").and_then(Json::as_str),
        Some("1")
    );
    assert_eq!(
        field(body(&parsed), &["refinement", "refined_input_revision"]).as_str(),
        Some("2")
    );
    assert_eq!(
        field(body(&parsed), &["effect", "dispatch", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(
        native
            .source
            .supervisor
            .host()
            .unwrap()
            .inspect()
            .executions,
        0
    );
    for arguments in [
        ReadArguments::None,
        ReadArguments::ReviewExplanation { round: 0 },
        ReadArguments::ReviewExplanation { round: ROUND + 1 },
    ] {
        let mut wrong = query.clone();
        wrong.arguments = arguments;
        assert!(validate_response(original.as_bytes(), &wrong).is_err());
    }
    let mut wrong_verb = query.clone();
    wrong_verb.verb = Verb::Get;
    assert!(validate_response(original.as_bytes(), &wrong_verb).is_err());
    for (from, to) in [
        (
            "fa.publication-refinement-explanation/1",
            "fa.publication-refinement-explanation/2",
        ),
        (
            "\"kind\":\"refinement-explanation\"",
            "\"kind\":\"review-explanation\"",
        ),
        (
            "historical_completed_refinement",
            "current_refinement_authority",
        ),
        ("\"round\":\"101\"", "\"round\":\"102\""),
        ("\"request\":\"701\"", "\"request\":\"702\""),
        ("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        (
            "\"helper_inputs_disclosed\":false",
            "\"helper_inputs_disclosed\":true",
        ),
        (
            "\"witness_values_disclosed\":false",
            "\"witness_values_disclosed\":true",
        ),
        (
            "\"live_evidence_read\":false",
            "\"live_evidence_read\":true",
        ),
        (
            "\"current_eligibility\":\"not_observed\"",
            "\"current_eligibility\":\"allowed\"",
        ),
        ("\"historical\":true", "\"historical\":true,\"extra\":null"),
        (
            "\"state\":\"known\",\"basis\"",
            "\"state\":\"unknown\",\"basis\"",
        ),
        (
            "\"application\":{\"state\":\"not_attempted\"}",
            "\"application\":{\"state\":\"applied\"}",
        ),
        (
            "\"application\":{\"state\":\"not_attempted\"}",
            "\"application\":{\"state\":\"not_attempted\",\"receipt\":{}}",
        ),
        (
            "\"kind\":\"kv_exact_residual\"",
            "\"kind\":\"automatic_approval\"",
        ),
        (
            "\"kind\":\"kv_exact_residual\"",
            "\"kind\":\"kv_exact_residual\",\"extra\":null",
        ),
        (
            "\"requires_fresh_review\":true",
            "\"requires_fresh_review\":false",
        ),
        (
            "\"requires_fresh_review\":true",
            "\"requires_fresh_review\":\"true\"",
        ),
        (
            "\"reviewed_input_revision\":\"1\"",
            "\"reviewed_input_revision\":\"2\"",
        ),
        (
            "\"refined_input_revision\":\"2\"",
            "\"refined_input_revision\":\"1\"",
        ),
        (
            "\"refined_input_revision\":\"2\"",
            "\"refined_input_revision\":\"0\"",
        ),
        (
            "\"refined_input_revision\":\"2\"",
            "\"refined_input_revision\":\"02\"",
        ),
        ("\"side\":\"value\"", "\"side\":\"values\""),
        ("\"side\":\"value\"", "\"side\":null"),
        ("\"layer\":\"1\"", "\"layer\":\"01\""),
        ("\"position\":\"3\"", "\"position\":\"-1\""),
        ("\"head\":\"0\"", "\"head\":0"),
        ("\"head\":\"0\"", "\"head\":\"18446744073709551616\""),
        ("\"head\":\"0\"", "\"head\":\"0\",\"extra\":null"),
        (
            "\"scope\":\"original_reference_publication_sink\"",
            "\"scope\":\"current_effect_authority\"",
        ),
        (
            "\"dispatch\":{\"state\":\"unknown\",\"reason\":\"not_observed\"}",
            "\"dispatch\":{\"state\":\"unknown\",\"reason\":\"permitted\"}",
        ),
    ] {
        let damaged = replace(&original, from, to);
        assert!(
            validate_response(damaged.as_bytes(), &query).is_err(),
            "{from} -> {to}"
        );
    }

    // The observed update supplied neither key. The original independent
    // human and the fresh Allow congress still drive the actual publication.
    native.publish();
    let later = native.exchange(&query);
    let parsed = parse_response(later.as_bytes(), &query).unwrap();
    let dispatch = field(body(&parsed), &["effect", "dispatch", "value"]);
    assert_eq!(
        dispatch.get("selected_review").and_then(Json::as_bool),
        Some(false)
    );
    assert_eq!(
        field(dispatch, &["automatic", "round"]).as_str(),
        Some("102")
    );
    assert_eq!(
        field(dispatch, &["automatic", "input_revision"]).as_str(),
        Some("2")
    );
    assert_eq!(
        field(
            body(&parsed),
            &["effect", "outcome", "value", "outcome", "kind"]
        )
        .as_str(),
        Some("executed")
    );
    assert_eq!(
        field(body(&parsed), &["application", "state"]).as_str(),
        Some("not_attempted")
    );
    for (from, to) in [
        ("\"selected_review\":false", "\"selected_review\":true"),
        (
            "\"automatic\":{\"round\":\"102\"",
            "\"automatic\":{\"round\":\"101\"",
        ),
        (
            "\"automatic\":{\"round\":\"102\"",
            "\"automatic\":{\"round\":\"0102\"",
        ),
        (
            "\"automatic\":{\"round\":\"102\"",
            "\"automatic\":{\"round\":\"0\"",
        ),
        ("\"input_revision\":\"2\"", "\"input_revision\":\"1\""),
        (
            "\"disposition\":\"consumed\"",
            "\"disposition\":\"approved\"",
        ),
    ] {
        let damaged = replace(&later, from, to);
        assert!(
            validate_response(damaged.as_bytes(), &query).is_err(),
            "{from} -> {to}"
        );
    }
    let missing = request(999);
    let unknown = native.exchange(&missing);
    let parsed = parse_response(unknown.as_bytes(), &missing).unwrap();
    assert_eq!(
        field(&parsed, &["result", "kind"]).as_str(),
        Some("review-explanation")
    );
    assert_eq!(
        field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    let relabeled = unknown
        .replace(
            "\"kind\":\"review-explanation\"",
            "\"kind\":\"refinement-explanation\"",
        )
        .replace(
            "fa.publication-review-explanation/1",
            "fa.publication-refinement-explanation/1",
        )
        .replace(
            "historical_completed_review",
            "historical_completed_refinement",
        );
    assert!(validate_response(relabeled.as_bytes(), &missing).is_err());
    assert_eq!(native.grant.remaining_queries(), 0);
}

#[test]
fn authenticated_client_preserves_original_refinement_applied_unknown_and_exhausted_responses() {
    let root = Directory::new();
    let profile = settings(&root);
    let expected = [
        request(ROUND),
        request(ROUND),
        request(source::APPLIED_ROUND),
        request(999),
        request(ROUND),
    ];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, request)| root.document(&format!("refinement-{index}.json"), request))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeRefinement::new(4);
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for (index, expected) in expected.into_iter().enumerate() {
            let request = decode_request(&read_document(&mut socket)).unwrap();
            assert_eq!(request, expected);
            if index == 1 {
                native.publish();
            }
            let response = native.exchange(&request);
            let parsed = parse_response(response.as_bytes(), &request).unwrap();
            assert_eq!(
                field(&parsed, &["result", "kind"]).as_str(),
                Some(if index < 2 {
                    "refinement-explanation"
                } else {
                    "review-explanation"
                })
            );
            if index < 3 {
                assert_eq!(
                    field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
                    Some("known")
                );
            } else if index == 3 {
                assert_eq!(
                    field(&parsed, &["result", "value", "knowledge", "state"]).as_str(),
                    Some("unknown")
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
            "unexpected automatic refinement retry"
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
fn attributed_refinement_dispatch_stops_before_emitting_or_sending_the_next_document() {
    let root = Directory::new();
    let profile = settings(&root);
    let first = request(ROUND);
    let documents = [
        root.document("first.json", &first),
        root.document("later.json", &request(source::APPLIED_ROUND)),
    ];
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeRefinement::new(2);
        native.publish();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), first);
        let original = native.exchange(&first);
        let damaged = replace(
            &original,
            "\"selected_review\":false",
            "\"selected_review\":true",
        );
        socket.write_all(damaged.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.flush().unwrap();
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "invalid attribution sent a later request"
        );
        assert_eq!(native.grant.remaining_queries(), 1);
    });
    let mut output = Vec::new();
    let error = operator_command(&args(&profile, &documents), &mut output).unwrap_err();
    assert!(error.contains("invalid or unbound Tower response"));
    assert!(output.is_empty());
    server.join().unwrap();
}
