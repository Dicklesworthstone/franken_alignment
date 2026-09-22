//! Strict import of the raw ByteLevel BPE subset of tokenizer.json (version 1.0).
//!
//! The admitted pipeline has no normalization, regex splitting, prefix insertion,
//! added tokens, padding, truncation or postprocessing. Everything else refuses;
//! in particular, this is NOT an importer for arbitrary GPT-2/Llama tokenizers.
//! The vocabulary's reversible ByteLevel alphabet is converted to original bytes,
//! then the ORIGINAL ByteBpe constructor validates the entire token/merge graph.
//! Declaring a model profile does not authenticate this file or training semantics.

use super::{ByteBpe, Merge, TokenBytes, MAX_MERGES, MAX_TOKEN_BYTES, MAX_VOCABULARY_BYTES};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderProfile;
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::collections::BTreeMap;

pub const MAX_JSON_BYTES: usize = 32 * 1_048_576;
const MAX_JSON_ITEMS: usize = 1_000_000;
const ROOT_FIELDS: &[&str] = &[
    "version", "truncation", "padding", "added_tokens", "normalizer",
    "pre_tokenizer", "post_processor", "decoder", "model",
];
const MODEL_FIELDS: &[&str] = &[
    "type", "dropout", "unk_token", "continuing_subword_prefix", "end_of_word_suffix",
    "fuse_unk", "byte_fallback", "ignore_merges", "vocab", "merges",
];
const BYTE_LEVEL_FIELDS: &[&str] = &["type", "add_prefix_space", "trim_offsets", "use_regex"];

type Object = BTreeMap<String, Json>;

impl ByteBpe {
    pub const MAX_HUGGINGFACE_JSON_BYTES: usize = MAX_JSON_BYTES;

    /// Import an explicitly declared, transformation-free ByteLevel BPE pipeline.
    ///
    /// The complete version-1.0 schema and every behavior flag must be present.
    /// Unknown fields and unsupported behavior refuse rather than selecting native
    /// defaults. Both homogeneous legacy "left right" and pair-array merge lists
    /// are supported. Rank is list order; IDs are the supplied numeric vocabulary
    /// IDs, NOT byte values, map iteration order or merge ranks.
    ///
    /// expected is independently trusted model DATA. The JSON contains no model
    /// identity and cannot establish that this vocabulary was trained with those
    /// weights. Exact vocabulary cardinality is checked; the resulting immutable
    /// tokenizer binds the whole supplied profile. Persisting it with to_bytes
    /// additionally binds that profile in the existing native archive header.
    ///
    /// Original-byte spans remain the native TokenizedInput partition, not Hugging
    /// Face's Unicode-character offset convention. Decoding remains strict bytes,
    /// not a lossy UTF-8 string. No inference, network/file access or effects occur.
    pub fn from_huggingface_json(expected: &DecoderProfile, bytes: &[u8], max_bytes: usize)
        -> Result<Self, Error>
    {
        if max_bytes == 0 || max_bytes > MAX_JSON_BYTES || bytes.len() > max_bytes {
            return Err(Error::Limit);
        }
        let value = strict_json::parse(bytes, Limits {
            max_bytes, max_depth: 16, max_items: MAX_JSON_ITEMS,
            // A legacy merge contains two independently bounded UTF-8 spellings.
            max_string_bytes: 4 * MAX_TOKEN_BYTES + 1,
        }).map_err(|error| match error.kind {
            ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit
                | ErrorKind::StringLimit => Error::Limit,
            ErrorKind::DuplicateKey(_) => Error::Duplicate,
            ErrorKind::UnexpectedEof => Error::Incomplete,
            _ => Error::InvalidInput,
        })?;
        let root = exact_object(&value, ROOT_FIELDS)?;
        if required(root, "version")?.as_str() != Some("1.0") { return Err(Error::Binding); }
        for field in ["truncation", "padding", "normalizer", "post_processor"] {
            require_null(required(root, field)?)?;
        }
        let added = required(root, "added_tokens")?.as_array().ok_or(Error::InvalidInput)?;
        if !added.is_empty() { return Err(Error::Binding); }
        byte_level(required(root, "pre_tokenizer")?)?;
        byte_level(required(root, "decoder")?)?;
        let model = exact_object(required(root, "model")?, MODEL_FIELDS)?;
        if required(model, "type")?.as_str() != Some("BPE") { return Err(Error::Binding); }
        for field in ["dropout", "unk_token"] { require_null(required(model, field)?)?; }
        for field in ["continuing_subword_prefix", "end_of_word_suffix"] {
            let value = required(model, field)?;
            if !value.is_null() && value.as_str() != Some("") { return Err(Error::Binding); }
        }
        for field in ["fuse_unk", "byte_fallback", "ignore_merges"] {
            require_false(required(model, field)?)?;
        }
        let declared = required(model, "vocab")?.as_object().ok_or(Error::InvalidInput)?;
        let count = expected.shape().vocabulary;
        if declared.len() != count { return Err(Error::Binding); }
        if count < 256 { return Err(Error::Incomplete); }
        let rules = required(model, "merges")?.as_array().ok_or(Error::InvalidInput)?;
        if rules.len() > MAX_MERGES { return Err(Error::Limit); }
        let alphabet = alphabet_inverse();
        let mut slots = Vec::new();
        slots.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        slots.resize_with(count, || None);
        let mut retained = 0_usize;
        for (spelling, id) in declared {
            let id = token_id(id)?;
            let slot = slots.get_mut(id as usize).ok_or(Error::Binding)?;
            if slot.is_some() { return Err(Error::Duplicate); }
            let decoded = spelling_bytes(spelling, &alphabet)?;
            retained = retained.checked_add(decoded.len()).ok_or(Error::Limit)?;
            if retained > MAX_VOCABULARY_BYTES { return Err(Error::Limit); }
            *slot = Some(TokenBytes::Content(decoded));
        }
        let mut vocabulary = Vec::new();
        vocabulary.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        for slot in slots { vocabulary.push(slot.ok_or(Error::Incomplete)?); }
        let mut merges = Vec::new();
        merges.try_reserve_exact(rules.len()).map_err(|_| Error::Limit)?;
        let legacy = rules.first().is_some_and(|rule| matches!(rule, Json::String(_)));
        for rule in rules {
            let (left, right) = merge_pair(rule, legacy)?;
            let left_id = lookup(declared, left)?;
            let right_id = lookup(declared, right)?;
            // Bound the CONCATENATION before allocating it. Individual valid
            // operands must not cause an oversized temporary or unchecked slice.
            let content_len = |id: u32| match vocabulary.get(id as usize) {
                Some(TokenBytes::Content(bytes)) => Ok(bytes.len()),
                _ => Err(Error::Binding),
            };
            let result_len = content_len(left_id)?.checked_add(content_len(right_id)?).ok_or(Error::Limit)?;
            if result_len > MAX_TOKEN_BYTES { return Err(Error::Limit); }
            let joined_len = left.len().checked_add(right.len()).ok_or(Error::Limit)?;
            let mut joined = String::new();
            joined.try_reserve_exact(joined_len).map_err(|_| Error::Limit)?;
            joined.push_str(left);
            joined.push_str(right);
            merges.push(Merge { left: left_id, right: right_id, result: lookup(declared, &joined)? });
        }
        // Reuse the original total-byte, singleton, ordering, duplicate-pair and
        // reachability checks. A parsed JSON graph alone never admits a tokenizer.
        Self::new(expected.clone(), vocabulary, merges)
    }
}

fn exact_object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a Object, Error> {
    let object = value.as_object().ok_or(Error::InvalidInput)?;
    if object.keys().any(|key| !fields.contains(&key.as_str())) { return Err(Error::Binding); }
    if object.len() != fields.len() { return Err(Error::Incomplete); }
    Ok(object)
}
fn required<'a>(object: &'a Object, field: &str) -> Result<&'a Json, Error> {
    object.get(field).ok_or(Error::Incomplete)
}
fn require_null(value: &Json) -> Result<(), Error> {
    if value.is_null() { Ok(()) } else { Err(Error::Binding) }
}
fn require_false(value: &Json) -> Result<(), Error> {
    match value.as_bool() {
        Some(false) => Ok(()),
        Some(true) => Err(Error::Binding),
        None => Err(Error::InvalidInput),
    }
}
fn byte_level(value: &Json) -> Result<(), Error> {
    let object = exact_object(value, BYTE_LEVEL_FIELDS)?;
    if required(object, "type")?.as_str() != Some("ByteLevel") { return Err(Error::Binding); }
    for field in ["add_prefix_space", "trim_offsets", "use_regex"] {
        require_false(required(object, field)?)?;
    }
    Ok(())
}
fn token_id(value: &Json) -> Result<u32, Error> {
    u32::try_from(value.as_u64().ok_or(Error::InvalidInput)?).map_err(|_| Error::Limit)
}
fn lookup(vocabulary: &Object, spelling: &str) -> Result<u32, Error> {
    token_id(vocabulary.get(spelling).ok_or(Error::Missing)?)
}

// The finite ByteLevel alphabet fixes visible Latin-1 bytes and assigns remaining
// bytes successive code points starting at U+0100. No Unicode normalization or
// permissive fallback: each admitted glyph has exactly one original byte.
fn alphabet_inverse() -> [Option<u8>; 512] {
    let mut inverse = [None; 512];
    let mut extension = 256;
    for byte in 0_u16..=255 {
        let code = if matches!(byte, 33..=126 | 161..=172 | 174..=255) {
            usize::from(byte)
        } else {
            let code = extension;
            extension += 1;
            code
        };
        inverse[code] = Some(byte as u8);
    }
    inverse
}
fn spelling_bytes(spelling: &str, alphabet: &[Option<u8>; 512]) -> Result<Vec<u8>, Error> {
    let count = spelling.chars().count();
    if count == 0 { return Err(Error::InvalidInput); }
    if count > MAX_TOKEN_BYTES { return Err(Error::Limit); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for glyph in spelling.chars() {
        bytes.push(alphabet.get(glyph as usize).copied().flatten().ok_or(Error::Binding)?);
    }
    Ok(bytes)
}
fn merge_pair(value: &Json, legacy: bool) -> Result<(&str, &str), Error> {
    let pair = if legacy {
        let text = value.as_str().ok_or(Error::InvalidInput)?;
        let (left, right) = text.split_once(' ').ok_or(Error::InvalidInput)?;
        if right.contains(' ') { return Err(Error::InvalidInput); }
        (left, right)
    } else {
        let pair = value.as_array().ok_or(Error::InvalidInput)?;
        if pair.len() != 2 { return Err(Error::InvalidInput); }
        (pair[0].as_str().ok_or(Error::InvalidInput)?, pair[1].as_str().ok_or(Error::InvalidInput)?)
    };
    if pair.0.is_empty() || pair.1.is_empty() { return Err(Error::InvalidInput); }
    Ok(pair)
}

#[cfg(test)]
mod tests;
