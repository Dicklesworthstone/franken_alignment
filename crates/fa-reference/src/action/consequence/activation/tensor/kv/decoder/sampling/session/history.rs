//! Shared original-token/RNG history execution for pinned and portable checks.
use super::{SampleBudget, SampledSession};
use super::super::{Sampler, SamplerSnapshot, MAX_DECODER_VOCABULARY};
use super::super::super::{DecoderBudget, DecoderModel};
use crate::Error;

impl DecoderModel {
    pub(super) fn replay_sampled_history(
        &self, stream: u64, tokens: &[u32], initial: &SamplerSnapshot,
        sampled_positions: &[u64], budget: SampleBudget,
    ) -> Result<SampledSession, Error> {
        self.estimate(0, tokens.len())?.check(budget.decoder)?;
        let vocabulary = self.profile().shape().vocabulary;
        if budget.sampling.vocabulary > MAX_DECODER_VOCABULARY
            || budget.sampling.vocabulary < vocabulary { return Err(Error::Limit); }
        if initial.policy().vocabulary() != vocabulary || initial.draws() != 0
            || sampled_positions.windows(2).any(|pair| pair[0] >= pair[1])
            || sampled_positions.iter().any(|p| *p == 0 || *p >= tokens.len() as u64)
            || tokens.iter().any(|token| *token as usize >= vocabulary) { return Err(Error::Binding); }
        let mut replay = SampledSession { decoder: self.session(stream)?,
            sampler: Sampler::from_snapshot(initial), initial: initial.clone(), sampled_positions: Vec::new() };
        let mut positions = sampled_positions.iter().copied().peekable();
        for (position, token) in tokens.iter().copied().enumerate() {
            let position64 = position as u64;
            let decoder = DecoderBudget { scalar_products: self.estimate(position, 1)?.scalar_products()? };
            if positions.peek() == Some(&position64) {
                let actual = replay.advance_sampled(position64, SampleBudget { decoder, sampling: budget.sampling })?;
                if actual.choice.token != token { return Err(Error::Binding); }
                positions.next();
            } else { replay.advance_forced(position64, token, decoder)?; }
        }
        Ok(replay)
    }
}
