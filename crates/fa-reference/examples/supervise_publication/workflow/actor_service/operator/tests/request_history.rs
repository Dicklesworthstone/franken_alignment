//! Direct original request history, independently granted Explain and actual
//! authenticated reconnects. No Tail disclosure or numerical replay is selected.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_REQUEST_HISTORY_EVENTS, MAX_REQUEST_HISTORY_QUERIES, MAX_REQUEST_HISTORY_REVIEWS,
};

#[path = "../../../../../../tests/support/file_oversight_actor.rs"]
mod native;

fn selection(queries: usize, events: usize, reviews: usize) -> String {
    format!(
        r#"{{"queries":{queries},"max_history_events":{events},"max_reviews":{reviews},"disclosure":"recorded_request_relationships"}}"#
    )
}

fn with_history(profile: &str, selection: &str) -> String {
    profile.replacen('{', &format!("{{\"request_history\":{selection},"), 1)
}

fn fixture(queries: usize, events: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, 0, |config| {
        let clock_domain = config.profile.delivery.clock_domain;
        config.profile = native::profile();
        config.profile.delivery.clock_domain = clock_domain;
    });
    fixture.settings = Settings::decode(
        with_history(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            &selection(queries, events, MAX_REQUEST_HISTORY_REVIEWS),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture
}

fn complete(fixture: &mut Fixture) {
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let attempt = native::attempt(&host, fixture.actor.request);
    let action = host.request_action(fixture.actor.request).unwrap().clone();
    let inputs = native::oversight::inputs(&action, b"PRIVATE-ORIGINAL-HELPER-INPUT");
    native::oversight::review_existing(&mut host, attempt, 107, &inputs);
}

fn request(fixture: &Fixture) -> TowerRequest {
    let mut request = fixture.request(Verb::Explain);
    request.verb = Verb::Related;
    request
}

fn remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .request_history
        .as_ref()
        .unwrap()
        .remaining_queries()
}

fn report(response: &Json) -> &Json {
    known(response, "request-history");
    field(response, &["result", "value", "knowledge", "value"])
}

#[test]
fn request_history_profile_requires_finite_explicit_relationship_disclosure() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let path = Path::new("/operator/history.sock");
    let original = profile_json(path, identity, 1, 1);
    for compatible in [original.clone(), with_history(&original, "null")] {
        let decoded = Settings::decode(compatible.as_bytes()).unwrap();
        assert!(decoded.request_history.is_none());
        assert_eq!(
            (decoded.inspection_queries, decoded.explanation_queries),
            (1, 1)
        );
    }
    let largest = selection(
        MAX_REQUEST_HISTORY_QUERIES,
        MAX_REQUEST_HISTORY_EVENTS,
        MAX_REQUEST_HISTORY_REVIEWS,
    );
    let decoded =
        Settings::decode(with_history(&profile_json(path, identity, 0, 0), &largest).as_bytes())
            .unwrap();
    decoded.check_learned_operator().unwrap();
    assert_eq!(
        (decoded.inspection_queries, decoded.explanation_queries),
        (0, 0)
    );
    assert!(decoded.journal_tail.is_none());
    assert!(decoded.review_explanation.is_none());
    assert!(decoded.resolution.is_none());
    assert!(decoded.fence.is_none());
    let selected = decoded.request_history.unwrap();
    assert_eq!(selected.queries, MAX_REQUEST_HISTORY_QUERIES);
    assert_eq!(selected.max_history_events, MAX_REQUEST_HISTORY_EVENTS);
    assert_eq!(selected.max_reviews, MAX_REQUEST_HISTORY_REVIEWS);
    for invalid in [
        "[]".to_owned(),
        "false".into(),
        "{}".into(),
        selection(0, 1, 1),
        selection(1, 0, 1),
        selection(1, 1, 0),
        selection(MAX_REQUEST_HISTORY_QUERIES + 1, 1, 1),
        selection(1, MAX_REQUEST_HISTORY_EVENTS + 1, 1),
        selection(1, 1, MAX_REQUEST_HISTORY_REVIEWS + 1),
        selection(1, 1, 1).replace("recorded_request_relationships", "none"),
        selection(1, 1, 1).replace(
            "recorded_request_relationships",
            "policy_and_congress_trace",
        ),
        selection(1, 1, 1).replace("\"queries\":1", "\"queries\":\"1\""),
        selection(1, 1, 1).replace("\"max_reviews\":1", "\"max_reviews\":1.5"),
        selection(1, 1, 1).replacen('{', "{\"queries\":1,", 1),
        selection(1, 1, 1).replacen('{', "{\"numerical_replay_events\":1,", 1),
        selection(1, 1, 1).replacen('{', "{\"effect_authority\":true,", 1),
    ] {
        assert!(
            Settings::decode(with_history(&original, &invalid).as_bytes()).is_err(),
            "accepted request history {invalid}"
        );
    }
    assert!(
        Settings::decode(with_history(&profile_json(path, identity, 0, 0), "null").as_bytes())
            .is_err()
    );
}

#[test]
fn original_request_history_survives_reconnects_and_discovers_independently_explained_review() {
    let mut fixture = fixture(2, MAX_REQUEST_HISTORY_EVENTS);
    let with_explain = with_history(
        &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
        &selection(2, MAX_REQUEST_HISTORY_EVENTS, MAX_REQUEST_HISTORY_REVIEWS),
    ).replacen('{', "{\"review_explanation\":{\"queries\":1,\"max_replay_events\":4096,\"disclosure\":\"policy_and_congress_trace\"},", 1);
    fixture.settings = Settings::decode(with_explain.as_bytes()).unwrap();
    fixture.settings.connection_limit = 8;
    fixture.submit(7);
    fixture.snapshot();
    let initial = fixture.driver.supervisor().host().unwrap().inspect();
    let initial_files = fixture.files();
    let mut service = fixture.start();
    let first = fixture.query(&mut service, request(&fixture));
    assert!(
        field(report(&first), &["reviews"])
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(field(report(&first), &["admitted"]).as_bool(), Some(true));
    assert_eq!(
        field(report(&first), &["enumeration_complete"]).as_bool(),
        Some(true)
    );
    assert_eq!(remaining(&service), 1);
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        initial
    );
    assert_eq!(fixture.files(), initial_files);
    complete(&mut fixture);
    let completed = fixture.driver.supervisor().host().unwrap().inspect();
    let completed_files = fixture.files();
    let response = fixture.query(&mut service, request(&fixture));
    let rows = field(report(&response), &["reviews"]).as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], &["round"]).as_str(), Some("107"));
    assert_eq!(
        field(&rows[0], &["completion_kind"]).as_str(),
        Some("application_attempted")
    );
    assert_eq!(remaining(&service), 0);
    let mut explain = fixture.request(Verb::Explain);
    explain.operation_id = "related-explain-107".into();
    explain.arguments = ReadArguments::ReviewExplanation { round: 107 };
    let encoded = encode_request(&explain).unwrap();
    assert_eq!(
        rows[0].get("explain_request"),
        Some(&strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap())
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .review_explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        1
    );
    let exhausted = fixture.query(&mut service, request(&fixture));
    assert_eq!(
        field(&exhausted, &["result", "value", "error", "code"]).as_str(),
        Some("capacity")
    );
    let explained = fixture.query(&mut service, explain);
    known(&explained, "review-explanation");
    for coordinate in ["begin_revision", "completed_revision"] {
        assert_eq!(
            rows[0].get(coordinate),
            field(&explained, &["result", "value", "knowledge", "basis"]).get(coordinate)
        );
    }
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .review_explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        0
    );
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        completed
    );
    assert_eq!(fixture.files(), completed_files);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    assert!(!format!("{response:?}").contains("PRIVATE-ORIGINAL-HELPER-INPUT"));
    assert!(service.active.as_ref().unwrap().journal_tail.is_none());
    assert!(service.active.as_ref().unwrap().resolution.is_none());
    service.retire();
    assert!(service.active.is_none());
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn request_history_requires_its_own_grant_and_never_grants_admission_or_partial_history() {
    let mut ordinary = Fixture::new(1, 1);
    ordinary.submit(7);
    let mut service = ordinary.start();
    let refused = ordinary.query(&mut service, request(&ordinary));
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .inspection
            .as_ref()
            .unwrap()
            .remaining_queries(),
        1
    );
    service.retire();

    let mut fixture = fixture(1, 1);
    fixture.submit(7);
    complete(&mut fixture);
    fixture.snapshot();
    let original = fixture.driver.supervisor().host().unwrap().inspect();
    let files = fixture.files();
    let mut service = fixture.start();
    let mut propose = fixture.request(Verb::Propose);
    propose.arguments = ReadArguments::Propose {
        request: 8,
        proposal: fixture.proposal(),
    };
    propose.idempotency_key = Some("8".into());
    let refused = fixture.query(&mut service, propose);
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("PurposeMismatch")
    );
    assert_eq!(remaining(&service), 1);
    let short = fixture.query(&mut service, request(&fixture));
    assert_eq!(
        field(&short, &["envelope", "reason_code"]).as_str(),
        Some("BudgetExhausted")
    );
    assert_eq!(
        field(&short, &["result", "value", "error", "code"]).as_str(),
        Some("capacity")
    );
    assert!(
        field(&short, &["result", "value"])
            .get("knowledge")
            .is_none()
    );
    assert_eq!(remaining(&service), 0);
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        original
    );
    assert_eq!(fixture.files(), files);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 8 }
    );
}
