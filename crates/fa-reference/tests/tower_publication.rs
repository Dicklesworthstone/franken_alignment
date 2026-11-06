//! The Tower's publication profile uses the original durable actor gateway.
//! These tests retain the real supervisor, review inputs and two publication
//! keys separately; a Tower response is only the actor's restricted observation.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorOutcome, ActorProposal, BasisSource, Knowledge as ActorKnowledge,
    UnknownReason as ActorUnknownReason,
};
use fa_reference::action::consequence::oversight::actor_wire::{WireError, WireResponse};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolverPlane};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{AuthorityKind, Knowledge};
use fa_reference::strict_json;
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, ResponseStatus, TowerAccess, TowerInstallError,
    TowerRequest, Verb,
};
use fa_reference::typestate_cx::PurposeTag;
use std::collections::BTreeSet;
use support::{Directory, Port, Supervisor, create, observe, oversight, profile, ready, snapshot};

const REGISTRATION_GENERATION: u64 = 41;

fn mount() -> Address {
    let scope = profile().delivery.scope;
    Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(REGISTRATION_GENERATION),
    )
    .unwrap()
}

fn tower(port: Port) -> PublicationTower {
    PublicationTower::new(mount(), port, PublicationLimits::default()).unwrap()
}

fn actor_scope() -> CallerScope {
    CallerScope {
        caller_tenant: profile().delivery.scope.tenant.to_string(),
        allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
        granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
    }
}

// The actor has neither operator status nor visibility into other store planes.
fn access() -> TowerAccess {
    TowerAccess::new(actor_scope(), false, vec![PurposeTag::Permit], 0)
}

fn read_access() -> TowerAccess {
    TowerAccess::new(actor_scope(), false, Vec::new(), 0)
}

fn propose(tower: &PublicationTower, request: u64, proposal: &ActorProposal) -> TowerRequest {
    let mut call = TowerRequest::new(
        format!("propose-{request}"),
        tower.mount_address().clone(),
        Verb::Propose,
    );
    call.arguments = ReadArguments::Propose {
        request,
        proposal: proposal.clone(),
    };
    call.idempotency_key = Some(request.to_string());
    call
}

fn poll(tower: &mut PublicationTower, request: u64) -> PublicationResponse {
    let call = TowerRequest::new(
        format!("poll-{request}"),
        tower.request_address(request).unwrap(),
        Verb::Get,
    );
    tower.dispatch(&read_access(), call)
}

fn native(response: &PublicationResponse) -> &WireResponse {
    match &response.result {
        PublicationResult::Request(value) => value,
        other => panic!("expected original actor observation, got {other:?}"),
    }
}

fn pending(response: &PublicationResponse, request: u64) {
    assert_eq!(response.envelope.status, ResponseStatus::Ok);
    assert_eq!(native(response).request, Some(request));
    assert_eq!(
        native(response).result,
        Ok(ActorKnowledge::Pending { request })
    );
    no_global_claims(response);
}

fn known(response: &PublicationResponse, request: u64, expected: ActorOutcome) {
    assert_eq!(native(response).request, Some(request));
    match &native(response).result {
        Ok(ActorKnowledge::Known { value, basis }) => {
            assert_eq!(*value, expected);
            assert_eq!(basis.request, request);
        }
        other => panic!("expected native terminal outcome, got {other:?}"),
    }
    no_global_claims(response);
}

fn unavailable(response: &PublicationResponse) {
    assert_eq!(
        native(response).result,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        })
    );
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::LowerLayerUnverified)
    );
}

fn refused(response: &PublicationResponse) {
    assert_eq!(response.envelope.status, ResponseStatus::Error);
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    no_global_claims(response);
}

fn no_global_claims(response: &PublicationResponse) {
    fn unknown_or_withheld<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    let envelope = &response.envelope;
    unknown_or_withheld(&envelope.generation);
    unknown_or_withheld(&envelope.control_seq);
    unknown_or_withheld(&envelope.source_frontiers);
    unknown_or_withheld(&envelope.budgets_remaining);
    unknown_or_withheld(&envelope.realized_cost);
    unknown_or_withheld(&envelope.affordances);
}

fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn publish(
    supervisor: &mut Supervisor,
    reviewer: &FileHumanReviewer,
    request: u64,
) -> oversight::Keys {
    let keys = ready(supervisor, reviewer, request);
    let mut host = supervisor.host_mut().unwrap();
    oversight::dispatch(&mut host, &keys);
    let revision = host.revision();
    host.publish(revision, keys.automatic.attempt()).unwrap();
    let revision = host.revision();
    assert!(matches!(
        host.reconcile(revision, keys.automatic.attempt()).unwrap(),
        Reconciliation::Resolved(_)
    ));
    keys
}

#[test]
fn real_review_and_both_keys_publish_while_every_poll_observes_current_native_state() {
    let root = Directory::new();
    let (port, mut supervisor, reviewer, proposal) = create(&root);
    let original_port = port.clone();
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let call = propose(&adapter, 70, &proposal);
    let before = canonical(&root);
    pending(&adapter.dispatch(&access(), call.clone()), 70);
    assert_ne!(canonical(&root), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    let native_attempt = support::attempt(&supervisor.host().unwrap(), 70);
    assert_ne!(native_attempt, 70);
    assert_ne!(
        adapter.request_address(70).unwrap().id,
        native_attempt.to_string()
    );

    // The original native port is an independent observation oracle. Exact
    // reacquisition neither needs another snapshot nor creates another record.
    let ticket = original_port.submit(70, &proposal).unwrap();
    assert_eq!(
        native(&poll(&mut adapter, 70)).result,
        Ok(original_port.poll(&ticket))
    );
    let keys = ready(&mut supervisor, &reviewer, 70);
    let before_poll = canonical(&root);
    pending(&poll(&mut adapter, 70), 70);
    assert_eq!(canonical(&root), before_poll);

    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
    }
    let sent = poll(&mut adapter, 70);
    assert_eq!(native(&sent).result, Ok(original_port.poll(&ticket)));
    assert_eq!(
        native(&sent).result,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::OutcomeUnknown,
        })
    );
    assert_eq!(sent.envelope.reason_code, Some(ReasonCode::OutcomeUnknown));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.publish(revision, native_attempt).unwrap();
        assert_eq!(host.inspect().payload, proposal.payload);
        assert_eq!(host.inspect().executions, 1);
    }
    // Publication without accepted reconciliation is still an unknown outcome.
    assert_eq!(native(&poll(&mut adapter, 70)).result, native(&sent).result);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(matches!(
            host.reconcile(revision, native_attempt).unwrap(),
            Reconciliation::Resolved(_)
        ));
    }
    let completed = poll(&mut adapter, 70);
    known(&completed, 70, ActorOutcome::Executed);
    assert_eq!(native(&completed).result, Ok(original_port.poll(&ticket)));
    let ActorKnowledge::Known { basis, .. } = native(&completed).result.as_ref().unwrap() else {
        panic!("terminal actor result");
    };
    assert_eq!(basis.source, BasisSource::ControlLedger);
    assert_eq!(
        basis.generation,
        supervisor
            .host()
            .unwrap()
            .request_status(70)
            .unwrap()
            .generation
    );
    assert_ne!(basis.generation, supervisor.host().unwrap().revision());
    let final_image = canonical(&root);
    known(
        &adapter.dispatch(&access(), call),
        70,
        ActorOutcome::Executed,
    );
    assert_eq!(canonical(&root), final_image);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
}

#[test]
fn exact_durable_retry_after_reopen_recovers_only_the_current_sessions_ticket() {
    let root = Directory::new();
    let (port, mut supervisor, reviewer, proposal) = create(&root);
    let mut original = tower(port);
    observe(&mut supervisor);
    let call = propose(&original, 77, &proposal);
    pending(&original.dispatch(&access(), call.clone()), 77);
    publish(&mut supervisor, &reviewer, 77);
    known(&poll(&mut original, 77), 77, ActorOutcome::Executed);

    // Retaining the actor adapter cannot retain the authority owner's file lock.
    drop(supervisor);
    unavailable(&poll(&mut original, 77));
    let (host, _fresh_reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let (fresh_port, supervisor) = host.into_actor_gateway();
    let mut recovered = tower(fresh_port);
    let image = canonical(&root);
    let revision = supervisor.host().unwrap().revision();
    assert_eq!(
        native(&poll(&mut recovered, 77)).result,
        Ok(ActorKnowledge::Withheld {
            authority_required: "own_request",
        })
    );

    let mut different = proposal.clone();
    different.payload = b"different".to_vec();
    let wrong = propose(&recovered, 77, &different);
    assert_eq!(
        native(&recovered.dispatch(&access(), wrong)).result,
        Err(WireError::IdempotencyConflict)
    );
    assert!(matches!(
        native(&poll(&mut recovered, 77)).result,
        Ok(ActorKnowledge::Withheld { .. })
    ));

    // The original epoch/deadline are exact request data on retry. Recovery
    // does not require a new admission snapshot or rewrite that old proposal.
    let retry = recovered.dispatch(&access(), call);
    known(&retry, 77, ActorOutcome::Executed);
    assert!(retry.envelope.retry_safe);
    known(&poll(&mut recovered, 77), 77, ActorOutcome::Executed);
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(canonical(&root), image);
    unavailable(&poll(&mut original, 77));
}

#[test]
fn recovering_an_unexecuted_request_cannot_restore_the_original_publication_keys() {
    let root = Directory::new();
    let (port, mut supervisor, reviewer, proposal) = create(&root);
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let call = propose(&adapter, 1, &proposal);
    pending(&adapter.dispatch(&access(), call.clone()), 1);
    let keys = ready(&mut supervisor, &reviewer, 1);
    drop(supervisor);

    let (host, _new_reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    let (port, mut supervisor) = host.into_actor_gateway();
    let mut recovered = tower(port);
    let image = canonical(&root);
    known(
        &recovered.dispatch(&access(), call),
        1,
        ActorOutcome::CancelledBeforeDispatch,
    );
    assert_eq!(canonical(&root), image);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(
            reviewer
                .approve(&mut host, revision, &keys.request)
                .is_err()
        );
        assert!(
            host.dispatch(
                revision,
                &keys.automatic,
                &keys.human,
                &keys.action,
                &keys.inputs,
                snapshot(),
            )
            .is_err()
        );
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().payload, b"initial");
    }
}

#[test]
fn request_admission_rejects_wrong_keys_generations_and_shapes_before_consuming_snapshot() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = create(&root);
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let valid = propose(&adapter, 11, &proposal);
    let image = canonical(&root);
    let revision = supervisor.host().unwrap().revision();
    let mut invalid = Vec::new();
    for key in [
        None,
        Some(""),
        Some("011"),
        Some("12"),
        Some("+11"),
        Some(" 11"),
    ] {
        let mut call = valid.clone();
        call.idempotency_key = key.map(str::to_owned);
        invalid.push(call);
    }
    for generation in [
        None,
        Some(REGISTRATION_GENERATION - 1),
        Some(REGISTRATION_GENERATION + 1),
    ] {
        let mut call = valid.clone();
        call.address.generation = generation;
        invalid.push(call);
    }
    for id in ["wrong-run", "03"] {
        let mut call = valid.clone();
        call.address.id = id.to_owned();
        invalid.push(call);
    }
    for tenant in ["2", "bad/tenant"] {
        let mut call = valid.clone();
        call.address.tenant = tenant.to_owned();
        invalid.push(call);
    }
    for operation in [String::new(), "x".repeat(129)] {
        let mut call = valid.clone();
        call.operation_id = operation;
        invalid.push(call);
    }
    let mut wrong_noun = valid.clone();
    wrong_noun.address.kind = Noun::Attempt;
    invalid.push(wrong_noun);
    let mut missing_arguments = valid.clone();
    missing_arguments.arguments = ReadArguments::None;
    invalid.push(missing_arguments);
    let mut wrong_arguments = valid.clone();
    wrong_arguments.arguments = ReadArguments::Tail {
        since_seq: 0,
        max_events: 1,
    };
    invalid.push(wrong_arguments);
    invalid.push(propose(&adapter, 0, &proposal));
    for call in invalid {
        refused(&adapter.dispatch(&access(), call));
    }
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    assert_eq!(canonical(&root), image);

    // The same single supplied observation remains available after all of the
    // wrapper refusals, proving that admission preceded the native owner call.
    pending(&adapter.dispatch(&access(), valid), 11);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    let next = propose(&adapter, 12, &proposal);
    assert_eq!(
        native(&adapter.dispatch(&access(), next)).result,
        Err(WireError::Unavailable)
    );
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
}

#[test]
fn actor_grants_are_checked_again_on_retries_and_reads_need_no_permit_purpose() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = create(&root);
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let first = propose(&adapter, 21, &proposal);
    let image = canonical(&root);
    let mut foreign = actor_scope();
    foreign.caller_tenant = "2".to_owned();
    let mut no_plane = actor_scope();
    no_plane.allowed_planes.clear();
    let mut no_authority = actor_scope();
    no_authority.granted_authorities.clear();
    for scope in [foreign, no_plane, no_authority] {
        let denied = TowerAccess::new(scope, true, vec![PurposeTag::Permit], u64::MAX);
        let response = adapter.dispatch(&denied, first.clone());
        assert!(matches!(
            response.result,
            PublicationResult::Refused(Knowledge::Withheld { .. })
        ));
        refused(&response);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
    }
    for purposes in [Vec::new(), vec![PurposeTag::Experiment]] {
        let denied = TowerAccess::new(actor_scope(), true, purposes, u64::MAX);
        let response = adapter.dispatch(&denied, first.clone());
        refused(&response);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::PurposeMismatch)
        );
    }
    assert_eq!(canonical(&root), image);
    pending(&adapter.dispatch(&access(), first.clone()), 21);
    pending(&poll(&mut adapter, 21), 21);

    observe(&mut supervisor);
    let denied_retry = adapter.dispatch(&read_access(), first);
    refused(&denied_retry);
    assert_eq!(
        denied_retry.envelope.reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    let second = propose(&adapter, 22, &proposal);
    pending(&adapter.dispatch(&access(), second), 22);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 2);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn full_width_identity_and_original_exact_binding_survive_session_reconnection() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = create(&root);
    let reconnect = port.clone();
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let request = u64::MAX;
    let first = propose(&adapter, request, &proposal);
    pending(&adapter.dispatch(&access(), first.clone()), request);
    let image = canonical(&root);
    assert_eq!(
        adapter.request_address(request).unwrap().id,
        format!("request-3-2-4-5-{request}")
    );
    let mut new_operation_label = first.clone();
    new_operation_label.operation_id = "reconnected-exact-request".to_owned();
    pending(&adapter.dispatch(&access(), new_operation_label), request);

    let mut changed = proposal.clone();
    changed.payload = b"different".to_vec();
    let conflict = propose(&adapter, request, &changed);
    assert_eq!(
        native(&adapter.dispatch(&access(), conflict.clone())).result,
        Err(WireError::IdempotencyConflict)
    );
    pending(&poll(&mut adapter, request), request);
    let mut fresh = tower(reconnect);
    assert!(matches!(
        native(&poll(&mut fresh, request)).result,
        Ok(ActorKnowledge::Withheld { .. })
    ));
    assert_eq!(
        native(&fresh.dispatch(&access(), conflict)).result,
        Err(WireError::IdempotencyConflict)
    );
    assert!(matches!(
        native(&poll(&mut fresh, request)).result,
        Ok(ActorKnowledge::Withheld { .. })
    ));
    pending(&fresh.dispatch(&access(), first), request);
    let current = poll(&mut fresh, request);
    pending(&current, request);
    assert!(
        current
            .to_json()
            .unwrap()
            .contains(&format!("\"{request}\""))
    );
    assert_eq!(canonical(&root), image);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
}

#[test]
fn borrowed_or_dropped_original_owner_remains_unavailable_without_stale_cached_success() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = create(&root);
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let first = propose(&adapter, 9, &proposal);
    pending(&adapter.dispatch(&access(), first.clone()), 9);
    let image = canonical(&root);
    {
        let _original_owner = supervisor.host_mut().unwrap();
        unavailable(&poll(&mut adapter, 9));
        let new = propose(&adapter, 10, &proposal);
        assert_eq!(
            native(&adapter.dispatch(&access(), new)).result,
            Err(WireError::Unavailable)
        );
        refused(&adapter.dispatch(&read_access(), first.clone()));
    }
    pending(&adapter.dispatch(&access(), first), 9);
    assert_eq!(canonical(&root), image);
    drop(supervisor);
    unavailable(&poll(&mut adapter, 9));
    let new = propose(&adapter, 10, &proposal);
    assert_eq!(
        native(&adapter.dispatch(&access(), new)).result,
        Err(WireError::Unavailable)
    );
    // A still-live Tower and its retained tickets hold no strong file owner.
    let (host, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn failed_native_durable_replacement_never_creates_a_speculative_request_ticket() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = create(&root);
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let image = canonical(&root);
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    let call = propose(&adapter, 81, &proposal);
    let response = adapter.dispatch(&access(), call);
    assert_eq!(native(&response).result, Err(WireError::Unavailable));
    assert!(response.envelope.retry_safe);
    assert_eq!(canonical(&root), image);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    assert!(supervisor.host().unwrap().storage_failure().is_some());
    assert!(matches!(
        native(&poll(&mut adapter, 81)).result,
        Ok(ActorKnowledge::Withheld { .. })
    ));
    drop(supervisor);

    let (mut host, _fresh_reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.retained_requests(), 0);
    let revision = host.revision();
    host.observe_time(revision, ElapsedTick(2)).unwrap();
    let fresh = support::proposal(&oversight::spec(&host, b"published"));
    let (port, mut supervisor) = host.into_actor_gateway();
    let mut recovered = tower(port);
    observe(&mut supervisor);
    let call = propose(&recovered, 82, &fresh);
    pending(&recovered.dispatch(&access(), call), 82);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn native_not_admitted_is_a_retained_terminal_observation_not_a_permit() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, mut proposal) = create(&root);
    let mut adapter = tower(port);
    // A structurally valid frozen proposal names a non-current policy epoch.
    proposal.expected_policy_epoch += 1;
    observe(&mut supervisor);
    let call = propose(&adapter, 51, &proposal);
    let response = adapter.dispatch(&access(), call.clone());
    known(&response, 51, ActorOutcome::NotAdmitted);
    let Ok(ActorKnowledge::Known { basis, .. }) = &native(&response).result else {
        unreachable!();
    };
    assert_eq!(basis.source, BasisSource::Intake);
    let image = canonical(&root);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .inspect()
            .control
            .ledger
            .available,
        100
    );
    known(
        &adapter.dispatch(&access(), call),
        51,
        ActorOutcome::NotAdmitted,
    );
    assert_eq!(canonical(&root), image);
}

#[test]
fn complete_common_envelope_preserves_native_knowledge_without_private_review_data() {
    let root = Directory::new();
    let (port, mut supervisor, reviewer, proposal) = create(&root);
    let mut adapter = tower(port);
    observe(&mut supervisor);
    let call = propose(&adapter, 60, &proposal);
    let mut responses = vec![adapter.dispatch(&access(), call)];
    let keys = publish(&mut supervisor, &reviewer, 60);
    responses.push(poll(&mut adapter, 60));
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.stages[&keys.automatic.attempt()],
        ActionState::Confirmed
    );
    let missing = poll(&mut adapter, 61);
    assert!(matches!(
        native(&missing).result,
        Ok(ActorKnowledge::Withheld { .. })
    ));
    responses.push(missing);
    let mut wrong = propose(&adapter, 60, &proposal);
    wrong.idempotency_key = Some("060".to_owned());
    responses.push(adapter.dispatch(&access(), wrong));
    let image = canonical(&root);
    let expected: BTreeSet<&str> = ENVELOPE_FIELDS.into_iter().collect();
    for response in responses {
        no_global_claims(&response);
        let encoded = response.to_json().unwrap();
        assert!(encoded.len() <= response.envelope.limits.max_response_bytes);
        let parsed = strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
        let outer = parsed.as_object().unwrap();
        assert_eq!(outer.len(), 2);
        let envelope = outer["envelope"].as_object().unwrap();
        assert_eq!(
            envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            expected
        );
        if let PublicationResult::Request(original) = &response.result {
            let native_json =
                strict_json::parse(&original.encode(), KNOWLEDGE_JSON_LIMITS).unwrap();
            let result = outer["result"].as_object().unwrap();
            assert_eq!(result["kind"].as_str(), Some("publication-request"));
            assert_eq!(result["value"], native_json);
        }
        for private in [
            "PRIVATE-SOURCE-BYTES",
            "505249564154452d534f555243452d4259544553",
            "original-alpha",
            "original-beta",
            "exact-view-file-v1",
            "Approve this exact publication?",
            "published",
            "attempt_id",
            "ledger.available",
            "human_request",
        ] {
            assert!(
                !encoded.contains(private),
                "private detail leaked: {private}"
            );
        }
    }
    assert_eq!(canonical(&root), image);
}

#[test]
fn mount_is_bound_to_native_identity_and_bounded_registration_before_any_request() {
    let root = Directory::new();
    let (port, supervisor, _reviewer, _proposal) = create(&root);
    let image = canonical(&root);
    let mut invalid = Vec::new();
    let mut wrong_tenant = mount();
    wrong_tenant.tenant = "2".to_owned();
    invalid.push(wrong_tenant);
    let mut padded_tenant = mount();
    padded_tenant.tenant = "01".to_owned();
    invalid.push(padded_tenant);
    let mut wrong_run = mount();
    wrong_run.id = "4".to_owned();
    invalid.push(wrong_run);
    let mut wrong_noun = mount();
    wrong_noun.kind = Noun::Branch;
    invalid.push(wrong_noun);
    for generation in [None, Some(0)] {
        let mut address = mount();
        address.generation = generation;
        invalid.push(address);
    }
    for address in invalid {
        assert!(
            PublicationTower::new(address, port.clone(), PublicationLimits::default()).is_err()
        );
    }
    for max_response_bytes in [0, 16 * 1024 - 1, 1024 * 1024 + 1] {
        assert!(matches!(
            PublicationTower::new(
                mount(),
                port.clone(),
                PublicationLimits { max_response_bytes },
            ),
            Err(TowerInstallError::InvalidLimits)
        ));
    }
    let mut adapter = tower(port);
    assert_eq!(adapter.mount_address(), &mount());
    assert_eq!(
        adapter.request_address(0),
        Err(TowerInstallError::InvalidAddress)
    );
    // Raw ledger attempt IDs and another request namespace are not session tickets.
    for id in ["1", "request-99-2-4-5-1", "request-3-2-4-5-01"] {
        let address = Address::new("1", Noun::Attempt, id, Some(REGISTRATION_GENERATION)).unwrap();
        let call = TowerRequest::new("invalid-ticket", address, Verb::Get);
        refused(&adapter.dispatch(&read_access(), call));
    }
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    assert_eq!(canonical(&root), image);
}
