//! Actual tiny learned decoder and codec, using the original numerical engine.
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
        sampling::{
            SamplingPolicy, SamplingStart,
            monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget},
        },
    },
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use crate::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedConfig;
use crate::action::consequence::oversight::{
    decoder_monitoring::LearnedDecoderBindingLimits, learned_source::LearnedSourceConfig,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn model() -> DecoderModel {
    model_with_output(0.0)
}
pub(super) fn model_with_output(output: f32) -> DecoderModel {
    let profile = DecoderProfile::new(
        DecoderIdentity {
            tenant: 1,
            model: 9,
            model_generation: 1,
            tokenizer_generation: 1,
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
pub(super) fn source(model: &DecoderModel, alarm: bool, top_k: usize) -> LearnedSourceConfig {
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
pub(super) fn config(alarm: bool, top_k: usize) -> FileLearnedConfig {
    let model = model();
    let source = source(&model, alarm, top_k);
    FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default())
        .unwrap()
        .with_required_sidecar()
        .unwrap()
}
