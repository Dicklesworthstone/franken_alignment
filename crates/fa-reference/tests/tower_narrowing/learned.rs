//! Ordinary narrowing cannot bypass the original learned stream contract.
use super::{
    Directory, OPERATION, Supervisor, access, image, mount, native, next, plan,
    query, refused, support,
};
use fa_reference::action::consequence::delivery::{
    persistent::{
        observed::{FileOversight, FileOversightProfile, decoder::learned::FileLearnedConfig},
        requests::actor::{
            FileNarrowBlockReason, FileNarrowError, LearnedTextRelease, MAX_NARROW_REPLAY_EVENTS,
        },
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::oversight::{
    actor::Knowledge as ActorKnowledge,
    decoder_monitoring::LearnedDecoderBindingLimits,
    learned_source::LearnedEvidenceLimits,
};
use fa_reference::action::ElapsedTick;
use fa_reference::tower::publication::{PublicationLimits, PublicationTower};
use fa_reference::tower::{ReadArguments, ReasonCode, TowerRequest, Verb};

#[path = "../support/learned_text_model.rs"]
mod numerical;

fn profile() -> FileOversightProfile {
    let mut profile = support::profile();
    profile.delivery.total = 4096;
    profile.delivery.initial_payload.clear();
    profile.delivery.actor = ActorState::new(
        RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 3,
            tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::AuditOnly },
        Vec::new(), vec![0], vec![0], 0,
    ).unwrap();
    profile
}

fn recipe() -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    FileLearnedConfig::new_text_stream(
        model.clone(), numerical::tokenizer(&model), numerical::config(&model),
        LearnedDecoderBindingLimits::default(), StreamProfile::new(1, 1, 4, 64, 128).unwrap(),
    ).unwrap()
}

fn generate(supervisor: &mut Supervisor) {
    let mut host = supervisor.host_mut().unwrap();
    for _ in 0..4 {
        let state = host.learned_generation_inspection().unwrap().numerical;
        if !state.status.is_active() { break; }
        let revision = host.revision();
        host.advance_learned_generation(revision, state.actor_revision, state.position).unwrap().unwrap();
    }
    assert!(!host.learned_generation_inspection().unwrap().numerical.status.is_active());
}

#[test]
fn learned_observers_and_unsupported_control_history_cannot_import_ordinary_narrowing() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create_with_learned_text_stream(root.store(), profile(), recipe()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    generate(&mut supervisor);
    let target = supervisor.host().unwrap().inspect().target;
    let (port, grant) = supervisor.narrow_port(4, 2, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let mut observer = PublicationTower::new_learned_observer(
        mount(), actor.clone(), PublicationLimits::default(),
    ).unwrap();
    observer.set_narrow(Some(port.clone())).unwrap();
    refused(&observer.dispatch(&access(true, true), query(OPERATION, vec![target])), ReasonCode::CapabilityUnsupported);
    assert_eq!(grant.remaining_queries(), 4);
    let mut control = PublicationTower::new_learned_control_operator(
        mount(), actor.clone(), PublicationLimits::default(),
    ).unwrap();
    control.set_narrow(Some(port)).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let stream = supervisor.host().unwrap().stream_snapshot().unwrap();
    let generation = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let message = supervisor.host().unwrap().learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes().to_vec();
    assert_eq!(message, b"OK");
    let saved = image(&root);
    let proposed = next(&mut control, OPERATION, vec![target]);
    assert_eq!(plan(&proposed).block_reason(), Some(FileNarrowBlockReason::UnsupportedHistory));
    assert!(!plan(&proposed).can_narrow());
    assert!(plan(&proposed).request().is_none());
    let mut command = TowerRequest::new("unsupported-ordinary-narrow", mount(), Verb::Narrow);
    command.idempotency_key = Some(OPERATION.to_string());
    command.arguments = ReadArguments::Narrow {
        operation: OPERATION, expected: plan(&proposed).basis(), retained_targets: vec![target],
    };
    refused(&observer.dispatch(&access(true, true), command.clone()), ReasonCode::CapabilityUnsupported);
    // A native unsupported refusal cannot spend or consume a source-only slot.
    support::observe(&mut supervisor);
    assert_eq!(native(&control.dispatch(&access(true, true), command)), &Err(FileNarrowError::Unsupported));
    assert_eq!((grant.remaining_queries(), grant.remaining_operations()), (3, 2));
    assert_eq!(image(&root), saved);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap(), stream);
    assert_eq!(supervisor.host().unwrap().learned_generation_inspection().unwrap(), generation);
    assert_eq!(supervisor.host().unwrap().learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), message);
    let ticket = actor.submit(701, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert!(matches!(actor.poll(&ticket), ActorKnowledge::Pending { .. }));
    assert_eq!(supervisor.host().unwrap().request_action(701).unwrap().spec().payload, b"OK");
}
