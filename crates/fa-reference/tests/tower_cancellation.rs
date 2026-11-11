//! Scoped cancellation goes through the original durable Cancel transition.
//! Real congress and both publication keys exercise a permitted unsent effect;
//! dispatched work and unrelated requests retain their original lifecycle.
#![cfg(unix)]

#[path = "tower_cancellation/learned.rs"]
mod learned;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileCancellationBlockReason, FileCancellationError, FileCancellationPlan,
    FileCancellationPlanResult, FileCancellationReceipt, FileCancellationResult,
    FileSupervisorCancellationGrant, MAX_CANCELLATION_REPLAY_EVENTS, MAX_RESOLUTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::delivery::{
    EndpointOutcome, persistent::observed::FileHumanReviewer,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorOutcome, ActorProposal, Knowledge as ActorKnowledge,
    UnknownReason as ActorUnknown,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerRequest,
    Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
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
        "next-undispatched-cancellation",
        facade.request_address(key).unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::CancellationNext { operation };
    request
}

fn next(facade: &mut PublicationTower, operation: u64) -> PublicationResponse {
    let request = next_request(facade, REQUEST, operation);
    facade.exchange(
        &access(true, true),
        encode_request(&request).unwrap().as_bytes(),
    )
}

fn native_next(response: &PublicationResponse) -> &FileCancellationPlanResult {
    let PublicationResult::CancellationNext(linked) = &response.result else {
        panic!("expected original cancellation plan: {response:?}");
    };
    linked.native()
}

fn plan(response: &PublicationResponse) -> &FileCancellationPlan {
    let Ok(ActorKnowledge::Known { value, basis }) = native_next(response) else {
        panic!("expected acknowledged plan: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    value
}

fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::CancellationNext(linked) = &response.result else {
        panic!("original cancellation plan");
    };
    linked
        .action()
        .unwrap()
        .expect("one permitted undispatched cancellation")
}

fn native(response: &PublicationResponse) -> &FileCancellationResult {
    let PublicationResult::Cancellation(linked) = &response.result else {
        panic!("expected original cancellation result: {response:?}");
    };
    linked.native()
}

fn receipt(response: &PublicationResponse) -> &FileCancellationReceipt {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected original cancellation acknowledgment: {response:?}");
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
    grant: FileSupervisorCancellationGrant,
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
        .cancellation_port(32, 8, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    facade.set_cancellation(Some(port)).unwrap();
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

#[test]
fn explicit_next_cancels_one_original_reservation_once_and_keeps_later_publication_available() {
    let mut f = authorized();
    let original = image(&f.root);
    let state = f.supervisor.host().unwrap().inspect();
    let proposed = next(&mut f.facade, OPERATION);
    assert_eq!(plan(&proposed).stage(), ActionState::Authorized);
    assert!(plan(&proposed).can_cancel());
    assert_eq!(plan(&proposed).basis().request, REQUEST);
    assert_ne!(plan(&proposed).basis().attempt, REQUEST);
    assert_eq!(f.grant.remaining_operations(), 8);
    assert_eq!(image(&f.root), original);
    assert_eq!(f.supervisor.host().unwrap().inspect(), state);

    let command = action(&proposed);
    assert_eq!(command.verb, Verb::Resolve);
    assert_eq!(command.idempotency_key, Some(OPERATION.to_string()));
    assert!(matches!(
        command.arguments,
        ReadArguments::CancelUndispatched { .. }
    ));
    let wire = encode_request(&command).unwrap();
    assert!(wire.contains("\"action\":\"cancel_undispatched\""));
    assert_eq!(decode_request(wire.as_bytes()).unwrap(), command);
    let parsed = document(&proposed);
    let body = field(field(&parsed, "result"), "value");
    let value = field(field(body, "knowledge"), "value");
    assert_eq!(
        field(value, "action"),
        &strict_json::parse(wire.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap()
    );

    // A real acknowledged Cancel invalidates the prior admission snapshot.
    observe(&mut f.supervisor);
    let completed = f.facade.exchange(&access(true, true), wire.as_bytes());
    let result = receipt(&completed);
    assert_eq!(result.before(), plan(&proposed).basis());
    assert_eq!(result.before_stage(), ActionState::Authorized);
    assert_eq!(result.stage(), ActionState::Cancelled);
    assert_eq!(result.refunded_units(), 16);
    assert_eq!(
        result.after().journal_revision,
        result.before().journal_revision + 1
    );
    assert_eq!(result.after().recorded_at, result.before().recorded_at);
    let after = f.supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 0);
    assert_eq!(
        after.control.ledger.reserved,
        state.control.ledger.reserved - 16
    );
    assert_eq!(after.control.ledger.charged, state.control.ledger.charged);
    assert_eq!(after.control.ledger.elapsed, state.control.ledger.elapsed);
    assert_eq!(after.control.ledger.epoch, state.control.ledger.epoch);
    let ticket = f.actor.submit(REQUEST, &f.proposal).unwrap();
    assert!(matches!(
        f.actor.poll(&ticket),
        ActorKnowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        }
    ));
    assert_eq!(f.grant.remaining_operations(), 7);
    assert!(matches!(
        f.actor.submit(REQUEST + 1, &f.proposal),
        Err(ActorError::Unavailable)
    ));

    let committed = image(&f.root);
    assert_eq!(
        native(&f.facade.exchange(&access(true, true), wire.as_bytes())),
        native(&completed)
    );
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.grant.remaining_operations(), 7);

    observe(&mut f.supervisor);
    f.actor.submit(REQUEST + 1, &f.proposal).unwrap();
    let keys = ready(&mut f.supervisor, &f.reviewer, REQUEST + 1);
    {
        let mut host = f.supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        assert_eq!(
            host.publish(revision, keys.automatic.attempt()).unwrap(),
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
    }
    assert_eq!(f.supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(
        f.supervisor
            .host()
            .unwrap()
            .inspect()
            .control
            .ledger
            .charged,
        16
    );
    let progressed = image(&f.root);
    assert_eq!(
        native(&f.facade.dispatch(&access(true, true), command)),
        native(&completed)
    );
    assert_eq!(image(&f.root), progressed);
    assert_eq!(f.grant.remaining_operations(), 7);
}

#[test]
fn separate_cancellation_custody_and_explicit_control_mount_are_required() {
    let mut f = authorized();
    f.facade.set_cancellation(None).unwrap();
    let (resolution, resolution_grant) = f
        .supervisor
        .resolution_port(4, 2, MAX_RESOLUTION_REPLAY_EVENTS)
        .unwrap();
    f.facade.set_resolution(Some(resolution)).unwrap();
    let original = image(&f.root);
    let query = next_request(&f.facade, REQUEST, OPERATION);
    refused(
        &f.facade.dispatch(&access(true, true), query.clone()),
        ReasonCode::CapabilityUnsupported,
    );
    let mut legacy = query.clone();
    legacy.arguments = ReadArguments::ResolutionNext {
        operation: OPERATION,
    };
    let observed = f.facade.exchange(
        &access(true, true),
        encode_request(&legacy).unwrap().as_bytes(),
    );
    let PublicationResult::ResolutionNext(linked) = observed.result else {
        panic!("legacy reconciliation profile");
    };
    assert!(linked.action().unwrap().is_none());

    let (port, grant) = f
        .supervisor
        .cancellation_port(4, 2, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    f.facade.set_cancellation(Some(port.clone())).unwrap();
    let mut legacy_mount = PublicationTower::new_reconciliation_operator(
        mount(),
        f.actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    legacy_mount.set_cancellation(Some(port)).unwrap();
    refused(
        &legacy_mount.dispatch(&access(true, true), query.clone()),
        ReasonCode::CapabilityUnsupported,
    );
    refused(
        &f.facade.dispatch(&access(false, true), query.clone()),
        ReasonCode::DeniedByPolicy,
    );
    refused(
        &f.facade.dispatch(&access(true, false), query.clone()),
        ReasonCode::PurposeMismatch,
    );
    let mut proposal = TowerRequest::new("operator-cannot-submit", mount(), Verb::Propose);
    proposal.arguments = ReadArguments::Propose {
        request: REQUEST + 1,
        proposal: f.proposal.clone(),
    };
    proposal.idempotency_key = Some((REQUEST + 1).to_string());
    refused(
        &f.facade.exchange(
            &access(true, true),
            encode_request(&proposal).unwrap().as_bytes(),
        ),
        ReasonCode::CapabilityUnsupported,
    );
    let poll = TowerRequest::new(
        "operator-cannot-acquire-ticket",
        f.facade.request_address(REQUEST).unwrap(),
        Verb::Get,
    );
    refused(
        &f.facade.dispatch(&access(true, true), poll),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(resolution_grant.remaining_operations(), 2);
    assert_eq!(image(&f.root), original);

    let permitted = next(&mut f.facade, OPERATION);
    let completed = f.facade.dispatch(&access(true, true), action(&permitted));
    assert_eq!(receipt(&completed).stage(), ActionState::Cancelled);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(resolution_grant.remaining_operations(), 2);
}

#[test]
fn exact_cut_and_explicit_action_are_checked_before_spending_or_cancelling() {
    let mut f = authorized();
    let proposed = next(&mut f.facade, OPERATION);
    let command = action(&proposed);
    let original = image(&f.root);
    let queries = f.grant.remaining_queries();
    let operations = f.grant.remaining_operations();
    for malformed in [
        encode_request(&command)
            .unwrap()
            .replace("cancel_undispatched", "hold"),
        encode_request(&command)
            .unwrap()
            .replace("cancel_undispatched", "cancel_all"),
        encode_request(&command)
            .unwrap()
            .replace("\"operation\":\"5009\"", "\"operation\":5009"),
        encode_request(&command)
            .unwrap()
            .replace("\"operation\":\"5009\"", "\"operation\":\"0\""),
        encode_request(&command).unwrap().replace(
            "\"action\":\"cancel_undispatched\"",
            "\"action\":\"cancel_undispatched\",\"now\":\"999\"",
        ),
    ] {
        refused(
            &f.facade.exchange(&access(true, true), malformed.as_bytes()),
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
        let ReadArguments::CancelUndispatched { expected, .. } = &mut changed.arguments else {
            panic!("cancellation");
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
            assert_eq!(
                native(&denied),
                &Err(FileCancellationError::StalePredecessor)
            );
        }
        assert_eq!(image(&f.root), original);
        assert_eq!(f.grant.remaining_queries(), queries);
        assert_eq!(f.grant.remaining_operations(), operations);
    }
    let completed = f.facade.dispatch(&access(true, true), command.clone());
    let after = receipt(&completed).after();
    let committed = image(&f.root);
    let mut conflicting = command;
    if let ReadArguments::CancelUndispatched { expected, .. } = &mut conflicting.arguments {
        *expected = after;
    }
    assert_eq!(
        native(&f.facade.dispatch(&access(true, true), conflicting)),
        &Err(FileCancellationError::IdempotencyConflict)
    );
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.grant.remaining_operations(), operations - 1);
}

#[test]
fn dispatched_and_unknown_requests_cannot_be_cancelled_or_refunded() {
    let mut f = authorized();
    let before_dispatch = next(&mut f.facade, OPERATION);
    let stale = action(&before_dispatch);
    {
        let mut host = f.supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &f.keys);
    }
    let dispatched = image(&f.root);
    assert_eq!(
        native(&f.facade.dispatch(&access(true, true), stale)),
        &Err(FileCancellationError::StalePredecessor)
    );
    assert_eq!(image(&f.root), dispatched);
    for unknown in [false, true] {
        if unknown {
            let mut host = f.supervisor.host_mut().unwrap();
            let revision = host.revision();
            host.reconcile(revision, f.keys.automatic.attempt())
                .unwrap();
        }
        let before = image(&f.root);
        let state = f.supervisor.host().unwrap().inspect();
        let proposed = next(&mut f.facade, OPERATION);
        assert_eq!(
            plan(&proposed).stage(),
            if unknown {
                ActionState::Unknown
            } else {
                ActionState::Dispatching
            }
        );
        assert_eq!(
            plan(&proposed).block_reason(),
            Some(FileCancellationBlockReason::AlreadyDispatched)
        );
        assert!(!plan(&proposed).can_cancel());
        let PublicationResult::CancellationNext(linked) = &proposed.result else {
            panic!("cancellation plan");
        };
        assert!(linked.action().unwrap().is_none());
        let mut forged = TowerRequest::new(
            "unsupported-cancellation",
            f.facade.request_address(REQUEST).unwrap(),
            Verb::Resolve,
        );
        forged.arguments = ReadArguments::CancelUndispatched {
            operation: OPERATION,
            expected: plan(&proposed).basis(),
        };
        forged.idempotency_key = Some(OPERATION.to_string());
        assert_eq!(
            native(&f.facade.dispatch(&access(true, true), forged)),
            &Err(FileCancellationError::Unsupported)
        );
        assert_eq!(f.grant.remaining_operations(), 8);
        assert_eq!(image(&f.root), before);
        assert_eq!(f.supervisor.host().unwrap().inspect(), state);
        assert_eq!(state.control.ledger.charged, 16);
        assert_eq!(state.executions, 0);
    }
}

#[test]
fn shared_grant_custody_is_revocable_and_foreign_equal_scope_cannot_replace_it() {
    let mut f = authorized();
    let (port, grant) = f
        .supervisor
        .cancellation_port(2, 1, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    f.facade.set_cancellation(Some(port.clone())).unwrap();
    let mut second = tower(f.actor.clone());
    second.set_cancellation(Some(port.clone())).unwrap();
    let proposed = next(&mut f.facade, OPERATION);
    let command = action(&proposed);
    let completed = second.dispatch(&access(true, true), command.clone());
    assert_eq!(receipt(&completed).stage(), ActionState::Cancelled);
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
            Ok(ActorKnowledge::Unknown {
                reason: ActorUnknown::ControllerUnavailable
            })
        ));
    }
    let foreign_root = Directory::new();
    let (_, foreign, _, _) = create(&foreign_root);
    let (foreign_port, foreign_grant) = foreign
        .cancellation_port(2, 1, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    assert!(f.facade.set_cancellation(Some(foreign_port)).is_err());
    refused(
        &next(&mut f.facade, OPERATION + 1),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(foreign_grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_operations(), 1);
    f.facade.set_cancellation(Some(port)).unwrap();
    grant.revoke();
    assert!(matches!(
        native(&f.facade.dispatch(&access(true, true), command)),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_cancellation"
        })
    ));
    assert!(matches!(
        native_next(&next(&mut second, OPERATION + 1)),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_cancellation"
        })
    ));
    assert_eq!(image(&f.root), committed);
}

#[test]
fn canonical_cancellation_results_bind_every_correlation_and_preserve_unknown_economics() {
    let mut f = authorized();
    let (port, grant) = f
        .supervisor
        .cancellation_port(2, 2, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    f.facade.set_cancellation(Some(port)).unwrap();
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
    assert_eq!(
        field(body, "schema").as_str(),
        Some("fa.publication-cancellation/1")
    );
    assert_eq!(
        field(body, "scope").as_str(),
        Some("original_undispatched_request")
    );
    assert_eq!(
        field(body, "idempotency_scope").as_str(),
        Some("live_cancellation_grant")
    );
    assert_eq!(field(body, "may_write_journal").as_bool(), Some(true));
    for name in [
        "publication_attempted",
        "seal_attempted",
        "clock_read",
        "effect_authority",
        "payload_disclosed",
    ] {
        assert_eq!(field(body, name).as_bool(), Some(false));
    }
    let value = field(field(body, "knowledge"), "value");
    assert_eq!(field(value, "before"), field(body, "expected"));
    assert_eq!(field(value, "before_stage").as_str(), Some("authorized"));
    assert_eq!(field(value, "stage").as_str(), Some("cancelled"));
    assert_eq!(field(value, "refunded_units").as_str(), Some("16"));
    let observed = document(&proposed);
    let plan_value = field(
        field(field(field(&observed, "result"), "value"), "knowledge"),
        "value",
    );
    for name in [
        "cost_estimate",
        "expected_information_gain",
        "ranking",
        "current_eligibility",
    ] {
        assert_eq!(
            field(field(plan_value, name), "state").as_str(),
            Some("unknown")
        );
    }
    let unavailable = {
        let _host = f.supervisor.host_mut().unwrap();
        f.facade.dispatch(&access(true, true), command.clone())
    };
    grant.revoke();
    let withheld_next = next(&mut f.facade, OPERATION + 1);
    let withheld = f.facade.dispatch(&access(true, true), command);
    for response in [
        proposed,
        missing,
        exhausted,
        completed,
        unavailable,
        withheld_next,
        withheld,
    ] {
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
