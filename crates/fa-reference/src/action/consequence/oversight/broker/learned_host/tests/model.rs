//! Original numerical fixture, with nonzero attention and a token-2 alarm.
use crate::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy, LearnedStreamRetention}},
    experiment::KvSide, model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use std::collections::BTreeMap;

pub(super) fn model() -> DecoderModel { sized_model(2, 2, 16, 3) }
pub(super) fn sized_model(hidden: usize, layers: usize, context: usize, generation: u64) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: generation, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden, intermediate: 2, layers,
            query_heads: 1, cache_heads: 1, context }, 0.00001, 10000.0).unwrap();
    let mut identity = vec![0.0; hidden * hidden];
    for index in 0..hidden { identity[index * hidden + index] = 1.0; }
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; hidden],
        queries: identity.clone(), keys: identity.clone(), values: identity.clone(),
        attention_output: identity.iter().map(|v| v * 0.25).collect(),
        feed_forward_norm: vec![1.0; hidden], gate: vec![0.0; 2 * hidden],
        up: vec![0.0; 2 * hidden], down: vec![0.0; 2 * hidden] };
    let mut embeddings = vec![0.0; 3 * hidden];
    embeddings[0] = 1.0; embeddings[hidden] = -1.0; embeddings[2 * hidden + 1] = 1.0;
    let mut output = vec![0.0; 3 * hidden]; output[2 * hidden] = 1.0;
    DecoderModel::new(profile, embeddings, vec![layer; layers], vec![1.0; hidden], output).unwrap()
}
pub(super) fn policy(model: &DecoderModel, alarm: bool) -> LearnedDecoderPolicy {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if alarm && *layer == 1 && side == KvSide::Value {
                weights[1] = 1.0; 0.5
            } else { 1.0 };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps,
        LearnedAuditBudget { rows: model.profile().shape().layers * 2, ..LearnedAuditBudget::default() }).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap()
}
