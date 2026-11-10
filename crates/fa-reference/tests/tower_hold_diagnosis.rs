//! Recorded publication blockers through the original separately granted owner.
//! These controls execute native intake, congress, human approval and effects;
//! an observed predicate never substitutes for a publication key.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileHoldCondition, FileHoldDiagnosis, FileHoldDiagnosisResult,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::{ActionState, ElapsedTick};
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
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerBuilder,
    TowerEncodingError, TowerLimits, TowerRequest, Verb, encode_request,
};
use std::collections::BTreeSet;
use support::{Directory, Port, create, observe, oversight, profile, ready};

const REGISTRATION: u64 = 41;

fn tower(port: Port) -> PublicationTower {
    PublicationTower::new(
        Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap(),
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
        "diagnose-recorded-hold",
        tower.request_address(key).unwrap(),
        Verb::WhyHeld,
    )
}

fn diagnose(tower: &mut PublicationTower, key: u64) -> PublicationResponse {
    let request = request(tower, key);
    tower.exchange(&access(true), encode_request(&request).unwrap().as_bytes())
}

fn native(response: &PublicationResponse) -> &FileHoldDiagnosisResult {
    let PublicationResult::HoldDiagnosis(linked) = &response.result else {
        panic!("expected original hold diagnosis: {response:?}");
    };
    assert_eq!(linked.query_address(), &response.envelope.address);
    assert_eq!(linked.registration().generation, Some(REGISTRATION));
    linked.native()
}

fn known(response: &PublicationResponse) -> &FileHoldDiagnosis {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected native Known diagnosis: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert!(response.envelope.reason_code.is_none());
    assert!(matches!(response.envelope.generation, Knowledge::Unknown { .. }));
    assert!(matches!(response.envelope.control_seq, Knowledge::Unknown { .. }));
    assert!(matches!(response.envelope.source_frontiers, Knowledge::Unknown { .. }));
    assert!(matches!(response.envelope.affordances, Knowledge::Unknown { .. }));
    value
}

fn unknown(response: &PublicationResponse) {
    assert!(matches!(
        native(response),
        Ok(ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable })
    ));
    assert_eq!(response.envelope.reason_code, Some(ReasonCode::LowerLayerUnverified));
}

fn fact(value: &FileHoldDiagnosis, condition: FileHoldCondition) -> Option<bool> {
    let predicate = value
        .predicates()
        .iter()
        .find(|predicate| predicate.condition() == condition)
        .expect("condition is retained");
    match predicate.satisfied() {
        ActorKnowledge::Known { value: satisfied, basis } => {
            assert_eq!(*basis, value.basis());
            Some(*satisfied)
        }
        ActorKnowledge::Unknown { reason: ActorUnknown::ControllerUnavailable } => None,
        other => panic!("unexpected predicate knowledge: {other:?}"),
    }
}

fn no_live_permission(value: &FileHoldDiagnosis) {
    for condition in [
        FileHoldCondition::CurrentSourceEvidence,
        FileHoldCondition::CurrentApprovals,
        FileHoldCondition::CurrentClock,
        FileHoldCondition::FinalPublicationEligibility,
    ] {
        assert_eq!(fact(value, condition), None);
    }
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
fn original_diagnosis_tracks_review_authorization_and_real_publication_without_spending_authority() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.hold_diagnosis_port(4).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    observe(&mut supervisor);
    let before = image(&root);
    let state = supervisor.host().unwrap().inspect();
    unknown(&diagnose(&mut facade, 999));
    assert_eq!(image(&root), before);
    assert_eq!(supervisor.host().unwrap().inspect(), state);

    // A read of a missing request did not consume the admission snapshot.
    let ticket = actor.submit(701, &proposal).unwrap();
    let before = image(&root);
    let state = supervisor.host().unwrap().inspect();
    let pending = diagnose(&mut facade, 701);
    let value = known(&pending);
    assert_eq!(value.basis().request, 701);
    assert_ne!(value.basis().attempt, 701);
    assert_eq!(value.basis().request_generation, 0);
    assert_eq!(value.stage(), ActionState::Reviewing);
    assert_eq!(fact(value, FileHoldCondition::RecordedStageAuthorized), Some(false));
    assert_eq!(fact(value, FileHoldCondition::DispatchNotRecorded), Some(true));
    no_live_permission(value);
    assert_eq!(image(&root), before);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request: 701 });

    let keys = ready(&mut supervisor, &reviewer, 701);
    let before = image(&root);
    let authorized = diagnose(&mut facade, 701);
    let value = known(&authorized);
    assert_eq!(value.stage(), ActionState::Authorized);
    assert_eq!(fact(value, FileHoldCondition::RecordedStageAuthorized), Some(true));
    assert_eq!(value.basis().request_generation, 0);
    no_live_permission(value);
    assert_eq!(image(&root), before);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.publish(revision, keys.automatic.attempt()).unwrap();
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let before = image(&root);
    let completed = diagnose(&mut facade, 701);
    let value = known(&completed);
    assert_eq!(value.stage(), ActionState::Confirmed);
    assert_eq!(fact(value, FileHoldCondition::DispatchNotRecorded), Some(false));
    assert_eq!(fact(value, FileHoldCondition::RecordedStageAuthorized), Some(false));
    assert!(value.basis().journal_revision > known(&pending).basis().journal_revision);
    no_live_permission(value);
    assert_eq!(known(&pending).stage(), ActionState::Reviewing);
    assert_eq!(image(&root), before);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 16);
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn native_request_mapping_never_uses_an_equal_attempt_number_as_an_actor_request() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.hold_diagnosis_port(3).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let attempt = support::attempt(&supervisor.host().unwrap(), 701);
    assert_ne!(attempt, 701);
    unknown(&diagnose(&mut facade, attempt));
    unknown(&diagnose(&mut facade, 702));
    let response = diagnose(&mut facade, 701);
    assert_eq!(known(&response).basis().attempt, attempt);
    assert_eq!(known(&response).basis().request, 701);
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn independent_disclosure_and_caller_checks_precede_any_native_diagnosis() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let mut facade = tower(actor.clone());
    let query = request(&facade, 701);
    let actor_before = facade.dispatch(&access(false), query.clone());
    refused(&actor_before, ReasonCode::DeniedByPolicy);
    let (inspection, _inspection_grant) = supervisor.inspection_port(1).unwrap();
    facade.set_inspection(Some(inspection)).unwrap();
    let (explanation, _explanation_grant) = supervisor.explanation_port(1).unwrap();
    facade.set_explanation(Some(explanation)).unwrap();
    refused(&diagnose(&mut facade, 701), ReasonCode::CapabilityUnsupported);
    let (port, grant) = supervisor.hold_diagnosis_port(1).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    let actor_after = facade.dispatch(&access(false), query.clone());
    assert_eq!(actor_before, actor_after);

    let mut no_authority = CallerScope::full_tenant_access("1");
    no_authority.granted_authorities.clear();
    let mut no_plane = CallerScope::full_tenant_access("1");
    no_plane.allowed_planes.clear();
    for scope in [no_authority, no_plane, CallerScope::full_tenant_access("foreign")] {
        refused(
            &facade.dispatch(&TowerAccess::new(scope, true, Vec::new(), 0), query.clone()),
            ReasonCode::DeniedByPolicy,
        );
    }
    let mut stale = query.clone();
    stale.address.generation = Some(REGISTRATION - 1);
    refused(&facade.dispatch(&access(true), stale), ReasonCode::StaleGeneration);
    let mut malformed = query.clone();
    malformed.arguments = ReadArguments::Next { max_cost: 1 };
    refused(&facade.dispatch(&access(true), malformed), ReasonCode::IntegrityFailure);
    let mut malformed = query.clone();
    malformed.address.id = malformed.address.id.replacen("-701", "-0701", 1);
    refused(
        &facade.dispatch(&access(true), malformed),
        ReasonCode::MissingRequiredObservation,
    );
    let mut wrong_scope = query.clone();
    wrong_scope.address.id = "request-3-999-4-5-701".to_owned();
    refused(
        &facade.dispatch(&access(true), wrong_scope),
        ReasonCode::MissingRequiredObservation,
    );
    let mut run = query.clone();
    run.address = facade.mount_address().clone();
    refused(&facade.dispatch(&access(true), run), ReasonCode::MissingRequiredObservation);
    let document = encode_request(&query).unwrap();
    let extra = document.replacen('{', "{\"operator\":true,", 1);
    refused(
        &facade.exchange(&access(true), extra.as_bytes()),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 1);

    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    known(&facade.exchange(&access(true), document.as_bytes()));
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn shared_allowance_revocation_and_same_scope_foreign_owner_cannot_be_bypassed_by_remounting() {
    let root = Directory::new();
    let foreign_root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    let (_, foreign, _, _) = create(&foreign_root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let (port, grant) = supervisor.hold_diagnosis_port(3).unwrap();
    let (foreign_port, foreign_grant) = foreign.hold_diagnosis_port(2).unwrap();
    let mut first = tower(actor.clone());
    let mut second = tower(actor);
    first.set_hold_diagnosis(Some(port.clone())).unwrap();
    second.set_hold_diagnosis(Some(port.clone())).unwrap();
    known(&diagnose(&mut first, 701));
    assert!(first.set_hold_diagnosis(Some(foreign_port)).is_err());
    refused(&diagnose(&mut first, 701), ReasonCode::CapabilityUnsupported);
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_queries(), 2);
    first.set_hold_diagnosis(Some(port.clone())).unwrap();
    let mut repeated = request(&first, 701);
    repeated.idempotency_key = Some("a-read-does-not-cache-authority".to_owned());
    known(&first.dispatch(&access(true), repeated.clone()));
    known(&second.dispatch(&access(true), repeated));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(native(&diagnose(&mut second, 701)), &Err(ActorError::Capacity));
    second.set_hold_diagnosis(Some(port)).unwrap();
    assert_eq!(native(&diagnose(&mut second, 701)), &Err(ActorError::Capacity));

    let (port, live_grant) = supervisor.hold_diagnosis_port(2).unwrap();
    first.set_hold_diagnosis(Some(port.clone())).unwrap();
    second.set_hold_diagnosis(Some(port.clone())).unwrap();
    known(&diagnose(&mut first, 701));
    assert_eq!(live_grant.remaining_queries(), 1);
    live_grant.revoke();
    second.set_hold_diagnosis(Some(port)).unwrap();
    for response in [diagnose(&mut first, 701), diagnose(&mut second, 701)] {
        assert_eq!(
            native(&response),
            &Ok(ActorKnowledge::Withheld { authority_required: "supervisor_hold_diagnosis" })
        );
        assert_eq!(response.envelope.reason_code, Some(ReasonCode::DeniedByPolicy));
    }
}

#[test]
fn original_owner_borrow_storage_fault_and_reopening_never_revive_stale_diagnoses() {
    for faulted in [false, true] {
        let root = Directory::new();
        let (actor, mut supervisor, _, proposal) = create(&root);
        observe(&mut supervisor);
        actor.submit(701, &proposal).unwrap();
        let mut facade = tower(actor);
        let (port, grant) = supervisor.hold_diagnosis_port(3).unwrap();
        facade.set_hold_diagnosis(Some(port.clone())).unwrap();
        known(&diagnose(&mut facade, 701));
        {
            let mut host = supervisor.host_mut().unwrap();
            unknown(&diagnose(&mut facade, 701));
            assert_eq!(grant.remaining_queries(), 2);
            if faulted {
                std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
            }
            let revision = host.revision();
            let changed = host.observe_time(revision, ElapsedTick(2));
            assert_eq!(changed.is_err(), faulted);
        }
        if faulted {
            unknown(&diagnose(&mut facade, 701));
            assert_eq!(grant.remaining_queries(), 2);
        } else {
            let later = diagnose(&mut facade, 701);
            assert_eq!(known(&later).basis().recorded_at, Some(ElapsedTick(2)));
            assert_eq!(grant.remaining_queries(), 1);
        }
        let remaining = grant.remaining_queries();
        let before = image(&root);
        drop(supervisor);
        unknown(&diagnose(&mut facade, 701));
        assert_eq!(image(&root), before);
        let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let (new_actor, new_supervisor) = host.into_actor_gateway();
        unknown(&diagnose(&mut facade, 701));
        let mut reopened = tower(new_actor);
        assert!(reopened.set_hold_diagnosis(Some(port)).is_err());
        let (fresh, _grant) = new_supervisor.hold_diagnosis_port(1).unwrap();
        reopened.set_hold_diagnosis(Some(fresh)).unwrap();
        known(&diagnose(&mut reopened, 701));
        assert_eq!(grant.remaining_queries(), remaining);
    }
}

#[test]
fn canonical_report_keeps_recorded_predicate_bases_and_every_live_gate_unknown() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let mut facade = tower(actor);
    let (port, grant) = supervisor.hold_diagnosis_port(1).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    let response = diagnose(&mut facade, 701);
    let value = known(&response);
    no_live_permission(value);
    let parsed = document(&response);
    let envelope = field(&parsed, "envelope").as_object().unwrap();
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect()
    );
    let result = field(&parsed, "result");
    assert_eq!(field(result, "kind").as_str(), Some("hold-diagnosis"));
    let report = field(result, "value");
    assert_eq!(field(report, "schema").as_str(), Some("fa.publication-hold-diagnosis/1"));
    assert_eq!(field(report, "operation").as_str(), Some("why-held"));
    assert_eq!(field(report, "scope").as_str(), Some("recorded_publication_state"));
    assert_eq!(field(report, "request").as_str(), Some("701"));
    assert_eq!(field(report, "registration").as_str(), Some("fa://1/run/3@41"));
    assert_eq!(field(report, "current_eligibility").as_str(), Some("not_observed"));
    assert_eq!(field(report, "blocker_completeness").as_str(), Some("recorded_conditions_only"));
    assert_eq!(field(report, "effect_authority").as_bool(), Some(false));
    let knowledge = field(report, "knowledge");
    let basis = field(knowledge, "basis");
    assert_eq!(field(basis, "request_generation").as_str(), Some("0"));
    assert_eq!(
        field(basis, "journal_revision").as_str(),
        Some(value.basis().journal_revision.to_string().as_str())
    );
    let predicates = field(field(knowledge, "value"), "predicates").as_array().unwrap();
    let expected = [
        "recorded_admission_open",
        "recorded_not_suspended",
        "recorded_policy_epoch_matches",
        "recorded_deadline_open",
        "recorded_stage_authorized",
        "dispatch_not_recorded",
        "current_source_evidence",
        "current_approvals",
        "current_clock",
        "final_publication_eligibility",
    ];
    assert_eq!(predicates.len(), expected.len());
    for (index, (predicate, expected)) in predicates.iter().zip(expected).enumerate() {
        assert_eq!(field(predicate, "condition").as_str(), Some(expected));
        let truth = field(predicate, "satisfied");
        if index < 6 {
            assert_eq!(field(truth, "state").as_str(), Some("known"));
            assert_eq!(field(truth, "basis"), basis);
            assert!(field(truth, "value").as_bool().is_some());
        } else {
            assert_eq!(field(truth, "state").as_str(), Some("unknown"));
            assert_eq!(field(truth, "reason").as_str(), Some("controller_unavailable"));
        }
    }
    let json = response.to_json().unwrap();
    for private in ["PRIVATE-SOURCE-BYTES", "published", "Approve this exact publication?", "exact-view-file-v1"] {
        assert!(!json.contains(private), "private bytes leaked: {private}");
    }
    assert!(json.len() <= PublicationLimits::default().max_response_bytes);
    assert_eq!(response.to_json().unwrap(), json);
    assert_eq!(grant.remaining_queries(), 0);
    let mut too_small = response;
    too_small.envelope.limits.max_response_bytes = 1;
    assert_eq!(too_small.to_json(), Err(TowerEncodingError::OutputLimit));
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn sealed_query_and_registration_reject_relabeling_for_known_unknown_withheld_and_capacity() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let mut facade = tower(actor);
    let (port, grant) = supervisor.hold_diagnosis_port(2).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    let known_response = diagnose(&mut facade, 701);
    known(&known_response);
    let unknown_response = diagnose(&mut facade, 999);
    unknown(&unknown_response);
    let exhausted = diagnose(&mut facade, 701);
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    grant.revoke();
    let revoked = diagnose(&mut facade, 701);
    assert!(matches!(native(&revoked), Ok(ActorKnowledge::Withheld { .. })));
    for response in [known_response, unknown_response, exhausted, revoked] {
        response.to_json().unwrap();
        let mut subjects = Vec::new();
        let mut changed = response.envelope.address.clone();
        changed.generation = Some(REGISTRATION + 1);
        subjects.push(changed);
        let mut changed = response.envelope.address.clone();
        changed.generation = None;
        subjects.push(changed);
        let mut changed = response.envelope.address.clone();
        changed.id.push('1');
        subjects.push(changed);
        let mut changed = response.envelope.address.clone();
        changed.tenant = "2".to_owned();
        subjects.push(changed);
        for changed in subjects {
            let mut relabeled = response.clone();
            relabeled.envelope.address = changed;
            assert_eq!(relabeled.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
    }

    // Omitting a generation in the original read is supported. It does not
    // permit an encoder to add an allegedly observed generation afterwards.
    let (port, _grant) = supervisor.hold_diagnosis_port(1).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    let mut unversioned = request(&facade, 701);
    unversioned.address.generation = None;
    let unversioned = facade.dispatch(&access(true), unversioned);
    known(&unversioned);
    unversioned.to_json().unwrap();
    let mut relabeled = unversioned;
    relabeled.envelope.address.generation = Some(REGISTRATION);
    assert_eq!(relabeled.to_json(), Err(TowerEncodingError::InvalidKnowledge));
}

fn imported_native_decoy(address: &Address) -> fa_reference::tower::ReferenceTower {
    let declared = ScopedProof::Declared { scope_id: 71, generation: 7 };
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
        DomainEventJournal::new_production("1", "retained-inspection"),
        basis,
        TowerLimits::default(),
        4,
    )
    .unwrap();
    let mut decoy = address.clone();
    decoy.generation = Some(7);
    builder
        .insert_object(
            ResolvedObject {
                plane: decoy.kind.plane(),
                address: decoy,
                summary_digest: 0xdec0_1ded,
                generation: 7,
            },
            basis,
        )
        .unwrap();
    builder.build()
}

#[test]
fn native_router_never_falls_back_to_imported_evidence_when_diagnosis_is_unavailable() {
    let root = Directory::new();
    let (actor, mut supervisor, _, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let mut facade = tower(actor);
    let query = request(&facade, 701);
    let reference = imported_native_decoy(&query.address);
    let (port, grant) = supervisor.hold_diagnosis_port(3).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    let mut router = TowerRouter::with_publication(reference, facade).unwrap();
    let RoutedResponse::Publication(live) = router.dispatch(&access(true), query.clone()) else {
        panic!("native diagnosis selected an imported route");
    };
    known(&live);
    drop(supervisor);
    let RoutedResponse::Publication(lost) = router.dispatch(&access(true), query.clone()) else {
        panic!("owner loss selected an imported fallback");
    };
    unknown(&lost);
    assert_eq!(grant.remaining_queries(), 2);
    grant.revoke();
    let RoutedResponse::Publication(revoked) = router.dispatch(&access(true), query.clone()) else {
        panic!("revocation selected an imported fallback");
    };
    assert!(matches!(native(&revoked), Ok(ActorKnowledge::Withheld { .. })));
    let mut stale = query;
    stale.address.generation = Some(7);
    let RoutedResponse::Publication(stale) = router.dispatch(&access(true), stale) else {
        panic!("stale native registration selected imported generation");
    };
    refused(&stale, ReasonCode::StaleGeneration);
}

#[test]
fn unknown_and_explicitly_abandoned_effects_remain_charged_with_no_new_dispatch_advice() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    let ticket = actor.submit(701, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, 701);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.hold_diagnosis_port(2).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.fence(revision).unwrap();
    }
    let before = image(&root);
    let unresolved = diagnose(&mut facade, 701);
    let value = known(&unresolved);
    assert_eq!(value.stage(), ActionState::Unknown);
    assert_eq!(fact(value, FileHoldCondition::DispatchNotRecorded), Some(false));
    no_live_permission(value);
    assert_eq!(image(&root), before);
    {
        let mut host = supervisor.host_mut().unwrap();
        let control = host.inspect().control;
        let revision = host.revision();
        host.abandon_unknown(
            revision,
            fa_reference::action::consequence::delivery::persistent::abandonment::FileAbandonRequest {
                operation: 900,
                attempt: keys.automatic.attempt(),
                expected_control_sequence: control.sequence,
                expected_authority_epoch: control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let before = image(&root);
    let abandoned = diagnose(&mut facade, 701);
    let value = known(&abandoned);
    assert_eq!(value.stage(), ActionState::IrrecoverablyUnknown);
    assert_eq!(fact(value, FileHoldCondition::DispatchNotRecorded), Some(false));
    no_live_permission(value);
    assert_eq!(image(&root), before);
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 16);
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Unknown { reason: ActorUnknown::OutcomeUnknown }
    );
    assert_eq!(grant.remaining_queries(), 0);
}
