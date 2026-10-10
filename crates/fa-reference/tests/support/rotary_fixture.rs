//! Controlled weights isolate the actual rotary query while retaining causal
//! attention and nontrivial logits. They are not pretrained-model evidence.
#![allow(dead_code)]
#[path = "pretrained_fixture.rs"]
mod interchange;
use fa_reference::action::consequence::activation::{ProgressiveFrame, SourceFrame};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
    DecoderShape, RotaryScaling, MAX_DECODER_PRODUCTS,
    sampling::{SampleBudget, SamplingBudget, SamplingPolicy, SamplingStart},
};

pub fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape {
        vocabulary: 256, hidden: 16, intermediate: 16, layers: 1,
        query_heads: 1, cache_heads: 1, context: 16,
    }, 1e-5, 10000.0).unwrap()
}
pub fn sign(token: u32, channel: usize) -> f32 {
    if (token as usize + channel * 3) % 7 < 3 { 1.0 } else { -1.0 }
}
fn diagonal(scale: f32) -> Vec<f32> {
    let mut values = vec![0.0; 256];
    for i in 0..16 { values[i * 16 + i] = scale; }
    values
}
fn embeddings() -> Vec<f32> {
    (0..256).flat_map(|token| (0..16).map(move |channel| sign(token, channel))).collect()
}
fn head() -> Vec<f32> {
    (0..256).flat_map(|token| (0..16).map(move |channel|
        (((token * 5 + channel * 7) % 17) as f32 - 8.0) / 16.0)).collect()
}
fn layer() -> DecoderLayerWeights {
    DecoderLayerWeights { attention_norm: vec![1.0; 16], queries: diagonal(1.0),
        keys: diagonal(0.5), values: diagonal(0.25), attention_output: diagonal(1.0),
        feed_forward_norm: vec![1.0; 16], gate: vec![0.0; 256],
        up: vec![0.0; 256], down: vec![0.0; 256] }
}
pub fn model(scaling: RotaryScaling) -> DecoderModel {
    DecoderModel::new(profile().with_rotary_scaling(scaling).unwrap(), embeddings(),
        vec![layer()], vec![1.0; 16], head()).unwrap()
}
pub fn weights() -> Vec<u8> {
    let w = layer();
    let mut rows = vec![
        ("model.embed_tokens.weight".to_owned(), vec![256, 16], embeddings()),
        ("model.norm.weight".to_owned(), vec![16], vec![1.0; 16]),
        ("lm_head.weight".to_owned(), vec![256, 16], head()),
    ];
    for (suffix, shape, values) in [
        ("input_layernorm.weight", vec![16], w.attention_norm),
        ("self_attn.q_proj.weight", vec![16, 16], w.queries),
        ("self_attn.k_proj.weight", vec![16, 16], w.keys),
        ("self_attn.v_proj.weight", vec![16, 16], w.values),
        ("self_attn.o_proj.weight", vec![16, 16], w.attention_output),
        ("post_attention_layernorm.weight", vec![16], w.feed_forward_norm),
        ("mlp.gate_proj.weight", vec![16, 16], w.gate),
        ("mlp.up_proj.weight", vec![16, 16], w.up),
        ("mlp.down_proj.weight", vec![16, 16], w.down),
    ] { rows.push((format!("model.layers.0.{suffix}"), shape, values)); }
    interchange::encode(&rows.into_iter().map(|(name, shape, values)| interchange::Tensor {
        name, shape, dtype: "F32", bytes: values.into_iter().flat_map(f32::to_le_bytes).collect(),
    }).collect::<Vec<_>>())
}
pub fn config(extra: &str) -> Vec<u8> {
    format!(concat!("{{\"model_type\":\"llama\",\"vocab_size\":256,\"hidden_size\":16,",
        "\"intermediate_size\":16,\"num_hidden_layers\":1,\"num_attention_heads\":1,",
        "\"max_position_embeddings\":2048,\"rms_norm_eps\":1e-5{}}}"), extra).into_bytes()
}
pub fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
pub fn sample_budget() -> SampleBudget {
    SampleBudget { decoder: budget(), sampling: SamplingBudget { vocabulary: 256 } }
}
pub fn start() -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(4, 2, 256, 0.8, 7, 0.95).unwrap(),
        stream: 77, seed: 123 }
}
pub fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
pub fn values(source: &SourceFrame) -> Vec<f32> {
    let bytes = source.encode_initial(23).unwrap();
    ProgressiveFrame::from_initial(&source.verify_block(&bytes).unwrap()).unwrap().exact_values().unwrap()
}
