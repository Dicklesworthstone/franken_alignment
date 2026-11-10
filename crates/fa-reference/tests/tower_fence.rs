//! A Tower fence enters the original durable Stop exactly once.
//! Positive two-key publications distinguish unsent reservations from charged
//! unknown effects; all denials retain the same original owner and bytes.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::{
    RecoveryReserve, observed::{FileHumanReviewer, FileOversight},
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, FileFenceError, FileFenceReceipt, FileFenceResult,
    FileSupervisorFenceGrant, MAX_FENCE_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::ElapsedTick;
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError,
    TowerInstallError, TowerRequest, Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use support::{Directory, Port, Supervisor, observe, oversight, profile, ready};

const GENERATION: u64 = 41;
const OPERATION: u64 = 5009;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(GENERATION)).unwrap()
}

fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"), operator,
        if permit { vec![PurposeTag::Permit] } else { Vec::new() }, 0,
    )
}

fn create(root: &Directory) -> (Port, Supervisor, FileHumanReviewer, ActorProposal) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (port, supervisor) = host.into_actor_gateway();
    (port, supervisor, reviewer, proposal)
}

fn basis(supervisor: &Supervisor) -> FileFenceBasis {
    let host = supervisor.host().unwrap();
    let state = host.inspect();
    FileFenceBasis {
        scope: profile().delivery.scope,
        journal_revision: host.revision(),
        control_sequence: state.control.sequence,
        authority_epoch: state.control.ledger.epoch,
    }
}

fn command(expected: FileFenceBasis) -> TowerRequest {
    let mut request = TowerRequest::new("stop-original-domain", mount(), Verb::Fence);
    request.arguments = ReadArguments::Fence { operation: OPERATION, expected };
    request.idempotency_key = Some(OPERATION.to_string());
    request
}

fn granted(actor: Port, supervisor: &Supervisor) -> (PublicationTower, FileSupervisorFenceGrant) {
    let mut tower = PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default()).unwrap();
    let (port, grant) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    tower.set_fence(Some(port)).unwrap();
    (tower, grant)
}

fn native(response: &PublicationResponse) -> &FileFenceResult {
    let PublicationResult::Fence(linked) = &response.result else { panic!("expected native fence: {response:?}"); };
    linked.native()
}

fn receipt(response: &PublicationResponse) -> &FileFenceReceipt {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else { panic!("expected native acknowledgment: {response:?}"); };
    assert_eq!(value.after(), *basis);
    assert!(!response.envelope.retry_safe);
    assert_eq!(response.envelope.reason_code, None);
    value
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

#[test]
fn original_fence_cancels_unsent_work_but_keeps_dispatched_unknown_charged() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let _unsent_keys = ready(&mut supervisor, &reviewer, 701);
    observe(&mut supervisor);
    actor.submit(702, &proposal).unwrap();
    let sent_keys = ready(&mut supervisor, &reviewer, 702);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &sent_keys);
    }
    let unsent = support::attempt(&supervisor.host().unwrap(), 701);
    let sent = support::attempt(&supervisor.host().unwrap(), 702);
    let expected = basis(&supervisor);
    let initial = supervisor.host().unwrap().inspect();
    assert_eq!(initial.control.ledger.reserved, 16);
    assert_eq!(initial.control.ledger.charged, 16);
    let (mut tower, grant) = granted(actor, &supervisor);
    let request = command(expected);
    let encoded = encode_request(&request).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), request);
    let response = tower.exchange(&access(true, true), encoded.as_bytes());
    let result = receipt(&response);
    assert_eq!(result.before(), expected);
    assert_eq!(result.stop().cancelled(), &[unsent]);
    assert_eq!(result.stop().refunded_units(), 16);
    assert_eq!(result.progress().unresolved, vec![sent]);
    assert_eq!(result.progress().charged_units, 16);
    assert_eq!(result.progress().reserved_units, 0);
    assert!(!result.progress().endpoint_fenced);
    assert!(!result.progress().drained());
    let stopped = supervisor.host().unwrap().inspect();
    assert_eq!(stopped.executions, 0);
    assert_eq!(stopped.control.ledger.charged, 16);
    assert_eq!(stopped.control.ledger.elapsed, initial.control.ledger.elapsed);
    assert_eq!(grant.remaining_operations(), 0);
    let parsed = document(&response);
    assert_eq!(field(&parsed, "envelope").as_object().unwrap().len(), ENVELOPE_FIELDS.len());
    let body = field(field(&parsed, "result"), "value");
    assert_eq!(field(body, "scope").as_str(), Some("original_domain_admission"));
    assert_eq!(field(body, "drain_attempted").as_bool(), Some(false));
    let value = field(field(body, "knowledge"), "value");
    for name in ["actor_halted", "fleet_fenced"] {
        assert_eq!(field(field(value, name), "state").as_str(), Some("unknown"));
    }

    // Subsequent ORIGINAL endpoint work is a separate transition. An exact
    // fence retry still returns its historical before/after and charges.
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.progress_stop(revision, ElapsedTick(2)).unwrap();
    }
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 0);
    let bytes = image(&root);
    let repeated = tower.exchange(&access(true, true), encoded.as_bytes());
    assert_eq!(native(&repeated), native(&response));
    assert_eq!(image(&root), bytes);
    assert_eq!(receipt(&repeated).progress().charged_units, 16);
    assert_eq!(grant.remaining_operations(), 0);
}

#[test]
fn caller_purpose_registration_scope_and_full_predecessor_gate_native_mutation() {
    let root = Directory::new();
    let (actor, supervisor, _, _) = create(&root);
    let (mut tower, grant) = granted(actor, &supervisor);
    let request = command(basis(&supervisor));
    let bytes = image(&root);
    assert_eq!(tower.dispatch(&access(false, true), request.clone()).envelope.reason_code, Some(ReasonCode::DeniedByPolicy));
    assert_eq!(tower.dispatch(&access(true, false), request.clone()).envelope.reason_code, Some(ReasonCode::PurposeMismatch));
    let mut changed = request.clone();
    changed.address.generation = None;
    assert_eq!(tower.dispatch(&access(true, true), changed).envelope.reason_code, Some(ReasonCode::StaleGeneration));
    let mut changed = request.clone();
    changed.address.generation = Some(GENERATION + 1);
    assert_eq!(tower.dispatch(&access(true, true), changed).envelope.reason_code, Some(ReasonCode::StaleGeneration));
    let mut changed = request.clone();
    changed.idempotency_key = Some("5010".to_owned());
    assert_eq!(tower.dispatch(&access(true, true), changed).envelope.reason_code, Some(ReasonCode::IntegrityFailure));
    let mut changed = request.clone();
    let ReadArguments::Fence { expected, .. } = &mut changed.arguments else { panic!("fence"); };
    expected.scope.authority += 1;
    assert_eq!(tower.dispatch(&access(true, true), changed).envelope.reason_code, Some(ReasonCode::IntegrityFailure));
    for coordinate in 0..3 {
        let mut changed = request.clone();
        let ReadArguments::Fence { expected, .. } = &mut changed.arguments else { panic!("fence"); };
        match coordinate {
            0 => expected.journal_revision += 1,
            1 => expected.control_sequence += 1,
            _ => expected.authority_epoch += 1,
        }
        let refused = tower.dispatch(&access(true, true), changed);
        assert_eq!(native(&refused), &Err(FileFenceError::StalePredecessor));
    }
    assert_eq!(image(&root), bytes);
    assert_eq!(grant.remaining_operations(), 1);
    receipt(&tower.dispatch(&access(true, true), request));
}

#[test]
fn equal_numeric_scope_does_not_replace_the_original_grant_and_custody_is_revocable() {
    let root = Directory::new();
    let (actor, supervisor, _, _) = create(&root);
    let (mut tower, grant) = granted(actor, &supervisor);
    let request = command(basis(&supervisor));
    let foreign_root = Directory::new();
    let (_, foreign, _, _) = create(&foreign_root);
    let (foreign_port, _foreign_grant) = foreign.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    assert_eq!(tower.set_fence(Some(foreign_port)), Err(TowerInstallError::SnapshotMismatch));
    assert_eq!(tower.dispatch(&access(true, true), request.clone()).envelope.reason_code, Some(ReasonCode::CapabilityUnsupported));
    assert_eq!(grant.remaining_operations(), 1);
    let (port, custodian) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    tower.set_fence(Some(port)).unwrap();
    custodian.revoke();
    let result = tower.dispatch(&access(true, true), request.clone());
    assert!(matches!(native(&result), Ok(ActorKnowledge::Withheld { authority_required: "supervisor_fence" })));
    let (port, retained) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    tower.set_fence(Some(port)).unwrap();
    drop(supervisor);
    let result = tower.dispatch(&access(true, true), request);
    assert!(matches!(native(&result), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
    assert_eq!(retained.remaining_operations(), 1);
}

#[test]
fn legacy_reconciliation_mount_cannot_inherit_fencing_and_control_mount_cannot_submit() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let mut legacy = PublicationTower::new_reconciliation_operator(mount(), actor.clone(), PublicationLimits::default()).unwrap();
    let (port, custodian) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    legacy.set_fence(Some(port)).unwrap();
    let request = command(basis(&supervisor));
    assert_eq!(legacy.dispatch(&access(true, true), request).envelope.reason_code, Some(ReasonCode::CapabilityUnsupported));
    assert_eq!(custodian.remaining_operations(), 1);
    let (mut control, _grant) = granted(actor.clone(), &supervisor);
    observe(&mut supervisor);
    let mut proposed = TowerRequest::new("no-control-intake", mount(), Verb::Propose);
    proposed.arguments = ReadArguments::Propose { request: 701, proposal: proposal.clone() };
    proposed.idempotency_key = Some("701".to_owned());
    let bytes = image(&root);
    assert_eq!(control.dispatch(&access(true, true), proposed).envelope.reason_code, Some(ReasonCode::CapabilityUnsupported));
    assert_eq!(image(&root), bytes);
    // The ORIGINAL actor still owns its separate valid intake and snapshot.
    actor.submit(701, &proposal).unwrap();
    assert!(supervisor.host().unwrap().request_status(701).is_ok());
}

#[test]
fn sealed_fence_response_cannot_be_relabelled_after_a_real_durable_stop() {
    let root = Directory::new();
    let (actor, supervisor, _, _) = create(&root);
    let (mut tower, _grant) = granted(actor, &supervisor);
    let response = tower.dispatch(&access(true, true), command(basis(&supervisor)));
    receipt(&response);
    response.to_json().unwrap();
    let mut changed = response.clone();
    changed.envelope.address = Address::new("1", Noun::Run, "4", Some(GENERATION)).unwrap();
    assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    let mut changed = response.clone();
    changed.envelope.operation_id.push_str("-other");
    assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    let mut changed = response;
    changed.envelope.idempotency_key = Some("5010".to_owned());
    assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
}

#[test]
fn malformed_wire_never_spends_the_original_fence_or_clears_its_snapshot() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let (mut tower, grant) = granted(actor.clone(), &supervisor);
    observe(&mut supervisor);
    let request = command(basis(&supervisor));
    let encoded = encode_request(&request).unwrap();
    let malformed = encoded.replace("\"authority_epoch\":\"0\"", "\"authority_epoch\":\"0\",\"grant\":true");
    assert_ne!(malformed, encoded);
    let bytes = image(&root);
    let response = tower.exchange(&access(true, true), malformed.as_bytes());
    assert_eq!(response.envelope.reason_code, Some(ReasonCode::IntegrityFailure));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&root), bytes);
    actor.submit(701, &proposal).unwrap();
    assert!(supervisor.host().unwrap().request_status(701).is_ok());
}
