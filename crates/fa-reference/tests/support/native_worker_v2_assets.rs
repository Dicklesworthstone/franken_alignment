//! Explicit version-two fixtures over the existing synthetic numerical assets.
//! Repacking weights supplies input data, not an expected output or live proof.
use super::assets;
use fa_reference::strict_json::{self, Limits};
use std::fs;

pub fn quote(value: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            ch if ch.is_control() => quoted.push_str(&format!("\\u{:04x}", u32::from(ch))),
            ch => quoted.push(ch),
        }
    }
    quoted.push('"'); quoted
}

pub fn configure(fixture: &mut assets::Fixture, sharded: bool, json: bool) {
    assert_eq!(fixture.policy.stop_tokens, [263]);
    assert!(fixture.manifest.contains("\"schema\":\"fa.native-worker/1\""));
    let format = if json { "huggingface-raw-bytelevel" } else { "native-archive" };
    if json { fs::write(fixture.root.join("tokenizer.bin"), tokenizer_json()).unwrap(); }
    let weights = quote(fixture.root.join("weights.safetensors").to_str().unwrap());
    let source = if sharded {
        let file = fs::read(fixture.root.join("weights.safetensors")).unwrap();
        let header_bytes = usize::try_from(u64::from_le_bytes(file[..8].try_into().unwrap())).unwrap();
        let header = strict_json::parse(&file[8..8 + header_bytes], Limits::default()).unwrap();
        let raw = &file[8 + header_bytes..];
        let mut map = Vec::new(); let mut registered = Vec::new();
        for part in 0..2 {
            let label = format!("part-{part}.safetensors");
            let mut entries = Vec::new(); let mut body = Vec::new();
            for (name, tensor) in header.as_object().unwrap() {
                if name.starts_with("model.layers.") != (part == 1) { continue; }
                assert_eq!(tensor.get("dtype").unwrap().as_str(), Some("F32"));
                let offsets = tensor.get("data_offsets").unwrap().as_array().unwrap();
                let start = offsets[0].as_u64().unwrap() as usize;
                let end = offsets[1].as_u64().unwrap() as usize;
                let shape = tensor.get("shape").unwrap().as_array().unwrap().iter()
                    .map(|size| size.as_u64().unwrap().to_string()).collect::<Vec<_>>().join(",");
                let first = body.len(); body.extend_from_slice(&raw[start..end]);
                entries.push(format!("{}:{{\"dtype\":\"F32\",\"shape\":[{shape}],\"data_offsets\":[{first},{}]}}", quote(name), body.len()));
                map.push(format!("{}:{}", quote(name), quote(&label)));
            }
            assert!(!entries.is_empty());
            let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
            while !header.len().is_multiple_of(8) { header.push(b' '); }
            let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
            bytes.extend_from_slice(&header); bytes.extend_from_slice(&body);
            let path = fixture.root.join(format!("registered-{part}.bin"));
            fs::write(&path, bytes).unwrap();
            registered.push(format!("{}:{}", quote(&label), quote(path.to_str().unwrap())));
        }
        let index = fixture.root.join("weights.index.json");
        fs::write(&index, format!("{{\"weight_map\":{{{}}}}}", map.join(","))).unwrap();
        format!("{{\"kind\":\"sharded\",\"index\":{},\"shards\":{{{}}}}}",
            quote(index.to_str().unwrap()), registered.join(","))
    } else { format!("{{\"kind\":\"single\",\"path\":{weights}}}") };
    let old = format!("\"weights\":{weights}");
    assert!(fixture.manifest.contains(&old));
    fixture.manifest = fixture.manifest.replacen(&old, &format!("\"weights\":{source}"), 1)
        .replacen("\"schema\":\"fa.native-worker/1\"",
            &format!("\"schema\":\"fa.native-worker/2\",\"tokenizer_format\":\"{format}\""), 1);
    fixture.save();
}

fn tokenizer_json() -> Vec<u8> {
    // Same manually specified original IDs as native_worker_assets, not a
    // conversion by the importer under test. ByteLevel glyphs are data only.
    let mut vocabulary = (0_u16..=255).map(|byte| {
        let code = match byte {
            0..=32 => u32::from(byte) + 256,
            33..=126 | 161..=172 | 174..=255 => u32::from(byte),
            127..=160 => u32::from(byte) + 162,
            173 => 323,
            _ => unreachable!("byte"),
        };
        format!("{}:{byte}", quote(&char::from_u32(code).unwrap().to_string()))
    }).collect::<Vec<_>>();
    for (offset, word) in ["al", "all", "allo", "allow", "de", "den", "deny"].iter().enumerate() {
        vocabulary.push(format!("{}:{}", quote(word), offset + 256));
    }
    format!(concat!(
        "{{\"version\":\"1.0\",\"truncation\":null,\"padding\":null,",
        "\"added_tokens\":[{{\"id\":263,\"content\":\"<eos>\",\"single_word\":false,",
        "\"lstrip\":false,\"rstrip\":false,\"normalized\":false,\"special\":true}}],",
        "\"normalizer\":null,\"pre_tokenizer\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"post_processor\":null,\"decoder\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"model\":{{\"type\":\"BPE\",\"dropout\":null,\"unk_token\":null,",
        "\"continuing_subword_prefix\":null,\"end_of_word_suffix\":null,",
        "\"fuse_unk\":false,\"byte_fallback\":false,\"ignore_merges\":false,",
        "\"vocab\":{{{}}},\"merges\":[[\"a\",\"l\"],[\"al\",\"l\"],",
        "[\"all\",\"o\"],[\"allo\",\"w\"],[\"d\",\"e\"],[\"de\",\"n\"],[\"den\",\"y\"]]}}}}"
    ), vocabulary.join(",")).into_bytes()
}
