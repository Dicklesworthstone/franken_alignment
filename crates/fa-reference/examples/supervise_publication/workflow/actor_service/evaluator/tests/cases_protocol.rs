//! Closed discovery pages pair bounded progress with refusal of changed cuts,
//! skipped rows, fabricated continuations and private-data extensions.
use super::*;

fn settings() -> Settings {
    Settings::decode(
        br#"{
        "schema_version":"fa.native-evaluator-peer/1",
        "socket":"/tmp/fa-cases-protocol.sock",
        "scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5},
        "deployment":101,"evaluator":202,
        "supervisor":{"uid":1000,"gid":1000,"pid":100},
        "peer":{"uid":1000,"gid":1000,"pid":101},
        "candidate_limit":8,"runtime_ms":1000,"poll_ms":1
    }"#,
    )
    .unwrap()
}

fn cut() -> FileEvaluationCasesBasis {
    FileEvaluationCasesBasis {
        journal_revision: 31,
        evaluation_revision: 7,
    }
}

fn case(round: u64) -> Case {
    Case {
        round,
        attempt: round + 1000,
        policy: 2001,
        congress: 3001,
        root: [0xab; 32],
        status: FileEvaluationCaseStatus::Pending,
    }
}

fn first() -> (CasesRequest, Vec<u8>) {
    let request = CasesRequest::prepare(&settings(), 2, None, None).unwrap();
    let response = request
        .encode_page(
            cut(),
            &Page {
                total_cases: 3,
                remaining_cases: 3,
                unapplied_rounds: 1,
                cases: vec![case(101), case(102)],
                next_after: Some(102),
            },
        )
        .unwrap();
    (request, response)
}

fn continued() -> (CasesRequest, Vec<u8>) {
    let request = CasesRequest::prepare(&settings(), 2, Some(100), Some(cut())).unwrap();
    let response = request
        .encode_page(
            cut(),
            &Page {
                total_cases: 5,
                remaining_cases: 4,
                unapplied_rounds: 0,
                cases: vec![case(101), case(102)],
                next_after: Some(102),
            },
        )
        .unwrap();
    (request, response)
}

fn replace(wire: &[u8], from: &str, to: &str) -> Vec<u8> {
    let wire = std::str::from_utf8(wire).unwrap();
    assert_eq!(
        wire.matches(from).count(),
        1,
        "ambiguous listing mutation: {from}"
    );
    wire.replacen(from, to, 1).into_bytes()
}

#[test]
fn page_requests_preserve_full_width_cut_and_close_the_continuation_grammar() {
    let settings = settings();
    let expected = FileEvaluationCasesBasis {
        journal_revision: u64::MAX,
        evaluation_revision: u64::MAX,
    };
    let request = CasesRequest::prepare(&settings, 16, Some(u64::MAX), Some(expected)).unwrap();
    let accepted = CasesRequest::accept(&settings, &request.wire).unwrap();
    assert_eq!(accepted.after_round, Some(u64::MAX));
    assert_eq!(accepted.expected_basis, Some(expected));
    assert_eq!(accepted.limit, 16);
    assert_eq!(accepted.parsed, request.parsed);
    for (after, basis) in [(Some(1), None), (None, Some(cut())), (Some(0), Some(cut()))] {
        assert!(CasesRequest::prepare(&settings, 1, after, basis).is_err());
    }
    for limit in [0, 17, usize::MAX] {
        assert!(CasesRequest::prepare(&settings, limit, None, None).is_err());
    }
    let request = CasesRequest::prepare(&settings, 2, None, None).unwrap();
    for value in [
        "0",
        "03",
        "+1",
        "-1",
        "1.0",
        "1e1",
        "",
        "18446744073709551616",
    ] {
        assert!(
            CasesRequest::accept(
                &settings,
                &replace(
                    &request.wire,
                    "\"limit\":\"2\"",
                    &format!("\"limit\":\"{value}\""),
                )
            )
            .is_err()
        );
    }
    for replacement in [
        "\"limit\":2",
        "\"limit\":null",
        "\"limit\":\"2\",\"limit\":\"2\"",
        "\"limit\":\"2\",\"round\":\"101\"",
    ] {
        assert!(
            CasesRequest::accept(
                &settings,
                &replace(&request.wire, "\"limit\":\"2\"", replacement)
            )
            .is_err()
        );
    }
    for field in [
        "tenant",
        "principal",
        "run",
        "branch",
        "authority",
        "deployment",
        "evaluator",
    ] {
        let value = request
            .parsed
            .get("audience")
            .unwrap()
            .get(field)
            .unwrap()
            .as_u64()
            .unwrap();
        let changed = replace(
            &request.wire,
            &format!("\"{field}\":{value}"),
            &format!("\"{field}\":{}", value + 1),
        );
        assert!(
            CasesRequest::accept(&settings, &changed).is_err(),
            "accepted wrong {field}"
        );
    }
    assert!(super::super::case::CaseRequest::accept(&settings, &request.wire).is_err());
    assert!(super::super::Request::accept(&settings, &request.wire).is_err());
}

#[test]
fn maximum_page_and_padded_request_fit_original_transport_and_separate_parser_bounds() {
    let settings = settings();
    let request = CasesRequest::prepare(&settings, MAX_EVALUATION_CASE_PAGE, None, None).unwrap();
    let mut padded = request.wire.clone();
    padded.resize(CASES_REQUEST_BYTES, b' ');
    let request = CasesRequest::accept(&settings, &padded).unwrap();
    padded.push(b' ');
    assert!(CasesRequest::accept(&settings, &padded).is_err());
    let cases = (0..MAX_EVALUATION_CASE_PAGE)
        .map(|index| Case {
            round: u64::MAX - (MAX_EVALUATION_CASE_PAGE - 1 - index) as u64,
            attempt: u64::MAX,
            policy: u64::MAX,
            congress: u64::MAX,
            root: [0xff; 32],
            status: match index % 3 {
                0 => FileEvaluationCaseStatus::Pending,
                1 => FileEvaluationCaseStatus::Censored,
                _ => FileEvaluationCaseStatus::Resolved,
            },
        })
        .collect();
    let response = request
        .encode_page(
            FileEvaluationCasesBasis {
                journal_revision: u64::MAX,
                evaluation_revision: u64::MAX,
            },
            &Page {
                total_cases: MAX_EVALUATION_CASE_PAGE,
                remaining_cases: MAX_EVALUATION_CASE_PAGE,
                unapplied_rounds: MAX_EVALUATION_CASES - MAX_EVALUATION_CASE_PAGE,
                cases,
                next_after: None,
            },
        )
        .unwrap();
    assert!(response.len() <= RESPONSE_BYTES);
    assert!(validate(&request, &response).unwrap());
    // Existing scalar responses retain their smaller parser allowance.
    assert!(super::super::parse(&response, RESPONSE_BYTES).is_err());
    let value = parse(&response, RESPONSE_BYTES).unwrap();
    let cases = value
        .get("knowledge")
        .unwrap()
        .get("value")
        .unwrap()
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(cases.len(), MAX_EVALUATION_CASE_PAGE);
    assert!(
        cases
            .iter()
            .all(|case| case.as_object().unwrap().len() == 6)
    );
    assert_eq!(
        cases.last().unwrap().get("round").unwrap().as_str(),
        Some("18446744073709551615")
    );
}

#[test]
fn empty_known_inventory_preserves_zero_evaluation_revision_and_unapplied_rounds() {
    let request = CasesRequest::prepare(&settings(), 2, None, None).unwrap();
    for unapplied_rounds in [0, 1, MAX_EVALUATION_CASES] {
        let response = request
            .encode_page(
                FileEvaluationCasesBasis {
                    journal_revision: 1,
                    evaluation_revision: 0,
                },
                &Page {
                    total_cases: 0,
                    remaining_cases: 0,
                    unapplied_rounds,
                    cases: Vec::new(),
                    next_after: None,
                },
            )
            .unwrap();
        assert!(validate(&request, &response).unwrap());
    }
    let (request, response) = first();
    for evaluation in ["0", "2", "32"] {
        assert!(
            validate(
                &request,
                &replace(
                    &response,
                    "\"evaluation_revision\":\"7\"",
                    &format!("\"evaluation_revision\":\"{evaluation}\"")
                )
            )
            .is_err()
        );
    }
}

#[test]
fn page_validator_refuses_count_cursor_order_and_disclosure_contradictions() {
    let (request, response) = first();
    assert!(validate(&request, &response).unwrap());
    for (from, to) in [
        ("\"round\":\"102\"", "\"round\":\"101\""),
        ("\"round\":\"102\"", "\"round\":\"100\""),
        ("\"next_after\":\"102\"", "\"next_after\":\"101\""),
        ("\"next_after\":\"102\"", "\"next_after\":null"),
        ("\"next_after\":\"102\"", "\"next_after\":\"0\""),
        ("\"total_cases\":\"3\"", "\"total_cases\":\"1\""),
        ("\"total_cases\":\"3\"", "\"total_cases\":\"4\""),
        ("\"unapplied_rounds\":\"1\"", "\"unapplied_rounds\":\"510\""),
        ("\"attempt\":\"1101\"", "\"attempt\":\"01101\""),
        ("\"attempt\":\"1101\"", "\"attempt\":\"0\""),
        (
            "\"attempt\":\"1101\"",
            "\"attempt\":\"1101\",\"label\":\"benign\"",
        ),
        (
            "\"attempt\":\"1101\"",
            "\"attempt\":\"1101\",\"verdicts\":[]",
        ),
        (
            "\"attempt\":\"1101\"",
            "\"attempt\":\"1101\",\"payload\":\"private\"",
        ),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        (
            "\"effect_authority\":false",
            "\"effect_authority\":false,\"ticket\":\"invented\"",
        ),
        ("\"state\":\"known\"", "\"state\":\"absent\""),
        ("fa.live-evaluator-cases/1", "fa.live-evaluator-cases/2"),
    ] {
        assert!(
            validate(&request, &replace(&response, from, to)).is_err(),
            "accepted {to}"
        );
    }
    let text = String::from_utf8(response.clone()).unwrap();
    for changed in [
        text.replace("\"status\":\"pending\"", "\"status\":\"benign\""),
        text.replace(&"ab".repeat(32), &"00".repeat(32)),
        text.replace(&"ab".repeat(32), &"AB".repeat(32)),
    ] {
        assert!(validate(&request, changed.as_bytes()).is_err());
    }
    let another = CasesRequest::prepare(&settings(), 1, None, None).unwrap();
    assert!(validate(&another, &response).is_err());
    assert!(validate(&request, &response[..response.len() - 1]).is_err());
}

#[test]
fn continuation_pages_bind_both_revisions_and_exact_remaining_cardinality() {
    let (request, response) = continued();
    assert!(validate(&request, &response).unwrap());
    for (from, to) in [
        (
            "\"basis\":{\"journal_revision\":\"31\"",
            "\"basis\":{\"journal_revision\":\"32\"",
        ),
        (
            "\"basis\":{\"journal_revision\":\"31\",\"evaluation_revision\":\"7\"",
            "\"basis\":{\"journal_revision\":\"31\",\"evaluation_revision\":\"8\"",
        ),
        ("\"round\":\"101\"", "\"round\":\"100\""),
        ("\"next_after\":\"102\"", "\"next_after\":null"),
        ("\"remaining_cases\":\"4\"", "\"remaining_cases\":\"5\""),
        ("\"remaining_cases\":\"4\"", "\"remaining_cases\":\"1\""),
        ("\"total_cases\":\"5\"", "\"total_cases\":\"4\""),
    ] {
        assert!(
            validate(&request, &replace(&response, from, to)).is_err(),
            "accepted {to}"
        );
    }
    // Exactly a full final page is terminal; fullness alone does not mean more.
    let terminal = request
        .encode_page(
            cut(),
            &Page {
                total_cases: 3,
                remaining_cases: 2,
                unapplied_rounds: 0,
                cases: vec![case(101), case(102)],
                next_after: None,
            },
        )
        .unwrap();
    assert!(validate(&request, &terminal).unwrap());
    let exhausted = request
        .encode_page(
            cut(),
            &Page {
                total_cases: 1,
                remaining_cases: 0,
                unapplied_rounds: 0,
                cases: Vec::new(),
                next_after: None,
            },
        )
        .unwrap();
    assert!(validate(&request, &exhausted).unwrap());
}

#[test]
fn stale_or_missing_cursor_is_explicit_unknown_without_case_or_revision_disclosure() {
    let (request, _) = continued();
    for reason in ["stale_basis", "cursor_not_recorded"] {
        let response = request.encode_unknown(reason).unwrap();
        assert!(!validate(&request, &response).unwrap());
        let parsed = parse(&response, RESPONSE_BYTES).unwrap();
        let knowledge = parsed.get("knowledge").unwrap();
        assert_eq!(knowledge.as_object().unwrap().len(), 2);
        assert!(knowledge.get("basis").is_none());
        assert!(knowledge.get("value").is_none());
        let changed = replace(
            &response,
            &format!("\"reason\":\"{reason}\""),
            &format!("\"reason\":\"{reason}\",\"value\":[]"),
        );
        assert!(validate(&request, &changed).is_err());
    }
    let request = CasesRequest::prepare(&settings(), 1, None, None).unwrap();
    assert!(request.encode_unknown("stale_basis").is_err());
    assert!(request.encode_unknown("cursor_not_recorded").is_err());
}
