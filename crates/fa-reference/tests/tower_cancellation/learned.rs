//! The learned control mount uses the same source-only owner and independent
//! numerical work grant. No ordinary payload/ticket path is exposed.
use super::{
    Directory, OPERATION, REQUEST, Supervisor, access, action, image, mount, native, next, plan,
    receipt, refused, support,
};
use fa_reference::action::consequence::delivery::{
    persistent::{
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::learned::FileLearnedConfig,
        },
        requests::actor::{
            FileCancellationBlockReason, FileLearnedTextStreamActorPort, LearnedTextRelease,
            MAX_CANCELLATION_REPLAY_EVENTS,
        },
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, Knowledge as ActorKnowledge},
    decoder_monitoring::LearnedDecoderBindingLimits,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::tower::publication::{PublicationLimits, PublicationTower};
use fa_reference::tower::{ReadArguments, ReasonCode, TowerRequest, Verb, encode_request};

#[path = "../support/learned_text_model.rs"]
mod numerical;

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
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
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
            break;
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

fn tower(actor: FileLearnedTextStreamActorPort) -> PublicationTower {
    PublicationTower::new_learned_control_operator(mount(), actor, PublicationLimits::default())
        .unwrap()
}

#[test]
fn learned_source_cancellation_needs_a_control_mount_and_its_own_numerical_reservation() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    generate(&mut supervisor);
    support::observe(&mut supervisor);
    let ticket = actor
        .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    support::ready(&mut supervisor, &reviewer, REQUEST);
    let mut facade = tower(actor.clone());
    let original = image(&root);
    let before = supervisor.host().unwrap().inspect();
    let replay_events = before.revision as usize + 1;
    let (default_port, default_grant) = supervisor
        .cancellation_port(4, 2, MAX_CANCELLATION_REPLAY_EVENTS)
        .unwrap();
    facade.set_cancellation(Some(default_port)).unwrap();
    let unsupported = next(&mut facade, OPERATION);
    assert_eq!(
        plan(&unsupported).block_reason(),
        Some(FileCancellationBlockReason::NumericalReplayUnsupported)
    );
    assert!(!plan(&unsupported).can_cancel());
    assert_eq!(default_grant.remaining_operations(), 2);
    assert_eq!(image(&root), original);

    let total = 2 * MAX_CANCELLATION_REPLAY_EVENTS;
    let (port, grant) = supervisor
        .cancellation_port_with_numerical_budget(8, 2, MAX_CANCELLATION_REPLAY_EVENTS, total)
        .unwrap();
    let mut observer = PublicationTower::new_learned_observer(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    observer.set_cancellation(Some(port.clone())).unwrap();
    refused(
        &next(&mut observer, OPERATION),
        ReasonCode::CapabilityUnsupported,
    );
    let mut legacy = PublicationTower::new_learned_reconciliation_operator(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    legacy.set_cancellation(Some(port.clone())).unwrap();
    refused(
        &next(&mut legacy, OPERATION),
        ReasonCode::CapabilityUnsupported,
    );
    facade.set_cancellation(Some(port.clone())).unwrap();
    let proposal = support::proposal(&support::oversight::spec(
        &supervisor.host().unwrap(),
        b"caller-supplied",
    ));
    let mut submission = TowerRequest::new("no-learned-payload", mount(), Verb::Propose);
    submission.arguments = ReadArguments::Propose {
        request: REQUEST + 1,
        proposal,
    };
    submission.idempotency_key = Some((REQUEST + 1).to_string());
    refused(
        &facade.exchange(
            &access(true, true),
            encode_request(&submission).unwrap().as_bytes(),
        ),
        ReasonCode::CapabilityUnsupported,
    );
    let poll = TowerRequest::new(
        "no-learned-ticket",
        facade.request_address(REQUEST).unwrap(),
        Verb::Get,
    );
    refused(
        &facade.dispatch(&access(true, true), poll),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 8);
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total));
    assert_eq!(image(&root), original);

    let proposed = next(&mut facade, OPERATION);
    assert!(plan(&proposed).can_cancel());
    let command = action(&proposed);
    let wire = encode_request(&command).unwrap();
    let completed = facade.exchange(&access(true, true), wire.as_bytes());
    assert_eq!(receipt(&completed).stage(), ActionState::Cancelled);
    assert_eq!(receipt(&completed).before(), plan(&proposed).basis());
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.charged,
        before.control.ledger.charged
    );
    assert!(matches!(
        actor.poll(&ticket),
        ActorKnowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        }
    ));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(total - replay_events)
    );
    let committed = image(&root);
    let mut reconnected = tower(actor);
    reconnected.set_cancellation(Some(port)).unwrap();
    assert_eq!(
        native(&reconnected.exchange(&access(true, true), wire.as_bytes())),
        native(&completed)
    );
    assert_eq!(image(&root), committed);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(total - replay_events)
    );
}
