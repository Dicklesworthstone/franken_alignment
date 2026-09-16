//! Paired stochastic counterfactuals using the ORIGINAL sampler and decoder.
//! Both arms use a separately supplied experimental seed, never the actor's RNG.

use super::{DecoderComparisonBudget, DecoderIntervention, MAX_COMPARISON_LOGIT_VALUES, contrast};
use super::cursor::{DecoderComparisonStatus, DecoderComparisonWork};
use super::super::{DecoderExperimentArm, DecoderExperimentSession, DecoderExperimentStep};
use super::super::super::{DecoderBudget, DecoderWork};
use super::super::super::sampling::{SampledToken, Sampler, SamplingBudget, SamplingStart, SamplingWork};
use crate::Error;

/// Complete paired horizon, including the common forced first token. Sampling
/// logits counts BOTH arms over the remaining horizon, not a per-call allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderSampledComparisonBudget {
    pub comparison: DecoderComparisonBudget,
    pub sampling_logits: usize,
}

/// Only completed arm-pairs. None choices denote the common forced first token;
/// all later choices are actual samples consumed by the corresponding decoder.
/// There is no extra, unconsumed final draw disguised as a generated token.
#[derive(Clone, Debug)]
pub struct DecoderSampledPair {
    pub control: DecoderExperimentStep,
    pub intervention: DecoderExperimentStep,
    pub control_choice: Option<SampledToken>,
    pub intervention_choice: Option<SampledToken>,
    pub changed_logit_words: usize,
    pub max_abs_logit_delta: f64,
    pub l2_logit_delta: f64,
}

/// Failed/cancelled runs retain completed sampling even if its selected token's
/// numerical computation fails. Entered counters include an interrupted attempt.
/// These are logical work counts, not measured FLOPs, latency or peak allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderSampledComparisonWork {
    pub numerical: DecoderComparisonWork,
    pub planned_sampling_logits: usize,
    pub entered_sampling_calls: u64,
    pub entered_sampling_logits: usize,
    pub sampling: SamplingWork,
    pub control_draws: u64,
    pub intervention_draws: u64,
}

/// Experimental outcomes only; identical random words couple the two trajectories
/// but neither establish statistical significance nor import a live actor's RNG.
#[derive(Clone, Debug)]
pub struct DecoderSampledComparison {
    plan: DecoderIntervention,
    sampling: SamplingStart,
    first_token: u32,
    steps: Vec<DecoderSampledPair>,
    work: DecoderSampledComparisonWork,
}
impl DecoderSampledComparison {
    pub fn plan(&self) -> &DecoderIntervention { &self.plan }
    pub fn sampling_start(&self) -> &SamplingStart { &self.sampling }
    pub fn first_token(&self) -> u32 { self.first_token }
    pub fn steps(&self) -> &[DecoderSampledPair] { &self.steps }
    pub fn work(&self) -> DecoderSampledComparisonWork { self.work }
    pub fn first_different_logits(&self) -> Option<u64> {
        self.steps.iter().find(|pair| pair.changed_logit_words != 0).map(|pair| pair.control.position)
    }
    pub fn first_different_consumed_token(&self) -> Option<u64> {
        self.steps.iter().find(|pair| pair.control.token != pair.intervention.token).map(|pair| pair.control.position)
    }
}

/// One arm-token per advance, with fixed policy, seed, edits, horizon and budgets.
/// A caught unwind or failure is terminal. Sampling is experimental work: a draw
/// whose following computation fails is retained, never automatically rerolled.
/// No mutable sampler/session getter, reseeding or continuation-mode switch exists.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::sampled::DecoderSampledComparisonCursor;
/// fn duplicate(run: DecoderSampledComparisonCursor) { let _ = run.clone(); }
/// ```
#[derive(Debug)]
pub struct DecoderSampledComparisonCursor {
    plan: DecoderIntervention,
    sampling: SamplingStart,
    first_token: u32,
    count: usize,
    planned: DecoderWork,
    retained_limit: usize,
    planned_sampling_logits: usize,
    control: DecoderExperimentSession,
    intervention: DecoderExperimentSession,
    control_sampler: Sampler,
    intervention_sampler: Sampler,
    pending: Option<(DecoderExperimentStep, Option<SampledToken>)>,
    pairs: Vec<DecoderSampledPair>,
    entered_tokens: u64,
    entered_products: u64,
    entered_sampling_calls: u64,
    entered_sampling_logits: usize,
    sampling_work: SamplingWork,
    status: DecoderComparisonStatus,
}

impl DecoderIntervention {
    /// Start both arms with the SAME explicitly supplied token. Old checkpoint
    /// logits are not valid after a KV edit and are never used to choose it.
    /// Every later choice uses that arm's newly computed logits and the original
    /// temperature/top-k/top-p sampler. Shared experimental random words do not
    /// force shared token choices when the distributions differ.
    pub fn begin_sampled_comparison(&self, first_token: u32, steps: usize,
        sampling: SamplingStart, budget: DecoderSampledComparisonBudget)
        -> Result<DecoderSampledComparisonCursor, Error>
    {
        let (planned, retained_limit) = self.comparison_admission(&[first_token], steps, budget.comparison)?;
        let vocabulary = self.source().model().profile().shape().vocabulary;
        if sampling.policy.vocabulary() != vocabulary { return Err(Error::Binding); }
        let planned_sampling_logits = (steps - 1).checked_mul(2)
            .and_then(|count| count.checked_mul(vocabulary)).ok_or(Error::Limit)?;
        if budget.sampling_logits > MAX_COMPARISON_LOGIT_VALUES
            || planned_sampling_logits > budget.sampling_logits { return Err(Error::Limit); }
        let control_sampler = Sampler::seeded(sampling.policy.clone(), sampling.stream, sampling.seed)?;
        let intervention_sampler = Sampler::seeded(sampling.policy.clone(), sampling.stream, sampling.seed)?;
        let mut pairs = Vec::new();
        pairs.try_reserve_exact(steps).map_err(|_| Error::Limit)?;
        Ok(DecoderSampledComparisonCursor {
            plan: self.clone(), sampling, first_token, count: steps, planned, retained_limit,
            planned_sampling_logits, control: self.session(DecoderExperimentArm::Control),
            intervention: self.session(DecoderExperimentArm::Intervention), control_sampler,
            intervention_sampler, pending: None, pairs, entered_tokens: 0, entered_products: 0,
            entered_sampling_calls: 0, entered_sampling_logits: 0,
            sampling_work: SamplingWork { logits_scanned: 0, exponentials: 0, retained_candidates: 0, zero_weights: 0 },
            status: DecoderComparisonStatus::Running,
        })
    }
}

impl DecoderSampledComparisonCursor {
    pub fn status(&self) -> DecoderComparisonStatus { self.status }
    pub fn horizon(&self) -> usize { self.count }
    pub fn completed_pairs(&self) -> &[DecoderSampledPair] { &self.pairs }
    pub fn work(&self) -> Result<DecoderSampledComparisonWork, Error> {
        let completed = self.control.work().add(self.intervention.work())?;
        let vocabulary = self.plan.source().model().profile().shape().vocabulary;
        let retained_logit_values = usize::try_from(completed.tokens).map_err(|_| Error::Overflow)?
            .checked_mul(vocabulary).ok_or(Error::Overflow)?;
        Ok(DecoderSampledComparisonWork {
            numerical: DecoderComparisonWork { planned: self.planned, entered_tokens: self.entered_tokens,
                entered_products: self.entered_products, completed, completed_pairs: self.pairs.len(), retained_logit_values },
            planned_sampling_logits: self.planned_sampling_logits,
            entered_sampling_calls: self.entered_sampling_calls, entered_sampling_logits: self.entered_sampling_logits,
            sampling: self.sampling_work, control_draws: self.control_sampler.snapshot().draws(),
            intervention_draws: self.intervention_sampler.snapshot().draws(),
        })
    }
    pub fn advance(&mut self) -> Result<DecoderComparisonStatus, Error> {
        match self.status {
            DecoderComparisonStatus::Complete => return Ok(self.status),
            DecoderComparisonStatus::Running => {}
            _ => return Err(Error::WrongState),
        }
        // Sampling and numerical execution can both fail or unwind. Never resume
        // either a selected-but-unconsumed token or a half-finished pair then.
        self.status = DecoderComparisonStatus::Failed(Error::Incomplete);
        match self.advance_inner() {
            Ok(complete) => {
                self.status = if complete { DecoderComparisonStatus::Complete } else { DecoderComparisonStatus::Running };
                Ok(self.status)
            }
            Err(error) => { self.status = DecoderComparisonStatus::Failed(error); Err(error) }
        }
    }
    pub fn cancel(&mut self) -> Result<(), Error> {
        match self.status {
            DecoderComparisonStatus::Running | DecoderComparisonStatus::Cancelled => {
                self.status = DecoderComparisonStatus::Cancelled; Ok(())
            }
            _ => Err(Error::WrongState),
        }
    }
    pub fn finish(&self) -> Result<DecoderSampledComparison, Error> {
        if self.status != DecoderComparisonStatus::Complete { return Err(Error::Incomplete); }
        let work = self.work()?;
        if work.numerical.completed != self.planned || self.pending.is_some() || self.pairs.len() != self.count
            || work.numerical.retained_logit_values != self.retained_limit
            || work.sampling.logits_scanned != self.planned_sampling_logits
            || work.control_draws != (self.count - 1) as u64 || work.intervention_draws != work.control_draws
        { return Err(Error::Binding); }
        Ok(DecoderSampledComparison { plan: self.plan.clone(), sampling: self.sampling.clone(),
            first_token: self.first_token, steps: self.pairs.clone(), work })
    }
    fn advance_inner(&mut self) -> Result<bool, Error> {
        let offset = self.pairs.len();
        let start = self.plan.source().tokens().len();
        let position = (start + offset) as u64;
        let left = self.pending.is_none();
        let choice = if offset == 0 { None } else {
            let vocabulary = self.sampling.policy.vocabulary();
            self.entered_sampling_calls = self.entered_sampling_calls.checked_add(1).ok_or(Error::Overflow)?;
            self.entered_sampling_logits = self.entered_sampling_logits.checked_add(vocabulary).ok_or(Error::Overflow)?;
            let choice = if left {
                self.control_sampler.sample(self.control.logits()?, SamplingBudget { vocabulary })?
            } else {
                self.intervention_sampler.sample(self.intervention.logits()?, SamplingBudget { vocabulary })?
            };
            self.sampling_work = add_sampling(self.sampling_work, choice.work)?;
            Some(choice)
        };
        let token = choice.as_ref().map_or(self.first_token, |choice| choice.token);
        let products = self.plan.source().model().estimate(start + offset, 1)?.scalar_products()?;
        self.entered_tokens = self.entered_tokens.checked_add(1).ok_or(Error::Overflow)?;
        self.entered_products = self.entered_products.checked_add(products).ok_or(Error::Overflow)?;
        let budget = DecoderBudget { scalar_products: products };
        if left {
            self.pending = Some((self.control.advance(position, token, budget)?, choice));
            return Ok(false);
        }
        let b = self.intervention.advance(position, token, budget)?;
        let (a, control_choice) = self.pending.take().ok_or(Error::Incomplete)?;
        match (&control_choice, &choice) {
            (None, None) if offset == 0 => {}
            (Some(a), Some(b)) if a.draw == b.draw && a.stream == b.stream && a.random_word == b.random_word => {}
            _ => return Err(Error::Binding),
        }
        let (changed_logit_words, max_abs_logit_delta, l2_logit_delta) = contrast(&a.logits, &b.logits)?;
        self.pairs.push(DecoderSampledPair { control: a, intervention: b, control_choice,
            intervention_choice: choice, changed_logit_words, max_abs_logit_delta, l2_logit_delta });
        Ok(self.pairs.len() == self.count)
    }
}

fn add_sampling(a: SamplingWork, b: SamplingWork) -> Result<SamplingWork, Error> {
    let add = |a: usize, b: usize| a.checked_add(b).ok_or(Error::Overflow);
    Ok(SamplingWork { logits_scanned: add(a.logits_scanned, b.logits_scanned)?,
        exponentials: add(a.exponentials, b.exponentials)?, retained_candidates: add(a.retained_candidates, b.retained_candidates)?,
        zero_weights: add(a.zero_weights, b.zero_weights)? })
}
