//! Paired original-engine continuations over an independently fitted codebook.
//! The entire encoding and both inference arms are admitted before either runs.
use super::{LearnedDecoder, LearnedDecoderBudget, DecoderBudget, DecoderCheckpoint,
    DecoderWork, reconstruction_products, MAX_RECONSTRUCTION_PRODUCTS};
use super::super::{DecoderExperimentArm, comparison::{contrast, DecoderContinuationPolicy,
    DecoderContinuationStep, MAX_COMPARISON_LOGIT_VALUES}};
use super::super::super::super::model::learned::{CompressionBudget, LearnedKvCodec};
use crate::Error;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedComparisonBudget {
    pub compression: CompressionBudget,
    /// Combined original decoder matrix/attention products for BOTH arms.
    pub scalar_products: u64,
    /// Additional learned-prefix reconstruction products for the treatment.
    pub reconstruction_products: u64,
    pub retained_logit_values: usize,
}

/// Deliberately retains the original checkpoint for reproducible comparison.
/// Its total memory is NOT the compact image size. The independently retained
/// LearnedDecoder can outlive this comparison without keeping the original KV.
#[derive(Clone, Debug)]
pub struct LearnedComparison {
    source: DecoderCheckpoint,
    learned: LearnedDecoder,
    policy: DecoderContinuationPolicy,
    steps: Vec<DecoderContinuationStep>,
    work: DecoderWork,
    reconstruction_products: u64,
    reserved_reconstruction_products: u64,
    first_different_logits: Option<u64>,
    first_different_consumed_token: Option<u64>,
    first_different_next_choice: Option<u64>,
}
impl LearnedComparison {
    pub fn source(&self) -> &DecoderCheckpoint { &self.source }
    pub fn learned(&self) -> &LearnedDecoder { &self.learned }
    pub fn policy(&self) -> DecoderContinuationPolicy { self.policy }
    pub fn steps(&self) -> &[DecoderContinuationStep] { &self.steps }
    pub fn work(&self) -> DecoderWork { self.work }
    pub fn reconstruction_products(&self) -> u64 { self.reconstruction_products }
    pub fn reserved_reconstruction_products(&self) -> u64 { self.reserved_reconstruction_products }
    pub fn first_different_logits(&self) -> Option<u64> { self.first_different_logits }
    pub fn first_different_consumed_token(&self) -> Option<u64> { self.first_different_consumed_token }
    /// This is position + 1 and can lie OUTSIDE the consumed horizon. A changed
    /// diagnostic next choice is not falsely counted as an executed divergence.
    pub fn first_different_next_choice(&self) -> Option<u64> { self.first_different_next_choice }
}

impl DecoderCheckpoint {
    /// No fitting or threshold/rank tuning occurs here. The evaluation origin
    /// and checkpoint stream must be absent from the retained training corpus.
    pub fn compare_learned_forced(&self, id: u64, evaluation_origin: u64, codec: &LearnedKvCodec,
        tokens: &[u32], budget: LearnedComparisonBudget) -> Result<LearnedComparison, Error>
    {
        self.compare_learned(id, evaluation_origin, codec, tokens, tokens.len(),
            DecoderContinuationPolicy::TeacherForced, budget)
    }

    /// Both arms consume the explicit first token. Later tokens follow their
    /// own newly computed greedy outputs. No old logits, inferred EOS, stochastic
    /// state, automatic early stopping or recompression of suffix rows is used.
    pub fn compare_learned_greedy(&self, id: u64, evaluation_origin: u64, codec: &LearnedKvCodec,
        first_token: u32, count: usize, budget: LearnedComparisonBudget) -> Result<LearnedComparison, Error>
    {
        self.compare_learned(id, evaluation_origin, codec, &[first_token], count,
            DecoderContinuationPolicy::GreedyAfterFirstToken, budget)
    }

    fn compare_learned(&self, id: u64, evaluation_origin: u64, codec: &LearnedKvCodec,
        tokens: &[u32], count: usize, policy: DecoderContinuationPolicy, budget: LearnedComparisonBudget)
        -> Result<LearnedComparison, Error>
    {
        if id == 0 || count == 0 || tokens.is_empty() { return Err(Error::InvalidInput); }
        let model = self.model(); let start = self.tokens().len();
        let one = model.estimate(start, count)?;
        let expected_work = one.add(one)?;
        expected_work.check(DecoderBudget { scalar_products: budget.scalar_products })?;
        let reconstruction = reconstruction_products(model, start, codec.policy().rank(), count)?;
        if budget.reconstruction_products > MAX_RECONSTRUCTION_PRODUCTS || reconstruction > budget.reconstruction_products {
            return Err(Error::Limit);
        }
        let retained = count.checked_mul(model.profile().shape().vocabulary)
            .and_then(|n| n.checked_mul(2)).ok_or(Error::Overflow)?;
        if budget.retained_logit_values > MAX_COMPARISON_LOGIT_VALUES || retained > budget.retained_logit_values {
            return Err(Error::Limit);
        }
        if tokens.iter().any(|token| *token as usize >= model.profile().shape().vocabulary) { return Err(Error::InvalidInput); }
        let mut steps = Vec::new(); steps.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        // Compression performs its own complete profile/value/byte/work admission
        // before any scalar access. It cannot return a partially usable prefix.
        let learned = self.learned_experiment(id, evaluation_origin, codec, budget.compression)?;
        let original = self.intervene(id, BTreeMap::new(), 0)?;
        let mut control = original.session(DecoderExperimentArm::Control);
        let mut treatment = learned.session();
        let per_step_reconstruction = reconstruction_products(model, start, codec.policy().rank(), 1)?;
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
            let b = treatment.advance(position, right, LearnedDecoderBudget {
                decoder: allowance, reconstruction_products: per_step_reconstruction,
            })?;
            let (changed_logit_words, max_abs_logit_delta, l2_logit_delta) = contrast(&a.logits, &b.logits)?;
            let control_next_token = control.greedy_token()?;
            let intervention_next_token = treatment.greedy_token()?;
            if changed_logit_words != 0 && first_different_logits.is_none() { first_different_logits = Some(position); }
            if left != right && first_different_consumed_token.is_none() { first_different_consumed_token = Some(position); }
            if control_next_token != intervention_next_token && first_different_next_choice.is_none() {
                first_different_next_choice = Some(position + 1);
            }
            steps.push(DecoderContinuationStep { position, control_token: left, intervention_token: right,
                control_next_token, intervention_next_token, control_logits: a.logits, intervention_logits: b.logits,
                changed_logit_words, max_abs_logit_delta, l2_logit_delta });
        }
        let work = control.work().add(treatment.work())?;
        if work != expected_work || treatment.reconstruction_products() != reconstruction { return Err(Error::Binding); }
        Ok(LearnedComparison { source: self.clone(), learned, policy, steps, work,
            reconstruction_products: treatment.reconstruction_products(), reserved_reconstruction_products: reconstruction,
            first_different_logits, first_different_consumed_token, first_different_next_choice })
    }
}
