//! Predict from the actual owned decoder, never a caller-supplied stand-in frame.
use super::OversightBroker;
use crate::action::consequence::activation::consistency::Prediction;
use crate::Error;

impl OversightBroker {
    /// Fix one actual residual tap before the first numerical token or forecast.
    /// A failed binding changes neither the predictor nor its source selection.
    /// The source mode is retained OUTSIDE numerical checkpoints, so a reset
    /// cannot reopen the raw-frame API or substitute a new predictor stream.
    pub fn require_hosted_action_consistency(&mut self, layer: u64) -> Result<(), Error> {
        let state = self.consistency.as_ref().ok_or(Error::Incomplete)?;
        if state.hosted_layer.is_some() { return Err(Error::Duplicate); }
        if state.jobs != 0 || state.pending.is_some() || state.coverage_lost
            || !self.inputs.is_empty() || !self.started_rounds.is_empty()
            || self.inspect().sequence != 0 || self.inspect().suspended
            || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        let profile = state.config.model.profile();
        let dimensions = state.config.model.dimensions();
        let stream = state.config.stream;
        self.bind_hosted_forecast_source(layer, profile, dimensions, stream)?;
        self.consistency.as_mut().expect("validated predictor").hosted_layer = Some(layer);
        Ok(())
    }

    pub fn hosted_consistency_layer(&self) -> Option<u64> {
        self.consistency.as_ref().and_then(|state| state.hosted_layer)
    }

    /// No source values, timestamp override, sequence override or model argument.
    /// The same native predictor computes the forecast from the current complete
    /// quiet token, with its actual stream, position and sequence. No numerical
    /// token is recomputed and no sampler draw is consumed by this method.
    /// Configured freshness, one-outstanding-forecast and lifetime bounds apply.
    pub fn forecast_hosted_action(&mut self, attempt: u64, expected_actor_revision: u64)
        -> Result<Prediction, Error>
    {
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        let layer = self.hosted_consistency_layer().ok_or(Error::Incomplete)?;
        let source = self.hosted_forecast_source(layer)?;
        self.forecast_action_inner(attempt, expected_actor_revision, &source)
    }
}
