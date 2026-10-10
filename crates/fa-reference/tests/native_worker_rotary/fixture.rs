//! Keep the original input-sensitive synthetic weights and original token IDs.
//! Only independently declared static positional semantics vary here.
use super::{assets, v2_assets};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderProfile, RotaryScaling};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, Merge, TokenBytes,
};

pub const NONE: &str = r#"{"kind":"none"}"#;
pub const LINEAR: &str = r#"{"kind":"linear","factor":4}"#;
pub const LLAMA3: &str = r#"{"kind":"llama3","factor":8,"low_freq_factor":1,"high_freq_factor":4,"original_max_position_embeddings":128}"#;

pub fn configure(fixture: &mut assets::Fixture, declared: &str, sharded: bool, json: bool) {
    v2_assets::configure(fixture, sharded, json);
    let (scaling, configuration) = match declared {
        NONE => (RotaryScaling::None, ""),
        LINEAR => (RotaryScaling::linear(4.0).unwrap(),
            r#", "rope_scaling":{"rope_type":"linear","factor":4}"#),
        LLAMA3 => (RotaryScaling::llama3(8.0, 1.0, 4.0, 128).unwrap(),
            r#", "rope_scaling":{"rope_type":"llama3","factor":8,"low_freq_factor":1,"high_freq_factor":4,"original_max_position_embeddings":128}"#),
        _ => panic!("unregistered fixture mode"),
    };
    fixture.policy.decoder_profile = fixture.policy.decoder_profile.clone().with_rotary_scaling(scaling).unwrap();
    let path = fixture.root.join("config.json");
    let original = std::fs::read_to_string(&path).unwrap();
    let body = original.strip_suffix('}').unwrap();
    std::fs::write(path, format!("{body}{configuration}}}")).unwrap();
    assert!(fixture.manifest.contains("\"theta\":10000.0}"));
    fixture.manifest = fixture.manifest.replace("fa.native-worker/2", "fa.native-worker/3")
        .replacen("\"theta\":10000.0}", &format!("\"theta\":10000.0,\"rotary\":{declared}}}"), 1);
    if !json { std::fs::write(fixture.root.join("tokenizer.bin"), tokenizer(&fixture.policy.decoder_profile)).unwrap(); }
    fixture.save();
}

pub fn tokenizer(profile: &DecoderProfile) -> Vec<u8> {
    let mut vocabulary = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect::<Vec<_>>();
    for word in [b"al".as_slice(), b"all", b"allo", b"allow", b"de", b"den", b"deny"] {
        vocabulary.push(TokenBytes::Content(word.to_vec()));
    }
    vocabulary.push(TokenBytes::Control);
    let merges = [(97, 108, 256), (256, 108, 257), (257, 111, 258), (258, 119, 259),
        (100, 101, 260), (260, 110, 261), (261, 121, 262)].into_iter()
        .map(|(left, right, result)| Merge { left, right, result }).collect();
    ByteBpe::new(profile.clone(), vocabulary, merges).unwrap().to_bytes().unwrap()
}
