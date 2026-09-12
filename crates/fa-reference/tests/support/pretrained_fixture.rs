//! Literal-name interchange fixtures, not trained weights or serving evidence.
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape,
};

pub fn profile(context: usize) -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 6, hidden: 4, intermediate: 6, layers: 2,
            query_heads: 2, cache_heads: 1, context }, 0.00001, 10000.0).unwrap()
}
fn values(count: usize, offset: usize) -> Vec<f32> {
    (0..count).map(|index| (((index * 3 + offset * 5) % 11) as i32 - 5) as f32 / 8.0).collect()
}
fn layers() -> Vec<DecoderLayerWeights> {
    (0..2).map(|index| {
        let n = 1 + index * 9;
        DecoderLayerWeights { attention_norm: vec![1.0; 4], queries: values(16, n),
            keys: values(8, n + 1), values: values(8, n + 2), attention_output: values(16, n + 3),
            feed_forward_norm: vec![1.0; 4], gate: values(24, n + 4), up: values(24, n + 5), down: values(24, n + 6) }
    }).collect()
}
pub fn model(context: usize) -> DecoderModel {
    DecoderModel::new(profile(context), values(24, 11), layers(), vec![1.0; 4], values(24, 12)).unwrap()
}

#[derive(Clone)]
pub struct Tensor { pub name: String, pub shape: Vec<usize>, pub dtype: &'static str, pub bytes: Vec<u8> }

pub fn tensors(mixed: bool) -> Vec<Tensor> {
    let mut rows = vec![
        ("model.embed_tokens.weight".to_owned(), vec![6, 4], values(24, 11)),
        ("model.norm.weight".to_owned(), vec![4], vec![1.0; 4]),
        ("lm_head.weight".to_owned(), vec![6, 4], values(24, 12)),
    ];
    for (index, w) in layers().into_iter().enumerate() {
        for (suffix, shape, values) in [
            ("input_layernorm.weight", vec![4], w.attention_norm),
            ("self_attn.q_proj.weight", vec![4, 4], w.queries),
            ("self_attn.k_proj.weight", vec![2, 4], w.keys),
            ("self_attn.v_proj.weight", vec![2, 4], w.values),
            ("self_attn.o_proj.weight", vec![4, 4], w.attention_output),
            ("post_attention_layernorm.weight", vec![4], w.feed_forward_norm),
            ("mlp.gate_proj.weight", vec![6, 4], w.gate),
            ("mlp.up_proj.weight", vec![6, 4], w.up),
            ("mlp.down_proj.weight", vec![4, 6], w.down),
        ] { rows.push((format!("model.layers.{index}.{suffix}"), shape, values)); }
    }
    rows.into_iter().enumerate().map(|(index, (name, shape, values))| {
        let dtype = if mixed { ["F32", "F16", "BF16"][index % 3] } else { "F32" };
        let bytes = values.iter().flat_map(|value| {
            let bits = value.to_bits();
            match dtype {
                "F32" => bits.to_le_bytes().to_vec(),
                "BF16" => { assert_eq!(bits & 65535, 0); ((bits >> 16) as u16).to_le_bytes().to_vec() }
                _ => {
                    let word = if bits & 0x7fff_ffff == 0 { (bits >> 16) as u16 }
                        else {
                            let exponent = ((bits >> 23) & 255) as i32 - 112;
                            assert!((1..31).contains(&exponent)); assert_eq!(bits & 8191, 0);
                            ((bits >> 16) as u16 & 0x8000) | ((exponent as u16) << 10) | ((bits >> 13) as u16 & 1023)
                        };
                    word.to_le_bytes().to_vec()
                }
            }
        }).collect();
        Tensor { name, shape, dtype, bytes }
    }).collect()
}

pub fn frame(header: &str, data: &[u8]) -> Vec<u8> {
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header.as_bytes()); bytes.extend_from_slice(data); bytes
}
pub fn header_data(tensors: &[Tensor]) -> (String, Vec<u8>) {
    let mut entries = Vec::new();
    let mut data = Vec::new();
    for tensor in tensors {
        let start = data.len(); data.extend_from_slice(&tensor.bytes);
        entries.push(format!("\"{}\":{{\"dtype\":\"{}\",\"shape\":{:?},\"data_offsets\":[{},{}]}}",
            tensor.name, tensor.dtype, tensor.shape, start, data.len()));
    }
    (format!("{{{}}}", entries.join(",")), data)
}
pub fn encode(tensors: &[Tensor]) -> Vec<u8> {
    let (header, data) = header_data(tensors); frame(&header, &data)
}
