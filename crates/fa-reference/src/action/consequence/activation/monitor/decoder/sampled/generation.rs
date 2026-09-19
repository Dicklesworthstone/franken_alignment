//! Bounded inference requests over the original monitored decoder. These are
//! supervisor-side numerical observations, not actor knowledge or effect permits.
//! Full-prompt numerical admission precedes all inference; monitor holds and
//! numerical failures still retain actual work and never roll back a sampled draw.

use super::{MonitoredSampledDecoder, MonitoredStep, MonitoringStatus};
use super::super::DecoderReview;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderProfile, DecoderWork, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{
    SampleBudget, SamplingBudget,
};
use crate::Error;
use std::collections::BTreeSet;
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
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::GenerationReport;
/// fn grant(report: GenerationReport) -> Permit { report }
/// ```
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

pub(crate) fn drive<O: GenerationOwner>(owner: &mut O, expected_position: u64,
    request: GenerationRequest) -> Result<GenerationReport, Error>
{
    if owner.generation_status()? != MonitoringStatus::Ready { return Err(Error::WrongState); }
    if owner.generation_position()? != expected_position { return Err(Error::Stale); }
    let shape = owner.generation_profile()?.shape();
    let vocabulary = shape.vocabulary;
    let count = request.prompt.len().checked_add(request.max_new_tokens).ok_or(Error::Overflow)?;
    if count > MAX_GENERATION_TOKENS || request.stop_tokens.len() > MAX_STOP_TOKENS
        || request.budget.scalar_products > MAX_DECODER_PRODUCTS
        || request.budget.sampling_entries > MAX_SAMPLING_ENTRIES
    { return Err(Error::Limit); }
    if count == 0 || (request.prompt.is_empty() && expected_position == 0)
        || request.prompt.iter().chain(&request.stop_tokens).any(|token| *token as usize >= vocabulary)
    { return Err(Error::InvalidInput); }
    let start = usize::try_from(expected_position).map_err(|_| Error::Limit)?;
    if start.checked_add(count).ok_or(Error::Overflow)? > shape.context { return Err(Error::Limit); }
    let stops: BTreeSet<u32> = request.stop_tokens.iter().copied().collect();
    if stops.len() != request.stop_tokens.len() { return Err(Error::Duplicate); }
    // Do not leave a quiet but predictably truncated supplied prompt behind.
    // This is the ORIGINAL whole-prefix estimate, not another cost formula or
    // a reservation copied into a branch. Per-step work is still charged once.
    let prompt_products = owner.generation_estimate(request.prompt.len())?.scalar_products()?;
    if prompt_products > request.budget.scalar_products { return Err(Error::Limit); }
    let mut tokens = Vec::new();
    tokens.try_reserve_exact(request.max_new_tokens).map_err(|_| Error::Limit)?;
    let mut report = GenerationReport {
        start_position: expected_position, end_position: expected_position,
        requested_prompt_tokens: request.prompt.len(), reviewed_prompt_tokens: 0,
        tokens, finish: GenerationFinish::TokenLimit, work: GenerationWork::default(), last_review: None,
    };
    let vocabulary = u64::try_from(vocabulary).map_err(|_| Error::Limit)?;
    for index in 0..count {
        let sampled = index >= request.prompt.len();
        let position = owner.generation_position()?;
        let products = match owner.generation_estimate(1).and_then(DecoderWork::scalar_products) {
            Ok(products) => products,
            Err(error) => { report.finish = GenerationFinish::Failed(error); report.last_review = None; break; }
        };
        let sampling = if sampled { vocabulary } else { 0 };
        // Check the entire compound admission before charging EITHER budget.
        if products > request.budget.scalar_products - report.work.admitted_scalar_products
            || sampling > request.budget.sampling_entries - report.work.admitted_sampling_entries
        { report.finish = GenerationFinish::BudgetExhausted; break; }
        report.work.admitted_scalar_products += products;
        report.work.admitted_sampling_entries += sampling;
        if sampled { report.work.attempted_samples += 1; }
        let budget = DecoderBudget { scalar_products: products };
        let result = if sampled {
            owner.generation_sampled(position, SampleBudget {
                decoder: budget, sampling: SamplingBudget { vocabulary: vocabulary as usize },
            })
        } else { owner.generation_forced(position, request.prompt[index], budget) };
        report.end_position = owner.generation_position()?;
        match result {
            Err(error) => {
                report.finish = GenerationFinish::Failed(error);
                report.last_review = None;
                break;
            }
            Ok(MonitoredStep::Held(review)) => {
                report.last_review = Some(review);
                report.finish = GenerationFinish::Held;
                break;
            }
            Ok(MonitoredStep::Released(step)) => {
                let token = step.step().token;
                report.last_review = Some(step.review);
                if sampled {
                    if stops.contains(&token) { report.finish = GenerationFinish::StopToken; break; }
                    report.tokens.push(token);
                } else { report.reviewed_prompt_tokens += 1; }
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::fixture;

    fn request(prompt: &[u32], max_new_tokens: usize) -> GenerationRequest {
        GenerationRequest {
            prompt: prompt.to_vec(), max_new_tokens, stop_tokens: Vec::new(),
            budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS,
                sampling_entries: MAX_SAMPLING_ENTRIES },
        }
    }

    #[test]
    fn completion_matches_original_stepwise_sampler_and_charges_the_whole_request() {
        let (mut run, budget) = fixture();
        let (mut original, _) = fixture();
        let report = run.generate(0, request(&[0], 2)).unwrap();
        original.advance_forced(0, 0, budget.decoder).unwrap();
        let mut expected = Vec::new();
        for position in 1..3 {
            let step = original.advance_sampled(position, budget).unwrap().into_monitored();
            let MonitoredStep::Released(step) = step else { panic!("quiet control"); };
            expected.push(step.step().token);
        }
        assert_eq!(report.tokens(), expected.as_slice());
        assert_eq!(report.finish(), GenerationFinish::TokenLimit);
        assert_eq!(report.reviewed_prompt_tokens(), 1);
        assert_eq!(report.start_position(), 0);
        assert_eq!(report.end_position(), 3);
        assert_eq!(run.sampled_draws(), 2);
        assert_eq!(report.work().attempted_samples, 2);
        assert_eq!(report.work().admitted_sampling_entries, 4);
        assert_eq!(report.work().admitted_scalar_products, run.decoder_work().scalar_products().unwrap());
        assert_eq!(run.decoder_work(), original.decoder_work());
        assert_eq!(run.monitoring_work(), original.monitoring_work());
        assert_eq!(report.last_review().unwrap().position(), 2);
    }

    #[test]
    fn stop_token_is_reviewed_and_charged_but_never_returned_as_output() {
        let (mut run, _) = fixture();
        let mut input = request(&[0], 3);
        input.stop_tokens = vec![0, 1];
        let report = run.generate(0, input).unwrap();
        assert_eq!(report.finish(), GenerationFinish::StopToken);
        assert!(report.tokens().is_empty());
        assert_eq!(report.end_position(), 2);
        assert_eq!(report.reviewed_prompt_tokens(), 1);
        assert_eq!(report.work().attempted_samples, 1);
        assert_eq!(run.sampled_draws(), 1);
        assert_eq!(run.monitoring_work().frame_reviews, 2);
        assert_eq!(run.status(), MonitoringStatus::Ready);
    }

    #[test]
    fn invalid_prompt_tail_stop_set_context_and_predecessor_are_atomic_refusals() {
        let (mut run, _) = fixture();
        let mut invalid_stop = request(&[0], 1);
        invalid_stop.stop_tokens = vec![2];
        let mut duplicate_stop = request(&[0], 1);
        duplicate_stop.stop_tokens = vec![0, 0];
        for (position, input, error) in [
            (0, request(&[0, 2], 1), Error::InvalidInput),
            (0, invalid_stop, Error::InvalidInput),
            (0, duplicate_stop, Error::Duplicate),
            (0, request(&[0], 4), Error::Limit),
            (1, request(&[0], 1), Error::Stale),
            (0, request(&[], 1), Error::InvalidInput),
        ] {
            assert_eq!(run.generate(position, input).unwrap_err(), error);
            assert_eq!(run.position(), 0);
            assert_eq!(run.sampled_draws(), 0);
            assert_eq!(run.decoder_work(), DecoderWork::default());
            assert_eq!(run.status(), MonitoringStatus::Ready);
        }
        assert_eq!(run.generate(0, request(&[0], 1)).unwrap().tokens().len(), 1);
    }

    #[test]
    fn product_budget_is_shared_by_prefill_and_sampling_not_reset_per_step() {
        let (mut run, _) = fixture();
        let prefix_products = run.estimate(1).unwrap().scalar_products().unwrap();
        let mut input = request(&[0], 2);
        input.budget.scalar_products = prefix_products;
        let report = run.generate(0, input).unwrap();
        assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
        assert_eq!(report.reviewed_prompt_tokens(), 1);
        assert!(report.tokens().is_empty());
        assert_eq!(report.work().admitted_scalar_products, prefix_products);
        assert_eq!(report.work().admitted_sampling_entries, 0);
        assert_eq!(run.position(), 1);
        assert_eq!(run.sampled_draws(), 0);
        // A new explicitly budgeted request may continue the quiet prefix.
        assert_eq!(run.generate(1, request(&[], 1)).unwrap().tokens().len(), 1);
    }

    #[test]
    fn sampling_budget_checks_the_entire_admission_before_charging_decoder_work() {
        for allowance in [1, 2] {
            let (mut run, budget) = fixture();
            run.advance_forced(0, 0, budget.decoder).unwrap();
            let before = run.decoder_work();
            let mut input = request(&[], 1);
            input.budget.sampling_entries = allowance;
            let report = run.generate(1, input).unwrap();
            if allowance == 1 {
                assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
                assert_eq!(report.work(), GenerationWork::default());
                assert_eq!(run.decoder_work(), before);
                assert_eq!(run.sampled_draws(), 0);
            } else {
                assert_eq!(report.finish(), GenerationFinish::TokenLimit);
                assert_eq!(report.tokens().len(), 1);
                assert_eq!(report.work().admitted_sampling_entries, 2);
                assert_eq!(run.sampled_draws(), 1);
            }
        }
    }

    #[test]
    fn a_held_sample_retains_its_draw_and_cannot_be_rerolled_or_exposed() {
        let (mut run, budget) = fixture();
        run.advance_forced(0, 0, budget.decoder).unwrap();
        let used = run.monitoring_work();
        run.monitored.budget = super::super::RefinementBudget {
            encoded_bytes: used.encoded_bytes, probe_coordinates: used.probe_coordinates,
        };
        let report = run.generate(1, request(&[], 2)).unwrap();
        assert_eq!(report.finish(), GenerationFinish::Held);
        assert!(report.tokens().is_empty());
        assert_eq!(report.end_position(), 2);
        assert_eq!(report.work().attempted_samples, 1);
        assert!(report.work().admitted_scalar_products > 0);
        assert_eq!(run.sampled_draws(), 1);
        assert_eq!(run.status(), MonitoringStatus::Held);
        assert_eq!(run.generate(2, request(&[], 1)).unwrap_err(), Error::WrongState);
        assert_eq!(run.sampled_draws(), 1);
    }

    #[test]
    fn post_compute_error_returns_partial_accounting_without_a_token_or_old_quiet_review() {
        let (mut run, budget) = fixture();
        run.advance_forced(0, 0, budget.decoder).unwrap();
        run.monitored.work.frame_reviews = u64::MAX;
        let report = run.generate(1, request(&[], 2)).unwrap();
        assert_eq!(report.finish(), GenerationFinish::Failed(Error::Overflow));
        assert!(report.tokens().is_empty());
        assert!(report.last_review().is_none());
        assert_eq!(report.end_position(), 2);
        assert!(report.work().admitted_scalar_products > 0);
        assert_eq!(report.work().attempted_samples, 1);
        assert_eq!(run.sampled_draws(), 1);
        assert_eq!(run.status(), MonitoringStatus::Failed(Error::Overflow));
    }

    #[test]
    fn prefill_only_reviews_stop_ids_as_prompt_and_allows_later_continuation() {
        let (mut run, _) = fixture();
        let mut input = request(&[0, 1], 0);
        input.stop_tokens = vec![0, 1];
        let report = run.generate(0, input).unwrap();
        assert_eq!(report.finish(), GenerationFinish::TokenLimit);
        assert_eq!(report.reviewed_prompt_tokens(), 2);
        assert_eq!(report.work().attempted_samples, 0);
        assert!(report.tokens().is_empty());
        assert_eq!(run.position(), 2);
        assert_eq!(run.sampled_draws(), 0);
        assert_eq!(run.generate(2, request(&[], 2)).unwrap().tokens().len(), 2);
    }

    #[test]
    fn full_prompt_product_boundary_refuses_before_any_quiet_partial_prefix() {
        // Two fixture tokens need 32 matrix products each, then 4 and 8
        // attention products: 76 total, independently of the admission helper.
        for allowance in [75, 76] {
            let (mut run, _) = fixture();
            assert_eq!(run.estimate(2).unwrap().scalar_products().unwrap(), 76);
            let mut input = request(&[0, 1], 1);
            input.budget.scalar_products = allowance;
            let result = run.generate(0, input);
            if allowance == 75 {
                assert_eq!(result.unwrap_err(), Error::Limit);
                assert_eq!(run.position(), 0);
                assert_eq!(run.decoder_work(), DecoderWork::default());
                assert_eq!(run.monitoring_work().frame_reviews, 0);
            } else {
                let report = result.unwrap();
                assert_eq!(report.finish(), GenerationFinish::BudgetExhausted);
                assert_eq!(report.requested_prompt_tokens(), 2);
                assert_eq!(report.reviewed_prompt_tokens(), 2);
                assert_eq!(report.work().admitted_scalar_products, 76);
                assert_eq!(run.position(), 2);
            }
            assert_eq!(run.sampled_draws(), 0);
        }
    }
}
