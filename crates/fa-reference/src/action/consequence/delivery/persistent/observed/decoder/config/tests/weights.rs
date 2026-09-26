//! Genuine tied-head numerical fixture shared by durable and executable tests.
//! Two-dimensional residuals produce ab -> A -> Control under the real sampler.
//! This is synthetic computation, not trained model or detector qualification.
pub(super) fn tied_weights(stored_head: bool, conflict: bool) -> Vec<u8> {
    tied_weights_for(259, 257, stored_head, conflict)
}

pub(super) fn tied_weights_for(vocabulary: usize, prompt: usize, stored_head: bool, conflict: bool) -> Vec<u8> {
    assert!(vocabulary > 256 && prompt < vocabulary && prompt != 65 && prompt != 256);
    let mut embeddings: Vec<f32> = (0..vocabulary).flat_map(|_| [0.1, 0.0]).collect();
    embeddings[65 * 2] = 2.0; embeddings[65 * 2 + 1] = 1.0;
    embeddings[256 * 2] = 0.0; embeddings[256 * 2 + 1] = 6.0;
    embeddings[prompt * 2] = 1.0;
    let mut tensors = std::collections::BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), (vec![vocabulary, 2], embeddings.clone())),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0_f32; 2])),
    ]);
    if stored_head {
        // Numerically equal signed zero is NOT an equal normalized parameter bit.
        if conflict { embeddings[1] = -0.0; }
        tensors.insert("lm_head.weight".to_owned(), (vec![vocabulary, 2], embeddings));
    }
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
    bytes.extend_from_slice(header.as_bytes()); bytes.extend(data);
    bytes
}
