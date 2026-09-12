//! Bounded SafeTensors ingestion into the existing original-token decoder.
//! The caller supplies an explicit profile and authenticates parameter bytes.
//! This is a data parser, not executable deserialization or a serving adapter.

use super::{DecoderLayerWeights, DecoderModel, DecoderProfile, MAX_DECODER_PARAMETERS};
use super::super::super::{ByteOrder, ScalarEncoding, decode_scalar};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_WEIGHT_HEADER_BYTES: usize = 1_048_576;
pub const MAX_WEIGHT_TENSORS: usize = 9 * 128 + 3;
pub const MAX_WEIGHT_FILE_BYTES: usize = 8 + MAX_WEIGHT_HEADER_BYTES + 4 * MAX_DECODER_PARAMETERS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorIssue { Descriptor, Shape, Offsets, Encoding, NonFinite }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeightError {
    Header,
    Limit,
    Inventory,
    Tensor { name: String, issue: TensorIssue },
    Model(Error),
}
impl fmt::Display for WeightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for WeightError {}

/// Interpretation and byte counts, NOT a digest, authentication or fidelity claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorLoad {
    pub shape: Vec<usize>,
    pub encoding: ScalarEncoding,
    pub data_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeightLoadReceipt {
    pub profile: DecoderProfile,
    pub file_bytes: usize,
    pub header_bytes: usize,
    pub data_bytes: usize,
    pub normalized_bytes: usize,
    pub tensors: BTreeMap<String, TensorLoad>,
}

struct Tensor<'a> {
    shape: Vec<usize>,
    encoding: ScalarEncoding,
    bytes: &'a [u8],
}

impl DecoderModel {
    /// HF Llama tensor NAMES with the explicitly supplied narrow decoder profile.
    /// Every required tensor must exist, including the untied lm_head. No bias,
    /// transpose, quantization, missing norm, or tied-output fallback is inferred.
    /// ALL shapes, offsets and coverage are checked before decoding ANY scalar.
    /// NaN/Inf refuse; F16/BF16 expand exactly through the existing capture decoder.
    pub fn from_safetensors(
        profile: DecoderProfile, bytes: &[u8],
    ) -> Result<(Self, WeightLoadReceipt), WeightError> {
        let (tensors, header_bytes) = inspect(&profile, bytes)?;
        let read = |name: &str| read_tensor(name, &tensors);
        let embeddings = read("model.embed_tokens.weight")?;
        let final_norm = read("model.norm.weight")?;
        let output = read("lm_head.weight")?;
        let mut layers = Vec::new();
        layers.try_reserve_exact(profile.shape().layers).map_err(|_| WeightError::Limit)?;
        for index in 0..profile.shape().layers {
            let prefix = format!("model.layers.{index}");
            layers.push(DecoderLayerWeights {
                attention_norm: read(&format!("{prefix}.input_layernorm.weight"))?,
                queries: read(&format!("{prefix}.self_attn.q_proj.weight"))?,
                keys: read(&format!("{prefix}.self_attn.k_proj.weight"))?,
                values: read(&format!("{prefix}.self_attn.v_proj.weight"))?,
                attention_output: read(&format!("{prefix}.self_attn.o_proj.weight"))?,
                feed_forward_norm: read(&format!("{prefix}.post_attention_layernorm.weight"))?,
                gate: read(&format!("{prefix}.mlp.gate_proj.weight"))?,
                up: read(&format!("{prefix}.mlp.up_proj.weight"))?,
                down: read(&format!("{prefix}.mlp.down_proj.weight"))?,
            });
        }
        let receipt = WeightLoadReceipt {
            normalized_bytes: profile.parameter_count() * 4,
            profile: profile.clone(), file_bytes: bytes.len(), header_bytes,
            data_bytes: bytes.len() - 8 - header_bytes,
            tensors: tensors.iter().map(|(name, tensor)| (name.clone(), TensorLoad {
                shape: tensor.shape.clone(), encoding: tensor.encoding, data_bytes: tensor.bytes.len(),
            })).collect(),
        };
        let model = Self::new(profile, embeddings, layers, final_norm, output).map_err(WeightError::Model)?;
        Ok((model, receipt))
    }
}

fn issue(name: &str, issue: TensorIssue) -> WeightError {
    // Only a name in the generated, bounded expected inventory reaches here.
    WeightError::Tensor { name: name.to_owned(), issue }
}

fn inventory(profile: &DecoderProfile) -> BTreeMap<String, Vec<usize>> {
    let s = profile.shape();
    let mut result = BTreeMap::from([
        ("model.embed_tokens.weight".to_owned(), vec![s.vocabulary, s.hidden]),
        ("model.norm.weight".to_owned(), vec![s.hidden]),
        ("lm_head.weight".to_owned(), vec![s.vocabulary, s.hidden]),
    ]);
    for index in 0..s.layers {
        for (suffix, shape) in [
            ("input_layernorm.weight", vec![s.hidden]),
            ("post_attention_layernorm.weight", vec![s.hidden]),
            ("self_attn.q_proj.weight", vec![s.hidden, s.hidden]),
            ("self_attn.k_proj.weight", vec![profile.cache_width(), s.hidden]),
            ("self_attn.v_proj.weight", vec![profile.cache_width(), s.hidden]),
            ("self_attn.o_proj.weight", vec![s.hidden, s.hidden]),
            ("mlp.gate_proj.weight", vec![s.intermediate, s.hidden]),
            ("mlp.up_proj.weight", vec![s.intermediate, s.hidden]),
            ("mlp.down_proj.weight", vec![s.hidden, s.intermediate]),
        ] { result.insert(format!("model.layers.{index}.{suffix}"), shape); }
    }
    result
}

fn inspect<'a>(
    profile: &DecoderProfile, bytes: &'a [u8],
) -> Result<(BTreeMap<String, Tensor<'a>>, usize), WeightError> {
    if bytes.len() > MAX_WEIGHT_FILE_BYTES { return Err(WeightError::Limit); }
    let prefix: [u8; 8] = bytes.get(..8).ok_or(WeightError::Header)?
        .try_into().map_err(|_| WeightError::Header)?;
    let length = usize::try_from(u64::from_le_bytes(prefix)).map_err(|_| WeightError::Limit)?;
    if length > MAX_WEIGHT_HEADER_BYTES { return Err(WeightError::Limit); }
    let end = 8_usize.checked_add(length).ok_or(WeightError::Limit)?;
    let header = bytes.get(8..end).ok_or(WeightError::Header)?;
    if header.first() != Some(&b'{') { return Err(WeightError::Header); }
    let parsed = strict_json::parse(header, Limits {
        max_bytes: MAX_WEIGHT_HEADER_BYTES, max_depth: 4,
        max_items: MAX_WEIGHT_TENSORS * 20 + 1024, max_string_bytes: 4096,
    }).map_err(|error| match error.kind {
        ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => WeightError::Limit,
        _ => WeightError::Header,
    })?;
    let root = parsed.as_object().ok_or(WeightError::Header)?;
    let expected = inventory(profile);
    if let Some(metadata) = root.get("__metadata__") {
        let metadata = metadata.as_object().ok_or(WeightError::Header)?;
        if metadata.values().any(|value| value.as_str().is_none()) { return Err(WeightError::Header); }
    }
    if root.len() != expected.len() + usize::from(root.contains_key("__metadata__"))
        || expected.keys().any(|name| !root.contains_key(name))
    { return Err(WeightError::Inventory); }
    let data = &bytes[end..];
    // Narrower than the global cap when an explicitly small model was requested.
    if data.len() > profile.parameter_count() * 4 { return Err(WeightError::Limit); }
    let mut tensors = BTreeMap::new();
    let mut intervals = Vec::new();
    intervals.try_reserve_exact(expected.len()).map_err(|_| WeightError::Limit)?;
    for (name, shape) in expected {
        let object = root[&name].as_object().ok_or_else(|| issue(&name, TensorIssue::Descriptor))?;
        if object.len() != 3 || ["dtype", "shape", "data_offsets"].iter().any(|key| !object.contains_key(*key)) {
            return Err(issue(&name, TensorIssue::Descriptor));
        }
        let encoding = match object["dtype"].as_str() {
            Some("F32") => ScalarEncoding::Binary32,
            Some("F16") => ScalarEncoding::Binary16,
            Some("BF16") => ScalarEncoding::BFloat16,
            _ => return Err(issue(&name, TensorIssue::Encoding)),
        };
        let declared = object["shape"].as_array().ok_or_else(|| issue(&name, TensorIssue::Shape))?;
        if declared.len() != shape.len() || declared.iter().zip(&shape).any(|(value, dimension)| value.as_u64() != Some(*dimension as u64)) {
            return Err(issue(&name, TensorIssue::Shape));
        }
        let offsets = object["data_offsets"].as_array().ok_or_else(|| issue(&name, TensorIssue::Offsets))?;
        if offsets.len() != 2 { return Err(issue(&name, TensorIssue::Offsets)); }
        let offset = |value: &Json| value.as_u64().and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| issue(&name, TensorIssue::Offsets));
        let start = offset(&offsets[0])?;
        let end = offset(&offsets[1])?;
        let count = shape.iter().try_fold(1_usize, |n, dimension| n.checked_mul(*dimension))
            .ok_or(WeightError::Limit)?;
        let required = count.checked_mul(encoding.bytes()).ok_or(WeightError::Limit)?;
        if end.checked_sub(start) != Some(required) { return Err(issue(&name, TensorIssue::Offsets)); }
        let selected = data.get(start..end).ok_or_else(|| issue(&name, TensorIssue::Offsets))?;
        intervals.push((start, end));
        tensors.insert(name, Tensor { shape, encoding, bytes: selected });
    }
    // Offsets are relative to the data buffer. All nonempty registered tensors
    // must tile it exactly: no overlapping aliases, holes, or unindexed suffix.
    intervals.sort_unstable();
    let mut next = 0;
    for (start, end) in intervals {
        if start != next { return Err(WeightError::Header); }
        next = end;
    }
    if next != data.len() { return Err(WeightError::Header); }
    Ok((tensors, length))
}

fn read_tensor(name: &str, tensors: &BTreeMap<String, Tensor<'_>>) -> Result<Vec<f32>, WeightError> {
    let tensor = tensors.get(name).ok_or(WeightError::Inventory)?;
    let count = tensor.bytes.len() / tensor.encoding.bytes();
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| WeightError::Limit)?;
    for word in tensor.bytes.chunks_exact(tensor.encoding.bytes()) {
        values.push(decode_scalar(word, tensor.encoding, ByteOrder::Little)
            .map_err(|_| issue(name, TensorIssue::NonFinite))?);
    }
    Ok(values)
}
