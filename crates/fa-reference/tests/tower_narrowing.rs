//! Live target restriction reaches the original gate without replacing policy.
//! Synthetic helper verdicts exercise the mechanism, not model-safety claims.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;
#[path = "tower_narrowing/learned.rs"]
mod learned;

use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::requests::actor::{
        FileNarrowBlockReason, FileNarrowError, FileNarrowPlan,
        FileNarrowPlanResult, FileNarrowReceipt, FileNarrowResult,
        FileSupervisorNarrowGrant, MAX_NARROW_REPLAY_EVENTS,
    },
};
use fa_reference::action::consequence::oversight::actor::{
    Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::{ActionState, ElapsedTick, Purpose, ResolvedTarget};
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
    ENVELOPE_FIELDS, MAX_NARROWING_TARGETS, ReadArguments, ReasonCode,
    TowerAccess, TowerBuilder, TowerDecodingError, TowerEncodingError,
    TowerInstallError, TowerLimits, TowerRequest, Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::{PurposeTag, RootCx};
use support::{Directory, Port, Supervisor, create, observe, oversight, ready, snapshot};

const REGISTRATION: u64 = 41;
const OPERATION: u64 = 5009;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap()
}

fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"), operator,
        if permit { vec![PurposeTag::Permit, PurposeTag::Experiment] } else { Vec::new() }, 0,
    )
}

fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default()).unwrap()
}

fn granted(actor: Port, supervisor: &Supervisor) -> (PublicationTower, FileSupervisorNarrowGrant) {
    let (port, grant) = supervisor.narrow_port(32, 8, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let mut facade = tower(actor);
    facade.set_narrow(Some(port)).unwrap();
    (facade, grant)
}

fn query(operation: u64, retained_targets: Vec<ResolvedTarget>) -> TowerRequest {
    let mut request = TowerRequest::new("inspect-target-restriction", mount(), Verb::Next);
    request.arguments = ReadArguments::NarrowingNext { operation, retained_targets };
    request
}

fn next(facade: &mut PublicationTower, operation: u64, targets: Vec<ResolvedTarget>) -> PublicationResponse {
    facade.exchange(&access(true, true), encode_request(&query(operation, targets)).unwrap().as_bytes())
}

fn native_next(response: &PublicationResponse) -> &FileNarrowPlanResult {
    let PublicationResult::NarrowingNext(linked) = &response.result else {
        panic!("expected original narrowing plan: {response:?}");
    };
    linked.native()
}

fn plan(response: &PublicationResponse) -> &FileNarrowPlan {
    let Ok(ActorKnowledge::Known { value, basis }) = native_next(response) else {
        panic!("expected acknowledged plan: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    value
}

fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::NarrowingNext(linked) = &response.result else { panic!("narrowing plan"); };
    linked.action().unwrap().expect("original permitted restriction")
}

fn native(response: &PublicationResponse) -> &FileNarrowResult {
    let PublicationResult::Narrowing(linked) = &response.result else {
        panic!("expected original narrowing result: {response:?}");
    };
    linked.native()
}

fn receipt(response: &PublicationResponse) -> &FileNarrowReceipt {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected acknowledged restriction: {response:?}");
    };
    assert_eq!(value.after(), *basis);
    assert_eq!(response.envelope.reason_code, None);
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

#[test]
fn narrowing_refunds_only_outside_reservations_and_retained_work_really_publishes() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let outside = ready(&mut supervisor, &reviewer, 701);
    observe(&mut supervisor);
    let unknown_ticket = actor.submit(702, &proposal).unwrap();
    let unknown = ready(&mut supervisor, &reviewer, 702);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &unknown);
        let revision = host.revision();
        host.reconcile(revision, unknown.automatic.attempt()).unwrap();
    }
    // A real independently approved publication advances the endpoint version.
    // The old reservation and uncertain charge remain with this same owner.
    observe(&mut supervisor);
    actor.submit(703, &proposal).unwrap();
    let advanced = ready(&mut supervisor, &reviewer, 703);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &advanced);
        let revision = host.revision();
        assert!(matches!(host.publish(revision, advanced.automatic.attempt()).unwrap(), EndpointOutcome::Executed { .. }));
        let revision = host.revision();
        host.reconcile(revision, advanced.automatic.attempt()).unwrap();
    }
    let retained_target = supervisor.host().unwrap().inspect().target;
    assert_eq!(retained_target.expected_version, proposal.target.expected_version + 1);
    let mut retained_proposal = proposal.clone();
    retained_proposal.target = retained_target;
    retained_proposal.payload = b"retained publication".to_vec();
    retained_proposal.units = 24;
    observe(&mut supervisor);
    actor.submit(704, &retained_proposal).unwrap();
    let retained = ready(&mut supervisor, &reviewer, 704);
    let (mut facade, grant) = granted(actor.clone(), &supervisor);
    let before = supervisor.host().unwrap().inspect();
    let original = image(&root);
    let proposed = next(&mut facade, OPERATION, vec![retained_target]);
    assert!(plan(&proposed).can_narrow());
    assert_eq!(plan(&proposed).current_ceiling(), None);
    assert_eq!(plan(&proposed).effective_targets(), &[retained_target]);
    assert_eq!(image(&root), original);
    assert_eq!(grant.remaining_operations(), 8);
    let command = action(&proposed);
    assert_eq!(command.verb, Verb::Narrow);
    let encoded = encode_request(&command).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), command);
    let parsed = document(&proposed);
    let value = field(field(field(&parsed, "result"), "value"), "knowledge");
    let value = field(value, "value");
    assert_eq!(field(value, "action"), &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap());
    for name in ["cost_estimate", "expected_information_gain", "ranking", "current_eligibility"] {
        assert_eq!(field(field(value, name), "state").as_str(), Some("unknown"));
    }

    let completed = facade.exchange(&access(true, true), encoded.as_bytes());
    let result = receipt(&completed);
    assert_eq!(result.before(), plan(&proposed).basis());
    assert_eq!(result.previous_ceiling(), None);
    assert_eq!(result.effective_targets(), &[retained_target]);
    assert_eq!(result.cancelled(), &[outside.automatic.attempt()]);
    assert_eq!(result.refunded_units(), 16);
    assert_eq!(result.after().journal_revision, result.before().journal_revision + 1);
    assert_eq!(result.after().control_sequence, result.before().control_sequence + 1);
    assert_eq!(result.after().authority_epoch, result.before().authority_epoch);
    assert_eq!(result.after().recorded_at, result.before().recorded_at);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.control.ledger.charged, 32);
    assert_eq!(after.control.ledger.reserved, 24);
    assert_eq!(after.control.ledger.available, before.control.ledger.available + 16);
    assert_eq!(after.control.ledger.stages[&unknown.automatic.attempt()], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&outside.automatic.attempt()], ActionState::Cancelled);
    assert_eq!(after.control.ledger.stages[&retained.automatic.attempt()], ActionState::Authorized);
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert!(after.stop.is_none());
    let parsed = document(&completed);
    assert_eq!(field(&parsed, "envelope").as_object().unwrap().len(), ENVELOPE_FIELDS.len());
    let body = field(field(&parsed, "result"), "value");
    assert_eq!(field(body, "operation").as_str(), Some("narrow"));
    assert_eq!(field(body, "publication_attempted").as_bool(), Some(false));

    // The original reservation and automatic approval survive inside the
    // ceiling. The changed control sequence requires a NEW independent human key.
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.dispatch(revision, &retained.automatic, &retained.human,
            &retained.action, &retained.inputs, snapshot()).is_err());
        assert_eq!(host.revision(), revision);
        let human_request = host.request_human_approval(
            revision, 9001, retained.automatic.attempt(), &retained.inputs, ElapsedTick(31),
        ).unwrap();
        let revision = host.revision();
        let fresh_human = reviewer.approve(&mut host, revision, &human_request).unwrap();
        let revision = host.revision();
        host.dispatch(revision, &retained.automatic, &fresh_human,
            &retained.action, &retained.inputs, snapshot()).unwrap();
        let revision = host.revision();
        assert!(matches!(host.publish(revision, retained.automatic.attempt()).unwrap(), EndpointOutcome::Executed { .. }));
        let revision = host.revision();
        host.reconcile(revision, retained.automatic.attempt()).unwrap();
    }
    let published = supervisor.host().unwrap().inspect();
    assert_eq!(published.payload, retained_proposal.payload);
    assert_eq!(published.executions, 2);
    assert_eq!(published.control.ledger.charged, 56);
    assert_eq!(published.control.ledger.reserved, 0);
    assert!(matches!(actor.poll(&unknown_ticket), ActorKnowledge::Unknown { reason: ActorUnknown::OutcomeUnknown }));
    // Retry remains the original acknowledgment after unrelated journal work.
    let saved = image(&root);
    observe(&mut supervisor);
    let repeated = facade.exchange(&access(true, true), encoded.as_bytes());
    assert_eq!(native(&repeated), native(&completed));
    assert_eq!(image(&root), saved);
    assert_eq!(grant.remaining_operations(), 7);
    let blocked = next(&mut facade, OPERATION, vec![retained_target]);
    assert_eq!(plan(&blocked).block_reason(), Some(FileNarrowBlockReason::OperationAlreadyUsed));
    assert!(!plan(&blocked).can_narrow());
}

#[test]
fn wider_restrictions_and_empty_sets_never_restore_excluded_targets() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let (mut facade, grant) = granted(actor, &supervisor);
    let first = proposal.target;
    let mut second = first;
    second.expected_version += 1;
    let proposed = next(&mut facade, OPERATION, vec![first]);
    let completed = facade.dispatch(&access(true, true), action(&proposed));
    assert_eq!(receipt(&completed).effective_targets(), &[first]);
    let wider = next(&mut facade, OPERATION + 1, vec![first, second]);
    assert_eq!(plan(&wider).current_ceiling(), Some([first].as_slice()));
    assert_eq!(plan(&wider).retained_targets(), &[first, second]);
    assert_eq!(plan(&wider).effective_targets(), &[first]);
    let widened = facade.dispatch(&access(true, true), action(&wider));
    assert_eq!(receipt(&widened).previous_ceiling(), Some([first].as_slice()));
    assert_eq!(receipt(&widened).effective_targets(), &[first]);
    assert_eq!(receipt(&widened).refunded_units(), 0);
    let empty = next(&mut facade, OPERATION + 2, Vec::new());
    let narrowed = facade.dispatch(&access(true, true), action(&empty));
    assert!(receipt(&narrowed).effective_targets().is_empty());
    let attempted_restore = next(&mut facade, OPERATION + 3, vec![first, second]);
    assert_eq!(plan(&attempted_restore).current_ceiling(), Some([].as_slice()));
    assert!(plan(&attempted_restore).effective_targets().is_empty());
    let restored = facade.dispatch(&access(true, true), action(&attempted_restore));
    assert_eq!(receipt(&restored).previous_ceiling(), Some([].as_slice()));
    assert!(receipt(&restored).effective_targets().is_empty());
    let original = image(&root);
    {
        let mut host = supervisor.host_mut().unwrap();
        let spec = oversight::spec(&host, b"not restored");
        let revision = host.revision();
        assert!(host.propose(revision, 90, spec, snapshot()).is_err());
    }
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.epoch, 0);
    assert_eq!(grant.remaining_operations(), 4);
}

#[test]
fn caller_scope_authority_and_every_original_predecessor_coordinate_are_required() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let (mut facade, grant) = granted(actor.clone(), &supervisor);
    let proposed = next(&mut facade, OPERATION, vec![proposal.target]);
    let command = action(&proposed);
    observe(&mut supervisor);
    let saved = image(&root);
    refused(&facade.dispatch(&access(false, true), command.clone()), ReasonCode::DeniedByPolicy);
    refused(&facade.dispatch(&access(true, false), command.clone()), ReasonCode::PurposeMismatch);
    let foreign = TowerAccess::new(CallerScope::full_tenant_access("2"), true, vec![PurposeTag::Permit], 0);
    refused(&facade.dispatch(&foreign, command.clone()), ReasonCode::DeniedByPolicy);
    for generation in [None, Some(REGISTRATION + 1)] {
        let mut changed = command.clone();
        changed.address.generation = generation;
        refused(&facade.dispatch(&access(true, true), changed), ReasonCode::StaleGeneration);
    }
    for coordinate in 0..6 {
        let mut changed = command.clone();
        let ReadArguments::Narrow { expected, .. } = &mut changed.arguments else { panic!("narrow"); };
        match coordinate {
            0 => expected.scope.tenant += 1,
            1 => expected.scope.principal += 1,
            2 => expected.scope.run += 1,
            3 => expected.scope.branch += 1,
            4 => expected.scope.authority += 1,
            _ => expected.scope.purpose = Purpose::Experiment,
        }
        refused(&facade.dispatch(&access(true, true), changed), ReasonCode::IntegrityFailure);
    }
    for coordinate in 0..5 {
        let mut changed = command.clone();
        let ReadArguments::Narrow { expected, .. } = &mut changed.arguments else { panic!("narrow"); };
        match coordinate {
            0 => expected.journal_revision += 1,
            1 => expected.control_sequence += 1,
            2 => expected.authority_epoch += 1,
            3 => expected.recorded_at = Some(ElapsedTick(2)),
            _ => expected.recorded_at = None,
        }
        assert_eq!(native(&facade.dispatch(&access(true, true), changed)), &Err(FileNarrowError::StalePredecessor));
    }
    let mut changed = command.clone();
    changed.idempotency_key = Some("5010".into());
    refused(&facade.dispatch(&access(true, true), changed), ReasonCode::IntegrityFailure);
    assert_eq!(image(&root), saved);
    assert_eq!(grant.remaining_operations(), 8);
    assert_eq!(grant.remaining_queries(), 31);
    // Failed requests do not consume the separately supplied actor observation.
    actor.submit(701, &proposal).unwrap();
    let moved = facade.dispatch(&access(true, true), command);
    assert_eq!(native(&moved), &Err(FileNarrowError::StalePredecessor));
    let current = next(&mut facade, OPERATION, vec![proposal.target]);
    receipt(&facade.dispatch(&access(true, true), action(&current)));
}

#[test]
fn canonical_target_and_closed_wire_validation_precede_native_reads_and_mutations() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let (mut facade, grant) = granted(actor.clone(), &supervisor);
    let first = proposal.target;
    let mut second = first;
    second.expected_version += 1;
    let proposed = next(&mut facade, OPERATION, vec![first]);
    let command = action(&proposed);
    let encoded = encode_request(&query(OPERATION, vec![first])).unwrap();
    let encoded_command = encode_request(&command).unwrap();
    let malformed = [
        encoded.replace("\"action\":\"narrow_authority\",", ""),
        encoded.replace("narrow_authority", "abandon_unknown"),
        encoded.replace("\"adapter\":\"10\"", "\"adapter\":10"),
        encoded.replace("\"object\":\"11\"", "\"object\":\"011\""),
        encoded.replace("\"generation\":\"1\"", "\"generation\":\"1\",\"permission\":true"),
        encoded_command.replace("\"recorded_at\":\"1\"", "\"recorded_at\":\"1\",\"current\":true"),
        encoded_command.replace("\"verb\":\"narrow\"", "\"verb\":\"resolve\""),
    ];
    observe(&mut supervisor);
    let saved = image(&root);
    for malformed in malformed {
        assert_ne!(malformed, encoded);
        assert_ne!(malformed, encoded_command);
        assert!(decode_request(malformed.as_bytes()).is_err());
        refused(&facade.exchange(&access(true, true), malformed.as_bytes()), ReasonCode::IntegrityFailure);
    }
    let mut zero = first;
    zero.object = 0;
    for targets in [vec![first, first], vec![second, first], vec![zero]] {
        let invalid = query(OPERATION, targets.clone());
        assert_eq!(encode_request(&invalid), Err(TowerDecodingError::MalformedRequest));
        refused(&facade.dispatch(&access(true, true), invalid), ReasonCode::IntegrityFailure);
        let mut invalid = command.clone();
        let ReadArguments::Narrow { retained_targets, .. } = &mut invalid.arguments else { panic!("narrow"); };
        *retained_targets = targets;
        assert!(encode_request(&invalid).is_err());
        refused(&facade.dispatch(&access(true, true), invalid), ReasonCode::IntegrityFailure);
    }
    let oversized = (1..=MAX_NARROWING_TARGETS + 1).map(|version| {
        let mut target = first;
        target.expected_version = version as u64;
        target
    }).collect();
    let oversized = query(OPERATION, oversized);
    assert_eq!(encode_request(&oversized), Err(TowerDecodingError::Capacity));
    refused(&facade.dispatch(&access(true, true), oversized), ReasonCode::IntegrityFailure);
    assert_eq!(grant.remaining_queries(), 31);
    assert_eq!(grant.remaining_operations(), 8);
    assert_eq!(image(&root), saved);
    actor.submit(701, &proposal).unwrap();
}

#[test]
fn only_original_control_custody_can_narrow_and_disclosure_does_not_grant_actor_intake() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let (mut control, grant) = granted(actor.clone(), &supervisor);
    let proposed = next(&mut control, OPERATION, vec![proposal.target]);
    let command = action(&proposed);
    for mut other in [
        PublicationTower::new(mount(), actor.clone(), PublicationLimits::default()).unwrap(),
        PublicationTower::new_reconciliation_operator(mount(), actor.clone(), PublicationLimits::default()).unwrap(),
    ] {
        let (port, retained) = supervisor.narrow_port(2, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
        other.set_narrow(Some(port)).unwrap();
        refused(&other.dispatch(&access(true, true), query(OPERATION, vec![proposal.target])), ReasonCode::CapabilityUnsupported);
        refused(&other.dispatch(&access(true, true), command.clone()), ReasonCode::CapabilityUnsupported);
        assert_eq!((retained.remaining_queries(), retained.remaining_operations()), (2, 1));
    }
    let saved = image(&root);
    observe(&mut supervisor);
    let mut intake = TowerRequest::new("not-control-intake", mount(), Verb::Propose);
    intake.idempotency_key = Some("701".into());
    intake.arguments = ReadArguments::Propose { request: 701, proposal: proposal.clone() };
    refused(&control.dispatch(&access(true, true), intake), ReasonCode::CapabilityUnsupported);
    assert_eq!(image(&root), saved);
    actor.submit(701, &proposal).unwrap();
    let foreign_root = Directory::new();
    let (_, foreign, _, _) = create(&foreign_root);
    let (foreign_port, foreign_grant) = foreign.narrow_port(2, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    assert_eq!(control.set_narrow(Some(foreign_port)), Err(TowerInstallError::SnapshotMismatch));
    refused(&control.dispatch(&access(true, true), command.clone()), ReasonCode::CapabilityUnsupported);
    assert_eq!(grant.remaining_operations(), 8);
    assert_eq!(foreign_grant.remaining_operations(), 1);
    let (port, revoked) = supervisor.narrow_port(2, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    control.set_narrow(Some(port)).unwrap();
    revoked.revoke();
    let response = control.dispatch(&access(true, true), command.clone());
    assert!(matches!(native(&response), Ok(ActorKnowledge::Withheld { authority_required: "supervisor_narrow" })));
    let (port, retained) = supervisor.narrow_port(2, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    control.set_narrow(Some(port)).unwrap();
    {
        let _borrowed = supervisor.host_mut().unwrap();
        let response = control.dispatch(&access(true, true), command.clone());
        assert!(matches!(native(&response), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
    }
    assert_eq!(retained.remaining_operations(), 1);
    drop(supervisor);
    let response = control.dispatch(&access(true, true), command);
    assert!(matches!(native(&response), Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })));
    assert_eq!(retained.remaining_operations(), 1);
}

#[test]
fn sealed_plan_and_receipt_keep_run_operation_and_retry_identity() {
    let root = Directory::new();
    let (actor, supervisor, _, proposal) = create(&root);
    let (mut facade, grant) = granted(actor, &supervisor);
    let proposed = next(&mut facade, OPERATION, vec![proposal.target]);
    let command = action(&proposed);
    let completed = facade.dispatch(&access(true, true), command.clone());
    receipt(&completed);
    for response in [proposed, completed] {
        response.to_json().unwrap();
        let mut changed = response.clone();
        changed.envelope.address = Address::new("1", Noun::Run, "4", Some(REGISTRATION)).unwrap();
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.address.generation = Some(REGISTRATION + 1);
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.operation_id.push_str("-other");
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.idempotency_key = Some("5010".into());
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut tiny = response;
        tiny.envelope.limits.max_response_bytes = 1;
        assert_eq!(tiny.to_json(), Err(TowerEncodingError::OutputLimit));
    }
    let saved = image(&root);
    let mut changed = command;
    let ReadArguments::Narrow { retained_targets, .. } = &mut changed.arguments else { panic!("narrow"); };
    retained_targets.clear();
    assert_eq!(native(&facade.dispatch(&access(true, true), changed)), &Err(FileNarrowError::IdempotencyConflict));
    assert_eq!(image(&root), saved);
    assert_eq!(grant.remaining_operations(), 7);
}

#[test]
fn finite_plan_operation_and_replay_allowances_do_not_become_authority() {
    let root = Directory::new();
    let (actor, supervisor, _, proposal) = create(&root);
    let mut facade = tower(actor);
    let (port, grant) = supervisor.narrow_port(4, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    facade.set_narrow(Some(port)).unwrap();
    let first = next(&mut facade, OPERATION, vec![proposal.target]);
    let repeated_plan = next(&mut facade, OPERATION, vec![proposal.target]);
    assert_eq!(plan(&first), plan(&repeated_plan));
    assert_eq!((grant.remaining_queries(), grant.remaining_operations()), (2, 1));
    let command = action(&first);
    let completed = facade.dispatch(&access(true, true), command.clone());
    receipt(&completed);
    let saved = image(&root);
    for operation in [OPERATION + 1, OPERATION + 2] {
        let blocked = next(&mut facade, operation, vec![proposal.target]);
        assert_eq!(plan(&blocked).block_reason(), Some(FileNarrowBlockReason::OperationLimit));
        assert!(plan(&blocked).request().is_none());
    }
    let exhausted = next(&mut facade, OPERATION + 3, vec![proposal.target]);
    assert_eq!(native_next(&exhausted), &Err(FileNarrowError::Capacity));
    assert_eq!(exhausted.envelope.reason_code, Some(ReasonCode::BudgetExhausted));
    assert_eq!(native(&facade.dispatch(&access(true, true), command)), native(&completed));
    assert_eq!((grant.remaining_queries(), grant.remaining_operations()), (0, 0));
    assert_eq!(image(&root), saved);

    let other = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&other);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.narrow_port(2, 1, 1).unwrap();
    facade.set_narrow(Some(port)).unwrap();
    let blocked = next(&mut facade, OPERATION, vec![proposal.target]);
    assert_eq!(plan(&blocked).block_reason(), Some(FileNarrowBlockReason::ReplayLimit));
    let mut command = TowerRequest::new("bounded-replay-refusal", mount(), Verb::Narrow);
    command.idempotency_key = Some(OPERATION.to_string());
    command.arguments = ReadArguments::Narrow {
        operation: OPERATION, expected: plan(&blocked).basis(), retained_targets: vec![proposal.target],
    };
    let saved = image(&other);
    observe(&mut supervisor);
    let response = facade.dispatch(&access(true, true), command);
    assert_eq!(native(&response), &Err(FileNarrowError::Capacity));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&other), saved);
    actor.submit(701, &proposal).unwrap();
}

fn snapshot_basis() -> Basis {
    let proof = ScopedProof::Unsupported { scope_id: 1 };
    Basis::new(
        BasisIdentity { subject_id: 3, question_id: 1, profile_id: 1,
            semantic_generation: REGISTRATION, policy_epoch: 0, model_epoch: 0,
            tokenizer_epoch: 0, codec_epoch: 0, control_seq: 0 },
        ClaimClass::BoundedModel,
        ProofStates { authentic_origin: proof, complete_for_contract: proof,
            semantically_valid: proof, available_for_replay: proof },
    )
}

#[test]
fn imported_snapshots_and_experiment_branches_never_narrow_the_live_owner() {
    let root = Directory::new();
    let (actor, supervisor, _, proposal) = create(&root);
    let (mut facade, grant) = granted(actor, &supervisor);
    let proposed = next(&mut facade, OPERATION, vec![proposal.target]);
    let command = action(&proposed);
    let branch = RootCx::new_nucleus(1, 3, 0).mint_experiment(9).unwrap();
    for journal in [DomainEventJournal::new_production("1", "3"), DomainEventJournal::new_branch(&branch, "1", "3", 9)] {
        let mut imported = TowerBuilder::new(journal, snapshot_basis(), TowerLimits::default(), 2).unwrap().build();
        for request in [query(OPERATION, vec![proposal.target]), command.clone()] {
            let response = imported.dispatch(&access(true, true), request);
            assert_eq!(response.envelope.reason_code, Some(ReasonCode::CapabilityUnsupported));
            assert_eq!(imported.remaining_read_units(), 2);
            assert_eq!(imported.journal_control_seq(), 0);
        }
    }
    assert_eq!(grant.remaining_operations(), 8);
    let response = facade.dispatch(&access(true, true), command);
    assert_eq!(receipt(&response).effective_targets(), &[proposal.target]);
}
