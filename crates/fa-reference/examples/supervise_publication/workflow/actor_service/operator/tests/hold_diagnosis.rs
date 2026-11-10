//! Recorded publication conditions through the original authenticated operator.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_HOLD_DIAGNOSIS_QUERIES;

fn with_hold(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"hold_diagnosis\":{selected},"), 1)
}

fn selection(queries: usize) -> String {
    format!(r#"{{"queries":{queries},"disclosure":"recorded_publication_state"}}"#)
}

fn fixture(queries: usize, inspection: usize, explanation: usize) -> Fixture {
    let mut fixture = Fixture::new(1, explanation);
    fixture.settings = Settings::decode(
        with_hold(
            &profile_json(
                &fixture.settings.socket,
                fixture.settings.operator,
                inspection,
                explanation,
            ),
            &selection(queries),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture
}

fn request(fixture: &Fixture) -> TowerRequest {
    let mut request = fixture.request(Verb::Explain);
    request.verb = Verb::WhyHeld;
    request
}

fn remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .hold_diagnosis
        .as_ref()
        .unwrap()
        .remaining_queries()
}

fn predicate<'a>(response: &'a Json, name: &str) -> &'a Json {
    field(response, &["result", "value", "knowledge", "value", "predicates"])
        .as_array()
        .unwrap()
        .iter()
        .find(|predicate| predicate.get("condition").and_then(Json::as_str) == Some(name))
        .unwrap()
        .get("satisfied")
        .unwrap()
}

#[test]
fn hold_profile_requires_explicit_bounded_disclosure_without_upgrading_old_profiles() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let path = Path::new("/operator/hold.sock");
    let original = profile_json(path, identity, 2, 2);
    for profile in [original.clone(), with_hold(&original, "null")] {
        let decoded = Settings::decode(profile.as_bytes()).unwrap();
        assert!(decoded.hold_diagnosis.is_none());
        assert_eq!((decoded.inspection_queries, decoded.explanation_queries), (2, 2));
    }
    let hold_only = with_hold(
        &profile_json(path, identity, 0, 0),
        &selection(MAX_HOLD_DIAGNOSIS_QUERIES),
    );
    let decoded = Settings::decode(hold_only.as_bytes()).unwrap();
    assert_eq!((decoded.inspection_queries, decoded.explanation_queries), (0, 0));
    assert!(decoded.review_explanation.is_none());
    assert!(decoded.policy_replay.is_none());
    let granted = decoded.hold_diagnosis.unwrap();
    assert_eq!(granted.queries, MAX_HOLD_DIAGNOSIS_QUERIES);
    assert_eq!(granted.disclosure, "recorded_publication_state");
    for invalid in [
        "false".to_owned(),
        "[]".to_owned(),
        "{}".to_owned(),
        selection(0),
        selection(MAX_HOLD_DIAGNOSIS_QUERIES + 1),
        selection(1).replace("recorded_publication_state", "none"),
        selection(1).replace("recorded_publication_state", "policy_and_selected_witnesses"),
        selection(1).replace("\"queries\":1", "\"queries\":\"1\""),
        selection(1).replacen('{', "{\"queries\":1,", 1),
        selection(1).replacen('{', "{\"permit\":true,", 1),
    ] {
        assert!(
            Settings::decode(with_hold(&original, &invalid).as_bytes()).is_err(),
            "accepted hold selection {invalid}"
        );
    }
    assert!(
        Settings::decode(with_hold(&profile_json(path, identity, 0, 0), "null").as_bytes())
            .is_err()
    );
}

#[test]
fn real_diagnosis_preserves_source_admission_and_shares_work_across_operator_reconnects() {
    let mut fixture = fixture(2, 1, 1);
    fixture.submit(7);
    fixture.snapshot();
    let before = fixture.files();
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let mut service = fixture.start();
    let query = request(&fixture);
    let first = fixture.query(&mut service, query.clone());
    known(&first, "hold-diagnosis");
    assert_eq!(
        field(&first, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(
        predicate(&first, "recorded_stage_authorized").get("value").and_then(Json::as_bool),
        Some(false)
    );
    assert_eq!(
        predicate(&first, "current_clock").get("state").and_then(Json::as_str),
        Some("unknown")
    );
    assert_eq!(remaining(&service), 1);
    let second = fixture.query(&mut service, query);
    assert_eq!(second, first);
    assert_eq!(remaining(&service), 0);
    let exhausted = fixture.query(&mut service, request(&fixture));
    assert_eq!(
        field(&exhausted, &["envelope", "reason_code"]).as_str(),
        Some("BudgetExhausted")
    );
    let active = service.active.as_ref().unwrap();
    assert_eq!(active.inspection.as_ref().unwrap().remaining_queries(), 1);
    assert_eq!(active.explanation.as_ref().unwrap().remaining_queries(), 1);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    // Reads and failed exhausted reads left the original one-use actor view intact.
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(fixture.port.poll(&ticket), Knowledge::Pending { request: 8 });
    service.retire();
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn hold_diagnosis_is_separate_from_inspection_policy_disclosure_and_publication() {
    let mut ordinary = Fixture::new(1, 1);
    ordinary.submit(7);
    let mut service = ordinary.start();
    let denied = ordinary.query(&mut service, request(&ordinary));
    assert_eq!(
        field(&denied, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    assert!(service.active.as_ref().unwrap().hold_diagnosis.is_none());
    known(
        &ordinary.query(&mut service, ordinary.request(Verb::Situation)),
        "situation",
    );
    service.retire();

    let mut fixture = fixture(1, 0, 0);
    fixture.submit(7);
    fixture.snapshot();
    let before = fixture.files();
    let mut service = fixture.start();
    for verb in [Verb::Situation, Verb::Explain] {
        let denied = fixture.query(&mut service, fixture.request(verb));
        assert_eq!(
            field(&denied, &["envelope", "reason_code"]).as_str(),
            Some("CapabilityUnsupported")
        );
    }
    let mut proposal = fixture.request(Verb::Propose);
    proposal.arguments = ReadArguments::Propose {
        request: 8,
        proposal: fixture.proposal(),
    };
    proposal.idempotency_key = Some("8".into());
    let refused = fixture.query(&mut service, proposal);
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("PurposeMismatch")
    );
    assert_eq!(remaining(&service), 1);
    known(&fixture.query(&mut service, request(&fixture)), "hold-diagnosis");
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(fixture.port.poll(&ticket), Knowledge::Pending { request: 8 });
}

#[test]
fn fresh_diagnosis_follows_acknowledged_clock_progress_without_replaying_an_older_answer() {
    let mut fixture = fixture(3, 0, 0);
    fixture.submit(7);
    let mut service = fixture.start();
    let first = fixture.query(&mut service, request(&fixture));
    known(&first, "hold-diagnosis");
    assert_eq!(
        predicate(&first, "recorded_deadline_open").get("value").and_then(Json::as_bool),
        Some(true)
    );
    let deadline = fixture.proposal().deadline;
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, deadline).unwrap();
    }
    let before = fixture.files();
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let second = fixture.query(&mut service, request(&fixture));
    known(&second, "hold-diagnosis");
    assert_eq!(
        predicate(&second, "recorded_deadline_open").get("value").and_then(Json::as_bool),
        Some(false)
    );
    assert_ne!(
        field(&first, &["result", "value", "knowledge", "basis", "journal_revision"]),
        field(&second, &["result", "value", "knowledge", "basis", "journal_revision"])
    );
    assert_eq!(
        field(&first, &["result", "value", "knowledge", "basis", "request_generation"]),
        field(&second, &["result", "value", "knowledge", "basis", "request_generation"])
    );
    assert_eq!(
        predicate(&second, "current_clock").get("state").and_then(Json::as_str),
        Some("unknown")
    );
    let mut missing = request(&fixture);
    missing.address.id = "request-3-2-4-5-999".into();
    let missing = fixture.query(&mut service, missing);
    assert_eq!(
        field(&missing, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(remaining(&service), 0);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}
