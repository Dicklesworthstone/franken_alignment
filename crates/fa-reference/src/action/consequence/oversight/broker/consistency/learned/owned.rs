//! Forecast from the ORIGINAL generator's accepted, source-checked K/V audit.
//! Selection is frozen before numerical work; no caller chooses a capture/row.
use super::{CheckedLearnedKv, Error, KvRow, KvSide, LearnedConsistencyConfig,
    LearnedForecastReport, LearnedMonitorBudget, MAX_CHECKED_KV_BYTES, OversightBroker,
    scale_budget};
use crate::action::consequence::oversight::learned_source::LearnedAvailability;

impl OversightBroker {
    /// The existing lifetime ceilings, with the current original generation as
    /// the ONLY source. This neither performs inference nor fits another codec.
    pub fn enable_owned_learned_action_consistency(&mut self, config: LearnedConsistencyConfig)
        -> Result<(), Error>
    {
        let lifetime = scale_budget(config.budget, config.consistency.max_predictions)?;
        let bytes = MAX_CHECKED_KV_BYTES.checked_mul(config.consistency.max_predictions)
            .ok_or(Error::Overflow)?;
        self.enable_owned_learned_action_consistency_with_limits(config, lifetime, bytes)
    }

    /// Atomically bind layer, side, profile, dimensions and stream to the existing
    /// numerical owner BEFORE its first attempted token. Rewinding position alone
    /// cannot make spent cumulative work into a new bootstrap. No installed lane
    /// can be converted from supplied evidence or given a fresh lifetime budget.
    pub fn enable_owned_learned_action_consistency_with_limits(&mut self,
        config: LearnedConsistencyConfig, lifetime: LearnedMonitorBudget,
        max_retained_source_bytes: usize) -> Result<(), Error>
    {
        if self.action_consistency_required() { return Err(Error::Duplicate); }
        let state = self.hosted_learned_generation()?;
        if state.host_failure.is_some() || state.position != 0
            || state.cumulative_work.admitted_tokens != 0
            || state.availability != LearnedAvailability::Empty
            || self.inspect().suspended || self.stop_receipt().is_some() {
            return Err(Error::WrongState);
        }
        if config.layer == 0 { return Err(Error::InvalidInput); }
        let run = self.hosted_learned_original()?;
        // Original generator admission bound this codec profile to THIS model.
        // Metadata validation needs no generated token, reconstructed cache or
        // imported coefficients. The original forecast constructor validates the
        // probe/table and numerical allowances before the broker is changed.
        let layer = run.policy().codec().profile().layers().get(&config.layer).ok_or(Error::Binding)?;
        let tensor = match config.side { KvSide::Key => layer.keys(), KvSide::Value => layer.values() };
        if config.consistency.model.profile() != tensor.profile()
            || config.consistency.model.dimensions() != tensor.dimensions()
            || config.consistency.stream != self.hosted_learned_observation()?.stream() {
            return Err(Error::Binding);
        }
        self.enable_learned_action_consistency_with_limits(config, lifetime, max_retained_source_bytes)?;
        self.consistency.as_mut().expect("installed original lane").learned.as_mut()
            .expect("installed learned lane").owned = true;
        Ok(())
    }

    pub fn owned_learned_consistency_required(&self) -> bool {
        self.consistency.as_ref().and_then(|state| state.learned.as_ref()).is_some_and(|lane| lane.owned)
    }

    /// No source, row, probability, timestamp or numerical model argument. Only
    /// the latest accepted and completely quiet original event can supply this
    /// capture. The original forecast admission, lifetime accounting and optional
    /// containment execute unchanged; no generation draw or audit is repeated.
    ///
    /// This guarantees in-process owner provenance, not an authenticated remote
    /// tap, calibrated probabilities or forecast-before-sampling by itself.
    pub fn forecast_owned_learned_action(&mut self, attempt: u64, expected_actor_revision: u64)
        -> Result<LearnedForecastReport, Error>
    {
        if !self.owned_learned_consistency_required() { return Err(Error::Binding); }
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        let (source, row) = self.owned_learned_forecast_source()?;
        self.with_consistency_stop(|owner|
            owner.forecast_learned_observed(attempt, expected_actor_revision, &source, row))
    }

    fn owned_learned_forecast_source(&self) -> Result<(CheckedLearnedKv, KvRow), Error> {
        let lane = self.learned_consistency_lane()?;
        let state = self.hosted_learned_generation()?;
        if state.host_failure.is_some() || state.availability != LearnedAvailability::Ready {
            return Err(Error::Incomplete);
        }
        let run = self.hosted_learned_original()?;
        let event = run.last_event().ok_or(Error::Incomplete)?;
        let step = event.accepted().ok_or(Error::Incomplete)?;
        if event.status() != state.status || event.position() != step.position
            || step.position.checked_add(1) != Some(state.position)
            || run.position() != state.position || run.accepted_tokens().last() != Some(&step.token)
            || !event.audit().complete_quiet() || event.audit().end_position() != state.position {
            return Err(Error::Stale);
        }
        let source = event.audit().source();
        let row = KvRow { layer: lane.layer, side: lane.side, position: step.position };
        let (frame, _, _) = source.row_shape(row)?;
        if frame.stream != self.hosted_learned_observation()?.stream()
            || frame.sequence != state.position || frame.position != step.position {
            return Err(Error::Binding);
        }
        // This immutable clone shares the actual accepted audit, not a rebuilt
        // unchecked object. The original forecast charges its complete inventory
        // again and begins from the coarse view; no free refined view is adopted.
        Ok((source.clone(), row))
    }
}

#[cfg(test)]
mod tests;
