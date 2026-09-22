//! Bounded SafeTensors ingestion into the existing original-token decoder.
//! The caller supplies an explicit profile and authenticates parameter bytes.
//! This is a data parser, not executable deserialization or a serving adapter.

pub mod shards;
pub mod pretrained;
pub mod reader;

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
pub enum TensorIssue { Descriptor, Shape, Offsets, Encoding, NonFinite, TiedValues }

/// Independently declared output-head semantics, never inferred from a missing
/// tensor. Tying expands into the SAME immutable dense decoder representation.
/// This is an ingestion contract, not a training or parameter-sharing runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputHead { Independent, TiedEmbeddings }

const EMBEDDINGS: &str = "model.embed_tokens.weight";
const OUTPUT_HEAD: &str = "lm_head.weight";

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
struct TensorDescriptor {
    shape: Vec<usize>,
    encoding: ScalarEncoding,
    start: usize,
    end: usize,
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
        Self::from_safetensors_with_output_head(profile, bytes, OutputHead::Independent)
    }

    /// Explicit sharing permits ONLY an omitted lm_head, never missing embeddings
    /// or another parameter. If both matrices are stored, their normalized f32
    /// bits must match exactly (including signed zero); neither wins silently.
    /// The receipt lists physical tensors/bytes, while normalized_bytes includes
    /// the expanded head. Raw/default loading remains strictly independent.
    pub fn from_safetensors_with_output_head(
        profile: DecoderProfile, bytes: &[u8], output_head: OutputHead,
    ) -> Result<(Self, WeightLoadReceipt), WeightError> {
        let (tensors, header_bytes) = inspect_subset_with_output_head(&inventory(&profile), bytes, output_head)?;
        let receipt = WeightLoadReceipt {
            normalized_bytes: profile.parameter_count() * 4,
            profile: profile.clone(), file_bytes: bytes.len(), header_bytes,
            data_bytes: bytes.len() - 8 - header_bytes, tensors: describe(&tensors),
        };
        let stored_head = tensors.contains_key(OUTPUT_HEAD);
        let model = construct_with_output_head(profile, output_head, stored_head,
            |name| read_tensor(name, &tensors))?;
        Ok((model, receipt))
    }
}

fn construct<F>(profile: DecoderProfile, read: F) -> Result<DecoderModel, WeightError>
where F: FnMut(&str) -> Result<Vec<f32>, WeightError> {
    construct_with_output_head(profile, OutputHead::Independent, true, read)
}

// Only a fully validated tensor inventory supplies stored_head. The source of
// shared weights is fixed; there is no metadata-selected alias or second kernel.
fn construct_with_output_head<F>(profile: DecoderProfile, output_head: OutputHead,
    stored_head: bool, mut read: F) -> Result<DecoderModel, WeightError>
where F: FnMut(&str) -> Result<Vec<f32>, WeightError> {
    let embeddings = read(EMBEDDINGS)?;
    let final_norm = read("model.norm.weight")?;
    let output = if output_head == OutputHead::Independent || stored_head {
        let output = read(OUTPUT_HEAD)?;
        if output_head == OutputHead::TiedEmbeddings
            && (output.len() != embeddings.len() || output.iter().zip(&embeddings)
                .any(|(left, right)| left.to_bits() != right.to_bits()))
        { return Err(issue(OUTPUT_HEAD, TensorIssue::TiedValues)); }
        output
    } else {
        let mut output = Vec::new();
        output.try_reserve_exact(embeddings.len()).map_err(|_| WeightError::Limit)?;
        output.extend_from_slice(&embeddings);
        output
    };
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
    DecoderModel::new(profile, embeddings, layers, final_norm, output).map_err(WeightError::Model)
}
fn describe(tensors: &BTreeMap<String, Tensor<'_>>) -> BTreeMap<String, TensorLoad> {
    tensors.iter().map(|(name, tensor)| (name.clone(), TensorLoad {
        shape: tensor.shape.clone(), encoding: tensor.encoding, data_bytes: tensor.bytes.len(),
    })).collect()
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
fn normalized_bytes(expected: &BTreeMap<String, Vec<usize>>) -> Result<usize, WeightError> {
    expected.values().try_fold(0_usize, |sum, shape| {
        let count = shape.iter().try_fold(1_usize, |n, d| n.checked_mul(*d)).ok_or(WeightError::Limit)?;
        sum.checked_add(count.checked_mul(4).ok_or(WeightError::Limit)?).ok_or(WeightError::Limit)
    })
}

fn inspect_subset<'a>(expected: &BTreeMap<String, Vec<usize>>, bytes: &'a [u8]) -> Result<(BTreeMap<String, Tensor<'a>>, usize), WeightError> {
    inspect_subset_with_output_head(expected, bytes, OutputHead::Independent)
}
fn inspect_subset_with_output_head<'a>(expected: &BTreeMap<String, Vec<usize>>, bytes: &'a [u8],
    output_head: OutputHead) -> Result<(BTreeMap<String, Tensor<'a>>, usize), WeightError>
{
    if bytes.len() > MAX_WEIGHT_FILE_BYTES { return Err(WeightError::Limit); }
    let prefix: [u8; 8] = bytes.get(..8).ok_or(WeightError::Header)?
        .try_into().map_err(|_| WeightError::Header)?;
    let length = usize::try_from(u64::from_le_bytes(prefix)).map_err(|_| WeightError::Limit)?;
    if length > MAX_WEIGHT_HEADER_BYTES { return Err(WeightError::Limit); }
    let end = 8_usize.checked_add(length).ok_or(WeightError::Limit)?;
    let header = bytes.get(8..end).ok_or(WeightError::Header)?;
    let descriptors = inspect_directory_with_output_head(expected, header, output_head)?;
    let data = &bytes[end..];
    if data.len() > normalized_bytes(expected)? { return Err(WeightError::Limit); }
    let mut tensors = BTreeMap::new();
    let mut through = 0;
    for (name, tensor) in descriptors {
        let selected = data.get(tensor.start..tensor.end).ok_or_else(|| issue(&name, TensorIssue::Offsets))?;
        through = through.max(tensor.end);
        tensors.insert(name, Tensor { shape: tensor.shape, encoding: tensor.encoding, bytes: selected });
    }
    if through != data.len() { return Err(WeightError::Header); }
    Ok((tensors, length))
}

/// Same complete descriptor checks for memory, sharded and bounded-reader input.
/// No pointer into the raw data is needed to reject invalid inventory/coverage.
fn inspect_directory_with_output_head(expected: &BTreeMap<String, Vec<usize>>, header: &[u8],
    output_head: OutputHead) -> Result<BTreeMap<String, TensorDescriptor>, WeightError>
{
    if header.first() != Some(&b'{') { return Err(WeightError::Header); }
    let parsed = strict_json::parse(header, Limits {
        max_bytes: MAX_WEIGHT_HEADER_BYTES, max_depth: 4,
        max_items: MAX_WEIGHT_TENSORS * 20 + 1024, max_string_bytes: 4096,
    }).map_err(|error| match error.kind {
        ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => WeightError::Limit,
        _ => WeightError::Header,
    })?;
    let root = parsed.as_object().ok_or(WeightError::Header)?;
    if let Some(metadata) = root.get("__metadata__") {
        let metadata = metadata.as_object().ok_or(WeightError::Header)?;
        if metadata.values().any(|value| value.as_str().is_none()) { return Err(WeightError::Header); }
    }
    let omitted_head = output_head == OutputHead::TiedEmbeddings
        && expected.contains_key(OUTPUT_HEAD) && !root.contains_key(OUTPUT_HEAD);
    if root.len() != expected.len() - usize::from(omitted_head) + usize::from(root.contains_key("__metadata__"))
        || expected.keys().any(|name| !(root.contains_key(name) || (omitted_head && name == OUTPUT_HEAD)))
    { return Err(WeightError::Inventory); }
    let mut tensors = BTreeMap::new();
    let mut intervals = Vec::new();
    intervals.try_reserve_exact(expected.len()).map_err(|_| WeightError::Limit)?;
    for (name, shape) in expected {
        if omitted_head && name == OUTPUT_HEAD { continue; }
        let object = root[name].as_object().ok_or_else(|| issue(name, TensorIssue::Descriptor))?;
        if object.len() != 3 || ["dtype", "shape", "data_offsets"].iter().any(|key| !object.contains_key(*key)) {
            return Err(issue(name, TensorIssue::Descriptor));
        }
        let encoding = match object["dtype"].as_str() {
            Some("F32") => ScalarEncoding::Binary32,
            Some("F16") => ScalarEncoding::Binary16,
            Some("BF16") => ScalarEncoding::BFloat16,
            _ => return Err(issue(name, TensorIssue::Encoding)),
        };
        let declared = object["shape"].as_array().ok_or_else(|| issue(name, TensorIssue::Shape))?;
        if declared.len() != shape.len() || declared.iter().zip(shape).any(|(value, dimension)| value.as_u64() != Some(*dimension as u64)) {
            return Err(issue(name, TensorIssue::Shape));
        }
        let offsets = object["data_offsets"].as_array().ok_or_else(|| issue(name, TensorIssue::Offsets))?;
        if offsets.len() != 2 { return Err(issue(name, TensorIssue::Offsets)); }
        let offset = |value: &Json| value.as_u64().and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| issue(name, TensorIssue::Offsets));
        let start = offset(&offsets[0])?; let end = offset(&offsets[1])?;
        let count = shape.iter().try_fold(1_usize, |n, dimension| n.checked_mul(*dimension)).ok_or(WeightError::Limit)?;
        let required = count.checked_mul(encoding.bytes()).ok_or(WeightError::Limit)?;
        if end.checked_sub(start) != Some(required) { return Err(issue(name, TensorIssue::Offsets)); }
        intervals.push((start, end));
        tensors.insert(name.clone(), TensorDescriptor { shape: shape.clone(), encoding, start, end });
    }
    intervals.sort_unstable();
    let mut next = 0;
    for (start, end) in intervals {
        if start != next { return Err(WeightError::Header); }
        next = end;
    }
    Ok(tensors)
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

#[cfg(test)]
mod tied_tests;
