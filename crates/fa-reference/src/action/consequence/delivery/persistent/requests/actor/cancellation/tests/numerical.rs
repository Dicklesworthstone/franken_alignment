//! Original learned text and raw predictors exercise the separate replay grant.
//! Synthetic weights are deterministic controls, not detector qualification.
use super::*;
use crate::Snapshot;
use crate::action::ResolvedTarget;
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversightProfile, decoder::learned::FileLearnedConfig,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor, LearnedTextProposal, LearnedTextRelease,
};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::{
    decoder_monitoring::LearnedDecoderBindingLimits, learned_source::LearnedEvidenceLimits,
};

#[path = "../../../../observed/decoder/learned/config/text/intake/tests/fixture.rs"]
mod learned;

fn stream_profile() -> FileOversightProfile {
    let mut selected = learned::profile();
    selected.delivery.initial_payload.clear();
    selected.delivery.total = 1000;
    selected
}

fn stream_config() -> FileLearnedConfig {
    let (model, tokenizer, source) = learned::recipe();
    FileLearnedConfig::new_text_stream(
        model,
        tokenizer,
        source,
        LearnedDecoderBindingLimits::default(),
        StreamProfile::new(61, 1, 7, 32, 128).unwrap(),
    )
    .unwrap()
    .with_required_sidecar()
    .unwrap()
}

fn stream_host(root: &learned::Directory, config: &FileLearnedConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create_with_learned_text_stream(
        root.store(),
        stream_profile(),
        config.clone(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..3 {
        learned::step(&mut host);
    }
    assert_eq!(
        host.learned_text_message(LearnedEvidenceLimits::default())
            .unwrap()
            .bytes(),
        b"aa"
    );
    host
}

#[test]
fn actual_learned_stream_cancel_requires_exact_numerical_work_and_keeps_original_cleanup() {
    let mut reference = None;
    for through_port in [false, true] {
        let root = learned::Directory::new();
        let config = stream_config();
        let mut host = stream_host(&root, &config);
        // Exercise actual reconstruction, not merely the warm private cache.
        host.clear_learned_replay_cache();
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        learned::observe(&mut supervisor, learned::snapshot());
        let first = actor
            .submit(71, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        learned::observe(&mut supervisor, learned::snapshot());
        let second = actor
            .submit(72, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        let before = supervisor.host().unwrap().inspect();
        let before_numerical = supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap();
        let before_stream = supervisor.host().unwrap().stream_snapshot().unwrap();
        let events = before.revision as usize + 1;
        if through_port {
            learned::observe(&mut supervisor, learned::snapshot());
            let image = std::fs::read(root.store().join("delivery.bin")).unwrap();
            let (default, default_grant) = supervisor.cancellation_port(1, 1, events).unwrap();
            let offered = plan(default.next(71, 1201));
            assert_eq!(
                offered.block_reason(),
                Some(FileCancellationBlockReason::NumericalReplayUnsupported)
            );
            let input = FileCancellationRequest::new(1201, offered.basis()).unwrap();
            assert_eq!(
                default.cancel(input),
                Err(FileCancellationError::Unsupported)
            );
            assert_eq!(default_grant.remaining_operations(), 1);
            assert_eq!(default_grant.remaining_numerical_replay_events(), None);
            let (short, short_grant) = supervisor
                .cancellation_port_with_numerical_budget(1, 1, events, events - 1)
                .unwrap();
            assert_eq!(
                plan(short.next(71, 1201)).block_reason(),
                Some(FileCancellationBlockReason::NumericalReplayLimit)
            );
            assert_eq!(short.cancel(input), Err(FileCancellationError::Capacity));
            assert_eq!(short_grant.remaining_operations(), 1);
            assert_eq!(
                short_grant.remaining_numerical_replay_events(),
                Some(events - 1)
            );
            assert!(supervisor.owner.borrow().snapshot.is_some());
            assert_eq!(supervisor.host().unwrap().inspect(), before);
            assert_eq!(
                supervisor
                    .host()
                    .unwrap()
                    .learned_generation_inspection()
                    .unwrap(),
                before_numerical
            );
            assert_eq!(
                std::fs::read(root.store().join("delivery.bin")).unwrap(),
                image
            );
            let (exact, grant) = supervisor
                .cancellation_port_with_numerical_budget(2, 1, events, events)
                .unwrap();
            let offered = plan(exact.next(71, 1201));
            assert_eq!(offered.request(), Some(input));
            assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
            let cancelled = receipt(exact.cancel(input));
            assert_eq!(cancelled.before_stage(), ActionState::Reviewing);
            assert_eq!(cancelled.refunded_units(), 0);
            assert_eq!(grant.remaining_operations(), 0);
            assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
            assert!(supervisor.owner.borrow().snapshot.is_none());
            learned::observe(&mut supervisor, learned::snapshot());
            let after = std::fs::read(root.store().join("delivery.bin")).unwrap();
            assert_eq!(receipt(exact.clone().cancel(input)), cancelled);
            assert_eq!(
                std::fs::read(root.store().join("delivery.bin")).unwrap(),
                after
            );
            assert!(supervisor.owner.borrow().snapshot.is_some());
            assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        } else {
            let mut host = supervisor.host_mut().unwrap();
            let revision = host.revision();
            host.cancel_request(revision, 71).unwrap();
        }
        assert!(matches!(
            actor.poll(&first),
            Knowledge::Known {
                value: ActorOutcome::CancelledBeforeDispatch,
                ..
            }
        ));
        assert!(matches!(
            actor.poll(&second),
            Knowledge::Pending { request: 72 }
        ));
        let host = supervisor.host().unwrap();
        let after = host.inspect();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.executions, 0);
        assert!(after.stop.is_none());
        let numerical = host.learned_generation_inspection().unwrap();
        assert_eq!(numerical.numerical, before_numerical.numerical);
        let stream = host.stream_snapshot().unwrap();
        assert_eq!(stream.published, before_stream.published);
        assert!(stream.published.visible().is_empty() && !stream.published.finished());
        let message = host
            .learned_text_message(LearnedEvidenceLimits::default())
            .map(|message| message.bytes().to_vec());
        let actual = (after, numerical, stream, message);
        if through_port {
            assert_eq!(
                &actual,
                reference.as_ref().unwrap(),
                "the finite operator port must preserve the original Cancel's complete learned cleanup"
            );
        } else {
            reference = Some(actual);
        }
    }
}

fn raw_pending(root: &Root) -> FileOversight {
    use crate::action::consequence::activation::consistency::{
        BinaryForecast, ErrorBudget, ForecastRegistration,
    };
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters,
    };
    use crate::action::consequence::gate::containment::ActorState;
    let mut selected = profile();
    selected.delivery.actor = ActorState::new(
        selected.delivery.actor.profile(),
        vec![0],
        vec![1],
        vec![2],
        1,
    )
    .unwrap();
    let capture = CaptureProfile {
        tenant: 1,
        model: 9,
        model_generation: 1,
        tap: 4,
        layout_generation: 1,
    };
    let config = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 1,
        probe_generation: 1,
        profile: capture,
        weights: vec![1.0],
        bias: 0.0,
        threshold: 0.0,
        forecast: ForecastRegistration {
            domain: 19,
            generation: 1,
            policy_generation: 1,
            event_prefix: b"ok".to_vec(),
            negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(),
            positive: BinaryForecast::new(49152, 16384).unwrap(),
        },
        alpha: ErrorBudget::new(1, 4).unwrap(),
        stream: 17,
        max_predictions: 4,
        max_prediction_age_ticks: 10,
    })
    .unwrap();
    let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
    let observer = host
        .enable_action_consistency(host.revision(), config)
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let frame = SourceFrame::capture(
        FrameIdentity {
            profile: capture,
            stream: 17,
            sequence: 1,
            position: 0,
        },
        &[1.0],
    )
    .unwrap();
    let revision = host.revision();
    let actor_revision = host.actor_snapshot().unwrap().actor_revision;
    observer
        .forecast_request(&mut host, revision, 701, actor_revision, &frame)
        .unwrap()
        .unwrap();
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot())
        .unwrap();
    assert!(!host.decoder_required());
    assert!(host.action_consistency_required());
    host
}

#[test]
fn raw_predictor_also_requires_numerical_budget_and_never_reforecasts_on_cancel() {
    let root = Root::new();
    let host = raw_pending(&root);
    let before = host.inspect();
    let forecast = host.action_consistency_snapshot().unwrap();
    let observation = host.action_consistency_observation(1).unwrap().clone();
    let events = before.revision as usize + 1;
    let (_, mut supervisor) = host.into_actor_gateway();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    let (ordinary, grant) = supervisor.cancellation_port(1, 1, events).unwrap();
    let offered = plan(ordinary.next(701, 1301));
    assert_eq!(
        offered.block_reason(),
        Some(FileCancellationBlockReason::NumericalReplayUnsupported)
    );
    let input = FileCancellationRequest::new(1301, offered.basis()).unwrap();
    assert_eq!(
        ordinary.cancel(input),
        Err(FileCancellationError::Unsupported)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    let (explicit, grant) = supervisor
        .cancellation_port_with_numerical_budget(1, 1, events, events)
        .unwrap();
    let cancelled = receipt(explicit.cancel(input));
    assert_eq!(cancelled.refunded_units(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    let host = supervisor.host().unwrap();
    assert_eq!(
        host.inspect().control.ledger.charged,
        before.control.ledger.charged
    );
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.action_consistency_snapshot().unwrap(), forecast);
    assert_eq!(
        host.action_consistency_observation(1).unwrap(),
        &observation
    );
}

#[test]
fn numerical_storage_failure_keeps_its_full_reservation_and_plain_cancel_does_not_spend_it() {
    let root = Root::new();
    let host = raw_pending(&root);
    let events = host.revision() as usize + 1;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .cancellation_port_with_numerical_budget(1, 1, events, events)
        .unwrap();
    let input = plan(port.next(701, 1401)).request().unwrap();
    supervisor
        .host()
        .unwrap()
        .fail_storage_once(JournalIo::Write);
    assert_eq!(port.cancel(input), Err(FileCancellationError::Unavailable));
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    unknown(port.clone().cancel(input));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));

    let root = Root::new();
    let host = pending(&root);
    let events = host.revision() as usize + 1;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .cancellation_port_with_numerical_budget(1, 1, events, 1)
        .unwrap();
    let input = plan(port.next(701, 1402)).request().unwrap();
    assert_eq!(receipt(port.cancel(input)).stage(), ActionState::Cancelled);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(1));
}
