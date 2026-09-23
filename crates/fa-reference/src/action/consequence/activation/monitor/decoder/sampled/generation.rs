//! Bounded inference requests over the original monitored decoder. These are
//! supervisor-side numerical observations, not actor knowledge or effect permits.
//! Full-prompt numerical admission precedes all inference; monitor holds and
//! numerical failures still retain actual work and never roll back a sampled draw.

pub mod incremental;
pub mod tokenizer;
pub mod text;

use super::{MonitoredSampledDecoder, MonitoredStep, MonitoringStatus};
use super::super::DecoderReview;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderProfile, DecoderWork, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SampleBudget;
use crate::Error;
use std::fmt;
use std::rc::Rc;

pub const MAX_GENERATION_TOKENS: usize = 4_096;
pub const MAX_STOP_TOKENS: usize = 256;
pub const MAX_SAMPLING_ENTRIES: u64 = 16_777_216;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationBudget {
    /// Cumulative admission across prompt AND continuation, not a per-token
    /// allowance. Work admitted before a failure is never refunded.
    pub scalar_products: u64,
    /// Full vocabulary examinations admitted to the original sampler.
    pub sampling_entries: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationRequest {
    /// Entire additional prefix, validated and numerically budgeted before its
    /// first token computes. Predictable budget failure cannot truncate input.
    pub prompt: Vec<u32>,
    pub max_new_tokens: usize,
    /// Stop IDs are inspected only AFTER their own complete quiet review.
    /// They consume a draw and a context position but are not output tokens.
    pub stop_tokens: Vec<u32>,
    pub budget: GenerationBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationFinish {
    TokenLimit,
    StopToken,
    BudgetExhausted,
    Held,
    Failed(Error),
    /// A supervisor withdrew unfinished work at an acknowledged boundary.
    /// This does not assert complete prompt review or a monitored stop token.
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenerationWork {
    pub admitted_scalar_products: u64,
    pub admitted_sampling_entries: u64,
    /// Includes a sampled step that held or failed after admission.
    pub attempted_samples: usize,
}

/// Only completely reviewed continuation IDs are retained. No logits,
/// sampler words or token ID from a held computation enters this report.
/// Failed/held reports may contain the previously released quiet prefix;
/// neither that prefix nor TokenLimit claims semantic safety or authority.
/// Cloning copies observations, not the execution owner or its remaining budget.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::GenerationReport;
/// fn grant(report: GenerationReport) -> Permit { report }
/// ```
#[derive(Clone)]
pub struct GenerationReport {
    start_position: u64,
    end_position: u64,
    requested_prompt_tokens: usize,
    reviewed_prompt_tokens: usize,
    tokens: Vec<u32>,
    finish: GenerationFinish,
    work: GenerationWork,
    last_review: Option<Rc<DecoderReview>>,
}
impl GenerationReport {
    pub fn start_position(&self) -> u64 { self.start_position }
    pub fn end_position(&self) -> u64 { self.end_position }
    pub fn requested_prompt_tokens(&self) -> usize { self.requested_prompt_tokens }
    pub fn reviewed_prompt_tokens(&self) -> usize { self.reviewed_prompt_tokens }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn finish(&self) -> GenerationFinish { self.finish }
    pub fn work(&self) -> GenerationWork { self.work }
    pub fn last_review(&self) -> Option<&DecoderReview> { self.last_review.as_deref() }
}
impl fmt::Debug for GenerationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationReport")
            .field("start_position", &self.start_position)
            .field("end_position", &self.end_position)
            .field("reviewed_prompt_tokens", &self.reviewed_prompt_tokens)
            .field("released_tokens", &self.tokens.len())
            .field("finish", &self.finish).field("work", &self.work)
            .finish_non_exhaustive()
    }
}

// Only the durable supervisor's cancellation reducer calls this projection.
// There is no public constructor, cursor import, inference or owner extraction.
// An unstarted intent may not yet have passed numerical admission; cancellation
// does not turn it into an admitted/completely reviewed prompt.
#[cfg(unix)]
impl GenerationReport {
    pub(crate) fn cancelled(start: u64, request: &GenerationRequest,
        progress: Option<&incremental::GenerationProgress>) -> Result<Self, Error>
    {
        let mut report = Self { start_position: start, end_position: start,
            requested_prompt_tokens: request.prompt.len(), reviewed_prompt_tokens: 0,
            tokens: Vec::new(), finish: GenerationFinish::Cancelled,
            work: GenerationWork::default(), last_review: None };
        if let Some(progress) = progress {
            if progress.finish().is_some() || progress.start_position() != start
                || progress.requested_prompt_tokens() != request.prompt.len()
                || progress.reviewed_prompt_tokens() > request.prompt.len()
                || progress.tokens().len() > request.max_new_tokens
                || progress.work().admitted_scalar_products > request.budget.scalar_products
                || progress.work().admitted_sampling_entries > request.budget.sampling_entries
            { return Err(Error::Binding); }
            report.tokens.try_reserve_exact(progress.tokens().len()).map_err(|_| Error::Limit)?;
            report.tokens.extend_from_slice(progress.tokens());
            report.end_position = progress.position();
            report.reviewed_prompt_tokens = progress.reviewed_prompt_tokens();
            report.work = progress.work();
            report.last_review = progress.last_review().cloned().map(Rc::new);
        }
        Ok(report)
    }
}

// Closed to external implementations: this is composition of existing owners,
// not a public callback capable of asserting that arbitrary logits were quiet.
pub(crate) trait GenerationOwner {
    fn generation_profile(&self) -> Result<&DecoderProfile, Error>;
    fn generation_position(&self) -> Result<u64, Error>;
    fn generation_status(&self) -> Result<MonitoringStatus, Error>;
    fn generation_estimate(&self, tokens: usize) -> Result<DecoderWork, Error>;
    fn generation_forced(&mut self, position: u64, token: u32,
        budget: DecoderBudget) -> Result<MonitoredStep, Error>;
    fn generation_sampled(&mut self, position: u64,
        budget: SampleBudget) -> Result<MonitoredStep, Error>;
}
impl GenerationOwner for MonitoredSampledDecoder {
    fn generation_profile(&self) -> Result<&DecoderProfile, Error> { Ok(self.profile()) }
    fn generation_position(&self) -> Result<u64, Error> { Ok(self.position()) }
    fn generation_status(&self) -> Result<MonitoringStatus, Error> { Ok(self.status()) }
    fn generation_estimate(&self, tokens: usize) -> Result<DecoderWork, Error> { self.estimate(tokens) }
    fn generation_forced(&mut self, position: u64, token: u32,
        budget: DecoderBudget) -> Result<MonitoredStep, Error>
    { self.advance_forced(position, token, budget) }
    fn generation_sampled(&mut self, position: u64,
        budget: SampleBudget) -> Result<MonitoredStep, Error>
    { self.advance_sampled(position, budget).map(super::MonitoredSampledStep::into_monitored) }
}

impl MonitoredSampledDecoder {
    /// Run teacher forcing followed by bounded autoregressive sampling through
    /// the SAME mandatory monitor. An empty prompt continues only an existing
    /// reviewed prefix. Zero new tokens performs monitored prefill only.
    ///
    /// Structural/position/context errors and insufficient full-prompt numerical
    /// allowance refuse before ANY inference. After admission, runtime errors and
    /// holds return explicit partial progress. The monitor may still hold during
    /// prefill; this is not a promise that all prompt inference will succeed.
    /// This is synchronous numerical execution, not an async runtime, token
    /// transport, tokenizer, provider authentication or publication API.
    pub fn generate(&mut self, expected_position: u64, request: GenerationRequest)
        -> Result<GenerationReport, Error>
    { drive(self, expected_position, request) }
}

/// One-shot and incremental execution share the SAME admission and step reducer.
/// No caller can supply a cursor, precharged report or an alternative owner.
pub(crate) fn drive<O: GenerationOwner>(owner: &mut O, expected_position: u64,
    request: GenerationRequest) -> Result<GenerationReport, Error>
{
    let mut cursor = incremental::GenerationCursor::new(owner, expected_position, request)?;
    while !cursor.is_complete() { cursor.advance(owner)?; }
    cursor.into_report()
}

#[cfg(test)]
#[path = "generation/tests.rs"]
mod tests;
