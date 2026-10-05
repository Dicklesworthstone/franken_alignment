//! Original generated frames, durable identity and confirmed audience boundaries.
use super::*;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileLearnedTextStreamActorPort, LearnedTextRelease};
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;

pub(super) fn stream_profile() -> StreamProfile { StreamProfile::new(61, 1, 8, 32, 128).unwrap() }
pub(super) fn stream_config() -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    FileLearnedConfig::new_text_stream(model, tokenizer, source,
        LearnedDecoderBindingLimits::default(), stream_profile()).unwrap().with_required_sidecar().unwrap()
}
pub(super) fn stream_delivery_profile() -> FileOversightProfile {
    let mut profile = profile();
    profile.delivery.initial_payload.clear();
    profile.delivery.total = 1000;
    profile
}
pub(super) fn stream_owner(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(
        root.store(), stream_delivery_profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..3 { step(&mut host); }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    (host, reviewer)
}

#[test]
fn source_only_stream_derives_complete_frame_and_full_cost_but_cannot_finish_early() {
    let root = Directory::new(); let (host, _) = stream_owner(&root, &stream_config());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    assert!(matches!(port.submit(71, LearnedTextRelease::Finish, ElapsedTick(100)), Err(ActorError::Unavailable)));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    // Failed premature finish did not consume the one-shot observation.
    let ticket = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let host = supervisor.host().unwrap();
    let action = host.request_action(71).unwrap();
    let frame = ReleaseFrame::decode(&action.spec().payload).unwrap();
    assert_eq!(frame.message(), Some("aa")); assert!(frame.prior_messages().is_empty());
    assert_eq!(frame.profile(), stream_profile());
    assert_eq!(action.spec().units, action.spec().payload.len() as u64);
    assert!(action.spec().units > 2, "not merely the generated text's byte count");
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.stream_snapshot().unwrap().published.visible().is_empty());
    assert_eq!(host.inspect().executions, 0);
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
}

#[test]
fn stream_retry_binds_kind_and_deadline_without_spending_next_observation() {
    let root = Directory::new(); let (host, _) = stream_owner(&root, &stream_config());
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    let ticket = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    assert!(matches!(port.submit(71, LearnedTextRelease::Finish, ElapsedTick(100)), Err(ActorError::IdempotencyConflict)));
    assert!(matches!(port.submit(71, LearnedTextRelease::Message, ElapsedTick(99)), Err(ActorError::IdempotencyConflict)));
    assert_eq!(port.poll(&port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap()), port.poll(&ticket));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    // No dispatch exists: another proposal can enter the original gate, but it
    // still has no effect rights and cannot append without its own review.
    assert!(matches!(port.poll(&port.submit(72, LearnedTextRelease::Message, ElapsedTick(100)).unwrap()), Knowledge::Pending { .. }));
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn recovered_stream_request_does_not_resume_source_or_turn_cancellation_into_finish() {
    let root = Directory::new(); let config = stream_config(); let (host, _) = stream_owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    let ticket = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    port.cancel(&ticket).unwrap(); drop(supervisor);
    let (host, _) = FileOversight::open_with_learned_text_stream(root.store(), stream_delivery_profile(), &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    let revision = host.revision();
    let (reopened, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let retry = reopened.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert!(matches!(reopened.poll(&retry), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert!(matches!(reopened.poll(&ticket), Knowledge::Withheld { .. }));
    observe(&mut supervisor, snapshot());
    assert!(matches!(reopened.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)), Err(ActorError::Unavailable)));
    let host = supervisor.host().unwrap();
    assert_eq!(host.revision(), revision);
    let view = host.stream_snapshot().unwrap();
    assert!(view.published.visible().is_empty() && !view.published.finished());
    assert_eq!(view.publication.executions, 0);
}

#[test]
fn raw_and_stream_source_ports_are_not_interchangeable() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    assert!(matches!(host.into_learned_text_stream_actor_gateway(), Err(JournalError::Contract(Error::Binding))));
    let root = Directory::new(); let (host, _) = stream_owner(&root, &stream_config());
    assert!(matches!(host.into_learned_text_actor_gateway(), Err(JournalError::Contract(Error::Binding))));
    let root = Directory::new(); let (host, _) = stream_owner(&root, &stream_config());
    let (port, mut supervisor): (FileLearnedTextStreamActorPort, _) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    assert!(matches!(port.poll(&port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap()), Knowledge::Pending { .. }));
}

#[test]
fn all_source_stream_intake_storage_barriers_return_no_ticket_and_recovery_never_publishes() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = stream_config(); let (host, _) = stream_owner(&root, &config);
        let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot()); supervisor.host().unwrap().store.fail_once(barrier);
        assert!(matches!(port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)), Err(ActorError::Unavailable)));
        assert!(supervisor.host().unwrap().storage_failure().is_some());
        assert!(matches!(port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)), Err(ActorError::Unavailable)));
        drop(supervisor);
        let (host, _) = FileOversight::open_with_learned_text_stream(root.store(), stream_delivery_profile(), &config).unwrap();
        let view = host.stream_snapshot().unwrap();
        assert_eq!(view.publication.executions, 0); assert!(view.published.visible().is_empty());
        assert_eq!(view.publication.control.ledger.available, 1000);
        let recorded = host.request_status(71).is_ok();
        let revision = host.revision();
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        if recorded {
            let retry = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
            assert!(matches!(port.poll(&retry), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        } else { assert!(matches!(port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)), Err(ActorError::Unavailable))); }
        assert_eq!(supervisor.host().unwrap().revision(), revision);
    }
}
