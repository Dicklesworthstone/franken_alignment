//! Bounded causal attention replay over captured Q/K/V, without a serving host.
//!
//! Q and K must already include the host's positional transforms. This profile
//! implements scaled dot products, causal/full or sliding visibility, stable
//! softmax, and value mixing. It does not implement RoPE, bias, dropout, output
//! projection, residuals, or a complete transformer continuation. Binary64
//! arithmetic here is a reference calculation, not the exact-probe certificate.

use super::{KvContract, MAX_KV_POSITIONS};
use super::image::{KvImage, KvImageDescriptor};
use super::super::{TensorCapture, TensorContract};
use crate::action::consequence::activation::MAX_VALUES;
use crate::Error;

pub const MAX_ATTENTION_HEADS: usize = 128;
pub const MAX_ATTENTION_PRODUCTS: u64 = 134_217_728;
pub const MAX_ATTENTION_RESOLUTION_STEPS: u64 = 4_294_967_296;
pub const MAX_ATTENTION_WORKSPACE_BYTES: usize = 16_777_216;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttentionMask {
    /// Every position from zero through the query position must be available.
    FullPrefix,
    /// A declared sliding window, not inferred permission to omit old rows.
    Sliding { tokens: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttentionNumerics {
    /// Ascending-channel dot sums, ascending-position softmax and value sums.
    /// exp() and rounding are not promised bit-identical across platforms.
    Binary64SequentialStableSoftmaxV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttentionContract {
    id: u64,
    generation: u64,
    queries: TensorContract,
    cache: KvContract,
    mask: AttentionMask,
    scale_bits: u64,
}

impl AttentionContract {
    pub fn new(
        id: u64, generation: u64, queries: TensorContract, cache: KvContract,
        mask: AttentionMask, scale: f64,
    ) -> Result<Self, Error> {
        if id == 0 || generation == 0 || !scale.is_finite() || scale <= 0.0 || scale > 1_000_000.0 {
            return Err(Error::InvalidInput);
        }
        if let AttentionMask::Sliding { tokens } = mask {
            if tokens == 0 { return Err(Error::InvalidInput); }
            if tokens > MAX_KV_POSITIONS { return Err(Error::Limit); }
        }
        let q = queries.profile();
        let k = cache.keys().profile();
        let v = cache.values().profile();
        if q.tenant != k.tenant || q.model != k.model || q.model_generation != k.model_generation
            || q.tap == k.tap || q.tap == v.tap
            || queries.heads() != cache.query_heads() || queries.channels() != cache.keys().channels()
        { return Err(Error::Binding); }
        let outputs = queries.heads().checked_mul(cache.values().channels()).ok_or(Error::Overflow)?;
        if queries.heads() > MAX_ATTENTION_HEADS || outputs > MAX_VALUES { return Err(Error::Limit); }
        Ok(Self { id, generation, queries, cache, mask, scale_bits: scale.to_bits() })
    }

    pub fn id(&self) -> u64 { self.id }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn queries(&self) -> &TensorContract { &self.queries }
    pub fn cache(&self) -> &KvContract { &self.cache }
    pub fn mask(&self) -> AttentionMask { self.mask }
    pub fn scale(&self) -> f64 { f64::from_bits(self.scale_bits) }
    pub fn numerics(&self) -> AttentionNumerics { AttentionNumerics::Binary64SequentialStableSoftmaxV1 }
}

/// Per-call ceilings. They do not replenish or represent production rights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttentionBudget {
    pub scalar_products: u64,
    pub resolution_steps: u64,
    pub workspace_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttentionWork {
    /// Dot-product plus weighted-value terms, including repeated GQA reads.
    pub scalar_products: u64,
    pub exponentials: u64,
    /// Conservative base/branch-node visits; not CPU instructions or map comparisons.
    pub resolution_step_bound: u64,
    /// Retained f64 weights and outputs; excludes immutable sources and metadata.
    pub workspace_bytes: usize,
}

impl AttentionWork {
    pub(super) fn check(self, budget: AttentionBudget) -> Result<(), Error> {
        if budget.scalar_products > MAX_ATTENTION_PRODUCTS
            || budget.resolution_steps > MAX_ATTENTION_RESOLUTION_STEPS
            || budget.workspace_bytes > MAX_ATTENTION_WORKSPACE_BYTES
            || self.scalar_products > budget.scalar_products
            || self.resolution_step_bound > budget.resolution_steps
            || self.workspace_bytes > budget.workspace_bytes
        { return Err(Error::Limit); }
        Ok(())
    }
}

/// Computed values, not a capture, live judgment or native restart certificate.
#[derive(Clone, PartialEq)]
pub struct AttentionValues {
    heads: usize,
    channels: usize,
    first_position: u64,
    positions: usize,
    weights: Vec<f64>,
    output: Vec<f64>,
    zero_weights: usize,
}

impl std::fmt::Debug for AttentionValues {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("AttentionValues").field("heads", &self.heads)
            .field("channels", &self.channels).field("first_position", &self.first_position)
            .field("positions", &self.positions).field("zero_weights", &self.zero_weights)
            .finish_non_exhaustive()
    }
}

impl AttentionValues {
    pub fn heads(&self) -> usize { self.heads }
    pub fn channels(&self) -> usize { self.channels }
    pub fn first_position(&self) -> u64 { self.first_position }
    pub fn positions(&self) -> usize { self.positions }
    pub fn output(&self) -> &[f64] { &self.output }
    /// Includes exp or normalization underflow, never missing/unknown rows.
    pub fn zero_weights(&self) -> usize { self.zero_weights }
    pub fn weights(&self, head: usize) -> Result<&[f64], Error> {
        if head >= self.heads { return Err(Error::Missing); }
        Ok(&self.weights[head * self.positions..(head + 1) * self.positions])
    }
    pub fn head_output(&self, head: usize) -> Result<&[f64], Error> {
        if head >= self.heads { return Err(Error::Missing); }
        Ok(&self.output[head * self.channels..(head + 1) * self.channels])
    }
}

#[derive(Clone, Debug)]
pub struct AttentionReplay {
    pub contract: AttentionContract,
    pub source: KvImageDescriptor,
    /// The actual supplied query capture, not only a reusable numerical ID.
    pub query: TensorCapture,
    pub values: AttentionValues,
    pub work: AttentionWork,
}

impl KvImage {
    pub fn replay_attention(
        &self, contract: &AttentionContract, query: &TensorCapture, budget: AttentionBudget,
    ) -> Result<AttentionReplay, Error> {
        let plan = prepare(contract, query, self.descriptor())?;
        let work = plan.work(1)?;
        work.check(budget)?;
        let values = plan.evaluate(self)?;
        Ok(AttentionReplay { contract: contract.clone(), source: self.descriptor().clone(),
            query: query.clone(), values, work })
    }
}

// Only the image and its experimental branch implementations can supply rows.
// No public callback can fabricate successful capture or bypass preflight.
pub(super) trait AttentionRows {
    fn scalar(&self, values: bool, position: u64, head: usize, channel: usize) -> Result<f64, Error>;
}

impl AttentionRows for KvImage {
    fn scalar(&self, values: bool, position: u64, head: usize, channel: usize) -> Result<f64, Error> {
        let token = self.token(position)?;
        let (source, contract) = if values { (token.value(), self.descriptor().contract.values()) }
            else { (token.key(), self.descriptor().contract.keys()) };
        if head >= contract.heads() || channel >= contract.channels() { return Err(Error::Binding); }
        let bits = *source.words.get(head * contract.channels() + channel).ok_or(Error::Missing)?;
        finite(bits)
    }
}

pub(super) fn finite(bits: u32) -> Result<f64, Error> {
    let value = f32::from_bits(bits);
    if !value.is_finite() { return Err(Error::InvalidInput); }
    Ok(f64::from(value))
}

pub(super) struct AttentionPlan<'a> {
    contract: &'a AttentionContract,
    query: &'a TensorCapture,
    first: u64,
    count: usize,
}

pub(super) fn prepare<'a>(
    contract: &'a AttentionContract, query: &'a TensorCapture, image: &KvImageDescriptor,
) -> Result<AttentionPlan<'a>, Error> {
    image.encoded_len()?;
    let identity = query.source().identity();
    if query.receipt().contract() != contract.queries() || image.contract != contract.cache
        || identity.profile != contract.queries.profile()
        || identity.stream != image.stream || query.receipt().selection().batch != image.source_batch
        || query.source().dimensions() != contract.queries.dimensions()
    { return Err(Error::Binding); }
    let query_offset = identity.position.checked_sub(image.first_position).ok_or(Error::Missing)?;
    if query_offset >= image.token_count as u64 { return Err(Error::Missing); }
    if identity.sequence != image.first_sequence.checked_add(query_offset).ok_or(Error::Overflow)? {
        return Err(Error::Binding);
    }
    let end = identity.position.checked_add(1).ok_or(Error::Overflow)?;
    let first = match contract.mask {
        AttentionMask::FullPrefix => 0,
        AttentionMask::Sliding { tokens } => end.saturating_sub(tokens as u64),
    };
    if first < image.first_position { return Err(Error::Incomplete); }
    let count = usize::try_from(end - first).map_err(|_| Error::Limit)?;
    if count == 0 || count > MAX_KV_POSITIONS { return Err(Error::Limit); }
    Ok(AttentionPlan { contract, query, first, count })
}

impl AttentionPlan<'_> {
    pub(super) fn work(&self, resolution_steps_per_read: u64) -> Result<AttentionWork, Error> {
        if resolution_steps_per_read == 0 { return Err(Error::InvalidInput); }
        let heads = self.contract.queries.heads();
        let weights = heads.checked_mul(self.count).ok_or(Error::Overflow)?;
        let outputs = heads.checked_mul(self.contract.cache.values().channels()).ok_or(Error::Overflow)?;
        let channels = self.contract.queries.channels().checked_add(self.contract.cache.values().channels())
            .ok_or(Error::Overflow)?;
        let products = (weights as u64).checked_mul(channels as u64).ok_or(Error::Overflow)?;
        let workspace = weights.checked_add(outputs).and_then(|n| n.checked_mul(8)).ok_or(Error::Overflow)?;
        Ok(AttentionWork { scalar_products: products, exponentials: weights as u64,
            resolution_step_bound: products.checked_mul(resolution_steps_per_read).ok_or(Error::Overflow)?,
            workspace_bytes: workspace })
    }

    pub(super) fn evaluate(&self, rows: &impl AttentionRows) -> Result<AttentionValues, Error> {
        let heads = self.contract.queries.heads();
        let key_channels = self.contract.queries.channels();
        let channels = self.contract.cache.values().channels();
        let mut weights = zeroes(heads * self.count)?;
        let mut output = zeroes(heads * channels)?;
        let mut zero_weights = 0;
        for head in 0..heads {
            let cache_head = self.contract.cache.cache_head_for(head)?;
            let row = &mut weights[head * self.count..(head + 1) * self.count];
            let mut maximum = f64::NEG_INFINITY;
            for (offset, score) in row.iter_mut().enumerate() {
                let position = self.first + offset as u64;
                let mut dot = 0.0;
                for channel in 0..key_channels {
                    let query = finite(self.query.source().words[head * key_channels + channel])?;
                    dot += query * rows.scalar(false, position, cache_head, channel)?;
                }
                *score = dot * self.contract.scale();
                if !score.is_finite() { return Err(Error::Overflow); }
                maximum = maximum.max(*score);
            }
            let mut denominator = 0.0;
            for weight in row.iter_mut() {
                *weight = (*weight - maximum).exp();
                denominator += *weight;
            }
            // A nonempty row always includes exp(0) = 1. No all-masked zero fill.
            if !denominator.is_finite() || denominator <= 0.0 { return Err(Error::InvalidInput); }
            for weight in row.iter_mut() {
                *weight /= denominator;
                zero_weights += usize::from(*weight == 0.0);
            }
            for channel in 0..channels {
                let mut value = 0.0;
                for (offset, weight) in row.iter().enumerate() {
                    value += *weight * rows.scalar(true, self.first + offset as u64, cache_head, channel)?;
                }
                if !value.is_finite() { return Err(Error::Overflow); }
                output[head * channels + channel] = value;
            }
        }
        Ok(AttentionValues { heads, channels, first_position: self.first,
            positions: self.count, weights, output, zero_weights })
    }
}

fn zeroes(count: usize) -> Result<Vec<f64>, Error> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    values.resize(count, 0.0);
    Ok(values)
}
