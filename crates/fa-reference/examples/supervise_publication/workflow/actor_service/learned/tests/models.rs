//! Synthetic weight-controlled outputs, imported through the real public codecs.
//! These fixtures exercise contracts; they do not claim trained-model quality.
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape,
};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, TokenBytes, Merge,
};
use std::{collections::BTreeMap, fs, path::Path};

pub(super) fn identity(model: u64) -> DecoderIdentity {
    DecoderIdentity { tenant: 1, model, model_generation: 1, tokenizer_generation: 1, profile_generation: 1 }
}
pub(super) fn configuration(count: usize, context: usize) -> String {
    format!(r#"{{"model_type":"llama","vocab_size":{count},"hidden_size":2,"intermediate_size":2,"num_hidden_layers":1,"num_attention_heads":1,"num_key_value_heads":1,"max_position_embeddings":{context},"rms_norm_eps":0.00001,"rope_theta":10000.0}}"#)
}
fn profile(model: u64, count: usize, context: usize) -> DecoderProfile {
    DecoderProfile::new(identity(model), DecoderShape { vocabulary: count, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context },
        0.00001, 10000.0).unwrap()
}

pub(super) fn weights(count: usize, response: usize, stop: usize, kv: bool) -> Vec<u8> {
    let mut embeddings: Vec<f32> = (0..count).flat_map(|_| [1.0, 0.0]).collect();
    embeddings[response * 2] = 0.0; embeddings[response * 2 + 1] = 1.0;
    let mut head = vec![0.0_f32; count * 2];
    head[response * 2] = 10.0; head[stop * 2 + 1] = 10.0;
    let mut tensors = BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), (vec![count, 2], embeddings)),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0_f32; 2])),
        ("lm_head.weight".to_owned(), (vec![count, 2], head)),
    ]);
    for name in ["input_layernorm.weight", "post_attention_layernorm.weight"] {
        tensors.insert(format!("model.layers.0.{name}"), (vec![2], vec![1.0; 2]));
    }
    for name in ["self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight",
        "self_attn.o_proj.weight", "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"] {
        let values = if kv && matches!(name, "self_attn.k_proj.weight" | "self_attn.v_proj.weight") {
            vec![1.0, 0.0, 0.0, 1.0]
        } else { vec![0.0; 4] };
        tensors.insert(format!("model.layers.0.{name}"), (vec![2, 2], values));
    }
    let mut payload = Vec::new(); let mut fields = Vec::new();
    for (name, (shape, values)) in tensors {
        let start = payload.len();
        for value in values { payload.extend_from_slice(&value.to_le_bytes()); }
        fields.push(format!(r#""{name}":{{"dtype":"F32","shape":{shape:?},"data_offsets":[{start},{}]}}"#, payload.len()));
    }
    let mut header = format!("{{{}}}", fields.join(","));
    while header.len() % 8 != 0 { header.push(' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header.as_bytes()); bytes.extend(payload); bytes
}

pub(super) fn sampling(count: usize) -> String {
    format!(r#"{{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":{count},"temperature":1.0,"top_k":1,"top_p":1.0,"stream":21,"seed":9}}"#)
}
pub(super) fn actor(root: &Path) -> DecoderModel {
    let configuration = configuration(257, 16);
    let weights = weights(257, 65, 256, true);
    fs::write(root.join("model.json"), &configuration).unwrap();
    fs::write(root.join("weights.safetensors"), &weights).unwrap();
    fs::write(root.join("sampling.json"), sampling(257)).unwrap();
    let tokenizer = crate::workflow::actor_service::tokenizer::fixture::raw_byte_level_json(false);
    fs::write(root.join("tokenizer.json"), tokenizer).unwrap();
    fs::write(root.join("prompt.txt"), b"xy").unwrap();
    DecoderModel::from_llama_safetensors(identity(9), 16, configuration.as_bytes(), &weights).unwrap().0
}

fn word(vocabulary: &mut Vec<TokenBytes>, merges: &mut Vec<Merge>, bytes: &[u8]) -> u32 {
    let mut left = u32::from(bytes[0]);
    for end in 2..=bytes.len() {
        if let Some(id) = vocabulary.iter().position(|token| matches!(token,
            TokenBytes::Content(value) if value == &bytes[..end])) {
            left = id as u32;
            continue;
        }
        let id = vocabulary.len() as u32;
        vocabulary.push(TokenBytes::Content(bytes[..end].to_vec()));
        merges.push(Merge { left, right: u32::from(bytes[end - 1]), result: id });
        left = id;
    }
    left
}
pub(super) fn native(root: &Path, spelling: &[u8]) -> usize {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    let mut merges = Vec::new();
    let response = word(&mut vocabulary, &mut merges, spelling);
    // Real reusable BPE pieces for the binary/text input, not a precomputed
    // action token or omitted prompt. Every original input byte is tokenized.
    word(&mut vocabulary, &mut merges, b"Should this exact publication proceed?");
    word(&mut vocabulary, &mut merges, b"reference/helper/alpha");
    word(&mut vocabulary, &mut merges, b"reference/helper/beta");
    let mut zero = 0_u32;
    for length in [2, 4, 8, 16, 32, 64] {
        let result = vocabulary.len() as u32;
        vocabulary.push(TokenBytes::Content(vec![0; length]));
        merges.push(Merge { left: zero, right: zero, result });
        zero = result;
    }
    let stop = vocabulary.len();
    vocabulary.push(TokenBytes::Control);
    let count = vocabulary.len();
    let profile = profile(31, count, 4096);
    let tokenizer = ByteBpe::new(profile, vocabulary, merges).unwrap();
    fs::write(root.join("helper-model.json"), configuration(count, 4096)).unwrap();
    fs::write(root.join("helper-weights.safetensors"), weights(count, response as usize, stop, false)).unwrap();
    fs::write(root.join("helper-tokenizer.bbpe"), tokenizer.to_bytes().unwrap()).unwrap();
    fs::write(root.join("helper-sampling.json"), sampling(count)).unwrap();
    fs::write(root.join("helper-monitor.json"), br#"{"schema":"fa.decoder-monitor/1","generation":1,"identity":{"tenant":1,"model":31,"model_generation":1,"tokenizer_generation":1,"profile_generation":1},"budget":{"encoded_bytes":100000,"probe_coordinates":100000},"layers":[{"layer":1,"levels":[23],"budget":{"encoded_bytes":100000,"probe_coordinates":100000},"probes":[{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":100.0}]}]}"#).unwrap();
    stop
}
