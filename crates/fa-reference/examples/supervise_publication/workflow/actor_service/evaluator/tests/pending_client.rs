//! The existing evaluator-case CLI discovers pending work with no round input
//! or operator endpoint. The bounded peer returns actual native observations;
//! deliberate wire mutations exercise validation before output.
use super::*;
use protocol::pending::{self, PendingRequest};
use std::os::unix::net::UnixListener;

fn native_response(
    fixture: &Fixture,
    continuation: Option<(u64, u64)>,
) -> (PendingRequest, Vec<u8>) {
    let request = PendingRequest::prepare(&fixture.settings, continuation).unwrap();
    let host = fixture.driver.supervisor().host().unwrap();
    let response = request
        .observe(&host, fixture.evaluator.as_ref().unwrap())
        .unwrap();
    (request, response)
}

fn exchange(
    fixture: &Fixture,
    name: &str,
    continuation: Option<(u64, u64)>,
    response: Vec<u8>,
) -> (Result<(), String>, Vec<u8>, Vec<u8>) {
    let socket = fixture.root.0.join(format!("{name}.sock"));
    let profile = fixture.root.0.join(format!("{name}.json"));
    fs::write(
        &profile,
        profile_json(&socket, &fixture.actor, fixture.actor.supervisor, 8),
    )
    .unwrap();
    let reader = super::case_client::peer(
        UnixListener::bind(&socket).unwrap(),
        fixture.actor.supervisor,
        Some(response),
    );
    let mut args = vec![
        "evaluator-case".to_owned(),
        profile.display().to_string(),
        "--pending".to_owned(),
    ];
    if let Some((revision, round)) = continuation {
        args.extend([
            "--evaluation-revision".to_owned(),
            revision.to_string(),
            "--after-round".to_owned(),
            round.to_string(),
        ]);
    }
    let mut output = Vec::new();
    let result = client::command(&args, &mut output);
    let received = reader.join().unwrap().unwrap();
    (result, output, received)
}

fn replace(wire: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = std::str::from_utf8(wire).unwrap();
    assert_eq!(
        text.matches(from).count(),
        1,
        "ambiguous pending-client mutation"
    );
    text.replacen(from, to, 1).into_bytes()
}

#[test]
fn pending_client_discovers_original_cases_and_validates_known_empty_without_round_input() {
    for recorded in [false, true] {
        let mut fixture = Fixture::new();
        if recorded {
            fixture.review(1);
        }
        let (request, response) = native_response(&fixture, None);
        let before = fixture.image();
        let (result, output, received) =
            exchange(&fixture, "initial-pending", None, response.clone());
        result.unwrap();
        assert_eq!(received, request.wire);
        let mut expected = response;
        expected.push(b'\n');
        assert_eq!(output, expected);
        assert!(pending::validate(&request, &output).unwrap());
        let parsed = pending::parse_response(&output).unwrap();
        let value = parsed.get("knowledge").unwrap().get("value").unwrap();
        assert_eq!(
            value.get("cases").unwrap().as_array().unwrap().len(),
            usize::from(recorded)
        );
        assert_eq!(value.get("complete").unwrap().as_bool(), Some(true));
        assert!(value.get("next_after_round").unwrap().is_null());
        assert_eq!(fixture.image(), before);
        assert_eq!(fixture.config.source.status().read_attempts, 0);
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    }
}

#[test]
fn pending_client_uses_explicit_discovered_cursor_without_automatic_followup() {
    let mut fixture = Fixture::with_case_capacity(32);
    for id in 1..=17 {
        fixture.review(id);
    }
    let (initial, initial_response) = native_response(&fixture, None);
    assert!(pending::validate(&initial, &initial_response).unwrap());
    let first = pending::parse_response(&initial_response).unwrap();
    let knowledge = first.get("knowledge").unwrap();
    let revision = knowledge
        .get("basis")
        .unwrap()
        .get("evaluation_revision")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let boundary = knowledge
        .get("value")
        .unwrap()
        .get("next_after_round")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let cursor = Some((revision, boundary));
    let (request, response) = native_response(&fixture, cursor);
    let before = fixture.image();
    let (result, output, received) = exchange(&fixture, "continued-pending", cursor, response);
    result.unwrap();
    assert_eq!(received, request.wire);
    assert!(pending::validate(&request, &output).unwrap());
    let parsed = pending::parse_response(&output).unwrap();
    let value = parsed.get("knowledge").unwrap().get("value").unwrap();
    let cases = value.get("cases").unwrap().as_array().unwrap();
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].get("round").unwrap().as_str(), Some("117"));
    assert_eq!(value.get("complete").unwrap().as_bool(), Some(true));
    assert_eq!(fixture.image(), before);
}

#[test]
fn pending_client_rejects_scope_inventory_cursor_label_and_authority_mutations_before_output() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let (request, response) = native_response(&fixture, None);
    assert!(pending::validate(&request, &response).unwrap());
    let before = fixture.image();
    for (index, (from, to)) in [
        ("fa.live-evaluator-pending/1", "fa.live-evaluator-case/1"),
        ("\"deployment\":101", "\"deployment\":102"),
        ("\"total_pending\":\"1\"", "\"total_pending\":\"0\""),
        ("\"total_pending\":\"1\"", "\"total_pending\":\"01\""),
        ("\"after_round\":null", "\"after_round\":\"100\""),
        ("\"complete\":true", "\"complete\":false"),
        ("\"status\":\"unassessed\"", "\"status\":\"benign\""),
        (
            "\"status\":\"unassessed\"",
            "\"status\":\"unassessed\",\"labels\":[]",
        ),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
    ]
    .into_iter()
    .enumerate()
    {
        let changed = replace(&response, from, to);
        let (result, output, received) =
            exchange(&fixture, &format!("invalid-pending-{index}"), None, changed);
        assert!(result.is_err(), "accepted mutation: {to}");
        assert!(
            output.is_empty(),
            "printed an unvalidated pending page: {to}"
        );
        assert_eq!(received, request.wire);
        assert_eq!(fixture.image(), before);
    }
}

#[test]
fn pending_client_prints_stale_cursor_unknown_once_without_retry_or_absence_claim() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let revision = fixture
        .driver
        .supervisor()
        .host()
        .unwrap()
        .credibility_report()
        .unwrap()
        .revision;
    fixture.review(2);
    let cursor = Some((revision, 101));
    let (request, response) = native_response(&fixture, cursor);
    assert!(!pending::validate(&request, &response).unwrap());
    let before = fixture.image();
    let (result, output, received) = exchange(&fixture, "stale-pending", cursor, response);
    assert!(result.unwrap_err().contains("stale or invalid"));
    assert_eq!(received, request.wire);
    assert!(!pending::validate(&request, &output).unwrap());
    let parsed = pending::parse_response(&output).unwrap();
    let knowledge = parsed.get("knowledge").unwrap();
    assert_eq!(knowledge.get("state").unwrap().as_str(), Some("unknown"));
    assert_eq!(
        knowledge.get("reason").unwrap().as_str(),
        Some("evaluation_revision_changed")
    );
    assert!(knowledge.get("value").is_none());
    assert!(knowledge.get("basis").is_none());
    assert_eq!(fixture.image(), before);
}

#[test]
fn pending_client_rejects_unpaired_ambiguous_and_noncanonical_cursors_before_profile_io() {
    let fixture = Fixture::new();
    let profile = fixture
        .root
        .0
        .join("missing-pending-profile.json")
        .display()
        .to_string();
    let initial = vec![
        "evaluator-case".to_owned(),
        profile.clone(),
        "--pending".to_owned(),
    ];
    let mut output = Vec::new();
    let missing = client::command(&initial, &mut output).unwrap_err();
    let complete = |revision: &str, round: &str| {
        vec![
            "evaluator-case".to_owned(),
            profile.clone(),
            "--pending".to_owned(),
            "--evaluation-revision".to_owned(),
            revision.to_owned(),
            "--after-round".to_owned(),
            round.to_owned(),
        ]
    };
    assert_eq!(
        client::command(
            &complete("18446744073709551615", "18446744073709551615"),
            &mut output
        )
        .unwrap_err(),
        missing
    );
    for invalid in ["0", "01", "-1", "+1", "1e2", "", "18446744073709551616"] {
        for args in [complete(invalid, "1"), complete("1", invalid)] {
            assert_ne!(client::command(&args, &mut output).unwrap_err(), missing);
        }
    }
    let mut wrong_order = complete("1", "2");
    wrong_order.swap(3, 5);
    let mut extra = complete("1", "2");
    extra.push("--all".into());
    let mut assessment = complete("1", "2");
    assessment[0] = "evaluator-assess".into();
    for args in [
        complete("1", "2")[..5].to_vec(),
        wrong_order,
        extra,
        assessment,
    ] {
        assert!(
            client::command(&args, &mut output)
                .unwrap_err()
                .starts_with("usage:")
        );
    }
    assert!(output.is_empty());
    assert!(!Path::new(&profile).exists());
}
