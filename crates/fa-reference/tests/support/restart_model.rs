//! Shared small, history-sensitive decoder for restart integration tests.
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, DecoderRestoreBudget, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderPolicy, LearnedStreamRetention,
            restart::incremental::IncrementalRestartBudget}},
    experiment::KvSide, image::IMAGE_HEADER_BYTES,
    model::{ModelKvImage, MAX_MODEL_KV_VALUES, learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy}},
};
use std::collections::{BTreeMap, BTreeSet};

pub fn model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2],
        queries: vec![1.0, 0.0, 0.0, 1.0], keys: vec![1.0, 0.0, 0.0, 1.0],
        values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.25, 0.0, 0.0, 0.25],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}

/// mode 0 is quiet; 1 needs source residual refinement on off-axis token 2;
/// mode 2 alarms on token 2. Other taps are still registered and fully reviewed.
pub fn policy(model: &DecoderModel, mode: u8, retention: u8) -> LearnedDecoderPolicy {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let retention = match retention {
        0 => LearnedStreamRetention::None,
        1 => LearnedStreamRetention::All,
        _ => LearnedStreamRetention::Heads(BTreeSet::from([*codec.groups().keys().next().unwrap()])),
    };
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            let threshold = if mode > 0 && *layer == 1 && side == KvSide::Value {
                weights[usize::from(mode == 2)] = 1.0; 0.5
            } else { 1.0 };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    // Exactly ONE position fits the fixed row cap: two layers times K and V.
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps,
        LearnedAuditBudget { rows: 4, ..LearnedAuditBudget::default() }).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, retention,
        LearnedAuditPreparationBudget::default(), inference).unwrap()
}
pub fn capture_limit() -> DecoderRestoreBudget { DecoderRestoreBudget { cache_values: MAX_MODEL_KV_VALUES } }
pub fn budget(policy: &LearnedDecoderPolicy, positions: usize) -> IncrementalRestartBudget {
    IncrementalRestartBudget { cache_values: MAX_MODEL_KV_VALUES, positions, per_position: policy.allowance() }
}
pub fn logits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
pub fn same_cache(left: &ModelKvImage, right: &ModelKvImage) {
    assert_eq!(left.profile(), right.profile());
    assert_eq!(left.len(), right.len());
    for layer in left.profile().layers().keys() {
        let a = left.layer(*layer).unwrap().encode().unwrap();
        let b = right.layer(*layer).unwrap().encode().unwrap();
        assert_eq!(&a[IMAGE_HEADER_BYTES..], &b[IMAGE_HEADER_BYTES..]);
    }
}
