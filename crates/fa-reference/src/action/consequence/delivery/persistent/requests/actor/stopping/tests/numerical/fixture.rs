//! Small actual learned model and raw predictor controls using original recipes.
//! Zero alarm probes are a control, not a detection or deployment claim.
use super::super::*;
use crate::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::{
    decoder::{
        DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}},
    },
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer,
    decoder::learned::FileLearnedConfig,
};
use crate::action::consequence::gate::containment::ActorState;
use crate::action::consequence::oversight::{
    decoder_monitoring::LearnedDecoderBindingLimits, learned_source::LearnedSourceConfig,
};
use std::collections::{BTreeMap, BTreeSet};

fn model() -> DecoderModel {
    model_with_output(0.0)
}
fn model_with_output(output: f32) -> DecoderModel {
    let profile = DecoderProfile::new(
        DecoderIdentity {
            tenant: 1,
            model: 2,
            model_generation: 3,
            tokenizer_generation: 4,
            profile_generation: 5,
        },
        DecoderShape {
            vocabulary: 3,
            hidden: 2,
            intermediate: 2,
            layers: 2,
            query_heads: 1,
            cache_heads: 1,
            context: 16,
        },
        0.00001,
        10000.0,
    )
    .unwrap();
    let layer = DecoderLayerWeights {
        attention_norm: vec![1.0; 2],
        queries: vec![0.0; 4],
        keys: vec![0.0; 4],
        values: vec![1.0, 0.0, 0.0, 1.0],
        attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4],
        up: vec![0.0; 4],
        down: vec![0.0; 4],
    };
    DecoderModel::new(
        profile,
        vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0],
        vec![layer.clone(), layer],
        vec![1.0; 2],
        vec![output, 0.0, 0.0, 0.0, 1.0, 0.0],
    )
    .unwrap()
}
fn source(model: &DecoderModel, alarm: bool, top_k: usize) -> LearnedSourceConfig {
    let inference = DecoderBudget {
        scalar_products: MAX_DECODER_PRODUCTS,
    };
    let training = model
        .recompute(11, &[0, 1], inference)
        .unwrap()
        .cache_image()
        .unwrap();
    let codec = LearnedKvCodec::fit(
        LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]),
        FitBudget::default(),
    )
    .unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [
            (KvSide::Key, contract.keys()),
            (KvSide::Value, contract.values()),
        ] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if alarm && *layer == 2 && side == KvSide::Value {
                weights[1] = 1.0;
                0.5
            } else {
                1.0
            };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(
                KvTap {
                    layer: *layer,
                    side,
                },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default())
                    .unwrap(),
            );
        }
    }
    let monitor = LearnedModelMonitor::new(
        model.cache_profile().clone(),
        taps,
        LearnedAuditBudget::default(),
    )
    .unwrap();
    let policy = LearnedDecoderPolicy::new(
        codec,
        monitor,
        LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(),
        inference,
    )
    .unwrap();
    LearnedSourceConfig {
        stream: 21,
        evaluation_origin: 201,
        monitor_generation: 31,
        spec: GenerationSpec::new(
            vec![0],
            3,
            BTreeSet::new(),
            SamplingStart {
                policy: SamplingPolicy::new(1, 1, 3, 0.8, top_k, 1.0).unwrap(),
                stream: 71,
                seed: 173,
            },
        )
        .unwrap(),
        policy,
        budget: GenerationBudget::default(),
        telemetry: GenerationTelemetryBudget::default(),
    }
}
fn config(alarm: bool, top_k: usize) -> FileLearnedConfig {
    let model = model();
    let source = source(&model, alarm, top_k);
    FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap()
}

fn dispatch_and_optionally_publish(host: &mut FileOversight, reviewer: &FileHumanReviewer, published: bool) {
    host.submit_request(host.revision(), 701, spec(b"ok"), snapshot()).unwrap();
    let (action, inputs, automatic, request) = prepare_existing(host, 701);
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    if published {
        let result = host.publish_checked(
            host.revision(), automatic.attempt(), Some(&inputs), snapshot(), ElapsedTick(1),
        ).unwrap();
        assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    }
    assert_eq!(host.request_status(701).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Dispatching });
}

pub(super) fn learned_owner(root: &Root, published: bool, cold: bool) -> FileOversight {
    let mut selected = profile();
    let mut actor_profile = selected.delivery.actor.profile();
    actor_profile.model_generation = 3;
    actor_profile.tokenizer_generation = 4;
    selected.delivery.actor = ActorState::new(actor_profile, Vec::new(), vec![0], vec![0], 0).unwrap();
    let (mut host, reviewer) = FileOversight::create(root.store(), selected).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), config(false, 1)).unwrap();
    let before = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), before.actor_revision, before.position)
        .unwrap().unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, before.position + 1);
    dispatch_and_optionally_publish(&mut host, &reviewer, published);
    if cold {
        // This public operation drops only an optimization. Native reconciliation
        // must reconstruct the same original numerical history without it.
        host.clear_learned_replay_cache();
    }
    assert!(host.learned_generation_required());
    assert!(host.decoder_required());
    host
}

pub(super) fn raw_owner(root: &Root, published: bool) -> FileOversight {
    use crate::action::consequence::activation::consistency::{
        BinaryForecast, ErrorBudget, ForecastRegistration,
    };
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters,
    };
    let mut selected = profile();
    selected.delivery.actor = ActorState::new(
        selected.delivery.actor.profile(), vec![0], vec![1], vec![2], 1,
    ).unwrap();
    let capture = CaptureProfile {
        tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1,
    };
    let config = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 1, probe_generation: 1, profile: capture,
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration {
            domain: 19, generation: 1, policy_generation: 1, event_prefix: b"ok".to_vec(),
            negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(),
            positive: BinaryForecast::new(49152, 16384).unwrap(),
        },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17,
        max_predictions: 4, max_prediction_age_ticks: 10,
    }).unwrap();
    let (mut host, reviewer) = FileOversight::create(root.store(), selected).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let observer = host.enable_action_consistency(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let frame = SourceFrame::capture(
        FrameIdentity { profile: capture, stream: 17, sequence: 1, position: 0 }, &[1.0],
    ).unwrap();
    let revision = host.revision();
    let actor_revision = host.actor_snapshot().unwrap().actor_revision;
    let prediction = observer.forecast_request(&mut host, revision, 701, actor_revision, &frame)
        .unwrap().unwrap();
    assert_eq!(prediction.forecast().null_numerator(), 49152);
    dispatch_and_optionally_publish(&mut host, &reviewer, published);
    assert!(!host.decoder_required());
    assert!(!host.learned_action_consistency_required());
    assert!(host.action_consistency_required());
    host
}

