//! Source-checked K/V forecasts in the ORIGINAL broker's one-shot action lane.
//! No probabilities, pre-refined views, clock overrides or extra risk process.
use super::{ConsistencyConfig, ConsistencyObservation, OversightBroker};
use crate::action::consequence::activation::FrameIdentity;
use crate::action::consequence::activation::consistency::{BinaryForecast, Prediction,
    learned::{LearnedForecastModel, LearnedForecastReport, LearnedPrediction}};
use crate::action::consequence::activation::monitor::learned::{LearnedMonitorBudget, LearnedMonitorWork};
use crate::action::consequence::activation::probe::learned::MAX_CHECKED_KV_BYTES;
use std::collections::BTreeMap;
mod lifetime;
mod owned;
use lifetime::{add_work, allowance, scale_budget};
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
    owned: bool,
    layer: u64,
    side: KvSide,
    lifetime: LearnedMonitorBudget,
    retained_limit: usize,
    work: LearnedMonitorWork,
    retained_source_bytes: usize,
    unreported_work: bool,
    reports: BTreeMap<u64, LearnedForecastReport>,
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
        // Preserve the existing per-job contract, with a checked finite lifetime
        // upper bound. Tighter supervisor limits are available at bootstrap below.
        let lifetime = scale_budget(config.budget, config.consistency.max_predictions)?;
        let retained_limit = MAX_CHECKED_KV_BYTES.checked_mul(config.consistency.max_predictions)
            .ok_or(Error::Overflow)?;
        self.enable_learned_action_consistency_with_limits(config, lifetime, retained_limit)
    }

    /// Freeze independent lifetime work and retained-source limits atomically.
    /// Zero allowances are valid refusal policies, not unlimited sentinels.
    /// Lifetime limits may exceed a per-job cap; each acquisition intersects
    /// its remainder with the model's original per-job limits before execution.
    pub fn enable_learned_action_consistency_with_limits(&mut self,
        config: LearnedConsistencyConfig, lifetime: LearnedMonitorBudget,
        max_retained_source_bytes: usize) -> Result<(), Error>
    {
        if config.layer == 0 { return Err(Error::InvalidInput); }
        let model = config.consistency.model.clone().into_learned(config.budget)?;
        let lane = LearnedLane { model, owned: false, layer: config.layer, side: config.side,
            lifetime, retained_limit: max_retained_source_bytes,
            work: LearnedMonitorWork::default(), retained_source_bytes: 0,
            unreported_work: false, reports: BTreeMap::new() };
        self.enable_action_consistency(config.consistency)?;
        self.consistency.as_mut().expect("new consistency lane").learned = Some(lane);
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
        if !self.learned_action_consistency_required() || self.owned_learned_consistency_required() {
            return Err(Error::Binding);
        }
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
        // Close eligibility before numerical work or retained-history allocation.
        // An error or unwind must not leave the earlier quiet process usable.
        state.coverage_lost = true;
        let lane = state.learned.as_mut().expect("frozen learned mode");
        let retained = lane.retained_source_bytes.checked_add(source.report().total_encoded_bytes)
            .ok_or(Error::Limit)?;
        if retained > lane.retained_limit { return Err(Error::Limit); }
        let remaining = allowance(lane.lifetime, lane.work, lane.model.budget())?;
        // Charge the complete checked inventory, including unused residuals.
        // No deduplication or source-sharing discount is assumed. A failed
        // admitted evaluation keeps this conservative reservation permanently.
        lane.retained_source_bytes = retained;
        lane.unreported_work = true;
        let report = lane.model.predict_with_budget(source, row, remaining)?;
        let work = add_work(lane.work, report.work());
        // Preserve completed refusals too, including when later containment
        // prevents the caller from receiving this report through the return path.
        lane.reports.insert(attempt, report.clone());
        lane.work = work?;
        allowance(lane.lifetime, lane.work, lane.model.budget())?;
        lane.unreported_work = false;
        if let Ok(prediction) = report.prediction() {
            self.finish_consistency_forecast(attempt, expected_actor_revision,
                context, ForecastEvidence::Learned(prediction.clone()));
            self.consistency.as_mut().expect("published learned forecast").coverage_lost = false;
        }
        Ok(report)
    }

    /// Completed numerical acquisition totals, including refused reports.
    /// Check has_unreported_work before treating these as complete acquisition
    /// costs. Fitting, capture, source checking and containment are not included.
    pub fn learned_consistency_work(&self) -> Result<LearnedMonitorWork, Error> {
        Ok(self.learned_consistency_lane()?.work)
    }

    /// Conservative charged inventory bytes, not a byte-exact allocator measure.
    /// Includes all retained residuals and reservations for admitted outer errors.
    pub fn learned_consistency_retained_source_bytes(&self) -> Result<usize, Error> {
        Ok(self.learned_consistency_lane()?.retained_source_bytes)
    }

    /// An admitted evaluation or accounting failure lacked a complete trusted
    /// receipt. The lane cannot be re-armed to clear this or refresh its budget.
    pub fn learned_consistency_has_unreported_work(&self) -> Result<bool, Error> {
        Ok(self.learned_consistency_lane()?.unreported_work)
    }

    /// Original typed acquisition receipt, whether it certified or refused.
    pub fn learned_consistency_report(&self, attempt: u64) -> Result<&LearnedForecastReport, Error> {
        self.learned_consistency_lane()?.reports.get(&attempt).ok_or(Error::Missing)
    }

    fn learned_consistency_lane(&self) -> Result<&LearnedLane, Error> {
        self.consistency.as_ref().and_then(|state| state.learned.as_ref()).ok_or(Error::Incomplete)
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

#[cfg(test)]
mod lifetime_tests;
