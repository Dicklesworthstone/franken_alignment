//! An explicit dense causal decoder profile, not an imported serving backend.
//! Matrices are row-major [output, input]. Parameters are data, never executable.

use super::super::attention::{AttentionContract, AttentionMask, MAX_ATTENTION_HEADS};
use super::super::model::{ModelKvProfile, MAX_MODEL_KV_LAYERS, MAX_MODEL_KV_VALUES};
use super::super::{KvContract, MAX_KV_POSITIONS, MAX_KV_VALUES};
use super::super::super::{ByteOrder, ScalarEncoding, TensorContract, TensorLayout};
use crate::action::consequence::activation::CaptureProfile;
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub const MAX_DECODER_PARAMETERS: usize = 16_777_216;
pub const MAX_DECODER_HIDDEN: usize = 2_048;
pub const MAX_DECODER_INTERMEDIATE: usize = 8_192;
pub const MAX_DECODER_VOCABULARY: usize = 65_536;

/// Declared identifiers do not authenticate learned weights or tokenizer files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderIdentity {
    pub tenant: u64,
    pub model: u64,
    pub model_generation: u64,
    pub tokenizer_generation: u64,
    pub profile_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderShape {
    pub vocabulary: usize,
    pub hidden: usize,
    pub intermediate: usize,
    pub layers: usize,
    pub query_heads: usize,
    pub cache_heads: usize,
    pub context: usize,
}

/// V1: batch one, full causal prefix, pre-RMSNorm, half-split unscaled RoPE,
/// bias-free Q/K/V/O, SwiGLU, residuals, final RMSNorm and untied vocabulary head.
/// f32 storage; sequential f64 reductions/transcendentals; f32 layer boundaries.
/// This explicitly does NOT promise another backend's rounding or model format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecoderProfile {
    identity: DecoderIdentity,
    shape: DecoderShape,
    epsilon_bits: u64,
    theta_bits: u64,
    parameters: usize,
}

impl DecoderProfile {
    pub fn new(identity: DecoderIdentity, shape: DecoderShape, epsilon: f64, theta: f64) -> Result<Self, Error> {
        if [identity.tenant, identity.model, identity.model_generation,
            identity.tokenizer_generation, identity.profile_generation].contains(&0)
            || [shape.vocabulary, shape.hidden, shape.intermediate, shape.layers,
                shape.query_heads, shape.cache_heads, shape.context].contains(&0)
            || !epsilon.is_finite() || epsilon <= 0.0 || epsilon > 1.0
            || !theta.is_finite() || !(1.0..=1_000_000_000.0).contains(&theta)
        { return Err(Error::InvalidInput); }
        if shape.vocabulary > MAX_DECODER_VOCABULARY || shape.hidden > MAX_DECODER_HIDDEN
            || shape.intermediate > MAX_DECODER_INTERMEDIATE || shape.layers > MAX_MODEL_KV_LAYERS
            || shape.query_heads > MAX_ATTENTION_HEADS || shape.cache_heads > shape.query_heads
            || shape.context > MAX_KV_POSITIONS
        { return Err(Error::Limit); }
        if !shape.hidden.is_multiple_of(shape.query_heads)
            || !shape.query_heads.is_multiple_of(shape.cache_heads)
            || !(shape.hidden / shape.query_heads).is_multiple_of(2)
        { return Err(Error::InvalidInput); }
        let h = shape.hidden as u64;
        let k = (shape.cache_heads * (shape.hidden / shape.query_heads)) as u64;
        let i = shape.intermediate as u64;
        let per_layer = 2 * h * h + 2 * k * h + 3 * i * h + 2 * h;
        let parameters = per_layer * shape.layers as u64 + 2 * shape.vocabulary as u64 * h + h;
        let layer_cache = 2 * k * shape.context as u64;
        if parameters > MAX_DECODER_PARAMETERS as u64 || layer_cache > MAX_KV_VALUES as u64
            || layer_cache * shape.layers as u64 > MAX_MODEL_KV_VALUES as u64
        { return Err(Error::Limit); }
        Ok(Self { identity, shape, epsilon_bits: epsilon.to_bits(), theta_bits: theta.to_bits(),
            parameters: parameters as usize })
    }
    pub fn identity(&self) -> DecoderIdentity { self.identity }
    pub fn shape(&self) -> DecoderShape { self.shape }
    pub fn epsilon(&self) -> f64 { f64::from_bits(self.epsilon_bits) }
    pub fn theta(&self) -> f64 { f64::from_bits(self.theta_bits) }
    pub fn parameter_count(&self) -> usize { self.parameters }
    pub fn head_width(&self) -> usize { self.shape.hidden / self.shape.query_heads }
    pub fn cache_width(&self) -> usize { self.shape.cache_heads * self.head_width() }

    pub(super) fn tensor(&self, tap: u64, heads: usize, channels: usize) -> Result<TensorContract, Error> {
        let id = self.identity;
        TensorContract::new(CaptureProfile { tenant: id.tenant, model: id.model,
            model_generation: id.model_generation, tap, layout_generation: id.profile_generation },
            ScalarEncoding::Binary32, ByteOrder::Little, heads, channels)
    }
}

/// Complete layer inventory in execution order. No inferred biases, transposes,
/// missing norm defaults, or weight tying. Every vector is validated at admission.
#[derive(Clone)]
pub struct DecoderLayerWeights {
    pub attention_norm: Vec<f32>,
    pub queries: Vec<f32>,
    pub keys: Vec<f32>,
    pub values: Vec<f32>,
    pub attention_output: Vec<f32>,
    pub feed_forward_norm: Vec<f32>,
    pub gate: Vec<f32>,
    pub up: Vec<f32>,
    pub down: Vec<f32>,
}

impl DecoderLayerWeights {
    pub(super) fn tensors(&self) -> [&[f32]; 9] {
        [&self.attention_norm, &self.queries, &self.keys, &self.values, &self.attention_output,
            &self.feed_forward_norm, &self.gate, &self.up, &self.down]
    }
}

pub(super) struct Layer {
    pub weights: DecoderLayerWeights,
    pub attention: AttentionContract,
    pub residual: TensorContract,
}
pub(super) struct ModelData {
    pub profile: DecoderProfile,
    pub embeddings: Vec<f32>,
    pub layers: Vec<Layer>,
    pub final_norm: Vec<f32>,
    pub output: Vec<f32>,
    pub cache: ModelKvProfile,
    pub key_layout: TensorLayout,
    pub value_layout: TensorLayout,
}

/// Clones share immutable parameters. No RNG, mutable cache or authority is shared.
#[derive(Clone)]
pub struct DecoderModel { pub(super) data: Rc<ModelData> }
impl fmt::Debug for DecoderModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderModel").field("profile", &self.data.profile).finish_non_exhaustive()
    }
}

impl DecoderModel {
    pub fn new(profile: DecoderProfile, embeddings: Vec<f32>, layers: Vec<DecoderLayerWeights>,
        final_norm: Vec<f32>, output: Vec<f32>) -> Result<Self, Error>
    {
        let shape = profile.shape;
        if layers.len() != shape.layers { return Err(Error::Binding); }
        let h = shape.hidden;
        let k = profile.cache_width();
        let i = shape.intermediate;
        // Length checks precede scanning values; malformed huge buffers do not
        // create an unbounded validation pass under a small declared profile.
        if embeddings.len() != shape.vocabulary * h || output.len() != shape.vocabulary * h
            || final_norm.len() != h { return Err(Error::Binding); }
        let sizes = [h, h * h, k * h, k * h, h * h, h, i * h, i * h, h * i];
        for layer in &layers {
            for (tensor, size) in layer.tensors().into_iter().zip(sizes) {
                if tensor.len() != size { return Err(Error::Binding); }
            }
        }
        for tensor in [&embeddings[..], &final_norm[..], &output[..]].into_iter()
            .chain(layers.iter().flat_map(DecoderLayerWeights::tensors))
        {
            if tensor.iter().any(|value| !value.is_finite()) { return Err(Error::InvalidInput); }
        }
        let mut admitted = Vec::new();
        admitted.try_reserve_exact(shape.layers).map_err(|_| Error::Limit)?;
        let mut cache_layers = BTreeMap::new();
        for (index, weights) in layers.into_iter().enumerate() {
            let tap = 4 * index as u64;
            let q = profile.tensor(tap + 1, shape.query_heads, profile.head_width())?;
            let k = profile.tensor(tap + 2, shape.cache_heads, profile.head_width())?;
            let v = profile.tensor(tap + 3, shape.cache_heads, profile.head_width())?;
            let cache = KvContract::new(k, v, shape.query_heads)?;
            let attention = AttentionContract::new(index as u64 + 1, profile.identity.profile_generation,
                q, cache.clone(), AttentionMask::FullPrefix, 1.0 / (profile.head_width() as f64).sqrt())?;
            let residual = profile.tensor(tap + 4, 1, h)?;
            cache_layers.insert(index as u64 + 1, cache);
            admitted.push(Layer { weights, attention, residual });
        }
        let cache = ModelKvProfile::new(profile.identity.model, profile.identity.profile_generation, cache_layers)?;
        let key_layout = dense_layout(shape.cache_heads, profile.head_width())?;
        let value_layout = key_layout.clone();
        Ok(Self { data: Rc::new(ModelData { profile, embeddings, layers: admitted, final_norm,
            output, cache, key_layout, value_layout }) })
    }
    pub fn profile(&self) -> &DecoderProfile { &self.data.profile }
    pub fn cache_profile(&self) -> &ModelKvProfile { &self.data.cache }

    /// Actual residual tap contract installed by this model, not a guessed ID.
    pub fn residual_contract(&self, layer: u64) -> Result<&TensorContract, Error> {
        let index = layer.checked_sub(1).ok_or(Error::InvalidInput)?;
        let index = usize::try_from(index).map_err(|_| Error::Missing)?;
        self.data.layers.get(index).map(|layer| &layer.residual).ok_or(Error::Missing)
    }
}

pub(super) fn dense_layout(heads: usize, channels: usize) -> Result<TensorLayout, Error> {
    TensorLayout::new([1, 1, heads, channels], [0, 0, channels * 4, 4], 0,
        ScalarEncoding::Binary32, ByteOrder::Little)
}

pub(super) fn rounded(value: f64) -> Result<f32, Error> {
    let result = value as f32;
    if !result.is_finite() { return Err(Error::Overflow); }
    Ok(result)
}

pub(super) fn matrix(weights: &[f32], rows: usize, input: &[f32]) -> Result<Vec<f32>, Error> {
    if weights.len() != rows * input.len() || input.is_empty() { return Err(Error::Binding); }
    let mut result = Vec::new();
    result.try_reserve_exact(rows).map_err(|_| Error::Limit)?;
    for row in weights.chunks_exact(input.len()) {
        let mut sum = 0.0_f64;
        for (&weight, &value) in row.iter().zip(input) { sum += f64::from(weight) * f64::from(value); }
        result.push(rounded(sum)?);
    }
    Ok(result)
}

pub(super) fn rms(input: &[f32], scales: &[f32], epsilon: f64) -> Result<Vec<f32>, Error> {
    if input.is_empty() || input.len() != scales.len() { return Err(Error::Binding); }
    let mut squares = 0.0_f64;
    for value in input { squares += f64::from(*value) * f64::from(*value); }
    let denominator = (squares / input.len() as f64 + epsilon).sqrt();
    let mut output = Vec::new();
    output.try_reserve_exact(input.len()).map_err(|_| Error::Limit)?;
    for (&value, &scale) in input.iter().zip(scales) {
        output.push(rounded((f64::from(value) / denominator) * f64::from(scale))?);
    }
    Ok(output)
}

pub(super) fn rotary(values: &mut [f32], width: usize, position: u64, theta: f64) -> Result<(), Error> {
    if width == 0 || !width.is_multiple_of(2) || !values.len().is_multiple_of(width) { return Err(Error::Binding); }
    for row in values.chunks_exact_mut(width) {
        for index in 0..width / 2 {
            let angle = position as f64 * theta.powf(-((2 * index) as f64) / width as f64);
            let (sine, cosine) = angle.sin_cos();
            let left = f64::from(row[index]);
            let right = f64::from(row[index + width / 2]);
            row[index] = rounded(left * cosine - right * sine)?;
            row[index + width / 2] = rounded(left * sine + right * cosine)?;
        }
    }
    Ok(())
}

pub(super) fn residual(left: &[f32], right: &[f32]) -> Result<Vec<f32>, Error> {
    if left.len() != right.len() { return Err(Error::Binding); }
    left.iter().zip(right).map(|(&a, &b)| rounded(f64::from(a) + f64::from(b))).collect()
}

pub(super) fn swiglu(mut gate: Vec<f32>, up: &[f32]) -> Result<Vec<f32>, Error> {
    if gate.len() != up.len() { return Err(Error::Binding); }
    for (value, up) in gate.iter_mut().zip(up) {
        let x = f64::from(*value);
        let silu = if x >= 0.0 { x / (1.0 + (-x).exp()) }
            else { let exponential = x.exp(); x * exponential / (1.0 + exponential) };
        // V1 materializes SiLU in f32 before the elementwise product.
        *value = rounded(f64::from(rounded(silu)?) * f64::from(*up))?;
    }
    Ok(gate)
}
