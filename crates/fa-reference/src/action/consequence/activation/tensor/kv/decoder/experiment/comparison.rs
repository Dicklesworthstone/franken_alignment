//! Paired full-decoder rollouts with complete admission before either arm runs.
//! Both arms consume the same first token; later forcing and feedback are explicit.

use super::{DecoderExperimentArm, DecoderIntervention};
use super::super::{DecoderBudget, DecoderWork};
use crate::Error;
use std::fmt;
use std::rc::Rc;

pub const MAX_COMPARISON_LOGIT_VALUES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecoderComparisonBudget {
    /// Combined matrix/attention product terms for BOTH full continuations.
    pub scalar_products: u64,
    /// Combined retained f32 logit coordinates, not bytes or peak memory.
    pub retained_logit_values: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderContinuationPolicy { TeacherForced, GreedyAfterFirstToken }

#[derive(Clone)]
pub struct DecoderContinuationStep {
    pub position: u64,
    pub control_token: u32,
    pub intervention_token: u32,
    /// Choices at position + 1, including a diagnostic unconsumed final choice.
    pub control_next_token: u32,
    pub intervention_next_token: u32,
    pub control_logits: Rc<[f32]>,
    pub intervention_logits: Rc<[f32]>,
    pub changed_logit_words: usize,
    pub max_abs_logit_delta: f64,
    pub l2_logit_delta: f64,
}
impl fmt::Debug for DecoderContinuationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecoderContinuationStep").field("position", &self.position)
            .field("control_token", &self.control_token).field("intervention_token", &self.intervention_token)
            .field("changed_logit_words", &self.changed_logit_words).finish_non_exhaustive()
    }
}

/// Complete numerical comparison retaining the exact source/intervention plan.
/// These are descriptive outcomes, not causal necessity, calibrated judgments,
/// authenticated host receipts, release certificates or permission to act.
#[derive(Clone, Debug)]
pub struct DecoderContinuationComparison {
    plan: DecoderIntervention,
    policy: DecoderContinuationPolicy,
    steps: Vec<DecoderContinuationStep>,
    work: DecoderWork,
    retained_logit_values: usize,
    first_different_logits: Option<u64>,
    first_different_consumed_token: Option<u64>,
    first_different_next_choice: Option<u64>,
}
impl DecoderContinuationComparison {
    pub fn plan(&self) -> &DecoderIntervention { &self.plan }
    pub fn policy(&self) -> DecoderContinuationPolicy { self.policy }
    pub fn steps(&self) -> &[DecoderContinuationStep] { &self.steps }
    pub fn work(&self) -> DecoderWork { self.work }
    pub fn retained_logit_values(&self) -> usize { self.retained_logit_values }
    /// Position of the consumed token whose following logit vector first differs.
    pub fn first_different_logits(&self) -> Option<u64> { self.first_different_logits }
    pub fn first_different_consumed_token(&self) -> Option<u64> { self.first_different_consumed_token }
    /// Absolute position of a differing next choice; it may be outside the
    /// consumed horizon, and teacher forcing need not consume either choice.
    pub fn first_different_next_choice(&self) -> Option<u64> { self.first_different_next_choice }
}
impl DecoderIntervention {
    /// Consume identical ORIGINAL tokens in both arms. Every ID and the whole
    /// combined budget are checked before computing even the control's first step.
    pub fn compare_forced(
        &self, tokens: &[u32], budget: DecoderComparisonBudget,
    ) -> Result<DecoderContinuationComparison, Error> {
        self.compare_continuations(tokens, tokens.len(), DecoderContinuationPolicy::TeacherForced, budget)
    }

    /// steps includes the common first token. Later inputs are each arm's own
    /// greedy choices, so differences include subsequent token-feedback effects.
    /// Fixed horizon: no inferred EOS, stop strings, sampling or early exit.
    pub fn compare_greedy(
        &self, first_token: u32, steps: usize, budget: DecoderComparisonBudget,
    ) -> Result<DecoderContinuationComparison, Error> {
        self.compare_continuations(&[first_token], steps, DecoderContinuationPolicy::GreedyAfterFirstToken, budget)
    }

    fn compare_continuations(
        &self, tokens: &[u32], count: usize, policy: DecoderContinuationPolicy, budget: DecoderComparisonBudget,
    ) -> Result<DecoderContinuationComparison, Error> {
        if count == 0 || tokens.is_empty() { return Err(Error::InvalidInput); }
        let model = self.source().model();
        let start = self.source().tokens().len();
        let one_arm = model.estimate(start, count)?;
        let expected_work = one_arm.add(one_arm)?;
        expected_work.check(DecoderBudget { scalar_products: budget.scalar_products })?;
        let retained_logit_values = count.checked_mul(model.profile().shape().vocabulary)
            .and_then(|n| n.checked_mul(2)).ok_or(Error::Overflow)?;
        if budget.retained_logit_values > MAX_COMPARISON_LOGIT_VALUES
            || retained_logit_values > budget.retained_logit_values { return Err(Error::Limit); }
        if tokens.iter().any(|token| *token as usize >= model.profile().shape().vocabulary) {
            return Err(Error::InvalidInput);
        }
        let mut steps = Vec::new();
        steps.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let mut control = self.session(DecoderExperimentArm::Control);
        let mut intervention = self.session(DecoderExperimentArm::Intervention);
        let mut first_different_logits = None;
        let mut first_different_consumed_token = None;
        let mut first_different_next_choice = None;
        for offset in 0..count {
            let position = (start + offset) as u64;
            let (left, right) = if policy == DecoderContinuationPolicy::TeacherForced {
                (tokens[offset], tokens[offset])
            } else if offset == 0 { (tokens[0], tokens[0]) }
            else { (control.greedy_token()?, intervention.greedy_token()?) };
            let scalar_products = model.estimate(start + offset, 1)?.scalar_products()?;
            let a = control.advance(position, left, DecoderBudget { scalar_products })?;
            let b = intervention.advance(position, right, DecoderBudget { scalar_products })?;
            let (changed_logit_words, max_abs_logit_delta, l2_logit_delta) = contrast(&a.logits, &b.logits)?;
            let control_next_token = control.greedy_token()?;
            let intervention_next_token = intervention.greedy_token()?;
            if changed_logit_words > 0 && first_different_logits.is_none() { first_different_logits = Some(position); }
            if left != right && first_different_consumed_token.is_none() { first_different_consumed_token = Some(position); }
            if control_next_token != intervention_next_token && first_different_next_choice.is_none() {
                first_different_next_choice = Some(position + 1);
            }
            steps.push(DecoderContinuationStep {
                position, control_token: left, intervention_token: right, control_next_token, intervention_next_token,
                control_logits: a.logits, intervention_logits: b.logits,
                changed_logit_words, max_abs_logit_delta, l2_logit_delta,
            });
        }
        let work = control.work().add(intervention.work())?;
        if work != expected_work { return Err(Error::Binding); }
        Ok(DecoderContinuationComparison { plan: self.clone(), policy, steps, work, retained_logit_values,
            first_different_logits, first_different_consumed_token, first_different_next_choice })
    }
}

fn contrast(left: &[f32], right: &[f32]) -> Result<(usize, f64, f64), Error> {
    if left.is_empty() || left.len() != right.len() { return Err(Error::Binding); }
    let mut changed = 0;
    let mut maximum = 0.0_f64;
    let mut squared = 0.0_f64;
    for (a, b) in left.iter().zip(right) {
        if !a.is_finite() || !b.is_finite() { return Err(Error::InvalidInput); }
        changed += usize::from(a.to_bits() != b.to_bits());
        let delta = f64::from(*b) - f64::from(*a);
        maximum = maximum.max(delta.abs());
        squared += delta * delta;
    }
    if !squared.is_finite() { return Err(Error::Overflow); }
    Ok((changed, maximum, squared.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contrast_distinguishes_word_equality_from_numerical_distance() {
        assert_eq!(contrast(&[0.0, 1.0], &[-0.0, 1.0]).unwrap(), (1, 0.0, 0.0));
        assert_eq!(contrast(&[0.0, 0.0], &[3.0, 4.0]).unwrap(), (2, 4.0, 5.0));
        assert_eq!(contrast(&[1.0], &[1.0, 2.0]), Err(Error::Binding));
        assert_eq!(contrast(&[f32::NAN], &[1.0]), Err(Error::InvalidInput));
        assert!(contrast(&[f32::MAX], &[-f32::MAX]).unwrap().2.is_finite());
    }
}
