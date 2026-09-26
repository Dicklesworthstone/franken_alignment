//! Sampled generation resumed only after ordered complete incremental KV audit.
use super::{GenerationKvCheckpoint, LearnedGeneration, SampledToken, RestartAudit,
    GenerationWork, GenerationTelemetryWork, GenerationStatus};
use crate::action::consequence::activation::tensor::kv::{model::ModelKvDescriptor,
    decoder::monitoring::restart::incremental::{IncrementalKvRestart,
        IncrementalKvRestartReceipt, IncrementalRestartBudget, IncrementalRestartStatus,
        IncrementalRestartWork, RestartAuditCost}};
use crate::Error;
use std::fmt;
use std::rc::Rc;

impl GenerationKvCheckpoint {
    /// Incremental verification can span multiple bounded caller turns. The
    /// sealed prompt, phase, random state, horizon and spent generation budgets
    /// remain fixed; fresh auditing is a separately bounded restart operation.
    pub fn begin_incremental_restart(&self, resumed_stream: u64, budget: IncrementalRestartBudget)
        -> Result<IncrementalGenerationRestart, Error>
    {
        let mut samples = Vec::new();
        samples.try_reserve_exact(self.spec.max_new_tokens()).map_err(|_| Error::Limit)?;
        samples.extend_from_slice(&self.samples);
        let restart = self.guard.begin_incremental_restart(resumed_stream, budget)?;
        Ok(IncrementalGenerationRestart { checkpoint: self.clone(), restart, samples })
    }
}

/// Holds no accessible generation, pending logits or mutable sampler. A partial
/// or held audit cannot release even the earlier quiet portion of the prefix.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::restart::incremental::IncrementalGenerationRestart;
/// fn bypass(restart: &mut IncrementalGenerationRestart) { restart.generation_mut(); }
/// ```
pub struct IncrementalGenerationRestart {
    checkpoint: GenerationKvCheckpoint,
    restart: IncrementalKvRestart,
    samples: Vec<SampledToken>,
}
impl fmt::Debug for IncrementalGenerationRestart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IncrementalGenerationRestart").field("next_position", &self.next_position())
            .field("status", &self.status()).field("generation_status", &self.checkpoint.status)
            .finish_non_exhaustive()
    }
}
impl IncrementalGenerationRestart {
    pub fn status(&self) -> IncrementalRestartStatus { self.restart.status() }
    pub fn next_position(&self) -> u64 { self.restart.next_position() }
    pub fn position_count(&self) -> usize { self.restart.position_count() }
    pub fn work(&self) -> IncrementalRestartWork { self.restart.work() }
    pub fn reservation(&self) -> RestartAuditCost { self.restart.reservation() }
    pub fn source(&self) -> ModelKvDescriptor { self.restart.source() }
    pub fn last_audit(&self) -> Option<&RestartAudit> { self.restart.last_audit() }
    pub fn is_ready(&self) -> bool { self.restart.is_ready() }
    pub fn advance(&mut self, expected_position: u64) -> Result<Rc<RestartAudit>, Error> {
        self.restart.advance(expected_position)
    }
    pub fn finish(self) -> Result<(LearnedGeneration, IncrementalGenerationRestartReceipt), Error> {
        let Self { checkpoint, restart, samples } = self;
        let (guard, kv) = restart.finish()?;
        let receipt = IncrementalGenerationRestartReceipt { kv, historical_work: checkpoint.work,
            historical_telemetry: checkpoint.telemetry_work, status: checkpoint.status,
            sampler_draws: checkpoint.sampler.draws() };
        // The same composition seam as all-at-once restart: no reseeding,
        // allowance renewal, phase reset or old event republished as fresh.
        Ok((checkpoint.resume(guard, samples), receipt))
    }
}

#[derive(Clone, Debug)]
pub struct IncrementalGenerationRestartReceipt {
    kv: IncrementalKvRestartReceipt,
    historical_work: GenerationWork,
    historical_telemetry: GenerationTelemetryWork,
    status: GenerationStatus,
    sampler_draws: u64,
}
impl IncrementalGenerationRestartReceipt {
    pub fn kv(&self) -> &IncrementalKvRestartReceipt { &self.kv }
    pub fn historical_work(&self) -> GenerationWork { self.historical_work }
    pub fn historical_telemetry(&self) -> GenerationTelemetryWork { self.historical_telemetry }
    pub fn status(&self) -> GenerationStatus { self.status }
    pub fn sampler_draws(&self) -> u64 { self.sampler_draws }
}
