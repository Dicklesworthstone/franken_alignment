//! Causal native-worker fixtures. The invalid content token really has a higher
//! output-head score than either permitted verdict; no vote is injected.
use super::assets;
use fa_reference::strict_json::{self, Limits};

pub fn configure(fixture: &mut assets::Fixture, grammar: &str) {
    assert!(fixture.manifest.contains("fa.native-worker/2"));
    fixture.manifest = fixture.manifest.replacen("\"schema\":\"fa.native-worker/2\"",
        &format!("\"schema\":\"fa.native-worker/5\",\"adapter\":null,\"verdict_grammar\":\"{grammar}\""), 1)
        .replacen("\"theta\":10000.0", "\"theta\":10000.0,\"rotary\":{\"kind\":\"none\"}", 1);
    fixture.save();
}

pub fn prefer_invalid_token(fixture: &assets::Fixture) {
    set_row(fixture, "model.embed_tokens.weight", usize::from(b'x'), [-1.0, -1.0]);
    set_row(fixture, "lm_head.weight", usize::from(b'x'), [20.0, 0.0]);
}

pub fn hold_terminal(fixture: &assets::Fixture) {
    set_row(fixture, "model.embed_tokens.weight", fixture.policy.stop_tokens[0] as usize, [-2.0, -2.0]);
    let path = fixture.root.join("monitor.json");
    let monitoring = std::fs::read_to_string(&path).unwrap();
    assert!(monitoring.contains("\"threshold\":100"));
    std::fs::write(path, monitoring.replace("\"threshold\":100", "\"threshold\":3")).unwrap();
}

fn set_row(fixture: &assets::Fixture, name: &str, row: usize, values: [f32; 2]) {
    let path = fixture.root.join("weights.safetensors");
    let mut bytes = std::fs::read(&path).unwrap();
    let header_bytes = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let header = strict_json::parse(&bytes[8..8 + header_bytes], Limits::default()).unwrap();
    let tensor = header.get(name).unwrap();
    assert_eq!(tensor.get("dtype").unwrap().as_str(), Some("F32"));
    assert_eq!(tensor.get("shape").unwrap().as_array().unwrap()[1].as_u64(), Some(2));
    let offset = tensor.get("data_offsets").unwrap().as_array().unwrap()[0].as_u64().unwrap() as usize;
    let position = 8 + header_bytes + offset + row * 8;
    for (index, value) in values.iter().enumerate() {
        bytes[position + index * 4..position + (index + 1) * 4].copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}
