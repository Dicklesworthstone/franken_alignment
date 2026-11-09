//! Explicit page custody stays with the original authenticated operator service.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_REPLAY_PAGE_QUERIES;
use fa_reference::tower::policy_replay_client::PolicyReplayCapture;

fn page_selection(queries: &str) -> String {
    selection(1, 64, 1_048_576).replacen('{', &format!("{{\"page_queries\":{queries},"), 1)
}

fn paged(fixture: &Fixture) -> TowerRequest {
    let mut query = request(fixture, 71, candidate(1));
    query.arguments = ReadArguments::PolicyReplayPaged {
        campaign: 71,
        expected_policy_generation: fixture.config.profile.delivery.policy.generation(),
        candidate: candidate(1),
    };
    query
}

fn pages_remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .policy_replay
        .as_ref()
        .unwrap()
        .remaining_pages()
}

#[test]
fn page_queries_are_optional_closed_and_bounded_before_setup() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let original = profile_json(Path::new("/operator/inspection.sock"), identity, 0, 0);
    let disabled =
        Settings::decode(with_replay(&original, &selection(1, 64, 1_048_576)).as_bytes()).unwrap();
    assert_eq!(disabled.policy_replay.unwrap().page_queries, 0);
    for count in [1, MAX_POLICY_REPLAY_PAGE_QUERIES] {
        let selected = Settings::decode(
            with_replay(&original, &page_selection(&count.to_string())).as_bytes(),
        )
        .unwrap();
        assert_eq!(selected.policy_replay.unwrap().page_queries, count);
    }
    for invalid in ["0", "1025", "null", "true", "\"1\"", "1.5", "-1"] {
        assert!(
            Settings::decode(with_replay(&original, &page_selection(invalid)).as_bytes()).is_err()
        );
    }
    let valid = page_selection("8");
    for invalid in [
        valid.replacen('{', "{\"page_queries\":1,", 1),
        valid.replacen('{', "{\"pages\":8,", 1),
    ] {
        assert!(Settings::decode(with_replay(&original, &invalid).as_bytes()).is_err());
    }
    let top_level = with_replay(&original, &valid).replacen('{', "{\"page_queries\":8,", 1);
    assert!(Settings::decode(top_level.as_bytes()).is_err());
}

#[test]
fn authenticated_pages_and_cached_header_keep_one_grant_across_reconnects() {
    let mut fixture = Fixture::new(1, 0);
    for key in 7..27 {
        fixture.submit(key);
    }
    fixture.settings = Settings::decode(
        with_response_bytes(
            &with_replay(
                &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
                &page_selection("4"),
            ),
            &MAX_PUBLICATION_RESPONSE_BYTES.to_string(),
        )
        .as_bytes(),
    )
    .unwrap();
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let files = fixture.files();
    let first = paged(&fixture);
    let mut capture = PolicyReplayCapture::new(fixture.actor.scope, &first, 4).unwrap();
    let mut service = fixture.start();
    assert_eq!(pages_remaining(&service), 4);
    let (header_bytes, header) = selected_query(&fixture, &mut service, first.clone());
    let mut progress = capture.accept(&first, &header_bytes).unwrap();
    assert!(!progress.complete);
    assert_eq!(progress.cases_seen, 0);
    assert_eq!(remaining(&service), 0);
    assert_eq!(pages_remaining(&service), 3);
    let retained = retained_bytes(&service);

    let mut retry = first.clone();
    retry.operation_id = "same-original-header".into();
    let (_, repeated) = selected_query(&fixture, &mut service, retry);
    assert_eq!(
        field(&header, &["result", "value", "knowledge"]),
        field(&repeated, &["result", "value", "knowledge"])
    );
    assert_eq!(pages_remaining(&service), 3);
    for page in 1..=2 {
        let mut next = first.clone();
        next.operation_id = format!("operator-case-page-{page}");
        next.arguments = ReadArguments::PolicyReplayContinue {
            campaign: 71,
            cursor: progress.next_cursor.unwrap(),
        };
        let (bytes, _) = selected_query(&fixture, &mut service, next.clone());
        progress = capture.accept(&next, &bytes).unwrap();
        assert_eq!(progress.cases_seen, if page == 1 { 16 } else { 20 });
        assert_eq!(pages_remaining(&service), 3 - page);
        assert_eq!(retained_bytes(&service), retained);
    }
    assert!(progress.complete);
    assert_eq!(progress.next_cursor, None);
    assert_eq!(progress.pages_seen, 3);
    assert_eq!(remaining(&service), 0);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.files(), files);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    assert!(service.active.as_ref().unwrap().inspection.is_none());
    assert!(service.active.as_ref().unwrap().explanation.is_none());
}

#[test]
fn legacy_profile_does_not_grant_pages_or_spend_a_campaign_for_a_page_request() {
    let mut fixture = Fixture::new(1, 0);
    fixture.submit(7);
    select(&mut fixture, 1);
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let mut service = fixture.start();
    let (_, response) = selected_query(&fixture, &mut service, paged(&fixture));
    assert_eq!(
        field(&response, &["result", "value", "error", "code"]).as_str(),
        Some("withheld")
    );
    let grant = service
        .active
        .as_ref()
        .unwrap()
        .policy_replay
        .as_ref()
        .unwrap();
    assert!(!grant.pages_enabled());
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_pages(), 0);
    assert_eq!(grant.retained_bytes(), 0);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}
