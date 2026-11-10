//! Independent cumulative numerical allowance for original admission history.
use super::*;

fn selected(original: &str, allowance: &str) -> String {
    original.replacen(
        '{',
        &format!("{{\"explanation_numerical_replay_events\":{allowance},"),
        1,
    )
}

#[test]
fn numerical_admission_profile_requires_its_own_positive_cumulative_allowance() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let path = Path::new("/operator/admission.sock");
    let original = profile_json(path, identity, 0, 2).replace(
        &format!("\"explanation_replay_events\":{MAX_EXPLANATION_EVENTS}"),
        "\"explanation_replay_events\":10",
    );
    let legacy = Settings::decode(original.as_bytes()).unwrap();
    assert_eq!(legacy.explanation_numerical_replay_events, None);
    for allowance in [1, 20] {
        let decoded = Settings::decode(selected(&original, &allowance.to_string()).as_bytes())
            .unwrap();
        assert_eq!(decoded.explanation_numerical_replay_events, Some(allowance));
        assert_eq!(
            (decoded.explanation_queries, decoded.explanation_replay_events),
            (2, 10)
        );
        assert_eq!(decoded.explanation_disclosure, "policy_and_selected_witnesses");
        assert!(decoded.review_explanation.is_none());
        assert!(decoded.resolution.is_none());
        assert!(decoded.policy_replay.is_none());
    }
    for invalid in ["0", "21", "null", "false", "\"1\"", "1.0", "-1", "18446744073709551615"] {
        assert!(
            Settings::decode(selected(&original, invalid).as_bytes()).is_err(),
            "accepted numerical admission selection {invalid}"
        );
    }
    for invalid in [
        selected(&original, "1").replace(
            "explanation_numerical_replay_events",
            "explanation_numerical_work",
        ),
        selected(&original, "1")
            .replacen('{', "{\"explanation_numerical_replay_events\":2,", 1),
        selected(&profile_json(path, identity, 1, 0), "1"),
    ] {
        assert!(Settings::decode(invalid.as_bytes()).is_err());
    }
    // Programmatic selection is checked again before the service can issue a grant.
    let mut disabled = Settings::decode(profile_json(path, identity, 1, 0).as_bytes()).unwrap();
    disabled.explanation_numerical_replay_events = Some(1);
    assert!(disabled.validate().is_err());
}

#[test]
fn selected_admission_numerical_custody_preserves_plain_explanations_and_source_work() {
    let mut fixture = Fixture::new(0, 2);
    fixture.settings.explanation_numerical_replay_events = Some(1);
    fixture.submit(7);
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let before = fixture.files();
    let mut service = fixture.start();
    for remaining in [1, 0] {
        known(
            &fixture.query(&mut service, fixture.request(Verb::Explain)),
            "explanation",
        );
        let grant = service.active.as_ref().unwrap().explanation.as_ref().unwrap();
        assert_eq!(grant.remaining_queries(), remaining);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(1));
    }
    let exhausted = fixture.query(&mut service, fixture.request(Verb::Explain));
    assert_eq!(
        field(&exhausted, &["result", "value", "error", "code"]).as_str(),
        Some("capacity")
    );
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}
