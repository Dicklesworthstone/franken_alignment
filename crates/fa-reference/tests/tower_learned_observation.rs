//! Real learned generation observed through its original weak supervisor grants.
//! Numerical weights and synthetic congress ballots are controls, not detection
//! quality or deployment qualification evidence.
#![cfg(unix)]

#[path = "support/learned_text_model.rs"]
mod numerical;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::learned::FileLearnedConfig,
        },
        requests::actor::{FileInspectionPage, FileLearnedTextStreamActorPort, LearnedTextRelease},
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
        if !matches!(verb, Verb::Situation | Verb::Explain | Verb::PolicyReplay) {
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
