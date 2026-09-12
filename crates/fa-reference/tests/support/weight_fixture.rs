//! An independent file writer for deterministic fixture parameters, not training.
#[path = "decoder_fixture.rs"]
pub mod decoder;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderProfile, DecoderModel, DecoderLayerWeights};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Tensor { pub name: String, pub shape: Vec<usize>, pub dtype: String, pub bytes: Vec<u8> }
impl Tensor {
    fn f32(name: String, shape: Vec<usize>, values: Vec<f32>) -> Self {
        Self { name, shape, dtype: "F32".into(), bytes: values.iter().flat_map(|v| v.to_le_bytes()).collect() }
    }
    pub fn values(&self) -> Vec<f32> {
        match self.dtype.as_str() {
            "F32" => self.bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect(),
            "BF16" => self.bytes.chunks_exact(2).map(|b| f32::from_bits(u32::from(u16::from_le_bytes(b.try_into().unwrap())) << 16)).collect(),
            "F16" => self.bytes.chunks_exact(2).map(|b| {
                let word = u16::from_le_bytes(b.try_into().unwrap());
                let exponent = i32::from((word >> 10) & 31);
                assert_ne!(exponent, 31);
                let fraction = u32::from(word & 1023);
                let magnitude = if exponent == 0 { f64::from(fraction) * 2_f64.powi(-24) }
                    else { f64::from(1024 + fraction) * 2_f64.powi(exponent - 25) };
                (if word & 0x8000 == 0 { magnitude } else { -magnitude }) as f32
            }).collect(),
            _ => panic!("unsupported test dtype"),
        }
    }
}

pub fn tensors(p: &DecoderProfile) -> Vec<Tensor> {
    let s = p.shape(); let h = s.hidden; let k = p.cache_width(); let i = s.intermediate;
    let mut result = vec![
        Tensor::f32("model.embed_tokens.weight".into(), vec![s.vocabulary, h], decoder::values(s.vocabulary * h, 11)),
        Tensor::f32("model.norm.weight".into(), vec![h], vec![1.0; h]),
        Tensor::f32("lm_head.weight".into(), vec![s.vocabulary, h], decoder::values(s.vocabulary * h, 12)),
    ];
    for (index, layer) in decoder::layers(p).into_iter().enumerate() {
        for (name, shape, values) in [
            ("input_layernorm.weight", vec![h], layer.attention_norm),
            ("self_attn.q_proj.weight", vec![h, h], layer.queries),
            ("self_attn.k_proj.weight", vec![k, h], layer.keys),
            ("self_attn.v_proj.weight", vec![k, h], layer.values),
            ("self_attn.o_proj.weight", vec![h, h], layer.attention_output),
            ("post_attention_layernorm.weight", vec![h], layer.feed_forward_norm),
            ("mlp.gate_proj.weight", vec![i, h], layer.gate),
            ("mlp.up_proj.weight", vec![i, h], layer.up),
            ("mlp.down_proj.weight", vec![h, i], layer.down),
        ] {
            result.push(Tensor::f32(format!("model.layers.{index}.{name}"), shape, values));
        }
    }
    result
}

pub fn direct(p: DecoderProfile, tensors: &[Tensor]) -> DecoderModel {
    let all: BTreeMap<_, _> = tensors.iter().map(|t| (t.name.as_str(), t.values())).collect();
    let mut layers = Vec::new();
    for index in 0..p.shape().layers {
        let get = |suffix: &str| all[format!("model.layers.{index}.{suffix}").as_str()].clone();
        layers.push(DecoderLayerWeights {
            attention_norm: get("input_layernorm.weight"), queries: get("self_attn.q_proj.weight"),
            keys: get("self_attn.k_proj.weight"), values: get("self_attn.v_proj.weight"),
            attention_output: get("self_attn.o_proj.weight"), feed_forward_norm: get("post_attention_layernorm.weight"),
            gate: get("mlp.gate_proj.weight"), up: get("mlp.up_proj.weight"), down: get("mlp.down_proj.weight"),
        });
    }
    DecoderModel::new(p, all["model.embed_tokens.weight"].clone(), layers,
        all["model.norm.weight"].clone(), all["lm_head.weight"].clone()).unwrap()
}

pub fn parts(tensors: &[Tensor]) -> (String, Vec<u8>) {
    let mut entries = Vec::new(); let mut data = Vec::new();
    for tensor in tensors {
        let begin = data.len(); data.extend_from_slice(&tensor.bytes);
        entries.push(format!("\"{}\":{{\"dtype\":\"{}\",\"shape\":{:?},\"data_offsets\":[{},{}]}}",
            tensor.name, tensor.dtype, tensor.shape, begin, data.len()));
    }
    // Deliberately make directory order different from data order.
    entries.reverse();
    (format!("{{{}}}", entries.join(",")), data)
}
pub fn frame(header: &str, data: &[u8]) -> Vec<u8> {
    let mut padded = header.as_bytes().to_vec();
    while !padded.len().is_multiple_of(8) { padded.push(b' '); }
    let mut bytes = (padded.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(&padded); bytes.extend_from_slice(data); bytes
}
pub fn archive(tensors: &[Tensor]) -> Vec<u8> {
    let (header, data) = parts(tensors); frame(&header, &data)
}
