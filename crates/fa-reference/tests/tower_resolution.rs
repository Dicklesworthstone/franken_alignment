//! Native Next emits a command that reconciles the same original request.
//! Real congress, both keys and the original endpoint provide the positive
//! controls; the operator facade never supplies a new publication or clock.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;
#[path = "tower_resolution/learned.rs"]
mod learned;

use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, observed::FileOversight};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileResolutionBlockReason, FileResolutionError, FileResolutionPlan, FileResolutionPlanResult,
    FileResolutionReceipt, FileResolutionResult, FileSupervisorResolutionGrant,
    MAX_RESOLUTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{PublicationLimits, PublicationResponse, PublicationResult, PublicationTower};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError,
    TowerRequest, Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use std::collections::BTreeSet;
use support::{Directory, Port, Supervisor, create, observe, oversight, profile, ready};

const REQUEST: u64 = 701;
const REGISTRATION: u64 = 41;
const OPERATION: u64 = 5009;

fn tower(port: Port) -> PublicationTower {
    PublicationTower::new_reconciliation_operator(
        Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap(),
        port, PublicationLimits::default(),
    ).unwrap()
}

fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"), operator,
        if permit { vec![PurposeTag::Permit, PurposeTag::Experiment] } else { Vec::new() }, 0,
    )
}

fn next_request(facade: &PublicationTower, key: u64, operation: u64) -> TowerRequest {
    let mut request = TowerRequest::new("next-original-reconciliation", facade.request_address(key).unwrap(), Verb::Next);
    request.arguments = ReadArguments::ResolutionNext { operation };
    request
}

fn next(facade: &mut PublicationTower, operation: u64) -> PublicationResponse {
    let query = next_request(facade, REQUEST, operation);
    facade.exchange(&access(true, true), encode_request(&query).unwrap().as_bytes())
}

fn native_next(response: &PublicationResponse) -> &FileResolutionPlanResult {
    let PublicationResult::ResolutionNext(linked) = &response.result else {
        panic!("expected native reconciliation plan: {response:?}");
    };
    linked.native()
}

fn plan(response: &PublicationResponse) -> &FileResolutionPlan {
    let Ok(ActorKnowledge::Known { value, basis }) = native_next(response) else {
        panic!("expected original acknowledged plan: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    value
}

fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::ResolutionNext(linked) = &response.result else { panic!("native plan"); };
    linked.action().unwrap().expect("one connected original operation")
}

fn native_resolution(response: &PublicationResponse) -> &FileResolutionResult {
    let PublicationResult::Resolution(linked) = &response.result else {
        panic!("expected original reconciliation result: {response:?}");
    };
    linked.native()
}

fn receipt(response: &PublicationResponse) -> &FileResolutionReceipt {
    let Ok(ActorKnowledge::Known { value, basis }) = native_resolution(response) else {
        panic!("expected acknowledged native receipt: {response:?}");
    };
    assert_eq!(value.after(), *basis);
    assert!(!response.envelope.retry_safe);
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
    strict_json::parse(response.to_json().unwrap().as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap()
}

struct Fixture {
    root: Directory,
    actor: Port,
    supervisor: Supervisor,
    proposal: ActorProposal,
    facade: PublicationTower,
    grant: FileSupervisorResolutionGrant,
}

fn dispatched(published: bool) -> Fixture {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, REQUEST);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        if published {
            let revision = host.revision();
            host.publish(revision, keys.automatic.attempt()).unwrap();
        }
    }
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.resolution_port(32, 8, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    facade.set_resolution(Some(port)).unwrap();
    Fixture { root, actor, supervisor, proposal, facade, grant }
}

#[test]
fn next_emits_a_canonical_executable_request_that_accepts_the_original_publication_receipt_once() {
    let mut f = dispatched(true);
    let before = image(&f.root);
    let state = f.supervisor.host().unwrap().inspect();
    let response = next(&mut f.facade, OPERATION);
    assert_eq!(plan(&response).stage(), ActionState::Dispatching);
    assert_eq!(plan(&response).basis().request, REQUEST);
    assert_ne!(plan(&response).basis().attempt, REQUEST);
    assert!(plan(&response).can_reconcile());
    assert_eq!(image(&f.root), before);
    assert_eq!(f.supervisor.host().unwrap().inspect(), state);
    assert_eq!(f.grant.remaining_operations(), 8);

    let command = action(&response);
    assert_eq!(command.address.generation, Some(REGISTRATION));
    assert_eq!(command.idempotency_key, Some(OPERATION.to_string()));
    let encoded = encode_request(&command).unwrap();
    let decoded = decode_request(encoded.as_bytes()).unwrap();
    assert_eq!(decoded, command);
    let parsed = document(&response);
    let body = field(field(&parsed, "result"), "value");
    let value = field(field(body, "knowledge"), "value");
    assert_eq!(field(value, "action"), &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap());
    for name in ["cost_estimate", "expected_information_gain", "ranking", "current_eligibility"] {
        let unknown = field(value, name);
        assert_eq!(field(unknown, "state").as_str(), Some("unknown"));
        assert_eq!(field(unknown, "reason").as_str(), Some("controller_unavailable"));
    }
    let completed = f.facade.dispatch(&access(true, true), decoded);
    let original = receipt(&completed);
    assert_eq!(original.before(), plan(&response).basis());
    assert_eq!(original.after().journal_revision, original.before().journal_revision + 1);
    assert_eq!(original.after().recorded_at, original.before().recorded_at);
    assert_eq!(original.stage(), ActionState::Confirmed);
    assert_eq!(original.outcome(), Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    assert_eq!(f.supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(f.supervisor.host().unwrap().inspect().control.ledger.charged, 16);
    assert_eq!(f.grant.remaining_operations(), 7);

    let committed = image(&f.root);
    let repeated = f.facade.exchange(&access(true, true), encoded.as_bytes());
    assert_eq!(native_resolution(&repeated), native_resolution(&completed));
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.grant.remaining_operations(), 7);
    let finished = next(&mut f.facade, OPERATION + 1);
    assert_eq!(plan(&finished).block_reason(), Some(FileResolutionBlockReason::Terminal));
    assert!(!plan(&finished).can_reconcile());
}

#[test]
fn a_real_nonexecution_receipt_refunds_once_without_a_new_publication_or_seal() {
    let mut f = dispatched(false);
    {
        let mut host = f.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(102)).unwrap();
        let attempt = support::attempt(&host, REQUEST);
        let revision = host.revision();
        assert_eq!(host.publish(revision, attempt).unwrap(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    }
    let state = f.supervisor.host().unwrap().inspect();
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 16);
    let proposed = next(&mut f.facade, OPERATION);
    let command = action(&proposed);
    let completed = f.facade.dispatch(&access(true, true), command.clone());
    assert_eq!(receipt(&completed).outcome(), Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed }));
    assert_eq!(receipt(&completed).stage(), ActionState::ConfirmedNotExecuted);
    let after = f.supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 0);
    assert_eq!(after.control.ledger.charged, 0);
    assert_eq!(after.control.ledger.elapsed, state.control.ledger.elapsed);
    let committed = image(&f.root);
    let duplicate = f.facade.dispatch(&access(true, true), command);
    assert_eq!(native_resolution(&duplicate), native_resolution(&completed));
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.supervisor.host().unwrap().inspect(), after);
}

#[test]
fn an_unresolved_query_stays_charged_and_invalidates_only_the_old_admission_snapshot() {
    let mut f = dispatched(false);
    observe(&mut f.supervisor);
    let original = image(&f.root);
    let response = next(&mut f.facade, OPERATION);
    assert_eq!(image(&f.root), original);
    let command = action(&response);
    let unresolved = f.facade.dispatch(&access(true, true), command.clone());
    assert_eq!(receipt(&unresolved).outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(receipt(&unresolved).stage(), ActionState::Unknown);
    let state = f.supervisor.host().unwrap().inspect();
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 16);
    assert!(matches!(f.actor.submit(REQUEST + 1, &f.proposal), Err(ActorError::Unavailable)));
    let after = image(&f.root);
    assert_eq!(native_resolution(&f.facade.dispatch(&access(true, true), command)), native_resolution(&unresolved));
    assert_eq!(image(&f.root), after);
    observe(&mut f.supervisor);
    let fresh = f.actor.submit(REQUEST + 1, &f.proposal).unwrap();
    assert_eq!(f.actor.poll(&fresh), ActorKnowledge::Pending { request: REQUEST + 1 });
    assert_eq!(f.supervisor.host().unwrap().inspect().executions, 0);
    assert_eq!(f.supervisor.host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn restricted_operator_profile_refuses_actor_intake_even_with_broad_purpose_tags() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.resolution_port(4, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    facade.set_resolution(Some(port)).unwrap();
    observe(&mut supervisor);
    let before = image(&root);
    let state = supervisor.host().unwrap().inspect();
    let query = next_request(&facade, REQUEST, OPERATION);
    refused(&facade.dispatch(&access(false, true), query.clone()), ReasonCode::DeniedByPolicy);
    refused(&facade.dispatch(&access(true, false), query.clone()), ReasonCode::PurposeMismatch);
    let mut generic_budget = query.clone();
    generic_budget.arguments = ReadArguments::Next { max_cost: u64::MAX };
    refused(&facade.dispatch(&access(true, true), generic_budget), ReasonCode::IntegrityFailure);
    for verb in Verb::ALL {
        if !matches!(verb, Verb::Situation | Verb::Explain | Verb::WhyHeld | Verb::Next | Verb::Resolve | Verb::PolicyReplay) {
            let denied = facade.dispatch(&access(true, true), TowerRequest::new("closed-operator", facade.mount_address().clone(), verb));
            refused(&denied, ReasonCode::CapabilityUnsupported);
        }
    }
    let mut submission = TowerRequest::new("no-operator-payload", facade.mount_address().clone(), Verb::Propose);
    submission.arguments = ReadArguments::Propose { request: REQUEST, proposal: proposal.clone() };
    submission.idempotency_key = Some(REQUEST.to_string());
    refused(&facade.exchange(&access(true, true), encode_request(&submission).unwrap().as_bytes()), ReasonCode::CapabilityUnsupported);
    for arguments in [
        ReadArguments::Precheck { query: 9, proposal: proposal.clone() },
        ReadArguments::Capabilities { query: 9, expected_policy_epoch: 0 },
    ] {
        let verb = if matches!(&arguments, ReadArguments::Precheck { .. }) { Verb::Precheck } else { Verb::Capabilities };
        let mut denied = TowerRequest::new("no-actor-policy-probe", facade.mount_address().clone(), verb);
        denied.arguments = arguments;
        refused(&facade.exchange(&access(true, true), encode_request(&denied).unwrap().as_bytes()), ReasonCode::CapabilityUnsupported);
    }
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(image(&root), before);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    let missing = facade.dispatch(&access(true, true), query);
    assert!(matches!(native_next(&missing), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
    let admitted = actor.submit(REQUEST, &proposal).unwrap();
    assert_eq!(actor.poll(&admitted), ActorKnowledge::Pending { request: REQUEST });
    let reviewing = next(&mut facade, OPERATION);
    assert_eq!(plan(&reviewing).block_reason(), Some(FileResolutionBlockReason::NotDispatched));
    assert_eq!(plan(&reviewing).basis().request_generation, 0);
    assert!(!plan(&reviewing).can_reconcile());
    assert_eq!(grant.remaining_operations(), 2);
}

#[test]
fn caller_registration_and_every_original_predecessor_field_are_checked_before_mutation() {
    let mut f = dispatched(false);
    let proposed = next(&mut f.facade, OPERATION);
    let original = action(&proposed);
    let before = image(&f.root);
    let remaining = f.grant.remaining_operations();
    refused(&f.facade.dispatch(&access(false, true), original.clone()), ReasonCode::DeniedByPolicy);
    refused(&f.facade.dispatch(&access(true, false), original.clone()), ReasonCode::PurposeMismatch);
    for generation in [None, Some(REGISTRATION + 1)] {
        let mut stale = original.clone();
        stale.address.generation = generation;
        refused(&f.facade.dispatch(&access(true, true), stale), ReasonCode::StaleGeneration);
    }
    let mut wrong_key = original.clone();
    wrong_key.idempotency_key = Some((OPERATION + 1).to_string());
    refused(&f.facade.dispatch(&access(true, true), wrong_key), ReasonCode::IntegrityFailure);
    let mut wrong_namespace = original.clone();
    wrong_namespace.address.id = wrong_namespace.address.id.replace("-701", "-0701");
    refused(&f.facade.dispatch(&access(true, true), wrong_namespace), ReasonCode::MissingRequiredObservation);
    for change in 0..7 {
        let mut changed = original.clone();
        let ReadArguments::Resolve { expected, .. } = &mut changed.arguments else { panic!("native request"); };
        match change {
            0 => expected.scope.principal += 1,
            1 => expected.request += 1,
            2 => expected.attempt += 1,
            3 => expected.request_generation += 1,
            4 => expected.journal_revision += 1,
            5 => expected.control_sequence += 1,
            _ => expected.recorded_at = Some(ElapsedTick(expected.recorded_at.unwrap().0 + 1)),
        }
        let response = f.facade.dispatch(&access(true, true), changed);
        if change < 2 {
            refused(&response, ReasonCode::IntegrityFailure);
        } else {
            assert_eq!(native_resolution(&response), &Err(FileResolutionError::StalePredecessor));
        }
        assert_eq!(f.grant.remaining_operations(), remaining);
        assert_eq!(image(&f.root), before);
    }
    let completed = f.facade.dispatch(&access(true, true), original.clone());
    let after = receipt(&completed).after();
    let committed = image(&f.root);
    let mut conflicting = original;
    if let ReadArguments::Resolve { expected, .. } = &mut conflicting.arguments { *expected = after; }
    assert_eq!(native_resolution(&f.facade.dispatch(&access(true, true), conflicting)), &Err(FileResolutionError::IdempotencyConflict));
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.grant.remaining_operations(), remaining - 1);

    let later = next(&mut f.facade, OPERATION + 1);
    let suggested = action(&later);
    {
        let mut host = f.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let changed_owner = image(&f.root);
    assert_eq!(native_resolution(&f.facade.dispatch(&access(true, true), suggested)), &Err(FileResolutionError::StalePredecessor));
    assert_eq!(image(&f.root), changed_owner);
    assert_eq!(f.grant.remaining_operations(), remaining - 1);
}

#[test]
fn native_grant_work_is_shared_revocable_and_never_replaced_by_equal_numeric_scope() {
    let mut f = dispatched(false);
    let (port, grant) = f.supervisor.resolution_port(3, 1, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    f.facade.set_resolution(Some(port.clone())).unwrap();
    let mut second = tower(f.actor.clone());
    second.set_resolution(Some(port.clone())).unwrap();
    let response = next(&mut f.facade, OPERATION);
    let command = action(&response);
    assert!(plan(&next(&mut second, OPERATION + 1)).can_reconcile());
    let completed = second.dispatch(&access(true, true), command.clone());
    assert_eq!(receipt(&completed).outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(grant.remaining_operations(), 0);
    let before = image(&f.root);
    assert_eq!(native_resolution(&f.facade.dispatch(&access(true, true), command.clone())), native_resolution(&completed));
    assert_eq!(image(&f.root), before);
    assert_eq!(plan(&next(&mut second, OPERATION + 1)).block_reason(), Some(FileResolutionBlockReason::OperationLimit));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(native_next(&next(&mut second, OPERATION + 2)), &Err(FileResolutionError::Capacity));

    let foreign_root = Directory::new();
    let (_, foreign, _, _) = create(&foreign_root);
    let (foreign_port, foreign_grant) = foreign.resolution_port(2, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    assert!(f.facade.set_resolution(Some(foreign_port)).is_err());
    refused(&next(&mut f.facade, OPERATION + 2), ReasonCode::CapabilityUnsupported);
    assert_eq!(foreign_grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_operations(), 2);
    f.facade.set_resolution(Some(port)).unwrap();
    grant.revoke();
    assert!(matches!(native_next(&next(&mut f.facade, OPERATION + 2)), Ok(ActorKnowledge::Withheld { authority_required: "supervisor_resolution" })));
    assert!(matches!(native_resolution(&second.dispatch(&access(true, true), command)), Ok(ActorKnowledge::Withheld { authority_required: "supervisor_resolution" })));
    assert_eq!(image(&f.root), before);
}

#[test]
fn borrowed_faulted_and_reopened_owners_cannot_reuse_a_success_cache_as_current_authority() {
    for faulted in [false, true] {
        let mut f = dispatched(true);
        let (port, grant) = f.supervisor.resolution_port(4, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
        f.facade.set_resolution(Some(port.clone())).unwrap();
        let proposed = next(&mut f.facade, OPERATION);
        let command = action(&proposed);
        receipt(&f.facade.dispatch(&access(true, true), command.clone()));
        let remaining = grant.remaining_operations();
        {
            let mut host = f.supervisor.host_mut().unwrap();
            assert!(matches!(native_resolution(&f.facade.dispatch(&access(true, true), command.clone())), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
            assert!(matches!(native_next(&next(&mut f.facade, OPERATION + 1)), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
            if faulted {
                std::fs::write(f.root.store().join("delivery.pending"), b"occupied staging slot").unwrap();
                let revision = host.revision();
                assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
            }
        }
        if faulted {
            assert!(matches!(native_resolution(&f.facade.dispatch(&access(true, true), command.clone())), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
        }
        let before = image(&f.root);
        drop(f.supervisor);
        assert!(matches!(native_resolution(&f.facade.dispatch(&access(true, true), command)), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
        assert_eq!(image(&f.root), before);
        let (host, _) = FileOversight::open(f.root.store(), profile()).unwrap();
        let (new_actor, _new_supervisor) = host.into_actor_gateway();
        let mut reopened = tower(new_actor);
        assert!(reopened.set_resolution(Some(port)).is_err());
        assert_eq!(grant.remaining_operations(), remaining);
    }
}

#[test]
fn canonical_wire_keeps_native_cuts_unknown_economics_and_sealed_all_outcome_correlations() {
    let mut f = dispatched(false);
    let (port, grant) = f.supervisor.resolution_port(2, 2, MAX_RESOLUTION_REPLAY_EVENTS).unwrap();
    f.facade.set_resolution(Some(port)).unwrap();
    let observed = next(&mut f.facade, OPERATION);
    let command = action(&observed);
    let missing_query = next_request(&f.facade, REQUEST + 90, OPERATION + 1);
    let missing = f.facade.dispatch(&access(true, true), missing_query);
    let exhausted = next(&mut f.facade, OPERATION + 2);
    let resolved = f.facade.dispatch(&access(true, true), command.clone());
    let parsed = document(&resolved);
    let envelope = field(&parsed, "envelope").as_object().unwrap();
    assert_eq!(envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(), ENVELOPE_FIELDS.into_iter().collect());
    let body = field(field(&parsed, "result"), "value");
    assert_eq!(field(body, "schema").as_str(), Some("fa.publication-resolution/1"));
    assert_eq!(field(body, "may_write_journal").as_bool(), Some(true));
    assert_eq!(field(body, "publication_attempted").as_bool(), Some(false));
    assert_eq!(field(body, "seal_attempted").as_bool(), Some(false));
    assert_eq!(field(body, "clock_read").as_bool(), Some(false));
    assert_eq!(field(body, "idempotency_scope").as_str(), Some("live_resolution_grant"));
    let expected = field(body, "expected");
    let value = field(field(body, "knowledge"), "value");
    assert_eq!(field(value, "before"), expected);
    assert_eq!(field(field(body, "knowledge"), "basis").get("request").unwrap().as_str(), Some("701"));
    let mut stale_request = command.clone();
    if let ReadArguments::Resolve { operation, expected } = &mut stale_request.arguments {
        *operation += 1;
        expected.journal_revision += 10;
    }
    stale_request.idempotency_key = Some((OPERATION + 1).to_string());
    let stale = f.facade.dispatch(&access(true, true), stale_request);
    let unavailable = {
        let _host = f.supervisor.host_mut().unwrap();
        f.facade.dispatch(&access(true, true), command.clone())
    };
    grant.revoke();
    let withheld_next = next(&mut f.facade, OPERATION + 1);
    let withheld_resolution = f.facade.dispatch(&access(true, true), command);
    for response in [observed, missing, exhausted, resolved, stale, unavailable, withheld_next, withheld_resolution] {
        let json = response.to_json().unwrap();
        assert!(!json.contains("PRIVATE-SOURCE-BYTES"));
        assert!(!json.contains("Approve this exact publication?"));
        assert!(json.len() <= PublicationLimits::default().max_response_bytes);
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
