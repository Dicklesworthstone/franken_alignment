//! Original-engine continuation over a learned compact prefix, never a recapture
//! of approximate values as live state. New suffix rows stay full precision.
pub mod comparison;
use super::continuation::{Continuation, Prefix};
use super::super::{DecoderBudget, DecoderCheckpoint, DecoderModel, DecoderWork};
use super::super::super::experiment::KvCell;
use super::super::super::model::learned::{CompressionBudget, CompressionReport, LearnedKvCodec, LearnedKvImage};
use crate::Error;
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

pub const MAX_RECONSTRUCTION_PRODUCTS: u64 = super::super::MAX_DECODER_PRODUCTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedDecoderBudget {
    pub decoder: DecoderBudget,
    /// Additional products in reconstructing the learned PREFIX. The original
    /// decoder/attention budget does not count them; do not hide this cost there.
    pub reconstruction_products: u64,
}
struct Data {
    id: u64,
    evaluation_origin: u64,
    model: DecoderModel,
    stream: u64,
    tokens: Vec<u32>,
    image: LearnedKvImage,
    report: CompressionReport,
}
#[derive(Clone)]
pub struct LearnedDecoder { data: Rc<Data> }
impl fmt::Debug for LearnedDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedDecoder").field("id", &self.id())
            .field("evaluation_origin", &self.data.evaluation_origin).field("image", self.image()).finish_non_exhaustive()
    }
}
impl DecoderCheckpoint {
    /// The evaluated task and source stream must not be in the fitted corpus.
    /// This uses the ORIGINAL complete checkpoint only to construct a lossy
    /// image and pin its actual model parameters and token metadata. The returned
    /// plan retains neither original KV arrays nor old next-token logits.
    pub fn learned_experiment(&self, id: u64, evaluation_origin: u64, codec: &LearnedKvCodec,
        budget: CompressionBudget) -> Result<LearnedDecoder, Error>
    {
        if id == 0 { return Err(Error::InvalidInput); }
        if self.cache().profile() != self.model().cache_profile() || self.cache().len() != self.tokens().len() {
            return Err(Error::Binding);
        }
        for layer in self.cache().descriptor().layers().values() {
            if layer.first_position != 0 || layer.first_sequence != 1 || layer.source_batch != 0
                || layer.stream != self.stream() { return Err(Error::Binding); }
        }
        let (image, report) = codec.evaluate_held_out(evaluation_origin, self.cache(), budget)?;
        let mut tokens = Vec::new(); tokens.try_reserve_exact(self.tokens().len()).map_err(|_| Error::Limit)?;
        tokens.extend_from_slice(self.tokens());
        Ok(LearnedDecoder { data: Rc::new(Data { id, evaluation_origin, model: self.model().clone(),
            stream: self.stream(), tokens, image, report }) })
    }
}
impl LearnedDecoder {
    pub fn id(&self) -> u64 { self.data.id }
    pub fn evaluation_origin(&self) -> u64 { self.data.evaluation_origin }
    pub fn model(&self) -> &DecoderModel { &self.data.model }
    pub fn prefix_tokens(&self) -> &[u32] { &self.data.tokens }
    pub fn image(&self) -> &LearnedKvImage { &self.data.image }
    pub fn report(&self) -> &CompressionReport { &self.data.report }
    pub fn session(&self) -> LearnedDecoderSession {
        LearnedDecoderSession { plan: self.clone(), state: Continuation::default(), reconstruction_products: 0 }
    }
    /// Full-prefix original attention performs one K and one V read per query
    /// coordinate per stored-prefix position. Each reconstruction uses rank
    /// products. The runtime meter also enforces the admitted bound independently.
    pub fn reconstruction_products_for(&self, tokens: usize) -> Result<u64, Error> {
        self.model().estimate(self.prefix_tokens().len(), tokens)?;
        reconstruction_products(self.model(), self.prefix_tokens().len(), self.image().codec().policy().rank(), tokens)
    }
}
pub(super) fn reconstruction_products(model: &DecoderModel, prefix: usize, rank: usize, tokens: usize) -> Result<u64, Error> {
    let shape = model.profile().shape();
    let count = (shape.layers as u64).checked_mul(shape.hidden as u64).and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_mul(prefix as u64)).and_then(|n| n.checked_mul(rank as u64))
        .and_then(|n| n.checked_mul(tokens as u64)).ok_or(Error::Overflow)?;
    if count > MAX_RECONSTRUCTION_PRODUCTS { return Err(Error::Limit); }
    Ok(count)
}
impl Prefix for LearnedDecoder {
    fn model(&self) -> &DecoderModel { self.model() }
    fn stream(&self) -> u64 { self.data.stream }
    fn len(&self) -> usize { self.prefix_tokens().len() }
    fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> { self.image().bits(layer, cell) }
}
struct MeteredPrefix<'a> { plan: &'a LearnedDecoder, products: Cell<u64>, limit: u64 }
impl Prefix for MeteredPrefix<'_> {
    fn model(&self) -> &DecoderModel { self.plan.model() }
    fn stream(&self) -> u64 { self.plan.data.stream }
    fn len(&self) -> usize { self.plan.prefix_tokens().len() }
    fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> {
        let next = self.products.get().checked_add(self.plan.image().codec().policy().rank() as u64).ok_or(Error::Overflow)?;
        if next > self.limit { return Err(Error::Limit); }
        let bits = self.plan.image().bits(layer, cell)?;
        self.products.set(next);
        Ok(bits)
    }
}
#[derive(Clone)]
pub struct LearnedDecoderStep {
    pub experiment: u64,
    pub token: u32,
    pub position: u64,
    pub logits: Rc<[f32]>,
    pub work: DecoderWork,
    pub reconstruction_products: u64,
}
impl fmt::Debug for LearnedDecoderStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedDecoderStep").field("experiment", &self.experiment)
            .field("position", &self.position).field("work", &self.work)
            .field("reconstruction_products", &self.reconstruction_products).finish_non_exhaustive()
    }
}

/// The first consumed token is explicitly supplied: the checkpoint's old logits
/// were produced by a different cache. Later greedy choices use only completed
/// computations through this lossy prefix. Each sibling owns its original suffix
/// state independently. Failed forward passes publish neither token nor counters.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderSession, experiment::learned::LearnedDecoderSession};
/// fn activate(experiment: LearnedDecoderSession) -> DecoderSession { experiment }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::learned::LearnedDecoderSession;
/// fn relabel(experiment: LearnedDecoderSession) { experiment.checkpoint(); }
/// ```
pub struct LearnedDecoderSession { plan: LearnedDecoder, state: Continuation, reconstruction_products: u64 }
impl fmt::Debug for LearnedDecoderSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LearnedDecoderSession").field("plan", &self.plan)
            .field("position", &self.position()).finish_non_exhaustive()
    }
}
impl LearnedDecoderSession {
    pub fn plan(&self) -> &LearnedDecoder { &self.plan }
    pub fn position(&self) -> u64 { self.state.position(&self.plan) }
    pub fn continuation_tokens(&self) -> &[u32] { self.state.tokens() }
    pub fn work(&self) -> DecoderWork { self.state.work() }
    /// Actual prefix reconstruction products from successful generated steps.
    /// Ad hoc scalar inspection and failed attempts are not included.
    pub fn reconstruction_products(&self) -> u64 { self.reconstruction_products }
    pub fn logits(&self) -> Result<&[f32], Error> { self.state.logits() }
    pub fn greedy_token(&self) -> Result<u32, Error> { self.state.greedy_token() }
    pub fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> { self.state.bits(&self.plan, layer, cell) }
    pub fn advance(&mut self, expected_position: u64, token: u32, budget: LearnedDecoderBudget) -> Result<LearnedDecoderStep, Error> {
        if expected_position != self.position() { return Err(Error::Stale); }
        let bound = reconstruction_products(self.plan.model(), self.plan.prefix_tokens().len(),
            self.plan.image().codec().policy().rank(), 1)?;
        if budget.reconstruction_products > MAX_RECONSTRUCTION_PRODUCTS || bound > budget.reconstruction_products {
            return Err(Error::Limit);
        }
        self.reconstruction_products.checked_add(bound).ok_or(Error::Overflow)?;
        let prefix = MeteredPrefix { plan: &self.plan, products: Cell::new(0), limit: bound };
        // Original Continuation stages the whole original forward pass before
        // appending its new rows and logits. Nothing fallible follows that commit.
        let step = self.state.advance(&prefix, expected_position, token, budget.decoder)?;
        let products = prefix.products.get();
        self.reconstruction_products += products;
        Ok(LearnedDecoderStep { experiment: self.plan.id(), token: step.token, position: step.position,
            logits: step.logits, work: step.work, reconstruction_products: products })
    }
    pub fn advance_greedy(&mut self, expected_position: u64, budget: LearnedDecoderBudget) -> Result<LearnedDecoderStep, Error> {
        if expected_position != self.position() { return Err(Error::Stale); }
        let token = self.greedy_token()?;
        self.advance(expected_position, token, budget)
    }
}
