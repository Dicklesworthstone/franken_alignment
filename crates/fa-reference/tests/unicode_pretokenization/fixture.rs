use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::{ByteBpe, Merge, PreTokenization, TokenBytes},
    tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape},
};
use std::collections::BTreeMap;

// Independent external pattern literal, not the production profile's accessor.
pub(super) const LLAMA3_REGEX: &str = r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+";

#[derive(Clone)]
pub(super) struct Fixture {
    pub profile: DecoderProfile,
    pub vocabulary: Vec<TokenBytes>,
    pub merges: Vec<Merge>,
    pub names: BTreeMap<u32, Vec<u8>>,
}
pub(super) fn byte_id(byte: u8) -> u32 { (u32::from(byte) + 17) % 256 }
pub(super) fn quote(value: &str) -> String {
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
fn spelling(bytes: &[u8]) -> String { bytes.iter().map(|byte| glyph(*byte)).collect() }

impl Fixture {
    pub fn new(named: bool) -> Self {
        let mut vocabulary = vec![TokenBytes::Control; 256];
        for byte in 0..=255_u8 {
            vocabulary[byte_id(byte) as usize] = TokenBytes::Content(vec![byte]);
        }
        let mut merges = Vec::new();
        // Explicit cross-class and permitted within-piece merges, followed by
        // actual native helper answers. Every multibyte token is reachable.
        for word in ["a1", " a", "  ", "'s", "'S", "12", "123", "1234", "allow", "deny",
            "é", "'ſ", "ſa", "１２３４"]
        {
            for length in 2..=word.len() {
                let bytes = &word.as_bytes()[..length];
                if vocabulary.iter().any(|entry| matches!(entry, TokenBytes::Content(value) if value == bytes)) {
                    continue;
                }
                let left = vocabulary.iter().position(|entry| matches!(entry,
                    TokenBytes::Content(value) if value == &bytes[..length - 1])).unwrap() as u32;
                let result = vocabulary.len() as u32;
                vocabulary.push(TokenBytes::Content(bytes.to_vec()));
                merges.push(Merge { left, right: byte_id(bytes[length - 1]), result });
            }
        }
        let mut names = BTreeMap::new();
        if named {
            for name in ["<eos>", "<qé>"] {
                names.insert(vocabulary.len() as u32, name.as_bytes().to_vec());
                vocabulary.push(TokenBytes::Control);
            }
        }
        let profile = DecoderProfile::new(DecoderIdentity {
            tenant: 1, model: 2, model_generation: 3, tokenizer_generation: 4, profile_generation: 5,
        }, DecoderShape { vocabulary: vocabulary.len(), hidden: 2, intermediate: 2,
            layers: 1, query_heads: 1, cache_heads: 1, context: 64 }, 1e-5, 10000.0).unwrap();
        Self { profile, vocabulary, merges, names }
    }

    pub fn id(&self, bytes: &[u8]) -> u32 {
        self.vocabulary.iter().position(|entry| matches!(entry,
            TokenBytes::Content(value) if value == bytes)).unwrap() as u32
    }

    pub fn named_id(&self, name: &[u8]) -> u32 {
        *self.names.iter().find(|(_, value)| value.as_slice() == name).unwrap().0
    }

    pub fn tokenizer(&self, mode: PreTokenization) -> ByteBpe {
        ByteBpe::new_with_pre_tokenization(self.profile.clone(), self.vocabulary.clone(),
            self.merges.clone(), self.names.clone(), mode).unwrap()
    }

    pub fn json(&self, mode: PreTokenization) -> String {
        let entries: Vec<_> = self.vocabulary.iter().enumerate().filter_map(|(id, entry)| match entry {
            TokenBytes::Content(bytes) => Some(format!("{}:{id}", quote(&spelling(bytes)))),
            TokenBytes::Control => None,
        }).collect();
        let names = self.names.iter().map(|(id, name)| format!(
            r#"{{"id":{id},"content":{},"single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}}"#,
            quote(std::str::from_utf8(name).unwrap()))).collect::<Vec<_>>().join(",");
        let merges = self.merges.iter().map(|rule| {
            let bytes = |id: u32| match &self.vocabulary[id as usize] {
                TokenBytes::Content(value) => value.as_slice(),
                TokenBytes::Control => panic!("fixture merge cannot contain a control"),
            };
            format!("[{},{}]", quote(&spelling(bytes(rule.left))), quote(&spelling(bytes(rule.right))))
        }).collect::<Vec<_>>().join(",");
        let pre = match mode {
            PreTokenization::RawBytes => r#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}"#.to_string(),
            PreTokenization::Gpt2Unicode17 => r#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":true,"use_regex":true}"#.to_string(),
            PreTokenization::Llama3Unicode17 => format!(
                r#"{{"type":"Sequence","pretokenizers":[{{"type":"Split","pattern":{{"Regex":{}}},"behavior":"Isolated","invert":false}},{{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":true,"use_regex":false}}]}}"#,
                quote(LLAMA3_REGEX)),
        };
        let decoder = if mode == PreTokenization::RawBytes {
            r#"{"type":"ByteLevel","add_prefix_space":false,"trim_offsets":false,"use_regex":false}"#
        } else {
            r#"{"type":"ByteLevel","add_prefix_space":true,"trim_offsets":true,"use_regex":true}"#
        };
        format!(r#"{{"version":"1.0","truncation":null,"padding":null,"added_tokens":[{names}],"normalizer":null,"pre_tokenizer":{pre},"post_processor":null,"decoder":{decoder},"model":{{"type":"BPE","dropout":null,"unk_token":null,"continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":false,"vocab":{{{}}},"merges":[{merges}]}}}}"#,
            entries.join(","))
    }

    pub fn imported(&self, mode: PreTokenization) -> ByteBpe {
        let json = self.json(mode);
        ByteBpe::from_huggingface_json_with_pre_tokenization(&self.profile, json.as_bytes(), json.len(), mode).unwrap()
    }
}
