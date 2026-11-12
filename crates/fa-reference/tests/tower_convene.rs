//! Original held-review authority through an independently granted Tower mount.
//! Real file capture and socket congress remain separate from Pending admission.
#![cfg(unix)]
#[path = "support/file_convene.rs"]
mod fixture;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileConveneOutcome, FileConvenePlanResult, FileConveneReceipt, FileConveneResult,
};
use fa_reference::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError, TowerRequest,
    Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use fixture::*;

const REGISTRATION: u64 = 41;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap()
}
fn tower(file: &FileRig) -> PublicationTower {
    PublicationTower::new_control_operator(
        mount(),
        file.rig.port.clone(),
        PublicationLimits::default(),
    )
    .unwrap()
}
fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        if permit {
            vec![PurposeTag::Permit]
        } else {
            Vec::new()
        },
        0,
    )
}
fn query(tower: &PublicationTower, request: u64) -> TowerRequest {
    let mut query = TowerRequest::new(
        "original-held-review",
        tower.request_address(request).unwrap(),
        Verb::Next,
    );
    query.arguments = ReadArguments::ConveneNext { round: NEXT_ROUND };
    query
}
fn native_next(response: &PublicationResponse) -> &FileConvenePlanResult {
    let PublicationResult::ConveneNext(linked) = &response.result else {
        panic!("expected original held-review plan: {response:?}");
    };
    linked.native()
}
fn native(response: &PublicationResponse) -> &FileConveneResult {
    let PublicationResult::Convene(linked) = &response.result else {
        panic!("expected original convening result: {response:?}");
    };
    linked.native()
}
fn observed(response: &PublicationResponse) -> &FileConveneReceipt {
    let Ok(Knowledge::Known { value, basis }) = native(response) else {
        panic!("expected original convening disposition: {response:?}");
    };
    assert_eq!(value.request().expected(), *basis);
    value
}
fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::ConveneNext(linked) = &response.result else {
        panic!("expected linked held-review plan");
    };
    linked
        .action()
        .unwrap()
        .expect("original independently granted successor")
}
fn exchange(tower: &mut PublicationTower, request: &TowerRequest) -> PublicationResponse {
    tower.exchange(
        &access(true, true),
        encode_request(request).unwrap().as_bytes(),
    )
}
fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
}
fn json(response: &PublicationResponse) -> Json {
    let bytes = response.to_json().unwrap();
    for private in [
        "\"payload\"",
        "ALPHA-INDEPENDENT-FILE-CONTEXT",
        "PRIVATE-POLICY-VALUE",
    ] {
        assert!(!bytes.contains(private), "disclosed {private}: {bytes}");
    }
    strict_json::parse(bytes.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap()
}
fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, name| value.get(name).unwrap())
}
fn disposition(response: &PublicationResponse, expected: &str, opened: bool) {
    let document = json(response);
    let profile = field(&document, &["result", "value"]);
    for name in [
        "live_evidence_read",
        "clock_read",
        "effect_authority",
        "automatic_retry",
        "queued_work_is_durable",
    ] {
        assert_eq!(profile.get(name).unwrap().as_bool(), Some(false));
    }
    assert_eq!(
        field(profile, &["knowledge", "value", "outcome"]).as_str(),
        Some(expected)
    );
    assert_eq!(
        field(profile, &["knowledge", "value", "round_opened"]).as_bool(),
        Some(opened)
    );
    assert_eq!(
        matches!(
            field(profile, &["knowledge", "value", "opened"]),
            Json::Null
        ),
        !opened
    );
}

#[test]
fn explicit_wire_admission_stays_pending_until_actual_successor_and_human_publication() {
    let mut file = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(4, 1).unwrap();
    let mut facade = tower(&file);
    facade.set_convene(Some(port.clone())).unwrap();
    let query = query(&facade, REQUEST);
    let encoded_query = encode_request(&query).unwrap();
    assert_eq!(decode_request(encoded_query.as_bytes()).unwrap(), query);
    let before = image(&file);
    let reads = file.source.status();
    let proposed = exchange(&mut facade, &query);
    let command = action(&proposed);
    assert_eq!(command.verb, Verb::Convene);
    assert_eq!(command.address, facade.request_address(REQUEST).unwrap());
    assert_eq!(command.idempotency_key.as_deref(), Some("102"));
    let encoded = encode_request(&command).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), command);
    let document = json(&proposed);
    assert_eq!(
        field(
            &document,
            &["result", "value", "knowledge", "value", "action"]
        ),
        &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap()
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status(), reads);

    let queued = exchange(&mut facade, &command);
    assert_eq!(observed(&queued).outcome(), FileConveneOutcome::Pending);
    disposition(&queued, "pending", false);
    let original = *observed(&queued).request();
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status(), reads);
    assert_eq!(grant.remaining_operations(), 0);
    let mut reconnected = tower(&file);
    reconnected.set_convene(Some(port)).unwrap();
    assert_eq!(exchange(&mut reconnected, &command).result, queued.result);
    assert_eq!(image(&file), before);

    launch(&mut file, &original);
    let opened = exchange(&mut reconnected, &command);
    assert_eq!(observed(&opened).outcome(), FileConveneOutcome::Opened);
    disposition(&opened, "opened", true);
    assert_eq!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .executions,
        0
    );
    finish_continue(&mut file);
    publish(&mut file);
    let confirmed = image(&file);
    assert_eq!(exchange(&mut reconnected, &command).result, opened.result);
    assert_eq!(image(&file), confirmed);
}

#[test]
fn strict_convening_codec_rejects_incomplete_ambiguous_or_unbound_authority() {
    let file = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(4, 1).unwrap();
    let mut facade = tower(&file);
    facade.set_convene(Some(port)).unwrap();
    let query = query(&facade, REQUEST);
    let encoded_query = encode_request(&query).unwrap();
    let command = action(&exchange(&mut facade, &query));
    let encoded = encode_request(&command).unwrap();
    let malformed = [
        encoded_query.replace("convene_held", "convene_any"),
        encoded_query.replace("\"round\":\"102\"", "\"round\":102"),
        encoded_query.replace("\"round\":\"102\"", "\"round\":\"0102\""),
        encoded_query.replace("\"round\":\"102\"", "\"round\":\"102\",\"round\":\"103\""),
        encoded.replace("\"held_round\":\"101\",", ""),
        encoded.replace("\"held_round\":\"101\"", "\"held_round\":\"0\""),
        encoded.replace("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
        encoded.replace("\"recorded_at\":\"1\"", "\"recorded_at\":1"),
        encoded.replace("\"round\":\"102\"", "\"round\":\"102\",\"opened\":true"),
    ];
    let before = image(&file);
    let reads = file.source.status();
    let queries = grant.remaining_queries();
    for invalid in malformed {
        assert_ne!(invalid, encoded);
        assert_ne!(invalid, encoded_query);
        assert!(decode_request(invalid.as_bytes()).is_err(), "{invalid}");
        refused(
            &facade.exchange(&access(true, true), invalid.as_bytes()),
            ReasonCode::IntegrityFailure,
        );
    }
    for mutation in 0..4 {
        let mut invalid = command.clone();
        match mutation {
            0 => invalid.idempotency_key = None,
            1 => invalid.idempotency_key = Some("0102".into()),
            2 => invalid.address.generation = None,
            _ => invalid.arguments = ReadArguments::None,
        }
        assert!(encode_request(&invalid).is_err());
        assert!(matches!(
            facade.dispatch(&access(true, true), invalid).result,
            PublicationResult::Refused(_)
        ));
    }
    assert_eq!(grant.remaining_queries(), queries);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status(), reads);
}

#[test]
fn only_original_control_custody_can_spend_the_independent_convening_grant() {
    let file = held();
    let foreign = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(4, 1).unwrap();
    let mut facade = tower(&file);
    let query = query(&facade, REQUEST);
    refused(
        &exchange(&mut facade, &query),
        ReasonCode::CapabilityUnsupported,
    );
    facade.set_convene(Some(port.clone())).unwrap();
    let command = action(&exchange(&mut facade, &query));
    let before = image(&file);
    let queries = grant.remaining_queries();
    for request in [query.clone(), command.clone()] {
        refused(
            &facade.dispatch(&access(false, true), request.clone()),
            ReasonCode::DeniedByPolicy,
        );
        refused(
            &facade.dispatch(&access(true, false), request.clone()),
            ReasonCode::PurposeMismatch,
        );
        for constructor in [
            PublicationTower::new,
            PublicationTower::new_reconciliation_operator,
        ] {
            let mut legacy =
                constructor(mount(), file.rig.port.clone(), PublicationLimits::default()).unwrap();
            legacy.set_convene(Some(port.clone())).unwrap();
            refused(
                &legacy.dispatch(&access(true, true), request.clone()),
                ReasonCode::CapabilityUnsupported,
            );
        }
    }
    let (other, foreign_grant) = foreign.rig.driver.supervisor().convene_port(2, 1).unwrap();
    assert_eq!(
        facade.set_convene(Some(other)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &exchange(&mut facade, &command),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), queries);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(foreign_grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_operations(), 1);
    assert_eq!(image(&file), before);
    facade.set_convene(Some(port)).unwrap();
    grant.revoke();
    let revoked = exchange(&mut facade, &command);
    assert!(matches!(
        native(&revoked),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_convene"
        })
    ));
    assert_eq!(
        revoked.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    json(&revoked);
    assert_eq!(image(&file), before);
}

#[test]
fn sealed_plan_and_operation_cannot_be_reconstructed_under_another_correlation() {
    let file = held();
    let (port, _grant) = file.rig.driver.supervisor().convene_port(2, 1).unwrap();
    let mut facade = tower(&file);
    facade.set_convene(Some(port)).unwrap();
    let query = query(&facade, REQUEST);
    let proposed = exchange(&mut facade, &query);
    let command = action(&proposed);
    let pending = exchange(&mut facade, &command);
    for response in [proposed, pending] {
        response.to_json().unwrap();
        let mut changed = response.clone();
        changed.envelope.address = facade.request_address(REQUEST + 1).unwrap();
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.address.generation = Some(REGISTRATION + 1);
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.operation_id.push_str("-different");
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.idempotency_key = Some("103".into());
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut tiny = response;
        tiny.envelope.limits.max_response_bytes = 1;
        assert_eq!(tiny.to_json(), Err(TowerEncodingError::OutputLimit));
    }
}

#[test]
fn failed_launch_is_a_finite_historical_result_and_unknown_requests_remain_unknown() {
    let mut file = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(3, 1).unwrap();
    let mut facade = tower(&file);
    facade.set_convene(Some(port.clone())).unwrap();
    let initial_query = query(&facade, REQUEST);
    let command = action(&exchange(&mut facade, &initial_query));
    disposition(&exchange(&mut facade, &command), "pending", false);
    let ticket = file
        .rig
        .driver
        .take_pending_convene(REQUEST)
        .unwrap()
        .unwrap();
    let launch = new_launch(&mut file, NEXT_ROUND);
    std::fs::remove_file(&file.path).unwrap();
    assert!(
        file.rig
            .driver
            .start_file_convened_review(&mut file.source, ticket, launch, || ElapsedTick(1),)
            .is_err()
    );
    let failed = exchange(&mut facade, &command);
    assert_eq!(observed(&failed).outcome(), FileConveneOutcome::Failed);
    disposition(&failed, "failed", false);
    replace(&file.path, &document(1));
    let before = image(&file);
    let reads = file.source.status();
    let mut reconnected = tower(&file);
    reconnected.set_convene(Some(port)).unwrap();
    assert_eq!(exchange(&mut reconnected, &command).result, failed.result);
    assert!(
        file.rig
            .driver
            .take_pending_convene(REQUEST)
            .unwrap()
            .is_none()
    );
    assert_eq!(grant.remaining_operations(), 0);

    let missing = query(&reconnected, 999);
    let unknown = exchange(&mut reconnected, &missing);
    assert!(matches!(
        native_next(&unknown),
        Ok(Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        })
    ));
    assert_eq!(
        unknown.envelope.reason_code,
        Some(ReasonCode::MissingRequiredObservation)
    );
    let document = json(&unknown);
    assert_eq!(
        field(&document, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(
        field(&document, &["result", "value", "knowledge", "reason"]).as_str(),
        Some("outcome_unknown")
    );
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status(), reads);
}
