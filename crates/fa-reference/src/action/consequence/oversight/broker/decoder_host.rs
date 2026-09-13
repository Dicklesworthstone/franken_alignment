//! Run the actual monitored sampler inside the original effect-controller owner.
//! Numerical output is still evidence; the original congress and permits decide effects.

mod checkpoint;
mod automatic_stop;
pub use automatic_stop::{HostedStopCause, HostedStopIncident, HostedStopPolicy};
pub use checkpoint::{HostedCheckpointHandle, HostedRecoveryUsage, HostedResetReceipt, HostedResetRequest};

use super::{OversightBroker, decoder_gate::DecoderBindingLimits};
use crate::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus, MonitoringWork};
use crate::action::consequence::activation::monitor::decoder::sampled::{MonitoredSampledDecoder, MonitoredSampledStep};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderWork};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SampleBudget;
use crate::action::consequence::gate::containment::{ActorState, RestartProfile, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES};
use crate::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedDecoderInspection {
    pub actor_revision: u64,
    pub position: u64,
    pub sampled_draws: u64,
    pub status: MonitoringStatus,
    pub monitoring: MonitoringWork,
    pub numerical: DecoderWork,
    pub cache_bytes: usize,
    pub sampler_bytes: usize,
}

#[derive(Debug)]
pub(super) struct DecoderHost {
    run: MonitoredSampledDecoder,
    profile: RestartProfile,
    recovery: checkpoint::RecoveryState,
    automatic_stop: Option<automatic_stop::HostedStopState>,
}

impl OversightBroker {
    /// Consume the only numerical owner at trusted bootstrap. Actor state is
    /// captured through the existing all-layer cache codec and sampler encoding.
    /// This can attach an empty or already fully reviewed prefix, never a held
    /// owner. External ActorState replacement/reset cannot bypass this profile.
    /// There is no mutable decoder getter, reseeding API, or ownership extraction.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::oversight::OversightBroker;
    /// fn bypass(broker: &mut OversightBroker) { broker.hosted_decoder_mut(); }
    /// ```
    pub fn own_sampled_decoder(&mut self, run: MonitoredSampledDecoder, limits: DecoderBindingLimits) -> Result<(), Error> {
        if self.decoder_host.is_some() || self.decoder.is_some() { return Err(Error::Duplicate); }
        if self.inspect().suspended || !self.inputs.is_empty() || !self.started_rounds.is_empty()
            || self.inspect().sequence != 0 { return Err(Error::WrongState); }
        if run.status() != MonitoringStatus::Ready { return Err(Error::WrongState); }
        let revision = self.actor_revision();
        revision.checked_add(1).ok_or(Error::Overflow)?;
        let profile = self.delivery.controller().actor().profile();
        let captured = run.capture_host_state(MAX_CACHE_BYTES, MAX_SAMPLER_BYTES)?;
        let actor = ActorState::new(profile, captured.tokens, captured.cache, captured.sampler, captured.position)?;
        // Every remaining actor-update predicate has been preflighted. The
        // source bootstrap performs all identity/limit checks before mutation.
        self.enable_decoder_monitoring(run.observation(), limits)?;
        self.delivery.replace_actor_state(revision, actor).expect("preflighted owned actor update");
        self.decoder_host = Some(DecoderHost { run, profile, recovery: checkpoint::RecoveryState::new(), automatic_stop: None });
        Ok(())
    }

    /// Supervisor-only costs/state, without token IDs, logits, cache or RNG words.
    /// Numerical work includes completed abandoned runs and checkpoint replays.
    pub fn hosted_decoder(&self) -> Result<HostedDecoderInspection, Error> {
        let host = self.decoder_host.as_ref().ok_or(Error::Incomplete)?;
        let actor = self.delivery.controller().actor();
        Ok(HostedDecoderInspection { actor_revision: self.actor_revision(), position: host.run.position(),
            sampled_draws: host.run.sampled_draws(), status: host.run.status(),
            monitoring: host.run.monitoring_work(), numerical: host.recovery.cumulative(host.run.decoder_work())?,
            cache_bytes: actor.cache().len(), sampler_bytes: actor.sampler().len() })
    }

    pub fn advance_hosted_forced(&mut self, expected_actor_revision: u64, expected_position: u64,
        token: u32, budget: DecoderBudget) -> Result<MonitoredStep, Error>
    {
        self.with_hosted_stop(|owner| owner.advance_hosted(expected_actor_revision, expected_position,
            |run| run.advance_forced(expected_position, token, budget)))
    }

    pub fn advance_hosted_sampled(&mut self, expected_actor_revision: u64, expected_position: u64,
        budget: SampleBudget) -> Result<MonitoredSampledStep, Error>
    {
        self.with_hosted_stop(|owner| owner.advance_hosted(expected_actor_revision, expected_position,
            |run| run.advance_sampled(expected_position, budget)))
    }

    // Only the two methods above can supply this callback. No caller code runs
    // between numerical computation, state capture, and the original host update.
    fn advance_hosted<T>(&mut self, expected_revision: u64, position: u64,
        advance: impl FnOnce(&mut MonitoredSampledDecoder) -> Result<T, Error>) -> Result<T, Error>
    {
        if self.inspect().suspended { return Err(Error::WrongState); }
        if expected_revision != self.actor_revision() { return Err(Error::Stale); }
        expected_revision.checked_add(1).ok_or(Error::Overflow)?;
        let host = self.decoder_host.as_mut().ok_or(Error::Incomplete)?;
        if position != host.run.position() { return Err(Error::Stale); }
        host.run.check_host_state_size(position.checked_add(1).ok_or(Error::Overflow)?, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES)?;
        let result = advance(&mut host.run)?;
        // A computed-but-held step also advances actual state and sampled draws.
        // Its source remains held and neither the token nor state bytes escape.
        let state = host.run.capture_host_state(MAX_CACHE_BYTES, MAX_SAMPLER_BYTES).and_then(|captured| {
            ActorState::new(host.profile, captured.tokens, captured.cache, captured.sampler, captured.position)
        });
        let state = match state {
            Ok(state) => state,
            Err(error) => { host.run.fail_host(error); return Err(error); }
        };
        if let Err(error) = self.delivery.replace_actor_state(expected_revision, state) {
            host.run.fail_host(error);
            return Err(error);
        }
        Ok(result)
    }
}
