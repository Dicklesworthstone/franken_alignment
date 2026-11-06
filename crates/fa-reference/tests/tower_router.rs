//! Fixed Tower routing preserves the original native owner and snapshot basis.
//! The publication controls use the real durable source, congress and human key;
//! imported snapshot objects deliberately disagree with live request state.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use std::collections::{BTreeMap, BTreeSet};

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight, source::FileSourcePolicy,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{PrecheckWireError, PrecheckWireResponse},
};
use fa_reference::action::consequence::oversight::{
    actor::{
        ActorOutcome, ActorProposal, BasisSource, Knowledge as ActorKnowledge,
        UnknownReason as ActorUnknown,
    },
    actor_wire::{WireError, WireResponse},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolvedObject};
use fa_reference::event_journal::DomainEventJournal;
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{
    Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
};
use fa_reference::round::{Verdict, commitment};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::router::{RoutedResponse, TowerRouter};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, ReferenceTower, TowerAccess, TowerBuilder,
    TowerInstallError, TowerLimits, TowerRequest, TowerResponse, TowerValue, Verb,
};
use fa_reference::typestate_cx::{PurposeTag, RootCx};
use support::{Directory, Port, Supervisor, observe, oversight, profile, snapshot};

const TENANT: &str = "1";
const SNAPSHOT_DOMAIN: &str = "retained-inspection";
const SNAPSHOT_GENERATION: u64 = 7;
const MOUNT_GENERATION: u64 = 41;
const DISCLOSURE: u64 = 81;
const SOURCE_BYTES: &[u8] = b"PRIVATE-SOURCE-BYTES";

fn imported_basis() -> Basis {
    let declared = ScopedProof::Declared {
        scope_id: 71,
        generation: SNAPSHOT_GENERATION,
    };
    Basis::new(
        BasisIdentity {
            subject_id: 72,
            question_id: 73,
            profile_id: 74,
            semantic_generation: SNAPSHOT_GENERATION,
            policy_epoch: 0,
            model_epoch: 11,
            tokenizer_epoch: 13,
            codec_epoch: 17,
            control_seq: 0,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: declared,
            complete_for_contract: declared,
            semantically_valid: ScopedProof::Unsupported { scope_id: 75 },
            available_for_replay: declared,
        },
    )
}

fn evidence(tenant: &str) -> Address {
    Address::new(
        tenant,
        Noun::Evidence,
        "retained-source",
        Some(SNAPSHOT_GENERATION),
    )
    .unwrap()
}

fn insert(builder: &mut TowerBuilder, mut address: Address) {
    address.generation = Some(SNAPSHOT_GENERATION);
    builder
        .insert_object(
            ResolvedObject {
                plane: address.kind.plane(),
                address,
                summary_digest: 0xdec0_1ded,
                generation: SNAPSHOT_GENERATION,
            },
            imported_basis(),
        )
        .unwrap();
}

fn imported(tenant: &str, decoys: &[Address], units: u64) -> ReferenceTower {
    let mut builder = TowerBuilder::new(
        DomainEventJournal::new_production(tenant, SNAPSHOT_DOMAIN),
        imported_basis(),
        TowerLimits::default(),
        units,
    )
    .unwrap();
    insert(&mut builder, evidence(tenant));
    for address in decoys {
        insert(&mut builder, address.clone());
    }
    builder.build()
}

fn branch() -> ReferenceTower {
    let cx = RootCx::new_nucleus(1, 3, 0).mint_experiment(91).unwrap();
    let journal = DomainEventJournal::new_branch(&cx, TENANT, SNAPSHOT_DOMAIN, 91);
    let mut builder =
        TowerBuilder::new(journal, imported_basis(), TowerLimits::default(), 2).unwrap();
    builder.enable_replay(cx).unwrap();
    insert(&mut builder, evidence(TENANT));
    builder.build()
}

fn publication(port: Port) -> PublicationTower {
    let scope = profile().delivery.scope;
    PublicationTower::new(
        Address::new(
            scope.tenant.to_string(),
            Noun::Run,
            scope.run.to_string(),
            Some(MOUNT_GENERATION),
        )
        .unwrap(),
        port,
        PublicationLimits::default(),
    )
    .unwrap()
}

fn access() -> TowerAccess {
    // The actor can read the explicitly provisioned evidence and use the native
    // intake. It has no operator status and cannot issue either publication key.
    TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        false,
        vec![PurposeTag::Permit],
        0,
    )
}

fn operator(tenant: &str) -> TowerAccess {
    TowerAccess::new(CallerScope::full_tenant_access(tenant), true, Vec::new(), 0)
}

fn reader() -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        false,
        Vec::new(),
        0,
    )
}

fn submit(address: &Address, request: u64, proposal: &ActorProposal) -> TowerRequest {
    let mut call = TowerRequest::new("native-submit", address.clone(), Verb::Propose);
    call.arguments = ReadArguments::Propose {
        request,
        proposal: proposal.clone(),
    };
    call.idempotency_key = Some(request.to_string());
    call
}

fn precheck(address: &Address, query: u64, proposal: &ActorProposal) -> TowerRequest {
    let mut call = TowerRequest::new("native-precheck", address.clone(), Verb::Precheck);
    call.arguments = ReadArguments::Precheck {
        query,
        proposal: proposal.clone(),
    };
    call
}

fn get(address: &Address) -> TowerRequest {
    TowerRequest::new("observe", address.clone(), Verb::Get)
}

fn reference(response: &RoutedResponse) -> &TowerResponse {
    match response {
        RoutedResponse::Reference(value) => value,
        RoutedResponse::Publication(_) => panic!("expected original snapshot response"),
    }
}

fn live(response: &RoutedResponse) -> &PublicationResponse {
    let RoutedResponse::Publication(value) = response else {
        panic!("a native route must never fall back to the snapshot");
    };
    no_global_native_claims(value);
    assert_eq!(response.to_json().unwrap(), value.to_json().unwrap());
    value
}

fn request_result(response: &RoutedResponse) -> &WireResponse {
    match &live(response).result {
        PublicationResult::Request(value) => value,
        other => panic!("expected original native request observation: {other:?}"),
    }
}

fn precheck_result(response: &RoutedResponse) -> &PrecheckWireResponse {
    match &live(response).result {
        PublicationResult::Precheck(value) => value,
        other => panic!("expected original native precheck observation: {other:?}"),
    }
}

fn no_global_native_claims(response: &PublicationResponse) {
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

fn known_evidence(response: &RoutedResponse, tenant: &str) {
    let original = reference(response);
    let Knowledge::Known {
        value: TowerValue::Object(object),
        basis,
    } = &original.result
    else {
        panic!("expected the imported evidence object, got {original:?}");
    };
    assert_eq!(object.address, evidence(tenant));
    assert_eq!(*basis, imported_basis());
    assert_eq!(response.to_json().unwrap(), original.to_json().unwrap());
}

fn parse(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, KNOWLEDGE_JSON_LIMITS).unwrap()
}

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value
        .get(name)
        .unwrap_or_else(|| panic!("missing field {name}"))
}

fn assert_native_json(response: &RoutedResponse) {
    let encoded = response.to_json().unwrap();
    let document = parse(encoded.as_bytes());
    let Json::Object(envelope) = field(&document, "envelope") else {
        panic!("canonical envelope object");
    };
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect::<BTreeSet<_>>()
    );
    let result = field(&document, "result");
    assert_eq!(field(result, "kind").as_str(), Some("publication-request"));
    assert_eq!(
        field(result, "value"),
        &parse(&request_result(response).encode())
    );
}

fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn captured_source() -> EvidenceSnapshot {
    EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: profile().delivery.scope,
        },
        snapshot(),
        BTreeMap::from([
            ("alpha".to_owned(), SOURCE_BYTES.to_vec()),
            ("beta".to_owned(), SOURCE_BYTES.to_vec()),
        ]),
    )
    .unwrap()
}

fn source_owner(
    root: &Directory,
) -> (
    Port,
    Supervisor,
    FileHumanReviewer,
    ActorProposal,
    FileEvidenceSource,
) {
    let profile = profile();
    let (mut host, reviewer) = FileOversight::create(root.store(), profile.clone()).unwrap();
    host.enable_file_source(
        host.revision(),
        FileSourcePolicy {
            source: StateSource {
                scope: profile.delivery.scope,
                source: 51,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(50).unwrap(),
        },
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let captured = captured_source();
    let path = root.0.join("source.json");
    std::fs::write(&path, captured.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path, 51, profile.delivery.scope, 1_048_576).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (port, supervisor) = host.into_actor_gateway();
    (port, supervisor, reviewer, proposal, source)
}

fn ready_from_source(
    supervisor: &mut Supervisor,
    reviewer: &FileHumanReviewer,
    request: u64,
) -> oversight::Keys {
    let mut host = supervisor.host_mut().unwrap();
    let id = support::attempt(&host, request);
    let action = host.request_action(request).unwrap().clone();
    // Source-backed owners compare the original complete inputs_for output.
    // A legacy hand-built packet with identical prose would still be different.
    let source = captured_source();
    let inputs = source.inputs_for(&action, &profile().committee).unwrap();
    let root = source.reference_root();
    let revision = host.revision();
    let input_revision = host.input_revision(id).unwrap();
    host.record_inputs(revision, id, input_revision, inputs.clone())
        .unwrap();
    let round = request + 100;
    let revision = host.revision();
    let window = oversight::window(&host);
    host.begin_review(revision, id, round, root, window, snapshot())
        .unwrap();
    for member in oversight::MEMBERS {
        let committed = commitment(
            round,
            member,
            &root,
            Verdict::Allow,
            &oversight::salt(member),
        )
        .unwrap();
        let revision = host.revision();
        host.commit_review(revision, round, member, committed)
            .unwrap();
    }
    let revision = host.revision();
    host.open_reveals(revision, round).unwrap();
    for member in oversight::MEMBERS {
        let revision = host.revision();
        host.reveal_review(
            revision,
            round,
            member,
            Verdict::Allow,
            oversight::salt(member),
        )
        .unwrap();
    }
    let revision = host.revision();
    host.finish_review(revision, round, Some(&inputs), snapshot())
        .unwrap()
        .unwrap();
    let revision = host.revision();
    let automatic = host.authorize(revision, id, &inputs, snapshot()).unwrap();
    let revision = host.revision();
    let human_request = host
        .request_human_approval(revision, request + 1000, id, &inputs, ElapsedTick(31))
        .unwrap();
    let revision = host.revision();
    let human = reviewer
        .approve(&mut host, revision, &human_request)
        .unwrap();
    oversight::Keys {
        action,
        inputs,
        automatic,
        human,
        request: human_request,
    }
}

#[test]
fn evidence_precheck_and_real_two_key_publication_share_one_router_without_sharing_authority() {
    let root = Directory::new();
    let (port, mut supervisor, reviewer, proposal, source) = source_owner(&root);
    let original_port = port.clone();
    let mut adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let request_address = adapter.request_address(70).unwrap();
    let (precheck_port, grant) = supervisor
        .disclose_exact_precheck(supervisor.host().unwrap().revision(), DISCLOSURE, 4)
        .unwrap();
    let precheck_oracle = precheck_port.clone();
    adapter.set_precheck(Some(precheck_port)).unwrap();
    let mut router = TowerRouter::with_publication(imported(TENANT, &[], 2), adapter).unwrap();
    known_evidence(&router.dispatch(&access(), get(&evidence(TENANT))), TENANT);

    let before = canonical(&root);
    let reads = source.status().read_attempts;
    let checked = router.dispatch(&reader(), precheck(&mount, 801, &proposal));
    assert_eq!(
        precheck_result(&checked).result,
        precheck_oracle
            .check(801, &proposal)
            .map_err(PrecheckWireError::from)
    );
    let Ok(ActorKnowledge::Known { value, basis }) = &precheck_result(&checked).result else {
        panic!("source-backed native precheck");
    };
    assert_eq!(*value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!(
        (basis.disclosure, basis.query, basis.policy_epoch),
        (DISCLOSURE, 801, 0)
    );
    assert_eq!(basis.observed_at, ElapsedTick(1));
    let mut forbidden = proposal.clone();
    forbidden.target.object += 1;
    assert!(matches!(
        precheck_result(&router.dispatch(&reader(), precheck(&mount, 802, &forbidden))).result,
        Ok(ActorKnowledge::Known {
            value: FilePrecheckOutcome::DeniedByPolicy,
            ..
        })
    ));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(canonical(&root), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);

    // Passing exact policy grants no admission snapshot, request ticket or key.
    let call = submit(&mount, 70, &proposal);
    assert_eq!(
        request_result(&router.dispatch(&access(), call.clone())).result,
        Err(WireError::Unavailable)
    );
    assert_eq!(canonical(&root), before);
    observe(&mut supervisor);
    assert_eq!(
        request_result(&router.dispatch(&access(), call.clone())).result,
        Ok(ActorKnowledge::Pending { request: 70 })
    );
    let ticket = original_port.submit(70, &proposal).unwrap();
    let keys = ready_from_source(&mut supervisor, &reviewer, 70);
    let id = keys.automatic.attempt();
    assert_ne!(id, 70);
    assert_eq!(
        request_result(&router.dispatch(&reader(), get(&request_address))).result,
        Ok(original_port.poll(&ticket))
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
    }
    let uncertain = router.dispatch(&reader(), get(&request_address));
    assert_eq!(
        request_result(&uncertain).result,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::OutcomeUnknown
        })
    );
    assert_eq!(uncertain.reason_code(), Some(ReasonCode::OutcomeUnknown));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.publish(revision, id).unwrap();
        assert_eq!(host.inspect().payload, proposal.payload);
        assert_eq!(host.inspect().executions, 1);
    }
    assert_eq!(
        request_result(&router.dispatch(&reader(), get(&request_address))).result,
        request_result(&uncertain).result
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(matches!(
            host.reconcile(revision, id).unwrap(),
            Reconciliation::Resolved(_)
        ));
    }
    let completed = router.dispatch(&reader(), get(&request_address));
    assert_eq!(
        request_result(&completed).result,
        Ok(original_port.poll(&ticket))
    );
    let Ok(ActorKnowledge::Known { value, basis }) = &request_result(&completed).result else {
        panic!("confirmed original outcome");
    };
    assert_eq!(*value, ActorOutcome::Executed);
    assert_eq!(
        (basis.request, basis.source),
        (70, BasisSource::ControlLedger)
    );
    assert_native_json(&completed);
    let final_image = canonical(&root);
    let retried = router.dispatch(&access(), call);
    assert_eq!(
        request_result(&retried).result,
        request_result(&completed).result
    );
    assert_eq!(canonical(&root), final_image);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);

    // All native calls leave the snapshot's original two-read budget intact.
    let last_read = router.dispatch(&operator(TENANT), get(&evidence(TENANT)));
    known_evidence(&last_read, TENANT);
    assert!(matches!(
        reference(&last_read).envelope.budgets_remaining,
        Knowledge::Known { value: 0, .. }
    ));
    assert_eq!(
        router
            .dispatch(&operator(TENANT), get(&evidence(TENANT)))
            .reason_code(),
        Some(ReasonCode::BudgetExhausted)
    );
}

#[test]
fn every_live_verb_and_generation_shadows_conflicting_snapshot_objects_without_fallback() {
    let root = Directory::new();
    let (port, supervisor, _reviewer, _proposal) = support::create(&root);
    let adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let request_address = adapter.request_address(9).unwrap();
    let prefix = request_address.id.strip_suffix('9').unwrap();
    let mut reserved = vec![request_address.clone()];
    for suffix in ["", "0", "09", "invalid", "18446744073709551616"] {
        reserved
            .push(Address::new(TENANT, Noun::Attempt, format!("{prefix}{suffix}"), None).unwrap());
    }
    let foreign = Address::new(TENANT, Noun::Attempt, "request-3-999-4-5-9", None).unwrap();
    let mut decoys = vec![mount.clone(), foreign.clone()];
    decoys.extend(reserved.clone());

    // Positive control: these are genuinely present in the imported snapshot.
    let mut baseline = imported(TENANT, &decoys, 32);
    for address in &decoys {
        assert!(matches!(
            baseline
                .dispatch(&operator(TENANT), get(&address.without_generation()))
                .result,
            Knowledge::Known {
                value: TowerValue::Object(_),
                ..
            }
        ));
    }
    let mut unmounted = TowerRouter::new(imported(TENANT, &decoys, 1));
    for address in reserved.iter().chain(std::iter::once(&foreign)) {
        let response = unmounted.dispatch(&access(), get(&address.without_generation()));
        assert_eq!(
            response.reason_code(),
            Some(ReasonCode::MissingRequiredObservation)
        );
        assert!(matches!(
            reference(&response).result,
            Knowledge::Unknown { .. }
        ));
    }
    known_evidence(
        &unmounted.dispatch(&operator(TENANT), get(&evidence(TENANT))),
        TENANT,
    );
    let mut router = TowerRouter::with_publication(imported(TENANT, &decoys, 1), adapter).unwrap();
    let before = canonical(&root);
    for address in std::iter::once(&mount).chain(reserved.iter()) {
        for generation in [
            None,
            Some(MOUNT_GENERATION),
            Some(SNAPSHOT_GENERATION),
            Some(42),
        ] {
            let mut target = address.clone();
            target.generation = generation;
            for verb in Verb::ALL {
                let result = router.dispatch(
                    &access(),
                    TowerRequest::new("shadowed-native-route", target.clone(), verb),
                );
                let native = live(&result);
                assert!(native.envelope.reason_code.is_some());
                if ![Verb::Get, Verb::Propose, Verb::Precheck, Verb::Capabilities].contains(&verb) {
                    assert_eq!(
                        native.envelope.reason_code,
                        Some(ReasonCode::CapabilityUnsupported)
                    );
                }
            }
        }
    }
    let wrong_scope = router.dispatch(&access(), get(&foreign));
    assert_eq!(
        wrong_scope.reason_code(),
        Some(ReasonCode::MissingRequiredObservation)
    );
    assert!(matches!(
        reference(&wrong_scope).result,
        Knowledge::Unknown { .. }
    ));
    assert_eq!(canonical(&root), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    let available = router.dispatch(&operator(TENANT), get(&evidence(TENANT)));
    known_evidence(&available, TENANT);
    assert!(matches!(
        reference(&available).envelope.budgets_remaining,
        Knowledge::Known { value: 0, .. }
    ));
}

#[test]
fn denials_and_malformed_requests_hide_route_presence_and_preserve_native_admission() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = support::create(&root);
    let adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let ticket_address = adapter.request_address(71).unwrap();
    let decoys = [mount.clone(), ticket_address.clone()];
    let mut live_router =
        TowerRouter::with_publication(imported(TENANT, &decoys, 1), adapter).unwrap();
    let mut snapshot_router = TowerRouter::new(imported(TENANT, &decoys, 1));
    observe(&mut supervisor);
    let before = canonical(&root);
    let mut no_plane = CallerScope::full_tenant_access(TENANT);
    no_plane.allowed_planes.clear();
    let mut no_authority = CallerScope::full_tenant_access(TENANT);
    no_authority.granted_authorities.clear();
    let denials = [
        TowerAccess::new(
            CallerScope::full_tenant_access("other"),
            true,
            vec![PurposeTag::Permit],
            1,
        ),
        TowerAccess::new(no_plane, true, vec![PurposeTag::Permit], 1),
        TowerAccess::new(no_authority, true, vec![PurposeTag::Permit], 1),
    ];
    for denied in &denials {
        for request in [
            submit(&mount, 71, &proposal),
            precheck(&mount, 1, &proposal),
            get(&ticket_address),
            get(&evidence(TENANT)),
        ] {
            let first = live_router.dispatch(denied, request.clone());
            let second = snapshot_router.dispatch(denied, request);
            assert_eq!(first, second);
            assert_eq!(first.to_json().unwrap(), second.to_json().unwrap());
            assert_eq!(first.reason_code(), Some(ReasonCode::DeniedByPolicy));
            assert!(matches!(
                reference(&first).result,
                Knowledge::Withheld { .. }
            ));
        }
    }
    let original = submit(&mount, 71, &proposal);
    let mut invalid = Vec::new();
    for operation in [String::new(), "x".repeat(129)] {
        let mut request = original.clone();
        request.operation_id = operation;
        invalid.push(request);
    }
    for key in [String::new(), "9".repeat(129)] {
        let mut request = original.clone();
        request.idempotency_key = Some(key);
        invalid.push(request);
    }
    for id in ["bad/id".to_owned(), "x".repeat(257)] {
        let mut request = original.clone();
        request.address.id = id;
        invalid.push(request);
    }
    for request in invalid {
        let first = live_router.dispatch(&access(), request.clone());
        let second = snapshot_router.dispatch(&access(), request);
        assert_eq!(first, second);
        assert_eq!(first.to_json().unwrap(), second.to_json().unwrap());
        assert_eq!(first.reason_code(), Some(ReasonCode::IntegrityFailure));
        assert_eq!(reference(&first).envelope.operation_id, "<invalid-request>");
    }
    assert_eq!(canonical(&root), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    // The rejected requests did not consume the original one-use snapshot.
    assert_eq!(
        request_result(&live_router.dispatch(&access(), original)).result,
        Ok(ActorKnowledge::Pending { request: 71 })
    );
    for router in [&mut live_router, &mut snapshot_router] {
        let last = router.dispatch(&operator(TENANT), get(&evidence(TENANT)));
        known_evidence(&last, TENANT);
        assert!(matches!(
            reference(&last).envelope.budgets_remaining,
            Knowledge::Known { value: 0, .. }
        ));
    }
}

#[test]
fn native_generation_key_and_purpose_refusals_do_not_consume_the_original_snapshot() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = support::create(&root);
    let adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let mut router =
        TowerRouter::with_publication(imported(TENANT, &[mount.clone()], 1), adapter).unwrap();
    observe(&mut supervisor);
    let before = canonical(&root);
    let valid = submit(&mount, 72, &proposal);
    for generation in [None, Some(MOUNT_GENERATION - 1), Some(MOUNT_GENERATION + 1)] {
        let mut stale = valid.clone();
        stale.address.generation = generation;
        let response = router.dispatch(&access(), stale);
        assert_eq!(
            live(&response).envelope.reason_code,
            Some(ReasonCode::StaleGeneration)
        );
        assert_eq!(
            live(&response).envelope.would_change_this,
            vec![mount.clone()]
        );
    }
    for key in ["072", "73"] {
        let mut changed = valid.clone();
        changed.idempotency_key = Some(key.to_owned());
        let response = router.dispatch(&access(), changed);
        assert_eq!(
            live(&response).envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
    }
    let denied = router.dispatch(&reader(), valid.clone());
    assert_eq!(
        live(&denied).envelope.reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    assert_eq!(canonical(&root), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    assert_eq!(
        request_result(&router.dispatch(&access(), valid)).result,
        Ok(ActorKnowledge::Pending { request: 72 })
    );
    known_evidence(
        &router.dispatch(&operator(TENANT), get(&evidence(TENANT))),
        TENANT,
    );
}

#[test]
fn withdrawn_disclosure_and_closed_owner_keep_native_routes_reserved() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal, _source) = source_owner(&root);
    let mut adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let request_address = adapter.request_address(73).unwrap();
    let (port, grant) = supervisor
        .disclose_exact_precheck(supervisor.host().unwrap().revision(), DISCLOSURE, 2)
        .unwrap();
    adapter.set_precheck(Some(port)).unwrap();
    let mut router = TowerRouter::with_publication(
        imported(TENANT, &[mount.clone(), request_address.clone()], 1),
        adapter,
    )
    .unwrap();
    assert!(matches!(
        precheck_result(&router.dispatch(&reader(), precheck(&mount, 1, &proposal))).result,
        Ok(ActorKnowledge::Known {
            value: FilePrecheckOutcome::PrecheckPassed,
            ..
        })
    ));
    drop(grant);
    let revoked = router.dispatch(&reader(), precheck(&mount, 1, &proposal));
    assert_eq!(revoked.reason_code(), Some(ReasonCode::DeniedByPolicy));
    assert!(matches!(
        precheck_result(&revoked).result,
        Ok(ActorKnowledge::Withheld { .. })
    ));
    observe(&mut supervisor);
    let call = submit(&mount, 73, &proposal);
    assert_eq!(
        request_result(&router.dispatch(&access(), call.clone())).result,
        Ok(ActorKnowledge::Pending { request: 73 })
    );
    drop(supervisor);
    let before = canonical(&root);
    let missing_owner = router.dispatch(&reader(), get(&request_address));
    assert_eq!(
        missing_owner.reason_code(),
        Some(ReasonCode::LowerLayerUnverified)
    );
    assert_eq!(
        request_result(&missing_owner).result,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::ControllerUnavailable
        })
    );
    let retry = router.dispatch(&access(), call);
    assert_eq!(retry.reason_code(), Some(ReasonCode::LowerLayerUnverified));
    assert_eq!(request_result(&retry).result, Err(WireError::Unavailable));
    let run = router.dispatch(&reader(), get(&mount));
    assert_eq!(
        live(&run).envelope.reason_code,
        Some(ReasonCode::MissingRequiredObservation)
    );
    assert_eq!(canonical(&root), before);
    known_evidence(
        &router.dispatch(&operator(TENANT), get(&evidence(TENANT))),
        TENANT,
    );
}

#[test]
fn branch_and_tenant_boundaries_refuse_live_mounts_without_disabling_snapshot_reads() {
    let root = Directory::new();
    let (port, mut supervisor, _reviewer, proposal) = support::create(&root);
    let before = canonical(&root);
    assert_eq!(
        TowerRouter::with_publication(branch(), publication(port.clone())).err(),
        Some(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        TowerRouter::with_publication(imported("other", &[], 1), publication(port.clone())).err(),
        Some(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(canonical(&root), before);

    let mut branch_router = TowerRouter::new(branch());
    assert!(branch_router.publication_address().is_none());
    assert_eq!(
        branch_router.publication_request_address(1),
        Err(TowerInstallError::MissingSource)
    );
    known_evidence(
        &branch_router.dispatch(&operator(TENANT), get(&evidence(TENANT))),
        TENANT,
    );
    let tail = TowerRequest::new(
        "branch-tail",
        branch_router.snapshot_address().clone(),
        Verb::Tail,
    );
    let response = branch_router.dispatch(&operator(TENANT), tail);
    assert!(matches!(
        reference(&response).result,
        Knowledge::Known {
            value: TowerValue::Tail(_),
            ..
        }
    ));
    assert_eq!(
        reference(&response).envelope.control_seq,
        Knowledge::known(0, imported_basis())
    );
    let mut other_router = TowerRouter::new(imported("other", &[], 1));
    known_evidence(
        &other_router.dispatch(&operator("other"), get(&evidence("other"))),
        "other",
    );

    // The identical original port mounts under a production snapshot from its
    // own tenant, without comparing snapshot and registration generations.
    let adapter = publication(port);
    let mount = adapter.mount_address().clone();
    let mut router = TowerRouter::with_publication(imported(TENANT, &[], 1), adapter).unwrap();
    observe(&mut supervisor);
    assert_eq!(
        request_result(&router.dispatch(&access(), submit(&mount, 74, &proposal))).result,
        Ok(ActorKnowledge::Pending { request: 74 })
    );
}
