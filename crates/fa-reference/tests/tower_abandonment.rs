//! Explicit Unknown disposition uses the original durable abandonment transition.
//! Real congress and both publication keys establish uncertain retained liability;
//! unrelated requests keep their original publication lifecycle.
#![cfg(unix)]

#[path = "tower_abandonment/learned.rs"]
mod learned;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileAbandonmentBlockReason, FileAbandonmentError, FileAbandonmentPlan,
    FileAbandonmentPlanResult, FileAbandonmentReceipt, FileAbandonmentResult,
    FileSupervisorAbandonmentGrant, MAX_ABANDONMENT_REPLAY_EVENTS, MAX_RESOLUTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::delivery::{
    EndpointOutcome, persistent::observed::FileHumanReviewer,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge as ActorKnowledge,
    UnknownReason as ActorUnknown,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::event_journal::DomainEventJournal;
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{
    Basis, BasisIdentity, ClaimClass, ProofStates, ScopedProof,
};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerRequest,
    TowerBuilder, TowerLimits, Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::{PurposeTag, RootCx};
use std::collections::BTreeSet;
use support::{Directory, Port, Supervisor, create, observe, oversight, ready};

const REQUEST: u64 = 701;
const REGISTRATION: u64 = 41;
const OPERATION: u64 = 5009;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap()
}

fn tower(port: Port) -> PublicationTower {
    PublicationTower::new_control_operator(mount(), port, PublicationLimits::default()).unwrap()
}

fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        if permit {
            vec![PurposeTag::Permit, PurposeTag::Experiment]
        } else {
            Vec::new()
        },
        0,
    )
}

fn next_request(facade: &PublicationTower, key: u64, operation: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "next-unknown-abandonment",
        facade.request_address(key).unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::AbandonmentNext { operation };
    request
}

fn next(facade: &mut PublicationTower, operation: u64) -> PublicationResponse {
    let request = next_request(facade, REQUEST, operation);
    facade.exchange(
        &access(true, true),
        encode_request(&request).unwrap().as_bytes(),
    )
}

fn native_next(response: &PublicationResponse) -> &FileAbandonmentPlanResult {
    let PublicationResult::AbandonmentNext(linked) = &response.result else {
        panic!("expected original abandonment plan: {response:?}");
    };
    linked.native()
}

fn plan(response: &PublicationResponse) -> &FileAbandonmentPlan {
    let Ok(ActorKnowledge::Known { value, basis }) = native_next(response) else {
        panic!("expected acknowledged plan: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    value
}

fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::AbandonmentNext(linked) = &response.result else {
        panic!("original abandonment plan");
    };
    linked
        .action()
        .unwrap()
        .expect("one permitted original Unknown disposition")
}

fn native(response: &PublicationResponse) -> &FileAbandonmentResult {
    let PublicationResult::Abandonment(linked) = &response.result else {
        panic!("expected original abandonment result: {response:?}");
    };
    linked.native()
}

fn receipt(response: &PublicationResponse) -> &FileAbandonmentReceipt {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected original abandonment acknowledgment: {response:?}");
    };
    assert_eq!(value.after(), *basis);
    assert!(!response.envelope.retry_safe);
    assert_eq!(response.envelope.reason_code, None);
    value
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value.get(name).unwrap_or_else(|| panic!("missing {name}"))
}

fn document(response: &PublicationResponse) -> Json {
    strict_json::parse(
        response.to_json().unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap()
}

struct Fixture {
    root: Directory,
    actor: Port,
    supervisor: Supervisor,
    reviewer: FileHumanReviewer,
    proposal: ActorProposal,
    facade: PublicationTower,
    grant: FileSupervisorAbandonmentGrant,
    keys: oversight::Keys,
}

fn authorized() -> Fixture {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, REQUEST);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor
        .abandonment_port(32, 8, MAX_ABANDONMENT_REPLAY_EVENTS)
        .unwrap();
    facade.set_abandonment(Some(port)).unwrap();
    Fixture {
        root,
        actor,
        supervisor,
        reviewer,
        proposal,
        facade,
        grant,
        keys,
    }
}


fn unknown() -> Fixture {
    let mut f = authorized();
    {
        let mut host = f.supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &f.keys);
        let revision = host.revision();
        host.reconcile(revision, f.keys.automatic.attempt()).unwrap();
    }
    f
}

#[test]
fn explicit_abandonment_retains_selected_liability_and_keeps_other_publication_available() {
    let mut f = unknown();
    // A second independently reviewed effect makes total charge different from
    // the selected original liability. Abandoning REQUEST cannot cancel it.
    observe(&mut f.supervisor);
    f.actor.submit(REQUEST + 1, &f.proposal).unwrap();
    let other_keys = ready(&mut f.supervisor, &f.reviewer, REQUEST + 1);
    {
        let mut host = f.supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &other_keys);
    }
    let original = image(&f.root);
    let before = f.supervisor.host().unwrap().inspect();
    assert_eq!(before.control.ledger.charged, 32);
    let proposed = next(&mut f.facade, OPERATION);
    assert_eq!(plan(&proposed).stage(), ActionState::Unknown);
    assert!(plan(&proposed).can_abandon());
    assert_eq!(plan(&proposed).basis().request, REQUEST);
    assert_ne!(plan(&proposed).basis().attempt, REQUEST);
    assert_eq!(f.grant.remaining_operations(), 8);
    assert_eq!(image(&f.root), original);
    assert_eq!(f.supervisor.host().unwrap().inspect(), before);

    let command = action(&proposed);
    assert_eq!(command.verb, Verb::Resolve);
    assert_eq!(command.idempotency_key, Some(OPERATION.to_string()));
    assert!(matches!(command.arguments, ReadArguments::AbandonUnknown { .. }));
    let wire = encode_request(&command).unwrap();
    assert!(wire.contains("\"action\":\"abandon_unknown\""));
    assert_eq!(decode_request(wire.as_bytes()).unwrap(), command);
    let parsed = document(&proposed);
    let body = field(field(&parsed, "result"), "value");
    let value = field(field(body, "knowledge"), "value");
    assert_eq!(
        field(value, "action"),
        &strict_json::parse(wire.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap()
    );

    // This records a permanent accounting disposition; the effect's external
    // outcome stays unknown and every charged unit remains unavailable.
    observe(&mut f.supervisor);
    let completed = f.facade.exchange(&access(true, true), wire.as_bytes());
    let result = receipt(&completed);
    assert_eq!(result.before(), plan(&proposed).basis());
    assert_eq!(result.before_stage(), ActionState::Unknown);
    assert_eq!(result.stage(), ActionState::IrrecoverablyUnknown);
    assert_eq!(result.charged_units(), 16);
    assert_eq!(
        result.after().journal_revision,
        result.before().journal_revision + 1
    );
    assert_eq!(result.after().recorded_at, result.before().recorded_at);
    let after = f.supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.available, before.control.ledger.available);
    assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
    assert_eq!(after.control.ledger.charged, 32);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(
        after.control.ledger.stages[&f.keys.automatic.attempt()],
        ActionState::IrrecoverablyUnknown
    );
    assert_eq!(
        after.control.ledger.stages[&other_keys.automatic.attempt()],
        ActionState::Dispatching
    );
    let ticket = f.actor.submit(REQUEST, &f.proposal).unwrap();
    assert!(matches!(
        f.actor.poll(&ticket),
        ActorKnowledge::Unknown { reason: ActorUnknown::OutcomeUnknown }
    ));
    assert_eq!(f.grant.remaining_operations(), 7);
    assert!(matches!(
        f.actor.submit(REQUEST + 2, &f.proposal),
        Err(ActorError::Unavailable)
    ));

    let committed = image(&f.root);
    assert_eq!(
        native(&f.facade.exchange(&access(true, true), wire.as_bytes())),
        native(&completed)
    );
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.grant.remaining_operations(), 7);
    {
        let mut host = f.supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.publish(revision, f.keys.automatic.attempt()).is_err());
        assert_eq!(image(&f.root), committed);
        let revision = host.revision();
        assert_eq!(
            host.publish(revision, other_keys.automatic.attempt()).unwrap(),
            EndpointOutcome::Executed { resulting_version: 2 }
        );
    }
    assert_eq!(f.supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(f.supervisor.host().unwrap().inspect().control.ledger.charged, 32);
    let progressed = image(&f.root);
    assert_eq!(
        native(&f.facade.dispatch(&access(true, true), command)),
        native(&completed)
    );
    assert_eq!(image(&f.root), progressed);
    assert_eq!(f.grant.remaining_operations(), 7);
    assert!(matches!(
        f.actor.poll(&ticket),
        ActorKnowledge::Unknown { reason: ActorUnknown::OutcomeUnknown }
    ));

    // The disposition is request-local; it does not fence later admission.
    observe(&mut f.supervisor);
    let later = support::proposal(&oversight::spec(
        &f.supervisor.host().unwrap(),
        b"later-publication",
    ));
    f.actor.submit(REQUEST + 2, &later).unwrap();
}

#[test]
fn separate_abandonment_custody_and_explicit_control_mount_are_required() {
    let mut f = unknown();
    let permitted = next(&mut f.facade, OPERATION);
    let command = action(&permitted);
    f.facade.set_abandonment(None).unwrap();
    let (resolution, resolution_grant) = f.supervisor
        .resolution_port(4, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    f.facade.set_resolution(Some(resolution)).unwrap();
    let original = image(&f.root);
    let query = next_request(&f.facade, REQUEST, OPERATION);
    for request in [query.clone(), command.clone()] {
        refused(
            &f.facade.dispatch(&access(true, true), request),
            ReasonCode::CapabilityUnsupported,
        );
    }
    // Omitting the discriminator retains the existing reconciliation profile.
    let mut legacy = query.clone();
    legacy.arguments = ReadArguments::ResolutionNext { operation: OPERATION };
    let wire = encode_request(&legacy).unwrap();
    assert!(!wire.contains("\"action\""));
    assert_eq!(decode_request(wire.as_bytes()).unwrap(), legacy);
    let observed = f.facade.exchange(&access(true, true), wire.as_bytes());
    let PublicationResult::ResolutionNext(linked) = observed.result else {
        panic!("legacy reconciliation profile");
    };
    let legacy_action = linked.action().unwrap().expect("original reconciliation remains offered");
    assert!(matches!(legacy_action.arguments, ReadArguments::Resolve { .. }));
    assert!(!encode_request(&legacy_action).unwrap().contains("abandon_unknown"));

    let (port, grant) = f.supervisor
        .abandonment_port(8, 2, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    f.facade.set_abandonment(Some(port.clone())).unwrap();
    let mut legacy_mount = PublicationTower::new_reconciliation_operator(
        mount(), f.actor.clone(), PublicationLimits::default(),
    ).unwrap();
    legacy_mount.set_abandonment(Some(port.clone())).unwrap();
    let mut actor_mount = PublicationTower::new(
        mount(), f.actor.clone(), PublicationLimits::default(),
    ).unwrap();
    actor_mount.set_abandonment(Some(port)).unwrap();
    for request in [query.clone(), command.clone()] {
        refused(
            &legacy_mount.dispatch(&access(true, true), request.clone()),
            ReasonCode::CapabilityUnsupported,
        );
        // Even operator=true and a retained native port cannot upgrade actor intake.
        refused(
            &actor_mount.dispatch(&access(true, true), request.clone()),
            ReasonCode::CapabilityUnsupported,
        );
        refused(
            &f.facade.dispatch(&access(false, true), request.clone()),
            ReasonCode::DeniedByPolicy,
        );
        refused(
            &f.facade.dispatch(&access(true, false), request),
            ReasonCode::PurposeMismatch,
        );
    }
    let foreign = TowerAccess::new(
        CallerScope::full_tenant_access("2"), true, vec![PurposeTag::Permit], 0,
    );
    refused(
        &f.facade.dispatch(&foreign, command.clone()),
        ReasonCode::DeniedByPolicy,
    );
    let mut proposal = TowerRequest::new("operator-cannot-submit", mount(), Verb::Propose);
    proposal.arguments = ReadArguments::Propose {
        request: REQUEST + 1, proposal: f.proposal.clone(),
    };
    proposal.idempotency_key = Some((REQUEST + 1).to_string());
    refused(
        &f.facade.exchange(&access(true, true), encode_request(&proposal).unwrap().as_bytes()),
        ReasonCode::CapabilityUnsupported,
    );
    let poll = TowerRequest::new(
        "operator-cannot-acquire-ticket", f.facade.request_address(REQUEST).unwrap(), Verb::Get,
    );
    refused(
        &f.facade.dispatch(&access(true, true), poll),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 8);
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(resolution_grant.remaining_operations(), 2);
    assert_eq!(image(&f.root), original);
    let completed = f.facade.dispatch(&access(true, true), command);
    assert_eq!(receipt(&completed).stage(), ActionState::IrrecoverablyUnknown);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(resolution_grant.remaining_operations(), 2);
}

#[test]
fn exact_predecessor_and_explicit_action_are_checked_before_spending_or_abandoning() {
    let mut f = unknown();
    let proposed = next(&mut f.facade, OPERATION);
    let command = action(&proposed);
    let original = image(&f.root);
    let queries = f.grant.remaining_queries();
    let operations = f.grant.remaining_operations();
    let wire = encode_request(&command).unwrap();
    let malformed = [
        wire.replace("abandon_unknown", "hold"),
        wire.replace("abandon_unknown", "abandon_all"),
        wire.replace("\"operation\":\"5009\"", "\"operation\":5009"),
        wire.replace("\"operation\":\"5009\"", "\"operation\":\"0\""),
        wire.replace(
            "\"action\":\"abandon_unknown\"",
            "\"action\":\"abandon_unknown\",\"now\":\"999\"",
        ),
        wire.replace(
            "\"action\":\"abandon_unknown\"",
            "\"action\":\"abandon_unknown\",\"action\":\"abandon_unknown\"",
        ),
        wire.replace("\"origin\":\"acknowledged_owner\"", "\"origin\":\"caller\""),
        wire.replace("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
        wire.replace("\"request\":\"701\"", "\"request\":701"),
        wire.replace("\"scope\":{", "\"scope\":{\"extra\":\"1\","),
    ];
    for input in malformed {
        assert_ne!(input, wire, "negative fixture must change the original request");
        assert!(decode_request(input.as_bytes()).is_err());
        refused(
            &f.facade.exchange(&access(true, true), input.as_bytes()),
            ReasonCode::IntegrityFailure,
        );
    }
    for generation in [None, Some(REGISTRATION + 1)] {
        let mut changed = command.clone();
        changed.address.generation = generation;
        refused(
            &f.facade.dispatch(&access(true, true), changed),
            ReasonCode::StaleGeneration,
        );
    }
    let mut wrong_idempotency = command.clone();
    wrong_idempotency.idempotency_key = Some((OPERATION + 1).to_string());
    refused(
        &f.facade.dispatch(&access(true, true), wrong_idempotency),
        ReasonCode::IntegrityFailure,
    );
    let mut wrong_namespace = command.clone();
    wrong_namespace.address.id = wrong_namespace.address.id.replace("-701", "-0701");
    refused(
        &f.facade.dispatch(&access(true, true), wrong_namespace),
        ReasonCode::MissingRequiredObservation,
    );
    for change in 0..7 {
        let mut changed = command.clone();
        let ReadArguments::AbandonUnknown { expected, .. } = &mut changed.arguments else {
            panic!("abandonment");
        };
        match change {
            0 => expected.scope.principal += 1,
            1 => expected.request += 1,
            2 => expected.attempt += 1,
            3 => expected.request_generation += 1,
            4 => expected.journal_revision += 1,
            5 => expected.control_sequence += 1,
            _ => expected.recorded_at = Some(ElapsedTick(expected.recorded_at.unwrap().0 + 1)),
        }
        let denied = f.facade.dispatch(&access(true, true), changed);
        if change < 2 {
            refused(&denied, ReasonCode::IntegrityFailure);
        } else {
            assert_eq!(native(&denied), &Err(FileAbandonmentError::StalePredecessor));
        }
        assert_eq!(image(&f.root), original);
        assert_eq!(f.grant.remaining_queries(), queries);
        assert_eq!(f.grant.remaining_operations(), operations);
    }
    let completed = f.facade.dispatch(&access(true, true), command.clone());
    let after = receipt(&completed).after();
    let committed = image(&f.root);
    let mut conflicting = command;
    if let ReadArguments::AbandonUnknown { expected, .. } = &mut conflicting.arguments {
        *expected = after;
    }
    assert_eq!(
        native(&f.facade.dispatch(&access(true, true), conflicting)),
        &Err(FileAbandonmentError::IdempotencyConflict)
    );
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.grant.remaining_operations(), operations - 1);
}

#[test]
fn only_recorded_unknown_can_be_abandoned_and_an_old_offer_cannot_dispose_a_new_cut() {
    let mut f = authorized();
    for dispatched in [false, true] {
        if dispatched {
            let mut host = f.supervisor.host_mut().unwrap();
            oversight::dispatch(&mut host, &f.keys);
        }
        let original = image(&f.root);
        let before = f.supervisor.host().unwrap().inspect();
        let proposed = next(&mut f.facade, OPERATION);
        assert_eq!(
            plan(&proposed).stage(),
            if dispatched { ActionState::Dispatching } else { ActionState::Authorized }
        );
        assert_eq!(plan(&proposed).block_reason(), Some(FileAbandonmentBlockReason::NotUnknown));
        assert!(!plan(&proposed).can_abandon());
        let PublicationResult::AbandonmentNext(linked) = &proposed.result else {
            panic!("original abandonment plan");
        };
        assert!(linked.action().unwrap().is_none());
        let mut forged = TowerRequest::new(
            "unsupported-abandonment", f.facade.request_address(REQUEST).unwrap(), Verb::Resolve,
        );
        forged.arguments = ReadArguments::AbandonUnknown {
            operation: OPERATION, expected: plan(&proposed).basis(),
        };
        forged.idempotency_key = Some(OPERATION.to_string());
        assert_eq!(
            native(&f.facade.dispatch(&access(true, true), forged)),
            &Err(FileAbandonmentError::Unsupported)
        );
        assert_eq!(f.grant.remaining_operations(), 8);
        assert_eq!(image(&f.root), original);
        assert_eq!(f.supervisor.host().unwrap().inspect(), before);
    }
    {
        let mut host = f.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.reconcile(revision, f.keys.automatic.attempt()).unwrap();
    }
    let proposed = next(&mut f.facade, OPERATION);
    let stale = action(&proposed);
    {
        // A genuine subsequent observation invalidates the entire predecessor.
        let mut host = f.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let changed = image(&f.root);
    assert_eq!(
        native(&f.facade.dispatch(&access(true, true), stale)),
        &Err(FileAbandonmentError::StalePredecessor)
    );
    assert_eq!(image(&f.root), changed);
    assert_eq!(f.grant.remaining_operations(), 8);
    let current = next(&mut f.facade, OPERATION);
    let completed = f.facade.dispatch(&access(true, true), action(&current));
    assert_eq!(receipt(&completed).stage(), ActionState::IrrecoverablyUnknown);
    let terminal = next(&mut f.facade, OPERATION + 1);
    assert!(!plan(&terminal).can_abandon());
    assert_eq!(
        plan(&terminal).block_reason(),
        Some(FileAbandonmentBlockReason::IrrecoverablyUnknown)
    );
    let PublicationResult::AbandonmentNext(linked) = terminal.result else {
        panic!("terminal abandonment plan");
    };
    assert!(linked.action().unwrap().is_none());
}

#[test]
fn shared_grant_is_revocable_and_foreign_equal_scope_cannot_replace_its_owner() {
    let mut f = unknown();
    let (port, grant) = f.supervisor
        .abandonment_port(2, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    f.facade.set_abandonment(Some(port.clone())).unwrap();
    let mut second = tower(f.actor.clone());
    second.set_abandonment(Some(port.clone())).unwrap();
    let proposed = next(&mut f.facade, OPERATION);
    let command = action(&proposed);
    let completed = second.dispatch(&access(true, true), command.clone());
    assert_eq!(receipt(&completed).stage(), ActionState::IrrecoverablyUnknown);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(
        native(&f.facade.dispatch(&access(true, true), command.clone())),
        native(&completed)
    );
    let committed = image(&f.root);
    {
        let _host = f.supervisor.host_mut().unwrap();
        assert!(matches!(
            native(&second.dispatch(&access(true, true), command.clone())),
            Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })
        ));
    }
    let foreign_root = Directory::new();
    let (_, foreign, _, _) = create(&foreign_root);
    let (foreign_port, foreign_grant) = foreign
        .abandonment_port(2, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    assert!(f.facade.set_abandonment(Some(foreign_port)).is_err());
    refused(
        &next(&mut f.facade, OPERATION + 1),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(foreign_grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_operations(), 1);
    f.facade.set_abandonment(Some(port)).unwrap();
    grant.revoke();
    assert!(matches!(
        native(&f.facade.dispatch(&access(true, true), command)),
        Ok(ActorKnowledge::Withheld { authority_required: "supervisor_abandonment" })
    ));
    assert!(matches!(
        native_next(&next(&mut second, OPERATION + 1)),
        Ok(ActorKnowledge::Withheld { authority_required: "supervisor_abandonment" })
    ));
    assert_eq!(image(&f.root), committed);
}

#[test]
fn canonical_abandonment_results_seal_correlation_and_report_only_retained_liability() {
    let mut f = unknown();
    let (port, grant) = f.supervisor
        .abandonment_port(2, 2, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    f.facade.set_abandonment(Some(port)).unwrap();
    let proposed = next(&mut f.facade, OPERATION);
    let command = action(&proposed);
    let missing_request = next_request(&f.facade, REQUEST + 90, OPERATION + 1);
    let missing = f.facade.dispatch(&access(true, true), missing_request);
    let exhausted = next(&mut f.facade, OPERATION + 2);
    let completed = f.facade.dispatch(&access(true, true), command.clone());
    let parsed = document(&completed);
    let envelope = field(&parsed, "envelope").as_object().unwrap();
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect()
    );
    let body = field(field(&parsed, "result"), "value");
    assert_eq!(field(body, "schema").as_str(), Some("fa.publication-abandonment/1"));
    assert_eq!(field(body, "scope").as_str(), Some("original_unknown_request"));
    assert_eq!(field(body, "idempotency_scope").as_str(), Some("live_abandonment_grant"));
    assert_eq!(field(body, "may_write_journal").as_bool(), Some(true));
    for name in [
        "publication_attempted", "seal_attempted", "clock_read", "effect_authority",
        "payload_disclosed",
    ] {
        assert_eq!(field(body, name).as_bool(), Some(false));
    }
    let value = field(field(body, "knowledge"), "value");
    assert_eq!(field(value, "before"), field(body, "expected"));
    assert_eq!(field(value, "before_stage").as_str(), Some("unknown"));
    assert_eq!(field(value, "stage").as_str(), Some("irrecoverably_unknown"));
    assert_eq!(field(value, "charged_units").as_str(), Some("16"));
    assert!(value.get("refunded_units").is_none());
    let observed = document(&proposed);
    let plan_body = field(field(&observed, "result"), "value");
    assert_eq!(
        field(plan_body, "schema").as_str(),
        Some("fa.publication-abandonment-next/1")
    );
    let plan_value = field(field(plan_body, "knowledge"), "value");
    for name in ["cost_estimate", "expected_information_gain", "ranking", "current_eligibility"] {
        assert_eq!(field(field(plan_value, name), "state").as_str(), Some("unknown"));
    }
    let unavailable = {
        let _host = f.supervisor.host_mut().unwrap();
        f.facade.dispatch(&access(true, true), command.clone())
    };
    grant.revoke();
    let withheld_next = next(&mut f.facade, OPERATION + 1);
    let withheld = f.facade.dispatch(&access(true, true), command);
    for response in [proposed, missing, exhausted, completed, unavailable, withheld_next, withheld] {
        let json = response.to_json().unwrap();
        assert!(!json.contains("PRIVATE-SOURCE-BYTES"));
        assert!(!json.contains("Approve this exact publication?"));
        for change in 0..4 {
            let mut altered = response.clone();
            match change {
                0 => altered.envelope.address.id.push('9'),
                1 => altered.envelope.address.generation = None,
                2 => altered.envelope.operation_id.push('9'),
                _ => altered.envelope.idempotency_key = Some("90099".to_owned()),
            }
            assert_eq!(altered.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
        let mut tiny = response;
        tiny.envelope.limits.max_response_bytes = 1;
        assert_eq!(tiny.to_json(), Err(TowerEncodingError::OutputLimit));
    }
}

fn snapshot_basis() -> Basis {
    let proof = ScopedProof::Unsupported { scope_id: 1 };
    Basis::new(
        BasisIdentity {
            subject_id: 3,
            question_id: 1,
            profile_id: 1,
            semantic_generation: REGISTRATION,
            policy_epoch: 0,
            model_epoch: 0,
            tokenizer_epoch: 0,
            codec_epoch: 0,
            control_seq: 0,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: proof,
            complete_for_contract: proof,
            semantically_valid: proof,
            available_for_replay: proof,
        },
    )
}

#[test]
fn imported_snapshot_and_branch_never_dispatch_original_abandonment_or_spend_read_budget() {
    let mut f = unknown();
    let proposed = next(&mut f.facade, OPERATION);
    let command = action(&proposed);
    let query = next_request(&f.facade, REQUEST, OPERATION);
    let branch = RootCx::new_nucleus(1, 3, 0).mint_experiment(9).unwrap();
    for journal in [
        DomainEventJournal::new_production("1", "3"),
        DomainEventJournal::new_branch(&branch, "1", "3", 9),
    ] {
        let mut snapshot = TowerBuilder::new(
            journal, snapshot_basis(), TowerLimits::default(), 2,
        ).unwrap().build();
        for request in [query.clone(), command.clone()] {
            let refused = snapshot.dispatch(&access(true, true), request);
            assert_eq!(refused.envelope.reason_code, Some(ReasonCode::CapabilityUnsupported));
            assert_eq!(snapshot.remaining_read_units(), 2);
            assert_eq!(snapshot.journal_control_seq(), 0);
        }
    }
    assert_eq!(f.grant.remaining_queries(), 31);
    assert_eq!(f.grant.remaining_operations(), 8);
    let completed = f.facade.dispatch(&access(true, true), command);
    assert_eq!(receipt(&completed).stage(), ActionState::IrrecoverablyUnknown);
}
