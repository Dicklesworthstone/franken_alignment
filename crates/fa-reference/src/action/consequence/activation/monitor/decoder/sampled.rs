//! Sample through the original all-layer monitor, never around it.
//! Random state commits with successful numerical computation, even when review
//! subsequently holds. Only a completely quiet review exposes the sampled ID.

use super::{DecoderReview, MonitoredDecoder, MonitoredStep, MonitoringStatus, MonitoringWork, ReviewedStep};
use super::observation::DecoderObservation;
use super::super::{RefinementBudget, RefinementMonitor};
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderModel, DecoderProfile, DecoderWork, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{
    SampleBudget, SampledToken, Sampler, SamplingStart,
};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

/// The choice and its computation are available together, only after the
/// original complete residual review. Debug omits token, logits and random word.
pub struct ReviewedSampledStep {
    reviewed: ReviewedStep,
    choice: SampledToken,
}
impl ReviewedSampledStep {
    pub fn reviewed(&self) -> &ReviewedStep { &self.reviewed }
    pub fn choice(&self) -> &SampledToken { &self.choice }
    pub fn into_reviewed(self) -> ReviewedStep { self.reviewed }
}
impl fmt::Debug for ReviewedSampledStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReviewedSampledStep").field("reviewed", &self.reviewed)
            .field("draw", &self.choice.draw).finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum MonitoredSampledStep {
    Released(ReviewedSampledStep),
    Held(Rc<DecoderReview>),
}
impl MonitoredSampledStep {
    pub fn review(&self) -> &DecoderReview {
        match self { Self::Released(step) => step.reviewed.review(), Self::Held(review) => review }
    }
    /// Discard optional sampling diagnostics without weakening the review type.
    pub fn into_monitored(self) -> MonitoredStep {
        match self {
            Self::Released(step) => MonitoredStep::Released(step.into_reviewed()),
            Self::Held(review) => MonitoredStep::Held(review),
        }
    }
}

/// A new numerical session must review its entire supplied prefix. No import of
/// already advanced sessions, mutable sampler, seed replacement, raw checkpoint,
/// unreviewed logits or historical token-list accessor is provided. A held owner
/// stays held and cannot reroll. The separately supplied model is still trusted
/// host data; this is not OS isolation or production effect authorization.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
/// fn bypass(run: &mut MonitoredSampledDecoder) { run.sampler_mut(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
/// fn leak(run: &MonitoredSampledDecoder) { run.checkpoint(); }
/// ```
pub struct MonitoredSampledDecoder {
    monitored: MonitoredDecoder,
    sampler: Sampler,
}
impl fmt::Debug for MonitoredSampledDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitoredSampledDecoder").field("monitored", &self.monitored)
            .field("sampled_draws", &self.sampled_draws()).finish_non_exhaustive()
    }
}
impl MonitoredSampledDecoder {
    pub fn new(
        model: DecoderModel, stream: u64, generation: u64,
        monitors: BTreeMap<u64, RefinementMonitor>, monitoring_budget: RefinementBudget,
        sampling: SamplingStart,
    ) -> Result<Self, Error> {
        if sampling.policy.vocabulary() != model.profile().shape().vocabulary { return Err(Error::Binding); }
        let sampler = Sampler::seeded(sampling.policy, sampling.stream, sampling.seed)?;
        let monitored = MonitoredDecoder::new(model, stream, generation, monitors, monitoring_budget)?;
        Ok(Self { monitored, sampler })
    }

    pub fn profile(&self) -> &DecoderProfile { self.monitored.profile() }
    pub fn position(&self) -> u64 { self.monitored.position() }
    pub fn status(&self) -> MonitoringStatus { self.monitored.status() }
    pub fn observation(&self) -> DecoderObservation { self.monitored.observation() }
    pub fn monitoring_work(&self) -> MonitoringWork { self.monitored.monitoring_work() }
    pub fn decoder_work(&self) -> DecoderWork { self.monitored.decoder_work() }
    pub fn remaining_budget(&self) -> RefinementBudget { self.monitored.remaining_budget() }
    pub fn last_review(&self) -> Option<&DecoderReview> { self.monitored.last_review() }
    pub fn estimate(&self, tokens: usize) -> Result<DecoderWork, Error> { self.monitored.estimate(tokens) }
    /// Successfully computed sampled tokens, including one withheld by review.
    /// This reveals no unreviewed token, seed, current RNG words or next logits.
    pub fn sampled_draws(&self) -> u64 { self.sampler.snapshot().draws() }

    /// Explicit teacher forcing still reviews every layer and consumes no draw.
    pub fn advance_forced(&mut self, expected_position: u64, token: u32,
        budget: DecoderBudget) -> Result<MonitoredStep, Error>
    { self.monitored.advance(expected_position, token, budget) }

    pub fn advance_sampled(&mut self, expected_position: u64, budget: SampleBudget)
        -> Result<MonitoredSampledStep, Error>
    {
        self.monitored.check_position(expected_position)?;
        let products = self.estimate(1)?.scalar_products()?;
        if budget.decoder.scalar_products > MAX_DECODER_PRODUCTS || products > budget.decoder.scalar_products {
            return Err(Error::Limit);
        }
        // The original owner is Ready only after the previous token's complete
        // quiet review. Preparation mutates a private copy, not the live sampler.
        let mut staged = Sampler::from_snapshot(&self.sampler.snapshot());
        let choice = staged.sample(self.monitored.session.logits()?, budget.sampling)?;
        let Self { monitored, sampler } = self;
        let result = monitored.advance_with_commit(expected_position, choice.token, budget.decoder, || {
            // Called immediately after the SAME decoder's numeric commit, before
            // any fallible monitoring or evidence publication. No reroll on hold.
            *sampler = staged;
        })?;
        Ok(match result {
            MonitoredStep::Released(reviewed) => MonitoredSampledStep::Released(ReviewedSampledStep { reviewed, choice }),
            MonitoredStep::Held(review) => MonitoredSampledStep::Held(review),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
    use crate::action::consequence::activation::probe::LinearProbe;
    use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderLayerWeights, DecoderShape};
    use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingBudget, SamplingPolicy};
    fn fixture() -> (MonitoredSampledDecoder, SampleBudget) {
        let p = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
            tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 2, hidden: 2,
            intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap();
        let model = DecoderModel::new(p, vec![1.0, 0.0, 1.0, 0.0], vec![DecoderLayerWeights {
            attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
            attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
        }], vec![1.0; 2], vec![0.0; 4]).unwrap();
        let allowance = RefinementBudget { encoded_bytes: 10000, probe_coordinates: 10000 };
        let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(), &[1.0, 0.0], 0.0, 100.0).unwrap();
        let monitors = BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap())]);
        let start = SamplingStart { policy: SamplingPolicy::new(1, 1, 2, 1.0, 0, 1.0).unwrap(), stream: 9, seed: 0 };
        let run = MonitoredSampledDecoder::new(model, 7, 11, monitors, allowance, start).unwrap();
        (run, SampleBudget { decoder: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, sampling: SamplingBudget { vocabulary: 2 } })
    }
    #[test]
    fn error_after_numeric_commit_retains_draw_but_invalidates_live_evidence() {
        let (mut run, budget) = fixture();
        assert!(matches!(run.advance_forced(0, 0, budget.decoder).unwrap(), MonitoredStep::Released(_)));
        let source = run.observation(); let old = source.capture().unwrap();
        run.monitored.work.frame_reviews = u64::MAX;
        assert!(matches!(run.advance_sampled(1, budget), Err(Error::Overflow)));
        assert_eq!(run.position(), 2); assert_eq!(run.sampled_draws(), 1);
        assert_eq!(run.status(), MonitoringStatus::Failed(Error::Overflow));
        assert_eq!(source.availability(), DecoderAvailability::Failed);
        assert_eq!(source.validate(&old), Err(Error::Incomplete));
        assert!(matches!(run.advance_sampled(2, budget), Err(Error::WrongState)));
        assert_eq!(run.sampled_draws(), 1);
    }
    #[test]
    fn draw_counter_overflow_is_precomputation_and_does_not_invalidate_quiet_prefix() {
        let (mut run, budget) = fixture();
        run.advance_forced(0, 0, budget.decoder).unwrap();
        let source = run.observation(); let old = source.capture().unwrap();
        let snapshot = run.sampler.snapshot(); let mut bytes = snapshot.encode();
        bytes[56..64].copy_from_slice(&u64::MAX.to_be_bytes());
        let snapshot = crate::action::consequence::activation::tensor::kv::decoder::sampling::SamplerSnapshot::decode(&bytes, snapshot.policy()).unwrap();
        run.sampler = Sampler::from_snapshot(&snapshot);
        assert!(matches!(run.advance_sampled(1, budget), Err(Error::Overflow)));
        assert_eq!(run.position(), 1); assert_eq!(run.sampled_draws(), u64::MAX);
        assert_eq!(run.status(), MonitoringStatus::Ready); source.validate(&old).unwrap();
    }
}
