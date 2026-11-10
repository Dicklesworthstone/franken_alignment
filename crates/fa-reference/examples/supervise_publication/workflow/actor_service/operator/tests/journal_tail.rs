//! Original acknowledged journal metadata through the authenticated operator.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_JOURNAL_TAIL_PAGE_EVENTS, MAX_JOURNAL_TAIL_QUERIES,
};

fn with_tail(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"journal_tail\":{selected},"), 1)
}

fn selection(queries: usize, events_per_page: usize) -> String {
    format!(
        r#"{{"queries":{queries},"events_per_page":{events_per_page},"disclosure":"redacted_journal_events"}}"#,
    )
}

fn fixture(queries: usize, events_per_page: usize) -> Fixture {
    let mut fixture = Fixture::new(1, 1);
    fixture.settings = Settings::decode(
        with_tail(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 1, 1),
            &selection(queries, events_per_page),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture
}

fn request(fixture: &Fixture, after_revision: u64) -> TowerRequest {
    let mut request = fixture.request(Verb::Tail);
    request.arguments = ReadArguments::JournalTail { after_revision };
    request
}

fn remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .journal_tail
        .as_ref()
        .unwrap()
        .remaining_queries()
}

fn page(response: &Json) -> &Json {
    field(response, &["result", "value", "knowledge", "value"])
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

#[test]
fn tail_profile_requires_explicit_metadata_disclosure_and_finite_pages() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let path = Path::new("/operator/tail.sock");
    let original = profile_json(path, identity, 1, 1);
    for unchanged in [original.clone(), with_tail(&original, "null")] {
        let decoded = Settings::decode(unchanged.as_bytes()).unwrap();
        assert!(decoded.journal_tail.is_none());
        assert_eq!(
            (decoded.inspection_queries, decoded.explanation_queries),
            (1, 1)
        );
    }
    let tail_only = with_tail(
        &profile_json(path, identity, 0, 0),
        &selection(MAX_JOURNAL_TAIL_QUERIES, MAX_JOURNAL_TAIL_PAGE_EVENTS),
    );
    let decoded = Settings::decode(tail_only.as_bytes()).unwrap();
    decoded.check_learned_operator().unwrap();
    assert_eq!(
        (decoded.inspection_queries, decoded.explanation_queries),
        (0, 0)
    );
    assert!(decoded.resolution.is_none());
    let selected = decoded.journal_tail.unwrap();
    assert_eq!(selected.queries, MAX_JOURNAL_TAIL_QUERIES);
    assert_eq!(selected.events_per_page, MAX_JOURNAL_TAIL_PAGE_EVENTS);
    assert_eq!(selected.disclosure, "redacted_journal_events");
    for invalid in [
        "false".to_owned(),
        "[]".to_owned(),
        "{}".to_owned(),
        selection(0, 1),
        selection(1, 0),
        selection(MAX_JOURNAL_TAIL_QUERIES + 1, 1),
        selection(1, MAX_JOURNAL_TAIL_PAGE_EVENTS + 1),
        selection(1, 1).replace("redacted_journal_events", "none"),
        selection(1, 1).replace("redacted_journal_events", "policy_and_selected_witnesses"),
        selection(1, 1).replace("\"queries\":1", "\"queries\":\"1\""),
        selection(1, 1).replace("\"events_per_page\":1", "\"events_per_page\":1.5"),
        selection(1, 1).replacen('{', "{\"queries\":1,", 1),
        selection(1, 1).replacen('{', "{\"payloads\":true,", 1),
    ] {
        assert!(
            Settings::decode(with_tail(&original, &invalid).as_bytes()).is_err(),
            "accepted tail selection {invalid}"
        );
    }
    assert!(
        Settings::decode(with_tail(&profile_json(path, identity, 0, 0), "null").as_bytes())
            .is_err()
    );
}

#[test]
fn native_tail_retains_its_cut_and_read_allowance_across_authenticated_reconnects() {
    let mut fixture = fixture(2, 2);
    fixture.submit(7);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        for tick in 1_001..1_006 {
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(tick)).unwrap();
        }
    }
    fixture.snapshot();
    let captured = fixture.driver.supervisor().host().unwrap().inspect();
    let files = fixture.files();
    let mut service = fixture.start();
    let first = fixture.query(&mut service, request(&fixture, 0));
    known(&first, "journal-tail");
    assert_eq!(number(page(&first).get("after_revision").unwrap()), 0);
    assert_eq!(number(page(&first).get("through_revision").unwrap()), 2);
    assert_eq!(
        number(field(
            &first,
            &["result", "value", "knowledge", "basis", "journal_revision"]
        )),
        captured.revision,
    );
    assert_eq!(
        page(&first)
            .get("events")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        page(&first)
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(false)
    );
    let cursor = number(page(&first).get("continuation").unwrap());
    assert_eq!(remaining(&service), 1);
    assert_eq!(fixture.files(), files);
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        captured
    );

    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1_006)).unwrap();
    }
    fixture.snapshot();
    let later = fixture.driver.supervisor().host().unwrap().inspect();
    let later_files = fixture.files();
    let mut next = fixture.request(Verb::Tail);
    next.arguments = ReadArguments::JournalTailPage { cursor };
    let second = fixture.query(&mut service, next.clone());
    known(&second, "journal-tail");
    assert_eq!(
        field(&first, &["result", "value", "knowledge", "basis"]),
        field(&second, &["result", "value", "knowledge", "basis"]),
    );
    let events = page(&second).get("events").unwrap().as_array().unwrap();
    assert_eq!(number(events[0].get("revision").unwrap()), 3);
    assert_eq!(number(events[1].get("revision").unwrap()), 4);
    assert_eq!(number(page(&second).get("through_revision").unwrap()), 4);
    assert_eq!(
        page(&second)
            .get("enumeration_complete")
            .and_then(Json::as_bool),
        Some(false)
    );
    assert_eq!(page(&second).get("continuation"), Some(&Json::Null));
    assert_eq!(remaining(&service), 0);
    let exhausted = fixture.query(&mut service, request(&fixture, 0));
    assert_eq!(
        field(&exhausted, &["envelope", "reason_code"]).as_str(),
        Some("BudgetExhausted"),
    );
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), later);
    assert_eq!(fixture.files(), later_files);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let active = service.active.as_ref().unwrap();
    assert_eq!(active.inspection.as_ref().unwrap().remaining_queries(), 1);
    assert_eq!(active.explanation.as_ref().unwrap().remaining_queries(), 1);
    // The same fresh actor view remains available after tail and exhausted tail.
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 8 }
    );
    service.retire();
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn ordinary_disclosure_does_not_grant_tail_and_future_revision_is_not_absence() {
    let mut fixture = Fixture::new(2, 1);
    fixture.submit(7);
    let files = fixture.files();
    let mut service = fixture.start();
    let unavailable = fixture.query(&mut service, request(&fixture, 0));
    assert_eq!(
        field(&unavailable, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported"),
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
        2
    );
    assert_eq!(fixture.files(), files);
    service.retire();

    let mut selected = self::fixture(1, MAX_JOURNAL_TAIL_PAGE_EVENTS);
    selected.submit(7);
    let mut service = selected.start();
    let future = selected.driver.supervisor().host().unwrap().revision() + 1;
    let response = selected.query(&mut service, request(&selected, future));
    assert_eq!(
        field(&response, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown"),
    );
    assert_eq!(remaining(&service), 0);
    assert!(
        field(&response, &["result", "value", "knowledge"])
            .get("value")
            .is_none()
    );
}
