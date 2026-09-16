//! Cooperative paired experiments using the original numerical sessions.
//! One advance computes at most one arm-token. Partial pairs never form a report.

use super::{DecoderComparisonBudget, DecoderContinuationComparison, DecoderContinuationPolicy,
    DecoderContinuationStep, DecoderIntervention, DecoderWork, contrast};
use super::super::{DecoderExperimentArm, DecoderExperimentSession, DecoderExperimentStep};
use super::super::super::DecoderBudget;
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderComparisonStatus { Running, Complete, Cancelled, Failed(Error) }

/// Entered terms are upper bounds for whole attempted tokens, not measured FLOPs.
/// Completed work comes from the original sessions, including an unpaired arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderComparisonWork {
    pub planned: DecoderWork,
    pub entered_tokens: u64,
    pub entered_products: u64,
    pub completed: DecoderWork,
    pub completed_pairs: usize,
    pub retained_logit_values: usize,
}

/// Immutable intervention, token policy, horizon and complete budget. There is
/// no arm/session getter, reseed, budget extension or reset after cancellation.
/// A numerical failure (or caught unwind) is terminal; use work() to retain its
/// cost rather than calling a completed subset a successful experiment.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::cursor::DecoderComparisonCursor;
/// fn duplicate(cursor: DecoderComparisonCursor) { let _ = cursor.clone(); }
/// ```
#[derive(Debug)]
pub struct DecoderComparisonCursor {
    plan: DecoderIntervention,
    policy: DecoderContinuationPolicy,
    tokens: Vec<u32>,
    count: usize,
    planned: DecoderWork,
    retained_limit: usize,
    control: DecoderExperimentSession,
    intervention: DecoderExperimentSession,
    pending: Option<DecoderExperimentStep>,
    pairs: Vec<DecoderContinuationStep>,
    entered_tokens: u64,
    entered_products: u64,
    status: DecoderComparisonStatus,
}

impl DecoderIntervention {
    pub fn begin_forced_comparison(&self, tokens: &[u32], budget: DecoderComparisonBudget)
        -> Result<DecoderComparisonCursor, Error>
    {
        DecoderComparisonCursor::new(self, tokens, tokens.len(), DecoderContinuationPolicy::TeacherForced, budget)
    }

    /// The first token is common; each arm subsequently consumes its own greedy
    /// choice. The unconsumed last next-choice is diagnostic, not an extra token.
    pub fn begin_greedy_comparison(&self, first_token: u32, steps: usize, budget: DecoderComparisonBudget)
        -> Result<DecoderComparisonCursor, Error>
    {
        DecoderComparisonCursor::new(self, &[first_token], steps,
            DecoderContinuationPolicy::GreedyAfterFirstToken, budget)
    }
}

impl DecoderComparisonCursor {
    fn new(plan: &DecoderIntervention, tokens: &[u32], count: usize,
        policy: DecoderContinuationPolicy, budget: DecoderComparisonBudget) -> Result<Self, Error>
    {
        let (planned, retained_limit) = plan.comparison_admission(tokens, count, budget)?;
        let mut retained_tokens = Vec::new();
        retained_tokens.try_reserve_exact(tokens.len()).map_err(|_| Error::Limit)?;
        retained_tokens.extend_from_slice(tokens);
        let mut pairs = Vec::new();
        pairs.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        Ok(Self { plan: plan.clone(), policy, tokens: retained_tokens, count, planned, retained_limit,
            control: plan.session(DecoderExperimentArm::Control),
            intervention: plan.session(DecoderExperimentArm::Intervention), pending: None, pairs,
            entered_tokens: 0, entered_products: 0, status: DecoderComparisonStatus::Running })
    }

    pub fn status(&self) -> DecoderComparisonStatus { self.status }
    pub fn horizon(&self) -> usize { self.count }
    /// Explicitly partial evidence. These pairs cover only the completed prefix.
    pub fn completed_pairs(&self) -> &[DecoderContinuationStep] { &self.pairs }

    pub fn work(&self) -> Result<DecoderComparisonWork, Error> {
        let completed = self.control.work().add(self.intervention.work())?;
        let retained_logit_values = usize::try_from(completed.tokens).map_err(|_| Error::Overflow)?
            .checked_mul(self.plan.source().model().profile().shape().vocabulary).ok_or(Error::Overflow)?;
        Ok(DecoderComparisonWork { planned: self.planned, entered_tokens: self.entered_tokens,
            entered_products: self.entered_products, completed, completed_pairs: self.pairs.len(),
            retained_logit_values })
    }

    /// Cooperative numerical boundary, not preemptive cancellation or a latency
    /// bound. Completion is idempotent; failed/cancelled cursors cannot reenter.
    pub fn advance(&mut self) -> Result<DecoderComparisonStatus, Error> {
        match self.status {
            DecoderComparisonStatus::Complete => return Ok(self.status),
            DecoderComparisonStatus::Running => {}
            _ => return Err(Error::WrongState),
        }
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

    /// Only a full, successful horizon can become the original comparison type.
    /// Repeated finish clones observation data; it does not rerun either arm.
    pub fn finish(&self) -> Result<DecoderContinuationComparison, Error> {
        if self.status != DecoderComparisonStatus::Complete { return Err(Error::Incomplete); }
        let work = self.work()?;
        if work.completed != self.planned || self.pairs.len() != self.count || self.pending.is_some()
            || work.retained_logit_values != self.retained_limit { return Err(Error::Binding); }
        Ok(DecoderContinuationComparison {
            plan: self.plan.clone(), policy: self.policy, steps: self.pairs.clone(), work: work.completed,
            retained_logit_values: work.retained_logit_values,
            first_different_logits: self.pairs.iter().find(|s| s.changed_logit_words > 0).map(|s| s.position),
            first_different_consumed_token: self.pairs.iter().find(|s| s.control_token != s.intervention_token).map(|s| s.position),
            first_different_next_choice: self.pairs.iter().find(|s| s.control_next_token != s.intervention_next_token).map(|s| s.position + 1),
        })
    }

    fn advance_inner(&mut self) -> Result<bool, Error> {
        let offset = self.pairs.len();
        let start = self.plan.source().tokens().len();
        let position = (start + offset) as u64;
        let left_arm = self.pending.is_none();
        let token = if self.policy == DecoderContinuationPolicy::TeacherForced { self.tokens[offset] }
            else if offset == 0 { self.tokens[0] }
            else if left_arm { self.control.greedy_token()? } else { self.intervention.greedy_token()? };
        let products = self.plan.source().model().estimate(start + offset, 1)?.scalar_products()?;
        self.entered_tokens = self.entered_tokens.checked_add(1).ok_or(Error::Overflow)?;
        self.entered_products = self.entered_products.checked_add(products).ok_or(Error::Overflow)?;
        let budget = DecoderBudget { scalar_products: products };
        if left_arm {
            self.pending = Some(self.control.advance(position, token, budget)?);
            return Ok(false);
        }
        let b = self.intervention.advance(position, token, budget)?;
        let a = self.pending.take().ok_or(Error::Incomplete)?;
        let (changed_logit_words, max_abs_logit_delta, l2_logit_delta) = contrast(&a.logits, &b.logits)?;
        self.pairs.push(DecoderContinuationStep {
            position, control_token: a.token, intervention_token: b.token,
            control_next_token: self.control.greedy_token()?, intervention_next_token: self.intervention.greedy_token()?,
            control_logits: a.logits, intervention_logits: b.logits,
            changed_logit_words, max_abs_logit_delta, l2_logit_delta,
        });
        if self.pairs.len() == self.count {
            let work = self.work()?;
            if work.completed != self.planned || work.retained_logit_values != self.retained_limit {
                return Err(Error::Binding);
            }
            return Ok(true);
        }
        Ok(false)
    }
}
