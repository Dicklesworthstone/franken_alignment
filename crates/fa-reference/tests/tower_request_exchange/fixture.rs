//! The existing durable source/approval fixture, shared by the wire regressions.
//! No fake effect owner, synthetic outcome or Tower-managed approval is used.

use std::collections::{BTreeMap, BTreeSet};

use super::support::{self, Directory, Port, Supervisor, oversight, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight, source::FileSourcePolicy,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::PrecheckWireResponse;
use fa_reference::action::consequence::oversight::{
    actor::ActorProposal,
    actor_wire::WireResponse,
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
use fa_reference::tower::router::RoutedResponse;
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReferenceTower, TowerAccess, TowerBuilder, TowerLimits, TowerResponse,
    TowerValue,
};
use fa_reference::typestate_cx::PurposeTag;

pub(super) const TENANT: &str = "1";
const SNAPSHOT_DOMAIN: &str = "retained-inspection";
const SNAPSHOT_GENERATION: u64 = 7;
const MOUNT_GENERATION: u64 = 41;
const SOURCE_BYTES: &[u8] = b"PRIVATE-SOURCE-BYTES";

pub(super) fn imported_basis() -> Basis {
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

pub(super) fn evidence(tenant: &str) -> Address {
    Address::new(
        tenant,
        Noun::Evidence,
        "retained-source",
        Some(SNAPSHOT_GENERATION),
    )
    .unwrap()
}

pub(super) fn insert(builder: &mut TowerBuilder, mut address: Address) {
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

pub(super) fn imported(tenant: &str, decoys: &[Address], units: u64) -> ReferenceTower {
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

pub(super) fn publication(port: Port) -> PublicationTower {
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

pub(super) fn access() -> TowerAccess {
    // The actor can read the explicitly provisioned evidence and use the native
    // intake. It has no operator status and cannot issue either publication key.
    TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        false,
        vec![PurposeTag::Permit],
        0,
    )
}

pub(super) fn operator(tenant: &str) -> TowerAccess {
    TowerAccess::new(CallerScope::full_tenant_access(tenant), true, Vec::new(), 0)
}

pub(super) fn reader() -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        false,
        Vec::new(),
        0,
    )
}

pub(super) fn reference(response: &RoutedResponse) -> &TowerResponse {
    match response {
        RoutedResponse::Reference(value) => value,
        RoutedResponse::Publication(_) => panic!("expected original snapshot response"),
    }
}

pub(super) fn live(response: &RoutedResponse) -> &PublicationResponse {
    let RoutedResponse::Publication(value) = response else {
        panic!("a native route must never fall back to the snapshot");
    };
    no_global_native_claims(value);
    assert_eq!(response.to_json().unwrap(), value.to_json().unwrap());
    value
}

pub(super) fn request_result(response: &RoutedResponse) -> &WireResponse {
    match &live(response).result {
        PublicationResult::Request(value) => value,
        other => panic!("expected original native request observation: {other:?}"),
    }
}

pub(super) fn precheck_result(response: &RoutedResponse) -> &PrecheckWireResponse {
    match &live(response).result {
        PublicationResult::Precheck(value) => value,
        other => panic!("expected original native precheck observation: {other:?}"),
    }
}

pub(super) fn no_global_native_claims(response: &PublicationResponse) {
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

pub(super) fn known_evidence(response: &RoutedResponse, tenant: &str) {
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

pub(super) fn parse(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, KNOWLEDGE_JSON_LIMITS).unwrap()
}

pub(super) fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value
        .get(name)
        .unwrap_or_else(|| panic!("missing field {name}"))
}

pub(super) fn assert_native_json(response: &RoutedResponse) {
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

pub(super) fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

pub(super) fn captured_source() -> EvidenceSnapshot {
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

pub(super) fn source_owner(
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

pub(super) fn ready_from_source(
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
