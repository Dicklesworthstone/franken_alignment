//! Native model judgments for the ORIGINAL helper packet, not effect authority.
//! The supervisor/worker operator registers the mapping from an input profile to
//! an exact decoder profile. Equal epoch numbers are not an authentication claim.

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
    TokenizationBudget, MAX_DECODE_BYTES, MAX_HEAP_POPS, MAX_INPUT_BYTES, MAX_PAIR_LOOKUPS,
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
    /// A caught unwind remains here; it can never rerun inference.
    Evaluating,
    Judged(Verdict),
    Failed(NativeEvaluationError),
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
    decoder: TextDecoder,
    policy: NativeHelperPolicy,
    status: NativeEvaluationStatus,
    input: Option<WorkerInput>,
    report: Option<TextGenerationReport>,
}
impl fmt::Debug for NativeEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeEvaluator").field("status", &self.status)
            .field("position", &self.decoder.position())
            .field("sampled_draws", &self.decoder.sampled_draws()).finish_non_exhaustive()
    }
}
impl NativeEvaluator {
    pub fn new(decoder: TextDecoder, policy: NativeHelperPolicy) -> Result<Self, Error> {
        if decoder.profile() != &policy.decoder_profile { return Err(Error::Binding); }
        if decoder.position() != 0 || decoder.sampled_draws() != 0
            || decoder.status() != MonitoringStatus::Ready { return Err(Error::WrongState); }
        if policy.input_profile.profile_bytes.len() > MAX_PROFILE_BYTES
            || policy.max_new_tokens > MAX_GENERATION_TOKENS
            || policy.stop_tokens.len() > MAX_STOP_TOKENS
            || policy.tokenization.input_bytes > MAX_INPUT_BYTES
            || policy.tokenization.pair_lookups > MAX_PAIR_LOOKUPS
            || policy.tokenization.heap_pops > MAX_HEAP_POPS
            || policy.generation.scalar_products > MAX_DECODER_PRODUCTS
            || policy.generation.sampling_entries > MAX_SAMPLING_ENTRIES
            || policy.max_output_bytes > MAX_DECODE_BYTES { return Err(Error::Limit); }
        if policy.max_new_tokens == 0 || policy.stop_tokens.is_empty() { return Err(Error::InvalidInput); }
        let mut stops = BTreeSet::new();
        for token in &policy.stop_tokens {
            // Suppressing an ordinary content token could hide a contradictory
            // suffix. Only explicitly registered non-text terminals may stop.
            if !decoder.tokenizer().is_control(*token)? { return Err(Error::Binding); }
            if !stops.insert(*token) { return Err(Error::Duplicate); }
        }
        if decoder.tokenizer().control_tokens().iter().any(|id| !stops.contains(id)) {
            return Err(Error::Incomplete);
        }
        let output_bound = policy.max_new_tokens.checked_mul(decoder.tokenizer().max_content_bytes())
            .ok_or(Error::Limit)?;
        if output_bound > policy.max_output_bytes { return Err(Error::Limit); }
        Ok(Self { decoder, policy, status: NativeEvaluationStatus::AwaitingInput, input: None, report: None })
    }

    pub fn policy(&self) -> &NativeHelperPolicy { &self.policy }
    pub fn status(&self) -> NativeEvaluationStatus { self.status }
    /// Historical source data, not current evidence or an authority handle.
    pub fn input(&self) -> Option<&WorkerInput> { self.input.as_ref() }
    /// Includes held, truncated, invalid-schema and budget-exhausted generations.
    /// No synthetic report is returned for an admission failure.
    pub fn report(&self) -> Option<&TextGenerationReport> { self.report.as_ref() }
    pub fn position(&self) -> u64 { self.decoder.position() }
    pub fn sampled_draws(&self) -> u64 { self.decoder.sampled_draws() }

    /// Run once over ALL original submitted bytes. Worker framing, round IDs,
    /// private salts, and peer votes are never appended to the model prompt.
    /// Only a complete reviewed StopToken finish can yield a categorical verdict.
    /// A refusal is not turned into Allow, Hold or an invented abstention vote.
    pub fn evaluate(&mut self, input: &WorkerInput) -> Result<Verdict, NativeEvaluationError> {
        if self.status != NativeEvaluationStatus::AwaitingInput {
            return Err(Error::WrongState.into());
        }
        self.status = NativeEvaluationStatus::Evaluating;
        self.input = Some(input.clone());
        let result = self.evaluate_once(input);
        self.status = match result {
            Ok(verdict) => NativeEvaluationStatus::Judged(verdict),
            Err(error) => NativeEvaluationStatus::Failed(error),
        };
        result
    }

    fn evaluate_once(&mut self, input: &WorkerInput) -> Result<Verdict, NativeEvaluationError> {
        let actual = input.actual_input();
        if actual.input_profile() != &self.policy.input_profile { return Err(Error::Binding.into()); }
        let report = self.decoder.generate(0, TextGenerationRequest {
            prompt: actual.submitted_bytes().to_vec(), prefix_controls: Vec::new(),
            max_new_tokens: self.policy.max_new_tokens, stop_tokens: self.policy.stop_tokens.clone(),
            tokenization: self.policy.tokenization, generation: self.policy.generation,
            max_output_bytes: self.policy.max_output_bytes,
        }).map_err(NativeEvaluationError::Admission)?;
        // Retain the actual numerical report BEFORE inspecting its result. Bad
        // output cannot erase consumed draws, computation or an earlier prefix.
        self.report = Some(report);
        let report = self.report.as_ref().expect("retained native generation");
        let numerical = report.generation();
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
