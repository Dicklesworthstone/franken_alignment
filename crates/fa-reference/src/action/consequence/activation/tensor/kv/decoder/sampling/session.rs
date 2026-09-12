//! Sampling state and computed decoder state advance as one CPU-RAM operation.
//! No new forward engine or cache is introduced. Checkpoints retain exactly
//! which original-token positions consumed a draw, for independent recomputation.

use super::{Sampler, SamplerSnapshot, SampledToken, SamplingBudget, SamplingPolicy};
use super::super::{DecoderBudget, DecoderCheckpoint, DecoderModel, DecoderRestoreBudget,
    DecoderRestoreReceipt, DecoderSession, DecoderStep, DecoderWork};
use super::super::super::model::ModelKvImage;
use crate::Error;
use std::fmt;
use std::rc::Rc;

#[derive(Clone)]
pub struct SamplingStart {
    pub policy: SamplingPolicy,
    pub stream: u64,
    pub seed: u64,
}
impl fmt::Debug for SamplingStart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SamplingStart").field("policy", &self.policy)
            .field("stream", &self.stream).finish_non_exhaustive()
    }
}
impl SamplingStart {
    fn initialize(self, vocabulary: usize) -> Result<Sampler, Error> {
        if self.policy.vocabulary() != vocabulary { return Err(Error::Binding); }
        Sampler::seeded(self.policy, self.stream, self.seed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleBudget {
    pub decoder: DecoderBudget,
    pub sampling: SamplingBudget,
}

#[derive(Clone, Debug)]
pub struct SampledStep {
    pub choice: SampledToken,
    pub computation: DecoderStep,
}

/// No mutable decoder/sampler accessor or seed replacement can split the token
/// from its draw. Forced tokens are explicit and consume no randomness.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::SampledSession;
/// fn bypass(session: &mut SampledSession) { session.decoder_mut(); }
/// ```
#[derive(Debug)]
pub struct SampledSession {
    decoder: DecoderSession,
    sampler: Sampler,
    initial: SamplerSnapshot,
    sampled_positions: Vec<u64>,
}

/// Complete state for this stochastic numerical profile. A caller cannot pair
/// arbitrary imported RNG bytes with an unrelated numeric checkpoint. Cloning
/// this evidence cannot copy a production effect budget, permit or revocation.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::SampledCheckpoint;
/// use fa_reference::action::Permit;
/// fn grant(checkpoint: SampledCheckpoint) -> Permit { checkpoint }
/// ```
#[derive(Clone, Debug)]
pub struct SampledCheckpoint {
    numerical: DecoderCheckpoint,
    initial: SamplerSnapshot,
    sampler: SamplerSnapshot,
    sampled_positions: Rc<[u64]>,
}
impl SampledCheckpoint {
    pub fn numerical(&self) -> &DecoderCheckpoint { &self.numerical }
    pub fn sampler_state(&self) -> &SamplerSnapshot { &self.sampler }
    pub fn sampled_positions(&self) -> &[u64] { &self.sampled_positions }

    fn check(&self, model: &DecoderModel, resumed_stream: u64) -> Result<(), Error> {
        if !Rc::ptr_eq(&model.data, &self.numerical.model().data) { return Err(Error::Binding); }
        if resumed_stream == 0 || resumed_stream == self.numerical.stream() { return Err(Error::InvalidInput); }
        if self.initial.draws() != 0 || self.initial.policy() != self.sampler.policy()
            || self.initial.stream() != self.sampler.stream()
            || self.sampler.policy().vocabulary() != model.profile().shape().vocabulary
            || self.sampler.draws() != self.sampled_positions.len() as u64
            || self.sampled_positions.windows(2).any(|pair| pair[0] >= pair[1])
            || self.sampled_positions.iter().any(|p| *p == 0 || *p >= self.numerical.tokens().len() as u64)
        { return Err(Error::Binding); }
        Ok(())
    }
}

impl DecoderModel {
    pub fn sampled_session(&self, stream: u64, start: SamplingStart) -> Result<SampledSession, Error> {
        let sampler = start.initialize(self.profile().shape().vocabulary)?;
        let initial = sampler.snapshot();
        Ok(SampledSession { decoder: self.session(stream)?, sampler, initial, sampled_positions: Vec::new() })
    }

    /// The complete prefix is teacher-forced original IDs; its recomputation
    /// does not secretly consume samples. All sampling configuration is checked
    /// before invoking the existing complete-prefix product-budget admission.
    pub fn recompute_sampled(&self, stream: u64, tokens: &[u32], budget: DecoderBudget,
        start: SamplingStart) -> Result<SampledSession, Error>
    {
        let sampler = start.initialize(self.profile().shape().vocabulary)?;
        let initial = sampler.snapshot();
        Ok(SampledSession { decoder: self.recompute(stream, tokens, budget)?, sampler,
            initial, sampled_positions: Vec::new() })
    }

    /// Restore the exact recorded next draw alongside the original full cache
    /// restoration. Numerical capture uses a new stream, while the preserved RNG
    /// stream is the same replay lineage, not an invented independent seed.
    pub fn restore_sampled_checkpoint(&self, checkpoint: &SampledCheckpoint, stream: u64,
        budget: DecoderRestoreBudget) -> Result<(SampledSession, DecoderRestoreReceipt), Error>
    {
        checkpoint.check(self, stream)?;
        let sampled_positions = checkpoint.sampled_positions.to_vec();
        let sampler = Sampler::from_snapshot(&checkpoint.sampler);
        let initial = checkpoint.initial.clone();
        let (decoder, receipt) = self.restore_checkpoint(&checkpoint.numerical, stream, budget)?;
        Ok((SampledSession { decoder, sampler, initial, sampled_positions }, receipt))
    }

    /// Recompute from the ORIGINAL IDs, reproducing stochastic choices only at
    /// their recorded positions. Verify every selected ID, the exact ending RNG
    /// state, all KV scalar bits and all final logit bits. This does not trust a
    /// final RNG counter as evidence that the intervening draws were reproduced.
    pub fn recompute_sampled_checkpoint(&self, checkpoint: &SampledCheckpoint, stream: u64,
        budget: SampleBudget) -> Result<SampledSession, Error>
    {
        checkpoint.check(self, stream)?;
        self.estimate(0, checkpoint.numerical.tokens().len())?.check(budget.decoder)?;
        let vocabulary = self.profile().shape().vocabulary;
        if budget.sampling.vocabulary > super::MAX_DECODER_VOCABULARY
            || budget.sampling.vocabulary < vocabulary { return Err(Error::Limit); }
        let mut replay = SampledSession { decoder: self.session(stream)?,
            sampler: Sampler::from_snapshot(&checkpoint.initial), initial: checkpoint.initial.clone(),
            sampled_positions: Vec::new() };
        let mut positions = checkpoint.sampled_positions.iter().copied().peekable();
        for (position, token) in checkpoint.numerical.tokens().iter().copied().enumerate() {
            let position64 = position as u64;
            let decoder = DecoderBudget { scalar_products: self.estimate(position, 1)?.scalar_products()? };
            if positions.peek() == Some(&position64) {
                let actual = replay.advance_sampled(position64, SampleBudget { decoder, sampling: budget.sampling })?;
                if actual.choice.token != token { return Err(Error::Binding); }
                positions.next();
            } else { replay.advance_forced(position64, token, decoder)?; }
        }
        let cache = replay.cache_image()?;
        if replay.sampler_state() != checkpoint.sampler
            || replay.sampled_positions.as_slice() != checkpoint.sampled_positions.as_ref()
            || !super::super::checkpoint::same_values(&cache, checkpoint.numerical.cache())?
            || !same_logits(replay.decoder.logits().ok(), checkpoint.numerical.logits())
        { return Err(Error::Binding); }
        Ok(replay)
    }
}

impl SampledSession {
    pub fn model(&self) -> &DecoderModel { self.decoder.model() }
    pub fn position(&self) -> u64 { self.decoder.position() }
    pub fn tokens(&self) -> &[u32] { self.decoder.tokens() }
    pub fn logits(&self) -> Result<&[f32], Error> { self.decoder.logits() }
    pub fn cache_image(&self) -> Result<ModelKvImage, Error> { self.decoder.cache_image() }
    pub fn work(&self) -> DecoderWork { self.decoder.work() }
    pub fn sampler_state(&self) -> SamplerSnapshot { self.sampler.snapshot() }
    pub fn sampled_positions(&self) -> &[u64] { &self.sampled_positions }

    pub fn checkpoint(&self) -> Result<SampledCheckpoint, Error> {
        Ok(SampledCheckpoint { numerical: self.decoder.checkpoint()?, initial: self.initial.clone(),
            sampler: self.sampler.snapshot(), sampled_positions: self.sampled_positions.clone().into() })
    }

    /// Explicit teacher forcing; succeeds/fails through the existing decoder and
    /// never advances RNG state. Mixed forced/sampled histories are replayable.
    pub fn advance_forced(&mut self, expected_position: u64, token: u32,
        budget: DecoderBudget) -> Result<DecoderStep, Error>
    { self.decoder.advance(expected_position, token, budget) }

    pub fn advance_sampled(&mut self, expected_position: u64, budget: SampleBudget) -> Result<SampledStep, Error> {
        if self.position() != expected_position { return Err(Error::Stale); }
        self.model().estimate(self.tokens().len(), 1)?.check(budget.decoder)?;
        let prepared = self.sampler.prepare(self.logits()?, budget.sampling)?;
        self.sampled_positions.try_reserve(1).map_err(|_| Error::Limit)?;
        let computation = self.decoder.advance(expected_position, prepared.sample.token, budget.decoder)?;
        // Inference has committed all layers and logits. Only infallible owned
        // assignments and a pre-reserved push remain. Failed inference leaves
        // the previous RNG state and draw-position history exactly unchanged.
        self.sampler.snapshot = prepared.next;
        self.sampled_positions.push(expected_position);
        Ok(SampledStep { choice: prepared.sample, computation })
    }
}

fn same_logits(left: Option<&[f32]>, right: Option<&[f32]>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(a), Some(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::activation::tensor::kv::decoder::{
        DecoderIdentity, DecoderLayerWeights, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
    };
    fn fixture() -> (DecoderModel, SampledCheckpoint, SampleBudget) {
        let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 1,
            tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 4, hidden: 2,
            intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 8 }, 0.00001, 10000.0).unwrap();
        let m = DecoderModel::new(profile, vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0, -1.0, 1.0], vec![DecoderLayerWeights {
            attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
            attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4],
            up: vec![0.0; 4], down: vec![0.0; 4],
        }], vec![1.0; 2], vec![0.0; 8]).unwrap();
        let budget = SampleBudget { decoder: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
            sampling: SamplingBudget { vocabulary: 4 } };
        let mut s = m.recompute_sampled(1, &[0], budget.decoder, SamplingStart {
            policy: SamplingPolicy::new(1, 1, 4, 1.0, 0, 1.0).unwrap(), stream: 10, seed: 0,
        }).unwrap();
        assert_eq!(s.advance_sampled(1, budget).unwrap().choice.token, 2);
        s.advance_forced(2, 1, budget.decoder).unwrap();
        assert_eq!(s.advance_sampled(3, budget).unwrap().choice.token, 2);
        let cp = s.checkpoint().unwrap();
        (m, cp, budget)
    }
    #[test]
    fn recomputation_checks_final_generator_words_not_only_draw_count() {
        let (m, mut cp, budget) = fixture();
        assert!(m.recompute_sampled_checkpoint(&cp, 2, budget).is_ok());
        cp.sampler.state[0] ^= 1;
        assert_eq!(cp.sampler.draws(), 2);
        assert_eq!(m.recompute_sampled_checkpoint(&cp, 2, budget).unwrap_err(), Error::Binding);
    }
    #[test]
    fn recomputation_checks_sampled_positions_even_when_logits_and_cache_are_constant() {
        let (m, mut cp, budget) = fixture();
        assert!(m.recompute_sampled_checkpoint(&cp, 2, budget).is_ok());
        assert_eq!(cp.sampled_positions(), &[1, 3]);
        cp.sampled_positions = vec![2, 3].into();
        // This preserves sorted positions, counts and all numeric tensor values,
        // but would attribute the forced token 1 to a draw that actually chose 2.
        assert_eq!(m.recompute_sampled_checkpoint(&cp, 2, budget).unwrap_err(), Error::Binding);
    }
}
