use std::collections::BTreeMap;

// Analytic model: the sole layer preserves embeddings [1,0] / [2,0]. The
// vocabulary head prefers token 1. top_k=1 still consumes one original RNG draw.
pub fn weights() -> Vec<u8> {
    let mut tensors = BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), (vec![2, 2], vec![1.0_f32, 0.0, 2.0, 0.0])),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0; 2])),
        ("lm_head.weight".to_owned(), (vec![2, 2], vec![0.0, 0.0, 1.0, 0.0])),
    ]);
    for suffix in ["input_layernorm.weight", "post_attention_layernorm.weight"] {
        tensors.insert(format!("model.layers.0.{suffix}"), (vec![2], vec![1.0; 2]));
    }
    for suffix in ["self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight",
        "self_attn.o_proj.weight", "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"] {
        tensors.insert(format!("model.layers.0.{suffix}"), (vec![2, 2], vec![0.0; 4]));
    }
    let mut data = Vec::new(); let mut fields = Vec::new();
    for (name, (shape, values)) in tensors {
        let start = data.len();
        for value in values { data.extend_from_slice(&value.to_le_bytes()); }
        fields.push(format!(r#""{name}":{{"dtype":"F32","shape":{shape:?},"data_offsets":[{start},{}]}}"#, data.len()));
    }
    let mut header = format!("{{{}}}", fields.join(","));
    while header.len() % 8 != 0 { header.push(' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header.as_bytes()); bytes.extend(data); bytes
}

pub fn monitor(threshold: f32) -> Vec<u8> {
    r#"{"schema":"fa.decoder-monitor/1","generation":1,
      "identity":{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1},
      "budget":{"encoded_bytes":10000,"probe_coordinates":10000},
      "layers":[{"layer":1,"levels":[23],"budget":{"encoded_bytes":10000,"probe_coordinates":10000},
        "probes":[{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":THRESHOLD}]}]}"#
        .replace("THRESHOLD", &threshold.to_string()).into_bytes()
}
pub fn sampling() -> Vec<u8> {
    br#"{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":2,
      "temperature":1.0,"top_k":1,"top_p":1.0,"stream":21,"seed":9}"#.to_vec()
}
