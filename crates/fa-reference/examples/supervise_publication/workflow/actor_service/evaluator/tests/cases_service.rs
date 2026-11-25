//! Discovery, exact-case observation and labeling share one authenticated owner,
//! lifetime and candidate budget. Pages never acquire time or source authority.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileEvaluationCasesBasis;
use fa_reference::strict_json::{self, Json, Limits};
use protocol::{
    case::CaseRequest,
    cases::{self, CasesRequest},
};

pub(super) fn parsed_page(bytes: &[u8]) -> Json {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: protocol::RESPONSE_BYTES,
            max_depth: 6,
            max_items: 512,
            max_string_bytes: 128,
        },
    )
    .unwrap()
}

pub(super) fn page_basis(reply: &[u8]) -> FileEvaluationCasesBasis {
    let parsed = parsed_page(reply);
    let basis = parsed.get("knowledge").unwrap().get("basis").unwrap();
    let integer = |key| basis.get(key).unwrap().as_str().unwrap().parse().unwrap();
    FileEvaluationCasesBasis {
        journal_revision: integer("journal_revision"),
        evaluation_revision: integer("evaluation_revision"),
    }
}

fn continuation(settings: &Settings, reply: &[u8], limit: usize) -> CasesRequest {
    let parsed = parsed_page(reply);
    let after = parsed
        .get("knowledge")
        .unwrap()
        .get("value")
        .unwrap()
        .get("next_after")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    CasesRequest::prepare(settings, limit, Some(after), Some(page_basis(reply))).unwrap()
}

fn no_clock() -> Result<ElapsedTick, String> {
    panic!("read-only discovery or stale continuation acquired a clock")
}

#[test]
fn bounded_pages_discover_and_label_an_original_case_without_out_of_band_round_ids() {
    let mut fixture = Fixture::new();
    for id in [3, 1, 2] {
        fixture.review(id);
    }
    fixture.settings.candidates = 5;
    let first = CasesRequest::prepare(&fixture.settings, 2, None, None).unwrap();
    let mut service = fixture.start();
    let before = fixture.image();
    let first_reply = fixture.exchange(&mut service, &first.wire, &mut no_clock);
    assert!(cases::validate(&first, &first_reply).unwrap());
    let parsed = parsed_page(&first_reply);
    let page = parsed.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(page.get("total_cases").unwrap().as_str(), Some("3"));
    assert_eq!(page.get("remaining_cases").unwrap().as_str(), Some("3"));
    let entries = page.get("cases").unwrap().as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get("round").unwrap().as_str(), Some("101"));
    assert_eq!(entries[1].get("round").unwrap().as_str(), Some("102"));
    assert!(
        entries
            .iter()
            .all(|entry| entry.get("status").unwrap().as_str() == Some("pending"))
    );
    let second = continuation(&fixture.settings, &first_reply, 2);
    let second_reply = fixture.exchange(&mut service, &second.wire, &mut no_clock);
    assert!(cases::validate(&second, &second_reply).unwrap());
    assert_eq!(page_basis(&first_reply), page_basis(&second_reply));
    let parsed = parsed_page(&second_reply);
    let last = parsed.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(last.get("remaining_cases").unwrap().as_str(), Some("1"));
    assert!(last.get("next_after").unwrap().is_null());
    assert_eq!(
        last.get("cases").unwrap().as_array().unwrap()[0]
            .get("round")
            .unwrap()
            .as_str(),
        Some("103")
    );
    assert_eq!(fixture.image(), before);

    // The list response alone chooses the exact-case request. Existing case
    // observation supplies the native assessment binding; list status is never truth.
    let round = entries[0]
        .get("round")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let exact = CaseRequest::prepare(&fixture.settings, round).unwrap();
    let exact_reply = fixture.exchange(&mut service, &exact.wire, &mut no_clock);
    let label = super::case_service::assessment_from_case(&fixture.settings, &exact_reply, 1);
    assert_eq!(label.assessment.round, round);
    let reply = fixture.exchange(&mut service, &label.wire, &mut || Ok(ElapsedTick(20)));
    assert_eq!(
        protocol::validate(&label, &reply).unwrap(),
        Outcome::Applied
    );
    let labeled = fixture.image();
    let stale = fixture.exchange(&mut service, &second.wire, &mut no_clock);
    assert!(!cases::validate(&second, &stale).unwrap());
    assert_eq!(
        parsed_page(&stale)
            .get("knowledge")
            .unwrap()
            .get("reason")
            .unwrap()
            .as_str(),
        Some("stale_basis")
    );
    assert_eq!(fixture.image(), labeled);
    assert_eq!(service.candidates, 5);
    assert!(service.transferred <= service.max_bytes);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    {
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.credibility_report().unwrap().pending_cases, 2);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
        assert_eq!(host.inspect().executions, 0);
    }
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
    assert!(service.retired);
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.image(), labeled);
}

#[test]
fn time_only_changes_stale_the_original_cut_and_missing_cursors_are_never_empty_pages() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let first = CasesRequest::prepare(&fixture.settings, 1, None, None).unwrap();
    let mut service = fixture.start();
    let reply = fixture.exchange(&mut service, &first.wire, &mut no_clock);
    let old = page_basis(&reply);
    let next = continuation(&fixture.settings, &reply, 1);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
        assert_eq!(
            host.credibility_report().unwrap().revision,
            old.evaluation_revision
        );
    }
    let changed = fixture.image();
    let reply = fixture.exchange(&mut service, &next.wire, &mut no_clock);
    assert!(!cases::validate(&next, &reply).unwrap());
    let knowledge = parsed_page(&reply).get("knowledge").unwrap().clone();
    assert_eq!(
        knowledge.get("reason").unwrap().as_str(),
        Some("stale_basis")
    );
    assert!(knowledge.get("basis").is_none());
    assert!(knowledge.get("value").is_none());

    let refreshed = fixture.exchange(&mut service, &first.wire, &mut no_clock);
    assert!(cases::validate(&first, &refreshed).unwrap());
    let basis = page_basis(&refreshed);
    assert_eq!(basis.evaluation_revision, old.evaluation_revision);
    assert_eq!(basis.journal_revision, old.journal_revision + 1);
    let missing = CasesRequest::prepare(&fixture.settings, 1, Some(999), Some(basis)).unwrap();
    let reply = fixture.exchange(&mut service, &missing.wire, &mut no_clock);
    assert!(!cases::validate(&missing, &reply).unwrap());
    assert_eq!(
        parsed_page(&reply)
            .get("knowledge")
            .unwrap()
            .get("reason")
            .unwrap()
            .as_str(),
        Some("cursor_not_recorded")
    );
    let next = continuation(&fixture.settings, &refreshed, 1);
    let reply = fixture.exchange(&mut service, &next.wire, &mut no_clock);
    assert!(cases::validate(&next, &reply).unwrap());
    assert_eq!(fixture.image(), changed);
    assert_eq!(service.candidates, 5);
}

#[test]
fn begun_unapplied_round_is_explicit_even_when_no_applied_case_exists() {
    let mut fixture = Fixture::new();
    credibility::begin(&mut fixture.driver.supervisor_mut().host_mut().unwrap(), 1);
    let request = CasesRequest::prepare(&fixture.settings, 16, None, None).unwrap();
    let mut service = fixture.start();
    let before = fixture.image();
    let reply = fixture.exchange(&mut service, &request.wire, &mut no_clock);
    assert!(cases::validate(&request, &reply).unwrap());
    assert_eq!(page_basis(&reply).evaluation_revision, 0);
    let parsed = parsed_page(&reply);
    let page = parsed.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(page.get("total_cases").unwrap().as_str(), Some("0"));
    assert_eq!(page.get("unapplied_rounds").unwrap().as_str(), Some("1"));
    assert!(page.get("cases").unwrap().as_array().unwrap().is_empty());
    assert!(page.get("next_after").unwrap().is_null());
    assert_eq!(fixture.image(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn an_unsealed_listing_discloses_nothing_and_yields_to_original_stop() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = CasesRequest::prepare(&fixture.settings, 16, None, None).unwrap();
    let mut service = fixture.start();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket.write_all(&request.wire).unwrap();
    socket.set_nonblocking(true).unwrap();
    let before = fixture.image();
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
    let mut bytes = [0; 64];
    assert!(matches!(socket.read(&mut bytes), Err(error) if transient(&error)));
    assert_eq!(fixture.image(), before);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let state = host.inspect();
        let revision = host.revision();
        host.request_stop(
            revision,
            StopRequest {
                operation: 1,
                expected_control_sequence: state.control.sequence,
                expected_authority_epoch: state.control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let stopped = fixture.image();
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
    assert!(service.retired);
    assert_eq!(fixture.image(), stopped);
    assert_eq!(service.candidates, 1);
    match socket.read(&mut bytes) {
        Ok(0) => {}
        Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
        result => panic!("stopped listing disclosed or retained a connection: {result:?}"),
    }
}
