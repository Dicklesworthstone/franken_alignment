//! Independent discovery supplies the actual case and predecessor used by a
//! later label. Reads and labels retain one authenticated finite live service.
use super::*;
use protocol::case::{self, CaseRequest};

fn parsed_case(bytes: &[u8]) -> fa_reference::strict_json::Json {
    fa_reference::strict_json::parse(
        bytes,
        fa_reference::strict_json::Limits {
            max_bytes: protocol::RESPONSE_BYTES,
            max_depth: 6,
            max_items: 128,
            max_string_bytes: 16_384,
        },
    )
    .unwrap()
}

/// Deliberately use ONLY the authenticated response to construct the case
/// binding. Native fixture access cannot conceal an unusable disclosure.
pub(super) fn assessment_from_case(settings: &Settings, reply: &[u8], origin: u64) -> Request {
    let response = parsed_case(reply);
    let root = response.as_object().unwrap();
    let knowledge = root["knowledge"].as_object().unwrap();
    assert_eq!(knowledge["state"].as_str(), Some("known"));
    let case = knowledge["value"].as_object().unwrap();
    let basis = knowledge["basis"].as_object().unwrap();
    let integer = |key: &str| case[key].as_str().unwrap().parse::<u64>().unwrap();
    let revision = basis["evaluation_revision"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let document = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},"policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{}","expected_evaluation_revision":{revision},"origin":{origin},"evidence_id_hex":"{}","truth":"benign"}}"#,
        integer("round"),
        integer("attempt"),
        integer("policy_generation"),
        integer("congress_generation"),
        case["case_evidence_root_hex"].as_str().unwrap(),
        "13".repeat(32),
    );
    Request::prepare(settings, document.as_bytes()).unwrap()
}

fn no_clock() -> Result<ElapsedTick, String> {
    panic!("read-only, malformed or stale case request acquired a clock")
}

#[test]
fn authenticated_case_discovery_drives_assessment_with_one_shared_candidate_budget() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.settings.candidates = 3;
    let read = CaseRequest::prepare(&fixture.settings, 101).unwrap();
    let mut service = fixture.start();
    let original = fixture.image();
    let reply = fixture.exchange(&mut service, &read.wire, &mut no_clock);
    assert!(case::validate(&read, &reply).unwrap());
    assert_eq!(fixture.image(), original);
    assert_eq!(service.candidates, 1);

    let label = assessment_from_case(&fixture.settings, &reply, 1);
    assert_eq!(label.assessment.round, 101);
    let original_revision = label.assessment.expected_revision;
    assert!(original_revision > 0);
    let clocks = Cell::new(0);
    let labeled = fixture.exchange(&mut service, &label.wire, &mut || {
        clocks.set(clocks.get() + 1);
        Ok(ElapsedTick(20))
    });
    assert_eq!(
        protocol::validate(&label, &labeled).unwrap(),
        Outcome::Applied
    );
    assert_eq!(clocks.get(), 1);
    assert_eq!(service.candidates, 2);

    let after_label = fixture.image();
    let refreshed = fixture.exchange(&mut service, &read.wire, &mut no_clock);
    assert!(case::validate(&read, &refreshed).unwrap());
    assert_eq!(fixture.image(), after_label);
    let current_label = assessment_from_case(&fixture.settings, &refreshed, 1);
    assert_eq!(
        current_label.assessment.expected_revision,
        original_revision + 1
    );
    assert_eq!(current_label.assessment.root, label.assessment.root);
    assert_eq!(service.candidates, 3);
    {
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.credibility_report().unwrap().pending_cases, 0);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
        assert_eq!(host.inspect().payload, b"initial");
        assert_eq!(host.inspect().executions, 0);
    }
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
    assert!(service.retired);
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.image(), after_label);
}

#[test]
fn missing_round_is_unknown_and_a_present_round_remains_readable_without_time() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let missing = CaseRequest::prepare(&fixture.settings, 999).unwrap();
    let present = CaseRequest::prepare(&fixture.settings, 101).unwrap();
    let mut service = fixture.start();
    let before = fixture.image();
    let reply = fixture.exchange(&mut service, &missing.wire, &mut no_clock);
    assert!(!case::validate(&missing, &reply).unwrap());
    let response = parsed_case(&reply);
    let knowledge = response.as_object().unwrap()["knowledge"]
        .as_object()
        .unwrap();
    assert_eq!(knowledge["reason"].as_str(), Some("case_not_recorded"));
    assert!(!knowledge.contains_key("basis"));
    assert!(!knowledge.contains_key("value"));
    assert_eq!(fixture.image(), before);
    let reply = fixture.exchange(&mut service, &present.wire, &mut no_clock);
    assert!(case::validate(&present, &reply).unwrap());
    assert_eq!(fixture.image(), before);
    assert_eq!(service.candidates, 2);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn discovered_case_does_not_authorize_a_stale_label_after_another_assessment() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let first = CaseRequest::prepare(&fixture.settings, 101).unwrap();
    let second = CaseRequest::prepare(&fixture.settings, 102).unwrap();
    let mut service = fixture.start();
    let first_reply = fixture.exchange(&mut service, &first.wire, &mut no_clock);
    assert!(case::validate(&first, &first_reply).unwrap());
    let second_reply = fixture.exchange(&mut service, &second.wire, &mut no_clock);
    assert!(case::validate(&second, &second_reply).unwrap());
    let stale = assessment_from_case(&fixture.settings, &first_reply, 1);
    let advance = assessment_from_case(&fixture.settings, &second_reply, 2);
    let original_revision = stale.assessment.expected_revision;
    assert!(original_revision > 0);
    assert_eq!(advance.assessment.expected_revision, original_revision);
    let applied = fixture.exchange(&mut service, &advance.wire, &mut || Ok(ElapsedTick(20)));
    assert_eq!(
        protocol::validate(&advance, &applied).unwrap(),
        Outcome::Applied
    );
    let before_refusal = fixture.image();
    let refused = fixture.exchange(&mut service, &stale.wire, &mut no_clock);
    assert_eq!(
        protocol::validate(&stale, &refused).unwrap(),
        Outcome::Refused
    );
    assert_eq!(fixture.image(), before_refusal);

    let refreshed = fixture.exchange(&mut service, &first.wire, &mut no_clock);
    assert!(case::validate(&first, &refreshed).unwrap());
    let current = assessment_from_case(&fixture.settings, &refreshed, 1);
    assert_eq!(current.assessment.expected_revision, original_revision + 1);
    assert_eq!(current.assessment.root, stale.assessment.root);
    let applied = fixture.exchange(&mut service, &current.wire, &mut || Ok(ElapsedTick(21)));
    assert_eq!(
        protocol::validate(&current, &applied).unwrap(),
        Outcome::Applied
    );
    let host = fixture.driver.supervisor().host().unwrap();
    assert_eq!(
        host.credibility_report().unwrap().revision,
        original_revision + 2,
    );
    assert_eq!(host.credibility_report().unwrap().pending_cases, 0);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 2);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(service.candidates, 6);
}

#[test]
fn same_numeric_owner_cannot_use_another_independent_evaluator_custodian() {
    let mut fixture = Fixture::new();
    let mut foreign = Fixture::new();
    fixture.review(1);
    foreign.review(1);
    assert_eq!(fixture.actor.scope, foreign.actor.scope);
    let request = CaseRequest::prepare(&fixture.settings, 101).unwrap();
    let before = fixture.image();
    {
        let host = fixture.driver.supervisor().host().unwrap();
        let original = request
            .observe(&host, fixture.evaluator.as_ref().unwrap())
            .unwrap();
        assert!(case::validate(&request, &original).unwrap());
        assert!(
            request
                .observe(&host, foreign.evaluator.as_ref().unwrap())
                .is_err()
        );
    }
    assert!(
        Service::start(
            &fixture.settings,
            &fixture.actor,
            foreign.evaluator.take().unwrap(),
            &fixture.driver,
            &deadline(),
        )
        .is_err()
    );
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.image(), before);
    let mut service = fixture.start();
    let reply = fixture.exchange(&mut service, &request.wire, &mut no_clock);
    assert!(case::validate(&request, &reply).unwrap());
    assert_eq!(fixture.image(), before);
}

#[test]
fn a_running_service_cannot_read_or_timestamp_a_same_scope_replacement_owner() {
    for assessment in [false, true] {
        let mut fixture = Fixture::new();
        let mut foreign = Fixture::new();
        fixture.review(1);
        foreign.review(1);
        assert_eq!(fixture.actor.scope, foreign.actor.scope);
        let wire = if assessment {
            fixture.request(1).wire
        } else {
            CaseRequest::prepare(&fixture.settings, 101).unwrap().wire
        };
        let mut service = fixture.start();
        let original = fixture.image();
        let replacement = foreign.image();
        let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
        socket.write_all(&wire).unwrap();
        socket.shutdown(Shutdown::Write).unwrap();
        socket.set_nonblocking(true).unwrap();
        service.checkpoint_with_clock(foreign.driver.supervisor_mut(), &mut no_clock);
        assert!(service.retired);
        assert_eq!(fixture.image(), original);
        assert_eq!(foreign.image(), replacement);
        assert_eq!(service.candidates, 1);
        let mut bytes = [0_u8; 64];
        match socket.read(&mut bytes) {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            result => panic!("replacement owner disclosed a case or label receipt: {result:?}"),
        }
    }
}

#[test]
fn wrong_case_audience_and_malformed_frames_spend_the_original_read_budget() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.settings.candidates = 3;
    let request = CaseRequest::prepare(&fixture.settings, 101).unwrap();
    let wire = String::from_utf8(request.wire.clone()).unwrap();
    let wrong_audience = wire.replacen("\"deployment\":101", "\"deployment\":999", 1);
    assert_ne!(wrong_audience, wire);
    let malformed = &request.wire[..request.wire.len() - 1];
    let mut service = fixture.start();
    let before = fixture.image();
    for (index, bytes) in [wrong_audience.as_bytes(), malformed]
        .into_iter()
        .enumerate()
    {
        let reply = fixture.exchange(&mut service, bytes, &mut no_clock);
        assert!(reply.is_empty());
        assert_eq!(service.candidates, (index + 1) as u32);
        assert_eq!(fixture.image(), before);
    }
    // A lawful final candidate remains available; neither earlier failure
    // retires the original role or replenishes the shared allowance.
    let reply = fixture.exchange(&mut service, &request.wire, &mut no_clock);
    assert!(case::validate(&request, &reply).unwrap());
    assert_eq!(service.candidates, 3);
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
    assert!(service.retired);
    assert_eq!(fixture.image(), before);
}

#[test]
fn a_partial_case_frame_discloses_nothing_and_yields_to_original_stop() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = CaseRequest::prepare(&fixture.settings, 101).unwrap();
    let mut service = fixture.start();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket
        .write_all(&request.wire[..request.wire.len() / 2])
        .unwrap();
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
    assert_eq!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .credibility_report()
            .unwrap()
            .pending_cases,
        1,
    );
    // Unconsumed prefix bytes can cause a reset rather than orderly EOF; both
    // terminal outcomes must carry no response payload.
    match socket.read(&mut bytes) {
        Ok(0) => {}
        Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
        result => panic!("stopped case reader disclosed or retained a connection: {result:?}"),
    }
}

#[test]
fn allowed_case_at_initial_authority_epoch_is_discoverable() {
    let mut fixture = Fixture::new();
    credibility::review(
        &mut fixture.driver.supervisor_mut().host_mut().unwrap(),
        1,
        Verdict::Allow,
    );
    {
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.epoch, 0);
        assert!(!host.evaluation_ticket(101).unwrap().case().stopped);
    }
    let read = CaseRequest::prepare(&fixture.settings, 101).unwrap();
    let mut service = fixture.start();
    let before = fixture.image();
    let response = fixture.exchange(&mut service, &read.wire, &mut no_clock);
    assert!(case::validate(&read, &response).unwrap());
    let parsed = parsed_case(&response);
    let knowledge = parsed.get("knowledge").unwrap();
    let basis = knowledge.get("basis").unwrap();
    assert_eq!(basis.get("authority_epoch").unwrap().as_str(), Some("0"));
    assert_eq!(fixture.image(), before);
}
