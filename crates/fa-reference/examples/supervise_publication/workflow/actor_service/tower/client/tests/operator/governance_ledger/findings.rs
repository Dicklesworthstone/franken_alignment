//! Reuse actual independent-review fixtures and the authenticated command path;
//! a selected Findings query consumes the same original allowance as Ledger.
use super::super::super::super::governance_findings as findings_validation;
use super::*;

fn findings_request() -> TowerRequest {
    let mut query = request();
    query.verb = Verb::Findings;
    query.operation_id = "original-incident-findings".into();
    query
}

fn accepted_findings(bytes: &str, query: &TowerRequest) -> Json {
    let parsed = parse_response(bytes.as_bytes(), query).unwrap();
    findings_validation::check_profile(&parsed, query, native::profile().delivery.scope).unwrap();
    parsed
}

#[test]
fn authenticated_ledger_and_findings_share_one_budget_and_preserve_original_incident_support() {
    let root = Directory::new();
    let profile = settings(&root, 2, 4);
    let expected = [request(), findings_request(), findings_request()];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, request)| root.document(&format!("governance-{index}.json"), request))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeLedger::new(2, Fixture::Labelled);
        let before = native.supervisor.host().unwrap().inspect();
        let bytes = native.image();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for (index, query) in expected.iter().enumerate() {
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), *query);
            let reply = native.response(query, true);
            if index == 0 {
                accepted(&reply, query);
            } else {
                let parsed = accepted_findings(&reply, query);
                if index == 1 {
                    let incident =
                        &field(ledger(&parsed), &["andon_incidents", "value", "incidents"])
                            .as_array()
                            .unwrap()[0];
                    assert_eq!(
                        field(incident, &["support_rounds"]),
                        &Json::Array(vec![Json::String("101".into()), Json::String("102".into())])
                    );
                    assert_eq!(
                        field(incident, &["total_cost"]).as_str(),
                        Some("36893488147419103230")
                    );
                    let coverage = field(ledger(&parsed), &["risk_theater"])
                        .as_object()
                        .unwrap();
                    assert_eq!(coverage.len(), 6);
                    for value in coverage.values() {
                        assert_eq!(field(value, &["state"]).as_str(), Some("unknown"));
                    }
                } else {
                    assert_eq!(
                        field(body(&parsed), &["error", "code"]).as_str(),
                        Some("capacity")
                    );
                }
            }
            assert_eq!(native.remaining(), usize::from(index == 0));
            socket.write_all(format!("{reply}\n").as_bytes()).unwrap();
            replies.push(reply);
        }
        assert_eq!(
            socket.read(&mut [0_u8; 1]).unwrap(),
            0,
            "unsupplied follow-up"
        );
        assert_eq!(native.supervisor.host().unwrap().inspect(), before);
        assert_eq!(native.image(), bytes);
        replies
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn unavailable_source_and_installed_empty_incidents_have_different_knowledge() {
    let query = findings_request();
    for fixture in [Fixture::Absent, Fixture::Empty] {
        let mut native = NativeLedger::new(1, fixture);
        let response = native.response(&query, true);
        let parsed = accepted_findings(&response, &query);
        let values = ledger(&parsed);
        for name in ["credibility_incident", "andon_incidents"] {
            let status = field(values, &[name, "state"]).as_str();
            if matches!(fixture, Fixture::Absent) {
                assert_eq!(status, Some("unknown"));
                assert_eq!(
                    field(values, &[name, "reason"]).as_str(),
                    Some("source_not_installed")
                );
            } else {
                assert_eq!(status, Some("known"));
            }
        }
        if matches!(fixture, Fixture::Empty) {
            assert!(
                field(values, &["andon_incidents", "value", "incidents"])
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

#[test]
fn findings_envelope_dependencies_match_the_original_native_result() {
    let mut native = NativeLedger::new(1, Fixture::Labelled);
    let query = findings_request();
    let requested = format!("[\"{}\"]", query.address.canonical_string());
    for exhausted in [false, true] {
        let original = native.response(&query, true);
        let parsed = accepted_findings(&original, &query);
        assert_eq!(
            field(&parsed, &["envelope", "status"]).as_str(),
            Some(if exhausted { "error" } else { "ok" })
        );
        let (expected, changed) = if exhausted {
            (requested.as_str(), "[]")
        } else {
            ("[]", requested.as_str())
        };
        for field in ["missing", "would_change_this"] {
            let from = format!("\"{field}\":{expected}");
            let to = format!("\"{field}\":{changed}");
            let forged = original.replacen(&from, &to, 1);
            assert_ne!(forged, original, "mutation missed {field}");
            assert!(
                validate_response(forged.as_bytes(), &query).is_err(),
                "accepted altered {field} for exhausted={exhausted}"
            );
        }
    }
}

#[test]
fn client_rejects_invented_query_coverage_erased_support_and_inconsistent_incident_accounting() {
    let mut native = NativeLedger::new(1, Fixture::Labelled);
    let query = findings_request();
    let original = native.response(&query, true);
    accepted_findings(&original, &query);
    for (from, to) in [
        ("fa.governance-findings/1", "fa.governance-findings/2"),
        ("\"state\":\"Unknown\"", "\"state\":\"Known\""),
        (
            "\"type\":\"SemanticGeneration\"",
            "\"type\":\"SignedGlobalGeneration\"",
        ),
        ("\"reason\":\"Unavailable\"", "\"reason\":\"Incomplete\""),
        ("\"operation\":\"findings\"", "\"operation\":\"ledger\""),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        (
            "\"query_address\":\"fa://1/run/3@11\"",
            "\"query_address\":\"fa://1/run/9@11\"",
        ),
        (
            "\"consequence_drift\":{\"state\":\"unknown\",\"reason\":\"lower_layer_unverified\"},",
            "",
        ),
        (
            "\"andon_closure_mode\":{\"state\":\"unknown\"",
            "\"andon_closure_mode\":{\"state\":\"known\"",
        ),
        (
            "\"coverage_erosion\":{\"state\":\"unknown\"",
            "\"coverage_erosion\":{\"state\":\"absent\"",
        ),
        (
            "\"reason\":\"lower_layer_unverified\"",
            "\"reason\":\"clean\"",
        ),
        (
            "\"support_rounds\":[\"101\",\"102\"]",
            "\"support_rounds\":[\"102\"]",
        ),
        (
            "\"support_rounds\":[\"101\",\"102\"]",
            "\"support_rounds\":[\"101\",\"101\"]",
        ),
        (
            "\"support_rounds\":[\"101\",\"102\"]",
            "\"support_rounds\":[\"102\",\"101\"]",
        ),
        ("\"opened_by_round\":\"102\"", "\"opened_by_round\":\"103\""),
        ("\"inspected_windows\":\"1\"", "\"inspected_windows\":\"0\""),
        (
            "\"lifetime_false_stops\":\"2\"",
            "\"lifetime_false_stops\":\"1\"",
        ),
        (
            "\"calibration_incident_open\":true",
            "\"calibration_incident_open\":false",
        ),
        (
            "\"total_cost\":\"36893488147419103230\"",
            "\"total_cost\":\"36893488147419103229\"",
        ),
        (
            "\"total_cost\":\"36893488147419103230\"",
            "\"total_cost\":\"340282366920938463463374607431768211456\"",
        ),
        ("\"window\":\"1\"", "\"window\":1"),
        ("\"window\":\"1\"", "\"window\":\"01\""),
        (
            "\"false_stops\":\"2\"",
            "\"false_stops\":\"2\",\"policy_relaxed\":true",
        ),
        ("\"principal\":\"2\"", "\"principal\":\"22\""),
    ] {
        let changed = original.replacen(from, to, 1);
        assert_ne!(changed, original, "mutation missed {from}");
        assert!(
            validate_response(changed.as_bytes(), &query).is_err(),
            "accepted {to}"
        );
    }
}

#[test]
fn invalid_or_foreign_findings_emit_nothing_and_prevent_later_requests() {
    for mutation in [
        "missing_family",
        "foreign_scope",
        "lost_support",
        "false_clean",
    ] {
        let root = Directory::new();
        let profile = settings(&root, 2, 4);
        let query = findings_request();
        let documents = [
            root.document("findings.json", &query),
            root.document("later.json", &query),
        ];
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut native = NativeLedger::new(2, Fixture::Labelled);
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
            let original = native.response(&query, true);
            accepted_findings(&original, &query);
            let changed = match mutation {
                "missing_family" => original.replace(
                    "\"consequence_drift\":{\"state\":\"unknown\",\"reason\":\"lower_layer_unverified\"},",
                    "",
                ),
                "foreign_scope" => original.replace(
                    "\"principal\":\"2\"", "\"principal\":\"22\""
                ),
                "lost_support" => original.replace(
                    "\"support_rounds\":[\"101\",\"102\"]", "\"support_rounds\":[\"102\"]"
                ),
                _ => original.replace(
                    "\"andon_closure_mode\":{\"state\":\"unknown\"",
                    "\"andon_closure_mode\":{\"state\":\"absent\"",
                ),
            };
            assert_ne!(changed, original);
            if mutation == "foreign_scope" {
                let parsed = parse_response(changed.as_bytes(), &query).unwrap();
                assert!(
                    findings_validation::check_profile(
                        &parsed,
                        &query,
                        native::profile().delivery.scope
                    )
                    .is_err()
                );
            } else {
                assert!(validate_response(changed.as_bytes(), &query).is_err());
            }
            socket.write_all(format!("{changed}\n").as_bytes()).unwrap();
            assert_eq!(
                socket.read(&mut [0_u8; 1]).unwrap(),
                0,
                "sent another request after {mutation}"
            );
            assert_eq!(native.remaining(), 1);
        });
        let mut output = Vec::new();
        assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
        assert!(output.is_empty());
        server.join().unwrap();
    }
}

#[test]
fn findings_enforces_global_support_across_incidents_without_charging_unshown_windows() {
    let mut native = NativeLedger::new(1, Fixture::AdjacentWindows);
    let query = findings_request();
    let original = native.response(&query, true);
    let parsed = accepted_findings(&original, &query);
    let value = field(ledger(&parsed), &["andon_incidents", "value"]);
    assert_eq!(field(value, &["inspected_windows"]).as_str(), Some("3"));
    assert_eq!(field(value, &["incidents"]).as_array().unwrap().len(), 2);

    // Move only a non-opening round into the other incident's support. Both
    // per-window lists stay ordered, with the correct length and opening round.
    for (from, to) in [
        (
            r#""support_rounds":["101","102"]"#,
            r#""support_rounds":["101","103"]"#,
        ),
        (r#""start":"2""#, r#""start":"1""#),
        (
            r#""lifetime_false_stops":"4""#,
            r#""lifetime_false_stops":"2""#,
        ),
        (
            r#""lifetime_false_stops":"4""#,
            r#""lifetime_false_stops":"513""#,
        ),
    ] {
        let changed = original.replacen(from, to, 1);
        assert_ne!(original, changed);
        assert!(
            parse_response(changed.as_bytes(), &query).is_err(),
            "accepted {to}"
        );
    }
}
