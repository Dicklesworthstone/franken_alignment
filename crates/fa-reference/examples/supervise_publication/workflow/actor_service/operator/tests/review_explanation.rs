//! A separately selected completed-review disclosure over the actual operator socket.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_REVIEW_EXPLANATION_EVENTS, MAX_REVIEW_EXPLANATION_QUERIES,
};

#[path = "../../../../../../tests/support/file_oversight_actor.rs"]
mod native;

const ROUND: u64 = 107;

fn with_review(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"review_explanation\":{selected},"), 1)
}

fn selection(queries: usize, events: usize) -> String {
    format!(
        r#"{{"queries":{queries},"max_replay_events":{events},"disclosure":"policy_and_congress_trace"}}"#
    )
}

fn fixture(queries: usize, events: usize, admission: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, admission, |config| {
        let clock_domain = config.profile.delivery.clock_domain;
        config.profile = native::profile();
        config.profile.delivery.clock_domain = clock_domain;
    });
    fixture.settings = Settings::decode(
        with_review(
            &profile_json(
                &fixture.settings.socket,
                fixture.settings.operator,
                0,
                admission,
            ),
            &selection(queries, events),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture
}

fn complete(fixture: &mut Fixture) {
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let request = fixture.actor.request;
    let attempt = native::attempt(&host, request);
    let action = host.request_action(request).unwrap().clone();
    let inputs = native::oversight::inputs(&action, b"PRIVATE-CONGRESS-INPUT");
    native::oversight::review_existing(&mut host, attempt, ROUND, &inputs);
}

fn request(fixture: &Fixture, round: u64) -> TowerRequest {
    let mut request = fixture.request(Verb::Explain);
    request.arguments = ReadArguments::ReviewExplanation { round };
    request
}

fn remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .review_explanation
        .as_ref()
        .unwrap()
        .remaining_queries()
}

#[test]
fn closed_review_profile_keeps_old_grants_separate_and_requires_bounded_explicit_disclosure() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let path = Path::new("/operator/review.sock");
    let original = profile_json(path, identity, 2, 2);
    for profile in [original.clone(), with_review(&original, "null")] {
        let decoded = Settings::decode(profile.as_bytes()).unwrap();
        assert!(decoded.review_explanation.is_none());
        assert_eq!(
            (decoded.inspection_queries, decoded.explanation_queries),
            (2, 2)
        );
    }
    let selected = selection(
        MAX_REVIEW_EXPLANATION_QUERIES,
        MAX_REVIEW_EXPLANATION_EVENTS,
    );
    let profile = with_review(&profile_json(path, identity, 0, 0), &selected);
    let decoded = Settings::decode(profile.as_bytes()).unwrap();
    assert_eq!(
        (decoded.inspection_queries, decoded.explanation_queries),
        (0, 0)
    );
    assert!(decoded.policy_replay.is_none());
    let grant = decoded.review_explanation.unwrap();
    assert_eq!(grant.queries, MAX_REVIEW_EXPLANATION_QUERIES);
    assert_eq!(grant.max_replay_events, MAX_REVIEW_EXPLANATION_EVENTS);
    assert_eq!(grant.disclosure, "policy_and_congress_trace");
    for invalid in [
        "false".to_owned(),
        "[]".to_owned(),
        "{}".to_owned(),
        selection(0, 1),
        selection(1, 0),
        selection(MAX_REVIEW_EXPLANATION_QUERIES + 1, 1),
        selection(1, MAX_REVIEW_EXPLANATION_EVENTS + 1),
        selected.replace("policy_and_congress_trace", "policy_and_selected_witnesses"),
        selected.replace("policy_and_congress_trace", "none"),
        selected.replacen('{', "{\"permit\":true,", 1),
        selected.replacen('{', "{\"queries\":1,", 1),
        selected.replace("\"max_replay_events\":", "\"events\":"),
        selection(1, 1).replace("\"queries\":1", "\"queries\":\"1\""),
    ] {
        assert!(
            Settings::decode(with_review(&original, &invalid).as_bytes()).is_err(),
            "accepted review selection {invalid}"
        );
    }
    assert!(
        Settings::decode(with_review(&profile_json(path, identity, 0, 0), "null").as_bytes())
            .is_err()
    );
}

#[test]
fn completed_native_review_crosses_the_operator_socket_without_reading_source_or_changing_authority()
 {
    let mut fixture = fixture(2, MAX_REVIEW_EXPLANATION_EVENTS, 2);
    fixture.submit(7);
    complete(&mut fixture);
    let before = fixture.files();
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let mut service = fixture.start();
    let query = request(&fixture, ROUND);
    let first = fixture.query(&mut service, query.clone());
    known(&first, "review-explanation");
    assert_eq!(
        field(&first, &["result", "value", "round"]).as_str(),
        Some("107")
    );
    assert_eq!(
        field(
            &first,
            &["result", "value", "knowledge", "basis", "request"]
        )
        .as_str(),
        Some("7")
    );
    assert_eq!(
        field(&first, &["result", "value", "knowledge", "basis", "round"]).as_str(),
        Some("107")
    );
    assert_eq!(
        field(&first, &["result", "value", "effect_authority"]).as_bool(),
        Some(false)
    );
    assert_eq!(
        field(&first, &["result", "value", "helper_inputs_disclosed"]).as_bool(),
        Some(false)
    );
    assert_eq!(remaining(&service), 1);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        2
    );
    // This is another authenticated connection to the original service/grant.
    let second = fixture.query(&mut service, query);
    assert_eq!(second, first);
    assert_eq!(remaining(&service), 0);
    let admission = fixture.query(&mut service, fixture.request(Verb::Explain));
    known(&admission, "explanation");
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        1
    );
    let exhausted = fixture.query(&mut service, request(&fixture, ROUND));
    assert_eq!(
        field(&exhausted, &["envelope", "reason_code"]).as_str(),
        Some("BudgetExhausted")
    );
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    assert!(!format!("{first:?}").contains("PRIVATE-CONGRESS-INPUT"));
    service.retire();
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn admission_only_cannot_disclose_congress_and_review_only_cannot_admit_or_explain_policy_admission()
 {
    let mut ordinary = Fixture::new(0, 2);
    ordinary.submit(7);
    let mut service = ordinary.start();
    let denied = ordinary.query(&mut service, request(&ordinary, ROUND));
    assert_eq!(
        field(&denied, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    assert!(
        service
            .active
            .as_ref()
            .unwrap()
            .review_explanation
            .is_none()
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        2
    );
    known(
        &ordinary.query(&mut service, ordinary.request(Verb::Explain)),
        "explanation",
    );
    service.retire();

    let mut fixture = fixture(1, MAX_REVIEW_EXPLANATION_EVENTS, 0);
    fixture.submit(7);
    complete(&mut fixture);
    fixture.snapshot();
    let before = fixture.files();
    let mut service = fixture.start();
    let admission = fixture.query(&mut service, fixture.request(Verb::Explain));
    assert_eq!(
        field(&admission, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    let mut proposed = fixture.request(Verb::Propose);
    proposed.arguments = ReadArguments::Propose {
        request: 8,
        proposal: fixture.proposal(),
    };
    proposed.idempotency_key = Some("8".into());
    let proposed = fixture.query(&mut service, proposed);
    assert_eq!(
        field(&proposed, &["envelope", "reason_code"]).as_str(),
        Some("PurposeMismatch")
    );
    assert_eq!(remaining(&service), 1);
    known(
        &fixture.query(&mut service, request(&fixture, ROUND)),
        "review-explanation",
    );
    assert_eq!(remaining(&service), 0);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    // The rejected operator proposal did not consume the original actor's view.
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 8 }
    );
}

#[test]
fn unobserved_round_and_exceeded_replay_bounds_are_charged_without_reading_current_source() {
    for (events, selected_round) in [(MAX_REVIEW_EXPLANATION_EVENTS, ROUND + 1), (1, ROUND)] {
        let mut fixture = fixture(1, events, 0);
        fixture.submit(7);
        complete(&mut fixture);
        let before = fixture.files();
        let mut service = fixture.start();
        let result = fixture.query(&mut service, request(&fixture, selected_round));
        assert_eq!(
            field(&result, &["result", "kind"]).as_str(),
            Some("review-explanation")
        );
        assert_eq!(
            field(&result, &["result", "value", "knowledge", "state"]).as_str(),
            Some("unknown")
        );
        assert_eq!(remaining(&service), 0);
        let exhausted = fixture.query(&mut service, request(&fixture, ROUND));
        assert_eq!(
            field(&exhausted, &["envelope", "reason_code"]).as_str(),
            Some("BudgetExhausted")
        );
        assert_eq!(fixture.files(), before);
        assert_eq!(fixture.config.source.status().read_attempts, 0);
    }
}
