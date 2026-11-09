//! Original, complete-corpus policy experiments through separately held Tower custody.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyReplay, FilePolicyReplayResult,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::consequence::policy_campaign::{PolicyDelta, ReplayCaseId, ReplayLimits};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolvedObject};
use fa_reference::event_journal::DomainEventJournal;
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{
    Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::router::{RoutedResponse, TowerRouter};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, ReferenceTower, TowerAccess, TowerBuilder,
    TowerEncodingError, TowerInstallError, TowerLimits, TowerRequest, Verb, decode_request,
    encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use std::collections::BTreeSet;
use support::{Directory, Port, Supervisor, create, observe, oversight, profile, ready, snapshot};

const PRIVATE_PAYLOAD: &[u8] = b"PRIVATE-PAYLOAD";
const PRIVATE_WITNESS: &[u8] = b"PRIVATE-WITNESS-BYTES";

fn limits() -> ReplayLimits {
    ReplayLimits {
        cases: 16,
        input_bytes: 1_048_576,
    }
}

fn candidate() -> Policy {
    Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()
}

fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new(
        Address::new("1", Noun::Run, "3", Some(41)).unwrap(),
        actor,
        PublicationLimits::default(),
    )
    .unwrap()
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        vec![PurposeTag::Experiment],
        0,
    )
}

fn request_at(address: &Address, campaign: u64, expected: u64, candidate: Policy) -> TowerRequest {
    let mut request = TowerRequest::new("compare-policy", address.clone(), Verb::PolicyReplay);
    request.arguments = ReadArguments::PolicyReplay {
        campaign,
        expected_policy_generation: expected,
        candidate,
    };
    request.idempotency_key = Some(campaign.to_string());
    request
}

fn request(facade: &PublicationTower, campaign: u64, candidate: Policy) -> TowerRequest {
    request_at(facade.mount_address(), campaign, 1, candidate)
}

fn replay(facade: &mut PublicationTower, campaign: u64, candidate: Policy) -> PublicationResponse {
    let document = encode_request(&request(facade, campaign, candidate)).unwrap();
    facade.exchange(&access(true), document.as_bytes())
}

fn native(response: &PublicationResponse) -> &FilePolicyReplayResult {
    let PublicationResult::PolicyReplay(result) = &response.result else {
        panic!("expected original policy replay: {response:?}");
    };
    no_global_claims(response);
    result.native()
}

fn known(response: &PublicationResponse) -> &FilePolicyReplay {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected complete native comparison: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(response.envelope.reason_code, None);
    value
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
    no_global_claims(response);
}

fn unavailable(response: &PublicationResponse) {
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
}

fn no_global_claims(response: &PublicationResponse) {
    fn unknown<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unknown(&response.envelope.generation);
    unknown(&response.envelope.control_seq);
    unknown(&response.envelope.source_frontiers);
    unknown(&response.envelope.budgets_remaining);
    unknown(&response.envelope.realized_cost);
    unknown(&response.envelope.affordances);
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn corpus(root: &Directory) -> (Port, Supervisor, oversight::Keys) {
    let (actor, mut supervisor, reviewer, mut proposal) = create(root);
    proposal.payload = PRIVATE_PAYLOAD.to_vec();
    observe(&mut supervisor);
    let _ticket = actor.submit(701, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, 701);
    {
        let mut host = supervisor.host_mut().unwrap();
        let mut denied = snapshot();
        denied.values.insert(7, PRIVATE_WITNESS.to_vec());
        let spec = oversight::spec(&host, b"denied");
        let revision = host.revision();
        host.propose(revision, 2, spec, denied).unwrap();
    }
    (actor, supervisor, keys)
}

fn field<'a>(document: &'a Json, name: &str) -> &'a Json {
    document
        .get(name)
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn complete_original_corpus_includes_denials_and_reviews_without_spending_existing_authority() {
    let root = Directory::new();
    let (actor, mut supervisor, keys) = corpus(&root);
    let original = supervisor
        .host()
        .unwrap()
        .replay_candidate_policy(candidate(), limits())
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let mut facade = tower(actor);
    facade.set_policy_replay(Some(port)).unwrap();

    let response = replay(&mut facade, 91, candidate());
    let result = known(&response);
    assert_eq!(result.report(), &original);
    assert_eq!(result.report().cases().len(), 3);
    assert_eq!(
        result.report().newly_reviewable(),
        vec![ReplayCaseId::Proposal(2)]
    );
    assert_eq!(
        result
            .report()
            .cases()
            .iter()
            .filter(|case| matches!(case.id(), ReplayCaseId::Review { .. }))
            .count(),
        1
    );
    assert!(!result.report().requires_shadow());
    assert_eq!(result.basis().query, 91);
    assert_eq!(result.basis().scope, profile().delivery.scope);
    assert_eq!(
        result.basis().journal_revision,
        supervisor.host().unwrap().revision()
    );
    assert_eq!(result.basis().policy_generation, 1);
    assert_eq!(result.basis().recorded_at, Some(ElapsedTick(1)));
    assert_eq!(result.limits(), limits());
    assert_eq!(result.remaining_queries(), 1);
    assert_eq!(grant.remaining_queries(), 1);
    assert!(grant.retained_bytes() > 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);

    let wire = response.to_json().unwrap();
    assert!(wire.contains("fa.publication-policy-replay/1"));
    for secret in [PRIVATE_PAYLOAD, PRIVATE_WITNESS, b"PRIVATE-SOURCE-BYTES"] {
        assert!(!wire.contains(std::str::from_utf8(secret).unwrap()));
        assert!(!wire.contains(&hex(secret)));
    }
    assert!(!wire.contains(root.0.to_str().unwrap()));
    let document = strict_json::parse(wire.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(
        field(&document, "envelope")
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect::<BTreeSet<_>>()
    );
    let value = field(field(&document, "result"), "value");
    assert_eq!(
        field(value, "scope").as_str(),
        Some("retained_current_policy_corpus")
    );
    assert_eq!(field(value, "historical").as_bool(), Some(true));
    for name in [
        "policy_changed",
        "helpers_rerun",
        "live_evidence_read",
        "effect_authority",
        "promotion_authority",
        "payload_disclosed",
        "witness_values_disclosed",
    ] {
        assert_eq!(field(value, name).as_bool(), Some(false));
    }
    let knowledge = field(value, "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    assert_eq!(
        field(field(knowledge, "basis"), "campaign").as_str(),
        Some("91")
    );
    let report = field(knowledge, "value");
    assert_eq!(field(report, "case_count").as_str(), Some("3"));
    let cases = field(report, "cases").as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let action = field(case, "action").as_object().unwrap();
        assert_eq!(
            action.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            [
                "target",
                "units",
                "payload_bytes",
                "policy_epoch",
                "deadline"
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
        );
        assert!(field(case, "original").get("witness_count").is_some());
        assert!(field(case, "original").get("witnesses").is_none());
        assert!(field(case, "candidate").get("witnesses").is_none());
    }

    let stricter = Policy::new(2, vec![Predicate::PayloadAtMost(0)]).unwrap();
    let blocked = replay(&mut facade, 92, stricter);
    assert_eq!(known(&blocked).report().cases().len(), 3);
    assert_eq!(known(&blocked).report().newly_blocked().len(), 2);
    assert!(known(&blocked).report().newly_reviewable().is_empty());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);

    // No replay report is needed by publication: its original two keys still
    // perform exactly one real dispatch and effect under the unchanged policy.
    let mut host = supervisor.host_mut().unwrap();
    oversight::dispatch(&mut host, &keys);
    let revision = host.revision();
    host.publish(revision, keys.automatic.attempt()).unwrap();
    let revision = host.revision();
    host.reconcile(revision, keys.automatic.attempt()).unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.current_policy().unwrap(), &profile().delivery.policy);
}

#[test]
fn exact_campaign_retries_keep_the_original_cut_across_aliases_and_later_proposals() {
    let root = Directory::new();
    let (actor, mut supervisor, _keys) = corpus(&root);
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let mut first = tower(actor.clone());
    let mut second = tower(actor);
    first.set_policy_replay(Some(port.clone())).unwrap();
    second.set_policy_replay(Some(port)).unwrap();
    let original = replay(&mut first, 7, candidate());
    let original_value = known(&original).clone();
    let retained = grant.retained_bytes();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
        let spec = oversight::spec(&host, b"later proposal");
        let revision = host.revision();
        host.propose(revision, 3, spec, snapshot()).unwrap();
    }
    let after = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let mut retry = request(&second, 7, candidate());
    retry.operation_id = "a-different-transport-operation".into();
    let repeated = second.dispatch(&access(true), retry);
    assert_eq!(known(&repeated), &original_value);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.retained_bytes(), retained);

    let changed = replay(&mut second, 8, candidate());
    assert_eq!(known(&changed).report().cases().len(), 4);
    assert!(known(&changed).basis().journal_revision > original_value.basis().journal_revision);
    assert_eq!(known(&changed).basis().recorded_at, Some(ElapsedTick(2)));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(known(&replay(&mut first, 7, candidate())), &original_value);
    let conflict = Policy::new(3, vec![Predicate::PayloadAtMost(128)]).unwrap();
    assert_eq!(
        native(&replay(&mut first, 7, conflict)),
        &Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(
        native(&replay(&mut first, 9, candidate())),
        &Err(ActorError::Capacity)
    );
    assert_eq!(supervisor.host().unwrap().inspect(), after);
    assert_eq!(image(&root), bytes);
}

#[test]
fn admission_requires_operator_exact_mount_and_separate_grant_before_query_spending() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root);
    let mut facade = tower(actor);
    let (inspection, _inspection_custodian) = supervisor.inspection_port(2).unwrap();
    let (explanation, _explanation_custodian) = supervisor.explanation_port(2).unwrap();
    facade.set_inspection(Some(inspection)).unwrap();
    facade.set_explanation(Some(explanation)).unwrap();
    refused(
        &replay(&mut facade, 7, candidate()),
        ReasonCode::CapabilityUnsupported,
    );
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    facade.set_policy_replay(Some(port)).unwrap();
    let bytes = image(&root);
    let query = request(&facade, 7, candidate());
    refused(
        &facade.dispatch(&access(false), query.clone()),
        ReasonCode::DeniedByPolicy,
    );
    let no_experiment = TowerAccess::new(CallerScope::full_tenant_access("1"), true, Vec::new(), 0);
    refused(
        &facade.dispatch(&no_experiment, query.clone()),
        ReasonCode::PurposeMismatch,
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
    let mut wrong_subject = query.clone();
    wrong_subject.address = facade.request_address(701).unwrap();
    refused(
        &facade.dispatch(&access(true), wrong_subject),
        ReasonCode::MissingRequiredObservation,
    );
    let mut malformed = query.clone();
    malformed.arguments = ReadArguments::Next { max_cost: 1 };
    refused(
        &facade.dispatch(&access(true), malformed),
        ReasonCode::IntegrityFailure,
    );
    let mut missing_key = query.clone();
    missing_key.idempotency_key = None;
    refused(
        &facade.dispatch(&access(true), missing_key),
        ReasonCode::IntegrityFailure,
    );
    let mut alias_key = query.clone();
    alias_key.idempotency_key = Some("007".into());
    refused(
        &facade.dispatch(&access(true), alias_key),
        ReasonCode::IntegrityFailure,
    );
    let mut implicit_generation = query.clone();
    implicit_generation.address.generation = None;
    refused(
        &facade.dispatch(&access(true), implicit_generation),
        ReasonCode::StaleGeneration,
    );
    for (campaign, expected, policy) in [
        (0, 1, candidate()),
        (7, 0, candidate()),
        (
            7,
            1,
            Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        ),
    ] {
        let malformed = request_at(facade.mount_address(), campaign, expected, policy);
        let response = facade.dispatch(&access(true), malformed);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
        if let PublicationResult::PolicyReplay(linked) = &response.result {
            assert_eq!(linked.native(), &Err(ActorError::MalformedProposal));
        }
    }
    let valid = encode_request(&query).unwrap();
    let forged = valid.replacen("\"arguments\":", "\"operator\":true,\"arguments\":", 1);
    refused(
        &facade.exchange(&access(true), forged.as_bytes()),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.retained_bytes(), 0);
    assert_eq!(image(&root), bytes);
    assert_eq!(
        known(&replay(&mut facade, 7, candidate()))
            .report()
            .cases()
            .len(),
        3
    );
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn replacing_a_grant_from_a_different_original_owner_clears_prior_custody() {
    let root = Directory::new();
    let other_root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root);
    let (_other_actor, other_owner, _other_reviewer, _proposal) = create(&other_root);
    let (original, original_grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let (foreign, foreign_grant) = other_owner.policy_replay_port(2, limits()).unwrap();
    let mut facade = tower(actor);
    facade.set_policy_replay(Some(original.clone())).unwrap();
    assert_eq!(
        facade.set_policy_replay(Some(foreign)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &replay(&mut facade, 7, candidate()),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(original_grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_queries(), 2);
    facade.set_policy_replay(Some(original)).unwrap();
    known(&replay(&mut facade, 7, candidate()));
    assert_eq!(original_grant.remaining_queries(), 1);
    assert_eq!(foreign_grant.remaining_queries(), 2);
}

#[test]
fn cached_reports_reauthenticate_busy_owner_loss_and_revocation_before_disclosure() {
    let root = Directory::new();
    let (actor, mut supervisor, _keys) = corpus(&root);
    let (port, grant) = supervisor.policy_replay_port(3, limits()).unwrap();
    let mut first = tower(actor.clone());
    let mut second = tower(actor);
    first.set_policy_replay(Some(port.clone())).unwrap();
    second.set_policy_replay(Some(port)).unwrap();
    let original = replay(&mut first, 7, candidate());
    let retained = grant.retained_bytes();
    {
        let _borrow = supervisor.host_mut().unwrap();
        unavailable(&replay(&mut second, 7, candidate()));
        unavailable(&replay(&mut second, 8, candidate()));
    }
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.retained_bytes(), retained);
    assert_eq!(
        native(&replay(&mut second, 7, candidate())),
        native(&original)
    );
    drop(supervisor);
    for facade in [&mut first, &mut second] {
        unavailable(&replay(facade, 7, candidate()));
    }
    assert_eq!(grant.remaining_queries(), 2);
    grant.revoke();
    for facade in [&mut first, &mut second] {
        let response = replay(facade, 7, candidate());
        assert_eq!(
            native(&response),
            &Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_policy_replay",
            })
        );
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
    }
}

#[test]
fn stale_generation_and_complete_corpus_capacity_failures_are_shared_one_shot_results() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root);
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let mut facade = tower(actor.clone());
    facade.set_policy_replay(Some(port)).unwrap();
    let future = Policy::new(3, vec![Predicate::PayloadAtMost(128)]).unwrap();
    let stale = request_at(facade.mount_address(), 7, 2, future);
    let response = facade.dispatch(&access(true), stale.clone());
    assert_eq!(
        native(&response),
        &Ok(ActorKnowledge::Stale {
            generation: 2,
            current: 1,
        })
    );
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(
        native(&facade.dispatch(&access(true), stale)),
        native(&response)
    );
    assert_eq!(grant.remaining_queries(), 1);

    for constrained in [
        ReplayLimits {
            cases: 2,
            ..limits()
        },
        ReplayLimits {
            input_bytes: 1,
            ..limits()
        },
    ] {
        let (port, grant) = supervisor.policy_replay_port(1, constrained).unwrap();
        let mut limited = tower(actor.clone());
        limited.set_policy_replay(Some(port)).unwrap();
        let response = replay(&mut limited, 11, candidate());
        assert_eq!(native(&response), &Err(ActorError::Capacity));
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::BudgetExhausted)
        );
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(
            native(&replay(&mut limited, 11, candidate())),
            native(&response)
        );
        assert_eq!(grant.remaining_queries(), 0);
    }
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn an_empty_corpus_is_unknown_and_its_retry_does_not_invent_new_observations() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let mut facade = tower(actor.clone());
    facade.set_policy_replay(Some(port)).unwrap();
    unavailable(&replay(&mut facade, 7, candidate()));
    assert_eq!(grant.remaining_queries(), 1);
    observe(&mut supervisor);
    let _ticket = actor.submit(701, &proposal).unwrap();
    unavailable(&replay(&mut facade, 7, candidate()));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(
        known(&replay(&mut facade, 8, candidate()))
            .report()
            .cases()
            .len(),
        1
    );
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn missing_candidate_observations_require_shadow_even_under_a_satisfied_disjunction() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root);
    let (port, grant) = supervisor.policy_replay_port(1, limits()).unwrap();
    let mut facade = tower(actor);
    facade.set_policy_replay(Some(port)).unwrap();
    let policy = Policy::new(
        2,
        vec![
            Predicate::PayloadAtMost(128),
            Predicate::Absent { key: 99 },
            Predicate::Any(vec![0, 1]),
        ],
    )
    .unwrap();
    let before = image(&root);
    let response = replay(&mut facade, 7, policy);
    let report = known(&response).report();
    assert_eq!(report.cases().len(), 3);
    assert!(report.requires_shadow());
    assert!(report.newly_reviewable().is_empty());
    for case in report.cases() {
        assert_eq!(case.delta(), PolicyDelta::RequiresShadow);
        assert_eq!(case.missing_nodes(), &[1]);
        assert!(case.candidate().is_none());
    }
    let wire = response.to_json().unwrap();
    assert!(wire.contains("requires_shadow"));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(image(&root), before);
}

#[test]
fn output_capacity_retains_the_complete_native_campaign_for_an_exact_larger_transport_retry() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root);
    let (port, grant) = supervisor.policy_replay_port(1, limits()).unwrap();
    let mut bounded = tower(actor.clone());
    bounded.set_policy_replay(Some(port.clone())).unwrap();
    let candidate = Policy::new(2, vec![Predicate::PayloadIs(vec![0xaa; 20_000])]).unwrap();
    let bytes = image(&root);
    let refused_output = replay(&mut bounded, 7, candidate.clone());
    refused(&refused_output, ReasonCode::BudgetExhausted);
    assert!(!refused_output.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 0);
    let retained = grant.retained_bytes();
    let mut larger = PublicationTower::new(
        bounded.mount_address().clone(),
        actor,
        PublicationLimits {
            max_response_bytes: 128 * 1024,
        },
    )
    .unwrap();
    larger.set_policy_replay(Some(port)).unwrap();
    let complete = replay(&mut larger, 7, candidate);
    assert_eq!(known(&complete).report().cases().len(), 3);
    assert!(complete.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.retained_bytes(), retained);
    assert_eq!(image(&root), bytes);
}

#[test]
fn encoder_rejects_rebound_registration_subject_and_campaign_for_known_and_failed_comparisons() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root);
    let (port, _grant) = supervisor.policy_replay_port(1, limits()).unwrap();
    let mut facade = tower(actor);
    facade.set_policy_replay(Some(port)).unwrap();
    let response = replay(&mut facade, 7, candidate());
    known(&response);
    response.to_json().unwrap();
    let exhausted = replay(&mut facade, 8, candidate());
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    exhausted.to_json().unwrap();

    for response in [response, exhausted] {
        let PublicationResult::PolicyReplay(linked) = &response.result else {
            panic!("original linked comparison");
        };
        assert_eq!(linked.registration(), facade.mount_address());
        assert_eq!(
            response.envelope.idempotency_key.as_deref(),
            Some(linked.campaign().to_string().as_str())
        );
        for generation in [None, Some(0), Some(40), Some(42), Some(u64::MAX)] {
            let mut changed = response.clone();
            changed.envelope.address.generation = generation;
            assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
        let mut changed = response.clone();
        changed.envelope.address = facade.request_address(701).unwrap();
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.address.tenant = "another-owner".into();
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        for key in [None, Some("9".into()), Some("007".into())] {
            let mut changed = response.clone();
            changed.envelope.idempotency_key = key;
            assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
    }
}

#[test]
fn request_roundtrips_every_predicate_and_full_width_campaign_without_caller_custody_fields() {
    let root = Directory::new();
    let (actor, _owner, _reviewer, _proposal) = create(&root);
    let facade = tower(actor);
    let policy = Policy::new(
        u64::MAX,
        vec![
            Predicate::TargetIs(profile().delivery.target),
            Predicate::PayloadIs(vec![0, 0xff]),
            Predicate::PayloadAtMost(128),
            Predicate::UnitsAtMost(16),
            Predicate::ExactValue {
                key: u64::MAX,
                value: vec![0, 0xff],
            },
            Predicate::Absent { key: 8 },
            Predicate::EmptyRange { start: 20, end: 30 },
            Predicate::Not(5),
            Predicate::Any(vec![5, 7]),
            Predicate::All(vec![0, 1, 2, 3, 4, 6, 8]),
        ],
    )
    .unwrap();
    let query = request_at(facade.mount_address(), u64::MAX, u64::MAX - 1, policy);
    let wire = encode_request(&query).unwrap();
    assert_eq!(decode_request(wire.as_bytes()).unwrap(), query);
    assert_eq!(
        encode_request(&decode_request(wire.as_bytes()).unwrap()).unwrap(),
        wire
    );
    let forbidden = wire.replacen("\"arguments\":", "\"governor\":true,\"arguments\":", 1);
    assert!(decode_request(forbidden.as_bytes()).is_err());
    let duplicate = wire.replacen("\"campaign\":", "\"campaign\":\"1\",\"campaign\":", 1);
    assert!(decode_request(duplicate.as_bytes()).is_err());
    let alias = wire.replacen(
        "\"campaign\":\"18446744073709551615\"",
        "\"campaign\":\"018446744073709551615\"",
        1,
    );
    assert_ne!(alias, wire);
    assert!(decode_request(alias.as_bytes()).is_err());
    let number = wire.replacen(
        "\"campaign\":\"18446744073709551615\"",
        "\"campaign\":18446744073709551615",
        1,
    );
    assert!(decode_request(number.as_bytes()).is_err());
    let mut key_mismatch = query.clone();
    key_mismatch.idempotency_key = Some("1".into());
    assert!(encode_request(&key_mismatch).is_err());
    let invalid_graph = wire.replacen("\"child\":\"5\"", "\"child\":\"9\"", 1);
    assert_ne!(invalid_graph, wire);
    assert!(decode_request(invalid_graph.as_bytes()).is_err());
    let unknown_node = wire.replacen("\"op\":\"not\"", "\"op\":\"unsafe_allow\"", 1);
    assert_ne!(unknown_node, wire);
    assert!(decode_request(unknown_node.as_bytes()).is_err());
    let extra_node_field = wire.replacen("\"child\":\"5\"", "\"child\":\"5\",\"trusted\":true", 1);
    assert!(decode_request(extra_node_field.as_bytes()).is_err());
}

fn imported(address: &Address) -> ReferenceTower {
    let declared = ScopedProof::Declared {
        scope_id: 71,
        generation: 7,
    };
    let basis = Basis::new(
        BasisIdentity {
            subject_id: 72,
            question_id: 73,
            profile_id: 74,
            semantic_generation: 7,
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
    );
    let mut builder = TowerBuilder::new(
        DomainEventJournal::new_production("1", "policy-replay-snapshot"),
        basis,
        TowerLimits::default(),
        4,
    )
    .unwrap();
    let mut address = address.clone();
    address.generation = Some(7);
    builder
        .insert_object(
            ResolvedObject {
                plane: address.kind.plane(),
                address,
                summary_digest: 0xdec0_1ded,
                generation: 7,
            },
            basis,
        )
        .unwrap();
    builder.build()
}

#[test]
fn the_unified_router_keeps_native_custody_and_never_falls_back_to_imported_run_objects() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = corpus(&root);
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let mut facade = tower(actor);
    facade.set_policy_replay(Some(port)).unwrap();
    let request = request(&facade, 7, candidate());
    let mut router =
        TowerRouter::with_publication(imported(facade.mount_address()), facade).unwrap();
    let response = router.exchange(&access(true), encode_request(&request).unwrap().as_bytes());
    let RoutedResponse::Publication(response) = response else {
        panic!("policy replay must use the original publication route");
    };
    assert_eq!(known(&response).report().cases().len(), 3);
    assert_eq!(grant.remaining_queries(), 1);
    drop(supervisor);
    let response = router.dispatch(&access(true), request);
    let RoutedResponse::Publication(response) = response else {
        panic!("owner loss must never consult the imported run object");
    };
    unavailable(&response);
    assert_eq!(grant.remaining_queries(), 1);
}
