//! Source-checked K/V forecasts in the ORIGINAL broker's one-shot action lane.
//! No probabilities, pre-refined views, clock overrides or extra risk process.
use super::{ConsistencyConfig, ConsistencyObservation, OversightBroker};
use crate::action::consequence::activation::FrameIdentity;
use crate::action::consequence::activation::consistency::{BinaryForecast, Prediction,
    learned::{LearnedForecastModel, LearnedForecastReport, LearnedPrediction}};
use crate::action::consequence::activation::monitor::learned::LearnedMonitorBudget;
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvRow};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::Error;

/// Bootstrap-only source selection and numerical limits. The original model,
/// event registration, age limit, job count and lifetime alpha remain in force.
/// Layer and K/V side are fixed; the current actor prefix fixes the position.
#[derive(Debug)]
pub struct LearnedConsistencyConfig {
    pub consistency: ConsistencyConfig,
    pub layer: u64,
    pub side: KvSide,
    pub budget: LearnedMonitorBudget,
}

/// Same observed-action metadata, but the actual learned observation and costs.
/// A raw observation is never manufactured to fit the existing raw accessor.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::consistency::{
///     ConsistencyObservation, LearnedConsistencyObservation};
/// fn relabel(observation: LearnedConsistencyObservation) -> ConsistencyObservation { observation }
/// ```
pub type LearnedConsistencyObservation = ConsistencyObservation<LearnedPrediction>;

#[derive(Debug)]
pub(super) struct LearnedLane {
    model: LearnedForecastModel,
    layer: u64,
    side: KvSide,
}
#[derive(Clone, Debug)]
pub(super) enum ForecastEvidence { Raw(Prediction), Learned(LearnedPrediction) }
impl ForecastEvidence {
    pub(super) fn frame(&self) -> FrameIdentity {
        match self { Self::Raw(p) => p.observation().frame(), Self::Learned(p) => p.observation().frame() }
    }
    pub(super) fn forecast(&self) -> BinaryForecast {
        match self { Self::Raw(p) => p.forecast(), Self::Learned(p) => p.forecast() }
    }
}
#[derive(Clone, Debug)]
pub(super) enum ForecastRecord { Raw(ConsistencyObservation), Learned(LearnedConsistencyObservation) }
impl ForecastRecord {
    pub(super) fn policy_epoch(&self) -> u64 {
        match self { Self::Raw(o) => o.policy_epoch(), Self::Learned(o) => o.policy_epoch() }
    }
    pub(super) fn observed_actor_revision(&self) -> u64 {
        match self { Self::Raw(o) => o.observed_actor_revision(), Self::Learned(o) => o.observed_actor_revision() }
    }
}

impl OversightBroker {
    /// Install atomically through the original bootstrap validator. There is no
    /// conversion of an existing lane, source-mode switch or alpha refresh. The
    /// scalar progressive mode is incompatible, not silently discarded.
    pub fn enable_learned_action_consistency(&mut self, config: LearnedConsistencyConfig)
        -> Result<(), Error>
    {
        if config.layer == 0 { return Err(Error::InvalidInput); }
        let model = config.consistency.model.clone().into_learned(config.budget)?;
        self.enable_action_consistency(config.consistency)?;
        self.consistency.as_mut().expect("new consistency lane").learned =
            Some(LearnedLane { model, layer: config.layer, side: config.side });
        Ok(())
    }

    pub fn learned_action_consistency_required(&self) -> bool {
        self.consistency.as_ref().is_some_and(|state| state.learned.is_some())
    }

    /// Forecast before accepting proposal bytes. This is a caller-supplied,
    /// locally source-checked capture, NOT an authenticated owned-decoder tap.
    /// The complete cache must end at the current actor prefix: selecting an old
    /// row from a cache containing later tokens is not a pre-action capture.
    ///
    /// A returned report may refuse to certify a probability. Such an admitted
    /// failure spends the job, retains its source-sequence floor and permanently
    /// loses coverage; it never creates a pending forecast or a likelihood sample.
    /// The original optional stop policy runs on both success and refusal. An
    /// outer evaluation/containment error has no complete returned work receipt.
    pub fn forecast_learned_action(&mut self, attempt: u64, expected_actor_revision: u64,
        source: &CheckedLearnedKv, row: KvRow) -> Result<LearnedForecastReport, Error>
    {
        if !self.learned_action_consistency_required() { return Err(Error::Binding); }
        self.with_consistency_stop(|owner| owner.forecast_learned_observed(attempt, expected_actor_revision, source, row))
    }

    fn forecast_learned_observed(&mut self, attempt: u64, expected_actor_revision: u64,
        source: &CheckedLearnedKv, row: KvRow) -> Result<LearnedForecastReport, Error>
    {
        let lane = self.consistency.as_ref().and_then(|state| state.learned.as_ref()).ok_or(Error::Incomplete)?;
        if (row.layer, row.side) != (lane.layer, lane.side) { return Err(Error::Binding); }
        let (identity, heads, channels) = source.row_shape(row)?;
        let dimensions = heads.checked_mul(channels).ok_or(Error::Overflow)?;
        let end = row.position.checked_add(1).ok_or(Error::Overflow)?;
        for layer in source.descriptor().layers().values() {
            let count = u64::try_from(layer.token_count).map_err(|_| Error::Overflow)?;
            if layer.first_position.checked_add(count) != Some(end) { return Err(Error::Binding); }
        }
        let context = self.begin_consistency_forecast(attempt, expected_actor_revision, identity, dimensions)?;
        let state = self.consistency.as_mut().expect("admitted consistency lane");
        let report = match state.learned.as_ref().expect("frozen learned mode").model.predict(source, row) {
            Ok(report) => report,
            Err(error) => { state.coverage_lost = true; return Err(error); }
        };
        match report.prediction() {
            Ok(prediction) => self.finish_consistency_forecast(attempt, expected_actor_revision,
                context, ForecastEvidence::Learned(prediction.clone())),
            Err(_) => self.consistency.as_mut().expect("admitted consistency lane").coverage_lost = true,
        }
        Ok(report)
    }

    pub fn learned_consistency_observation(&self, attempt: u64)
        -> Result<&LearnedConsistencyObservation, Error>
    {
        match self.consistency.as_ref().ok_or(Error::Incomplete)?.observations.get(&attempt).ok_or(Error::Missing)? {
            ForecastRecord::Learned(observation) => Ok(observation),
            ForecastRecord::Raw(_) => Err(Error::Binding),
        }
    }
}

#[cfg(test)]
mod tests;
