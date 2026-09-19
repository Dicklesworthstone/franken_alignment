//! Native model judgments for the ORIGINAL helper packet, not effect authority.
//! The supervisor/worker operator registers the mapping from an input profile to
//! an exact decoder profile. Equal epoch numbers are not an authentication claim.

pub mod peer;
#[cfg(unix)]
pub mod process;
pub mod bootstrap;
pub mod incremental;
pub use incremental::{NativeEvaluationProgress, NativeEvaluationWork};

#[cfg(test)]
mod tests;

use super::super::helper_workers::wire::WorkerInput;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, MAX_GENERATION_TOKENS, MAX_SAMPLING_ENTRIES,
    MAX_STOP_TOKENS,
};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::text::{
    TextDecoder, TextGenerationFailure, TextGenerationReport, TextGenerationRequest,
};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, TokenizationBudget, MAX_DECODE_BYTES, MAX_HEAP_POPS, MAX_INPUT_BYTES, MAX_PAIR_LOOKUPS,
};
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderProfile, MAX_DECODER_PRODUCTS};
use crate::full_input::{InputProfileBinding, MAX_PROFILE_BYTES};
use crate::round::Verdict;
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;

/// Independently provisioned mapping, frozen before receiving a worker request.
/// The two identity namespaces are explicit: no model-generation/epoch coercion.
/// No implicit BOS, chat template, question suffix, normalization or truncation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHelperPolicy {
    pub input_profile: InputProfileBinding,
    pub decoder_profile: DecoderProfile,
    pub max_new_tokens: usize,
    pub stop_tokens: Vec<u32>,
    pub tokenization: TokenizationBudget,
    pub generation: GenerationBudget,
    pub max_output_bytes: usize,
}

impl NativeHelperPolicy {
    // One policy admission for programmatic construction and cold checkpoint
    // startup. Invalid controls/budgets must not trigger an expensive weight read.
    fn check_tokenizer(&self, tokenizer: &ByteBpe)
        -> Result<(), Error>
    {
        if !tokenizer.binds(&self.decoder_profile) { return Err(Error::Binding); }
        if self.input_profile.profile_bytes.len() > MAX_PROFILE_BYTES
            || self.max_new_tokens > MAX_GENERATION_TOKENS
            || self.stop_tokens.len() > MAX_STOP_TOKENS
            || self.tokenization.input_bytes > MAX_INPUT_BYTES
            || self.tokenization.pair_lookups > MAX_PAIR_LOOKUPS
            || self.tokenization.heap_pops > MAX_HEAP_POPS
            || self.generation.scalar_products > MAX_DECODER_PRODUCTS
            || self.generation.sampling_entries > MAX_SAMPLING_ENTRIES
            || self.max_output_bytes > MAX_DECODE_BYTES { return Err(Error::Limit); }
        if self.max_new_tokens == 0 || self.stop_tokens.is_empty() { return Err(Error::InvalidInput); }
        let mut stops = BTreeSet::new();
        for token in &self.stop_tokens {
            // Suppressing an ordinary content token could hide a contradictory
            // suffix. Only explicitly registered non-text terminals may stop.
            if !tokenizer.is_control(*token)? { return Err(Error::Binding); }
            if !stops.insert(*token) { return Err(Error::Duplicate); }
        }
        if tokenizer.control_tokens().iter().any(|id| !stops.contains(id)) {
            return Err(Error::Incomplete);
        }
        let output_bound = self.max_new_tokens.checked_mul(tokenizer.max_content_bytes())
            .ok_or(Error::Limit)?;
        if output_bound > self.max_output_bytes { return Err(Error::Limit); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEvaluationError {
    Contract(Error),
    Admission(TextGenerationFailure),
    Incomplete(GenerationFinish),
    InvalidVerdict,
}
impl fmt::Display for NativeEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeEvaluationError {}
impl From<Error> for NativeEvaluationError {
    fn from(error: Error) -> Self { Self::Contract(error) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEvaluationStatus {
    AwaitingInput,
    /// Fully admitted input; the next call may compute one original token.
    Running,
    /// A caught unwind remains here; it can never rerun inference.
    Evaluating,
    Judged(Verdict),
    Failed(NativeEvaluationError),
    /// Destroyed unfinished ownership, not a completed judgment or refund.
    Cancelled,
}

/// One native decoder and one original wire input. No caller-supplied verdict,
/// mutable decoder, reset, seed replacement, or second evaluation is available.
/// The model's answer is an empirical judgment, NOT a certified safety decision.
/// A quiet monitor validates only its own registered numerical probe contract.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluator;
/// fn bypass(worker: &mut NativeEvaluator) { worker.decoder_mut(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluator;
/// use fa_reference::action::Permit;
/// fn elevate(worker: NativeEvaluator) -> Permit { worker }
/// ```
pub struct NativeEvaluator {
    execution: incremental::Execution,
    policy: NativeHelperPolicy,
    status: NativeEvaluationStatus,
    input: Option<WorkerInput>,
    report: Option<TextGenerationReport>,
}
impl fmt::Debug for NativeEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeEvaluator").field("status", &self.status)
            .field("position", &self.position())
            .field("sampled_draws", &self.sampled_draws()).finish_non_exhaustive()
    }
}
impl NativeEvaluator {
    pub fn new(decoder: TextDecoder, policy: NativeHelperPolicy) -> Result<Self, Error> {
        if decoder.profile() != &policy.decoder_profile { return Err(Error::Binding); }
        if decoder.position() != 0 || decoder.sampled_draws() != 0
            || decoder.status() != MonitoringStatus::Ready { return Err(Error::WrongState); }
        policy.check_tokenizer(decoder.tokenizer())?;
        Ok(Self { execution: incremental::Execution::Decoder(Box::new(decoder)), policy, status: NativeEvaluationStatus::AwaitingInput, input: None, report: None })
    }

    pub fn policy(&self) -> &NativeHelperPolicy { &self.policy }
    pub fn status(&self) -> NativeEvaluationStatus { self.status }
    /// Historical source data, not current evidence or an authority handle.
    pub fn input(&self) -> Option<&WorkerInput> { self.input.as_ref() }
    /// Includes held, truncated, invalid-schema and budget-exhausted generations.
    /// No synthetic report is returned for an admission failure.
    pub fn report(&self) -> Option<&TextGenerationReport> { self.report.as_ref() }
    pub fn position(&self) -> u64 { self.work().position }
    pub fn sampled_draws(&self) -> u64 { self.work().sampled_draws }

    /// The one-shot convenience API drives the SAME admitted input and original
    /// token steps as begin/advance. It still attempts only one request and never
    /// turns missing/held/truncated output into an invented vote.
    pub fn evaluate(&mut self, input: &WorkerInput) -> Result<Verdict, NativeEvaluationError> {
        self.begin(input)?;
        loop {
            let progress = self.advance(self.position())?;
            if let NativeEvaluationStatus::Judged(verdict) = progress.status { return Ok(verdict); }
        }
    }

    fn finish_report(&mut self, report: TextGenerationReport) -> Result<Verdict, NativeEvaluationError> {
        // Retain the actual numerical report BEFORE inspecting its result. Bad
        // output cannot erase consumed draws, computation or an earlier prefix.
        self.report = Some(report);
        let report = self.report.as_ref().expect("retained native generation");
        let numerical = report.generation();
        let actual = self.input.as_ref().ok_or(Error::Incomplete)?.actual_input();
        if report.prompt().source() != actual.submitted_bytes()
            || !report.prefix_controls().is_empty()
            || numerical.reviewed_prompt_tokens() != numerical.requested_prompt_tokens()
        { return Err(NativeEvaluationError::Incomplete(numerical.finish())); }
        if numerical.finish() != GenerationFinish::StopToken {
            return Err(NativeEvaluationError::Incomplete(numerical.finish()));
        }
        parse_verdict(report.bytes()?)
    }
}

// Exact schema: no trimming, first-word extraction, case folding, JSON repair,
// prose interpretation or replacement of malformed UTF-8. The prompt's registered
// question must specify this schema; a generated explanation is not a verdict.
fn parse_verdict(bytes: &[u8]) -> Result<Verdict, NativeEvaluationError> {
    match bytes {
        b"allow" => Ok(Verdict::Allow),
        b"hold" => Ok(Verdict::Hold),
        b"deny" => Ok(Verdict::Deny),
        b"abstain" => Ok(Verdict::Abstain),
        _ => Err(NativeEvaluationError::InvalidVerdict),
    }
}
