use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::{ByteBpe, Merge, PreTokenization, TokenBytes},
    tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape},
};
use fa_reference::Error;
use std::collections::BTreeMap;

pub(super) const BOS: u32 = 259;
pub(super) const BOS_NAME: &str = "<|begin_of_text|>";
// Independent upstream spelling, not the implementation's grammar accessor.
const LLAMA3_REGEX: &str = r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+";
pub(super) const TEMPLATE: &str = r#"{"type":"TemplateProcessing","single":[{"SpecialToken":{"id":"<|begin_of_text|>","type_id":0}},{"Sequence":{"id":"A","type_id":0}}],"pair":[{"SpecialToken":{"id":"<|begin_of_text|>","type_id":0}},{"Sequence":{"id":"A","type_id":0}},{"SpecialToken":{"id":"<|begin_of_text|>","type_id":1}},{"Sequence":{"id":"B","type_id":1}}],"special_tokens":{"<|begin_of_text|>":{"id":"<|begin_of_text|>","ids":[259],"tokens":["<|begin_of_text|>"]}}}"#;

pub(super) fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 11, model: 12, model_generation: 13,
        tokenizer_generation: 14, profile_generation: 15 }, DecoderShape {
        vocabulary: 261, hidden: 2, intermediate: 2, layers: 1, query_heads: 1,
        cache_heads: 1, context: 64 }, 1e-5, 10000.0).unwrap()
}

pub(super) fn vocabulary() -> Vec<TokenBytes> {
    // All byte IDs are permuted; BOS is neither zero nor the final token ID.
    let mut vocabulary = vec![TokenBytes::Control; 256];
    for byte in 0..=255_u8 {
        vocabulary[(usize::from(byte) + 17) % 256] = TokenBytes::Content(vec![byte]);
    }
    vocabulary.extend([TokenBytes::Content(b"aa".to_vec()),
        TokenBytes::Content(b"a1".to_vec()), TokenBytes::Content("é".as_bytes().to_vec()),
        TokenBytes::Control, TokenBytes::Control]);
    vocabulary
}

pub(super) fn merges() -> Vec<Merge> {
    vec![Merge { left: 114, right: 114, result: 256 },
        Merge { left: 114, right: 66, result: 257 },
        Merge { left: 212, right: 186, result: 258 }]
}

pub(super) fn names() -> BTreeMap<u32, Vec<u8>> {
    BTreeMap::from([(BOS, BOS_NAME.as_bytes().to_vec()), (260, b"<end>".to_vec())])
}

pub(super) fn tokenizer() -> ByteBpe {
    ByteBpe::new_with_bos_token(profile(), vocabulary(), merges(), names(), BOS).unwrap()
}

pub(super) fn plain(mode: PreTokenization) -> ByteBpe {
    ByteBpe::new_with_pre_tokenization(profile(), vocabulary(), merges(), names(), mode).unwrap()
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn glyph(byte: u8) -> char {
    char::from_u32(match byte {
        0..=32 => u32::from(byte) + 256,
        33..=126 | 161..=172 | 174..=255 => u32::from(byte),
        127..=160 => u32::from(byte) + 162,
        173 => 323,
    }).unwrap()
}

pub(super) fn document(post_processor: &str) -> String {
    let mut entries = (0..=255_u8).rev().map(|byte| {
        format!("{}:{}", quote(&glyph(byte).to_string()), (u32::from(byte) + 17) % 256)
    }).collect::<Vec<_>>();
    entries.extend([r#""aa":256"#.to_string(), r#""a1":257"#.to_string(),
        r#""Ã©":258"#.to_string()]);
    let added = r#"[{"id":259,"content":"<|begin_of_text|>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},{"id":260,"content":"<end>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}]"#;
    format!(r#"{{"version":"1.0","truncation":null,"padding":null,"added_tokens":{added},"normalizer":null,"pre_tokenizer":{{"type":"Sequence","pretokenizers":[{{"type":"Split","pattern":{{"Regex":{}}},"behavior":"Isolated","invert":false}},{{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":true,"use_regex":false}}]}},"post_processor":{post_processor},"decoder":{{"type":"ByteLevel","add_prefix_space":true,"trim_offsets":true,"use_regex":true}},"model":{{"type":"BPE","dropout":null,"unk_token":null,"continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":false,"vocab":{{{}}},"merges":[["a","a"],["a","1"],["Ã","©"]]}}}}"#,
        quote(LLAMA3_REGEX), entries.join(","))
}

pub(super) fn import(document: &str) -> Result<ByteBpe, Error> {
    ByteBpe::from_huggingface_llama3_bos_json(&profile(), document.as_bytes(), document.len())
}

pub(super) fn wrapped(template: &str, add_prefix_space: bool, use_regex: bool) -> String {
    format!(r#"{{"type":"Sequence","processors":[{{"type":"ByteLevel","add_prefix_space":{add_prefix_space},"trim_offsets":false,"use_regex":{use_regex}}},{template}]}}"#)
}
