//! Discover and assess original pending cases through the same finite service.
//! Every assessment identity below comes from the authenticated discovery reply.
use super::*;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::strict_json::Json;
use protocol::pending::{self, PendingRequest};

fn no_clock() -> Result<ElapsedTick, String> {
    panic!("pending-case discovery acquired a clock")
}

fn value(reply: &Json) -> &Json {
    reply.get("knowledge").unwrap().get("value").unwrap()
}

fn cases(reply: &Json) -> &[Json] {
    value(reply).get("cases").unwrap().as_array().unwrap()
}

fn revision(reply: &Json) -> u64 {
    reply
        .get("knowledge")
        .unwrap()
        .get("basis")
        .unwrap()
        .get("evaluation_revision")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

/// No native ticket or known fixture round is used to prepare this label.
fn assessment_from_page(
    settings: &Settings,
    reply: &Json,
    index: usize,
    origin: u64,
    truth: &str,
) -> Request {
    let case = &cases(reply)[index];
    let integer = |key: &str| {
        case.get(key)
            .unwrap()
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    let document = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},"policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{}","expected_evaluation_revision":{},"origin":{origin},"evidence_id_hex":"{}","truth":"{truth}"}}"#,
        integer("round"),
        integer("attempt"),
        integer("policy_generation"),
        integer("congress_generation"),
        case.get("case_evidence_root_hex")
            .unwrap()
            .as_str()
            .unwrap(),
        revision(reply),
        "13".repeat(32),
    );
    Request::prepare(settings, document.as_bytes()).unwrap()
}

fn read_page(fixture: &mut Fixture, service: &mut Service, request: &PendingRequest) -> Json {
    let image = fixture.image();
    let reply = fixture.exchange(service, &request.wire, &mut no_clock);
    assert!(pending::validate(request, &reply).unwrap());
    assert_eq!(
        fixture.image(),
        image,
        "discovery changed acknowledged native bytes"
    );
    pending::parse_response(&reply).unwrap()
}

#[test]
fn pending_discovery_drives_two_original_labels_without_preknown_rounds_or_effects() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    fixture.settings.candidates = 5;
    let request = PendingRequest::prepare(&fixture.settings, None).unwrap();
    let mut service = fixture.start();
    let first = read_page(&mut fixture, &mut service, &request);
    assert_eq!(
        value(&first).get("total_pending").unwrap().as_str(),
        Some("2")
    );
    assert_eq!(value(&first).get("complete").unwrap().as_bool(), Some(true));
    assert_eq!(cases(&first).len(), 2);
    let first_label = assessment_from_page(&fixture.settings, &first, 0, 1, "benign");
    let second_stale = assessment_from_page(&fixture.settings, &first, 1, 2, "violation");
    let reply = fixture.exchange(&mut service, &first_label.wire, &mut || Ok(ElapsedTick(20)));
    assert_eq!(
        protocol::validate(&first_label, &reply).unwrap(),
        Outcome::Applied
    );

    let second = read_page(&mut fixture, &mut service, &request);
    assert_eq!(cases(&second).len(), 1);
    assert_eq!(
        cases(&second)[0].get("round"),
        cases(&first)[1].get("round")
    );
    assert_eq!(revision(&second), revision(&first) + 1);
    let second_label = assessment_from_page(&fixture.settings, &second, 0, 2, "violation");
    assert_eq!(second_label.assessment.round, second_stale.assessment.round);
    assert_ne!(
        second_label.assessment.expected_revision,
        second_stale.assessment.expected_revision
    );
    let reply = fixture.exchange(&mut service, &second_label.wire, &mut no_clock);
    assert_eq!(
        protocol::validate(&second_label, &reply).unwrap(),
        Outcome::Applied
    );

    let empty = read_page(&mut fixture, &mut service, &request);
    assert!(cases(&empty).is_empty());
    assert_eq!(
        value(&empty).get("total_pending").unwrap().as_str(),
        Some("0")
    );
    assert_eq!(
        value(&empty).get("unfinished_reviews").unwrap().as_str(),
        Some("0")
    );
    assert_eq!(value(&empty).get("complete").unwrap().as_bool(), Some(true));
    assert_eq!(service.candidates, 5);
    assert!(service.io_calls > 0 && service.transferred > 0);
    let after = fixture.image();
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
    assert!(service.retired);
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.image(), after);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let host = fixture.driver.supervisor().host().unwrap();
    assert_eq!(host.credibility_report().unwrap().pending_cases, 0);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().payload, b"initial");
}

#[test]
fn censored_original_cases_remain_discoverable_but_unfinished_reviews_are_not_labelable() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        credibility::label(
            &mut host,
            fixture.evaluator.as_ref().unwrap(),
            1,
            1,
            GroundTruth::Censored,
        );
        credibility::begin(&mut host, 2);
    }
    let request = PendingRequest::prepare(&fixture.settings, None).unwrap();
    let mut service = fixture.start();
    let page = read_page(&mut fixture, &mut service, &request);
    assert_eq!(cases(&page).len(), 1);
    assert_eq!(
        cases(&page)[0].get("status").unwrap().as_str(),
        Some("censored")
    );
    assert_eq!(
        value(&page).get("total_pending").unwrap().as_str(),
        Some("1")
    );
    assert_eq!(
        value(&page).get("unfinished_reviews").unwrap().as_str(),
        Some("1")
    );
    let label = assessment_from_page(&fixture.settings, &page, 0, 1, "benign");
    let reply = fixture.exchange(&mut service, &label.wire, &mut || Ok(ElapsedTick(20)));
    assert_eq!(
        protocol::validate(&label, &reply).unwrap(),
        Outcome::Applied
    );
    let after = read_page(&mut fixture, &mut service, &request);
    assert!(cases(&after).is_empty());
    assert_eq!(
        value(&after).get("unfinished_reviews").unwrap().as_str(),
        Some("1")
    );
    assert_eq!(value(&after).get("complete").unwrap().as_bool(), Some(true));
    assert_eq!(service.candidates, 3);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn explicit_pending_pages_retain_one_revision_and_stale_cursor_requires_rediscovery() {
    let mut fixture = Fixture::with_case_capacity(32);
    for id in 1..=17 {
        fixture.review(id);
    }
    let first_request = PendingRequest::prepare(&fixture.settings, None).unwrap();
    // The largest accepted pending frame still fits a complete sixteen-case
    // response without retiring the original evaluator's bounded service.
    let mut padded = first_request.wire.clone();
    padded.resize(2 * 1024, b' ');
    let first_request = PendingRequest::accept(&fixture.settings, &padded).unwrap();
    let mut service = fixture.start();
    let first = read_page(&mut fixture, &mut service, &first_request);
    assert_eq!(cases(&first).len(), 16);
    assert_eq!(
        value(&first).get("total_pending").unwrap().as_str(),
        Some("17")
    );
    assert_eq!(
        value(&first).get("complete").unwrap().as_bool(),
        Some(false)
    );
    let boundary = value(&first)
        .get("next_after_round")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let next_request =
        PendingRequest::prepare(&fixture.settings, Some((revision(&first), boundary))).unwrap();
    let last = read_page(&mut fixture, &mut service, &next_request);
    assert_eq!(revision(&last), revision(&first));
    assert_eq!(cases(&last).len(), 1);
    assert_eq!(cases(&last)[0].get("round").unwrap().as_str(), Some("117"));
    assert_eq!(value(&last).get("complete").unwrap().as_bool(), Some(true));

    fixture.review(18);
    let before = fixture.image();
    let stale = fixture.exchange(&mut service, &next_request.wire, &mut no_clock);
    assert!(!pending::validate(&next_request, &stale).unwrap());
    let stale = pending::parse_response(&stale).unwrap();
    let knowledge = stale.get("knowledge").unwrap();
    assert_eq!(
        knowledge.get("reason").unwrap().as_str(),
        Some("evaluation_revision_changed")
    );
    assert!(knowledge.get("basis").is_none());
    assert!(knowledge.get("value").is_none());
    assert_eq!(fixture.image(), before);
    assert!(!service.retired);
    let refreshed = read_page(&mut fixture, &mut service, &first_request);
    assert_eq!(revision(&refreshed), revision(&first) + 1);
    assert_eq!(
        value(&refreshed).get("total_pending").unwrap().as_str(),
        Some("18")
    );
    assert_eq!(service.candidates, 4);
}

#[test]
fn invalid_pending_boundary_is_distinct_from_foreign_evaluator_custody() {
    let mut fixture = Fixture::new();
    let mut foreign = Fixture::new();
    fixture.review(1);
    foreign.review(1);
    let current = fixture
        .driver
        .supervisor()
        .host()
        .unwrap()
        .credibility_report()
        .unwrap()
        .revision;
    let invalid = PendingRequest::prepare(&fixture.settings, Some((current, 999))).unwrap();
    {
        let host = fixture.driver.supervisor().host().unwrap();
        assert!(
            invalid
                .observe(&host, foreign.evaluator.as_ref().unwrap())
                .is_err()
        );
        let response = invalid
            .observe(&host, fixture.evaluator.as_ref().unwrap())
            .unwrap();
        assert!(!pending::validate(&invalid, &response).unwrap());
    }
    let mut service = fixture.start();
    let before = fixture.image();
    let reply = fixture.exchange(&mut service, &invalid.wire, &mut no_clock);
    assert!(!pending::validate(&invalid, &reply).unwrap());
    assert_eq!(
        pending::parse_response(&reply)
            .unwrap()
            .get("knowledge")
            .unwrap()
            .get("reason")
            .unwrap()
            .as_str(),
        Some("invalid_continuation")
    );
    assert_eq!(fixture.image(), before);
    let initial = PendingRequest::prepare(&fixture.settings, None).unwrap();
    let page = read_page(&mut fixture, &mut service, &initial);
    assert_eq!(cases(&page).len(), 1);
    assert_eq!(service.candidates, 2);
}

#[test]
fn pending_discovery_waits_for_eof_and_original_stop_preempts_an_unfinished_frame() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = PendingRequest::prepare(&fixture.settings, None).unwrap();
    let mut service = fixture.start();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket.write_all(&request.wire).unwrap();
    socket.set_nonblocking(true).unwrap();
    let before = fixture.image();
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
    let mut bytes = [0_u8; 64];
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
        result => panic!("stopped pending reader disclosed a response: {result:?}"),
    }
}
