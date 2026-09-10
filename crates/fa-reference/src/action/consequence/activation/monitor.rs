//! Bounded refinement of a fixed probe family. Quiet is an observation, never a
//! permit. Ambiguity, threshold equality and exhausted capacity remain holds.
//! Source validation is local here; encoded byte counts are not network timings.

use super::{CaptureProfile, FrameIdentity, ProgressiveFrame, SourceFrame};
use super::probe::{LinearProbe, ProbeIdentity, ProbeObservation, ProbeOutcome};
use crate::Error;
use std::collections::BTreeSet;

pub const MAX_PROBES: usize = 16;
pub const MAX_LEVELS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefinementBudget {
    /// Actual encoded blocks, including headers and padding. Raw capture and
    /// the source-checker's extra encoding pass are separately reported work.
    pub encoded_bytes: usize,
    pub probe_coordinates: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorOutcome {
    NoAlarm,
    Alarm,
    AtThreshold,
    BudgetExhausted,
    Unresolved,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefinementStep {
    pub mantissa_bits: u8,
    pub encoded_bytes: usize,
    /// Only previously unresolved probes are reevaluated at a later rung.
    pub observations: Vec<ProbeObservation>,
}

/// A reproducible result of actual encoding, source checking, decoding and
/// exact probe evaluation. It contains no actor state, live gate or Permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefinementReport {
    frame: FrameIdentity,
    probes: Vec<ProbeIdentity>,
    outcome: MonitorOutcome,
    steps: Vec<RefinementStep>,
    raw_bytes: usize,
    encoded_bytes: usize,
    probe_coordinates: usize,
    codec_coordinates: usize,
}

impl RefinementReport {
    pub fn frame(&self) -> FrameIdentity { self.frame }
    pub fn probes(&self) -> &[ProbeIdentity] { &self.probes }
    pub fn outcome(&self) -> MonitorOutcome { self.outcome }
    pub fn steps(&self) -> &[RefinementStep] { &self.steps }
    pub fn raw_bytes(&self) -> usize { self.raw_bytes }
    pub fn encoded_bytes(&self) -> usize { self.encoded_bytes }
    pub fn probe_coordinates(&self) -> usize { self.probe_coordinates }
    /// Encode + independent source re-encode + decode, counted by coordinates.
    /// This excludes header scans, allocations and probe arithmetic.
    pub fn codec_coordinates(&self) -> usize { self.codec_coordinates }
}

/// Fixed at construction: neither evidence nor a model may lower the fidelity
/// ladder, drop a probe or enlarge the budget during this analysis.
#[derive(Clone, Debug)]
pub struct RefinementMonitor {
    profile: CaptureProfile,
    dimensions: usize,
    probes: Vec<LinearProbe>,
    levels: Vec<u8>,
    budget: RefinementBudget,
}

impl RefinementMonitor {
    pub fn new(probes: Vec<LinearProbe>, levels: Vec<u8>, budget: RefinementBudget) -> Result<Self, Error> {
        if probes.is_empty() || levels.is_empty() { return Err(Error::InvalidInput); }
        if probes.len() > MAX_PROBES || levels.len() > MAX_LEVELS { return Err(Error::Limit); }
        if levels.last() != Some(&23) || levels.iter().any(|v| *v > 23)
            || levels.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::InvalidInput);
        }
        let identity = probes[0].identity();
        let mut names = BTreeSet::new();
        for probe in &probes {
            let current = probe.identity();
            if current.profile != identity.profile || current.dimensions != identity.dimensions {
                return Err(Error::Binding);
            }
            if !names.insert(current.id) { return Err(Error::Duplicate); }
        }
        Ok(Self { profile: identity.profile, dimensions: identity.dimensions, probes, levels, budget })
    }

    pub fn profile(&self) -> CaptureProfile { self.profile }
    pub fn dimensions(&self) -> usize { self.dimensions }

    pub fn analyze(&self, source: &SourceFrame) -> Result<RefinementReport, Error> {
        if source.identity().profile != self.profile || source.dimensions() != self.dimensions {
            return Err(Error::Binding);
        }
        let mut report = RefinementReport {
            frame: source.identity(), probes: self.probes.iter().map(LinearProbe::identity).collect(),
            outcome: MonitorOutcome::Unresolved, steps: Vec::new(), raw_bytes: source.raw_bytes(),
            encoded_bytes: 0, probe_coordinates: 0, codec_coordinates: 0,
        };
        let mut latest = vec![None; self.probes.len()];
        let mut view: Option<ProgressiveFrame> = None;
        for bits in &self.levels {
            let from = view.as_ref().map(ProgressiveFrame::mantissa_bits);
            let length = source.encoded_len(from, *bits)?;
            let unresolved = latest.iter().filter(|v| matches!(v, None | Some(ProbeOutcome::NeedsRefinement))).count();
            let coordinates = self.dimensions.checked_mul(unresolved).ok_or(Error::Overflow)?;
            let total_bytes = report.encoded_bytes.checked_add(length).ok_or(Error::Overflow)?;
            let total_coordinates = report.probe_coordinates.checked_add(coordinates).ok_or(Error::Overflow)?;
            if total_bytes > self.budget.encoded_bytes || total_coordinates > self.budget.probe_coordinates {
                report.outcome = MonitorOutcome::BudgetExhausted;
                return Ok(report);
            }
            let bytes = match from {
                None => source.encode_initial(*bits)?,
                Some(from) => source.encode_refinement(from, *bits)?,
            };
            let checked = source.verify_block(&bytes)?;
            match &mut view {
                None => view = Some(ProgressiveFrame::from_initial(&checked)?),
                Some(view) => view.refine(&checked)?,
            }
            let current = view.as_ref().ok_or(Error::Incomplete)?;
            let mut observations = Vec::with_capacity(unresolved);
            for (index, probe) in self.probes.iter().enumerate() {
                if matches!(latest[index], None | Some(ProbeOutcome::NeedsRefinement)) {
                    let observation = probe.evaluate(current)?;
                    latest[index] = Some(observation.outcome());
                    observations.push(observation);
                }
            }
            report.encoded_bytes = total_bytes;
            report.probe_coordinates = total_coordinates;
            report.codec_coordinates = report.codec_coordinates.checked_add(self.dimensions * 3).ok_or(Error::Overflow)?;
            report.steps.push(RefinementStep { mantissa_bits: *bits, encoded_bytes: bytes.len(), observations });
            let outcome = if latest.contains(&Some(ProbeOutcome::CertifiedAlarm)) {
                Some(MonitorOutcome::Alarm)
            } else if latest.contains(&Some(ProbeOutcome::AtThreshold)) {
                Some(MonitorOutcome::AtThreshold)
            } else if latest.iter().all(|v| *v == Some(ProbeOutcome::CertifiedQuiet)) {
                Some(MonitorOutcome::NoAlarm)
            } else { None };
            if let Some(outcome) = outcome { report.outcome = outcome; return Ok(report); }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> FrameIdentity {
        FrameIdentity { profile: CaptureProfile { tenant: 1, model: 2, model_generation: 3,
            tap: 4, layout_generation: 5 }, stream: 6, sequence: 7, position: 0 }
    }
    fn detector(id: u64, weight: f32, threshold: f32) -> LinearProbe {
        LinearProbe::new(id, 1, identity().profile, &[weight], 0.0, threshold).unwrap()
    }
    fn budget() -> RefinementBudget { RefinementBudget { encoded_bytes: 4096, probe_coordinates: 100 } }

    #[test]
    fn coarse_quiet_stops_without_requesting_unneeded_refinement() {
        let monitor = RefinementMonitor::new(vec![detector(1, -1.0, 0.0)], vec![0, 8, 23], budget()).unwrap();
        let source = SourceFrame::capture(identity(), &[1.5]).unwrap();
        let report = monitor.analyze(&source).unwrap();
        assert_eq!(report.outcome(), MonitorOutcome::NoAlarm);
        assert_eq!(report.steps().len(), 1);
        assert_eq!(report.probe_coordinates(), 1);
        assert_eq!(report.codec_coordinates(), 3);
        assert_eq!(report.encoded_bytes(), source.encode_initial(0).unwrap().len());
    }

    #[test]
    fn rare_alarm_reaches_exact_residual_while_resolved_probe_is_reused() {
        let monitor = RefinementMonitor::new(vec![detector(1, 1.0, 0.0), detector(2, 0.0, 1.0)],
            vec![0, 8, 23], budget()).unwrap();
        let source = SourceFrame::capture(identity(), &[f32::from_bits(1)]).unwrap();
        let report = monitor.analyze(&source).unwrap();
        assert_eq!(report.outcome(), MonitorOutcome::Alarm);
        assert_eq!(report.steps().len(), 3);
        assert_eq!(report.steps()[0].observations.len(), 2);
        assert_eq!(report.steps()[1].observations.len(), 1);
        assert_eq!(report.steps()[2].observations.len(), 1);
        assert_eq!(report.probe_coordinates(), 4);
        assert_eq!(report.codec_coordinates(), 9);
        assert_eq!(report.encoded_bytes(), source.encoded_len(None, 0).unwrap()
            + source.encoded_len(Some(0), 8).unwrap() + source.encoded_len(Some(8), 23).unwrap());
    }

    #[test]
    fn budget_is_checked_before_each_encoded_block_and_probe_pass() {
        let source = SourceFrame::capture(identity(), &[f32::from_bits(1)]).unwrap();
        let initial = source.encoded_len(None, 0).unwrap();
        for limits in [RefinementBudget { encoded_bytes: initial, probe_coordinates: 99 },
            RefinementBudget { encoded_bytes: 4096, probe_coordinates: 1 }]
        {
            let monitor = RefinementMonitor::new(vec![detector(1, 1.0, 0.0)], vec![0, 23], limits).unwrap();
            let report = monitor.analyze(&source).unwrap();
            assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted);
            assert_eq!(report.steps().len(), 1);
            assert_eq!(report.encoded_bytes(), initial);
            assert_eq!(report.probe_coordinates(), 1);
        }
        let monitor = RefinementMonitor::new(vec![detector(1, 1.0, 0.0)], vec![0, 23],
            RefinementBudget { encoded_bytes: initial - 1, probe_coordinates: 99 }).unwrap();
        let report = monitor.analyze(&source).unwrap();
        assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted);
        assert!(report.steps().is_empty());
        assert_eq!(report.codec_coordinates(), 0);
    }

    #[test]
    fn threshold_equality_never_becomes_quiet_and_ladders_cannot_skip_exact_escape() {
        let monitor = RefinementMonitor::new(vec![detector(1, 1.0, 1.0)], vec![0, 23], budget()).unwrap();
        let source = SourceFrame::capture(identity(), &[1.0]).unwrap();
        assert_eq!(monitor.analyze(&source).unwrap().outcome(), MonitorOutcome::AtThreshold);
        for ladder in [vec![], vec![0, 8], vec![0, 8, 8, 23], vec![24, 23]] {
            assert!(RefinementMonitor::new(vec![detector(1, 1.0, 0.0)], ladder, budget()).is_err());
        }
        assert!(RefinementMonitor::new(vec![detector(1, 1.0, 0.0), detector(1, -1.0, 0.0)], vec![23], budget()).is_err());
    }
}
