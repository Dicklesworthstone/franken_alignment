//! Complete K/V campaigns delegated to the original training and exact scoring.
use super::{KvDecoderCorpus, LabelledPrefix};
use super::super::LayerPolicy;
use super::super::super::{CaseOrigin, DataSplit, SealedCorpus, TrainingBudget,
    MAX_CORPUS_COORDINATES, MAX_TRAINING_VISITS};
use super::super::super::calibration::{CalibrationRun, EvaluationReport, ScoringBudget,
    ScoringWork, MAX_SCORING_BYTES, MAX_THRESHOLD_COMPARISONS};
use crate::action::consequence::activation::{HEADER_BYTES, probe::LinearProbe,
    monitor::learned::model::KvTap, tensor::kv::decoder::{DecoderModel, DecoderProfile}};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KvCampaignWork {
    pub training_visits: u64,
    pub scoring_bytes: usize,
    pub scoring_coordinates: usize,
    pub threshold_comparisons: usize,
}
impl KvCampaignWork {
    fn add(self, other: Self) -> Result<Self, Error> {
        Ok(Self {
            training_visits: self.training_visits.checked_add(other.training_visits).ok_or(Error::Overflow)?,
            scoring_bytes: self.scoring_bytes.checked_add(other.scoring_bytes).ok_or(Error::Overflow)?,
            scoring_coordinates: self.scoring_coordinates.checked_add(other.scoring_coordinates).ok_or(Error::Overflow)?,
            threshold_comparisons: self.threshold_comparisons.checked_add(other.threshold_comparisons).ok_or(Error::Overflow)?,
        })
    }
}

/// One persistent allowance for every fit, calibration threshold and final
/// evaluation in the complete roster. Statistical failure does not refund it.
#[derive(Debug)]
pub struct KvCampaignBudget { remaining: KvCampaignWork }
impl KvCampaignBudget {
    pub fn new(limits: KvCampaignWork) -> Result<Self, Error> {
        if limits.training_visits > MAX_TRAINING_VISITS || limits.scoring_bytes > MAX_SCORING_BYTES
            || limits.scoring_coordinates > MAX_CORPUS_COORDINATES
            || limits.threshold_comparisons > MAX_THRESHOLD_COMPARISONS { return Err(Error::Limit); }
        Ok(Self { remaining: limits })
    }
    pub fn remaining(&self) -> KvCampaignWork { self.remaining }
    fn admit(&mut self, work: KvCampaignWork) -> Result<(), Error> {
        let current = self.remaining;
        let next = KvCampaignWork {
            training_visits: current.training_visits.checked_sub(work.training_visits).ok_or(Error::Limit)?,
            scoring_bytes: current.scoring_bytes.checked_sub(work.scoring_bytes).ok_or(Error::Limit)?,
            scoring_coordinates: current.scoring_coordinates.checked_sub(work.scoring_coordinates).ok_or(Error::Limit)?,
            threshold_comparisons: current.threshold_comparisons.checked_sub(work.threshold_comparisons).ok_or(Error::Limit)?,
        };
        self.remaining = next;
        Ok(())
    }
}

/// A missing evaluation means no threshold passed calibration. Its reserved
/// population is still retained in the original fitted corpus and cost record.
#[derive(Clone, Debug)]
pub struct KvTapCampaign { calibration: CalibrationRun, evaluation: Option<EvaluationReport> }
impl KvTapCampaign {
    pub fn calibration(&self) -> &CalibrationRun { &self.calibration }
    pub fn evaluation(&self) -> Option<&EvaluationReport> { self.evaluation.as_ref() }
    pub fn accepted(&self) -> bool { self.evaluation.as_ref().is_some_and(EvaluationReport::accepted) }
}

/// One immutable model and complete tap roster. Repeated origins across taps
/// are not independent trials. Passing means only the supplied finite-count
/// criteria passed on this population, not model safety or live policy promotion.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::probe::training::decoder::kv::KvDecoderCampaign;
/// use fa_reference::action::Permit;
/// fn authorize(report: KvDecoderCampaign) -> Permit { report }
/// ```
#[derive(Debug)]
pub struct KvDecoderCampaign {
    pub(super) model: DecoderModel,
    pub(super) cases: Rc<BTreeMap<CaseOrigin, LabelledPrefix>>,
    taps: BTreeMap<KvTap, KvTapCampaign>,
    admitted: KvCampaignWork,
    completed: KvCampaignWork,
}
impl KvDecoderCampaign {
    pub fn model(&self) -> &DecoderModel { &self.model }
    pub fn profile(&self) -> &DecoderProfile { self.model.profile() }
    pub fn cases(&self) -> &BTreeMap<CaseOrigin, LabelledPrefix> { &self.cases }
    pub fn taps(&self) -> &BTreeMap<KvTap, KvTapCampaign> { &self.taps }
    pub fn admitted_work(&self) -> KvCampaignWork { self.admitted }
    pub fn completed_work(&self) -> KvCampaignWork { self.completed }
    pub fn accepted(&self) -> bool {
        self.taps.len() == self.model.cache_profile().layers().len() * 2
            && self.taps.values().all(KvTapCampaign::accepted)
    }
    /// No partial-roster export and no replacement weights or threshold argument.
    pub fn probes(&self) -> Result<BTreeMap<KvTap, LinearProbe>, Error> {
        if !self.accepted() { return Err(Error::WrongState); }
        self.taps.iter().map(|(tap, result)| {
            Ok((*tap, result.evaluation.as_ref().ok_or(Error::Incomplete)?.probe()?))
        }).collect()
    }
}

impl KvDecoderCorpus {
    /// Check the entire frozen roster and reserve every final evaluation, even
    /// if a preceding calibration later returns no eligible operating point.
    pub fn estimate_campaign(&self, policies: &BTreeMap<KvTap, LayerPolicy>) -> Result<KvCampaignWork, Error> {
        if !self.taps.keys().eq(policies.keys()) { return Err(Error::Binding); }
        let mut work = KvCampaignWork::default();
        for (tap, corpus) in &self.taps {
            let policy = &policies[tap];
            let fit = corpus.estimate_fit(&policy.fit)?;
            let calibration = scoring(corpus, DataSplit::Calibration, policy.calibration.thresholds().count())?;
            let evaluation = scoring(corpus, DataSplit::Evaluation, 1)?;
            work = work.add(KvCampaignWork { training_visits: fit.source_coordinate_visits,
                ..KvCampaignWork::default() })?.add(scored(calibration))?.add(scored(evaluation))?;
        }
        KvCampaignBudget::new(work)?;
        Ok(work)
    }

    /// A statistical failure remains a tap result, with later taps still run.
    /// A hard arithmetic failure returns no partial campaign and keeps its whole
    /// admission. Evaluation inputs never enter fitting or threshold selection.
    pub fn run(&self, policies: BTreeMap<KvTap, LayerPolicy>, budget: &mut KvCampaignBudget)
        -> Result<KvDecoderCampaign, Error>
    {
        let admitted = self.estimate_campaign(&policies)?;
        budget.admit(admitted)?;
        let mut completed = KvCampaignWork::default();
        let mut taps = BTreeMap::new();
        for (tap, corpus) in &self.taps {
            let policy = &policies[tap];
            let fit_work = corpus.estimate_fit(&policy.fit)?;
            let fitted = corpus.fit(policy.fit.clone(), TrainingBudget {
                source_coordinate_visits: fit_work.source_coordinate_visits,
            })?;
            completed = completed.add(KvCampaignWork { training_visits: fitted.work().source_coordinate_visits,
                ..KvCampaignWork::default() })?;
            let work = scoring(corpus, DataSplit::Calibration, policy.calibration.thresholds().count())?;
            let calibration = fitted.calibrate(policy.calibration.clone(), allowance(work))?;
            completed = completed.add(scored(calibration.work()))?;
            let evaluation = if calibration.selected_threshold().is_some() {
                let work = scoring(corpus, DataSplit::Evaluation, 1)?;
                let report = calibration.evaluate(allowance(work))?;
                completed = completed.add(scored(report.work()))?;
                Some(report)
            } else { None };
            taps.insert(*tap, KvTapCampaign { calibration, evaluation });
        }
        Ok(KvDecoderCampaign { model: self.model.clone(), cases: Rc::clone(&self.cases),
            taps, admitted, completed })
    }
}

fn scoring(corpus: &SealedCorpus, split: DataSplit, thresholds: usize) -> Result<ScoringWork, Error> {
    let cases = corpus.counts(split).total();
    let coordinates = cases.checked_mul(corpus.dimensions()).ok_or(Error::Overflow)?;
    let bytes = corpus.dimensions().checked_mul(4).and_then(|n| n.checked_add(HEADER_BYTES)).ok_or(Error::Overflow)?;
    Ok(ScoringWork { cases, encoded_bytes: cases.checked_mul(bytes).ok_or(Error::Overflow)?,
        probe_coordinates: coordinates, threshold_comparisons: cases.checked_mul(thresholds).ok_or(Error::Overflow)? })
}
fn allowance(work: ScoringWork) -> ScoringBudget {
    ScoringBudget { encoded_bytes: work.encoded_bytes, probe_coordinates: work.probe_coordinates,
        threshold_comparisons: work.threshold_comparisons }
}
fn scored(work: ScoringWork) -> KvCampaignWork {
    KvCampaignWork { training_visits: 0, scoring_bytes: work.encoded_bytes,
        scoring_coordinates: work.probe_coordinates, threshold_comparisons: work.threshold_comparisons }
}
