//! Protocol fixtures exercise parsing/correlation, not live evaluator evidence.
use super::*;

const LABEL: &[u8] = br#"{"schema_version":"fa.native-assessment/1","round":101,"attempt":1,"policy_generation":1,"congress_generation":1,"case_evidence_root_hex":"0101010101010101010101010101010101010101010101010101010101010101","expected_evaluation_revision":1,"origin":1,"evidence_id_hex":"0202020202020202020202020202020202020202020202020202020202020202","truth":"benign"}"#;
fn peer() -> EvaluatorPeerProfile {
    EvaluatorPeerProfile::decode(br#"{"schema_version":"fa.native-evaluator-peer/1","socket":"/tmp/eval.sock","scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5},"deployment":101,"evaluator":202,"supervisor":{"uid":1,"gid":2,"pid":3},"peer":{"uid":4,"gid":5,"pid":6},"candidate_limit":3,"runtime_ms":1000,"poll_ms":1}"#).unwrap()
}
fn native(status: &str, retained: bool) -> Vec<u8> {
    let revision = if status == "applied" { 12 } else { 10 };
    format!(r#"{{"schema_version":"fa.native-andon-mutation-result/1","status":"{status}","recovery_fence_revision":"10","state":{{"schema_version":"fa.native-andon-result/1","operation":"assess","historical":false,"history_anchor_retained":{retained},"journal_revision":"{revision}","control_sequence":"1","authority_epoch":"1","evaluation_revision":"2","pending_cases":0,"censored_cases":0,"windows":[{{"window":"1","false_stops":"1","total_cost":"340282366920938463463374607431768211455","incident_opened_by_round":"101"}}]}}}}"#).into_bytes()
}

#[test]
fn exact_requests_round_trip_without_rewriting_assessment_bytes() {
    let peer = peer(); let request = Request::prepare(&peer, LABEL).unwrap();
    let accepted = Request::accept(&peer, request.wire()).unwrap();
    assert_eq!(accepted.parsed, request.parsed);
    assert!(matches!(accepted.mutation(), mutations::Mutation::Assess { round: 101, .. }));
    for (status, expected) in [("applied", Outcome::Applied), ("already_applied", Outcome::AlreadyApplied)] {
        let reply = capture(&accepted, &native(status, true), true).unwrap();
        assert_eq!(validate(&request, &reply).unwrap(), expected);
    }
}

#[test]
fn every_audience_component_is_checked_before_native_admission() {
    let peer = peer(); let original = Request::prepare(&peer, LABEL).unwrap();
    for field in ["tenant", "principal", "run", "branch", "authority", "deployment", "evaluator"] {
        let mut wrong = peer.clone();
        match field {
            "tenant" => wrong.scope.tenant += 1, "principal" => wrong.scope.principal += 1,
            "run" => wrong.scope.run += 1, "branch" => wrong.scope.branch += 1,
            "authority" => wrong.scope.authority += 1, "deployment" => wrong.deployment += 1,
            _ => wrong.evaluator_id += 1,
        }
        assert!(Request::accept(&wrong, original.wire()).is_err());
    }
    assert!(Request::accept(&peer, LABEL).is_err()); // No legacy unbound fallback.
}

#[test]
fn another_valid_assessment_cannot_satisfy_this_request() {
    let peer = peer(); let original = Request::prepare(&peer, LABEL).unwrap();
    let changed = std::str::from_utf8(LABEL).unwrap().replace("\"origin\":1", "\"origin\":2");
    let other = Request::prepare(&peer, changed.as_bytes()).unwrap();
    let response = capture(&other, &native("applied", true), true).unwrap();
    assert!(validate(&original, &response).is_err());
    assert!(validate(&other, &response).unwrap().completed());
}

#[test]
fn refused_unknown_and_applied_incomplete_are_never_success() {
    let request = Request::prepare(&peer(), LABEL).unwrap();
    for (body, expected) in [(native("refused", true), Outcome::Refused),
        (Vec::new(), Outcome::Unconfirmed), (native("applied", false), Outcome::Incomplete),
        (native("already_applied", true), Outcome::Incomplete)] {
        let response = capture(&request, &body, false).unwrap();
        let result = validate(&request, &response).unwrap();
        assert_eq!(result, expected); assert!(!result.completed());
    }
    for body in [Vec::new(), native("refused", true), native("applied", false)] {
        assert!(capture(&request, &body, true).is_err());
    }
}

#[test]
fn junk_extra_fields_duplicate_keys_and_unbound_legacy_reports_refuse() {
    let request = Request::prepare(&peer(), LABEL).unwrap();
    let reply = String::from_utf8(capture(&request, &native("applied", true), true).unwrap()).unwrap();
    for changed in ["success".to_owned(), "{\"ok\":true}".to_owned(),
        String::from_utf8(native("applied", true)).unwrap(), format!("{reply}{{}}"),
        reply.replacen("\"native_completed\":true", "\"native_completed\":true,\"native_completed\":true", 1),
        reply.replacen("\"native_completed\":true", "\"native_completed\":true,\"unexpected\":0", 1),
        reply.replacen("fa.native-evaluator-receipt/1", "fa.native-evaluator-receipt/2", 1)] {
        assert!(validate(&request, changed.as_bytes()).is_err());
    }
}

#[test]
fn native_state_schema_counters_and_acknowledgment_are_checked() {
    let request = Request::prepare(&peer(), LABEL).unwrap();
    let reply = String::from_utf8(capture(&request, &native("applied", true), true).unwrap()).unwrap();
    for changed in [
        reply.replace("\"operation\":\"assess\"", "\"operation\":\"promote\""),
        reply.replace("\"historical\":false", "\"historical\":true"),
        reply.replace("\"journal_revision\":\"12\"", "\"journal_revision\":\"10\""),
        reply.replace("\"journal_revision\":\"12\"", "\"journal_revision\":\"012\""),
        reply.replace("\"authority_epoch\":\"1\"", "\"authority_epoch\":\"18446744073709551616\""),
        reply.replace("\"status\":\"applied\"", "\"status\":\"refused\""),
        reply.replace("\"false_stops\":\"1\"", "\"false_stops\":\"0\""),
    ] { assert!(validate(&request, changed.as_bytes()).is_err()); }
}

#[test]
fn frame_bounds_apply_before_parsing_or_retaining_untrusted_data() {
    let request = Request::prepare(&peer(), LABEL).unwrap();
    assert!(Request::prepare(&peer(), &vec![b' '; ASSESSMENT_BYTES + 1]).is_err());
    assert!(Request::accept(&peer(), &vec![b' '; REQUEST_BYTES + 1]).is_err());
    assert!(validate(&request, &vec![b' '; transport::MAX_MESSAGE_BYTES + 1]).is_err());
}
