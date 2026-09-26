//! Direct exact-KV continuation of an originally audited sampled generation.
//!
//! Typed checkpoints retain the original spec, RNG, policy and SPENT lifetime
//! budgets. Restart needs a fresh complete prefix audit, but neither replays
//! prompt/sample inference nor refills any remaining continuation allowance.
use super::{GenerationBudget, GenerationEstimate, GenerationSpec, GenerationStatus,
    GenerationStop, GenerationTelemetryBudget, GenerationTelemetryWork, GenerationWork,
    LearnedGeneration, MAX_GENERATION_TOKENS};
use super::super::{Sampler, SamplerSnapshot, SampledToken};
use super::super::super::{DecoderModel, DecoderRestoreBudget,
    monitoring::{LearnedDecoderPolicy, restart::{KvRestartBudget, MonitoredKvCheckpoint,
        MonitoredKvRestart, MonitoredKvRestartReceipt, RestartAudit}}};
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// In-memory state made only by an entirely accepted original generation. No
/// decoder for untrusted bytes and no conversion from a bare KV image exist.
/// Restored output still needs the normal external observation/authority path.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::restart::GenerationKvCheckpoint;
/// fn import(bytes: &[u8]) { let _ = GenerationKvCheckpoint::decode(bytes); }
/// ```
#[derive(Clone)]
pub struct GenerationKvCheckpoint {
    model: DecoderModel,
    guard: MonitoredKvCheckpoint,
    sampler: SamplerSnapshot,
    spec: GenerationSpec,
    budget: GenerationBudget,
    telemetry_budget: GenerationTelemetryBudget,
    estimate: GenerationEstimate,
    status: GenerationStatus,
    work: GenerationWork,
    telemetry_work: GenerationTelemetryWork,
    samples: Rc<[SampledToken]>,
}
impl fmt::Debug for GenerationKvCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationKvCheckpoint").field("position", &self.position())
            .field("status", &self.status).field("work", &self.work).finish_non_exhaustive()
    }
}
impl LearnedGeneration {
    /// Capture only active or finished generation, never a held/failed attempt's
    /// shorter prefix. Model weights/cache arrays are retained through their
    /// original immutable objects; all histories remain bounded by the admitted
    /// profile/spec and MAX_GENERATION_TOKENS, not allocator/RSS guarantees.
    pub fn checkpoint_kv(&self, limits: DecoderRestoreBudget) -> Result<GenerationKvCheckpoint, Error> {
        if !self.status.is_active() && !matches!(self.status, GenerationStatus::Finished(_)) {
            return Err(Error::WrongState);
        }
        let count = self.accepted_tokens().len();
        let samples = count.saturating_sub(self.spec.prompt().len());
        let sampler = self.sampler.snapshot();
        let numerical = self.model.estimate(0, count)?;
        let scores = (samples as u64).checked_mul(self.spec.sampling().policy.vocabulary() as u64)
            .ok_or(Error::Overflow)?;
        if count > MAX_GENERATION_TOKENS || self.position() != count as u64
            || self.work.admitted_tokens != count as u64 || self.work.accepted_decoder != numerical
            || self.work.reserved_decoder_products != numerical.scalar_products()?
            || self.work.sampling_attempts != samples as u64 || self.work.reserved_vocabulary_scores != scores
            || self.samples.len() != samples || sampler.draws() != samples as u64
            || sampler.policy() != &self.spec.sampling().policy || sampler.stream() != self.spec.sampling().stream
            || !self.telemetry_work.fits(self.telemetry_budget) { return Err(Error::Binding); }
        let expected_status = if count < self.spec.prompt().len() {
            GenerationStatus::Prefilling
        } else if samples > 0 && self.spec.stop_tokens().contains(&self.accepted_tokens()[count - 1]) {
            GenerationStatus::Finished(GenerationStop::StopToken(self.accepted_tokens()[count - 1]))
        } else if samples == self.spec.max_new_tokens() {
            GenerationStatus::Finished(GenerationStop::TokenLimit)
        } else { GenerationStatus::Generating };
        let prompt = count.min(self.spec.prompt().len());
        if samples > self.spec.max_new_tokens() || self.status != expected_status
            || self.accepted_tokens()[..prompt] != self.spec.prompt()[..prompt]
            || self.samples.iter().enumerate().any(|(index, sample)| {
                sample.token != self.accepted_tokens()[self.spec.prompt().len() + index]
                    || sample.draw != index as u64 + 1 || sample.stream != sampler.stream()
            }) { return Err(Error::Binding); }
        let mut history = Vec::new();
        history.try_reserve_exact(samples).map_err(|_| Error::Limit)?;
        history.extend_from_slice(&self.samples);
        let guard = self.guard.checkpoint_kv(limits)?;
        Ok(GenerationKvCheckpoint { model: self.model.clone(), guard, sampler,
            spec: self.spec.clone(), budget: self.budget, telemetry_budget: self.telemetry_budget,
            estimate: self.estimate, status: self.status, work: self.work,
            telemetry_work: self.telemetry_work, samples: history.into() })
    }
}
impl GenerationKvCheckpoint {
    pub fn position(&self) -> u64 { self.guard.position() }
    pub fn status(&self) -> GenerationStatus { self.status }
    pub fn work(&self) -> GenerationWork { self.work }
    pub fn telemetry_work(&self) -> GenerationTelemetryWork { self.telemetry_work }
    pub fn cache_values(&self) -> usize { self.guard.cache_values() }
    pub fn policy(&self) -> &LearnedDecoderPolicy { self.guard.policy() }
    pub fn spec(&self) -> &GenerationSpec { &self.spec }

    /// Destination stream identifies derived KV; original sampling stream/seed
    /// and current random state are unchanged. Audit work is a separately bounded
    /// restart cost, never a refill of the original aggregate telemetry budget.
    pub fn begin_restart(&self, resumed_stream: u64, budget: KvRestartBudget)
        -> Result<GenerationKvRestart, Error>
    {
        // The ORIGINAL generation reserves all sample capacity before any token
        // can commit. Preserve that guarantee, not merely the current Vec length.
        let mut samples = Vec::new();
        samples.try_reserve_exact(self.spec.max_new_tokens()).map_err(|_| Error::Limit)?;
        samples.extend_from_slice(&self.samples);
        let restart = self.guard.begin_restart(resumed_stream, budget)?;
        Ok(GenerationKvRestart { checkpoint: self.clone(), restart, samples })
    }
}

/// No sampler/guard/output is available before complete fresh verification and
/// exact restoration. A missing audit or hold cannot be patched into success.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::restart::GenerationKvRestart;
/// fn bypass(restart: &mut GenerationKvRestart) { restart.generation_mut(); }
/// ```
pub struct GenerationKvRestart {
    checkpoint: GenerationKvCheckpoint,
    restart: MonitoredKvRestart,
    samples: Vec<SampledToken>,
}
impl fmt::Debug for GenerationKvRestart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationKvRestart").field("checkpoint", &self.checkpoint)
            .field("ready", &self.is_ready()).finish_non_exhaustive()
    }
}
impl GenerationKvRestart {
    pub fn audit(&self) -> Option<&RestartAudit> { self.restart.audit() }
    pub fn is_ready(&self) -> bool { self.restart.is_ready() }
    pub fn finish(self) -> Result<(LearnedGeneration, GenerationKvRestartReceipt), Error> {
        let Self { checkpoint, restart, samples } = self;
        let (guard, kv) = restart.finish()?;
        let receipt = GenerationKvRestartReceipt { kv, historical_work: checkpoint.work,
            historical_telemetry: checkpoint.telemetry_work, status: checkpoint.status,
            sampler_draws: checkpoint.sampler.draws() };
        // This is a typed original checkpoint, NOT the archive's untrusted State.
        // Fresh learned evidence gated the guard above; only immutable original
        // generation metadata is transferred. No old last_event is republished.
        let generation = LearnedGeneration { model: checkpoint.model, guard,
            sampler: Sampler::from_snapshot(&checkpoint.sampler), spec: checkpoint.spec,
            budget: checkpoint.budget, telemetry_budget: checkpoint.telemetry_budget,
            estimate: checkpoint.estimate, status: checkpoint.status, work: checkpoint.work,
            telemetry_work: checkpoint.telemetry_work, samples, last_event: None };
        Ok((generation, receipt))
    }
}

/// Historical generation spend remains separate from fresh KV/audit work.
/// Copies of a receipt convey no permit, fresh observation, refund or live state.
#[derive(Clone, Debug)]
pub struct GenerationKvRestartReceipt {
    kv: MonitoredKvRestartReceipt,
    historical_work: GenerationWork,
    historical_telemetry: GenerationTelemetryWork,
    status: GenerationStatus,
    sampler_draws: u64,
}
impl GenerationKvRestartReceipt {
    pub fn kv(&self) -> &MonitoredKvRestartReceipt { &self.kv }
    pub fn historical_work(&self) -> GenerationWork { self.historical_work }
    pub fn historical_telemetry(&self) -> GenerationTelemetryWork { self.historical_telemetry }
    pub fn status(&self) -> GenerationStatus { self.status }
    pub fn sampler_draws(&self) -> u64 { self.sampler_draws }
}
