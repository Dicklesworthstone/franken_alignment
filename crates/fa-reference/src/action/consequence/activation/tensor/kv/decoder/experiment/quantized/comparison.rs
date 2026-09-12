//! Paired full-decoder comparisons; encoding and both full arms are preflighted.
use super::{QuantizedDecoder, DecoderBudget, DecoderCheckpoint, DecoderWork};
use super::super::{DecoderExperimentArm, comparison::{DecoderContinuationPolicy, DecoderContinuationStep, MAX_COMPARISON_LOGIT_VALUES}};
use super::super::super::super::model::quantized::{KvQuantization, QuantizationBudget, MAX_QUANTIZED_BYTES};
use super::super::super::super::model::MAX_MODEL_KV_VALUES;
use crate::Error;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantizedComparisonBudget {
    pub quantization: QuantizationBudget,
    pub scalar_products: u64,
    pub retained_logit_values: usize,
}
/// A comparison intentionally retains both baseline and compressed state. Its
/// total memory is not the compressed image size; a standalone QuantizedDecoder
/// can outlive this report without retaining the baseline's full cache.
#[derive(Clone, Debug)]
pub struct QuantizedComparison {
    source: DecoderCheckpoint,
    quantized: QuantizedDecoder,
    policy: DecoderContinuationPolicy,
    steps: Vec<DecoderContinuationStep>,
    work: DecoderWork,
    first_different_logits: Option<u64>,
    first_different_consumed_token: Option<u64>,
    first_different_next_choice: Option<u64>,
}
impl QuantizedComparison {
    pub fn source(&self) -> &DecoderCheckpoint { &self.source }
    pub fn quantized(&self) -> &QuantizedDecoder { &self.quantized }
    pub fn policy(&self) -> DecoderContinuationPolicy { self.policy }
    pub fn steps(&self) -> &[DecoderContinuationStep] { &self.steps }
    pub fn work(&self) -> DecoderWork { self.work }
    pub fn first_different_logits(&self) -> Option<u64> { self.first_different_logits }
    pub fn first_different_consumed_token(&self) -> Option<u64> { self.first_different_consumed_token }
    pub fn first_different_next_choice(&self) -> Option<u64> { self.first_different_next_choice }
}
impl DecoderCheckpoint {
    pub fn compare_quantized_forced(&self, id: u64, codec: KvQuantization, tokens: &[u32],
        budget: QuantizedComparisonBudget) -> Result<QuantizedComparison, Error>
    {
        self.compare_quantized(id, codec, tokens, tokens.len(), DecoderContinuationPolicy::TeacherForced, budget)
    }
    /// The first token is shared. Later inputs follow each arm's own greedy
    /// choice, separating diagnostic choices from actually consumed differences.
    pub fn compare_quantized_greedy(&self, id: u64, codec: KvQuantization, first_token: u32,
        count: usize, budget: QuantizedComparisonBudget) -> Result<QuantizedComparison, Error>
    {
        self.compare_quantized(id, codec, &[first_token], count, DecoderContinuationPolicy::GreedyAfterFirstToken, budget)
    }
    fn compare_quantized(&self, id: u64, codec: KvQuantization, tokens: &[u32], count: usize,
        policy: DecoderContinuationPolicy, budget: QuantizedComparisonBudget) -> Result<QuantizedComparison, Error>
    {
        if id == 0 || count == 0 || tokens.is_empty() { return Err(Error::InvalidInput); }
        let model = self.model(); let start = self.tokens().len();
        let one = model.estimate(start, count)?; let expected_work = one.add(one)?;
        expected_work.check(DecoderBudget { scalar_products: budget.scalar_products })?;
        let retained = count.checked_mul(model.profile().shape().vocabulary)
            .and_then(|n| n.checked_mul(2)).ok_or(Error::Overflow)?;
        if budget.retained_logit_values > MAX_COMPARISON_LOGIT_VALUES || retained > budget.retained_logit_values
            || budget.quantization.values > MAX_MODEL_KV_VALUES || budget.quantization.encoded_bytes > MAX_QUANTIZED_BYTES
            || self.cache().normalized_values() > budget.quantization.values
            || self.cache().quantized_len()? > budget.quantization.encoded_bytes { return Err(Error::Limit); }
        if tokens.iter().any(|token| *token as usize >= model.profile().shape().vocabulary) { return Err(Error::InvalidInput); }
        let mut steps = Vec::new(); steps.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        let quantized = self.quantized_experiment(id, codec, budget.quantization)?;
        let original = self.intervene(id, BTreeMap::new(), 0)?;
        let mut control = original.session(DecoderExperimentArm::Control);
        let mut treatment = quantized.session();
        let mut first_different_logits = None;
        let mut first_different_consumed_token = None;
        let mut first_different_next_choice = None;
        for offset in 0..count {
            let position = (start + offset) as u64;
            let (left, right) = if policy == DecoderContinuationPolicy::TeacherForced { (tokens[offset], tokens[offset]) }
                else if offset == 0 { (tokens[0], tokens[0]) }
                else { (control.greedy_token()?, treatment.greedy_token()?) };
            let allowance = DecoderBudget { scalar_products: model.estimate(start + offset, 1)?.scalar_products()? };
            let a = control.advance(position, left, allowance)?;
            let b = treatment.advance(position, right, allowance)?;
            let mut changed_logit_words = 0;
            let mut max_abs_logit_delta = 0.0_f64; let mut squared = 0.0_f64;
            if a.logits.len() != b.logits.len() { return Err(Error::Binding); }
            for (left, right) in a.logits.iter().zip(b.logits.iter()) {
                changed_logit_words += usize::from(left.to_bits() != right.to_bits());
                let delta = f64::from(*right) - f64::from(*left);
                max_abs_logit_delta = max_abs_logit_delta.max(delta.abs()); squared += delta * delta;
            }
            if !squared.is_finite() { return Err(Error::Overflow); }
            let control_next_token = control.greedy_token()?; let intervention_next_token = treatment.greedy_token()?;
            if changed_logit_words != 0 && first_different_logits.is_none() { first_different_logits = Some(position); }
            if left != right && first_different_consumed_token.is_none() { first_different_consumed_token = Some(position); }
            if control_next_token != intervention_next_token && first_different_next_choice.is_none() {
                first_different_next_choice = Some(position + 1);
            }
            steps.push(DecoderContinuationStep { position, control_token: left, intervention_token: right,
                control_next_token, intervention_next_token, control_logits: a.logits, intervention_logits: b.logits,
                changed_logit_words, max_abs_logit_delta, l2_logit_delta: squared.sqrt() });
        }
        let work = control.work().add(treatment.work())?;
        if work != expected_work { return Err(Error::Binding); }
        Ok(QuantizedComparison { source: self.clone(), quantized, policy, steps, work,
            first_different_logits, first_different_consumed_token, first_different_next_choice })
    }
}
