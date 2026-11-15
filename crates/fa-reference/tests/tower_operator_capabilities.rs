//! Discover original operator custody without consuming its observation or
//! mutation allowance. Native review/receipts remain the only publication path.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        RecoveryReserve,
        observed::{FileHumanReviewer, FileOversight},
        requests::actor::{
            FileOperatorCapabilityBasis, FileOperatorCapabilityBudget, FileRequestHistoryDisclosure,
        },
    },
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge as NativeKnowledge, UnknownReason as NativeUnknown,
};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json;
use fa_reference::tower::publication::{
    LinkedOperatorCapabilities, OperatorCapabilityEntry, OperatorCapabilityKind as Kind,
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerRequest, Verb, decode_request,
    encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use support::{Directory, Port, Supervisor, observe, oversight, profile, ready};

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
}
fn access(operator: bool, mutation: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        if mutation {
            vec![PurposeTag::Permit, PurposeTag::Experiment]
        } else {
            Vec::new()
        },
        0,
    )
}
fn create(root: &Directory) -> (Port, Supervisor, FileHumanReviewer, ActorProposal) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (actor, supervisor) = host.into_actor_gateway();
    (actor, supervisor, reviewer, proposal)
}
fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default()).unwrap()
}
fn request() -> TowerRequest {
    let mut request = TowerRequest::new("discover-original-grants", mount(), Verb::Capabilities);
    request.arguments = ReadArguments::OperatorCapabilities;
    request.idempotency_key = Some("observational-label".to_owned());
    request
}
fn discover(tower: &mut PublicationTower) -> PublicationResponse {
    tower.exchange(
        &access(true, true),
        encode_request(&request()).unwrap().as_bytes(),
    )
}
fn linked(response: &PublicationResponse) -> &LinkedOperatorCapabilities {
    let PublicationResult::OperatorCapabilities(linked) = &response.result else {
        panic!("expected original configuration discovery: {response:?}");
    };
    assert_eq!(response.envelope.reason_code, None);
    linked
}
fn entry(response: &PublicationResponse, kind: Kind) -> &OperatorCapabilityEntry {
    linked(response)
        .entries()
        .iter()
        .find(|entry| entry.kind() == kind)
        .unwrap()
}
fn known(
    response: &PublicationResponse,
    kind: Kind,
) -> (&FileOperatorCapabilityBudget, FileOperatorCapabilityBasis) {
    let Some(NativeKnowledge::Known { value, basis }) = entry(response, kind).native() else {
        panic!("expected surviving original grant: {response:?}");
    };
    assert_eq!(basis.scope, profile().delivery.scope);
    (value, *basis)
}
fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

#[test]
fn discovering_actual_grants_preserves_the_admission_snapshot_and_original_two_key_publication() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    let mut facade = tower(actor.clone());
    let (inspection, inspection_grant) = supervisor.inspection_port(2).unwrap();
    let (tail, tail_grant) = supervisor.journal_tail_port(2, 4).unwrap();
    facade.set_inspection(Some(inspection)).unwrap();
    facade.set_journal_tail(Some(tail)).unwrap();
    observe(&mut supervisor);
    let bytes = image(&root);
    let before = supervisor.host().unwrap().inspect();
    for _ in 0..4 {
        let response = discover(&mut facade);
        assert_eq!(
            known(&response, Kind::Situation).0.limits()["remaining_queries"],
            2
        );
        assert_eq!(
            known(&response, Kind::JournalTail).0.limits()["events_per_page"],
            4
        );
        assert!(entry(&response, Kind::Situation).route_connected());
        assert!(!entry(&response, Kind::Fence).configured());
        assert_eq!(inspection_grant.remaining_queries(), 2);
        assert_eq!(tail_grant.remaining_queries(), 2);
        assert_eq!(image(&root), bytes);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
    }
    // A successful read neither consumes the captured input nor supplies a key.
    let ticket = actor.submit(701, &proposal).unwrap();
    assert_eq!(
        actor.poll(&ticket),
        NativeKnowledge::Pending { request: 701 }
    );
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    let keys = ready(&mut supervisor, &reviewer, 701);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        assert!(matches!(
            host.publish(revision, keys.automatic.attempt()).unwrap(),
            EndpointOutcome::Executed { .. }
        ));
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let after = discover(&mut facade);
    assert!(known(&after, Kind::Situation).1.journal_revision > before.revision);
    assert_eq!(inspection_grant.remaining_queries(), 2);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.charged,
        proposal.units
    );
}

#[test]
fn complete_grant_selection_reports_original_finite_limits_without_spending_any() {
    let root = Directory::new();
    let (actor, supervisor, _, _) = create(&root);
    let mut facade = tower(actor);
    let (p1, g1) = supervisor.inspection_port(3).unwrap();
    facade.set_inspection(Some(p1)).unwrap();
    let (p2, g2) = supervisor.journal_tail_port(3, 4).unwrap();
    facade.set_journal_tail(Some(p2)).unwrap();
    let (p3, g3) = supervisor
        .request_history_port(
            3,
            128,
            16,
            FileRequestHistoryDisclosure::RecordedRequestRelationships,
        )
        .unwrap();
    facade.set_request_history(Some(p3)).unwrap();
    let (p4, g4) = supervisor
        .explanation_port_with_numerical_budget(3, 128, 256)
        .unwrap();
    facade.set_explanation(Some(p4)).unwrap();
    let (p5, g5) = supervisor
        .review_explanation_port_with_numerical_budget(3, 128, 256)
        .unwrap();
    facade.set_review_explanation(Some(p5)).unwrap();
    let (p6, g6) = supervisor.hold_diagnosis_port(3).unwrap();
    facade.set_hold_diagnosis(Some(p6)).unwrap();
    let (p7, g7) = supervisor
        .resolution_port_with_numerical_budget(3, 2, 128, 256)
        .unwrap();
    facade.set_resolution(Some(p7)).unwrap();
    let (p8, g8) = supervisor
        .cancellation_port_with_numerical_budget(3, 2, 128, 256)
        .unwrap();
    facade.set_cancellation(Some(p8)).unwrap();
    let (p9, g9) = supervisor
        .abandonment_port_with_numerical_budget(3, 2, 128, 256)
        .unwrap();
    facade.set_abandonment(Some(p9)).unwrap();
    let (p10, g10) = supervisor
        .narrow_port_with_numerical_budget(3, 2, 128, 256)
        .unwrap();
    facade.set_narrow(Some(p10)).unwrap();
    let (p11, g11) = supervisor.convene_port(3, 2).unwrap();
    facade.set_convene(Some(p11)).unwrap();
    let (p12, g12) = supervisor
        .fence_port_with_numerical_budget(128, 128)
        .unwrap();
    facade.set_fence(Some(p12)).unwrap();
    let (p13, g13) = supervisor
        .policy_replay_port_with_pages(
            3,
            ReplayLimits {
                cases: 16,
                input_bytes: 65536,
            },
            8,
        )
        .unwrap();
    facade.set_policy_replay(Some(p13)).unwrap();
    let bytes = image(&root);
    let state = supervisor.host().unwrap().inspect();
    for _ in 0..3 {
        let response = discover(&mut facade);
        assert_eq!(linked(&response).entries().len(), 14);
        for kind in Kind::ALL {
            if kind == Kind::PolicyPromotion {
                assert!(!entry(&response, kind).configured());
                continue;
            }
            assert!(entry(&response, kind).configured());
            assert!(entry(&response, kind).route_connected());
            assert!(entry(&response, kind).purpose_allowed());
            let (budget, basis) = known(&response, kind);
            assert_eq!(basis.journal_revision, state.revision);
            if kind != Kind::Fence {
                assert_eq!(budget.limits()["remaining_queries"], 3);
            }
        }
        assert_eq!(
            known(&response, Kind::Resolution).0.limits()["remaining_numerical_replay_events"],
            256
        );
        assert_eq!(
            known(&response, Kind::Fence).0.limits()["remaining_operations"],
            1
        );
        assert_eq!(
            known(&response, Kind::PolicyReplay).0.limits()["remaining_page_queries"],
            8
        );
        assert_eq!(
            known(&response, Kind::PolicyReplay).0.limits()["case_limit"],
            16
        );
        let encoded = response.to_json().unwrap();
        assert!(encoded.len() <= PublicationLimits::default().max_response_bytes);
        strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
    }
    assert_eq!(
        [
            g1.remaining_queries(),
            g2.remaining_queries(),
            g3.remaining_queries(),
            g4.remaining_queries(),
            g5.remaining_queries(),
            g6.remaining_queries(),
            g7.remaining_queries(),
            g8.remaining_queries(),
            g9.remaining_queries(),
            g10.remaining_queries(),
            g11.remaining_queries(),
            g13.remaining_queries()
        ],
        [3; 12]
    );
    assert_eq!(g12.remaining_operations(), 1);
    assert_eq!(g13.remaining_pages(), 8);
    assert_eq!(image(&root), bytes);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
}

#[test]
fn discovery_distinguishes_exhausted_revoked_uninstalled_and_lost_original_custody() {
    let root = Directory::new();
    let (actor, mut supervisor, _, _) = create(&root);
    let mut facade = tower(actor.clone());
    assert!(
        entry(&discover(&mut facade), Kind::Situation)
            .native()
            .is_none()
    );
    let (port, grant) = supervisor.inspection_port(1).unwrap();
    facade.set_inspection(Some(port.clone())).unwrap();
    let mut reconnected = tower(actor);
    reconnected.set_inspection(Some(port.clone())).unwrap();
    for facade in [&mut facade, &mut reconnected] {
        assert_eq!(
            known(&discover(facade), Kind::Situation).0.limits()["remaining_queries"],
            1
        );
    }
    port.situation().unwrap();
    assert_eq!(
        known(&discover(&mut facade), Kind::Situation).0.limits()["remaining_queries"],
        0
    );
    assert_eq!(port.situation(), Err(ActorError::Capacity));
    {
        let _borrow = supervisor.host_mut().unwrap();
        assert!(matches!(
            entry(&discover(&mut facade), Kind::Situation).native(),
            Some(NativeKnowledge::Unknown {
                reason: NativeUnknown::ControllerUnavailable
            })
        ));
    }
    assert_eq!(
        known(&discover(&mut facade), Kind::Situation).0.limits()["remaining_queries"],
        0
    );
    drop(supervisor);
    assert!(matches!(
        entry(&discover(&mut facade), Kind::Situation).native(),
        Some(NativeKnowledge::Unknown {
            reason: NativeUnknown::ControllerUnavailable
        })
    ));
    let (_reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(matches!(
        entry(&discover(&mut facade), Kind::Situation).native(),
        Some(NativeKnowledge::Unknown { .. })
    ));
    grant.revoke();
    for facade in [&mut facade, &mut reconnected] {
        let response = discover(facade);
        assert!(entry(&response, Kind::Situation).configured());
        assert!(matches!(
            entry(&response, Kind::Situation).native(),
            Some(NativeKnowledge::Withheld {
                authority_required: "supervisor_inspection"
            })
        ));
        facade.set_inspection(None).unwrap();
        assert!(!entry(&discover(facade), Kind::Situation).configured());
    }
}

#[test]
fn same_scope_replacement_and_a_poisoned_owner_cannot_supply_old_grant_observations() {
    for replace in [false, true] {
        let root = Directory::new();
        let other_root = Directory::new();
        let (actor, mut supervisor, _, _) = create(&root);
        let mut facade = tower(actor);
        let (port, grant) = supervisor.inspection_port(2).unwrap();
        facade.set_inspection(Some(port)).unwrap();
        known(&discover(&mut facade), Kind::Situation);
        if replace {
            let (replacement, _) = oversight::create(&other_root);
            let _original = std::mem::replace(&mut *supervisor.host_mut().unwrap(), replacement);
            assert!(matches!(
                entry(&discover(&mut facade), Kind::Situation).native(),
                Some(NativeKnowledge::Unknown { .. })
            ));
        } else {
            std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
            let mut host = supervisor.host_mut().unwrap();
            let revision = host.revision();
            assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
            assert!(host.storage_failure().is_some());
        }
        let before = image(&root);
        assert!(matches!(
            entry(&discover(&mut facade), Kind::Situation).native(),
            Some(NativeKnowledge::Unknown { .. })
        ));
        assert_eq!(grant.remaining_queries(), 2);
        assert_eq!(image(&root), before);
    }
}

#[test]
fn wrong_callers_registrations_and_malformed_selectors_cannot_discover_grants() {
    let root = Directory::new();
    let (actor, supervisor, _, _) = create(&root);
    let mut facade = tower(actor);
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    facade.set_inspection(Some(port)).unwrap();
    let encoded = encode_request(&request()).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), request());
    for invalid in [
        encoded.replace("\"operator\":true", "\"operator\":false"),
        encoded.replace("\"operator\":true", "\"operator\":\"true\""),
        encoded.replace("\"operator\":true", "\"operator\":true,\"operator\":true"),
        encoded.replace("\"operator\":true", "\"operator\":true,\"query\":\"1\""),
        encoded.replace("\"operator\":true", "\"operator\":true,\"allow\":true"),
    ] {
        assert!(decode_request(invalid.as_bytes()).is_err());
        let response = facade.exchange(&access(true, true), invalid.as_bytes());
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
        assert!(matches!(response.result, PublicationResult::Refused(_)));
    }
    for caller in [
        access(false, true),
        TowerAccess::new(
            CallerScope::full_tenant_access("other"),
            true,
            vec![PurposeTag::Permit],
            0,
        ),
    ] {
        let response = facade.dispatch(&caller, request());
        assert!(matches!(response.result, PublicationResult::Refused(_)));
    }
    for (address, reason) in [
        (
            Address::new("1", Noun::Run, "3", Some(40)).unwrap(),
            ReasonCode::StaleGeneration,
        ),
        (
            Address::new("1", Noun::Run, "9", Some(41)).unwrap(),
            ReasonCode::MissingRequiredObservation,
        ),
    ] {
        let mut wrong = request();
        wrong.address = address;
        assert_eq!(
            facade
                .dispatch(&access(true, true), wrong)
                .envelope
                .reason_code,
            Some(reason)
        );
    }
    let response = facade.dispatch(&access(true, false), request());
    assert!(!entry(&response, Kind::Fence).purpose_allowed());
    assert!(!entry(&response, Kind::PolicyReplay).purpose_allowed());
    assert!(entry(&response, Kind::Situation).purpose_allowed());
    assert_eq!(grant.remaining_queries(), 2);
}

#[test]
fn sealed_configuration_cannot_be_relabelled_and_output_failure_spends_no_native_work() {
    let root = Directory::new();
    let (actor, supervisor, _, _) = create(&root);
    let mut facade = tower(actor);
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    facade.set_inspection(Some(port)).unwrap();
    let response = discover(&mut facade);
    let encoded = response.to_json().unwrap();
    let parsed = strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
    let result = parsed.get("result").unwrap();
    assert_eq!(
        result.get("kind").unwrap().as_str(),
        Some("operator-capabilities")
    );
    let value = result.get("value").unwrap();
    assert_eq!(
        value.get("current_eligibility").unwrap().as_str(),
        Some("not_observed")
    );
    assert_eq!(
        value.get("effect_authority").unwrap().as_bool(),
        Some(false)
    );
    for forbidden in [
        "PRIVATE-SOURCE-BYTES",
        "ExactValue",
        "\"payload\"",
        "\"token_ids\"",
        "\"candidate_policy\"",
    ] {
        assert!(!encoded.contains(forbidden));
    }
    for change in 0..4 {
        let mut forged = response.clone();
        match change {
            0 => forged.envelope.address.id = "9".to_owned(),
            1 => forged.envelope.address.generation = Some(42),
            2 => forged.envelope.operation_id = "another-operation".to_owned(),
            _ => forged.envelope.idempotency_key = Some("another-label".to_owned()),
        }
        assert_eq!(forged.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    }
    let mut small = response;
    small.envelope.limits.max_response_bytes = 64;
    assert_eq!(small.to_json(), Err(TowerEncodingError::OutputLimit));
    assert_eq!(grant.remaining_queries(), 2);
}

#[test]
fn mount_restrictions_and_foreign_replacement_remain_distinct_from_native_grant_liveness() {
    let root = Directory::new();
    let other = Directory::new();
    let (actor, supervisor, _, _) = create(&root);
    let (_, foreign, _, _) = create(&other);
    let mut facade =
        PublicationTower::new_reconciliation_operator(mount(), actor, PublicationLimits::default())
            .unwrap();
    let (narrow, narrow_grant) = supervisor.narrow_port(2, 1, 128).unwrap();
    facade.set_narrow(Some(narrow)).unwrap();
    let (resolve, resolve_grant) = supervisor.resolution_port(2, 1, 128).unwrap();
    facade.set_resolution(Some(resolve)).unwrap();
    let response = discover(&mut facade);
    assert!(entry(&response, Kind::Narrowing).configured());
    assert!(!entry(&response, Kind::Narrowing).route_connected());
    known(&response, Kind::Narrowing);
    assert!(entry(&response, Kind::Resolution).route_connected());
    let (inspection, grant) = supervisor.inspection_port(2).unwrap();
    facade.set_inspection(Some(inspection)).unwrap();
    let (foreign_port, foreign_grant) = foreign.inspection_port(2).unwrap();
    assert!(facade.set_inspection(Some(foreign_port)).is_err());
    assert!(!entry(&discover(&mut facade), Kind::Situation).configured());
    assert_eq!(
        [
            narrow_grant.remaining_queries(),
            resolve_grant.remaining_queries(),
            grant.remaining_queries(),
            foreign_grant.remaining_queries()
        ],
        [2; 4]
    );
}
