//! Forecast bands over ORIGINAL source-checked low-rank K/V evidence.
//! The original monitor owns selective XOR refinement and all scoring costs.
//! This numerical component has no request ledger, sampler or publication key.
use super::{BinaryForecast, CaptureProfile, ForecastModel, ProbeOutcome};
use crate::action::consequence::activation::monitor::{MonitorOutcome,
    learned::{LearnedMonitorBudget, LearnedMonitorReport, LearnedMonitorWork, LearnedRefinementMonitor}};
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv,
    KvRow, LearnedProbeObservation};
use crate::Error;

/// Frozen original coefficients, calibration table and refinement allowance.
/// Unlike mantissa ladders, this source uses its fitted codebook, checked error
/// envelopes and retained exact XOR groups. The budget cannot change mid-call.
#[derive(Clone, Debug)]
pub struct LearnedForecastModel {
    model: ForecastModel,
    monitor: LearnedRefinementMonitor,
}

/// A distinct learned observation, not a relabelled raw frame or mantissa count.
/// The view retains the source and actual refinement identities used to certify
/// this interval. This certifies a band, not the probability's calibration.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::consistency::{Prediction, learned::LearnedPrediction};
/// fn relabel(value: LearnedPrediction) -> Prediction { value }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::activation::consistency::learned::LearnedPrediction};
/// fn allow(value: LearnedPrediction) -> Permit { value }
/// ```
#[derive(Clone, Debug)]
pub struct LearnedPrediction {
    observation: LearnedProbeObservation,
    forecast: BinaryForecast,
    domain: u64,
    generation: u64,
    policy_generation: u64,
    work: LearnedMonitorWork,
}
impl LearnedPrediction {
    pub fn observation(&self) -> &LearnedProbeObservation { &self.observation }
    pub fn forecast(&self) -> BinaryForecast { self.forecast }
    pub fn domain(&self) -> u64 { self.domain }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn policy_generation(&self) -> u64 { self.policy_generation }
    /// Base representation plus every residual used, not just the last block.
    /// Fitting, initial compression/source checking and retained storage are
    /// separate upstream costs; no reuse discount is silently claimed here.
    pub fn work(&self) -> LearnedMonitorWork { self.work }
    pub fn encoded_bytes(&self) -> usize { self.work.encoded_bytes }
}

/// Preserve the ORIGINAL report even when no probability can be certified.
/// Callers cannot replace the report's interval, costs or failure with success.
/// Outer prediction errors below are preflight/evaluation failures with no
/// completed report; they must not be reported as zero-cost successful work.
#[derive(Clone, Debug)]
pub struct LearnedForecastReport {
    monitor: LearnedMonitorReport,
    prediction: Result<LearnedPrediction, Error>,
}
impl LearnedForecastReport {
    pub fn monitor(&self) -> &LearnedMonitorReport { &self.monitor }
    pub fn prediction(&self) -> Result<&LearnedPrediction, Error> {
        self.prediction.as_ref().map_err(|error| *error)
    }
    pub fn work(&self) -> LearnedMonitorWork { self.monitor.work() }
}

impl ForecastModel {
    /// Choose learned-source acquisition explicitly. Preserve this exact probe
    /// and table, using the original one-probe refinement monitor. An installed
    /// binary32 ladder is rejected rather than silently ignored or reinterpreted
    /// as a residual budget. Neither mode supplies a fitted/calibrated predictor.
    pub fn into_learned(self, budget: LearnedMonitorBudget) -> Result<LearnedForecastModel, Error> {
        if self.progressive.is_some() { return Err(Error::Binding); }
        let monitor = LearnedRefinementMonitor::new(vec![self.probe.clone()], budget)?;
        Ok(LearnedForecastModel { model: self, monitor })
    }
}
impl LearnedForecastModel {
    pub fn profile(&self) -> CaptureProfile { self.model.profile() }
    pub fn dimensions(&self) -> usize { self.model.dimensions() }
    pub fn policy_generation(&self) -> u64 { self.model.policy_generation() }
    pub fn event(&self, payload: &[u8]) -> bool { self.model.event(payload) }
    pub fn budget(&self) -> LearnedMonitorBudget { self.monitor.budget() }

    /// Judge an actual checked row; callers cannot import approximate values,
    /// a pre-refined free view, an MSE claim, a score or an outcome. The original
    /// monitor may stop on a certified coarse band or selectively refine exact
    /// groups. Missing residuals and exhausted allowances produce no forecast.
    /// No full reconstruction or raw-frame fallback is performed on refusal.
    pub fn predict(&self, source: &CheckedLearnedKv, row: KvRow)
        -> Result<LearnedForecastReport, Error>
    {
        let monitor = self.monitor.analyze(source, row)?;
        let prediction = self.certify(&monitor);
        Ok(LearnedForecastReport { monitor, prediction })
    }

    fn certify(&self, monitor: &LearnedMonitorReport) -> Result<LearnedPrediction, Error> {
        let expected = match monitor.outcome() {
            MonitorOutcome::NoAlarm => ProbeOutcome::CertifiedQuiet,
            MonitorOutcome::Alarm => ProbeOutcome::CertifiedAlarm,
            MonitorOutcome::AtThreshold => ProbeOutcome::AtThreshold,
            MonitorOutcome::BudgetExhausted => return Err(Error::Limit),
            MonitorOutcome::Unresolved => return Err(Error::Incomplete),
        };
        // Exactly one frozen probe. Never recover a probability from an older
        // observation if the complete monitor result was unresolved/exhausted.
        let step = monitor.steps().last().ok_or(Error::Incomplete)?;
        if step.observations.len() != 1 { return Err(Error::Binding); }
        let observation = &step.observations[0];
        if observation.probe() != self.model.probe.identity()
            || observation.frame() != monitor.frame() || observation.row() != monitor.row()
            || observation.outcome() != expected { return Err(Error::Binding); }
        Ok(LearnedPrediction { observation: observation.clone(),
            forecast: self.model.forecast_band(expected)?, domain: self.model.registration.domain,
            generation: self.model.registration.generation,
            policy_generation: self.model.registration.policy_generation, work: monitor.work() })
    }
}

#[cfg(test)]
mod tests;
