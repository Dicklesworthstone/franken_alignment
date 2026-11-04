//! Explicit operator input selection; the original native parsers own semantics.
//! No content sniffing, format fallback, inferred model identity or file access.
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{ByteBpe, MergePolicy, PreTokenization};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderProfile;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::text::MAX_FILE_TOKENIZER_BYTES;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::NativeTokenizerFormat;
use fa_reference::strict_json::Json;

pub(super) struct TokenizerInput {
    format: NativeTokenizerFormat,
    path: String,
}

impl TokenizerInput {
    /// Legacy recipe versions require their original native-archive path string.
    /// New versions require BOTH fields, even for the native archive format.
    pub(super) fn parse(value: Json, explicit: bool) -> Result<Self, String> {
        let (format, path) = if explicit {
            let Json::Object(mut fields) = value else {
                return Err("tokenizer input must be a format/path object".into());
            };
            let format = fields.remove("format").ok_or("missing tokenizer format")?;
            let path = fields.remove("path").ok_or("missing tokenizer path")?;
            if !fields.is_empty() {
                return Err("unknown tokenizer input field".into());
            }
            let format = match format.as_str() {
                Some("native_archive") => NativeTokenizerFormat::NativeArchive,
                Some("huggingface_raw_bytelevel") => NativeTokenizerFormat::HuggingFaceRawByteLevel,
                Some("huggingface_gpt2_unicode17") => NativeTokenizerFormat::HuggingFaceGpt2Unicode17,
                Some("huggingface_llama3_unicode17") => NativeTokenizerFormat::HuggingFaceLlama3Unicode17,
                Some("huggingface_llama3_unicode17_bos") => NativeTokenizerFormat::HuggingFaceLlama3Unicode17Bos,
                Some("huggingface_llama3_unicode17_whole_piece") => NativeTokenizerFormat::HuggingFaceLlama3Unicode17WholePiece,
                Some("huggingface_llama3_unicode17_bos_whole_piece") => NativeTokenizerFormat::HuggingFaceLlama3Unicode17BosWholePiece,
                _ => return Err("unknown tokenizer format".into()),
            };
            (format, path)
        } else {
            (NativeTokenizerFormat::NativeArchive, value)
        };
        let path = path.as_str().ok_or("tokenizer path must be text")?;
        if path.is_empty() || path.as_bytes().contains(&0) {
            return Err("invalid tokenizer input path".into());
        }
        Ok(Self {
            format,
            path: path.to_owned(),
        })
    }

    pub(super) fn path(&self) -> &str {
        &self.path
    }

    pub(super) fn byte_limit(&self) -> usize {
        match self.format {
            NativeTokenizerFormat::NativeArchive => MAX_FILE_TOKENIZER_BYTES,
            NativeTokenizerFormat::HuggingFaceRawByteLevel
                | NativeTokenizerFormat::HuggingFaceGpt2Unicode17
                | NativeTokenizerFormat::HuggingFaceLlama3Unicode17
                | NativeTokenizerFormat::HuggingFaceLlama3Unicode17Bos
                | NativeTokenizerFormat::HuggingFaceLlama3Unicode17WholePiece
                | NativeTokenizerFormat::HuggingFaceLlama3Unicode17BosWholePiece => ByteBpe::MAX_HUGGINGFACE_JSON_BYTES,
        }
    }

    pub(super) fn decode(&self, profile: &DecoderProfile, bytes: &[u8]) -> Result<ByteBpe, String> {
        match self.format {
            NativeTokenizerFormat::NativeArchive => ByteBpe::from_bytes(profile, bytes),
            NativeTokenizerFormat::HuggingFaceRawByteLevel =>
                ByteBpe::from_huggingface_json(profile, bytes, self.byte_limit()),
            NativeTokenizerFormat::HuggingFaceGpt2Unicode17 =>
                ByteBpe::from_huggingface_json_with_pre_tokenization(profile, bytes, self.byte_limit(), PreTokenization::Gpt2Unicode17),
            NativeTokenizerFormat::HuggingFaceLlama3Unicode17 =>
                ByteBpe::from_huggingface_json_with_pre_tokenization(profile, bytes, self.byte_limit(), PreTokenization::Llama3Unicode17),
            NativeTokenizerFormat::HuggingFaceLlama3Unicode17Bos =>
                ByteBpe::from_huggingface_llama3_bos_json(profile, bytes, self.byte_limit()),
            NativeTokenizerFormat::HuggingFaceLlama3Unicode17WholePiece =>
                ByteBpe::from_huggingface_json_with_merge_policy(profile, bytes, self.byte_limit(),
                    PreTokenization::Llama3Unicode17, MergePolicy::WholePieceFirst),
            NativeTokenizerFormat::HuggingFaceLlama3Unicode17BosWholePiece =>
                ByteBpe::from_huggingface_llama3_bos_json_with_merge_policy(profile, bytes,
                    self.byte_limit(), MergePolicy::WholePieceFirst),
        }.map_err(|error| format!("tokenizer input refused: {error:?}"))
    }
}

#[cfg(test)]
#[path = "tokenizer/fixture.rs"]
pub(super) mod fixture;

#[cfg(test)]
#[path = "tokenizer/direct.rs"]
pub(super) mod direct;

#[cfg(test)]
mod whole_piece_tests {
    use super::*;
    use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderShape};
    use fa_reference::strict_json::{self, Limits};

    #[test]
    fn tokenizer_whole_piece_operator_selection_preserves_ids_and_canonical_policy() {
        let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
            tokenizer_generation: 4, profile_generation: 5 }, DecoderShape {
            vocabulary: 259, hidden: 2, intermediate: 2, layers: 1, query_heads: 1,
            cache_heads: 1, context: 64 }, 1e-5, 10000.0).unwrap();
        let raw = fixture::raw_byte_level_json(true);
        assert!(raw.contains("[\"a\",\"b\"],"));
        let raw = raw.replace("\"ignore_merges\":false", "\"ignore_merges\":true")
            .replace("[\"a\",\"b\"],", "");
        for bos in [false, true] {
            let (format, json) = if bos {
                ("huggingface_llama3_unicode17_bos_whole_piece",
                    fixture::unicode_json::with_llama3_bos(&raw, "<eos>", 256))
            } else {
                ("huggingface_llama3_unicode17_whole_piece",
                    fixture::unicode_json::select(&raw, PreTokenization::Llama3Unicode17))
            };
            let recipe = format!(r#"{{"format":"{format}","path":"operator-tokenizer.json"}}"#);
            let parse = |source: &str| TokenizerInput::parse(
                strict_json::parse(source.as_bytes(), Limits::default()).unwrap(), true).unwrap();
            let input = parse(&recipe);
            let tokenizer = input.decode(&profile, json.as_bytes()).unwrap();
            let expected: &[u32] = if bos { &[256, 257] } else { &[257] };
            assert_eq!(tokenizer.encode(b"ab", Default::default()).unwrap().tokens(), expected);
            assert_eq!(tokenizer.merge_policy(), MergePolicy::WholePieceFirst);
            assert_eq!(&tokenizer.to_bytes().unwrap()[..8], b"FABBPE07");
            assert!(parse(&recipe.replace("_whole_piece", "")).decode(&profile, json.as_bytes()).is_err());
            let native = parse(&recipe.replace(format, "native_archive"));
            assert_eq!(native.decode(&profile, &tokenizer.to_bytes().unwrap()).unwrap()
                .to_bytes().unwrap(), tokenizer.to_bytes().unwrap());
        }
    }
}
