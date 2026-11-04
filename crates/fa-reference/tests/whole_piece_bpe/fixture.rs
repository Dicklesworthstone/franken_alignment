use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::{
        ByteBpe, Merge, MergePolicy, PreTokenization, TokenBytes,
    },
    tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape},
};
use std::collections::BTreeMap;

pub(super) const CONTROL: u32 = 260;
pub(super) const LLAMA3_PATTERN: &str = r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+";

pub(super) fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 11, model: 12, model_generation: 13,
        tokenizer_generation: 14, profile_generation: 15 }, DecoderShape {
        vocabulary: 261, hidden: 2, intermediate: 2, layers: 1, query_heads: 1,
        cache_heads: 1, context: 64 }, 1e-5, 10000.0).unwrap()
}

pub(super) fn vocabulary(direct_only: bool) -> Vec<TokenBytes> {
    // The numeric byte values deliberately differ from the original token IDs.
    let mut vocabulary = vec![TokenBytes::Control; 256];
    for byte in 0..=255_u8 {
        vocabulary[(usize::from(byte) + 17) % 256] = TokenBytes::Content(vec![byte]);
    }
    vocabulary.extend([TokenBytes::Content(b"ab".to_vec()),
        TokenBytes::Content(b"bc".to_vec()),
        TokenBytes::Content(if direct_only { b"xyz".to_vec() } else { b"abc".to_vec() }),
        TokenBytes::Content(b"a1".to_vec()), TokenBytes::Control]);
    vocabulary
}

pub(super) fn merges(direct_only: bool) -> Vec<Merge> {
    let mut rules = vec![Merge { left: 114, right: 115, result: 256 },
        Merge { left: 115, right: 116, result: 257 }];
    // A lower-ranked ab prevents abc from forming through a+bc on actual input.
    // abc is still graph-reachable, so both policies admit the SAME inventory.
    if !direct_only { rules.push(Merge { left: 114, right: 257, result: 258 }); }
    rules.push(Merge { left: 114, right: 66, result: 259 });
    rules
}

pub(super) fn names() -> BTreeMap<u32, Vec<u8>> {
    BTreeMap::from([(CONTROL, b"<q>".to_vec())])
}

pub(super) fn tokenizer(mode: PreTokenization, policy: MergePolicy) -> ByteBpe {
    ByteBpe::new_with_merge_policy(profile(), vocabulary(false), merges(false), names(),
        mode, policy).unwrap()
}

fn glyph(byte: u8) -> char {
    char::from_u32(match byte {
        0..=32 => u32::from(byte) + 256,
        33..=126 | 161..=172 | 174..=255 => u32::from(byte),
        127..=160 => u32::from(byte) + 162,
        173 => 323,
    }).unwrap()
}
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
fn spelling(bytes: &[u8]) -> String { bytes.iter().map(|byte| glyph(*byte)).collect() }

pub(super) fn document(mode: PreTokenization, whole_piece: bool, direct_only: bool, bos: bool) -> String {
    let vocabulary = vocabulary(direct_only);
    let entries = vocabulary.iter().enumerate().filter_map(|(id, token)| match token {
        TokenBytes::Content(bytes) => Some(format!("{}:{id}", quote(&spelling(bytes)))),
        TokenBytes::Control => None,
    }).collect::<Vec<_>>().join(",");
    let rules = merges(direct_only).iter().map(|rule| {
        let content = |id: u32| match &vocabulary[id as usize] {
            TokenBytes::Content(bytes) => spelling(bytes),
            TokenBytes::Control => panic!("fixture control in merge"),
        };
        format!("[{},{}]", quote(&content(rule.left)), quote(&content(rule.right)))
    }).collect::<Vec<_>>().join(",");
    let pre = match mode {
        PreTokenization::RawBytes => r#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}"#.to_owned(),
        PreTokenization::Gpt2Unicode17 => r#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":true}"#.to_owned(),
        PreTokenization::Llama3Unicode17 => format!(
            r#"{{"type":"Sequence","pretokenizers":[{{"type":"Split","pattern":{{"Regex":{}}},"behavior":"Isolated","invert":false}},{{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}}]}}"#,
            quote(LLAMA3_PATTERN)),
    };
    let post = if bos {
        r#"{"type":"TemplateProcessing","single":[{"SpecialToken":{"id":"<q>","type_id":0}},{"Sequence":{"id":"A","type_id":0}}],"pair":[{"SpecialToken":{"id":"<q>","type_id":0}},{"Sequence":{"id":"A","type_id":0}},{"SpecialToken":{"id":"<q>","type_id":1}},{"Sequence":{"id":"B","type_id":1}}],"special_tokens":{"<q>":{"id":"<q>","ids":[260],"tokens":["<q>"]}}}"#
    } else { "null" };
    format!(concat!(
        "{{\"version\":\"1.0\",\"truncation\":null,\"padding\":null,",
        "\"added_tokens\":[{{\"id\":260,\"content\":\"<q>\",\"single_word\":false,",
        "\"lstrip\":false,\"rstrip\":false,\"normalized\":false,\"special\":true}}],",
        "\"normalizer\":null,\"pre_tokenizer\":{},\"post_processor\":{},",
        "\"decoder\":{{\"type\":\"ByteLevel\",\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"model\":{{\"type\":\"BPE\",\"dropout\":null,\"unk_token\":null,",
        "\"continuing_subword_prefix\":null,\"end_of_word_suffix\":null,",
        "\"fuse_unk\":false,\"byte_fallback\":false,\"ignore_merges\":{},",
        "\"vocab\":{{{}}},\"merges\":[{}]}}}}"), pre, post, whole_piece, entries, rules)
}
