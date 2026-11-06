//! Grant-local discovery through the original source-backed publication owner.
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
        PrecheckWireError,
        cache::PrecheckCacheLimits,
        discovery::{PrecheckCapabilitiesResponse, decode_capabilities_response},
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    actor::{ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown},
    actor_wire::WireError,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource},
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
use fa_reference::typestate_cx::PurposeTag;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use support::{Directory, Port, Supervisor};

const REGISTRATION: u64 = 41;
const DISCLOSURE: u64 = 81;

fn profile() -> FileOversightProfile {
    let mut profile = support::profile();
    profile.delivery.policy = Policy::new(
        1,
        vec![Predicate::ExactValue {
            key: 7,
            value: b"ok".to_vec(),
        }],
    )
    .unwrap();
    profile
}

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]),
    }
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
    let path = root.0.join("capabilities-source.json");
    let evidence = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: profile.delivery.scope,
        },
        snapshot(),
        BTreeMap::from([
            ("alpha".to_owned(), b"SECRET-DISCOVERY-INPUT".to_vec()),
            ("beta".to_owned(), b"SECRET-DISCOVERY-INPUT".to_vec()),
        ]),
    )
    .unwrap();
    std::fs::write(&path, evidence.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path, 51, profile.delivery.scope, 1_048_576).unwrap();
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

fn reader() -> TowerAccess {
    TowerAccess::new(scope(), false, Vec::new(), 0)
}

fn disclose(
    supervisor: &Supervisor,
    queries: usize,
) -> (FileActorPrecheckPort, FilePrecheckDisclosure) {
    supervisor
        .disclose_exact_precheck(supervisor.host().unwrap().revision(), DISCLOSURE, queries)
        .unwrap()
}

fn request(adapter: &PublicationTower, query: u64, expected_policy_epoch: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "discover",
        adapter.mount_address().clone(),
        Verb::Capabilities,
    );
    request.arguments = ReadArguments::Capabilities {
        query,
        expected_policy_epoch,
    };
    request
}

fn discover(adapter: &mut PublicationTower, query: u64, epoch: u64) -> PublicationResponse {
    let request = request(adapter, query, epoch);
    adapter.dispatch(&reader(), request)
}

fn native(response: &PublicationResponse) -> &PrecheckCapabilitiesResponse {
    match &response.result {
        PublicationResult::Capabilities(value) => value,
        other => panic!("expected native discovery, got {other:?}"),
    }
}

fn known(
    response: &PublicationResponse,
    query: u64,
    remaining: usize,
    cache: bool,
) -> FilePrecheckBasis {
    let observation = native(response);
    assert_eq!(observation.query, Some(query));
    let Ok(ActorKnowledge::Known { value, basis }) = &observation.result else {
        panic!("expected Known discovery: {response:?}");
    };
    assert_eq!(value.remaining_queries(), remaining);
    assert_eq!(value.witness_cache_enabled(), cache);
    assert!(!value.policy_contents_available());
    assert_eq!(basis.disclosure, DISCLOSURE);
    assert_eq!(basis.query, query);
    assert_eq!(basis.policy_epoch, 0);
    assert_eq!(response.envelope.reason_code, None);
    no_global_claims(response);
    *basis
}

fn unknown(response: &PublicationResponse) {
    assert_eq!(
        native(response).result,
        Ok(ActorKnowledge::Unknown {
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

fn candidate() -> ActorProposal {
    ActorProposal {
        target: profile().delivery.target,
        payload: b"ok".to_vec(),
        units: 8,
        deadline: ElapsedTick(100),
        expected_policy_epoch: 0,
    }
}

fn submit(adapter: &mut PublicationTower) -> PublicationResponse {
    let mut request = TowerRequest::new("submit", adapter.mount_address().clone(), Verb::Propose);
    request.arguments = ReadArguments::Propose {
        request: 901,
        proposal: candidate(),
    };
    request.idempotency_key = Some("901".to_owned());
    let access = TowerAccess::new(scope(), false, vec![PurposeTag::Permit], 0);
    adapter.dispatch(&access, request)
}

#[test]
fn original_disclosure_is_required_and_discovery_preserves_source_state_and_review() {
    let root = Directory::new();
    let (actor, mut supervisor, source) = owner(&root);
    let mut adapter = tower(actor);
    refused(
        &discover(&mut adapter, 901, 0),
        ReasonCode::CapabilityUnsupported,
    );
    let (port, grant) = disclose(&supervisor, 3);
    adapter.set_precheck(Some(port)).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let image = files(&root);
    let reads = source.status().read_attempts;
    assert_eq!(
        known(&discover(&mut adapter, 901, 0), 901, 2, false).observed_at,
        ElapsedTick(1)
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(files(&root), image);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);

    // Discovery cannot supply the admission snapshot or satisfy normal review.
    let PublicationResult::Request(attempted) = submit(&mut adapter).result else {
        panic!("native submission");
    };
    assert_eq!(attempted.result, Err(WireError::Unavailable));
    assert_eq!(files(&root), image);
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    known(&discover(&mut adapter, 901, 0), 901, 1, false);
    let PublicationResult::Request(admitted) = submit(&mut adapter).result else {
        panic!("native submission");
    };
    assert_eq!(
        admitted.result,
        Ok(ActorKnowledge::Pending { request: 901 })
    );
    let inspection = supervisor.host().unwrap().inspect();
    assert_eq!(inspection.control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(inspection.executions, 0);
    assert_eq!(inspection.control.ledger.available, 100);
    assert_eq!(inspection.control.ledger.reserved, 0);
    assert!(inspection.control.decisions.is_empty());
    unknown(&discover(&mut adapter, 901, 0));
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn discovery_checks_clones_and_repeated_labels_share_one_allowance_without_cache_work() {
    let root = Directory::new();
    let (actor, supervisor, source) = owner(&root);
    let cache = supervisor
        .precheck_cache(PrecheckCacheLimits::default())
        .unwrap();
    let (port, grant) = supervisor
        .disclose_exact_precheck_cached(
            supervisor.host().unwrap().revision(),
            DISCLOSURE,
            4,
            &cache,
        )
        .unwrap();
    let mut adapter = tower(actor.clone());
    adapter.set_precheck(Some(port.clone())).unwrap();
    let mut reconnected = tower(actor);
    reconnected.set_precheck(Some(port.clone())).unwrap();
    let image = files(&root);
    let reads = source.status().read_attempts;
    let original_cache = cache.status().unwrap();
    let mut call = request(&adapter, u64::MAX, 0);
    call.idempotency_key = Some("same-read-label".to_owned());
    known(
        &adapter.dispatch(&reader(), call.clone()),
        u64::MAX,
        3,
        true,
    );
    known(&reconnected.dispatch(&reader(), call), u64::MAX, 2, true);
    assert_eq!(cache.status().unwrap(), original_cache);
    assert!(matches!(
        port.check(u64::MAX, &candidate()).unwrap(),
        ActorKnowledge::Known {
            value: FilePrecheckOutcome::PrecheckPassed,
            ..
        }
    ));
    assert_eq!(cache.status().unwrap().evaluations, 1);
    known(&discover(&mut adapter, u64::MAX, 0), u64::MAX, 0, true);
    assert_eq!(grant.remaining_queries(), 0);
    let exhausted = discover(&mut reconnected, 5, 0);
    assert_eq!(native(&exhausted).result, Err(PrecheckWireError::Capacity));
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    reconnected.set_precheck(Some(port)).unwrap();
    assert_eq!(
        native(&discover(&mut reconnected, 6, 0)).result,
        Err(PrecheckWireError::Capacity)
    );
    assert_eq!(cache.status().unwrap().evaluations, 1);
    assert_eq!(cache.status().unwrap().hits, 0);
    assert_eq!(files(&root), image);
    assert_eq!(source.status().read_attempts, reads);
}

#[test]
fn caller_mount_and_shape_refusals_do_not_spend_but_entered_stale_policy_does() {
    let root = Directory::new();
    let (actor, supervisor, _) = owner(&root);
    let mut adapter = tower(actor);
    let (port, grant) = disclose(&supervisor, 3);
    adapter.set_precheck(Some(port)).unwrap();
    let mut invalid = vec![(request(&adapter, 0, 0), ReasonCode::IntegrityFailure)];
    let mut wrong_shape = request(&adapter, 1, 0);
    wrong_shape.arguments = ReadArguments::None;
    invalid.push((wrong_shape, ReasonCode::IntegrityFailure));
    let mut wrong_mount = request(&adapter, 1, 0);
    wrong_mount.address.id = "999".to_owned();
    invalid.push((wrong_mount, ReasonCode::MissingRequiredObservation));
    let mut wrong_noun = request(&adapter, 1, 0);
    wrong_noun.address = adapter.request_address(901).unwrap();
    invalid.push((wrong_noun, ReasonCode::MissingRequiredObservation));
    let mut old_registration = request(&adapter, 1, 0);
    old_registration.address.generation = Some(REGISTRATION - 1);
    invalid.push((old_registration, ReasonCode::StaleGeneration));
    for (call, reason) in invalid {
        refused(&adapter.dispatch(&reader(), call), reason);
        assert_eq!(grant.remaining_queries(), 3);
    }
    let mut foreign_scope = scope();
    foreign_scope.caller_tenant = "other".to_owned();
    let foreign = TowerAccess::new(foreign_scope, false, Vec::new(), 0);
    let call = request(&adapter, 1, 0);
    let denied = adapter.dispatch(&foreign, call);
    assert!(matches!(
        denied.result,
        PublicationResult::Refused(Knowledge::Withheld { .. })
    ));
    assert_eq!(grant.remaining_queries(), 3);
    let stale = discover(&mut adapter, 1, 99);
    assert_eq!(
        native(&stale).result,
        Ok(ActorKnowledge::Stale {
            generation: 99,
            current: 0
        })
    );
    assert_eq!(
        stale.envelope.reason_code,
        Some(ReasonCode::StaleGeneration)
    );
    assert_eq!(grant.remaining_queries(), 2);
    known(&discover(&mut adapter, 1, 0), 1, 1, false);
}

#[test]
fn live_revocation_withholds_a_previously_known_result_without_reopening_the_grant() {
    let root = Directory::new();
    let (actor, supervisor, source) = owner(&root);
    let mut adapter = tower(actor.clone());
    let (port, grant) = disclose(&supervisor, 3);
    adapter.set_precheck(Some(port.clone())).unwrap();
    known(&discover(&mut adapter, 1, 0), 1, 2, false);
    assert_eq!(grant.remaining_queries(), 2);
    grant.revoke();
    let image = files(&root);
    let reads = source.status().read_attempts;
    for mut facade in [adapter, tower(actor)] {
        facade.set_precheck(Some(port.clone())).unwrap();
        let response = discover(&mut facade, 1, 0);
        assert_eq!(
            native(&response).result,
            Ok(ActorKnowledge::Withheld {
                authority_required: "exact_policy_disclosure",
            })
        );
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
        no_global_claims(&response);
    }
    assert_eq!(files(&root), image);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
}

#[test]
fn borrowed_stale_closed_and_recovered_owners_never_reuse_previous_discovery() {
    let root = Directory::new();
    let (actor, mut supervisor, _) = owner(&root);
    let mut adapter = tower(actor);
    let (port, grant) = disclose(&supervisor, 4);
    adapter.set_precheck(Some(port)).unwrap();
    known(&discover(&mut adapter, 1, 0), 1, 3, false);
    {
        let mut host = supervisor.host_mut().unwrap();
        unknown(&discover(&mut adapter, 1, 0));
        assert_eq!(grant.remaining_queries(), 3);
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    unknown(&discover(&mut adapter, 1, 0));
    assert_eq!(grant.remaining_queries(), 3);
    drop(supervisor);
    unknown(&discover(&mut adapter, 1, 0));
    let (recovered, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    unknown(&discover(&mut adapter, 1, 0));
    assert_eq!(recovered.retained_requests(), 0);
    assert_eq!(grant.remaining_queries(), 3);
    grant.revoke();
    let withheld = discover(&mut adapter, 1, 0);
    assert_eq!(
        native(&withheld).result,
        Ok(ActorKnowledge::Withheld {
            authority_required: "exact_policy_disclosure",
        })
    );
    assert_eq!(
        withheld.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
}

#[test]
fn same_numeric_scope_foreign_owner_cannot_supply_or_preserve_an_installed_disclosure() {
    let root = Directory::new();
    let foreign_root = Directory::new();
    let (actor, supervisor, _) = owner(&root);
    let (_, foreign_supervisor, _) = owner(&foreign_root);
    let mut adapter = tower(actor);
    let (original, grant) = disclose(&supervisor, 3);
    let (foreign, foreign_grant) = disclose(&foreign_supervisor, 3);
    adapter.set_precheck(Some(original.clone())).unwrap();
    known(&discover(&mut adapter, 1, 0), 1, 2, false);
    assert!(adapter.set_precheck(Some(foreign)).is_err());
    refused(
        &discover(&mut adapter, 1, 0),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_queries(), 3);
    adapter.set_precheck(Some(original)).unwrap();
    known(&discover(&mut adapter, 1, 0), 1, 1, false);
    adapter.set_precheck(None).unwrap();
    refused(
        &discover(&mut adapter, 1, 0),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn byte_exchange_decodes_completely_before_charging_and_preserves_native_discovery() {
    let root = Directory::new();
    let (actor, supervisor, source) = owner(&root);
    let mut adapter = tower(actor);
    let (port, grant) = disclose(&supervisor, 3);
    adapter.set_precheck(Some(port)).unwrap();
    let document = encode_request(&request(&adapter, u64::MAX, 0)).unwrap();
    let image = files(&root);
    let reads = source.status().read_attempts;
    for invalid in [
        document.replacen('{', "{\"scope\":\"self-granted\",", 1),
        format!("{document}{{}}"),
        document.replace("\"query\":\"18446744073709551615\"", "\"query\":\"0\""),
        document.replace(
            "\"expected_policy_epoch\":\"0\"",
            "\"expected_policy_epoch\":0",
        ),
    ] {
        let response = adapter.exchange(&reader(), invalid.as_bytes());
        refused(&response, ReasonCode::IntegrityFailure);
        assert_eq!(response.envelope.operation_id, "<invalid-request>");
        assert!(!response.envelope.retry_safe);
        assert_eq!(grant.remaining_queries(), 3);
    }
    for remaining in [2, 1] {
        let response = adapter.exchange(&reader(), document.as_bytes());
        known(&response, u64::MAX, remaining, false);
        let original = native(&response).encode().unwrap();
        assert_eq!(
            decode_capabilities_response(&original, u64::MAX, 0).unwrap(),
            *native(&response)
        );
    }
    assert_eq!(files(&root), image);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
}

#[test]
fn encoder_preserves_original_discovery_schema_basis_limits_and_knowledge_variants() {
    let root = Directory::new();
    let (actor, supervisor, _) = owner(&root);
    let mut adapter = tower(actor);
    let (port, grant) = disclose(&supervisor, 2);
    adapter.set_precheck(Some(port)).unwrap();
    let mut responses = vec![discover(&mut adapter, u64::MAX, 0)];
    responses.push(discover(&mut adapter, 2, 99));
    responses.push(discover(&mut adapter, 3, 0));
    grant.revoke();
    responses.push(discover(&mut adapter, 4, 0));
    let fields: BTreeSet<&str> = ENVELOPE_FIELDS.into_iter().collect();
    for response in &responses {
        no_global_claims(response);
        let native = native(response);
        let bytes = native.encode().unwrap();
        let encoded = response.to_json().unwrap();
        let parsed = strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
        let outer = parsed.as_object().unwrap();
        let envelope = outer["envelope"].as_object().unwrap();
        assert_eq!(
            envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            fields
        );
        let result = outer["result"].as_object().unwrap();
        assert_eq!(result["kind"].as_str(), Some("capabilities"));
        assert_eq!(
            result["value"],
            strict_json::parse(&bytes, KNOWLEDGE_JSON_LIMITS).unwrap()
        );
        if matches!(native.result, Ok(ActorKnowledge::Known { .. })) {
            assert_eq!(
                decode_capabilities_response(&bytes, u64::MAX, 0).unwrap(),
                *native
            );
            for fact in [
                "\"informational\":true",
                "\"signed_profile\":false",
                "\"effect_authority\":false",
                "\"review_required\":true",
                "\"witness_cache\":\"unavailable\"",
                "\"source_refresh\":\"unavailable\"",
            ] {
                assert!(encoded.contains(fact), "missing native limitation: {fact}");
            }
        }
        for private in [
            "SECRET-DISCOVERY-INPUT",
            "ExactValue",
            "original-alpha",
            "original-beta",
            "capabilities-source.json",
        ] {
            assert!(
                !encoded.contains(private),
                "private detail leaked: {private}"
            );
        }
    }
    // Public response reconstruction cannot bypass the original correlation
    // checks or recast a signed/global assurance profile into valid discovery.
    let mut invalid = responses[0].clone();
    let PublicationResult::Capabilities(native) = &mut invalid.result else {
        unreachable!()
    };
    native.query = Some(12);
    assert_eq!(invalid.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
}
