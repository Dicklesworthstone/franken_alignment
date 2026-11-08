//! Privileged, live native situations with separate revocable host custody.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::abandonment::FileAbandonRequest;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, replay::situation::FileSituationOrigin,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileInspectionPage, FileInspectionResult, MAX_INSPECTION_ATTEMPTS, MAX_INSPECTION_QUERIES,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolverPlane};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{AuthorityKind, Knowledge};
use fa_reference::strict_json;
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerRequest,
    Verb, encode_request,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use support::{Directory, Port, create, observe, oversight, profile, ready};

const REGISTRATION: u64 = 41;

#[path = "tower_inspection/pagination.rs"]
mod pagination;

fn tower(port: Port) -> PublicationTower {
    let scope = profile().delivery.scope;
    PublicationTower::new(
        Address::new(
            scope.tenant.to_string(),
            Noun::Run,
            scope.run.to_string(),
            Some(REGISTRATION),
        )
        .unwrap(),
        port,
        PublicationLimits::default(),
    )
    .unwrap()
}

fn scope() -> CallerScope {
    CallerScope {
        caller_tenant: profile().delivery.scope.tenant.to_string(),
        allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
        granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
    }
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(scope(), operator, Vec::new(), 0)
}

fn request(adapter: &PublicationTower) -> TowerRequest {
    TowerRequest::new("inspect", adapter.mount_address().clone(), Verb::Situation)
}

fn inspect(adapter: &mut PublicationTower) -> PublicationResponse {
    let call = request(adapter);
    adapter.dispatch(&access(true), call)
}

fn native(response: &PublicationResponse) -> &FileInspectionResult {
    match &response.result {
        PublicationResult::Situation(result) => result,
        other => panic!("expected original supervisor situation: {other:?}"),
    }
}

fn known(response: &PublicationResponse) -> &FileInspectionPage {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected native Known situation: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(basis.origin, FileSituationOrigin::AcknowledgedOwner);
    assert_eq!(basis.anchored_through, None);
    assert_eq!(response.envelope.reason_code, None);
    no_global_claims(response);
    value
}

fn unknown(response: &PublicationResponse) {
    assert_eq!(
        native(response),
        &Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::ControllerUnavailable,
        })
    );
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::LowerLayerUnverified)
    );
    no_global_claims(response);
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
    no_global_claims(response);
}

fn no_global_claims(response: &PublicationResponse) {
    fn unavailable<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unavailable(&response.envelope.generation);
    unavailable(&response.envelope.control_seq);
    unavailable(&response.envelope.source_frontiers);
    unavailable(&response.envelope.budgets_remaining);
    unavailable(&response.envelope.realized_cost);
    unavailable(&response.envelope.affordances);
}

fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    std::fs::read_dir(root.store())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

#[test]
fn original_live_situations_follow_real_review_and_publication_without_consuming_admission() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    let mut adapter = tower(actor.clone());
    refused(&inspect(&mut adapter), ReasonCode::CapabilityUnsupported);
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    adapter.set_inspection(Some(port)).unwrap();
    observe(&mut supervisor);
    let original = supervisor.host().unwrap().inspect();
    let image = files(&root);
    let empty = inspect(&mut adapter);
    let page = known(&empty);
    assert!(page.attempts().is_empty() && page.enumeration_complete());
    assert_eq!(page.summary().available_units, 100);
    assert_eq!(page.basis().journal_revision, original.revision);
    assert_eq!(page.basis().control_sequence, original.control.sequence);
    assert_eq!(page.basis().recorded_at, Some(ElapsedTick(1)));
    assert_eq!(files(&root), image);
    assert_eq!(supervisor.host().unwrap().inspect(), original);

    // The pending snapshot survives inspection. A real new submission can use
    // it, but the read has supplied neither review nor a publication key.
    let ticket = actor.submit(701, &proposal).unwrap();
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: 701 }
    );
    let pending = inspect(&mut adapter);
    let page = known(&pending);
    assert_eq!(page.summary().retained_requests, 1);
    assert_eq!(page.summary().pending_attempts, 1);
    assert_eq!(page.summary().executions, 0);
    assert_eq!(page.attempts()[0].stage, ActionState::Reviewing);
    assert_ne!(page.attempts()[0].attempt, 701);
    let keys = ready(&mut supervisor, &reviewer, 701);
    let approved = inspect(&mut adapter);
    assert_eq!(known(&approved).summary().reserved_units, 16);
    assert_eq!(known(&approved).summary().executions, 0);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.publish(revision, keys.automatic.attempt()).unwrap();
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let image = files(&root);
    let completed = inspect(&mut adapter);
    let page = known(&completed);
    assert_eq!(page.summary().executions, 1);
    assert_eq!(page.summary().terminal_attempts, 1);
    assert_eq!(page.summary().unresolved_attempts, 0);
    assert_eq!(page.summary().charged_units, 16);
    assert_eq!(page.attempts()[0].stage, ActionState::Confirmed);
    assert_eq!(files(&root), image);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(known(&empty).summary().attempts, 0);
    assert_eq!(known(&pending).attempts()[0].stage, ActionState::Reviewing);
}

#[test]
fn native_unknown_and_abandonment_remain_charged_unresolved_liabilities() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    let mut adapter = tower(actor.clone());
    let (port, _grant) = supervisor.inspection_port(2).unwrap();
    adapter.set_inspection(Some(port)).unwrap();
    observe(&mut supervisor);
    let ticket = actor.submit(701, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, 701);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.fence(revision).unwrap();
    }
    let unresolved = inspect(&mut adapter);
    let page = known(&unresolved);
    assert_eq!(page.summary().unresolved_attempts, 1);
    assert_eq!(page.summary().terminal_attempts, 0);
    assert_eq!(page.summary().charged_units, 16);
    assert_eq!(page.summary().executions, 0);
    assert_eq!(page.attempts()[0].stage, ActionState::Unknown);
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Unknown {
            reason: ActorUnknown::OutcomeUnknown
        }
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let control = host.inspect().control;
        let revision = host.revision();
        host.abandon_unknown(
            revision,
            FileAbandonRequest {
                operation: 900,
                attempt: keys.automatic.attempt(),
                expected_control_sequence: control.sequence,
                expected_authority_epoch: control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let image = files(&root);
    let abandoned = inspect(&mut adapter);
    let page = known(&abandoned);
    assert_eq!(page.summary().irrecoverably_unknown_attempts, 1);
    assert_eq!(page.summary().unresolved_attempts, 1);
    assert_eq!(page.summary().terminal_attempts, 0);
    assert_eq!(page.summary().charged_units, 16);
    assert_eq!(page.attempts()[0].stage, ActionState::IrrecoverablyUnknown);
    assert_eq!(files(&root), image);
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Unknown {
            reason: ActorUnknown::OutcomeUnknown
        }
    );
}

#[test]
fn grants_bound_repeated_reads_and_clones_and_revocation_cannot_be_reconnected_away() {
    let root = Directory::new();
    let (actor, supervisor, _reviewer, _proposal) = create(&root);
    assert!(supervisor.inspection_port(0).is_err());
    assert!(
        supervisor
            .inspection_port(MAX_INSPECTION_QUERIES + 1)
            .is_err()
    );
    let (_, maximum) = supervisor.inspection_port(MAX_INSPECTION_QUERIES).unwrap();
    assert_eq!(maximum.remaining_queries(), MAX_INSPECTION_QUERIES);
    let (port, grant) = supervisor.inspection_port(3).unwrap();
    let mut first = tower(actor.clone());
    first.set_inspection(Some(port.clone())).unwrap();
    let mut second = tower(actor);
    second.set_inspection(Some(port.clone())).unwrap();
    let mut call = request(&first);
    call.idempotency_key = Some("same-observation-label".to_owned());
    let image = files(&root);
    known(&first.dispatch(&access(true), call.clone()));
    known(&second.dispatch(&access(true), call));
    assert_eq!(grant.remaining_queries(), 1);
    assert!(matches!(port.situation(), Ok(ActorKnowledge::Known { .. })));
    assert_eq!(grant.remaining_queries(), 0);
    let exhausted = inspect(&mut first);
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    second.set_inspection(Some(port)).unwrap();
    assert_eq!(native(&inspect(&mut second)), &Err(ActorError::Capacity));
    grant.revoke();
    let revoked = inspect(&mut second);
    assert_eq!(
        native(&revoked),
        &Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_inspection",
        })
    );
    assert_eq!(
        revoked.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );

    // Revocation also withdraws a live, unspent grant. Reinstalling a clone
    // cannot recover its former allowance, whether custody is consumed
    // explicitly or simply dropped by the host.
    for explicit_revoke in [false, true] {
        let (port, live_grant) = supervisor.inspection_port(2).unwrap();
        first.set_inspection(Some(port.clone())).unwrap();
        second.set_inspection(Some(port.clone())).unwrap();
        known(&inspect(&mut first));
        assert_eq!(live_grant.remaining_queries(), 1);
        if explicit_revoke {
            live_grant.revoke();
        } else {
            drop(live_grant);
        }
        second.set_inspection(Some(port.clone())).unwrap();
        for response in [inspect(&mut first), inspect(&mut second)] {
            assert_eq!(
                native(&response),
                &Ok(ActorKnowledge::Withheld {
                    authority_required: "supervisor_inspection",
                })
            );
        }
        assert_eq!(
            port.situation(),
            Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_inspection",
            })
        );
    }
    assert_eq!(files(&root), image);
}

#[test]
fn operator_identity_mount_and_argument_checks_precede_native_inspection_spending() {
    let root = Directory::new();
    let (actor, supervisor, _reviewer, _proposal) = create(&root);
    let mut adapter = tower(actor);
    let call = request(&adapter);
    let ungranted_actor = adapter.dispatch(&access(false), call.clone());
    assert!(matches!(
        ungranted_actor.result,
        PublicationResult::Refused(Knowledge::Withheld { .. })
    ));
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    adapter.set_inspection(Some(port)).unwrap();
    let denied = adapter.dispatch(&access(false), call.clone());
    assert_eq!(denied, ungranted_actor);
    let mut missing_authority = scope();
    missing_authority.granted_authorities.clear();
    let mut missing_plane = scope();
    missing_plane.allowed_planes.clear();
    let mut wrong_tenant = scope();
    wrong_tenant.caller_tenant = "foreign".to_owned();
    for denied_scope in [missing_authority, missing_plane, wrong_tenant] {
        let denied = TowerAccess::new(denied_scope, true, Vec::new(), 0);
        assert!(matches!(
            adapter.dispatch(&denied, call.clone()).result,
            PublicationResult::Refused(Knowledge::Withheld { .. })
        ));
    }
    let mut malformed = call.clone();
    malformed.arguments = ReadArguments::Next { max_cost: 0 };
    refused(
        &adapter.dispatch(&access(true), malformed),
        ReasonCode::IntegrityFailure,
    );
    let mut stale = call.clone();
    stale.address.generation = Some(REGISTRATION - 1);
    refused(
        &adapter.dispatch(&access(true), stale),
        ReasonCode::StaleGeneration,
    );
    let mut foreign = call.clone();
    foreign.address.id = "other-run".to_owned();
    refused(
        &adapter.dispatch(&access(true), foreign),
        ReasonCode::MissingRequiredObservation,
    );
    let mut ticket = call.clone();
    ticket.address = adapter.request_address(701).unwrap();
    refused(
        &adapter.dispatch(&access(true), ticket),
        ReasonCode::MissingRequiredObservation,
    );
    let document = encode_request(&call).unwrap();
    let extra = document.replacen('{', "{\"operator\":true,", 1);
    refused(
        &adapter.exchange(&access(true), extra.as_bytes()),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 2);
    known(&adapter.exchange(&access(true), document.as_bytes()));
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn foreign_owner_replacement_withdraws_previous_inspection_without_spending_either_grant() {
    let root = Directory::new();
    let foreign_root = Directory::new();
    let (actor, supervisor, _reviewer, _proposal) = create(&root);
    let (_, foreign_supervisor, _, _) = create(&foreign_root);
    let (original, grant) = supervisor.inspection_port(3).unwrap();
    let (foreign, foreign_grant) = foreign_supervisor.inspection_port(3).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(original.clone())).unwrap();
    known(&inspect(&mut adapter));
    assert!(adapter.set_inspection(Some(foreign)).is_err());
    refused(&inspect(&mut adapter), ReasonCode::CapabilityUnsupported);
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_queries(), 3);
    adapter.set_inspection(Some(original)).unwrap();
    known(&inspect(&mut adapter));
    adapter.set_inspection(None).unwrap();
    refused(&inspect(&mut adapter), ReasonCode::CapabilityUnsupported);
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn borrow_fault_owner_loss_and_reopen_never_fall_back_to_a_cached_or_disk_situation() {
    for faulted in [false, true] {
        let root = Directory::new();
        let (actor, mut supervisor, _reviewer, _proposal) = create(&root);
        let mut adapter = tower(actor);
        let (port, grant) = supervisor.inspection_port(3).unwrap();
        adapter.set_inspection(Some(port.clone())).unwrap();
        known(&inspect(&mut adapter));
        {
            let mut host = supervisor.host_mut().unwrap();
            unknown(&inspect(&mut adapter));
            assert_eq!(grant.remaining_queries(), 2);
            if faulted {
                std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
            }
            let revision = host.revision();
            let changed = host.observe_time(revision, ElapsedTick(2));
            assert_eq!(changed.is_err(), faulted);
        }
        if faulted {
            unknown(&inspect(&mut adapter));
            assert_eq!(grant.remaining_queries(), 2);
        } else {
            let latest = inspect(&mut adapter);
            assert_eq!(known(&latest).basis().recorded_at, Some(ElapsedTick(2)));
            assert_eq!(grant.remaining_queries(), 1);
        }
        let remaining = grant.remaining_queries();
        let image = files(&root);
        drop(supervisor);
        unknown(&inspect(&mut adapter));
        assert_eq!(files(&root), image);
        let (host, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
        let (actor, new_supervisor) = host.into_actor_gateway();
        unknown(&inspect(&mut adapter));
        let mut reopened = tower(actor);
        assert!(reopened.set_inspection(Some(port)).is_err());
        let (fresh, _custodian) = new_supervisor.inspection_port(1).unwrap();
        reopened.set_inspection(Some(fresh)).unwrap();
        known(&inspect(&mut reopened));
        assert_eq!(grant.remaining_queries(), remaining);
    }
}

#[test]
fn bounded_first_page_retains_full_cut_counts_and_never_claims_full_enumeration() {
    let root = Directory::new();
    let mut config = profile();
    config.delivery.max_attempts = MAX_INSPECTION_ATTEMPTS + 1;
    let (mut host, _reviewer) = FileOversight::create(root.store(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (actor, mut supervisor) = host.into_actor_gateway();
    let mut adapter = tower(actor.clone());
    let (port, _grant) = supervisor.inspection_port(1).unwrap();
    adapter.set_inspection(Some(port)).unwrap();
    for id in 1..=MAX_INSPECTION_ATTEMPTS + 1 {
        observe(&mut supervisor);
        actor.submit(1000 + id as u64, &proposal).unwrap();
    }
    let image = files(&root);
    let response = inspect(&mut adapter);
    let page = known(&response);
    assert_eq!(page.attempts().len(), MAX_INSPECTION_ATTEMPTS);
    assert_eq!(page.summary().attempts, MAX_INSPECTION_ATTEMPTS + 1);
    assert_eq!(
        page.summary().retained_requests,
        MAX_INSPECTION_ATTEMPTS + 1
    );
    assert_eq!(page.summary().pending_attempts, MAX_INSPECTION_ATTEMPTS + 1);
    assert_eq!(page.summary().available_units, 100);
    assert_eq!(page.offset(), 0);
    assert!(!page.enumeration_complete());
    assert_eq!(
        page.attempts()
            .iter()
            .map(|row| row.attempt)
            .collect::<Vec<_>>(),
        (1..=MAX_INSPECTION_ATTEMPTS as u64).collect::<Vec<_>>()
    );
    let json = response.to_json().unwrap();
    assert!(json.contains("\"enumeration_complete\":false"));
    assert!(json.len() <= PublicationLimits::default().max_response_bytes);
    assert_eq!(files(&root), image);
}

#[test]
fn canonical_encoding_preserves_native_basis_and_refuses_reconstructed_provenance() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    let mut adapter = tower(actor.clone());
    let (port, grant) = supervisor.inspection_port(1).unwrap();
    adapter.set_inspection(Some(port)).unwrap();
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let response = inspect(&mut adapter);
    let page = known(&response);
    let json = response.to_json().unwrap();
    let parsed = strict_json::parse(json.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
    let outer = parsed.as_object().unwrap();
    let envelope = outer["envelope"].as_object().unwrap();
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect()
    );
    let result = outer["result"].as_object().unwrap();
    assert_eq!(result["kind"].as_str(), Some("situation"));
    let situation = result["value"].as_object().unwrap();
    assert_eq!(
        situation["schema"].as_str(),
        Some("fa.publication-situation/1")
    );
    assert_eq!(
        situation["current_eligibility"].as_str(),
        Some("not_observed")
    );
    let knowledge = situation["knowledge"].as_object().unwrap();
    let basis = knowledge["basis"].as_object().unwrap();
    assert_eq!(basis["origin"].as_str(), Some("acknowledged_owner"));
    assert_eq!(
        basis["journal_revision"].as_str(),
        Some(page.basis().journal_revision.to_string().as_str())
    );
    assert_eq!(
        basis["control_sequence"].as_str(),
        Some(page.basis().control_sequence.to_string().as_str())
    );
    for secret in [
        "PRIVATE-SOURCE-BYTES",
        "Approve this exact publication?",
        "exact-view-file-v1",
        "original-alpha",
        "original-beta",
    ] {
        assert!(!json.contains(secret), "private detail leaked: {secret}");
    }
    let mut forged = response.clone();
    let PublicationResult::Situation(Ok(ActorKnowledge::Known { basis, .. })) = &mut forged.result
    else {
        unreachable!()
    };
    basis.control_sequence += 1;
    assert_eq!(forged.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    forged.result = PublicationResult::Situation(Ok(ActorKnowledge::Pending { request: 701 }));
    assert_eq!(forged.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    let exhausted = inspect(&mut adapter);
    assert!(
        exhausted
            .to_json()
            .unwrap()
            .contains("\"code\":\"capacity\"")
    );
    grant.revoke();
    let revoked = inspect(&mut adapter);
    assert!(
        revoked
            .to_json()
            .unwrap()
            .contains("\"authority_required\":\"supervisor_inspection\"")
    );
    let (port, _grant) = supervisor.inspection_port(1).unwrap();
    adapter.set_inspection(Some(port)).unwrap();
    drop(supervisor);
    let lost = inspect(&mut adapter);
    assert!(
        lost.to_json()
            .unwrap()
            .contains("\"reason\":\"controller_unavailable\"")
    );
}
