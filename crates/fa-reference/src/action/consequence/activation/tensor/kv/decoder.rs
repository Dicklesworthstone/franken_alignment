//! Original-token execution of an explicit dense decoder profile (FA-025).
//!
//! The same checked attention and all-layer capture paths handle actual computed
//! Q/K/V. This is CPU reference inference, not a trained-model or serving-host
//! qualification. There is no effect authority, tokenizer, runtime or I/O here.

mod weights;
pub use weights::{DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape,
    MAX_DECODER_HIDDEN, MAX_DECODER_INTERMEDIATE, MAX_DECODER_PARAMETERS, MAX_DECODER_VOCABULARY};

use super::attention::{self, AttentionRows, MAX_ATTENTION_PRODUCTS,
    MAX_ATTENTION_RESOLUTION_STEPS, MAX_ATTENTION_WORKSPACE_BYTES, AttentionBudget};
use super::image::KvImageDescriptor;
use super::model::{ModelKvBudget, ModelKvCapture, ModelKvImage};
use super::{KvAppend, KvCapture};
use super::super::{BufferIdentity, HostTensor, TensorCapture, TensorContract, TokenSelection};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use weights::{dense_layout, matrix, residual, rms, rotary, rounded, swiglu};

pub const MAX_DECODER_PRODUCTS: u64 = 1_099_511_627_776;

/// Admission budget for matrix and attention product terms, not a FLOP, memory,
/// latency, wall-clock cancellation, or production-resource accounting claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderBudget { pub scalar_products: u64 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecoderWork {
    pub tokens: u64,
    pub matrix_products: u64,
    pub attention_products: u64,
    pub attention_exponentials: u64,
    pub normalization_coordinates: u64,
    pub rotary_pairs: u64,
    pub gate_coordinates: u64,
    pub cache_values_appended: u64,
}

impl DecoderWork {
    pub fn scalar_products(self) -> Result<u64, Error> {
        self.matrix_products.checked_add(self.attention_products).ok_or(Error::Overflow)
    }
    fn check(self, budget: DecoderBudget) -> Result<(), Error> {
        if budget.scalar_products > MAX_DECODER_PRODUCTS || self.scalar_products()? > budget.scalar_products {
            return Err(Error::Limit);
        }
        Ok(())
    }
    fn add(self, other: Self) -> Result<Self, Error> {
        let add = |a: u64, b: u64| a.checked_add(b).ok_or(Error::Overflow);
        Ok(Self {
            tokens: add(self.tokens, other.tokens)?,
            matrix_products: add(self.matrix_products, other.matrix_products)?,
            attention_products: add(self.attention_products, other.attention_products)?,
            attention_exponentials: add(self.attention_exponentials, other.attention_exponentials)?,
            normalization_coordinates: add(self.normalization_coordinates, other.normalization_coordinates)?,
            rotary_pairs: add(self.rotary_pairs, other.rotary_pairs)?,
            gate_coordinates: add(self.gate_coordinates, other.gate_coordinates)?,
            cache_values_appended: add(self.cache_values_appended, other.cache_values_appended)?,
        })
    }
}

impl DecoderModel {
    /// Exact loop-term count for the declared profile and contiguous positions.
    /// Transcendentals, normalization, allocations and byte movement are separate.
    pub fn estimate(&self, first_position: usize, tokens: usize) -> Result<DecoderWork, Error> {
        let s = self.profile().shape();
        let end = first_position.checked_add(tokens).ok_or(Error::Overflow)?;
        if end > s.context { return Err(Error::Limit); }
        let h = s.hidden as u64;
        let k = self.profile().cache_width() as u64;
        let i = s.intermediate as u64;
        let layers = s.layers as u64;
        let n = tokens as u64;
        let positions = n * (2 * first_position as u64 + n + 1) / 2;
        Ok(DecoderWork {
            tokens: n,
            matrix_products: n * (layers * (2 * h * h + 2 * k * h + 3 * i * h) + s.vocabulary as u64 * h),
            attention_products: layers * 2 * h * positions,
            attention_exponentials: layers * s.query_heads as u64 * positions,
            normalization_coordinates: n * (2 * layers + 1) * h,
            rotary_pairs: n * layers * (h + k) / 2,
            gate_coordinates: n * layers * i,
            cache_values_appended: n * layers * 2 * k,
        })
    }

    pub fn session(&self, stream: u64) -> Result<DecoderSession, Error> {
        let shape = self.profile().shape();
        let cache = ModelKvCapture::new(self.data.cache.clone(), stream, 0, 0, 1, ModelKvBudget {
            positions: shape.context,
            normalized_values: shape.context * self.data.cache.values_per_token(),
        })?;
        Ok(DecoderSession { model: self.clone(), stream, cache, tokens: Vec::new(), logits: None,
            work: DecoderWork::default() })
    }

    /// Uses the ORIGINAL token IDs, never a decode/re-tokenize round trip. All
    /// IDs, context bounds and the COMPLETE run budget are checked before work.
    /// A refusal returns no partially constructed session or imported cache.
    pub fn recompute(&self, stream: u64, tokens: &[u32], budget: DecoderBudget) -> Result<DecoderSession, Error> {
        self.estimate(0, tokens.len())?.check(budget)?;
        if tokens.iter().any(|token| *token as usize >= self.profile().shape().vocabulary) {
            return Err(Error::InvalidInput);
        }
        let mut session = self.session(stream)?;
        for (position, token) in tokens.iter().copied().enumerate() {
            let products = self.estimate(position, 1)?.scalar_products()?;
            session.advance(position as u64, token, DecoderBudget { scalar_products: products })?;
        }
        Ok(session)
    }
}

/// Computed captures from THIS reference implementation, not another host's
/// observations. Model identity and parameter authenticity are caller assumptions.
#[derive(Clone, Debug)]
pub struct DecoderLayerObservation {
    pub layer: u64,
    pub query: TensorCapture,
    pub residual: TensorCapture,
}

/// Successful single-token computation. Logits are numerical observations,
/// never permissions or evidence that a model's chosen token is acceptable.
#[derive(Clone)]
pub struct DecoderStep {
    pub token: u32,
    pub position: u64,
    pub logits: Rc<[f32]>,
    pub layers: Vec<DecoderLayerObservation>,
    pub work: DecoderWork,
}
impl fmt::Debug for DecoderStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderStep").field("token", &self.token).field("position", &self.position)
            .field("vocabulary", &self.logits.len()).field("work", &self.work).finish_non_exhaustive()
    }
}

/// Owns one actual computed cache with no mutable layer accessor. A failed token
/// leaves every layer, token history, logits and successful-work counter intact.
/// General allocator aborts are not recoverable Result-level transactions.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderSession;
/// use fa_reference::action::Permit;
/// fn authorize(session: DecoderSession) -> Permit { session }
/// ```
pub struct DecoderSession {
    model: DecoderModel,
    stream: u64,
    cache: ModelKvCapture,
    tokens: Vec<u32>,
    logits: Option<Rc<[f32]>>,
    work: DecoderWork,
}
impl fmt::Debug for DecoderSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderSession").field("profile", self.model.profile())
            .field("position", &self.tokens.len()).field("work", &self.work).finish_non_exhaustive()
    }
}

impl DecoderSession {
    pub fn model(&self) -> &DecoderModel { &self.model }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn position(&self) -> u64 { self.tokens.len() as u64 }
    pub fn work(&self) -> DecoderWork { self.work }
    pub fn logits(&self) -> Result<&[f32], Error> { self.logits.as_deref().ok_or(Error::Incomplete) }
    pub fn cache_image(&self) -> Result<ModelKvImage, Error> { self.cache.snapshot(self.cache.revision()) }

    /// Full vocabulary scan; equal scores select the lowest original token ID.
    /// EOS/stop strings and tokenization are not guessed by this numerical API.
    pub fn greedy_token(&self) -> Result<u32, Error> {
        let logits = self.logits()?;
        let mut selected = 0;
        for index in 1..logits.len() {
            if logits[index] > logits[selected] { selected = index; }
        }
        Ok(selected as u32)
    }

    pub fn advance(&mut self, expected_position: u64, token: u32, budget: DecoderBudget) -> Result<DecoderStep, Error> {
        if expected_position != self.position() { return Err(Error::Stale); }
        let shape = self.model.profile().shape();
        if token as usize >= shape.vocabulary { return Err(Error::InvalidInput); }
        let work = self.model.estimate(self.tokens.len(), 1)?;
        work.check(budget)?;
        let next_work = self.work.add(work)?;
        let position = self.position();
        let sequence = position.checked_add(1).ok_or(Error::Overflow)?;
        let selected = TokenSelection { batch: 0, token: 0, first_position: position,
            stream: self.stream, sequence };
        let data = &self.model.data;
        let h = shape.hidden;
        let start = token as usize * h;
        let mut hidden = data.embeddings[start..start + h].to_vec();
        let mut staged = Vec::new();
        let mut observations = Vec::new();
        staged.try_reserve_exact(shape.layers).map_err(|_| Error::Limit)?;
        observations.try_reserve_exact(shape.layers).map_err(|_| Error::Limit)?;
        for (index, layer) in data.layers.iter().enumerate() {
            let id = index as u64 + 1;
            let w = &layer.weights;
            let normalized = rms(&hidden, &w.attention_norm, data.profile.epsilon())?;
            let mut q = matrix(&w.queries, h, &normalized)?;
            let mut k = matrix(&w.keys, data.profile.cache_width(), &normalized)?;
            let v = matrix(&w.values, data.profile.cache_width(), &normalized)?;
            rotary(&mut q, data.profile.head_width(), position, data.profile.theta())?;
            rotary(&mut k, data.profile.head_width(), position, data.profile.theta())?;
            let query = capture(layer.attention.queries(), &q, selected)?;
            let descriptor = KvImageDescriptor {
                contract: layer.attention.cache().clone(), stream: self.stream, source_batch: 0,
                first_position: 0, first_sequence: 1, source_revision: sequence,
                token_count: self.tokens.len() + 1,
            };
            let plan = attention::prepare(&layer.attention, &query, &descriptor)?;
            plan.work(1)?.check(AttentionBudget { scalar_products: MAX_ATTENTION_PRODUCTS,
                resolution_steps: MAX_ATTENTION_RESOLUTION_STEPS, workspace_bytes: MAX_ATTENTION_WORKSPACE_BYTES })?;
            let rows = ExtendedRows { previous: self.cache.layer(id)?, position, keys: &k, values: &v };
            let attention = plan.evaluate(&rows)?;
            let mixed = attention.output().iter().copied().map(rounded).collect::<Result<Vec<_>, _>>()?;
            let projected = matrix(&w.attention_output, h, &mixed)?;
            hidden = residual(&hidden, &projected)?;
            let normalized = rms(&hidden, &w.feed_forward_norm, data.profile.epsilon())?;
            let gate = matrix(&w.gate, shape.intermediate, &normalized)?;
            let up = matrix(&w.up, shape.intermediate, &normalized)?;
            let activated = swiglu(gate, &up)?;
            let down = matrix(&w.down, h, &activated)?;
            hidden = residual(&hidden, &down)?;
            let residual = capture(&layer.residual, &hidden, selected)?;
            observations.push(DecoderLayerObservation { layer: id, query, residual });
            staged.push((id, words(&k), words(&v)));
        }
        let normalized = rms(&hidden, &data.final_norm, data.profile.epsilon())?;
        let logits: Rc<[f32]> = matrix(&data.output, shape.vocabulary, &normalized)?.into();
        let step = DecoderStep { token, position, logits: Rc::clone(&logits), layers: observations, work };
        self.tokens.try_reserve(1).map_err(|_| Error::Limit)?;
        let requests = staged.iter().map(|(id, keys, values)| {
            let contract = &data.cache.layers()[id];
            (*id, KvAppend {
                keys: HostTensor { identity: BufferIdentity { object: contract.keys().profile().tap, generation: sequence },
                    layout: &data.key_layout, bytes: keys },
                values: HostTensor { identity: BufferIdentity { object: contract.values().profile().tap, generation: sequence },
                    layout: &data.value_layout, bytes: values },
                first_token: 0, token_count: 1, buffer_first_position: position, first_sequence: sequence,
            })
        }).collect::<BTreeMap<_, _>>();
        // Original model-wide capture stages ALL layers before publishing any.
        // No fallible action follows its commit; no prior scalar rows are copied.
        self.cache.append(self.cache.revision(), requests)?;
        self.tokens.push(token);
        self.logits = Some(logits);
        self.work = next_work;
        Ok(step)
    }
}

struct ExtendedRows<'a> {
    previous: &'a KvCapture,
    position: u64,
    keys: &'a [f32],
    values: &'a [f32],
}
impl AttentionRows for ExtendedRows<'_> {
    fn scalar(&self, values: bool, position: u64, head: usize, channel: usize) -> Result<f64, Error> {
        let contract = if values { self.previous.contract().values() } else { self.previous.contract().keys() };
        if head >= contract.heads() || channel >= contract.channels() { return Err(Error::Binding); }
        let index = head * contract.channels() + channel;
        if position == self.position {
            let row = if values { self.values } else { self.keys };
            return row.get(index).map(|value| f64::from(*value)).ok_or(Error::Missing);
        }
        if position > self.position { return Err(Error::Missing); }
        let token = self.previous.token(position)?;
        let source = if values { token.value().source() } else { token.key().source() };
        attention::finite(*source.words.get(index).ok_or(Error::Missing)?)
    }
}

fn words(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_le_bytes()).collect()
}
fn capture(contract: &TensorContract, values: &[f32], selected: TokenSelection) -> Result<TensorCapture, Error> {
    let layout = dense_layout(contract.heads(), contract.channels())?;
    let bytes = words(values);
    contract.capture(HostTensor {
        identity: BufferIdentity { object: contract.profile().tap, generation: selected.sequence },
        layout: &layout, bytes: &bytes,
    }, selected)
}
