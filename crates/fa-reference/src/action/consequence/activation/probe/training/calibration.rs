//! Threshold selection and untouched evaluation of the emitted exact probe.
//! Count criteria are empirical operating points, not probability calibration,
//! confidence bounds, a learned-label guarantee or production qualification.

use super::{CaseLabel, CaseOrigin, ClassCounts, DataSplit, FittedProbe, MAX_CORPUS_CASES, MAX_CORPUS_COORDINATES};
use super::super::{ExactScore, LinearProbe, SignedSum};
use super::super::super::{FrameIdentity, ProgressiveFrame, HEADER_BYTES};
use crate::Error;
use std::cmp::Ordering;
use std::rc::Rc;

pub const MAX_THRESHOLD_CANDIDATES: usize = 256;
pub const MAX_SCORING_BYTES: usize = 16_777_216;
pub const MAX_THRESHOLD_COMPARISONS: usize = MAX_CORPUS_CASES * MAX_THRESHOLD_CANDIDATES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreeningCriteria { min_violation_alarms: usize, max_benign_holds: usize }
impl ScreeningCriteria {
    /// Equality is a hold but NOT a demonstrated alarm. Requiring at least one
    /// true alarm prevents qualification of a detector that always stays quiet.
    pub fn new(min_violation_alarms: usize, max_benign_holds: usize) -> Result<Self, Error> {
        if min_violation_alarms == 0 { return Err(Error::InvalidInput); }
        if min_violation_alarms > MAX_CORPUS_CASES || max_benign_holds > MAX_CORPUS_CASES { return Err(Error::Limit); }
        Ok(Self { min_violation_alarms, max_benign_holds })
    }
    pub fn min_violation_alarms(self) -> usize { self.min_violation_alarms }
    pub fn max_benign_holds(self) -> usize { self.max_benign_holds }
    fn accepts(self, counts: ConfusionCounts) -> bool {
        counts.violation_alarm >= self.min_violation_alarms && counts.benign_holds() <= self.max_benign_holds
    }
}

/// Both operating-point and final-evaluation criteria freeze before any scores
/// are computed by this campaign. No evaluation result can change this object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalibrationPolicy {
    id: u64,
    generation: u64,
    threshold_bits: Vec<u32>,
    calibration: ScreeningCriteria,
    evaluation: ScreeningCriteria,
}
impl CalibrationPolicy {
    pub fn new(id: u64, generation: u64, thresholds: &[f32], calibration: ScreeningCriteria,
        evaluation: ScreeningCriteria) -> Result<Self, Error>
    {
        if id == 0 || generation == 0 || thresholds.is_empty() { return Err(Error::InvalidInput); }
        if thresholds.len() > MAX_THRESHOLD_CANDIDATES { return Err(Error::Limit); }
        if thresholds.iter().any(|v| !v.is_finite()) || thresholds.windows(2).any(|p| p[0] >= p[1]) {
            return Err(Error::InvalidInput);
        }
        Ok(Self { id, generation, threshold_bits: thresholds.iter().map(|v| v.to_bits()).collect(),
            calibration, evaluation })
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn thresholds(&self) -> impl Iterator<Item = f32> + '_ { self.threshold_bits.iter().map(|v| f32::from_bits(*v)) }
    pub fn calibration_criteria(&self) -> ScreeningCriteria { self.calibration }
    pub fn evaluation_criteria(&self) -> ScreeningCriteria { self.evaluation }
}

/// Every case appears in exactly one cell. At-threshold cases never turn into
/// quiet results or disappear from either class denominator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConfusionCounts {
    pub benign_alarm: usize,
    pub benign_quiet: usize,
    pub benign_boundary: usize,
    pub violation_alarm: usize,
    pub violation_quiet: usize,
    pub violation_boundary: usize,
}
impl ConfusionCounts {
    pub fn classes(self) -> ClassCounts {
        ClassCounts { benign: self.benign_alarm + self.benign_quiet + self.benign_boundary,
            violation: self.violation_alarm + self.violation_quiet + self.violation_boundary }
    }
    pub fn benign_holds(self) -> usize { self.benign_alarm + self.benign_boundary }
    fn record(&mut self, label: CaseLabel, ordering: Ordering) {
        match (label, ordering) {
            (CaseLabel::Benign, Ordering::Greater) => self.benign_alarm += 1,
            (CaseLabel::Benign, Ordering::Less) => self.benign_quiet += 1,
            (CaseLabel::Benign, Ordering::Equal) => self.benign_boundary += 1,
            (CaseLabel::Violation, Ordering::Greater) => self.violation_alarm += 1,
            (CaseLabel::Violation, Ordering::Less) => self.violation_quiet += 1,
            (CaseLabel::Violation, Ordering::Equal) => self.violation_boundary += 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoringBudget {
    pub encoded_bytes: usize,
    pub probe_coordinates: usize,
    pub threshold_comparisons: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoringWork {
    pub cases: usize,
    pub encoded_bytes: usize,
    pub probe_coordinates: usize,
    pub threshold_comparisons: usize,
}
impl ScoringWork {
    fn check(self, budget: ScoringBudget) -> Result<(), Error> {
        if budget.encoded_bytes > MAX_SCORING_BYTES || budget.probe_coordinates > MAX_CORPUS_COORDINATES
            || budget.threshold_comparisons > MAX_THRESHOLD_COMPARISONS
            || self.encoded_bytes > budget.encoded_bytes || self.probe_coordinates > budget.probe_coordinates
            || self.threshold_comparisons > budget.threshold_comparisons { return Err(Error::Limit); }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseScore { origin: CaseOrigin, label: CaseLabel, frame: FrameIdentity, score: ExactScore }
impl CaseScore {
    pub fn origin(&self) -> CaseOrigin { self.origin }
    pub fn label(&self) -> CaseLabel { self.label }
    pub fn frame(&self) -> FrameIdentity { self.frame }
    /// Exact raw dot+bias before subtracting any candidate threshold.
    pub fn score(&self) -> &ExactScore { &self.score }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThresholdTrial { threshold_bits: u32, counts: ConfusionCounts, accepted: bool }
impl ThresholdTrial {
    pub fn threshold(&self) -> f32 { f32::from_bits(self.threshold_bits) }
    pub fn counts(&self) -> ConfusionCounts { self.counts }
    pub fn accepted(&self) -> bool { self.accepted }
}
#[derive(Debug)]
struct CalibrationData {
    fitted: FittedProbe,
    policy: CalibrationPolicy,
    scores: Vec<CaseScore>,
    trials: Vec<ThresholdTrial>,
    selected: Option<usize>,
    work: ScoringWork,
}

/// All tested operating points remain recorded, including a campaign with no
/// eligible threshold. The final evaluation cannot select a different candidate.
#[derive(Clone, Debug)]
pub struct CalibrationRun { data: Rc<CalibrationData> }
impl CalibrationRun {
    pub fn fitted(&self) -> &FittedProbe { &self.data.fitted }
    pub fn policy(&self) -> &CalibrationPolicy { &self.data.policy }
    pub fn scores(&self) -> &[CaseScore] { &self.data.scores }
    pub fn trials(&self) -> &[ThresholdTrial] { &self.data.trials }
    pub fn work(&self) -> ScoringWork { self.data.work }
    pub fn selected_threshold(&self) -> Option<f32> { self.data.selected.map(|i| self.data.trials[i].threshold()) }

    /// Consumes ALL of the originally reserved evaluation split exactly as
    /// labelled. No caller subset, threshold override or re-fit input is accepted.
    pub fn evaluate(&self, budget: ScoringBudget) -> Result<EvaluationReport, Error> {
        let threshold = self.selected_threshold().ok_or(Error::WrongState)?;
        let work = estimate(self.fitted(), DataSplit::Evaluation, 1)?;
        work.check(budget)?;
        let scores = score(self.fitted(), DataSplit::Evaluation, work)?;
        let counts = classify(&scores, threshold)?;
        if counts.classes() != self.fitted().corpus().counts(DataSplit::Evaluation) { return Err(Error::Binding); }
        let accepted = self.policy().evaluation.accepts(counts);
        Ok(EvaluationReport { calibration: self.clone(), scores, counts, accepted, work })
    }
}

/// Immutable evaluation outcome. Passing these supplied finite-sample count
/// rules is not held-out population coverage, trained-host quality or permission.
#[derive(Clone, Debug)]
pub struct EvaluationReport {
    calibration: CalibrationRun,
    scores: Vec<CaseScore>,
    counts: ConfusionCounts,
    accepted: bool,
    work: ScoringWork,
}
impl EvaluationReport {
    pub fn calibration(&self) -> &CalibrationRun { &self.calibration }
    pub fn scores(&self) -> &[CaseScore] { &self.scores }
    pub fn counts(&self) -> ConfusionCounts { self.counts }
    pub fn accepted(&self) -> bool { self.accepted }
    pub fn work(&self) -> ScoringWork { self.work }
    /// Feed the existing monitor only after this evaluation passed its frozen
    /// rules. This exports data, not a live policy-promotion or effect capability.
    pub fn probe(&self) -> Result<LinearProbe, Error> {
        if !self.accepted { return Err(Error::WrongState); }
        self.calibration.fitted().probe(self.calibration.selected_threshold().ok_or(Error::WrongState)?)
    }
}

impl FittedProbe {
    /// Highest violation-alarm count, then fewest benign holds, then the first
    /// (lowest) declared threshold. These rules never inspect evaluation scores.
    pub fn calibrate(&self, policy: CalibrationPolicy, budget: ScoringBudget) -> Result<CalibrationRun, Error> {
        let work = estimate(self, DataSplit::Calibration, policy.threshold_bits.len())?;
        work.check(budget)?;
        let scores = score(self, DataSplit::Calibration, work)?;
        let mut trials: Vec<ThresholdTrial> = Vec::new();
        trials.try_reserve_exact(policy.threshold_bits.len()).map_err(|_| Error::Limit)?;
        let mut selected: Option<usize> = None;
        for threshold in policy.thresholds() {
            let counts = classify(&scores, threshold)?;
            if counts.classes() != self.corpus().counts(DataSplit::Calibration) { return Err(Error::Binding); }
            let accepted = policy.calibration.accepts(counts);
            if accepted && selected.is_none_or(|i| {
                counts.violation_alarm > trials[i].counts.violation_alarm
                    || (counts.violation_alarm == trials[i].counts.violation_alarm
                        && counts.benign_holds() < trials[i].counts.benign_holds())
            }) { selected = Some(trials.len()); }
            trials.push(ThresholdTrial { threshold_bits: threshold.to_bits(), counts, accepted });
        }
        Ok(CalibrationRun { data: Rc::new(CalibrationData { fitted: self.clone(), policy,
            scores, trials, selected, work }) })
    }
}

fn estimate(fitted: &FittedProbe, split: DataSplit, thresholds: usize) -> Result<ScoringWork, Error> {
    let cases = fitted.corpus().counts(split).total();
    let dimensions = fitted.corpus().dimensions();
    let per_frame = dimensions.checked_mul(4).and_then(|n| n.checked_add(HEADER_BYTES)).ok_or(Error::Overflow)?;
    Ok(ScoringWork { cases, encoded_bytes: cases.checked_mul(per_frame).ok_or(Error::Overflow)?,
        probe_coordinates: cases.checked_mul(dimensions).ok_or(Error::Overflow)?,
        threshold_comparisons: cases.checked_mul(thresholds).ok_or(Error::Overflow)? })
}
fn score(fitted: &FittedProbe, split: DataSplit, work: ScoringWork) -> Result<Vec<CaseScore>, Error> {
    let probe = fitted.probe(0.0)?;
    let mut scores = Vec::new();
    scores.try_reserve_exact(work.cases).map_err(|_| Error::Limit)?;
    let mut encoded = 0;
    for (origin, row) in fitted.corpus().rows(split) {
        let bytes = row.source.encode_initial(23)?;
        encoded += bytes.len();
        let block = row.source.verify_block(&bytes)?;
        let frame = ProgressiveFrame::from_initial(&block)?;
        let observed = probe.evaluate(&frame)?;
        if observed.interval().lower != observed.interval().upper { return Err(Error::Binding); }
        scores.push(CaseScore { origin, label: row.label, frame: observed.frame(), score: observed.interval().lower.clone() });
    }
    if encoded != work.encoded_bytes || scores.len() != work.cases { return Err(Error::Binding); }
    Ok(scores)
}
fn classify(scores: &[CaseScore], threshold: f32) -> Result<ConfusionCounts, Error> {
    let mut value = SignedSum::new();
    value.product(threshold.to_bits(), 1.0_f32.to_bits())?;
    let threshold = value.finish();
    let mut counts = ConfusionCounts::default();
    for case in scores { counts.record(case.label, case.score.cmp(&threshold)); }
    Ok(counts)
}
