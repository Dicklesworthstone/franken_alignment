//! A bootstrap-only selection of the original learned source's reset lineage.
use super::OversightBroker;
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;

impl OversightBroker {
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
