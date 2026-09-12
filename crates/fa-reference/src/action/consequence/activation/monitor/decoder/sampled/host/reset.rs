//! Private numerical half of a controller-paired checkpoint. Replays never
//! import monitor approvals: every original token is computed and reviewed again.

use super::{CapturedState, MonitoredSampledDecoder};
use super::super::super::{MonitoredDecoder, MonitoredStep, MonitoringStatus, MonitoringWork};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderCheckpoint, DecoderWork, MAX_DECODER_PRODUCTS};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{Sampler, SamplerSnapshot};
use crate::Error;

#[derive(Clone, Debug)]
pub(crate) struct NumericalCheckpoint {
    numerical: DecoderCheckpoint,
    sampler: SamplerSnapshot,
    generation: u64,
    pub logical_bytes: usize,
}
impl NumericalCheckpoint {
    pub fn products(&self) -> Result<u64, Error> {
        self.numerical.model().estimate(0, self.numerical.tokens().len())?.scalar_products()
    }
}

pub(crate) struct ReplayedHost {
    pub run: MonitoredSampledDecoder,
    pub state: CapturedState,
}

pub(crate) struct ReplayAttempt {
    pub result: Result<ReplayedHost, Error>,
    pub numerical: DecoderWork,
}

impl MonitoredSampledDecoder {
    pub(crate) fn checkpoint_host(&self, cache_limit: usize, sampler_limit: usize)
        -> Result<(NumericalCheckpoint, CapturedState), Error>
    {
        if self.status() != MonitoringStatus::Ready || self.position() == 0 { return Err(Error::WrongState); }
        // Capturing confirms the live complete review, not a retained older one.
        self.observation().capture()?;
        let state = self.capture_host_state(cache_limit, sampler_limit)?;
        let numerical = self.monitored.session.checkpoint()?;
        let logical_bytes = state.tokens.len().checked_mul(4)
            .and_then(|n| n.checked_add(state.cache.len()))
            .and_then(|n| n.checked_add(state.sampler.len()))
            .and_then(|n| n.checked_add(numerical.logits().map_or(0, |logits| logits.len() * 4)))
            .ok_or(Error::Limit)?;
        Ok((NumericalCheckpoint { numerical, sampler: self.sampler.snapshot(),
            generation: self.monitored.generation, logical_bytes }, state))
    }

    /// The owning broker has already charged its bounded replay-attempt ledger.
    /// No old numerical state is mutated. Once preparation succeeds, withdrawal
    /// is sticky even if recomputation, monitoring, or comparison later fails.
    pub(crate) fn replay_host(&mut self, checkpoint: &NumericalCheckpoint, stream: u64,
        budget: DecoderBudget, cache_limit: usize, sampler_limit: usize) -> ReplayAttempt
    {
        let prepared = (|| {
            if stream <= self.observation().stream() || stream == checkpoint.numerical.stream()
                || self.profile() != checkpoint.numerical.model().profile()
                || self.monitored.generation != checkpoint.generation
                || self.sampler.snapshot().policy() != checkpoint.sampler.policy()
            { return Err(Error::Binding); }
            if budget.scalar_products > MAX_DECODER_PRODUCTS || checkpoint.products()? > budget.scalar_products {
                return Err(Error::Limit);
            }
            let mut monitored = MonitoredDecoder::new(checkpoint.numerical.model().clone(), stream,
                self.monitored.generation, self.monitored.monitors.clone(), self.monitored.budget)?;
            // Lifetime monitoring allowances, not the values at the checkpoint.
            monitored.work = self.monitored.work;
            Ok(MonitoredSampledDecoder { monitored, sampler: Sampler::from_snapshot(&checkpoint.sampler) })
        })();
        let mut candidate = match prepared {
            Ok(candidate) => candidate,
            Err(error) => return ReplayAttempt { result: Err(error), numerical: DecoderWork::default() },
        };
        self.fail_host(Error::Incomplete);
        let result = {
            // Also retain completed monitoring charges during a caught unwind.
            let accounting = ReplayAccounting { candidate: &mut candidate, charged: &mut self.monitored.work };
            replay_checked(&mut *accounting.candidate, checkpoint, cache_limit, sampler_limit)
        };
        let numerical = candidate.decoder_work();
        let result = match result {
            Ok(state) => Ok(ReplayedHost { run: candidate, state }),
            Err(error) => { self.fail_host(error); Err(error) }
        };
        ReplayAttempt { result, numerical }
    }
}

struct ReplayAccounting<'a> {
    candidate: &'a mut MonitoredSampledDecoder,
    charged: &'a mut MonitoringWork,
}
impl Drop for ReplayAccounting<'_> {
    fn drop(&mut self) { *self.charged = self.candidate.monitoring_work(); }
}

fn replay_checked(candidate: &mut MonitoredSampledDecoder, checkpoint: &NumericalCheckpoint,
    cache_limit: usize, sampler_limit: usize) -> Result<CapturedState, Error>
{
    for (position, token) in checkpoint.numerical.tokens().iter().copied().enumerate() {
        let products = candidate.estimate(1)?.scalar_products()?;
        if !matches!(candidate.advance_forced(position as u64, token,
            DecoderBudget { scalar_products: products })?, MonitoredStep::Released(_))
        { return Err(Error::Incomplete); }
    }
    let session = &candidate.monitored.session;
    if session.tokens() != checkpoint.numerical.tokens() || candidate.sampler.snapshot() != checkpoint.sampler {
        return Err(Error::Binding);
    }
    let logits = session.logits()?;
    let expected = checkpoint.numerical.logits().ok_or(Error::Incomplete)?;
    if logits.len() != expected.len() || logits.iter().zip(expected).any(|(a, b)| a.to_bits() != b.to_bits()) {
        return Err(Error::Binding);
    }
    let actual = session.cache_image()?;
    let expected = checkpoint.numerical.cache();
    if actual.profile() != expected.profile() || actual.len() != expected.len() { return Err(Error::Binding); }
    for layer in actual.profile().layers().keys() {
        let left = actual.layer(*layer)?;
        let right = expected.layer(*layer)?;
        for position in 0..actual.len() as u64 {
            let a = left.token(position)?;
            let b = right.token(position)?;
            // Internal source bits, not rounded summary statistics. Different
            // replay-stream provenance is deliberately not called a mismatch.
            if a.key().words.as_ref() != b.key().words.as_ref()
                || a.value().words.as_ref() != b.value().words.as_ref() { return Err(Error::Binding); }
        }
    }
    candidate.capture_host_state(cache_limit, sampler_limit)
}

pub(crate) fn sum_work(a: DecoderWork, b: DecoderWork) -> Result<DecoderWork, Error> {
    let add = |a: u64, b: u64| a.checked_add(b).ok_or(Error::Overflow);
    Ok(DecoderWork {
        tokens: add(a.tokens, b.tokens)?, matrix_products: add(a.matrix_products, b.matrix_products)?,
        attention_products: add(a.attention_products, b.attention_products)?,
        attention_exponentials: add(a.attention_exponentials, b.attention_exponentials)?,
        normalization_coordinates: add(a.normalization_coordinates, b.normalization_coordinates)?,
        rotary_pairs: add(a.rotary_pairs, b.rotary_pairs)?, gate_coordinates: add(a.gate_coordinates, b.gate_coordinates)?,
        cache_values_appended: add(a.cache_values_appended, b.cache_values_appended)?,
    })
}
