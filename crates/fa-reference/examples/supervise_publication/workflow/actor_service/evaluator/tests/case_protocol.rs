//! Causal wire-boundary checks for case discovery. Native observation and
//! transport tests separately exercise real review histories and role custody.
use super::*;

fn settings() -> Settings {
    Settings::decode(br#"{
        "schema_version":"fa.native-evaluator-peer/1",
        "socket":"/tmp/fa-case-protocol.sock",
        "scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5},
        "deployment":101,"evaluator":202,
        "supervisor":{"uid":1000,"gid":1000,"pid":100},
        "peer":{"uid":1000,"gid":1000,"pid":101},
        "candidate_limit":8,"runtime_ms":1000,"poll_ms":1
    }"#).unwrap()
}

fn known() -> (CaseRequest, Vec<u8>) {
    let request = CaseRequest::prepare(&settings(), 101).unwrap();
    let response = request
        .encode(&Knowledge::Known {
            value: CaseIdentity {
                round: 101,
                attempt: 1001,
                policy: 1002,
                congress: 1003,
                root: [0xab; 32],
            },
            basis: Basis {
                journal: 10,
                evaluation: 2,
                control: 3,
                epoch: 6,
            },
        })
        .unwrap();
    (request, response)
}

fn replace(wire: &[u8], from: &str, to: &str) -> Vec<u8> {
    let wire = std::str::from_utf8(wire).unwrap();
    assert_eq!(wire.matches(from).count(), 1, "ambiguous mutation: {from}");
    wire.replacen(from, to, 1).into_bytes()
}

#[test]
fn exact_case_requests_bind_the_complete_evaluator_audience_and_full_width_round() {
    let settings = settings();
    for round in [1, 101, u64::MAX] {
        let prepared = CaseRequest::prepare(&settings, round).unwrap();
        let accepted = CaseRequest::accept(&settings, &prepared.wire).unwrap();
        assert_eq!(accepted.round, round);
        assert_eq!(accepted.parsed, prepared.parsed);
        assert_eq!(accepted.parsed.get("round").unwrap().as_str(), Some(round.to_string().as_str()));
    }
    assert!(CaseRequest::prepare(&settings, 0).is_err());

    let request = CaseRequest::prepare(&settings, 101).unwrap();
    for field in ["tenant", "principal", "run", "branch", "authority", "deployment", "evaluator"] {
        let value = request.parsed.get("audience").unwrap().get(field).unwrap().as_u64().unwrap();
        let changed = replace(&request.wire, &format!("\"{field}\":{value}"), &format!("\"{field}\":{}", value + 1));
        assert!(CaseRequest::accept(&settings, &changed).is_err(), "accepted wrong {field}");
    }
}

#[test]
fn case_requests_refuse_ambiguous_numeric_duplicate_and_cross_operation_inputs() {
    let settings = settings();
    let request = CaseRequest::prepare(&settings, 101).unwrap();
    for round in ["0", "01", "+1", "-1", "1.0", "1e2", " 101", "", "18446744073709551616"] {
        let changed = replace(&request.wire, "\"round\":\"101\"", &format!("\"round\":\"{round}\""));
        assert!(CaseRequest::accept(&settings, &changed).is_err(), "accepted {round}");
    }
    for fields in [
        "\"round\":101",
        "\"round\":null",
        "\"round\":true",
        "\"round\":\"101\",\"round\":\"101\"",
        "\"round\":\"101\",\"assessment_hex\":\"\"",
    ] {
        let changed = replace(&request.wire, "\"round\":\"101\"", fields);
        assert!(CaseRequest::accept(&settings, &changed).is_err(), "accepted {fields}");
    }
    let changed = replace(&request.wire, "fa.native-evaluator-case-request/1", "fa.native-evaluator-request/1");
    assert!(CaseRequest::accept(&settings, &changed).is_err());
    assert!(CaseRequest::accept(&settings, &request.wire[..request.wire.len() - 1]).is_err());
    let mut oversized = request.wire.clone();
    oversized.resize(REQUEST_BYTES + 1, b' ');
    assert!(CaseRequest::accept(&settings, &oversized).is_err());
}

#[test]
fn known_case_results_preserve_full_width_identities_and_one_exact_basis() {
    let request = CaseRequest::prepare(&settings(), u64::MAX).unwrap();
    let response = request.encode(&Knowledge::Known {
        value: CaseIdentity {
            round: u64::MAX,
            attempt: u64::MAX,
            policy: u64::MAX,
            congress: u64::MAX,
            root: [1; 32],
        },
        basis: Basis {
            journal: u64::MAX,
            evaluation: u64::MAX,
            control: u64::MAX,
            epoch: u64::MAX,
        },
    }).unwrap();
    assert!(validate(&request, &response).unwrap());
    let parsed = parse(&response, RESPONSE_BYTES).unwrap();
    let knowledge = parsed.get("knowledge").unwrap();
    assert_eq!(knowledge.get("state").unwrap().as_str(), Some("known"));
    let value = knowledge.get("value").unwrap();
    for field in ["round", "attempt", "policy_generation", "congress_generation"] {
        assert_eq!(value.get(field).unwrap().as_str(), Some("18446744073709551615"));
    }
    let basis = knowledge.get("basis").unwrap();
    for field in ["journal_revision", "evaluation_revision", "control_sequence", "authority_epoch"] {
        assert_eq!(basis.get(field).unwrap().as_str(), Some("18446744073709551615"));
    }
    assert_eq!(value.as_object().unwrap().len(), 5);
    assert_eq!(parsed.get("effect_authority").unwrap().as_bool(), Some(false));
}

#[test]
fn case_validator_refuses_relabeling_and_authority_or_payload_extensions() {
    let (request, response) = known();
    assert!(validate(&request, &response).unwrap());
    let another = CaseRequest::prepare(&settings(), 102).unwrap();
    assert!(validate(&another, &response).is_err());
    for (from, to) in [
        ("fa.live-evaluator-case/1", "fa.live-evaluator-case/2"),
        ("\"value\":{\"round\":\"101\"", "\"value\":{\"round\":\"102\""),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        ("\"effect_authority\":false", "\"effect_authority\":\"false\""),
        ("\"effect_authority\":false", "\"effect_authority\":false,\"ticket\":\"invented\""),
        ("\"attempt\":\"1001\"", "\"attempt\":\"1001\",\"verdicts\":[]"),
        ("\"attempt\":\"1001\"", "\"attempt\":\"1001\",\"payload\":\"private\""),
        ("\"evaluation_revision\":\"2\"", "\"evaluation_revision\":\"2\",\"fresh_time\":true"),
    ] {
        assert!(validate(&request, &replace(&response, from, to)).is_err(), "accepted {to}");
    }
    let altered_request = replace(&response, "\"deployment\":101", "\"deployment\":102");
    assert!(validate(&request, &altered_request).is_err());
}

#[test]
fn case_validator_refuses_noncanonical_case_and_basis_numbers() {
    let (request, response) = known();
    for (field, value) in [
        ("attempt", "1001"),
        ("policy_generation", "1002"),
        ("congress_generation", "1003"),
        ("journal_revision", "10"),
        ("evaluation_revision", "2"),
        ("control_sequence", "3"),
        ("authority_epoch", "6"),
    ] {
        let from = format!("\"{field}\":\"{value}\"");
        for invalid in ["01", "-1", "+1", "1.0", "1e1", "", "18446744073709551616"] {
            let to = format!("\"{field}\":\"{invalid}\"");
            assert!(validate(&request, &replace(&response, &from, &to)).is_err(), "accepted {to}");
        }
        assert!(validate(&request, &replace(&response, &from, &format!("\"{field}\":{value}"))).is_err());
        if !matches!(field, "control_sequence" | "authority_epoch") {
            assert!(validate(&request, &replace(&response, &from, &format!("\"{field}\":\"0\""))).is_err());
        }
    }
}

#[test]
fn case_evidence_identity_requires_nonzero_exact_lowercase_bytes() {
    let (request, response) = known();
    let original = "ab".repeat(32);
    for invalid in [
        "00".repeat(32),
        "AB".repeat(32),
        "ab".repeat(31),
        "ab".repeat(33),
        "gg".repeat(32),
        String::new(),
    ] {
        assert!(validate(&request, &replace(&response, &original, &invalid)).is_err(), "accepted digest {invalid}");
    }
    let replacement = format!("\"case_evidence_root_hex\":\"{original}\"");
    assert!(validate(&request, &replace(&response, &replacement, "\"case_evidence_root_hex\":null")).is_err());
}

#[test]
fn unknown_case_results_cannot_claim_absence_or_carry_fabricated_native_data() {
    let request = CaseRequest::prepare(&settings(), 101).unwrap();
    let response = request.encode(&Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }).unwrap();
    assert!(!validate(&request, &response).unwrap());
    let parsed = parse(&response, RESPONSE_BYTES).unwrap();
    let knowledge = parsed.get("knowledge").unwrap();
    assert_eq!(knowledge.as_object().unwrap().len(), 2);
    assert!(knowledge.get("basis").is_none());
    assert!(knowledge.get("value").is_none());
    for (from, to) in [
        ("\"state\":\"unknown\"", "\"state\":\"absent\""),
        ("\"state\":\"unknown\"", "\"state\":\"known\""),
        ("\"reason\":\"case_not_recorded\"", "\"reason\":\"not_executed\""),
        ("\"reason\":\"case_not_recorded\"", "\"reason\":\"case_not_recorded\",\"basis\":null"),
        ("\"reason\":\"case_not_recorded\"", "\"reason\":\"case_not_recorded\",\"value\":{}"),
    ] {
        assert!(validate(&request, &replace(&response, from, to)).is_err(), "accepted {to}");
    }
    let mut oversized = response.clone();
    oversized.resize(RESPONSE_BYTES + 1, b' ');
    assert!(validate(&request, &oversized).is_err());
    assert!(validate(&request, &response[..response.len() - 1]).is_err());
}

#[test]
fn recorded_cases_may_retain_the_initial_zero_authority_epoch() {
    let (request, response) = known();
    let initial = replace(&response, "\"authority_epoch\":\"6\"", "\"authority_epoch\":\"0\"");
    assert!(validate(&request, &initial).unwrap());
}
