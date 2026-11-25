//! Synthetic protocol mutations are paired with valid complete pages. Native
//! service tests separately exercise the original ledger and evaluator custody.
use super::*;

fn settings() -> Settings {
    Settings::decode(
        br#"{
        "schema_version":"fa.native-evaluator-peer/1",
        "socket":"/tmp/fa-pending-protocol.sock",
        "scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5},
        "deployment":101,"evaluator":202,
        "supervisor":{"uid":1000,"gid":1000,"pid":100},
        "peer":{"uid":1000,"gid":1000,"pid":101},
        "candidate_limit":8,"runtime_ms":1000,"poll_ms":1
    }"#,
    )
    .unwrap()
}

fn identity(round: u64, status: &str) -> String {
    format!(
        "{{\"round\":\"{round}\",\"attempt\":\"77\",\"policy_generation\":\"8\",\"congress_generation\":\"9\",\"case_evidence_root_hex\":\"{}\",\"status\":\"{status}\"}}",
        "ab".repeat(32),
    )
}

fn page(
    request: &PendingRequest,
    evaluation: u64,
    total: usize,
    unfinished: usize,
    cases: &[String],
    next: Option<u64>,
) -> Vec<u8> {
    let after = optional_round(request.continuation.map(|(_, round)| round));
    let complete = next.is_none();
    let next = optional_round(next);
    let basis = Basis {
        journal: 123,
        evaluation,
        control: 0,
        epoch: 0,
    };
    format!(
        "{{\"schema_version\":\"fa.live-evaluator-pending/1\",\"request\":{},\"knowledge\":{{\"state\":\"known\",\"basis\":{},\"value\":{{\"after_round\":{after},\"total_pending\":\"{total}\",\"unfinished_reviews\":\"{unfinished}\",\"cases\":[{}],\"next_after_round\":{next},\"complete\":{complete}}}}},\"effect_authority\":false}}",
        std::str::from_utf8(&request.wire).unwrap(), basis.json(), cases.join(","),
    )
    .into_bytes()
}

fn replace(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = std::str::from_utf8(bytes).unwrap();
    assert_eq!(text.matches(from).count(), 1, "ambiguous mutation: {from}");
    text.replacen(from, to, 1).into_bytes()
}

#[test]
fn pending_request_binds_every_audience_field_and_an_explicit_exact_cursor() {
    let settings = settings();
    for continuation in [None, Some((1, 2)), Some((u64::MAX, u64::MAX))] {
        let request = PendingRequest::prepare(&settings, continuation).unwrap();
        let accepted = PendingRequest::accept(&settings, &request.wire).unwrap();
        assert_eq!(accepted.continuation, continuation);
        assert_eq!(accepted.parsed, request.parsed);
    }
    let request = PendingRequest::prepare(&settings, None).unwrap();
    let mut padded = request.wire.clone();
    padded.resize(PENDING_REQUEST_BYTES, b' ');
    assert_eq!(
        PendingRequest::accept(&settings, &padded).unwrap().parsed,
        request.parsed
    );
    padded.push(b' ');
    assert!(PendingRequest::accept(&settings, &padded).is_err());
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
            PendingRequest::accept(&settings, &changed).is_err(),
            "accepted wrong {field}"
        );
    }
    assert!(super::super::case::CaseRequest::accept(&settings, &request.wire).is_err());
    let exact = super::super::case::CaseRequest::prepare(&settings, 101).unwrap();
    assert!(PendingRequest::accept(&settings, &exact.wire).is_err());
    for cursor in [
        "{}",
        "false",
        "[]",
        "{\"evaluation_revision\":\"1\"}",
        "{\"evaluation_revision\":\"1\",\"after_round\":\"2\",\"limit\":16}",
        "{\"evaluation_revision\":\"1\",\"after_round\":\"2\",\"after_round\":\"2\"}",
    ] {
        assert!(
            PendingRequest::accept(
                &settings,
                &replace(
                    &request.wire,
                    "\"continuation\":null",
                    &format!("\"continuation\":{cursor}")
                )
            )
            .is_err()
        );
    }
    let request = PendingRequest::prepare(&settings, Some((7, 101))).unwrap();
    for (field, original) in [("evaluation_revision", "7"), ("after_round", "101")] {
        for invalid in [
            "0",
            "01",
            "-1",
            "+1",
            "1.0",
            "1e2",
            "",
            "18446744073709551616",
        ] {
            let from = format!("\"{field}\":\"{original}\"");
            let to = format!("\"{field}\":\"{invalid}\"");
            assert!(
                PendingRequest::accept(&settings, &replace(&request.wire, &from, &to)).is_err()
            );
        }
    }
}

#[test]
fn initial_pending_pages_cover_empty_unfinished_and_full_width_original_inventory() {
    let request = PendingRequest::prepare(&settings(), None).unwrap();
    // Beginning a review can leave the applied evaluation revision at zero.
    let empty = page(&request, 0, 0, 1, &[], None);
    assert!(validate(&request, &empty).unwrap());
    let cases = vec![identity(101, "unassessed"), identity(102, "censored")];
    let complete = page(&request, 3, 2, 1, &cases, None);
    assert!(validate(&request, &complete).unwrap());
    let cases: Vec<_> = (1..=MAX_PENDING_EVALUATION_PAGE as u64)
        .map(|round| {
            identity(u64::MAX - 100 + round, "unassessed")
                .replace("\"77\"", &format!("\"{}\"", u64::MAX))
                .replace("\"8\"", &format!("\"{}\"", u64::MAX))
                .replace("\"9\"", &format!("\"{}\"", u64::MAX))
        })
        .collect();
    let first = page(&request, u64::MAX, 17, 0, &cases, Some(u64::MAX - 84));
    assert!(validate(&request, &first).unwrap());
    assert!(first.len() < RESPONSE_BYTES);
    let mut padded = request.wire.clone();
    padded.resize(PENDING_REQUEST_BYTES, b' ');
    let padded = PendingRequest::accept(&settings(), &padded).unwrap();
    let full = page(&padded, u64::MAX, 17, 0, &cases, Some(u64::MAX - 84));
    assert!(validate(&padded, &full).unwrap());
    assert!(full.len() < RESPONSE_BYTES);
    let initial_epoch = replace(
        &complete,
        "\"authority_epoch\":\"0\"",
        "\"authority_epoch\":\"18446744073709551615\"",
    );
    assert!(validate(&request, &initial_epoch).unwrap());
    assert!(validate(&request, &page(&request, 0, 2, 0, &cases[..2], None)).is_err());
}

#[test]
fn pending_page_refuses_silent_omission_duplicate_support_and_invented_continuations() {
    let request = PendingRequest::prepare(&settings(), None).unwrap();
    let cases: Vec<_> = (101..117)
        .map(|round| identity(round, "unassessed"))
        .collect();
    let valid = page(&request, 17, 17, 0, &cases, Some(116));
    assert!(validate(&request, &valid).unwrap());
    for (from, to) in [
        ("\"total_pending\":\"17\"", "\"total_pending\":\"16\""),
        ("\"total_pending\":\"17\"", "\"total_pending\":\"513\""),
        (
            "\"unfinished_reviews\":\"0\"",
            "\"unfinished_reviews\":\"500\"",
        ),
        (
            "\"next_after_round\":\"116\"",
            "\"next_after_round\":\"115\"",
        ),
        ("\"next_after_round\":\"116\"", "\"next_after_round\":null"),
        ("\"complete\":false", "\"complete\":true"),
        ("\"complete\":false", "\"complete\":\"false\""),
        ("\"round\":\"102\"", "\"round\":\"101\""),
        ("\"round\":\"102\"", "\"round\":\"100\""),
    ] {
        assert!(
            validate(&request, &replace(&valid, from, to)).is_err(),
            "accepted {to}"
        );
    }
    assert!(validate(&request, &page(&request, 17, 17, 0, &cases, None)).is_err());
    assert!(
        validate(
            &request,
            &page(&request, 17, 17, 0, &cases[..15], Some(115))
        )
        .is_err()
    );
    let mut too_many = cases.clone();
    too_many.push(identity(117, "unassessed"));
    assert!(validate(&request, &page(&request, 17, 17, 0, &too_many, None)).is_err());
    let mut oversized = valid.clone();
    oversized.resize(RESPONSE_BYTES + 1, b' ');
    assert!(validate(&request, &oversized).is_err());
    assert!(validate(&request, &valid[..valid.len() - 1]).is_err());
}

#[test]
fn pending_continuation_pins_revision_lower_bound_and_minimal_disclosure() {
    let request = PendingRequest::prepare(&settings(), Some((8, 116))).unwrap();
    let valid = page(
        &request,
        8,
        18,
        0,
        &[identity(117, "censored"), identity(118, "unassessed")],
        None,
    );
    assert!(validate(&request, &valid).unwrap());
    let changes = [
        (
            "\"basis\":{\"journal_revision\":\"123\",\"evaluation_revision\":\"8\"",
            "\"basis\":{\"journal_revision\":\"123\",\"evaluation_revision\":\"9\"",
        ),
        (
            "\"value\":{\"after_round\":\"116\"",
            "\"value\":{\"after_round\":\"115\"",
        ),
        ("\"round\":\"117\"", "\"round\":\"116\""),
        ("\"status\":\"censored\"", "\"status\":\"benign\""),
        ("\"status\":\"censored\"", "\"status\":\"violation\""),
        (
            "\"status\":\"censored\"",
            "\"status\":\"censored\",\"origin\":1",
        ),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        (
            "\"effect_authority\":false",
            "\"effect_authority\":false,\"ticket\":\"invented\"",
        ),
    ];
    for (from, to) in changes {
        assert!(
            validate(&request, &replace(&valid, from, to)).is_err(),
            "accepted {to}"
        );
    }
    let other = PendingRequest::prepare(&settings(), Some((9, 116))).unwrap();
    assert!(validate(&other, &valid).is_err());
    let empty_suffix = page(&request, 8, 16, 0, &[], None);
    assert!(validate(&request, &empty_suffix).unwrap());
    assert!(validate(&request, &page(&request, 8, 0, 0, &[], None)).is_err());
}

#[test]
fn stale_pending_cursor_is_explicit_unknown_without_fabricated_basis_or_absence() {
    let request = PendingRequest::prepare(&settings(), Some((7, 101))).unwrap();
    for reason in ["evaluation_revision_changed", "invalid_continuation"] {
        let unknown = request.unknown(reason).unwrap();
        assert!(!validate(&request, &unknown).unwrap());
        let parsed = parse_response(&unknown).unwrap();
        let knowledge = parsed.get("knowledge").unwrap();
        assert_eq!(knowledge.as_object().unwrap().len(), 2);
        for (from, to) in [
            ("\"state\":\"unknown\"", "\"state\":\"absent\""),
            ("\"state\":\"unknown\"", "\"state\":\"known\""),
            (
                "\"state\":\"unknown\"",
                "\"state\":\"unknown\",\"basis\":null",
            ),
            (
                "\"state\":\"unknown\"",
                "\"state\":\"unknown\",\"value\":{\"cases\":[]}",
            ),
        ] {
            assert!(validate(&request, &replace(&unknown, from, to)).is_err());
        }
    }
    assert!(
        PendingRequest::prepare(&settings(), None)
            .unwrap()
            .unknown("evaluation_revision_changed")
            .is_err()
    );
    assert!(request.unknown("case_not_recorded").is_err());
}
