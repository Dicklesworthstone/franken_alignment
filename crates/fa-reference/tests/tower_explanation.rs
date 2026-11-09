//! Original admission predicates through separately granted native Tower reads.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileAdmissionExplanation, FileExplanationResult,
};
use fa_reference::action::consequence::gate::containment::session::policy::{
    Policy, Predicate, Truth,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::Knowledge;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerRequest,
    Verb, encode_request,
};
use std::collections::BTreeSet;
use support::{Directory, Port, Supervisor, create, observe, oversight, profile, ready};

fn tower(port: Port) -> PublicationTower {
    PublicationTower::new(
        Address::new("1", Noun::Run, "3", Some(41)).unwrap(),
        port,
        PublicationLimits::default(),
    )
    .unwrap()
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        Vec::new(),
        0,
    )
}

fn request(tower: &PublicationTower, key: u64) -> TowerRequest {
    TowerRequest::new(
        "explain-admission",
        tower.request_address(key).unwrap(),
        Verb::Explain,
    )
}

fn explain(tower: &mut PublicationTower, key: u64) -> PublicationResponse {
    let request = request(tower, key);
    tower.exchange(&access(true), encode_request(&request).unwrap().as_bytes())
}

fn native(response: &PublicationResponse) -> &FileExplanationResult {
    let PublicationResult::Explanation(result) = &response.result else {
        panic!("expected original admission explanation: {response:?}");
    };
    result
}

fn known(response: &PublicationResponse) -> &FileAdmissionExplanation {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected a native Known explanation: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert!(response.envelope.reason_code.is_none());
    assert!(matches!(
        response.envelope.generation,
        Knowledge::Unknown { .. }
    ));
    assert!(matches!(
        response.envelope.control_seq,
        Knowledge::Unknown { .. }
    ));
    assert!(matches!(
        response.envelope.source_frontiers,
        Knowledge::Unknown { .. }
    ));
    assert!(matches!(
        response.envelope.affordances,
        Knowledge::Unknown { .. }
    ));
    value
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
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

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn custom_policy(root: &Directory, nodes: Vec<Predicate>) -> (Port, Supervisor, ActorProposal) {
    let mut configuration = profile();
    configuration.delivery.policy = Policy::new(u64::MAX - 2, nodes).unwrap();
    let (mut host, _reviewer) = FileOversight::create(root.store(), configuration).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (actor, supervisor) = host.into_actor_gateway();
    (actor, supervisor, proposal)
}

#[test]
fn real_publication_keeps_its_original_admission_and_distinct_ending_cut() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.explanation_port(2).unwrap();
    facade.set_explanation(Some(port)).unwrap();
    observe(&mut supervisor);
    let ticket = actor.submit(701, &proposal).unwrap();
    let before = image(&root);
    let state = supervisor.host().unwrap().inspect();
    let admitted = explain(&mut facade, 701);
    let original = known(&admitted);
    assert_eq!(original.basis().request, 701);
    assert_ne!(original.basis().attempt, 701);
    assert_eq!(original.initial_state(), ActionState::Reviewing);
    assert_eq!(original.ending_state(), ActionState::Reviewing);
    assert_eq!(original.policy(), &profile().delivery.policy);
    assert_eq!(original.evaluation().result(), Truth::Satisfied);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(image(&root), before);
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: 701 }
    );

    let keys = ready(&mut supervisor, &reviewer, 701);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.publish(revision, keys.automatic.attempt()).unwrap();
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let published = image(&root);
    let completed = explain(&mut facade, 701);
    let ending = known(&completed);
    assert_eq!(ending.initial_state(), ActionState::Reviewing);
    assert_eq!(ending.ending_state(), ActionState::Confirmed);
    assert_eq!(
        ending.basis().admission_revision,
        original.basis().admission_revision
    );
    assert!(ending.basis().journal_revision > original.basis().journal_revision);
    assert_eq!(ending.evaluation(), original.evaluation());
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(image(&root), published);
    assert_eq!(grant.remaining_queries(), 0);
    let document = document(&completed);
    let value = field(field(&document, "result"), "value");
    assert_eq!(
        field(value, "scope").as_str(),
        Some("historical_exact_policy_admission")
    );
    assert_eq!(
        field(field(value, "current_hold_reason"), "state").as_str(),
        Some("unknown")
    );
    assert_eq!(
        field(value, "current_eligibility").as_str(),
        Some("not_observed")
    );
    assert!(
        !completed
            .to_json()
            .unwrap()
            .contains("PRIVATE-SOURCE-BYTES")
    );
}

#[test]
fn complete_tower_admission_precedes_policy_disclosure_or_query_spending() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    let mut facade = tower(actor.clone());
    let (inspection, _inspection_grant) = supervisor.inspection_port(2).unwrap();
    facade.set_inspection(Some(inspection)).unwrap();
    refused(
        &explain(&mut facade, 701),
        ReasonCode::CapabilityUnsupported,
    );
    let (port, grant) = supervisor.explanation_port(2).unwrap();
    facade.set_explanation(Some(port)).unwrap();
    observe(&mut supervisor);
    let before = image(&root);
    let query = request(&facade, 701);
    refused(
        &facade.dispatch(&access(false), query.clone()),
        ReasonCode::DeniedByPolicy,
    );

    let mut stale = query.clone();
    stale.address.generation = Some(40);
    refused(
        &facade.dispatch(&access(true), stale),
        ReasonCode::StaleGeneration,
    );
    let mut foreign = query.clone();
    foreign.address.tenant = "other".into();
    refused(
        &facade.dispatch(&access(true), foreign),
        ReasonCode::DeniedByPolicy,
    );
    let mut malformed = query.clone();
    malformed.arguments = ReadArguments::Next { max_cost: 1 };
    refused(
        &facade.dispatch(&access(true), malformed),
        ReasonCode::IntegrityFailure,
    );
    let mut wrong_address = query.clone();
    wrong_address.address = facade.mount_address().clone();
    refused(
        &facade.dispatch(&access(true), wrong_address),
        ReasonCode::MissingRequiredObservation,
    );
    let bytes = encode_request(&query).unwrap();
    let forged = bytes.replacen(
        "\"arguments\":null",
        "\"arguments\":null,\"operator\":true",
        1,
    );
    refused(
        &facade.exchange(&access(true), forged.as_bytes()),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(image(&root), before);

    // None of the rejected reads consumes the already installed admission.
    let _ticket = actor.submit(701, &proposal).unwrap();
    assert_eq!(known(&explain(&mut facade, 701)).basis().request, 701);
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn request_aliases_share_custody_and_never_cache_known_after_owner_loss() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    let _ticket = actor.submit(701, &proposal).unwrap();
    let (port, grant) = supervisor.explanation_port(3).unwrap();
    let mut first = tower(actor.clone());
    let mut second = tower(actor.clone());
    first.set_explanation(Some(port.clone())).unwrap();
    second.set_explanation(Some(port)).unwrap();
    known(&explain(&mut first, 701));
    let mut repeated = request(&second, 701);
    repeated.idempotency_key = Some("same-read".into());
    known(&second.dispatch(&access(true), repeated));
    assert_eq!(grant.remaining_queries(), 1);
    drop(supervisor);
    for facade in [&mut first, &mut second] {
        let unavailable = explain(facade, 701);
        assert_eq!(
            native(&unavailable),
            &Ok(ActorKnowledge::Unknown {
                reason: ActorUnknown::ControllerUnavailable,
            })
        );
    }
    assert_eq!(grant.remaining_queries(), 1);
    grant.revoke();
    assert_eq!(
        native(&explain(&mut first, 701)),
        &Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_explanation",
        })
    );
}

#[test]
fn canonical_explanation_encodes_every_original_predicate_without_reinterpreting_it() {
    let root = Directory::new();
    let target = profile().delivery.target;
    let nodes = vec![
        Predicate::TargetIs(target),
        Predicate::PayloadIs(b"published".to_vec()),
        Predicate::PayloadAtMost(128),
        Predicate::UnitsAtMost(16),
        Predicate::ExactValue {
            key: 7,
            value: b"ok".to_vec(),
        },
        Predicate::Absent { key: 8 },
        Predicate::EmptyRange { start: 20, end: 30 },
        Predicate::Not(5),
        Predicate::Any(vec![5, 7]),
        Predicate::All(vec![0, 1, 2, 3, 4, 6, 8]),
    ];
    let (actor, mut supervisor, proposal) = custom_policy(&root, nodes.clone());
    observe(&mut supervisor);
    let _ticket = actor.submit(u64::MAX, &proposal).unwrap();
    let (port, _grant) = supervisor.explanation_port(1).unwrap();
    let mut facade = tower(actor);
    facade.set_explanation(Some(port)).unwrap();
    let response = explain(&mut facade, u64::MAX);
    assert_eq!(known(&response).policy().nodes(), nodes.as_slice());
    let document = document(&response);
    let envelope = field(&document, "envelope").as_object().unwrap();
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect::<BTreeSet<_>>()
    );
    let native = field(field(&document, "result"), "value");
    let knowledge = field(native, "knowledge");
    assert_eq!(
        field(field(knowledge, "basis"), "request").as_str(),
        Some(u64::MAX.to_string().as_str())
    );
    let value = field(knowledge, "value");
    assert_eq!(
        field(value, "policy_generation").as_str(),
        Some((u64::MAX - 2).to_string().as_str())
    );
    let encoded = field(field(value, "predicate_tree"), "nodes")
        .as_array()
        .unwrap();
    let expected = [
        "target_is",
        "payload_is",
        "payload_at_most",
        "units_at_most",
        "exact_value",
        "absent",
        "empty_range",
        "not",
        "any",
        "all",
    ];
    assert_eq!(encoded.len(), expected.len());
    for (node, expected) in encoded.iter().zip(expected) {
        assert_eq!(
            field(field(node, "predicate"), "op").as_str(),
            Some(expected)
        );
    }
    assert_eq!(field(&encoded[7], "result").as_str(), Some("violated"));
    assert_eq!(field(&encoded[8], "result").as_str(), Some("satisfied"));
    assert_eq!(field(value, "result").as_str(), Some("satisfied"));
}

#[test]
fn encoding_rejects_rebound_provenance_and_subject_after_a_genuine_denial() {
    let root = Directory::new();
    // Reach a real exact-policy denial with a proposal that satisfies native
    // intake accounting. A 129-byte payload under 16 units never reached policy.
    let (actor, mut supervisor, proposal) = custom_policy(&root, vec![Predicate::PayloadAtMost(8)]);
    observe(&mut supervisor);
    let _ticket = actor.submit(701, &proposal).unwrap();
    let (port, _grant) = supervisor.explanation_port(1).unwrap();
    let mut facade = tower(actor);
    facade.set_explanation(Some(port)).unwrap();
    let response = explain(&mut facade, 701);
    assert_eq!(known(&response).initial_state(), ActionState::Denied);
    assert_eq!(known(&response).evaluation().result(), Truth::Violated);
    response.to_json().unwrap();

    let mut changed = response.clone();
    let PublicationResult::Explanation(Ok(ActorKnowledge::Known { basis, .. })) =
        &mut changed.result
    else {
        panic!("original known explanation");
    };
    basis.journal_revision += 1;
    assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    let mut changed = response.clone();
    changed.envelope.address = facade.request_address(702).unwrap();
    assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    let mut changed = response;
    changed.result = PublicationResult::Explanation(Ok(ActorKnowledge::Pending { request: 701 }));
    assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
}

#[test]
fn output_refusal_spends_the_original_query_without_partial_policy_or_mutation() {
    let root = Directory::new();
    let literal = vec![0xaa; 20_000];
    let (actor, mut supervisor, proposal) =
        custom_policy(&root, vec![Predicate::PayloadIs(literal)]);
    observe(&mut supervisor);
    let _ticket = actor.submit(701, &proposal).unwrap();
    let before = image(&root);
    let (port, grant) = supervisor.explanation_port(2).unwrap();
    let mut bounded = tower(actor.clone());
    bounded.set_explanation(Some(port.clone())).unwrap();
    let refused_output = explain(&mut bounded, 701);
    refused(&refused_output, ReasonCode::BudgetExhausted);
    assert!(!refused_output.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 1);
    let mut admitted = PublicationTower::new(
        bounded.mount_address().clone(),
        actor,
        PublicationLimits {
            max_response_bytes: 128 * 1024,
        },
    )
    .unwrap();
    admitted.set_explanation(Some(port)).unwrap();
    let complete = explain(&mut admitted, 701);
    assert_eq!(known(&complete).initial_state(), ActionState::Denied);
    assert!(complete.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 0);
    let exhausted = explain(&mut bounded, 701);
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    assert_eq!(image(&root), before);
}
