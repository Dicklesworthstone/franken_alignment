//! Ordinary promotion never silently acquires numerical-history work.
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

fn refuses(mut host: FileOversight, governor: &FilePolicyGovernor, root: &Root) {
    let key = approve(&mut host, governor, 91);
    let before = host.inspect();
    let decoder = host.decoder_inspection().ok();
    let predictor = host.action_consistency_snapshot().ok();
    let files = root.files();
    let (_, mut supervisor) = host.into_actor_gateway();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let (port, grant) = supervisor
        .policy_promotion_port(key, 1, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let offered = plan(port.next(91, 900));
    assert_eq!(
        offered.block_reason(),
        Some(FilePolicyPromotionBlockReason::NumericalReplayUnsupported)
    );
    assert_eq!(
        port.promote(FilePolicyPromotionRequest::new(900, offered.basis()).unwrap()),
        Err(FilePolicyPromotionError::Unsupported)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    let host = supervisor.host().unwrap();
    assert_eq!(host.inspect(), before);
    assert_eq!(host.decoder_inspection().ok(), decoder);
    assert_eq!(host.action_consistency_snapshot().ok(), predictor);
    assert_eq!(root.files(), files);
}

#[test]
fn real_decoder_and_raw_forecast_histories_do_not_gain_unbudgeted_reconstruction() {
    let root = Root::new();
    let (mut host, _, governor) = governed(&root);
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
    host.enable_decoder(host.revision(), config).unwrap();
    let cut = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(
        host.advance_decoder_forced(
            host.revision(),
            cut.actor_revision,
            cut.position,
            0,
            DecoderBudget {
                scalar_products: MAX_DECODER_PRODUCTS
            }
        )
        .unwrap()
        .unwrap(),
        MonitoredStep::Released(_)
    ));
    // An actual accepted numerical observation is not itself a policy case.
    // Admit the original proposal so the independent campaign compares a real
    // retained corpus under this decoder's native admission requirements.
    submit(&mut host, 701, b"ok");
    refuses(host, &governor, &root);

    use crate::action::consequence::activation::consistency::{
        BinaryForecast, ErrorBudget, ForecastRegistration,
    };
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters,
    };
    use crate::action::consequence::gate::containment::ActorState;
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
    let governor = host
        .enable_policy_campaigns(
            host.revision(),
            ReplayLimits {
                cases: 16,
                input_bytes: 1_048_576,
            },
            8,
        )
        .unwrap();
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
    assert!(!host.decoder_required());
    assert!(host.action_consistency_required());
    // Consume the preceding forecast for its exact original request; a stored
    // prediction alone cannot stand in for a policy replay case.
    submit(&mut host, 701, b"ok");
    refuses(host, &governor, &root);
}
