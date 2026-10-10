//! Independent literal tensor inventory and outer-product oracle; no LoRA engine helpers.
#![allow(dead_code)]
#[path = "../support/pretrained_fixture.rs"]
pub mod interchange;
pub use interchange::{Tensor, encode};
use fa_reference::action::consequence::activation::{ProgressiveFrame, SourceFrame};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape,
    RotaryScaling, MAX_DECODER_PRODUCTS,
    sampling::{SampleBudget, SamplingBudget, SamplingPolicy, SamplingStart},
};
pub const TARGETS: [&str; 7] = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"];
pub fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape {
        vocabulary: 256, hidden: 4, intermediate: 6, layers: 2,
        query_heads: 2, cache_heads: 1, context: 32,
    }, 0.00001, 10000.0).unwrap().with_rotary_scaling(RotaryScaling::linear(4.0).unwrap()).unwrap()
}
pub fn identity() -> DecoderIdentity {
    DecoderIdentity { model_generation: 6, profile_generation: 7, ..profile().identity() }
}
pub fn base_tensors() -> Vec<Tensor> {
    let mut tensors = interchange::tensors(false);
    for (index, name) in ["model.embed_tokens.weight", "lm_head.weight"].into_iter().enumerate() {
        let row = tensors.iter_mut().find(|row| row.name == name).unwrap();
        row.shape = vec![256, 4];
        row.bytes = (0..1024).flat_map(|i| {
            let value = (((i * 3 + (index + 11) * 5) % 11) as i32 - 5) as f32 / 8.0;
            value.to_le_bytes()
        }).collect();
    }
    tensors
}
pub fn base() -> DecoderModel { DecoderModel::from_safetensors(profile(), &encode(&base_tensors())).unwrap().0 }
pub fn config(rank: usize, targets: &[&str]) -> Vec<u8> {
    format!(r#"{{"peft_type":"LORA","task_type":"CAUSAL_LM","inference_mode":true,"r":{rank},"lora_alpha":4,"target_modules":[{}]}}"#,
        targets.iter().map(|target| format!("\"{target}\"")).collect::<Vec<_>>().join(",")).into_bytes()
}
pub fn words(tensor: &Tensor) -> Vec<f32> {
    assert_eq!(tensor.dtype, "F32");
    tensor.bytes.chunks_exact(4).map(|word| f32::from_le_bytes(word.try_into().unwrap())).collect()
}
pub fn adapter(rank: usize, zero: bool, targets: &[&str]) -> Vec<Tensor> {
    let mut output = Vec::new();
    for row in base_tensors() {
        if !targets.iter().any(|target| row.name.ends_with(&format!(".{target}.weight"))) { continue; }
        let output_width = row.shape[0]; let input = row.shape[1];
        let prefix = format!("base_model.model.{}", row.name.strip_suffix(".weight").unwrap());
        for (letter, shape, count) in [("A", vec![rank, input], rank * input),
            ("B", vec![output_width, rank], output_width * rank)]
        {
            let bytes = (0..count).flat_map(|i| {
                let value = if zero { 0.0 } else if letter == "A" {
                    ((i % 5) as i32 - 2) as f32 / 8.0
                } else { (((i * 3 + 1) % 7) as i32 - 3) as f32 / 16.0 };
                value.to_le_bytes()
            }).collect();
            output.push(Tensor { name: format!("{prefix}.lora_{letter}.weight"), shape, dtype: "F32", bytes });
        }
    }
    output
}
pub fn mixed(tensors: &[Tensor]) -> Vec<Tensor> {
    tensors.iter().enumerate().map(|(index, tensor)| {
        let mut t = tensor.clone();
        t.dtype = ["F32", "F16", "BF16"][index % 3];
        t.bytes = words(tensor).into_iter().flat_map(|value| {
            let bits = value.to_bits();
            match t.dtype {
                "F32" => bits.to_le_bytes().to_vec(),
                "BF16" => { assert_eq!(bits & 0xffff, 0); ((bits >> 16) as u16).to_le_bytes().to_vec() }
                _ => {
                    let half = if bits & 0x7fff_ffff == 0 { (bits >> 16) as u16 } else {
                        let exponent = ((bits >> 23) & 255) as i32 - 112;
                        assert!((1..31).contains(&exponent)); assert_eq!(bits & 8191, 0);
                        ((bits >> 16) as u16 & 0x8000) | (exponent as u16) << 10 | ((bits >> 13) as u16 & 1023)
                    };
                    half.to_le_bytes().to_vec()
                }
            }
        }).collect();
        t
    }).collect()
}
pub fn oracle(base: &[Tensor], adapter: &[Tensor], rank: usize) -> Vec<Tensor> {
    let mut output = base.to_vec();
    for a in adapter.iter().filter(|row| row.name.ends_with(".lora_A.weight")) {
        let base_name = a.name.strip_prefix("base_model.model.").unwrap()
            .strip_suffix(".lora_A.weight").unwrap();
        let b_name = a.name.replace(".lora_A.weight", ".lora_B.weight");
        let b = adapter.iter().find(|row| row.name == b_name).unwrap();
        let target = output.iter_mut().find(|row| row.name == format!("{base_name}.weight")).unwrap();
        let out = target.shape[0]; let input = target.shape[1];
        let av = words(a); let bv = words(b); let original = words(target);
        // Accumulate independent rank-one outer products into a dense update.
        let mut delta = vec![vec![0.0_f64; input]; out];
        for k in 0..rank {
            for row in 0..out {
                for col in 0..input { delta[row][col] += f64::from(bv[row * rank + k]) * f64::from(av[k * input + col]); }
            }
        }
        let mut bytes = Vec::new();
        for row in 0..out {
            for col in 0..input {
                let value = (f64::from(original[row * input + col]) + (4.0 / rank as f64) * delta[row][col]) as f32;
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        target.bytes = bytes;
    }
    output
}
pub fn dense(tensors: &[Tensor]) -> DecoderModel {
    let p = DecoderProfile::new(identity(), profile().shape(), profile().epsilon(), profile().theta()).unwrap()
        .with_rotary_scaling(profile().rotary_scaling()).unwrap();
    DecoderModel::from_safetensors(p, &encode(tensors)).unwrap().0
}
pub fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
pub fn sample_budget() -> SampleBudget { SampleBudget { decoder: budget(), sampling: SamplingBudget { vocabulary: 256 } } }
pub fn start() -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(4, 2, 256, 0.8, 7, 0.95).unwrap(), stream: 77, seed: 123 }
}
pub fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
pub fn frame(source: &SourceFrame) -> ProgressiveFrame {
    let bytes = source.encode_initial(23).unwrap();
    ProgressiveFrame::from_initial(&source.verify_block(&bytes).unwrap()).unwrap()
}
pub fn values(source: &SourceFrame) -> Vec<f32> { frame(source).exact_values().unwrap() }
