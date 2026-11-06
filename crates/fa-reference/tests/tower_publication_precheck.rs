//! Source-backed exact disclosure through the publication Tower. The native
//! owner, source lease, custodian and witness cache retain their original roles.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::Snapshot;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, FileOversightProfile, source::FileSourcePolicy,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorPrecheckPort, FilePrecheckBasis, FilePrecheckDisclosure, FilePrecheckOutcome,
    precheck_wire::{
        PrecheckWireError, PrecheckWireResponse,
        cache::{FilePrecheckCache, PrecheckCacheLimits},
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    actor::{
        ActorOutcome, ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
    },
    actor_wire::WireError,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use fa_reference::action::{ActionState, ElapsedTick, MAX_PAYLOAD_BYTES};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolverPlane};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{AuthorityKind, Knowledge};
use fa_reference::strict_json;
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerRequest, Verb,
};
use fa_reference::typestate_cx::PurposeTag;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use support::{Directory, Port, Supervisor};

const REGISTRATION: u64 = 41;
const DISCLOSURE: u64 = 81;
const PRIVATE_INPUT: &[u8] = b"SECRET-PRECHECK-SOURCE-BYTES";

fn profile() -> FileOversightProfile {
    let mut profile = support::profile();
    profile.delivery.policy = Policy::new(
        1,
        vec![
            Predicate::ExactValue {
                key: 7,
                value: b"ok".to_vec(),
            },
            Predicate::PayloadAtMost(4),
            Predicate::All(vec![0, 1]),
        ],
    )
    .unwrap();
    profile
}

fn snapshot(value: &[u8]) -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(7, value.to_vec())]),
    }
}

fn source_path(root: &Directory) -> PathBuf {
    root.0.join("precheck-source.json")
}

fn write_source(root: &Directory, generation: u64, value: &[u8]) {
    let source = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation,
            scope: profile().delivery.scope,
        },
        snapshot(value),
        BTreeMap::from([
            ("alpha".to_owned(), PRIVATE_INPUT.to_vec()),
            ("beta".to_owned(), PRIVATE_INPUT.to_vec()),
        ]),
    )
    .unwrap();
    std::fs::write(source_path(root), source.encode()).unwrap();
}

fn owner(root: &Directory) -> (Port, Supervisor, FileEvidenceSource) {
    let profile = profile();
    let (mut host, _reviewer) = FileOversight::create(root.store(), profile.clone()).unwrap();
    host.enable_file_source(
        host.revision(),
        FileSourcePolicy {
            source: StateSource {
                scope: profile.delivery.scope,
                source: 51,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(5).unwrap(),
        },
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    write_source(root, 1, b"ok");
    let mut source =
        FileEvidenceSource::new(source_path(root), 51, profile.delivery.scope, 1_048_576).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    (actor, supervisor, source)
}

fn tower(actor: Port) -> PublicationTower {
    let scope = profile().delivery.scope;
    PublicationTower::new(
        Address::new(
            scope.tenant.to_string(),
            Noun::Run,
            scope.run.to_string(),
            Some(REGISTRATION),
        )
        .unwrap(),
        actor,
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

// Exact policy inspection needs the separately installed disclosure capability,
// not an operator role, Permit purpose or visibility into unrelated data planes.
fn reader() -> TowerAccess {
    TowerAccess::new(scope(), false, Vec::new(), 0)
}

fn proposer() -> TowerAccess {
    TowerAccess::new(scope(), false, vec![PurposeTag::Permit], 0)
}

fn candidate(payload: &[u8]) -> ActorProposal {
    ActorProposal {
        target: profile().delivery.target,
        payload: payload.to_vec(),
        units: 8,
        deadline: ElapsedTick(100),
        expected_policy_epoch: 0,
    }
}

fn disclose(
    supervisor: &Supervisor,
    id: u64,
    queries: usize,
) -> (FileActorPrecheckPort, FilePrecheckDisclosure) {
    supervisor
        .disclose_exact_precheck(supervisor.host().unwrap().revision(), id, queries)
        .unwrap()
}

fn cached_disclosure(
    supervisor: &Supervisor,
    id: u64,
    queries: usize,
    cache: &FilePrecheckCache,
) -> (FileActorPrecheckPort, FilePrecheckDisclosure) {
    supervisor
        .disclose_exact_precheck_cached(supervisor.host().unwrap().revision(), id, queries, cache)
        .unwrap()
}

fn request(adapter: &PublicationTower, query: u64, proposal: &ActorProposal) -> TowerRequest {
    let mut call = TowerRequest::new("precheck", adapter.mount_address().clone(), Verb::Precheck);
    call.arguments = ReadArguments::Precheck {
        query,
        proposal: proposal.clone(),
    };
    call
}

fn check(
    adapter: &mut PublicationTower,
    query: u64,
    proposal: &ActorProposal,
) -> PublicationResponse {
    let call = request(adapter, query, proposal);
    adapter.dispatch(&reader(), call)
}

fn submit(
    adapter: &mut PublicationTower,
    key: u64,
    proposal: &ActorProposal,
) -> PublicationResponse {
    let mut call = TowerRequest::new("submit", adapter.mount_address().clone(), Verb::Propose);
    call.arguments = ReadArguments::Propose {
        request: key,
        proposal: proposal.clone(),
    };
    call.idempotency_key = Some(key.to_string());
    adapter.dispatch(&proposer(), call)
}

fn observation(response: &PublicationResponse) -> &PrecheckWireResponse {
    match &response.result {
        PublicationResult::Precheck(value) => value,
        other => panic!("expected original precheck result, got {other:?}"),
    }
}

fn known(
    response: &PublicationResponse,
    value: FilePrecheckOutcome,
    query: u64,
    disclosure: u64,
) -> FilePrecheckBasis {
    assert_eq!(observation(response).query, Some(query));
    let Ok(ActorKnowledge::Known {
        value: actual,
        basis,
    }) = &observation(response).result
    else {
        panic!("expected a native Known precheck result: {response:?}");
    };
    assert_eq!(*actual, value);
    assert_eq!(
        (basis.disclosure, basis.query, basis.policy_epoch),
        (disclosure, query, 0)
    );
    no_global_claims(response);
    *basis
}

fn unknown(response: &PublicationResponse) {
    assert_eq!(
        observation(response).result,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::ControllerUnavailable,
        })
    );
    no_global_claims(response);
}

fn unsupported(response: &PublicationResponse) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
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
fn source_backed_pass_and_denial_remain_read_only_and_submission_still_requires_review() {
    for (payload, expected, stage) in [
        (
            b"ok".as_slice(),
            FilePrecheckOutcome::PrecheckPassed,
            ActionState::Reviewing,
        ),
        (
            b"blocked".as_slice(),
            FilePrecheckOutcome::DeniedByPolicy,
            ActionState::Denied,
        ),
    ] {
        let root = Directory::new();
        let (actor, mut supervisor, source) = owner(&root);
        let mut adapter = tower(actor);
        let proposal = candidate(payload);
        unsupported(&check(&mut adapter, 901, &proposal));
        let (port, grant) = disclose(&supervisor, DISCLOSURE, 4);
        adapter.set_precheck(Some(port)).unwrap();
        let before = supervisor.host().unwrap().inspect();
        let original_files = files(&root);
        let reads = source.status().read_attempts;
        for _ in 0..2 {
            let basis = known(
                &check(&mut adapter, 901, &proposal),
                expected,
                901,
                DISCLOSURE,
            );
            assert_eq!(basis.observed_at, ElapsedTick(1));
        }
        assert_eq!(grant.remaining_queries(), 2);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(files(&root), original_files);
        assert_eq!(source.status().read_attempts, reads);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 0);

        // A successful precheck supplies neither the one-use snapshot nor a
        // ticket. Even the identical numeric label is still a new submission.
        let attempted = submit(&mut adapter, 901, &proposal);
        let PublicationResult::Request(native) = attempted.result else {
            panic!("native submit path");
        };
        assert_eq!(native.result, Err(WireError::Unavailable));
        assert_eq!(files(&root), original_files);
        let revision = supervisor.host().unwrap().revision();
        supervisor
            .set_snapshot(revision, Some(snapshot(b"ok")))
            .unwrap();
        known(
            &check(&mut adapter, 901, &proposal),
            expected,
            901,
            DISCLOSURE,
        );
        let admitted = submit(&mut adapter, 901, &proposal);
        let PublicationResult::Request(native) = admitted.result else {
            panic!("native submit path");
        };
        let submitted_observation = native.result.clone();
        if stage == ActionState::Reviewing {
            assert_eq!(native.result, Ok(ActorKnowledge::Pending { request: 901 }));
        } else {
            assert!(matches!(
                native.result,
                Ok(ActorKnowledge::Known {
                    value: ActorOutcome::Denied,
                    ..
                })
            ));
        }
        let host = supervisor.host().unwrap();
        let current = host.inspect();
        assert_eq!(host.retained_requests(), 1);
        assert_eq!(current.control.ledger.stages[&1], stage);
        assert_eq!(current.control.ledger.available, 100);
        assert_eq!(current.control.ledger.reserved, 0);
        assert_eq!(current.executions, 0);
        assert!(current.control.decisions.is_empty());
        drop(host);
        unknown(&check(&mut adapter, 901, &proposal));
        assert_eq!(grant.remaining_queries(), 1);
        let call = TowerRequest::new("poll", adapter.request_address(901).unwrap(), Verb::Get);
        let polled = adapter.dispatch(&reader(), call);
        let PublicationResult::Request(native) = polled.result else {
            panic!("previously acquired request ticket must remain available");
        };
        assert_eq!(native.result, submitted_observation);
    }
}

#[test]
fn repeated_labels_cache_hits_and_new_facades_share_the_same_finite_native_query_grant() {
    let root = Directory::new();
    let (actor, supervisor, source) = owner(&root);
    let mut adapter = tower(actor.clone());
    let cache = supervisor
        .precheck_cache(PrecheckCacheLimits::default())
        .unwrap();
    let (port, grant) = cached_disclosure(&supervisor, DISCLOSURE, 4, &cache);
    adapter.set_precheck(Some(port.clone())).unwrap();
    let mut reconnected = tower(actor);
    reconnected.set_precheck(Some(port.clone())).unwrap();
    let proposal = candidate(b"ok");
    let query = u64::MAX;
    let image = files(&root);
    let reads = source.status().read_attempts;
    let mut call = request(&adapter, query, &proposal);
    call.idempotency_key = Some("same-read-label".to_owned());
    for remaining in [3, 2] {
        known(
            &adapter.dispatch(&reader(), call.clone()),
            FilePrecheckOutcome::PrecheckPassed,
            query,
            DISCLOSURE,
        );
        assert_eq!(grant.remaining_queries(), remaining);
    }
    assert_eq!(cache.status().unwrap().evaluations, 1);
    assert_eq!(cache.status().unwrap().hits, 1);
    known(
        &check(&mut reconnected, query, &proposal),
        FilePrecheckOutcome::PrecheckPassed,
        query,
        DISCLOSURE,
    );
    assert_eq!(grant.remaining_queries(), 1);
    known(
        &check(&mut adapter, 2, &proposal),
        FilePrecheckOutcome::PrecheckPassed,
        2,
        DISCLOSURE,
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(cache.status().unwrap().evaluations, 1);
    assert_eq!(cache.status().unwrap().hits, 3);
    assert_eq!(
        observation(&check(&mut adapter, query, &proposal)).result,
        Err(PrecheckWireError::Capacity)
    );
    reconnected.set_precheck(Some(port)).unwrap();
    assert_eq!(
        observation(&check(&mut reconnected, query, &proposal)).result,
        Err(PrecheckWireError::Capacity)
    );
    assert_eq!(files(&root), image);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
}

#[test]
fn identical_numeric_scope_does_not_bind_another_owner_and_replacement_withdraws_previous_port() {
    let first = Directory::new();
    let second = Directory::new();
    let (actor, supervisor, _) = owner(&first);
    let (_, foreign_supervisor, _) = owner(&second);
    let mut adapter = tower(actor);
    let (original, grant) = disclose(&supervisor, DISCLOSURE, 2);
    let (foreign, foreign_grant) = disclose(&foreign_supervisor, DISCLOSURE, 2);
    adapter.set_precheck(Some(original.clone())).unwrap();
    let images = (files(&first), files(&second));
    assert!(adapter.set_precheck(Some(foreign)).is_err());
    unsupported(&check(&mut adapter, 1, &candidate(b"ok")));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_queries(), 2);
    adapter.set_precheck(Some(original.clone())).unwrap();
    known(
        &check(&mut adapter, 1, &candidate(b"ok")),
        FilePrecheckOutcome::PrecheckPassed,
        1,
        DISCLOSURE,
    );
    adapter.set_precheck(None).unwrap();
    unsupported(&check(&mut adapter, 1, &candidate(b"ok")));
    assert_eq!(grant.remaining_queries(), 1);
    // Withdrawal of one facade does not invent a global revoke or new grant.
    assert!(matches!(
        original.check(1, &candidate(b"ok")).unwrap(),
        ActorKnowledge::Known {
            value: FilePrecheckOutcome::PrecheckPassed,
            ..
        }
    ));
    adapter.set_precheck(Some(original)).unwrap();
    assert_eq!(
        observation(&check(&mut adapter, 1, &candidate(b"ok"))).result,
        Err(PrecheckWireError::Capacity)
    );
    assert_eq!((files(&first), files(&second)), images);
}

#[test]
fn malformed_and_unauthorized_requests_do_not_enter_the_native_query_allowance() {
    let root = Directory::new();
    let (actor, supervisor, _) = owner(&root);
    let mut adapter = tower(actor);
    let (port, grant) = disclose(&supervisor, DISCLOSURE, 2);
    adapter.set_precheck(Some(port)).unwrap();
    let proposal = candidate(b"ok");
    let valid = request(&adapter, 71, &proposal);
    let mut malformed = Vec::new();
    for generation in [Some(REGISTRATION - 1), Some(REGISTRATION + 1)] {
        let mut call = valid.clone();
        call.address.generation = generation;
        malformed.push(call);
    }
    let mut other_run = valid.clone();
    other_run.address.id = "4".to_owned();
    malformed.push(other_run);
    let mut wrong_noun = valid.clone();
    wrong_noun.address = adapter.request_address(71).unwrap();
    malformed.push(wrong_noun);
    let mut invalid_id = valid.clone();
    invalid_id.operation_id.clear();
    malformed.push(invalid_id);
    let mut invalid_address = valid.clone();
    invalid_address.address.tenant = "bad/tenant".to_owned();
    malformed.push(invalid_address);
    let mut wrong_arguments = valid.clone();
    wrong_arguments.arguments = ReadArguments::None;
    malformed.push(wrong_arguments);
    malformed.push(request(&adapter, 0, &proposal));
    let mut bad_target = proposal.clone();
    bad_target.target.object = 0;
    malformed.push(request(&adapter, 1, &bad_target));
    let mut oversized = proposal.clone();
    oversized.payload = vec![0; MAX_PAYLOAD_BYTES + 1];
    oversized.units = oversized.payload.len() as u64;
    malformed.push(request(&adapter, 1, &oversized));
    let image = files(&root);
    for call in malformed {
        let response = adapter.dispatch(&reader(), call);
        assert!(response.envelope.reason_code.is_some());
        assert_eq!(grant.remaining_queries(), 2);
    }
    let mut wrong_tenant = scope();
    wrong_tenant.caller_tenant = "2".to_owned();
    let mut no_plane = scope();
    no_plane.allowed_planes.clear();
    let mut no_authority = scope();
    no_authority.granted_authorities.clear();
    for denied in [wrong_tenant, no_plane, no_authority] {
        let grants = TowerAccess::new(denied, true, vec![PurposeTag::Permit], u64::MAX);
        let response = adapter.dispatch(&grants, valid.clone());
        assert!(matches!(
            response.result,
            PublicationResult::Refused(Knowledge::Withheld { .. })
        ));
        assert_eq!(grant.remaining_queries(), 2);
    }
    known(
        &adapter.dispatch(&reader(), valid.clone()),
        FilePrecheckOutcome::PrecheckPassed,
        71,
        DISCLOSURE,
    );
    let mut unversioned = valid;
    unversioned.address.generation = None;
    known(
        &adapter.dispatch(&reader(), unversioned),
        FilePrecheckOutcome::PrecheckPassed,
        71,
        DISCLOSURE,
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(files(&root), image);
}

#[test]
fn revocation_owner_borrow_and_acknowledged_transition_cannot_reuse_a_cached_pass() {
    let root = Directory::new();
    let (actor, mut supervisor, _) = owner(&root);
    let mut adapter = tower(actor);
    let (port, grant) = disclose(&supervisor, DISCLOSURE, 4);
    adapter.set_precheck(Some(port.clone())).unwrap();
    known(
        &check(&mut adapter, 1, &candidate(b"ok")),
        FilePrecheckOutcome::PrecheckPassed,
        1,
        DISCLOSURE,
    );
    {
        let _owner = supervisor.host_mut().unwrap();
        unknown(&check(&mut adapter, 1, &candidate(b"ok")));
        assert_eq!(grant.remaining_queries(), 3);
    }
    known(
        &check(&mut adapter, 1, &candidate(b"ok")),
        FilePrecheckOutcome::PrecheckPassed,
        1,
        DISCLOSURE,
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    unknown(&check(&mut adapter, 1, &candidate(b"ok")));
    assert_eq!(grant.remaining_queries(), 2);
    let (fresh, fresh_grant) = disclose(&supervisor, DISCLOSURE + 1, 2);
    adapter.set_precheck(Some(fresh)).unwrap();
    assert_eq!(
        known(
            &check(&mut adapter, 1, &candidate(b"ok")),
            FilePrecheckOutcome::PrecheckPassed,
            1,
            DISCLOSURE + 1
        )
        .observed_at,
        ElapsedTick(2)
    );
    fresh_grant.revoke();
    let image = files(&root);
    assert_eq!(
        observation(&check(&mut adapter, 1, &candidate(b"ok"))).result,
        Ok(ActorKnowledge::Withheld {
            authority_required: "exact_policy_disclosure"
        })
    );
    assert_eq!(files(&root), image);
    adapter.set_precheck(Some(port)).unwrap();
    unknown(&check(&mut adapter, 1, &candidate(b"ok")));
    assert_eq!(grant.remaining_queries(), 2);
}

#[test]
fn original_source_witness_cache_invalidates_changed_values_and_never_refreshes_from_actor_snapshot()
 {
    let root = Directory::new();
    let (actor, mut supervisor, mut source) = owner(&root);
    let mut adapter = tower(actor);
    let cache = supervisor
        .precheck_cache(PrecheckCacheLimits::default())
        .unwrap();
    let (port, original_grant) = cached_disclosure(&supervisor, DISCLOSURE, 3, &cache);
    adapter.set_precheck(Some(port)).unwrap();
    known(
        &check(&mut adapter, 1, &candidate(b"ok")),
        FilePrecheckOutcome::PrecheckPassed,
        1,
        DISCLOSURE,
    );
    write_source(&root, 2, b"no");
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.refresh_file_source(revision, &mut source, ElapsedTick(1))
            .unwrap();
    }
    unknown(&check(&mut adapter, 1, &candidate(b"ok")));
    assert_eq!(original_grant.remaining_queries(), 2);
    let (fresh, fresh_grant) = cached_disclosure(&supervisor, DISCLOSURE + 1, 3, &cache);
    adapter.set_precheck(Some(fresh)).unwrap();
    known(
        &check(&mut adapter, 1, &candidate(b"ok")),
        FilePrecheckOutcome::DeniedByPolicy,
        1,
        DISCLOSURE + 1,
    );
    let cache_after_change = cache.status().unwrap();
    assert_eq!(cache_after_change.evaluations, 2);
    assert_eq!(cache_after_change.hits, 0);
    assert_eq!(cache_after_change.invalidations, 1);

    std::fs::remove_file(source_path(&root)).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(
            host.refresh_file_source(revision, &mut source, ElapsedTick(1))
                .is_err()
        );
    }
    unknown(&check(&mut adapter, 1, &candidate(b"ok")));
    assert_eq!(fresh_grant.remaining_queries(), 2);
    let revision = supervisor.host().unwrap().revision();
    supervisor
        .set_snapshot(revision, Some(snapshot(b"ok")))
        .unwrap();
    assert!(
        supervisor
            .disclose_exact_precheck(revision, DISCLOSURE + 2, 1)
            .is_err()
    );
    let failed_image = files(&root);
    unknown(&check(&mut adapter, 1, &candidate(b"ok")));
    assert_eq!(files(&root), failed_image);

    // Only another actual captured source can provide a new eligible grant.
    write_source(&root, 3, b"ok");
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.refresh_file_source(revision, &mut source, ElapsedTick(1))
            .unwrap();
    }
    let (restored, restored_grant) = cached_disclosure(&supervisor, DISCLOSURE + 2, 1, &cache);
    adapter.set_precheck(Some(restored)).unwrap();
    known(
        &check(&mut adapter, 1, &candidate(b"ok")),
        FilePrecheckOutcome::PrecheckPassed,
        1,
        DISCLOSURE + 2,
    );
    assert_eq!(restored_grant.remaining_queries(), 0);
    assert_eq!(cache.status().unwrap().evaluations, 3);
    assert_eq!(cache.status().unwrap().hits, 0);
    assert_eq!(cache.status().unwrap().invalidations, 2);
}

#[test]
fn expired_source_or_dropped_owner_stays_unavailable_without_spending_disclosure_queries() {
    for drop_owner in [false, true] {
        let root = Directory::new();
        let (actor, mut supervisor, _) = owner(&root);
        let mut adapter = tower(actor);
        let (port, grant) = disclose(&supervisor, DISCLOSURE, 2);
        adapter.set_precheck(Some(port)).unwrap();
        known(
            &check(&mut adapter, 1, &candidate(b"ok")),
            FilePrecheckOutcome::PrecheckPassed,
            1,
            DISCLOSURE,
        );
        if drop_owner {
            drop(supervisor);
            unknown(&check(&mut adapter, 1, &candidate(b"ok")));
            let (host, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
            assert_eq!(host.retained_requests(), 0);
            assert_eq!(host.inspect().executions, 0);
            // Neither the old adapter nor its retained custodian holds the lock
            // or becomes a port to this distinct recovered owner.
            unknown(&check(&mut adapter, 1, &candidate(b"ok")));
        } else {
            {
                let mut host = supervisor.host_mut().unwrap();
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(7)).unwrap();
            }
            unknown(&check(&mut adapter, 1, &candidate(b"ok")));
            let revision = supervisor.host().unwrap().revision();
            assert!(
                supervisor
                    .disclose_exact_precheck(revision, DISCLOSURE + 1, 1)
                    .is_err()
            );
        }
        assert_eq!(grant.remaining_queries(), 1);
    }
}

#[test]
fn precheck_wire_retains_exact_query_basis_and_review_required_without_private_policy_or_source() {
    let root = Directory::new();
    let (actor, supervisor, _) = owner(&root);
    let mut adapter = tower(actor);
    let (port, grant) = disclose(&supervisor, DISCLOSURE, 3);
    adapter.set_precheck(Some(port)).unwrap();
    let mut responses = vec![check(&mut adapter, u64::MAX, &candidate(b"ok"))];
    responses.push(check(&mut adapter, 2, &candidate(b"blocked")));
    let mut stale = candidate(b"ok");
    stale.expected_policy_epoch = 99;
    let stale_response = check(&mut adapter, 3, &stale);
    assert_eq!(
        observation(&stale_response).result,
        Ok(ActorKnowledge::Stale {
            generation: 99,
            current: 0
        })
    );
    responses.push(stale_response);
    grant.revoke();
    responses.push(check(&mut adapter, 4, &candidate(b"ok")));
    let image = files(&root);
    let expected: BTreeSet<&str> = ENVELOPE_FIELDS.into_iter().collect();
    for response in responses {
        no_global_claims(&response);
        let native = observation(&response).encode().unwrap();
        let encoded = response.to_json().unwrap();
        let parsed = strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
        let outer = parsed.as_object().unwrap();
        let envelope = outer["envelope"].as_object().unwrap();
        assert_eq!(
            envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            expected
        );
        let result = outer["result"].as_object().unwrap();
        assert_eq!(
            result["value"],
            strict_json::parse(&native, KNOWLEDGE_JSON_LIMITS).unwrap()
        );
        if matches!(
            observation(&response).result,
            Ok(ActorKnowledge::Known {
                value: FilePrecheckOutcome::PrecheckPassed,
                ..
            })
        ) {
            assert!(encoded.contains("\"review_required\":true"));
            assert!(encoded.contains(&format!("\"{}\"", u64::MAX)));
        }
        assert!(encoded.contains("\"precheck\":true"));
        for private in [
            "SECRET-PRECHECK-SOURCE-BYTES",
            "ExactValue",
            "PayloadAtMost",
            "original-alpha",
            "original-beta",
            "precheck-source.json",
            "\"reserved\"",
            "\"available\"",
        ] {
            assert!(
                !encoded.contains(private),
                "private precheck detail leaked: {private}"
            );
        }
    }
    assert_eq!(files(&root), image);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
}
