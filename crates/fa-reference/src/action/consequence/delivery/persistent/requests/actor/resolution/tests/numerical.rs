//! The event-only grant never replays an actual numerical owner's history.
//! Ordinary original receipt resolution has positive counterparts in tests.rs.
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

#[test]
fn numerical_dispatched_history_is_not_replayed_under_an_event_only_operation_grant() {
    let root = Root::new();
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
            DecoderBudget {
                scalar_products: MAX_DECODER_PRODUCTS,
            },
        )
        .unwrap()
        .unwrap(),
        MonitoredStep::Released(_)
    ));
    assert!(matches!(
        host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
            .unwrap()
            .disposition,
        FileRequestDisposition::Admitted { .. }
    ));
    dispatch_existing(&mut host, &reviewer, 901);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS)
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    let offered = plan(port.next(901, 171));
    assert_eq!(offered.stage(), ActionState::Dispatching);
    assert_eq!((offered.basis().request, offered.basis().attempt), (901, 1));
    assert_eq!(offered.basis().journal_revision, before.revision);
    assert_eq!(
        offered.block_reason(),
        Some(FileResolutionBlockReason::NumericalReplayUnsupported)
    );
    assert_eq!(offered.request(), None);
    assert_eq!(
        port.resolve(FileResolutionRequest::new(171, offered.basis()).unwrap()),
        Err(FileResolutionError::Unsupported)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(before.control.ledger.charged, 8);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}

#[test]
fn raw_predictor_dispatch_and_unknown_cannot_enter_event_only_resolution() {
    use crate::action::consequence::activation::consistency::{
        BinaryForecast, ErrorBudget, ForecastRegistration,
    };
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters,
    };
    use crate::action::consequence::gate::containment::ActorState;

    for predictor in [false, true] {
        for uncertain in [false, true] {
            let root = Root::new();
            let mut selected = profile();
            selected.delivery.actor = ActorState::new(
                selected.delivery.actor.profile(),
                vec![0],
                vec![1],
                vec![2],
                1,
            )
            .unwrap();
            let (mut host, reviewer) = FileOversight::create(root.store(), selected).unwrap();
            let capture = CaptureProfile {
                tenant: 1,
                model: 9,
                model_generation: 1,
                tap: 4,
                layout_generation: 1,
            };
            let observer = predictor.then(|| {
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
                host.enable_action_consistency(host.revision(), config).unwrap()
            });
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            if let Some(observer) = observer {
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
                let prediction = observer
                    .forecast_request(&mut host, revision, 901, actor_revision, &frame)
                    .unwrap()
                    .unwrap();
                assert_eq!(prediction.forecast().null_numerator(), 49152);
            }
            assert!(matches!(
                host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
                    .unwrap()
                    .disposition,
                FileRequestDisposition::Admitted { attempt: 1, .. }
            ));
            // Both cases run the original congress and consume both original
            // approval keys at dispatch. No endpoint result is invented.
            dispatch_existing(&mut host, &reviewer, 901);
            if uncertain {
                assert_eq!(
                    host.reconcile(host.revision(), 1).unwrap(),
                    Reconciliation::AwaitingResolution
                );
            }
            let expected_stage = if uncertain {
                ActionState::Unknown
            } else {
                ActionState::Dispatching
            };
            assert_eq!(
                host.request_status(901).unwrap().disposition,
                FileRequestDisposition::Admitted {
                    attempt: 1,
                    stage: expected_stage,
                }
            );
            assert!(!host.decoder_required());
            assert!(!host.learned_action_consistency_required());
            assert_eq!(host.action_consistency_required(), predictor);
            let forecast = predictor.then(|| host.action_consistency_snapshot().unwrap());
            let (_, mut supervisor) = host.into_actor_gateway();
            let (port, grant) = supervisor
                .resolution_port(1, 1, MAX_RESOLUTION_REPLAY_EVENTS)
                .unwrap();
            let before = supervisor.host().unwrap().inspect();
            supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
            let files = root.files();
            let offered = plan(port.next(901, 171));
            assert_eq!(offered.stage(), expected_stage);
            assert_eq!((offered.basis().request, offered.basis().attempt), (901, 1));
            assert_eq!(grant.remaining_queries(), 0);
            assert_eq!(grant.remaining_operations(), 1);
            assert_eq!(before.executions, 0);
            assert_eq!(before.control.ledger.charged, 8);
            assert_eq!(supervisor.host().unwrap().inspect(), before);
            assert_eq!(root.files(), files);
            let input = FileResolutionRequest::new(171, offered.basis()).unwrap();
            if predictor {
                assert_eq!(
                    offered.block_reason(),
                    Some(FileResolutionBlockReason::NumericalReplayUnsupported)
                );
                assert_eq!(offered.request(), None);
                let clone = port.clone();
                for selected in [&port, &clone] {
                    assert_eq!(selected.resolve(input), Err(FileResolutionError::Unsupported));
                }
                assert_eq!(grant.remaining_operations(), 1);
                assert!(supervisor.owner.borrow().snapshot.is_some());
                assert_eq!(supervisor.host().unwrap().inspect(), before);
                assert_eq!(
                    supervisor.host().unwrap().action_consistency_snapshot().unwrap(),
                    forecast.unwrap()
                );
                assert_eq!(root.files(), files);
            } else {
                // The same original dispatched/uncertain plain request remains
                // executable. A missing receipt still cannot refund its charge.
                assert_eq!(offered.request(), Some(input));
                let resolved = receipt(port.resolve(input));
                assert_eq!(resolved.outcome(), Reconciliation::AwaitingResolution);
                assert_eq!(resolved.stage(), ActionState::Unknown);
                assert_eq!(grant.remaining_operations(), 0);
                let after = supervisor.host().unwrap().inspect();
                assert_eq!(after.revision, before.revision + 1);
                assert_eq!(after.executions, 0);
                assert_eq!(after.control.ledger.charged, 8);
                assert_eq!(after.payload, before.payload);
                assert!(supervisor.owner.borrow().snapshot.is_none());
                let committed = root.files();
                assert_eq!(receipt(port.clone().resolve(input)), resolved);
                assert_eq!(supervisor.host().unwrap().inspect(), after);
                assert_eq!(root.files(), committed);
            }
        }
    }
}
