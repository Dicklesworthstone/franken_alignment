//! Learned narrowing requires its own explicit numerical replay allowance.
use super::{
    Directory, OPERATION, Supervisor, access, action, image, mount, native, next, plan,
    query, receipt, refused, support,
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
fn learned_observers_and_unpaid_control_cannot_narrow_but_paid_original_control_can() {
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
    assert_eq!(plan(&proposed).block_reason(), Some(FileNarrowBlockReason::NumericalReplayUnsupported));
    assert!(proposed.to_json().unwrap().contains("numerical_replay_unsupported"));
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

    let before = supervisor.host().unwrap().inspect();
    let generation = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let stream = supervisor.host().unwrap().stream_snapshot().unwrap();
    let events = before.revision as usize + 1;
    let (paid, paid_grant) = supervisor.narrow_port_with_numerical_budget(2, 1, events, events).unwrap();
    control.set_narrow(Some(paid)).unwrap();
    let offered = next(&mut control, OPERATION, vec![target]);
    assert!(plan(&offered).can_narrow());
    let command = action(&offered);
    // The same exact command still cannot turn an observation mount into control.
    refused(&observer.dispatch(&access(true, true), command.clone()), ReasonCode::CapabilityUnsupported);
    assert_eq!(paid_grant.remaining_numerical_replay_events(), Some(events));
    let narrowed = control.dispatch(&access(true, true), command.clone());
    let acknowledged = receipt(&narrowed).clone();
    assert_eq!(acknowledged.effective_targets(), &[target]);
    assert!(acknowledged.cancelled().is_empty());
    assert_eq!(acknowledged.refunded_units(), 0);
    assert_eq!(paid_grant.remaining_operations(), 0);
    assert_eq!(paid_grant.remaining_numerical_replay_events(), Some(0));
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.sequence, before.control.sequence + 1);
    assert_eq!(after.control.ledger, before.control.ledger);
    assert_eq!(after.executions, 0);
    let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    assert_eq!(numerical.numerical, generation.numerical);
    assert_eq!(numerical.pending, generation.pending);
    assert_eq!(numerical.paused, generation.paused);
    let current = supervisor.host().unwrap().stream_snapshot().unwrap();
    assert_eq!(current.confirmed, stream.confirmed);
    assert_eq!(current.published, stream.published);
    assert_eq!(current.pending, stream.pending);
    assert_eq!(current.confirmed_target, stream.confirmed_target);
    assert_eq!(supervisor.host().unwrap().learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), message);
    let saved = image(&root);
    support::observe(&mut supervisor);
    assert_eq!(receipt(&control.dispatch(&access(true, true), command)), &acknowledged);
    assert_eq!(image(&root), saved);
    assert_eq!(supervisor.host().unwrap().inspect(), after);
    assert_eq!(paid_grant.remaining_numerical_replay_events(), Some(0));
}
