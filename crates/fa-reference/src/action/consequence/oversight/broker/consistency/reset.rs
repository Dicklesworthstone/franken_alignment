//! A bootstrap-only selection of the original learned source's reset lineage.
use super::OversightBroker;
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;

impl OversightBroker {
    pub(crate) fn enable_owned_learned_forecast_restart_audit(&mut self) -> Result<(), Error> {
        let state = self.consistency.as_ref().ok_or(Error::Incomplete)?;
        if state.owned_restart_audit { return Err(Error::Duplicate); }
        if !state.follow_learned_resets || !self.owned_learned_consistency_required()
            || self.learned_text_stream_required() { return Err(Error::Binding); }
        let source = self.hosted_learned_generation()?;
        if source.position != 0 || source.cumulative_work.admitted_tokens != 0
            || source.host_failure.is_some() || source.availability != LearnedAvailability::Empty
            || state.jobs != 0 || state.pending.is_some() || !state.observations.is_empty()
            || state.coverage_lost || state.evidence.samples() != 0
            || !self.inputs.is_empty() || !self.started_rounds.is_empty()
            || self.inspect().sequence != 0 || self.inspect().suspended
            || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        self.consistency.as_mut().expect("validated owned predictor").owned_restart_audit = true;
        Ok(())
    }

    pub(crate) fn owned_learned_forecast_restart_audit_required(&self) -> bool {
        self.consistency.as_ref().is_some_and(|state| state.owned_restart_audit)
    }

    // A new source audit cannot erase a pending statistical obligation, a gap,
    // spent lifetime budget or an already observed/sample-derived outcome. This
    // prompt-boundary profile deliberately has no same-origin deduplication API.
    pub(crate) fn check_owned_learned_forecast_restart(&self) -> Result<(), Error> {
        let Some(state) = self.consistency.as_ref().filter(|state| state.owned_restart_audit)
            else { return Ok(()); };
        if state.pending.is_some() || state.coverage_lost || state.evidence.crossed()
            || !self.owned_learned_consistency_required()
            || self.learned_consistency_has_unreported_work()?
        { return Err(Error::Incomplete); }
        if self.hosted_learned_generation()?.cumulative_work.sampling_attempts != 0 {
            return Err(Error::WrongState);
        }
        Ok(())
    }

    // Only the independently versioned durable recipe calls this after installing
    // its pinned predictor. No public live API may reinterpret old forecast events.
    pub(crate) fn enable_learned_forecast_reset_successors(&mut self) -> Result<(), Error> {
        let state = self.consistency.as_ref().ok_or(Error::Incomplete)?;
        if state.follow_learned_resets { return Err(Error::Duplicate); }
        let owned = self.owned_learned_consistency_required();
        if self.decoder_host.is_some()
            || (!owned && (state.learned.is_some() || state.hosted_layer.is_none()))
        { return Err(Error::Binding); }
        let source = self.hosted_learned_generation()?;
        if source.position != 0 || source.cumulative_work.admitted_tokens != 0
            || source.host_failure.is_some() || source.availability != LearnedAvailability::Empty
            || state.jobs != 0 || state.pending.is_some() || !state.observations.is_empty()
            || state.coverage_lost || state.evidence.samples() != 0
            || !self.inputs.is_empty() || !self.started_rounds.is_empty()
            || self.inspect().sequence != 0 || self.inspect().suspended
            || self.stop_receipt().is_some()
        { return Err(Error::WrongState); }
        if self.hosted_learned_observation()?.stream() != state.config.stream {
            return Err(Error::Binding);
        }
        self.consistency.as_mut().expect("validated original forecast lane").follow_learned_resets = true;
        Ok(())
    }
}
