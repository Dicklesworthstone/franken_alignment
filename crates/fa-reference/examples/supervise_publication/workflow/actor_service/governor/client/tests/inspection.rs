//! Complete original owner evidence with no pathname listener requirement.
use super::*;

fn original() -> (Settings, protocol::Request, Vec<u8>) {
    let (settings, request, response) = super::super::super::tests::inspection::original_document();
    let request = protocol::decode(&request, settings.report_bytes).unwrap();
    (settings, request, response)
}

#[test]
fn inspection_accepts_the_complete_original_cut_and_exact_selected_bounds() {
    let (settings, request, response) = original();
    validate_selected_response(&request, &response, &settings).unwrap();
    let ValidatedResponse::Inspection(inspection) =
        validate_response(&request, &response, settings.report_bytes).unwrap()
    else {
        panic!("native Get must return inspection evidence");
    };
    assert!(inspection.cut.journal_revision > 0);
    assert_eq!(inspection.cut.policy_generation, 1);
    assert_eq!(inspection.campaigns, settings.campaign_requirement());
    assert_eq!(inspection.campaign_limit, settings.campaign_limit);
    assert_eq!(inspection.report_bytes, settings.report_bytes);
    assert!(!inspection.stopped);
    assert_eq!(inspection.source_interrupted, None);

    // A valid refusal is correlated command data, not a fabricated Known value.
    let refused = protocol::refused(&request, "capacity");
    validate_selected_response(&request, &refused, &settings).unwrap();
    let reply = parsed(&refused);
    assert!(reply.get("knowledge").is_none());
    assert!(reply.get("campaign").is_none());
    assert_eq!(reply.get("query").unwrap().as_str(), Some("501"));
}

#[test]
fn inspection_requires_query_phase_full_scope_and_original_acknowledgment_basis() {
    let (settings, request, response) = original();
    for path in [
        vec!["query"],
        vec!["phase"],
        vec!["verb"],
        vec!["scope", "tenant"],
        vec!["scope", "principal"],
        vec!["scope", "run"],
        vec!["scope", "branch"],
        vec!["scope", "authority"],
        vec!["scope", "purpose"],
        vec!["knowledge", "basis", "origin"],
        vec!["knowledge", "basis", "query"],
        vec!["knowledge", "basis", "scope", "tenant"],
        vec!["knowledge", "basis", "scope", "principal"],
        vec!["knowledge", "basis", "scope", "run"],
        vec!["knowledge", "basis", "scope", "branch"],
        vec!["knowledge", "basis", "scope", "authority"],
        vec!["knowledge", "basis", "scope", "purpose"],
        vec!["knowledge", "basis", "journal_revision"],
        vec!["knowledge", "value", "journal_revision"],
    ] {
        let mut changed = parsed(&response);
        *field(&mut changed, &path) = Json::String("999999".into());
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err(),
            "accepted changed inspection correlation or basis: {path:?}",
        );
    }
    let mut fresh = parsed(&request.canonical);
    *field(&mut fresh, &["query"]) = Json::String("502".into());
    let fresh = protocol::decode(protocol::json(&fresh).as_bytes(), settings.report_bytes).unwrap();
    assert!(validate_selected_response(&fresh, &response, &settings).is_err());

    for path in [vec![], vec!["knowledge", "basis"]] {
        let mut changed = parsed(&response);
        let Json::Object(fields) = field(&mut changed, &path) else {
            panic!("object");
        };
        let query = fields.remove("query").unwrap();
        fields.insert("campaign".into(), query);
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err()
        );
    }
}

#[test]
fn inspection_rejects_partial_cut_unselected_limits_and_invented_authority() {
    let (settings, request, response) = original();
    for path in [
        vec!["knowledge", "basis"],
        vec!["knowledge", "value", "journal_revision"],
        vec!["knowledge", "value", "control_sequence"],
        vec!["knowledge", "value", "authority_epoch"],
        vec!["knowledge", "value", "policy_generation"],
        vec!["knowledge", "value", "campaigns"],
        vec!["knowledge", "value", "campaigns", "cases"],
        vec!["knowledge", "value", "campaigns", "input_bytes"],
        vec!["knowledge", "value", "campaigns", "max_campaigns"],
        vec!["knowledge", "value", "campaign_limit"],
        vec!["knowledge", "value", "report_bytes"],
        vec!["knowledge", "value", "stopped"],
        vec!["knowledge", "value", "source_interrupted"],
    ] {
        let mut changed = parsed(&response);
        let Json::Object(fields) = field(&mut changed, &path[..path.len() - 1]) else {
            panic!("object");
        };
        fields.remove(*path.last().unwrap()).unwrap();
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err(),
            "accepted incomplete inspection: {path:?}"
        );
    }
    // These changes stay inside the native maxima; the independently selected
    // connection profile must still reject both narrowed and widened claims.
    for (path, replacement) in [
        (vec!["campaigns", "cases"], "15"),
        (vec!["campaigns", "cases"], "17"),
        (vec!["campaigns", "input_bytes"], "1048575"),
        (vec!["campaigns", "max_campaigns"], "9"),
        (vec!["campaign_limit"], "7"),
        (vec!["report_bytes"], "1048575"),
    ] {
        let mut changed = parsed(&response);
        *field(field(&mut changed, &["knowledge", "value"]), &path) =
            Json::String(replacement.into());
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err(),
            "accepted a different selected inspection limit: {path:?}"
        );
    }
    for name in ["approval", "effect_authority", "policy_changed"] {
        let mut changed = parsed(&response);
        let Json::Object(value) = field(&mut changed, &["knowledge", "value"]) else {
            panic!("value");
        };
        value.insert(name.into(), Json::Bool(true));
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err()
        );
    }
    for (name, replacement) in [
        ("journal_revision", Json::String("0".into())),
        ("control_sequence", Json::String("00".into())),
        (
            "authority_epoch",
            Json::String("18446744073709551616".into()),
        ),
        ("policy_generation", Json::String("0".into())),
        ("stopped", Json::Null),
        ("source_interrupted", Json::String("available".into())),
    ] {
        let mut changed = parsed(&response);
        *field(&mut changed, &["knowledge", "value", name]) = replacement;
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err()
        );
    }
}

#[test]
fn inspection_request_is_closed_and_requires_an_explicit_canonical_nonzero_query() {
    let (settings, request, _) = original();
    for query in [
        Json::Null,
        Json::Bool(true),
        parsed(b"501"),
        Json::String("0".into()),
        Json::String("01".into()),
        Json::String("-1".into()),
        Json::String("18446744073709551616".into()),
    ] {
        let mut changed = parsed(&request.canonical);
        *field(&mut changed, &["query"]) = query;
        assert!(
            protocol::decode(protocol::json(&changed).as_bytes(), settings.report_bytes).is_err()
        );
    }
    for name in [
        "campaign",
        "operation",
        "candidate",
        "phase",
        "expected",
        "approval",
    ] {
        let mut changed = parsed(&request.canonical);
        let Json::Object(fields) = &mut changed else {
            panic!("request");
        };
        fields.insert(name.into(), Json::String("1".into()));
        assert!(
            protocol::decode(protocol::json(&changed).as_bytes(), settings.report_bytes).is_err()
        );
    }
    let mut missing = parsed(&request.canonical);
    let Json::Object(fields) = &mut missing else {
        panic!("request");
    };
    fields.remove("query").unwrap();
    assert!(protocol::decode(protocol::json(&missing).as_bytes(), settings.report_bytes).is_err());
}
