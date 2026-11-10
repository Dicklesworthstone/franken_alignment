//! Real learned generation observed through its original weak supervisor grants.
//! Numerical weights and synthetic congress ballots are controls, not detection
//! quality or deployment qualification evidence.
#![cfg(unix)]

#[path = "tower_learned_observation/admission_explanation.rs"]
mod admission_explanation;
#[path = "support/learned_text_model.rs"]
mod numerical;
#[path = "tower_learned_observation/refinement_explanation.rs"]
mod refinement_explanation;
#[path = "tower_learned_observation/review_explanation.rs"]
mod review_explanation;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::learned::FileLearnedConfig,
        },
        requests::actor::{
            FileHoldCondition, FileHoldDiagnosis, FileInspectionPage,
            FileLearnedTextStreamActorPort, LearnedTextRelease,
        },
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorError, ActorOutcome, ActorProposal, Knowledge as ActorKnowledge},
    decoder_monitoring::LearnedDecoderBindingLimits,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::observation_algebra::Knowledge;
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use support::{Directory, Supervisor, observe, oversight, snapshot};

fn profile() -> FileOversightProfile {
    let mut profile = support::profile();
    profile.delivery.total = 4096;
    profile.delivery.initial_payload.clear();
    profile.delivery.actor = ActorState::new(
        RestartProfile {
            id: 1,
            generation: 1,
            host_generation: 1,
            model_generation: 3,
            tokenizer_generation: 4,
            state_schema_generation: 1,
            grade: RestartGrade::AuditOnly,
        },
        Vec::new(),
        vec![0],
        vec![0],
        0,
    )
    .unwrap();
    profile
}

fn recipe() -> FileLearnedConfig {
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    FileLearnedConfig::new_text_stream(
        model.clone(),
        numerical::tokenizer(&model),
        numerical::config(&model),
        LearnedDecoderBindingLimits::default(),
        StreamProfile::new(1, 1, 4, 64, 128).unwrap(),
    )
    .unwrap()
}

fn create(
    root: &Directory,
) -> (
    FileLearnedTextStreamActorPort,
    Supervisor,
    FileHumanReviewer,
) {
    let (mut host, reviewer) =
        FileOversight::create_with_learned_text_stream(root.store(), profile(), recipe()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    (actor, supervisor, reviewer)
}

fn generate(supervisor: &mut Supervisor) {
    let mut host = supervisor.host_mut().unwrap();
    for _ in 0..4 {
        let state = host.learned_generation_inspection().unwrap().numerical;
        if !state.status.is_active() {
            return;
        }
        let revision = host.revision();
        host.advance_learned_generation(revision, state.actor_revision, state.position)
            .unwrap()
            .unwrap();
    }
    assert!(
        !host
            .learned_generation_inspection()
            .unwrap()
            .numerical
            .status
            .is_active()
    );
}

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
}

fn tower(actor: FileLearnedTextStreamActorPort) -> PublicationTower {
    PublicationTower::new_learned_observer(mount(), actor, PublicationLimits::default()).unwrap()
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        Vec::new(),
        0,
    )
}

fn request() -> TowerRequest {
    TowerRequest::new("learned-observation", mount(), Verb::Situation)
}

fn inspect(tower: &mut PublicationTower) -> PublicationResponse {
    tower.exchange(
        &access(true),
        encode_request(&request()).unwrap().as_bytes(),
    )
}

fn known(response: &PublicationResponse) -> &FileInspectionPage {
    let PublicationResult::Situation(Ok(ActorKnowledge::Known { value, basis })) = &response.result
    else {
        panic!("expected original native situation: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(response.envelope.reason_code, None);
    assert!(matches!(
        response.envelope.control_seq,
        Knowledge::Unknown { .. }
    ));
    value
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

#[test]
fn learned_observer_follows_original_numerical_intake_and_two_key_publication() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    facade.set_inspection(Some(port)).unwrap();

    let generation = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let original = image(&root);
    let before = inspect(&mut facade);
    assert_eq!(known(&before).summary().attempts, 0);
    assert_eq!(image(&root), original);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generation
    );

    generate(&mut supervisor);
    observe(&mut supervisor);
    let generated = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let original = image(&root);
    let generated_read = inspect(&mut facade);
    assert_eq!(known(&generated_read).summary().executions, 0);
    assert_eq!(image(&root), original);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generated
    );

    // Observation leaves the original one-use source snapshot and learned
    // release gateway intact; only that gateway derives the cumulative bytes.
    let ticket = actor
        .submit(701, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: 701 }
    );
    let pending = inspect(&mut facade);
    let page = known(&pending);
    assert_eq!(page.summary().pending_attempts, 1);
    assert_eq!(page.attempts()[0].external_request, Some(701));
    assert_eq!(page.attempts()[0].stage, ActionState::Reviewing);
    assert_eq!(page.summary().executions, 0);

    let keys = support::ready(&mut supervisor, &reviewer, 701);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        let result = host
            .publish_checked(
                revision,
                keys.automatic.attempt(),
                Some(&keys.inputs),
                snapshot(),
                ElapsedTick(2),
            )
            .unwrap();
        assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let original = image(&root);
    let completed = inspect(&mut facade);
    let page = known(&completed);
    assert_eq!(page.summary().executions, 1);
    assert_eq!(page.attempts()[0].stage, ActionState::Confirmed);
    assert_eq!(page.summary().charged_units, keys.action.spec().units);
    assert_eq!(page.summary().unresolved_attempts, 0);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(image(&root), original);
    assert!(matches!(
        actor.poll(&ticket),
        ActorKnowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    assert_eq!(known(&pending).attempts()[0].stage, ActionState::Reviewing);
}

#[test]
fn learned_observer_never_acquires_arbitrary_intake_even_with_broad_host_tags() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer) = create(&root);
    let mut facade = tower(actor.clone());
    let (port, grant) = supervisor.inspection_port(1).unwrap();
    facade.set_inspection(Some(port)).unwrap();
    generate(&mut supervisor);
    observe(&mut supervisor);
    let state = supervisor.host().unwrap().inspect();
    let original = image(&root);
    let mut proposal = TowerRequest::new("injected", mount(), Verb::Propose);
    proposal.idempotency_key = Some("701".into());
    proposal.arguments = ReadArguments::Propose {
        request: 701,
        proposal: ActorProposal {
            target: state.target,
            payload: b"actor-selected injection".to_vec(),
            units: 32,
            deadline: ElapsedTick(100),
            expected_policy_epoch: state.control.ledger.epoch,
        },
    };
    let broad = TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        true,
        vec![PurposeTag::Permit, PurposeTag::Experiment],
        1024,
    );
    for response in [
        facade.dispatch(&broad, proposal.clone()),
        facade.exchange(&broad, encode_request(&proposal).unwrap().as_bytes()),
    ] {
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::CapabilityUnsupported)
        );
        assert!(matches!(response.result, PublicationResult::Refused(_)));
    }
    for verb in Verb::ALL {
        if !matches!(
            verb,
            Verb::Situation | Verb::Explain | Verb::WhyHeld | Verb::PolicyReplay
        ) {
            let response = facade.dispatch(&broad, TowerRequest::new("closed", mount(), verb));
            assert_eq!(
                response.envelope.reason_code,
                Some(ReasonCode::CapabilityUnsupported)
            );
        }
    }
    let denied = facade.dispatch(&access(false), request());
    assert_eq!(
        denied.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(image(&root), original);
    let ticket = actor
        .submit(701, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: 701 }
    );
    assert_eq!(
        known(&inspect(&mut facade)).attempts()[0].external_request,
        Some(701)
    );
}

#[test]
fn learned_observers_share_original_read_work_and_revoke_without_retaining_owner() {
    let root = Directory::new();
    let (actor, supervisor, _reviewer) = create(&root);
    let mut first = tower(actor.clone());
    let mut second = tower(actor);
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    first.set_inspection(Some(port.clone())).unwrap();
    second.set_inspection(Some(port)).unwrap();
    assert_eq!(known(&inspect(&mut first)).summary().attempts, 0);
    assert_eq!(known(&inspect(&mut second)).summary().attempts, 0);
    assert_eq!(grant.remaining_queries(), 0);
    assert!(matches!(
        inspect(&mut first).result,
        PublicationResult::Situation(Err(ActorError::Capacity))
    ));

    let (port, live_grant) = supervisor.inspection_port(2).unwrap();
    first.set_inspection(Some(port)).unwrap();
    live_grant.revoke();
    assert!(matches!(
        inspect(&mut first).result,
        PublicationResult::Situation(Ok(ActorKnowledge::Withheld { .. }))
    ));

    let (port, retained_grant) = supervisor.inspection_port(2).unwrap();
    first.set_inspection(Some(port)).unwrap();
    assert_eq!(known(&inspect(&mut first)).summary().attempts, 0);
    drop(supervisor);
    assert!(matches!(
        inspect(&mut first).result,
        PublicationResult::Situation(Ok(ActorKnowledge::Unknown { .. }))
    ));
    assert_eq!(retained_grant.remaining_queries(), 1);
    let (reopened, _reviewer) =
        FileOversight::open_with_learned_text_stream(root.store(), profile(), &recipe()).unwrap();
    let (_actor, reopened_supervisor) = reopened.into_learned_text_stream_actor_gateway().unwrap();
    let (foreign, _foreign_grant) = reopened_supervisor.inspection_port(1).unwrap();
    assert_eq!(
        first.set_inspection(Some(foreign)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        inspect(&mut first).envelope.reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
}

#[test]
fn learned_observer_binds_read_grants_to_original_allocation_and_registration() {
    let root = Directory::new();
    let foreign_root = Directory::new();
    let (actor, supervisor, _reviewer) = create(&root);
    let (_foreign_actor, foreign, _reviewer) = create(&foreign_root);
    assert!(matches!(
        PublicationTower::new_learned_observer(
            Address::new("1", Noun::Run, "30", Some(41)).unwrap(),
            actor.clone(),
            PublicationLimits::default(),
        ),
        Err(TowerInstallError::SnapshotMismatch)
    ));
    let mut facade = tower(actor);
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    facade.set_inspection(Some(port)).unwrap();
    let mut stale = request();
    stale.address.generation = Some(42);
    assert_eq!(
        facade.dispatch(&access(true), stale).envelope.reason_code,
        Some(ReasonCode::StaleGeneration)
    );
    let denied = TowerAccess::new(CallerScope::full_tenant_access("2"), true, Vec::new(), 0);
    assert_eq!(
        facade.dispatch(&denied, request()).envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(known(&inspect(&mut facade)).summary().attempts, 0);
    let (port, _grant) = foreign.inspection_port(1).unwrap();
    assert_eq!(
        facade.set_inspection(Some(port)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        inspect(&mut facade).envelope.reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
}

#[test]
fn numerical_explanation_keeps_original_unknown_and_spent_work() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    generate(&mut supervisor);
    observe(&mut supervisor);
    let _ticket = actor
        .submit(701, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    // The requested congress really completed. Unknown cannot be explained
    // by an absent round or nonexistent admission: these grants lack the
    // numerical replay work contract even for this approved attempt.
    let _keys = support::ready(&mut supervisor, &reviewer, 701);
    let mut facade = tower(actor);
    let (admission, admission_grant) = supervisor.explanation_port(1).unwrap();
    let (review, review_grant) = supervisor.review_explanation_port(1).unwrap();
    facade.set_explanation(Some(admission)).unwrap();
    facade.set_review_explanation(Some(review)).unwrap();
    let original = image(&root);
    let state = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let mut request = TowerRequest::new(
        "numerical-history",
        facade.request_address(701).unwrap(),
        Verb::Explain,
    );
    let response = facade.dispatch(&access(true), request.clone());
    assert!(matches!(
        response.result,
        PublicationResult::Explanation(Ok(ActorKnowledge::Unknown { .. }))
    ));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::LowerLayerUnverified)
    );
    request.arguments = ReadArguments::ReviewExplanation { round: 801 };
    let response = facade.dispatch(&access(true), request);
    let PublicationResult::ReviewExplanation(linked) = response.result else {
        panic!("expected original completed-review profile");
    };
    assert!(matches!(
        linked.native(),
        Ok(ActorKnowledge::Unknown { .. })
    ));
    assert_eq!(admission_grant.remaining_queries(), 0);
    assert_eq!(review_grant.remaining_queries(), 0);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        state
    );
    assert_eq!(image(&root), original);
}

fn diagnosed(response: &PublicationResponse) -> &FileHoldDiagnosis {
    let PublicationResult::HoldDiagnosis(linked) = &response.result else {
        panic!("expected original learned hold diagnosis: {response:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = linked.native() else {
        panic!("expected a known recorded learned state: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(basis.request, 701);
    assert_ne!(basis.attempt, 701);
    assert_eq!(linked.query_address(), &response.envelope.address);
    assert_eq!(linked.registration(), &mount());
    assert_eq!(response.envelope.reason_code, None);
    for condition in [
        FileHoldCondition::CurrentSourceEvidence,
        FileHoldCondition::CurrentApprovals,
        FileHoldCondition::CurrentClock,
        FileHoldCondition::FinalPublicationEligibility,
    ] {
        let predicate = value
            .predicates()
            .iter()
            .find(|predicate| predicate.condition() == condition)
            .unwrap();
        assert!(matches!(
            predicate.satisfied(),
            ActorKnowledge::Unknown { .. }
        ));
    }
    value
}

#[test]
fn learned_hold_diagnosis_observes_original_message_without_advancing_or_approving_it() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    let mut facade = tower(actor.clone());
    let query = TowerRequest::new(
        "learned-recorded-hold",
        facade.request_address(701).unwrap(),
        Verb::WhyHeld,
    );
    assert_eq!(
        facade
            .dispatch(&access(true), query.clone())
            .envelope
            .reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
    let (port, grant) = supervisor.hold_diagnosis_port(5).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();

    // Real numerical execution produces the original cumulative Message bytes.
    // Inspecting the not-yet-submitted request must leave both that learned
    // state and the already supplied one-use source snapshot available.
    generate(&mut supervisor);
    observe(&mut supervisor);
    let generation = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let state = supervisor.host().unwrap().inspect();
    let original = image(&root);
    let missing = facade.dispatch(&access(true), query.clone());
    let PublicationResult::HoldDiagnosis(linked) = &missing.result else {
        panic!("missing learned request did not use its native diagnosis");
    };
    assert!(matches!(
        linked.native(),
        Ok(ActorKnowledge::Unknown { .. })
    ));
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generation
    );
    assert_eq!(
        facade
            .dispatch(&access(false), query.clone())
            .envelope
            .reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    let broad = TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        true,
        vec![PurposeTag::Permit, PurposeTag::Experiment],
        1024,
    );
    let mut poll = query.clone();
    poll.verb = Verb::Get;
    assert_eq!(
        facade.dispatch(&broad, poll).envelope.reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
    assert_eq!(grant.remaining_queries(), 4);

    let ticket = actor
        .submit(701, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    let generation = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let state = supervisor.host().unwrap().inspect();
    let original = image(&root);
    let pending = facade.exchange(&access(true), encode_request(&query).unwrap().as_bytes());
    assert_eq!(diagnosed(&pending).stage(), ActionState::Reviewing);
    assert_eq!(diagnosed(&pending).basis().request_generation, 0);
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: 701 }
    );
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generation
    );

    let keys = support::ready(&mut supervisor, &reviewer, 701);
    let original = image(&root);
    let authorized = facade.dispatch(&access(true), query.clone());
    assert_eq!(diagnosed(&authorized).stage(), ActionState::Authorized);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    assert_eq!(image(&root), original);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        let result = host
            .publish_checked(
                revision,
                keys.automatic.attempt(),
                Some(&keys.inputs),
                snapshot(),
                ElapsedTick(2),
            )
            .unwrap();
        assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let generation = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let state = supervisor.host().unwrap().inspect();
    let original = image(&root);
    let completed = facade.dispatch(&access(true), query.clone());
    assert_eq!(diagnosed(&completed).stage(), ActionState::Confirmed);
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.charged, keys.action.spec().units);
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        generation
    );
    assert!(matches!(
        actor.poll(&ticket),
        ActorKnowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    assert_eq!(diagnosed(&pending).stage(), ActionState::Reviewing);
    assert_eq!(grant.remaining_queries(), 1);
    let json = completed.to_json().unwrap();
    assert!(json.contains("\"schema\":\"fa.publication-hold-diagnosis/1\""));
    assert!(json.contains("\"current_eligibility\":\"not_observed\""));
    assert!(!json.contains("PRIVATE-SOURCE-BYTES"));

    grant.revoke();
    let revoked = facade.dispatch(&access(true), query.clone());
    let PublicationResult::HoldDiagnosis(linked) = &revoked.result else {
        panic!("revocation fell back to another observation");
    };
    assert!(matches!(
        linked.native(),
        Ok(ActorKnowledge::Withheld { .. })
    ));
    assert_eq!(image(&root), original);
    let (port, retained_grant) = supervisor.hold_diagnosis_port(1).unwrap();
    facade.set_hold_diagnosis(Some(port)).unwrap();
    drop(supervisor);
    let lost = facade.dispatch(&access(true), query);
    let PublicationResult::HoldDiagnosis(linked) = &lost.result else {
        panic!("owner loss fell back to another observation");
    };
    assert!(matches!(
        linked.native(),
        Ok(ActorKnowledge::Unknown { .. })
    ));
    assert_eq!(retained_grant.remaining_queries(), 1);
    assert_eq!(image(&root), original);
}
