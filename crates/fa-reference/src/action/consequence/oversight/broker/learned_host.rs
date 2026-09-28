//! Own learned-K/V generation and the exact actor copy at the existing effect gate.
//! Numerical acceptance synchronizes state, not congress approval or a permit.
mod stopping;
pub mod checkpoint;
pub mod sidecar;
pub mod text;
pub use stopping::{LearnedHostStopCause, LearnedHostStopIncident};
#[cfg(test)]
mod tests;

use super::OversightBroker;
use super::decoder_gate::LearnedDecoderBindingLimits;
use super::super::learned_source::{
    LearnedAvailability, LearnedObservation, LearnedSourceConfig, ObservedLearnedGeneration,
};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationEvent, GenerationStatus, GenerationTelemetryWork, GenerationWork,
};
use crate::action::consequence::gate::containment::RestartProfile;
use crate::Error;
use std::rc::Rc;

/// Supervisor-side state/costs without KV, random words or withheld token IDs.
/// Work includes held/failed attempts; actor_revision advances only when a quiet
/// accepted token and its actual cache/sampler copy have both been synchronized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedLearnedInspection {
    pub actor_revision: u64,
    pub position: u64,
    pub sampled_draws: u64,
    pub status: GenerationStatus,
    pub availability: LearnedAvailability,
    pub work: GenerationWork,
    pub telemetry: GenerationTelemetryWork,
    /// Logical completed/admitted work across all abandoned continuations.
    /// The active timeline's historical counters above may rewind; these do not.
    pub cumulative_work: GenerationWork,
    /// Includes completed fresh restart audits, even when restoration refuses.
    pub cumulative_telemetry: GenerationTelemetryWork,
    pub cache_bytes: usize,
    pub sampler_bytes: usize,
    pub host_failure: Option<Error>,
}

#[derive(Debug)]
pub(super) struct LearnedHost {
    run: ObservedLearnedGeneration,
    profile: RestartProfile,
    fault: Option<Error>,
    automatic_stop: Option<stopping::LearnedStopState>,
    sidecars: sidecar::LearnedSidecarState,
    text_stream: Option<text::stream::LearnedTextStreamBasis>,
    recovery: checkpoint::RecoveryState,
}

impl OversightBroker {
    /// Trusted bootstrap only. Construct the ORIGINAL learned generator from zero
    /// and install its exact liveness handle at the existing effect gate. There
    /// is no adoption of an advanced/restarted experiment or a replacement model.
    /// The full declared actor-cache horizon must fit before changing the broker.
    /// This does not establish the caller's registered restart grade. The paired
    /// checkpoint API separately checks that grade and the actual numerical state.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::oversight::OversightBroker;
    /// fn bypass(owner: &mut OversightBroker) { owner.learned_generation_mut(); }
    /// ```
    pub fn own_learned_generation(&mut self, model: DecoderModel, config: LearnedSourceConfig,
        limits: LearnedDecoderBindingLimits) -> Result<(), Error>
    {
        self.check_learned_bootstrap()?;
        let run = model.observed_learned_generation(config)?;
        self.install_fresh_learned_host(run, limits)
    }

    fn check_learned_bootstrap(&self) -> Result<(), Error> {
        if self.decoder.is_some() || self.decoder_host.is_some() || self.learned_host.is_some() {
            return Err(Error::Duplicate);
        }
        let actor = self.delivery.controller().actor();
        if self.inspect().suspended || self.stop_receipt().is_some() || self.inspect().sequence != 0
            || !self.inputs.is_empty() || !self.started_rounds.is_empty() || actor.next_position() != 0 {
            return Err(Error::WrongState);
        }
        let revision = self.actor_revision();
        revision.checked_add(1).ok_or(Error::Overflow)?;
        Ok(())
    }

    // Shared trusted bootstrap, not a public adopted-state constructor. Both
    // callers construct a fresh original source, never import a saved owner.
    fn install_fresh_learned_host(&mut self, run: ObservedLearnedGeneration,
        limits: LearnedDecoderBindingLimits) -> Result<(), Error>
    {
        self.check_learned_bootstrap()?;
        if run.position() != 0 || run.work().admitted_tokens != 0
            || run.observation().availability() != LearnedAvailability::Empty { return Err(Error::WrongState); }
        let revision = self.actor_revision();
        let profile = self.delivery.controller().actor().profile();
        run.check_host_horizon()?;
        let actor = run.capture_host_actor(profile)?;
        self.enable_learned_decoder_monitoring(run.observation(), limits)?;
        // Same profile, predecessor and nonsuspended authority were preflighted.
        // No caller callback or other authority transition intervenes.
        self.delivery.replace_actor_state(revision, actor).expect("preflighted learned bootstrap");
        self.learned_host = Some(LearnedHost { run, profile, fault: None, automatic_stop: None,
            sidecars: sidecar::LearnedSidecarState::default(), text_stream: None,
            recovery: checkpoint::RecoveryState::new() });
        Ok(())
    }

    // The journal compares original numbers, not Debug output or imported state.
    // This immutable crate-only view cannot advance, reset or reseed the owner.
    pub(crate) fn hosted_learned_original(&self)
        -> Result<&crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::LearnedGeneration, Error>
    {
        Ok(self.learned_host.as_ref().ok_or(Error::Incomplete)?.run.original_generation())
    }

    pub fn hosted_learned_observation(&self) -> Result<LearnedObservation, Error> {
        Ok(self.learned_host.as_ref().ok_or(Error::Incomplete)?.run.observation())
    }
    pub fn hosted_learned_generation(&self) -> Result<HostedLearnedInspection, Error> {
        let host = self.learned_host.as_ref().ok_or(Error::Incomplete)?;
        let actor = self.delivery.controller().actor();
        Ok(HostedLearnedInspection { actor_revision: self.actor_revision(), position: host.run.position(),
            sampled_draws: host.run.sampled_draws(), status: host.run.status(),
            availability: host.run.observation().availability(), work: host.run.work(),
            telemetry: host.run.telemetry_work(), cache_bytes: actor.cache().len(),
            cumulative_work: host.recovery.cumulative_work(host.run.work())?,
            cumulative_telemetry: host.recovery.cumulative_telemetry(host.run.telemetry_work())?,
            sampler_bytes: actor.sampler().len(), host_failure: host.fault })
    }

    /// One original prompt/sample step. No raw numerical, forced-token, reseed or
    /// budget override path is exposed. A held step retains its real costs but
    /// cannot synchronize its withheld candidate or uncommitted random draw.
    /// A sync failure or unwind withdraws the live source and latches this owner.
    pub fn advance_hosted_learned(&mut self, expected_actor_revision: u64, expected_position: u64)
        -> Result<Rc<GenerationEvent>, Error>
    {
        if self.enforce_learned_host_stop()?.is_some() { return Err(Error::WrongState); }
        let result = self.advance_learned_inner(expected_actor_revision, expected_position);
        let _ = self.enforce_learned_host_stop()?;
        result
    }
    fn advance_learned_inner(&mut self, expected_actor_revision: u64, expected_position: u64)
        -> Result<Rc<GenerationEvent>, Error>
    {
        if self.enforce_consistency_stop()?.is_some() || self.inspect().suspended { return Err(Error::WrongState); }
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        expected_actor_revision.checked_add(1).ok_or(Error::Overflow)?;
        let host = self.learned_host.as_mut().ok_or(Error::Incomplete)?;
        if host.fault.is_some() || !host.run.status().is_active() { return Err(Error::WrongState); }
        if expected_position != host.run.position() { return Err(Error::Stale); }
        // This latch survives an unwind even after numeric acceptance. The guard
        // also withdraws shared evidence if any later actor-copy step unwinds.
        host.fault = Some(Error::Incomplete);
        let guard = host.run.guard_host_sync();
        let result = host.run.advance(expected_position).and_then(|event| {
            if event.accepted().is_some() {
                let actor = host.run.capture_host_actor(host.profile)?;
                Ok((event, Some(actor)))
            } else { Ok((event, None)) }
        });
        let (event, actor) = match result {
            Ok(result) => result,
            Err(error) => { host.fault = Some(error); return Err(error); }
        };
        if let Some(actor) = actor {
            if let Err(error) = self.delivery.replace_actor_state(expected_actor_revision, actor) {
                self.learned_host.as_mut().expect("owned learned source").fault = Some(error);
                return Err(error);
            }
        }
        self.learned_host.as_mut().expect("owned learned source").fault = None;
        guard.confirm();
        Ok(event)
    }
}
