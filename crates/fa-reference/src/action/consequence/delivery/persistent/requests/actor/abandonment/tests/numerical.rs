//! Real numerical history must reserve the whole original reconstruction.
//! Small deterministic models are controls, not evidence of detector quality.
use super::*;
use crate::action::consequence::activation::monitor::decoder::MonitoredStep;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;

mod data {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/decoder_inputs.rs"
    ));
}

fn decoder_unknown(root: &Root) -> FileOversight {
    let decoder = DecoderProfile::new(
        DecoderIdentity {
            tenant: 1,
            model: 9,
            model_generation: 1,
            tokenizer_generation: 1,
            profile_generation: 1,
        },
        DecoderShape {
            vocabulary: 2,
            hidden: 2,
            intermediate: 2,
            layers: 1,
            query_heads: 1,
            cache_heads: 1,
            context: 4,
        },
        1e-5,
        10000.0,
    )
    .unwrap();
    let config = FileDecoderConfig::new(
        decoder,
        data::weights(),
        data::monitor(3.0),
        data::sampling(),
        5,
        DecoderBindingLimits::default(),
    )
    .unwrap();
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_decoder(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(
        host.advance_decoder_forced(
            host.revision(),
            numerical.actor_revision,
            numerical.position,
            0,
            DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        ).unwrap().unwrap(),
        MonitoredStep::Released(_)
    ));
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    assert_eq!(
        host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::AwaitingResolution
    );
    host
}

#[test]
fn actual_decoder_history_requires_complete_work_and_matches_the_original_disposition() {
    let mut original = None;
    for scoped in [false, true] {
        let root = Root::new();
        let mut host = decoder_unknown(&root);
        let before = host.inspect();
        let numerical = host.decoder_inspection().unwrap().numerical;
        let operation = 1701;
        let events = before.revision as usize + 1;
        if !scoped {
            host.abandon_unknown(host.revision(), FileAbandonRequest {
                operation,
                attempt: 1,
                expected_control_sequence: before.control.sequence,
                expected_authority_epoch: before.control.ledger.epoch,
            }).unwrap();
            original = Some((host.inspect(), host.decoder_inspection().unwrap().numerical));
            continue;
        }
        let (_, mut supervisor) = host.into_actor_gateway();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        let files = root.files();
        let (default, default_grant) = supervisor.abandonment_port(1, 1, events).unwrap();
        let offered = plan(default.next(701, operation));
        assert_eq!(offered.block_reason(), Some(FileAbandonmentBlockReason::NumericalReplayUnsupported));
        let input = FileAbandonmentRequest::new(operation, offered.basis()).unwrap();
        assert_eq!(default.abandon(input), Err(FileAbandonmentError::Unsupported));
        assert_eq!(default_grant.remaining_operations(), 1);
        assert_eq!(default_grant.remaining_numerical_replay_events(), None);
        let (short, short_grant) = supervisor
            .abandonment_port_with_numerical_budget(1, 1, events, events - 1).unwrap();
        assert_eq!(plan(short.next(701, operation)).block_reason(),
            Some(FileAbandonmentBlockReason::NumericalReplayLimit));
        assert_eq!(short.abandon(input), Err(FileAbandonmentError::Capacity));
        assert_eq!(short_grant.remaining_operations(), 1);
        assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(supervisor.host().unwrap().decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(root.files(), files);

        let (exact, grant) = supervisor
            .abandonment_port_with_numerical_budget(1, 1, events, events).unwrap();
        assert_eq!(plan(exact.next(701, operation)).request(), Some(input));
        assert_eq!(grant.remaining_numerical_replay_events(), Some(events));
        let acknowledged = receipt(exact.abandon(input));
        assert_eq!(acknowledged.charged_units(), 8);
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        assert!(supervisor.owner.borrow().snapshot.is_none());
        let after = supervisor.host().unwrap().inspect();
        let after_numerical = supervisor.host().unwrap().decoder_inspection().unwrap().numerical;
        assert_eq!(after_numerical, numerical);
        assert_eq!(&(after.clone(), after_numerical), original.as_ref().unwrap());
        supervisor.set_snapshot(after.revision, Some(snapshot())).unwrap();
        let files = root.files();
        assert_eq!(receipt(exact.clone().abandon(input)), acknowledged);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        assert_eq!(root.files(), files);
        assert!(supervisor.owner.borrow().snapshot.is_some());
    }
}

fn raw_unknown(root: &Root) -> FileOversight {
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
        vec![0], vec![1], vec![2], 1,
    ).unwrap();
    let capture = CaptureProfile {
        tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1,
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
    }).unwrap();
    let (mut host, reviewer) = FileOversight::create(root.store(), selected).unwrap();
    let observer = host.enable_action_consistency(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let frame = SourceFrame::capture(
        FrameIdentity { profile: capture, stream: 17, sequence: 1, position: 0 },
        &[1.0],
    ).unwrap();
    let revision = host.revision();
    let actor_revision = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_request(&mut host, revision, 701, actor_revision, &frame)
        .unwrap().unwrap();
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    dispatch_existing(&mut host, &reviewer, 701);
    host.reconcile(host.revision(), 1).unwrap();
    assert!(!host.decoder_required());
    assert!(host.action_consistency_required());
    host
}

#[test]
fn raw_predictor_work_is_separate_and_failed_storage_cannot_refund_its_reservation() {
    for fail in [false, true] {
        let root = Root::new();
        let host = raw_unknown(&root);
        let before = host.inspect();
        let forecast = host.action_consistency_snapshot().unwrap();
        let observed = host.action_consistency_observation(1).unwrap().clone();
        let events = before.revision as usize + 1;
        let (_, mut supervisor) = host.into_actor_gateway();
        supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
        let (default, default_grant) = supervisor.abandonment_port(1, 1, events).unwrap();
        let offered = plan(default.next(701, 1801));
        assert_eq!(offered.block_reason(),
            Some(FileAbandonmentBlockReason::NumericalReplayUnsupported));
        let input = FileAbandonmentRequest::new(1801, offered.basis()).unwrap();
        assert_eq!(default.abandon(input), Err(FileAbandonmentError::Unsupported));
        assert_eq!(default_grant.remaining_operations(), 1);
        assert!(supervisor.owner.borrow().snapshot.is_some());
        let (port, grant) = supervisor.abandonment_port_with_numerical_budget(
            1, 1, events, events,
        ).unwrap();
        assert_eq!(plan(port.next(701, 1801)).request(), Some(input));
        if fail {
            supervisor.host().unwrap().fail_storage_once(JournalIo::Write);
            assert_eq!(port.abandon(input), Err(FileAbandonmentError::Unavailable));
            unknown(port.clone().abandon(input));
        } else {
            assert_eq!(receipt(port.abandon(input)).charged_units(), 8);
            let host = supervisor.host().unwrap();
            let retained = host.action_consistency_snapshot().unwrap();
            assert_eq!(retained.journal_revision, before.revision + 1);
            assert_eq!(retained.evidence, forecast.evidence);
            assert_eq!(retained.pending_attempt, forecast.pending_attempt);
            assert_eq!(retained.coverage_lost, forecast.coverage_lost);
            assert_eq!(host.action_consistency_observation(1).unwrap(), &observed);
            assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().payload, before.payload);
        }
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        assert!(supervisor.owner.borrow().snapshot.is_none());
    }

    // Merely supplying a numerical allowance must not charge a plain history.
    let root = Root::new();
    let host = uncertain(&root, false);
    let events = host.revision() as usize + 1;
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .abandonment_port_with_numerical_budget(1, 1, events, 1).unwrap();
    let input = plan(port.next(701, 1802)).request().unwrap();
    assert_eq!(receipt(port.abandon(input)).charged_units(), 8);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(1));
}
