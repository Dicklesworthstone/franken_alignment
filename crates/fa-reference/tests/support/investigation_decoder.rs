#![allow(dead_code)]
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::{DecoderIntervention, DecoderLayerIntervention};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::DecoderComparisonBudget;
use fa_reference::action::consequence::activation::tensor::kv::experiment::{KvCell, KvEdit, KvEditScope, KvSide};
use std::collections::BTreeMap;

pub fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 2, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap()
}
pub fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
pub fn comparison_budget() -> DecoderComparisonBudget {
    DecoderComparisonBudget { scalar_products: MAX_DECODER_PRODUCTS, retained_logit_values: 64 }
}

// Zero Q/K/V gives uniform attention and a zero baseline cache. The nonzero O
// matrix makes a changed VALUE causally alter subsequent residuals and choices.
// This intentionally differs from fixtures whose output projection erases edits.
pub fn weights() -> Vec<u8> {
    let mut tensors = BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), (vec![2, 2], vec![1.0_f32, 0.0, 2.0, 0.0])),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0; 2])),
        ("lm_head.weight".to_owned(), (vec![2, 2], vec![1.0, 0.0, 0.0, 1.0])),
    ]);
    for suffix in ["input_layernorm.weight", "post_attention_layernorm.weight"] {
        tensors.insert(format!("model.layers.0.{suffix}"), (vec![2], vec![1.0; 2]));
    }
    for suffix in ["self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight",
        "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"] {
        tensors.insert(format!("model.layers.0.{suffix}"), (vec![2, 2], vec![0.0; 4]));
    }
    tensors.insert("model.layers.0.self_attn.o_proj.weight".to_owned(), (vec![2, 2], vec![4.0, 0.0, 0.0, 4.0]));
    let mut data = Vec::new(); let mut fields = Vec::new();
    for (name, (shape, values)) in tensors {
        let start = data.len();
        for value in values { data.extend_from_slice(&value.to_le_bytes()); }
        fields.push(format!(r#""{name}":{{"dtype":"F32","shape":{shape:?},"data_offsets":[{start},{}]}}"#, data.len()));
    }
    let mut header = format!("{{{}}}", fields.join(","));
    while !header.len().is_multiple_of(8) { header.push(' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header.as_bytes()); bytes.extend(data); bytes
}
pub fn model() -> DecoderModel { DecoderModel::from_safetensors(profile(), &weights()).unwrap().0 }
pub fn layers(value: f32) -> BTreeMap<u64, DecoderLayerIntervention> {
    BTreeMap::from([(1, DecoderLayerIntervention {
        scope: KvEditScope { first_position: 0, token_count: 1, keys: false, values: true },
        edits: vec![KvEdit { cell: KvCell { side: KvSide::Value, position: 0, head: 0, channel: 1 },
            expected_bits: 0.0_f32.to_bits(), replacement_bits: value.to_bits() }],
    })])
}
pub fn plan(value: f32) -> DecoderIntervention {
    let mut run = model().session(5).unwrap();
    run.advance(0, 0, budget()).unwrap();
    run.checkpoint().unwrap().intervene(71, layers(value), 1).unwrap()
}
