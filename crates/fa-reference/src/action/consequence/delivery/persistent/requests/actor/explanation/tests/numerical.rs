//! Event count is not a numerical-products budget. Real numerical ownership
//! therefore has a charged Unknown counterpart to ordinary policy explanation.
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
fn numerical_original_history_is_not_replayed_under_an_event_only_read_grant() {
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
    let (mut host, _reviewer) = FileOversight::create(root.store(), profile()).unwrap();
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
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.explanation_port(1).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let files = root.files();
    unknown(port.explain_request(901));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.explain_request(901), Err(ActorError::Capacity));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    assert_eq!(root.files(), files);
}

#[test]
fn raw_predictor_only_admission_is_unknown_while_the_plain_control_remains_explainable() {
    use crate::action::consequence::activation::consistency::{
        BinaryForecast, ErrorBudget, ForecastRegistration,
    };
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters,
    };
    use crate::action::consequence::gate::containment::ActorState;

    for predictor in [false, true] {
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
        let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
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
            FileRequestDisposition::Admitted { .. }
        ));
        assert!(!host.decoder_required());
        assert!(!host.learned_action_consistency_required());
        assert_eq!(host.action_consistency_required(), predictor);
        let forecast = predictor.then(|| host.action_consistency_snapshot().unwrap());
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.explanation_port(1).unwrap();
        let before = supervisor.host().unwrap().inspect();
        let files = root.files();
        let result = port.explain_request(901);
        if predictor {
            unknown(result);
        } else {
            let explained = known(result);
            assert_eq!(explained.basis().request, 901);
            assert_eq!(explained.basis().journal_revision, before.revision);
        }
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(port.explain_request(901), Err(ActorError::Capacity));
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        if let Some(forecast) = forecast {
            assert_eq!(
                supervisor.host().unwrap().action_consistency_snapshot().unwrap(),
                forecast
            );
        }
        assert_eq!(root.files(), files);
    }
}
