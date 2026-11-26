//! Original-grant Tower boundaries around a useful helper-weight transition.
//! Deterministic calibration fixtures test enforcement, not trained-helper quality.
#![cfg(unix)]

#[path = "support/file_credibility_promotion.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileCredibilityPromotionBasis, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::Knowledge as ActorKnowledge;
use fa_reference::action::consequence::oversight::joint_credibility::JointReplayBudget;
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::tower::publication::{
    OperatorCapabilityKind, PublicationLimits, PublicationResponse, PublicationResult,
    PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError, TowerRequest,
    Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;

const OPERATION: u64 = 9001;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
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

fn limits() -> PublicationLimits {
    PublicationLimits {
        max_response_bytes: 64 * 1024,
    }
}

fn query() -> TowerRequest {
    let mut request = TowerRequest::new("inspect-credibility", mount(), Verb::Next);
    request.arguments = ReadArguments::CredibilityPromotionNext {
        operation: OPERATION,
    };
    request
}

fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::CredibilityPromotionNext(linked) = &response.result else {
        panic!("original plan: {response:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = linked.native() else {
        panic!("known plan: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert!(value.can_promote());
    linked.action().unwrap().expect("qualified original action")
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
}

#[test]
fn closed_requests_round_trip_without_accepting_labels_weights_or_partial_predecessors() {
    let basis = FileCredibilityPromotionBasis {
        scope: support::profile(0).delivery.scope,
        journal_revision: 5,
        control_sequence: 6,
        authority_epoch: 7,
        evaluation_revision: 8,
        congress_generation: 9,
        policy_generation: 10,
        recorded_at: Some(ElapsedTick(11)),
    };
    let mut promote = TowerRequest::new("promote-credibility-9001", mount(), Verb::Promote);
    promote.arguments = ReadArguments::PromoteCredibility {
        operation: OPERATION,
        expected: basis,
    };
    promote.idempotency_key = Some(OPERATION.to_string());
    for request in [query(), promote.clone()] {
        let encoded = encode_request(&request).unwrap();
        assert_eq!(decode_request(encoded.as_bytes()).unwrap(), request);
        for invalid in [
            encoded.replace("credibility_promotion", "policy_promotion"),
            encoded.replace(
                "\"kind\":\"credibility_promotion\"",
                "\"kind\":\"credibility_promotion\",\"weights\":[]",
            ),
            encoded.replace(
                "\"kind\":\"credibility_promotion\"",
                "\"kind\":\"credibility_promotion\",\"labels\":[]",
            ),
            encoded.replace("\"operation\":\"9001\"", "\"operation\":9001"),
            encoded.replace("\"operation\":\"9001\"", "\"operation\":\"09001\""),
            encoded.replace(
                "\"operation\":\"9001\"",
                "\"operation\":\"9001\",\"operation\":\"9001\"",
            ),
        ] {
            assert_ne!(invalid, encoded);
            assert!(decode_request(invalid.as_bytes()).is_err(), "{invalid}");
        }
    }
    let encoded = encode_request(&promote).unwrap();
    for invalid in [
        encoded.replace("\"evaluation_revision\":\"8\",", ""),
        encoded.replace(
            "\"congress_generation\":\"9\"",
            "\"congress_generation\":\"0\"",
        ),
        encoded.replace("\"recorded_at\":\"11\"", "\"recorded_at\":\"011\""),
        encoded.replace("\"purpose\":\"effect\"", "\"purpose\":\"observe\""),
    ] {
        assert_ne!(invalid, encoded);
        assert!(decode_request(invalid.as_bytes()).is_err(), "{invalid}");
    }
    promote.idempotency_key = None;
    assert!(encode_request(&promote).is_err());
}

#[test]
fn authority_scope_generation_and_mount_gates_preserve_native_work() {
    let (directory, mut host, _human, evaluator) = support::ready(0, JointReplayBudget::default());
    support::calibrate(&mut host, &evaluator);
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port(8, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let mut facade =
        PublicationTower::new_control_operator(mount(), actor.clone(), limits()).unwrap();
    facade
        .set_credibility_promotion(Some(port.clone()))
        .unwrap();
    let original = directory.bytes();
    let state = supervisor.host().unwrap().inspect();

    refused(
        &facade.dispatch(&access(false, true), query()),
        ReasonCode::DeniedByPolicy,
    );
    refused(
        &facade.dispatch(&access(true, false), query()),
        ReasonCode::PurposeMismatch,
    );
    let mut foreign = query();
    foreign.address = Address::new("2", Noun::Run, "3", Some(41)).unwrap();
    refused(
        &facade.dispatch(&access(true, true), foreign),
        ReasonCode::DeniedByPolicy,
    );
    let mut other_run = query();
    other_run.address = Address::new("1", Noun::Run, "4", Some(41)).unwrap();
    refused(
        &facade.dispatch(&access(true, true), other_run),
        ReasonCode::MissingRequiredObservation,
    );
    let mut stale = query();
    stale.address.generation = Some(42);
    refused(
        &facade.dispatch(&access(true, true), stale),
        ReasonCode::StaleGeneration,
    );
    let mut malformed = query();
    malformed.arguments = ReadArguments::CredibilityPromotionNext { operation: 0 };
    refused(
        &facade.dispatch(&access(true, true), malformed),
        ReasonCode::IntegrityFailure,
    );

    for mut limited in [
        PublicationTower::new(mount(), actor.clone(), limits()).unwrap(),
        PublicationTower::new_reconciliation_operator(mount(), actor, limits()).unwrap(),
    ] {
        limited
            .set_credibility_promotion(Some(port.clone()))
            .unwrap();
        refused(
            &limited.dispatch(&access(true, true), query()),
            ReasonCode::CapabilityUnsupported,
        );
    }
    assert_eq!(grant.remaining_queries(), 8);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(directory.bytes(), original);
    assert_eq!(supervisor.host().unwrap().inspect(), state);

    let response = facade.dispatch(&access(true, true), query());
    action(&response);
    assert_eq!(grant.remaining_queries(), 7);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(directory.bytes(), original);
}

#[test]
fn foreign_replacement_withdraws_the_original_installation_without_spending_either_grant() {
    let (directory, mut host, _human, evaluator) = support::ready(0, JointReplayBudget::default());
    support::calibrate(&mut host, &evaluator);
    let (_other_directory, other_host, _other_human, _other_evaluator) =
        support::ready(0, JointReplayBudget::default());
    let (actor, supervisor) = host.into_actor_gateway();
    let (_other_actor, other_supervisor) = other_host.into_actor_gateway();
    let (original_port, original_grant) = supervisor
        .credibility_promotion_port(4, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let (foreign_port, foreign_grant) = other_supervisor
        .credibility_promotion_port(4, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let mut facade = PublicationTower::new_control_operator(mount(), actor, limits()).unwrap();
    facade
        .set_credibility_promotion(Some(original_port.clone()))
        .unwrap();
    let bytes = directory.bytes();
    assert_eq!(
        facade.set_credibility_promotion(Some(foreign_port)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &facade.dispatch(&access(true, true), query()),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(original_grant.remaining_queries(), 4);
    assert_eq!(foreign_grant.remaining_queries(), 4);
    assert_eq!(original_grant.remaining_operations(), 1);
    assert_eq!(foreign_grant.remaining_operations(), 1);
    assert_eq!(directory.bytes(), bytes);

    facade
        .set_credibility_promotion(Some(original_port))
        .unwrap();
    action(&facade.dispatch(&access(true, true), query()));
    original_grant.revoke();
    let revoked = facade.dispatch(&access(true, true), query());
    assert_eq!(
        revoked.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    let PublicationResult::CredibilityPromotionNext(linked) = revoked.result else {
        panic!("original revoked-port observation");
    };
    assert!(matches!(
        linked.native(),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_credibility_promotion"
        })
    ));
    assert_eq!(directory.bytes(), bytes);
}

#[test]
fn complete_receipt_capacity_is_reserved_before_mutation_and_the_same_action_then_succeeds() {
    let (directory, mut host, _human, evaluator) = support::ready(0, JointReplayBudget::default());
    support::calibrate(&mut host, &evaluator);
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port(4, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let mut small = PublicationTower::new_control_operator(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    small.set_credibility_promotion(Some(port.clone())).unwrap();
    let proposed = small.dispatch(&access(true, true), query());
    let command = action(&proposed);
    assert_eq!(
        decode_request(encode_request(&command).unwrap().as_bytes()).unwrap(),
        command
    );
    let bytes = directory.bytes();
    let state = supervisor.host().unwrap().inspect();
    refused(
        &small.dispatch(&access(true, true), command.clone()),
        ReasonCode::BudgetExhausted,
    );
    assert_eq!(directory.bytes(), bytes);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(grant.remaining_queries(), 3);

    let mut enough = PublicationTower::new_control_operator(mount(), actor, limits()).unwrap();
    enough.set_credibility_promotion(Some(port)).unwrap();
    let completed = enough.exchange(
        &access(true, true),
        encode_request(&command).unwrap().as_bytes(),
    );
    assert_eq!(completed.envelope.reason_code, None);
    assert!(!completed.envelope.retry_safe);
    let PublicationResult::CredibilityPromotion(linked) = &completed.result else {
        panic!("original acknowledged transition: {completed:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = linked.native() else {
        panic!("known receipt: {completed:?}");
    };
    assert_eq!(value.after(), *basis);
    assert_eq!(
        value.before().congress_generation + 1,
        basis.congress_generation
    );
    assert_eq!(
        value.before().evaluation_revision + 1,
        basis.evaluation_revision
    );
    let encoded = completed.to_json().unwrap();
    assert!(encoded.len() <= limits().max_response_bytes);
    assert!(encoded.contains("\"kind\":\"credibility-promotion\""));
    assert_eq!(grant.remaining_operations(), 0);
    let acknowledged = directory.bytes();
    let retry = enough.dispatch(&access(true, true), command);
    assert_eq!(retry.result, completed.result);
    assert_eq!(directory.bytes(), acknowledged);

    let mut relabelled = completed.clone();
    relabelled.envelope.operation_id.push_str("-foreign");
    assert_eq!(
        relabelled.to_json(),
        Err(TowerEncodingError::InvalidKnowledge)
    );
    let mut wrong_key = completed.clone();
    wrong_key.envelope.idempotency_key = Some("9002".to_owned());
    assert_eq!(
        wrong_key.to_json(),
        Err(TowerEncodingError::InvalidKnowledge)
    );
    let mut wrong_run = completed;
    wrong_run.envelope.address = Address::new("1", Noun::Run, "4", Some(41)).unwrap();
    assert_eq!(
        wrong_run.to_json(),
        Err(TowerEncodingError::InvalidKnowledge)
    );
    let mut changed_plan = proposed;
    changed_plan.envelope.operation_id.push_str("-foreign");
    assert_eq!(
        changed_plan.to_json(),
        Err(TowerEncodingError::InvalidKnowledge)
    );
}

#[test]
fn capability_discovery_exposes_the_original_grant_without_spending_it() {
    let (directory, host, _human, _evaluator) = support::ready(0, JointReplayBudget::default());
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port(4, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let mut facade = PublicationTower::new_control_operator(mount(), actor, limits()).unwrap();
    facade.set_credibility_promotion(Some(port)).unwrap();
    let mut request = TowerRequest::new("discover-credibility", mount(), Verb::Capabilities);
    request.arguments = ReadArguments::OperatorCapabilities;
    let bytes = directory.bytes();
    for _ in 0..3 {
        let response = facade.dispatch(&access(true, true), request.clone());
        let PublicationResult::OperatorCapabilities(linked) = &response.result else {
            panic!("original grant discovery: {response:?}");
        };
        assert_eq!(linked.entries().len(), OperatorCapabilityKind::ALL.len());
        let entry = linked
            .entries()
            .iter()
            .find(|entry| entry.kind() == OperatorCapabilityKind::CredibilityPromotion)
            .unwrap();
        assert!(entry.configured() && entry.route_connected() && entry.purpose_allowed());
        let Some(ActorKnowledge::Known { value, .. }) = entry.native() else {
            panic!("live original custodian");
        };
        assert_eq!(value.limits()["remaining_queries"], 4);
        assert_eq!(value.limits()["remaining_operations"], 1);
        response.to_json().unwrap();
    }
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(directory.bytes(), bytes);
}
