//! Deterministic nontrivial parameters are fixtures, not trained-model evidence.
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape,
};

pub fn profile(context: usize) -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 6, hidden: 4, intermediate: 6, layers: 2,
            query_heads: 2, cache_heads: 1, context }, 0.00001, 10000.0).unwrap()
}

pub fn values(count: usize, offset: usize) -> Vec<f32> {
    (0..count).map(|index| (((index * 7 + offset * 3) % 17) as i32 - 8) as f32 / 19.0).collect()
}

pub fn layers(p: &DecoderProfile) -> Vec<DecoderLayerWeights> {
    let s = p.shape();
    (0..s.layers).map(|layer| DecoderLayerWeights {
        attention_norm: (0..s.hidden).map(|j| 0.9 + ((j + layer) % 3) as f32 / 10.0).collect(),
        queries: values(s.hidden * s.hidden, 1 + layer),
        keys: values(p.cache_width() * s.hidden, 2 + layer),
        values: values(p.cache_width() * s.hidden, 3 + layer),
        attention_output: values(s.hidden * s.hidden, 4 + layer),
        feed_forward_norm: vec![1.0; s.hidden],
        gate: values(s.intermediate * s.hidden, 5 + layer),
        up: values(s.intermediate * s.hidden, 6 + layer),
        down: values(s.hidden * s.intermediate, 7 + layer),
    }).collect()
}

pub fn model(p: DecoderProfile) -> DecoderModel {
    let s = p.shape();
    DecoderModel::new(p.clone(), values(s.vocabulary * s.hidden, 11), layers(&p),
        vec![1.0; s.hidden], values(s.vocabulary * s.hidden, 12)).unwrap()
}

pub fn zero_layers(p: &DecoderProfile) -> Vec<DecoderLayerWeights> {
    let s = p.shape();
    (0..s.layers).map(|_| DecoderLayerWeights {
        attention_norm: vec![1.0; s.hidden], queries: vec![0.0; s.hidden * s.hidden],
        keys: vec![0.0; p.cache_width() * s.hidden], values: vec![0.0; p.cache_width() * s.hidden],
        attention_output: vec![0.0; s.hidden * s.hidden], feed_forward_norm: vec![1.0; s.hidden],
        gate: vec![0.0; s.intermediate * s.hidden], up: vec![0.0; s.intermediate * s.hidden],
        down: vec![0.0; s.hidden * s.intermediate],
    }).collect()
}
