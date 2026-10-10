//! Explicit operator input selection; the original native parsers own semantics.
//! No content sniffing, format fallback, inferred model identity or file access.
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
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
            if !fields.is_empty() { return Err("unknown tokenizer input field".into()); }
            let format = match format.as_str() {
                Some("native_archive") => NativeTokenizerFormat::NativeArchive,
                Some("huggingface_raw_bytelevel") => NativeTokenizerFormat::HuggingFaceRawByteLevel,
                _ => return Err("unknown tokenizer format".into()),
            };
            (format, path)
        } else { (NativeTokenizerFormat::NativeArchive, value) };
        let path = path.as_str().ok_or("tokenizer path must be text")?;
        if path.is_empty() || path.as_bytes().contains(&0) {
            return Err("invalid tokenizer input path".into());
        }
        Ok(Self { format, path: path.to_owned() })
    }

    pub(super) fn path(&self) -> &str { &self.path }

    pub(super) fn byte_limit(&self) -> usize {
        match self.format {
            NativeTokenizerFormat::NativeArchive => MAX_FILE_TOKENIZER_BYTES,
            NativeTokenizerFormat::HuggingFaceRawByteLevel => ByteBpe::MAX_HUGGINGFACE_JSON_BYTES,
        }
    }

    pub(super) fn decode(&self, profile: &DecoderProfile, bytes: &[u8]) -> Result<ByteBpe, String> {
        match self.format {
            NativeTokenizerFormat::NativeArchive => ByteBpe::from_bytes(profile, bytes),
            NativeTokenizerFormat::HuggingFaceRawByteLevel =>
                ByteBpe::from_huggingface_json(profile, bytes, self.byte_limit()),
        }.map_err(|error| format!("tokenizer input refused: {error:?}"))
    }
}

#[cfg(test)]
#[path = "tokenizer/fixture.rs"]
pub(super) mod fixture;

#[cfg(test)]
#[path = "tokenizer/direct.rs"]
pub(super) mod direct;
