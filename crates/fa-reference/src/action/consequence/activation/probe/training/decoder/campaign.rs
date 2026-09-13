//! One fixed all-layer campaign, using the existing optimizer and exact scorer.

use super::{DecoderCorpus, LabelledPrefix};
use super::super::{CaseOrigin, DataSplit, FitPolicy, SealedCorpus, TrainingBudget, MAX_TRAINING_VISITS, MAX_CORPUS_COORDINATES};
use super::super::calibration::{CalibrationPolicy, CalibrationRun, EvaluationReport,
    ScoringBudget, ScoringWork, MAX_SCORING_BYTES, MAX_THRESHOLD_COMPARISONS};
use crate::action::consequence::activation::{HEADER_BYTES, probe::LinearProbe};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderModel, DecoderProfile};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerPolicy { pub fit: FitPolicy, pub calibration: CalibrationPolicy }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CampaignWork {
    pub training_visits: u64,
    pub scoring_bytes: usize,
    pub scoring_coordinates: usize,
    pub threshold_comparisons: usize,
}
impl CampaignWork {
    fn add(self, other: Self) -> Result<Self, Error> {
        Ok(Self {
            training_visits: self.training_visits.checked_add(other.training_visits).ok_or(Error::Overflow)?,
            scoring_bytes: self.scoring_bytes.checked_add(other.scoring_bytes).ok_or(Error::Overflow)?,
            scoring_coordinates: self.scoring_coordinates.checked_add(other.scoring_coordinates).ok_or(Error::Overflow)?,
            threshold_comparisons: self.threshold_comparisons.checked_add(other.threshold_comparisons).ok_or(Error::Overflow)?,
        })
    }
}

/// Persistent full-campaign admission. Final evaluation is budgeted for every
/// layer, even one whose calibration later fails. Unused allowance is not returned.
#[derive(Debug)]
pub struct CampaignBudget { remaining: CampaignWork }
impl CampaignBudget {
    pub fn new(limits: CampaignWork) -> Result<Self, Error> {
        if limits.training_visits > MAX_TRAINING_VISITS || limits.scoring_bytes > MAX_SCORING_BYTES
            || limits.scoring_coordinates > MAX_CORPUS_COORDINATES
            || limits.threshold_comparisons > MAX_THRESHOLD_COMPARISONS { return Err(Error::Limit); }
        Ok(Self { remaining: limits })
    }
    pub fn remaining(&self) -> CampaignWork { self.remaining }
    fn admit(&mut self, work: CampaignWork) -> Result<(), Error> {
        let current = self.remaining;
        let next = CampaignWork {
            training_visits: current.training_visits.checked_sub(work.training_visits).ok_or(Error::Limit)?,
            scoring_bytes: current.scoring_bytes.checked_sub(work.scoring_bytes).ok_or(Error::Limit)?,
            scoring_coordinates: current.scoring_coordinates.checked_sub(work.scoring_coordinates).ok_or(Error::Limit)?,
            threshold_comparisons: current.threshold_comparisons.checked_sub(work.threshold_comparisons).ok_or(Error::Limit)?,
        };
        self.remaining = next;
        Ok(())
    }
}

/// None means no calibration threshold qualified, NOT an absent evaluation case.
/// All trials and denominators remain in the original calibration object.
#[derive(Clone, Debug)]
pub struct LayerCampaign { calibration: CalibrationRun, evaluation: Option<EvaluationReport> }
impl LayerCampaign {
    pub fn calibration(&self) -> &CalibrationRun { &self.calibration }
    pub fn evaluation(&self) -> Option<&EvaluationReport> { self.evaluation.as_ref() }
    pub fn accepted(&self) -> bool { self.evaluation.as_ref().is_some_and(EvaluationReport::accepted) }
}

/// Independent results stay per layer; repeated origins across layers are not
/// pooled into a larger statistical denominator. A failed layer is never removed
/// to manufacture a complete monitor roster. This is data, not policy promotion.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::probe::training::decoder::DecoderCampaign;
/// use fa_reference::action::Permit;
/// fn authorize(report: DecoderCampaign) -> Permit { report }
/// ```
#[derive(Debug)]
pub struct DecoderCampaign {
    pub(super) model: DecoderModel,
    pub(super) sources: Rc<BTreeMap<CaseOrigin, LabelledPrefix>>,
    profile: DecoderProfile,
    layers: BTreeMap<u64, LayerCampaign>,
    admitted: CampaignWork,
    completed: CampaignWork,
}
impl DecoderCampaign {
    pub fn profile(&self) -> &DecoderProfile { &self.profile }
    pub fn layers(&self) -> &BTreeMap<u64, LayerCampaign> { &self.layers }
    pub fn admitted_work(&self) -> CampaignWork { self.admitted }
    pub fn completed_work(&self) -> CampaignWork { self.completed }
    pub fn accepted(&self) -> bool {
        self.layers.len() == self.profile.shape().layers && self.layers.values().all(LayerCampaign::accepted)
    }
    /// Export the original evaluated probes, never refitted or retuned copies.
    /// The complete frozen roster must pass its own untouched evaluation rules.
    pub fn probes(&self) -> Result<BTreeMap<u64, LinearProbe>, Error> {
        if !self.accepted() { return Err(Error::WrongState); }
        self.layers.iter().map(|(layer, result)| {
            Ok((*layer, result.evaluation.as_ref().ok_or(Error::Incomplete)?.probe()?))
        }).collect()
    }
}

impl DecoderCorpus {
    /// Entire roster and complete work (fit, all thresholds, final population)
    /// are checked before the first fit. There is no per-layer budget renewal.
    pub fn estimate_campaign(&self, policies: &BTreeMap<u64, LayerPolicy>) -> Result<CampaignWork, Error> {
        if !self.layers.keys().eq(policies.keys()) { return Err(Error::Binding); }
        let mut work = CampaignWork::default();
        for (layer, corpus) in &self.layers {
            let policy = &policies[layer];
            let fit = corpus.estimate_fit(&policy.fit)?;
            let calibration = scoring(corpus, DataSplit::Calibration, policy.calibration.thresholds().count())?;
            let evaluation = scoring(corpus, DataSplit::Evaluation, 1)?;
            work = work.add(CampaignWork { training_visits: fit.source_coordinate_visits,
                ..CampaignWork::default() })?.add(scored(calibration))?.add(scored(evaluation))?;
        }
        CampaignBudget::new(work)?;
        Ok(work)
    }

    /// Statistical rejection is a retained result, not an error that drops a
    /// difficult layer. Hard numerical failure returns no partial campaign and
    /// keeps its full admission charge. No evaluation value can affect fitting
    /// or selection in the underlying three-way split implementation.
    pub fn run(&self, policies: BTreeMap<u64, LayerPolicy>, budget: &mut CampaignBudget) -> Result<DecoderCampaign, Error> {
        let admitted = self.estimate_campaign(&policies)?;
        budget.admit(admitted)?;
        let mut completed = CampaignWork::default();
        let mut layers = BTreeMap::new();
        for (layer, corpus) in &self.layers {
            let policy = &policies[layer];
            let fit_work = corpus.estimate_fit(&policy.fit)?;
            let fitted = corpus.fit(policy.fit.clone(), TrainingBudget { source_coordinate_visits: fit_work.source_coordinate_visits })?;
            completed = completed.add(CampaignWork { training_visits: fitted.work().source_coordinate_visits,
                ..CampaignWork::default() })?;
            let scoring_work = scoring(corpus, DataSplit::Calibration, policy.calibration.thresholds().count())?;
            let calibration = fitted.calibrate(policy.calibration.clone(), allowance(scoring_work))?;
            completed = completed.add(scored(calibration.work()))?;
            let evaluation = if calibration.selected_threshold().is_some() {
                let work = scoring(corpus, DataSplit::Evaluation, 1)?;
                let report = calibration.evaluate(allowance(work))?;
                completed = completed.add(scored(report.work()))?;
                Some(report)
            } else { None };
            layers.insert(*layer, LayerCampaign { calibration, evaluation });
        }
        Ok(DecoderCampaign { model: self.model.clone(), sources: Rc::clone(&self.cases),
            profile: self.profile.clone(), layers, admitted, completed })
    }
}

fn scoring(corpus: &SealedCorpus, split: DataSplit, thresholds: usize) -> Result<ScoringWork, Error> {
    let cases = corpus.counts(split).total();
    let coordinates = cases.checked_mul(corpus.dimensions()).ok_or(Error::Overflow)?;
    let per_frame = corpus.dimensions().checked_mul(4).and_then(|n| n.checked_add(HEADER_BYTES)).ok_or(Error::Overflow)?;
    Ok(ScoringWork { cases, encoded_bytes: cases.checked_mul(per_frame).ok_or(Error::Overflow)?,
        probe_coordinates: coordinates, threshold_comparisons: cases.checked_mul(thresholds).ok_or(Error::Overflow)? })
}
fn allowance(work: ScoringWork) -> ScoringBudget {
    ScoringBudget { encoded_bytes: work.encoded_bytes, probe_coordinates: work.probe_coordinates,
        threshold_comparisons: work.threshold_comparisons }
}
fn scored(work: ScoringWork) -> CampaignWork {
    CampaignWork { training_visits: 0, scoring_bytes: work.encoded_bytes,
        scoring_coordinates: work.probe_coordinates, threshold_comparisons: work.threshold_comparisons }
}
