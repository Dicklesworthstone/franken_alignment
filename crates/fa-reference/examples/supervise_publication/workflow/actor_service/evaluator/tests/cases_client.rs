//! The discovery client authenticates before sending, validates before output,
//! and never retries a stale continuation or interprets Unknown as an empty page.
use super::case_client::peer;
use super::cases_service::{page_basis, parsed_page};
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileEvaluationCasesBasis;
use protocol::cases::{self, CasesRequest};
use std::os::unix::net::UnixListener;

fn native_response(
    fixture: &Fixture,
    limit: usize,
    after: Option<u64>,
    basis: Option<FileEvaluationCasesBasis>,
) -> (CasesRequest, Vec<u8>) {
    let request = CasesRequest::prepare(&fixture.settings, limit, after, basis).unwrap();
    let host = fixture.driver.supervisor().host().unwrap();
    let response = request
        .observe(&host, fixture.evaluator.as_ref().unwrap())
        .unwrap();
    (request, response)
}

fn arguments(profile: &Path, request: &CasesRequest) -> Vec<String> {
    let mut args = vec![
        "evaluator-case".to_owned(),
        profile.display().to_string(),
        "--list".to_owned(),
        request.limit.to_string(),
    ];
    if let (Some(after), Some(basis)) = (request.after_round, request.expected_basis) {
        args.extend([
            "--after".to_owned(),
            after.to_string(),
            "--at-journal".to_owned(),
            basis.journal_revision.to_string(),
            "--at-evaluation".to_owned(),
            basis.evaluation_revision.to_string(),
        ]);
    }
    args
}

fn exchange(
    fixture: &Fixture,
    name: &str,
    request: &CasesRequest,
    response: Vec<u8>,
) -> (Result<(), String>, Vec<u8>, Vec<u8>) {
    let socket = fixture.root.0.join(format!("{name}.sock"));
    let profile = fixture.root.0.join(format!("{name}.json"));
    fs::write(
        &profile,
        profile_json(&socket, &fixture.actor, fixture.actor.supervisor, 8),
    )
    .unwrap();
    let reader = peer(
        UnixListener::bind(&socket).unwrap(),
        fixture.actor.supervisor,
        Some(response),
    );
    let mut output = Vec::new();
    let result = client::command(&arguments(&profile, request), &mut output);
    let received = reader.join().unwrap().unwrap();
    (result, output, received)
}

fn replace(wire: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = std::str::from_utf8(wire).unwrap();
    assert_eq!(
        text.matches(from).count(),
        1,
        "ambiguous client page mutation"
    );
    text.replacen(from, to, 1).into_bytes()
}

#[test]
fn case_listing_client_returns_original_pages_and_explicit_terminal_inventory() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let (request, response) = native_response(&fixture, 1, None, None);
    let before = fixture.image();
    let (result, output, received) = exchange(&fixture, "list-first", &request, response.clone());
    result.unwrap();
    assert_eq!(received, request.wire);
    let mut expected = response;
    expected.push(b'\n');
    assert_eq!(output, expected);
    assert!(cases::validate(&request, &output).unwrap());
    let basis = page_basis(&output);
    let parsed = parsed_page(&output);
    let page = parsed.get("knowledge").unwrap().get("value").unwrap();
    let after = page
        .get("next_after")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let (next, response) = native_response(&fixture, 1, Some(after), Some(basis));
    let (result, output, received) = exchange(&fixture, "list-next", &next, response);
    result.unwrap();
    assert_eq!(received, next.wire);
    assert!(cases::validate(&next, &output).unwrap());
    assert_eq!(page_basis(&output), basis);
    let parsed = parsed_page(&output);
    let page = parsed.get("knowledge").unwrap().get("value").unwrap();
    assert!(page.get("next_after").unwrap().is_null());
    assert_eq!(
        page.get("cases").unwrap().as_array().unwrap()[0]
            .get("round")
            .unwrap()
            .as_str(),
        Some("102")
    );
    assert_eq!(fixture.image(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let host = fixture.driver.supervisor().host().unwrap();
    assert_eq!(host.credibility_report().unwrap().pending_cases, 2);
    assert!(host.evaluation_history(101).unwrap().is_empty());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn case_listing_client_refuses_cross_request_changed_cut_and_private_extensions_before_output() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let (first, first_response) = native_response(&fixture, 1, None, None);
    let basis = page_basis(&first_response);
    let (request, response) = native_response(&fixture, 1, Some(101), Some(basis));
    let before = fixture.image();
    let changed_basis = format!(
        "\"basis\":{{\"journal_revision\":\"{}\"",
        basis.journal_revision
    );
    let replaced_basis = format!(
        "\"basis\":{{\"journal_revision\":\"{}\"",
        basis.journal_revision + 1
    );
    let mutations = vec![
        replace(&response, "\"deployment\":101", "\"deployment\":999"),
        replace(&response, &changed_basis, &replaced_basis),
        replace(&response, "\"round\":\"102\"", "\"round\":\"101\""),
        replace(&response, "\"next_after\":null", "\"next_after\":\"102\""),
        replace(&response, "\"attempt\":\"2\"", "\"attempt\":\"02\""),
        replace(
            &response,
            "\"attempt\":\"2\"",
            "\"attempt\":\"2\",\"truth\":\"benign\"",
        ),
        replace(
            &response,
            "\"effect_authority\":false",
            "\"effect_authority\":true",
        ),
        first_response,
    ];
    for (index, mutated) in mutations.into_iter().enumerate() {
        let (result, output, received) = exchange(
            &fixture,
            &format!("list-mutated-{index}"),
            &request,
            mutated,
        );
        assert!(result.is_err(), "accepted mutated page {index}");
        assert!(output.is_empty(), "printed unvalidated page {index}");
        assert_eq!(received, request.wire);
        assert_eq!(fixture.image(), before);
    }
    let (result, output, _) = exchange(&fixture, "list-still-valid", &request, response);
    result.unwrap();
    assert!(cases::validate(&request, &output).unwrap());
    assert!(first.expected_basis.is_none());
    assert_eq!(fixture.image(), before);
}

#[test]
fn stale_listing_client_reports_unknown_without_an_automatic_restart() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let (_, response) = native_response(&fixture, 1, None, None);
    let basis = page_basis(&response);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let (request, response) = native_response(&fixture, 1, Some(101), Some(basis));
    assert!(!cases::validate(&request, &response).unwrap());
    let before = fixture.image();
    let (result, output, received) = exchange(&fixture, "list-stale", &request, response.clone());
    assert!(result.unwrap_err().contains("case listing is unknown"));
    assert_eq!(received, request.wire);
    let mut expected = response;
    expected.push(b'\n');
    assert_eq!(output, expected);
    assert!(!cases::validate(&request, &output).unwrap());
    assert_eq!(fixture.image(), before);
}

#[test]
fn listing_client_authenticates_before_any_request_bytes() {
    let fixture = Fixture::new();
    let mut expected_server = fixture.actor.clone();
    expected_server.supervisor = PeerPolicy::new(
        fixture.actor.supervisor.uid(),
        fixture.actor.supervisor.gid(),
        Some(std::process::id() + 3),
    )
    .unwrap();
    let socket = fixture.root.0.join("list-wrong-server.sock");
    let profile = fixture.root.0.join("list-wrong-server.json");
    fs::write(
        &profile,
        profile_json(&socket, &expected_server, fixture.actor.supervisor, 8),
    )
    .unwrap();
    let reader = peer(
        UnixListener::bind(&socket).unwrap(),
        fixture.actor.supervisor,
        None,
    );
    let request = CasesRequest::prepare(&fixture.settings, 16, None, None).unwrap();
    let mut output = Vec::new();
    let error = client::command(&arguments(&profile, &request), &mut output).unwrap_err();
    assert!(error.contains("unverified supervisor"));
    assert!(reader.join().unwrap().unwrap().is_empty());
    assert!(output.is_empty());
}

#[test]
fn listing_cli_closes_limits_continuations_and_order_before_profile_io() {
    let fixture = Fixture::new();
    let missing = fixture.root.0.join("missing-list-profile.json");
    let mut output = Vec::new();
    let request = CasesRequest::prepare(&fixture.settings, 1, None, None).unwrap();
    let first = arguments(&missing, &request);
    let missing_error = client::command(&first, &mut output).unwrap_err();
    let continued = CasesRequest::prepare(
        &fixture.settings,
        16,
        Some(u64::MAX),
        Some(FileEvaluationCasesBasis {
            journal_revision: u64::MAX,
            evaluation_revision: 0,
        }),
    )
    .unwrap();
    let complete = arguments(&missing, &continued);
    assert_eq!(
        client::command(&complete, &mut output).unwrap_err(),
        missing_error
    );
    for (position, invalid) in [
        (3, "0"),
        (3, "17"),
        (3, "01"),
        (3, "+1"),
        (3, ""),
        (5, "0"),
        (5, "01"),
        (7, "0"),
        (7, "1e3"),
        (9, "01"),
        (9, "18446744073709551616"),
        (4, "--at-journal"),
        (6, "--after"),
        (8, "--unknown"),
    ] {
        let mut args = complete.clone();
        args[position] = invalid.to_owned();
        assert_ne!(
            client::command(&args, &mut output).unwrap_err(),
            missing_error,
            "invalid argument reached profile: {position}={invalid}"
        );
    }
    for length in [3, 5, 6, 7, 8, 9] {
        assert!(
            client::command(&complete[..length], &mut output)
                .unwrap_err()
                .starts_with("usage:")
        );
    }
    let mut extra = complete;
    extra.push("extra".to_owned());
    assert!(
        client::command(&extra, &mut output)
            .unwrap_err()
            .starts_with("usage:")
    );
    let mut assess = first;
    assess[0] = "evaluator-assess".to_owned();
    assert!(
        client::command(&assess, &mut output)
            .unwrap_err()
            .starts_with("usage:")
    );
    assert!(output.is_empty());
    assert!(!missing.exists());
}
