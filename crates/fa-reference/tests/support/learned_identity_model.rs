//! Actual tiny models differing in parameters while retaining identical labels.
//! Fixed intervals/zero monitors are causal controls, not calibrated fingerprints.
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelManifest, ModelPassport};
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::{SamplingPolicy, SamplingStart,
            monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}}},
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use fa_reference::action::consequence::oversight::learned_source::LearnedSourceConfig;
use std::collections::{BTreeMap, BTreeSet};

pub fn model(first_embedding: f32) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 3, hidden: 2,
        intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
        keys: vec![0.0; 4], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![first_embedding, 0.0, -1.0, 0.0, 0.0, 1.0],
        vec![layer.clone(), layer], vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}

pub fn source(model: &DecoderModel, alarm: bool) -> LearnedSourceConfig {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            if alarm && *layer == 2 && side == KvSide::Value { weights[1] = 1.0; }
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, 0.5).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap(),
        policy: LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
            LearnedAuditPreparationBudget::default(), inference).unwrap(),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() }
}

pub fn passport(model: &DecoderModel) -> ModelPassport {
    let id = model.profile().identity();
    ModelPassport::new(51, 1, ModelManifest { tenant: id.tenant, model: id.model,
        model_generation: id.model_generation, host_generation: 1,
        tokenizer_generation: id.tokenizer_generation, weights: [1; 32], adapters: [2; 32],
        tokenizer: [3; 32], architecture: [4; 32], numeric_profile: [5; 32] }, vec![
        IdentityAnchor::new(10, model.residual_contract(1).unwrap().profile(), 501,
            vec![2, 0], &[[1.0, 1.0], [0.0, 0.0]]).unwrap(),
        IdentityAnchor::new(20, model.residual_contract(2).unwrap().profile(), 502,
            vec![0, 1], &[[-1.0, -1.0], [0.0, 0.0]]).unwrap(),
    ]).unwrap()
}
