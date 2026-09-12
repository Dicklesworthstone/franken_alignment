//! Actual full-decoder continuation over a compact lossy prefix. Only new suffix
//! rows are full precision. No old logits, live captures or authority are imported.

pub mod comparison;

use super::continuation::{Continuation, Prefix};
use super::super::{DecoderBudget, DecoderCheckpoint, DecoderModel, DecoderWork};
use super::super::super::experiment::KvCell;
use super::super::super::model::quantized::{KvQuantization, QuantizationBudget, QuantizationReport, QuantizedKvImage};
use crate::Error;
use std::fmt;
use std::rc::Rc;

struct Data {
    id: u64,
    model: DecoderModel,
    stream: u64,
    tokens: Vec<u32>,
    image: QuantizedKvImage,
    report: QuantizationReport,
}
/// Pins original parameters and token metadata, but NOT the original full KV
/// cache or original logits. After the source checkpoint drops, only the compact
/// prefix, parameters and newly computed suffix are needed for this experiment.
#[derive(Clone)]
pub struct QuantizedDecoder { data: Rc<Data> }
impl fmt::Debug for QuantizedDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuantizedDecoder").field("id", &self.id())
            .field("prefix_tokens", &self.data.tokens.len()).field("image", self.image()).finish_non_exhaustive()
    }
}
impl DecoderCheckpoint {
    pub fn quantized_experiment(&self, id: u64, policy: KvQuantization,
        budget: QuantizationBudget) -> Result<QuantizedDecoder, Error>
    {
        if id == 0 { return Err(Error::InvalidInput); }
        if self.cache().profile() != self.model().cache_profile() || self.cache().len() != self.tokens().len() {
            return Err(Error::Binding);
        }
        for layer in self.cache().descriptor().layers().values() {
            if layer.first_position != 0 || layer.first_sequence != 1 || layer.source_batch != 0
                || layer.stream != self.stream() { return Err(Error::Binding); }
        }
        let (image, report) = self.cache().quantize(policy, budget)?;
        let mut tokens = Vec::new(); tokens.try_reserve_exact(self.tokens().len()).map_err(|_| Error::Limit)?;
        tokens.extend_from_slice(self.tokens());
        Ok(QuantizedDecoder { data: Rc::new(Data { id, model: self.model().clone(), stream: self.stream(),
            tokens, image, report }) })
    }
}
impl QuantizedDecoder {
    pub fn id(&self) -> u64 { self.data.id }
    pub fn model(&self) -> &DecoderModel { &self.data.model }
    pub fn prefix_tokens(&self) -> &[u32] { &self.data.tokens }
    pub fn image(&self) -> &QuantizedKvImage { &self.data.image }
    pub fn report(&self) -> &QuantizationReport { &self.data.report }
    pub fn session(&self) -> QuantizedDecoderSession {
        QuantizedDecoderSession { plan: self.clone(), state: Continuation::default() }
    }
}
impl Prefix for QuantizedDecoder {
    fn model(&self) -> &DecoderModel { self.model() }
    fn stream(&self) -> u64 { self.data.stream }
    fn len(&self) -> usize { self.data.tokens.len() }
    fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> { self.image().bits(layer, cell) }
}
#[derive(Clone)]
pub struct QuantizedDecoderStep {
    pub experiment: u64,
    pub token: u32,
    pub position: u64,
    pub logits: Rc<[f32]>,
    pub work: DecoderWork,
}
impl fmt::Debug for QuantizedDecoderStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuantizedDecoderStep").field("experiment", &self.experiment)
            .field("position", &self.position).field("work", &self.work).finish_non_exhaustive()
    }
}
/// The first continuation token is explicit. Old checkpoint logits were produced
/// by a different cache and cannot masquerade as outputs of the lossy prefix.
/// Every later token uses newly computed logits. Failed advancement is atomic.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderSession, experiment::quantized::QuantizedDecoderSession};
/// fn activate(experiment: QuantizedDecoderSession) -> DecoderSession { experiment }
/// ```
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::quantized::QuantizedDecoderStep;
/// use fa_reference::action::consequence::activation::SourceFrame;
/// fn relabel(step: QuantizedDecoderStep) -> SourceFrame { step }
/// ```
pub struct QuantizedDecoderSession { plan: QuantizedDecoder, state: Continuation }
impl fmt::Debug for QuantizedDecoderSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuantizedDecoderSession").field("plan", &self.plan)
            .field("position", &self.position()).finish_non_exhaustive()
    }
}
impl QuantizedDecoderSession {
    pub fn plan(&self) -> &QuantizedDecoder { &self.plan }
    pub fn position(&self) -> u64 { self.state.position(&self.plan) }
    pub fn continuation_tokens(&self) -> &[u32] { self.state.tokens() }
    pub fn work(&self) -> DecoderWork { self.state.work() }
    pub fn logits(&self) -> Result<&[f32], Error> { self.state.logits() }
    pub fn greedy_token(&self) -> Result<u32, Error> { self.state.greedy_token() }
    pub fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error> { self.state.bits(&self.plan, layer, cell) }
    pub fn advance(&mut self, expected_position: u64, token: u32, budget: DecoderBudget) -> Result<QuantizedDecoderStep, Error> {
        let step = self.state.advance(&self.plan, expected_position, token, budget)?;
        Ok(QuantizedDecoderStep { experiment: self.plan.id(), token: step.token, position: step.position,
            logits: step.logits, work: step.work })
    }
    pub fn advance_greedy(&mut self, expected_position: u64, budget: DecoderBudget) -> Result<QuantizedDecoderStep, Error> {
        if expected_position != self.position() { return Err(Error::Stale); }
        let token = self.greedy_token()?;
        self.advance(expected_position, token, budget)
    }
}
