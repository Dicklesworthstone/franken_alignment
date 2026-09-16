//! Fixed-seed stochastic investigations from the SAME original saved checkpoint.
//! All trials and total costs are admitted before computation; no success filter.

use super::{DecoderCheckpoint, DecoderLayerIntervention, FileDecoderCheckpoint, FileDecoderConfig,
    FileInvestigationOrigin, FileOversight, FileOversightProfile, JournalError, Purpose};
use crate::action::consequence::activation::tensor::kv::decoder::experiment::comparison::{
    DecoderComparisonBudget, cursor::DecoderComparisonStatus,
    sampled::{DecoderSampledComparisonBudget, DecoderSampledComparisonCursor, DecoderSampledComparisonWork, DecoderSampledPair},
};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const MAX_SAMPLED_INVESTIGATION_SEEDS: usize = 64;

/// The seed list is fixed before observing any trial. The supplied budget is the
/// TOTAL over all seeds and both arms, not an allowance refreshed at each seed.
/// A new invocation is a new campaign; this is not a fleet-wide budget ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSampledInvestigationRequest {
    pub experiment: u64,
    pub layers: BTreeMap<u64, DecoderLayerIntervention>,
    pub edit_limit: usize,
    pub first_token: u32,
    pub steps: usize,
    pub sampling_policy: SamplingPolicy,
    pub sampling_stream: u64,
    pub seeds: Vec<u64>,
    pub budget: DecoderSampledComparisonBudget,
}
impl FileSampledInvestigationRequest {
    fn validate_shape(&self) -> Result<(), Error> {
        if self.experiment == 0 || self.sampling_stream == 0 || self.steps == 0 || self.seeds.is_empty() {
            return Err(Error::InvalidInput);
        }
        if self.seeds.len() > MAX_SAMPLED_INVESTIGATION_SEEDS { return Err(Error::Limit); }
        let mut unique = BTreeSet::new();
        for seed in &self.seeds { if !unique.insert(*seed) { return Err(Error::Duplicate); } }
        Ok(())
    }
    fn start(&self, seed: u64) -> SamplingStart {
        SamplingStart { policy: self.sampling_policy.clone(), stream: self.sampling_stream, seed }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSampledTrialStatus { Completed, Failed(Error), Cancelled, NotRun }

#[derive(Clone, Debug)]
pub struct FileSampledTrialReport {
    seed: u64,
    status: FileSampledTrialStatus,
    work: DecoderSampledComparisonWork,
    pairs: Vec<DecoderSampledPair>,
}
impl FileSampledTrialReport {
    pub fn seed(&self) -> u64 { self.seed }
    pub fn status(&self) -> FileSampledTrialStatus { self.status }
    pub fn work(&self) -> DecoderSampledComparisonWork { self.work }
    /// May be only a prefix when status is not Completed.
    pub fn pairs(&self) -> &[DecoderSampledPair] { &self.pairs }
}

/// A full denominator, not an approval. Failed, cancelled and never-started seeds
/// remain present in their original order; a completed trial need not be benign.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::checkpoint::investigation::sampled::FileSampledInvestigationReport;
/// fn grant(report: FileSampledInvestigationReport) -> FilePermit { report }
/// ```
#[derive(Clone, Debug)]
pub struct FileSampledInvestigationReport {
    origin: FileInvestigationOrigin,
    request: FileSampledInvestigationRequest,
    planned: DecoderSampledComparisonBudget,
    trials: Vec<FileSampledTrialReport>,
    cancelled: bool,
    interrupted: bool,
}
impl FileSampledInvestigationReport {
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn origin(&self) -> &FileInvestigationOrigin { &self.origin }
    pub fn request(&self) -> &FileSampledInvestigationRequest { &self.request }
    pub fn planned(&self) -> DecoderSampledComparisonBudget { self.planned }
    pub fn trials(&self) -> &[FileSampledTrialReport] { &self.trials }
    pub fn cancelled(&self) -> bool { self.cancelled }
    pub fn interrupted(&self) -> bool { self.interrupted }
    pub fn completed_trials(&self) -> usize {
        self.trials.iter().filter(|trial| trial.status == FileSampledTrialStatus::Completed).count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSampledInvestigationProgress {
    Trial { index: usize, seed: u64, status: DecoderComparisonStatus },
    /// Every trial has reached a terminal outcome, not "every trial succeeded".
    Finished,
}

/// Private paired cursors, without a broker, actor RNG, endpoint or approval key.
/// Each advance executes one arm-token; an ordinary failed trial is recorded and
/// the next fixed seed may proceed. A caught unwind instead retires the campaign:
/// explicit cancellation can export partial evidence but never resume computation.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::checkpoint::investigation::sampled::FileSampledInvestigation;
/// fn duplicate(run: FileSampledInvestigation) { let _ = run.clone(); }
/// ```
#[derive(Debug)]
pub struct FileSampledInvestigation {
    origin: FileInvestigationOrigin,
    request: FileSampledInvestigationRequest,
    planned: DecoderSampledComparisonBudget,
    cursors: Vec<DecoderSampledComparisonCursor>,
    current: usize,
    cancelled: bool,
    interrupted: bool,
}
impl FileSampledInvestigation {
    fn prepare(origin: FileInvestigationOrigin, source: DecoderCheckpoint,
        mut request: FileSampledInvestigationRequest) -> Result<Self, Error>
    {
        request.validate_shape()?;
        let plan = source.intervene(request.experiment, std::mem::take(&mut request.layers), request.edit_limit)?;
        let first = plan.begin_sampled_comparison(request.first_token, request.steps,
            request.start(request.seeds[0]), request.budget)?;
        let first_work = first.work()?;
        let count = request.seeds.len();
        let per_trial = DecoderSampledComparisonBudget {
            comparison: DecoderComparisonBudget { scalar_products: first_work.numerical.planned.scalar_products()?,
                retained_logit_values: request.steps.checked_mul(2)
                    .and_then(|n| n.checked_mul(request.sampling_policy.vocabulary())).ok_or(Error::Limit)? },
            sampling_logits: first_work.planned_sampling_logits,
        };
        let planned = DecoderSampledComparisonBudget {
            comparison: DecoderComparisonBudget {
                scalar_products: per_trial.comparison.scalar_products.checked_mul(count as u64).ok_or(Error::Limit)?,
                retained_logit_values: per_trial.comparison.retained_logit_values.checked_mul(count).ok_or(Error::Limit)?,
            }, sampling_logits: per_trial.sampling_logits.checked_mul(count).ok_or(Error::Limit)?,
        };
        if planned.comparison.scalar_products > request.budget.comparison.scalar_products
            || planned.comparison.retained_logit_values > request.budget.comparison.retained_logit_values
            || planned.sampling_logits > request.budget.sampling_logits { return Err(Error::Limit); }
        // Every cursor is prepared before returning any one of them. No token or
        // sample has executed, including when aggregate admission above refuses.
        let mut cursors = Vec::new(); cursors.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        cursors.push(first);
        for seed in &request.seeds[1..] {
            cursors.push(plan.begin_sampled_comparison(request.first_token, request.steps, request.start(*seed), per_trial)?);
        }
        request.layers = plan.specification().clone();
        Ok(Self { origin, request, planned, cursors, current: 0, cancelled: false, interrupted: false })
    }
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn origin(&self) -> &FileInvestigationOrigin { &self.origin }
    pub fn request(&self) -> &FileSampledInvestigationRequest { &self.request }
    pub fn planned(&self) -> DecoderSampledComparisonBudget { self.planned }
    pub fn is_finished(&self) -> bool { self.cancelled || self.current == self.cursors.len() }
    pub fn trial_work(&self, index: usize) -> Result<DecoderSampledComparisonWork, Error> {
        self.cursors.get(index).ok_or(Error::Missing)?.work()
    }
    pub fn advance(&mut self) -> Result<FileSampledInvestigationProgress, Error> {
        if self.cancelled || self.interrupted { return Err(Error::WrongState); }
        if self.current == self.cursors.len() { return Ok(FileSampledInvestigationProgress::Finished); }
        let index = self.current;
        self.interrupted = true;
        let result = self.cursors[index].advance();
        let status = self.cursors[index].status();
        match (result, status) {
            (Ok(_), DecoderComparisonStatus::Running | DecoderComparisonStatus::Complete) => {}
            (Err(error), DecoderComparisonStatus::Failed(cause)) if error == cause => {}
            _ => return Err(Error::Binding),
        }
        if matches!(status, DecoderComparisonStatus::Complete | DecoderComparisonStatus::Failed(_)) { self.current += 1; }
        self.interrupted = false;
        Ok(FileSampledInvestigationProgress::Trial { index, seed: self.request.seeds[index], status })
    }
    pub fn cancel(&mut self) -> Result<(), Error> {
        if self.cancelled { return Ok(()); }
        if self.current == self.cursors.len() { return Err(Error::WrongState); }
        self.cancelled = true;
        for cursor in &mut self.cursors {
            if cursor.status() == DecoderComparisonStatus::Running { cursor.cancel()?; }
        }
        Ok(())
    }
    /// Normal finish requires all fixed seeds. Explicit cancellation may export
    /// an incomplete report, but cannot remove failed or unstarted rows from it.
    pub fn finish(&self) -> Result<FileSampledInvestigationReport, Error> {
        if !self.is_finished() { return Err(Error::Incomplete); }
        let mut trials = Vec::new(); trials.try_reserve_exact(self.cursors.len()).map_err(|_| Error::Limit)?;
        for (seed, cursor) in self.request.seeds.iter().zip(&self.cursors) {
            let work = cursor.work()?;
            let status = match cursor.status() {
                DecoderComparisonStatus::Complete => { cursor.finish()?; FileSampledTrialStatus::Completed }
                DecoderComparisonStatus::Failed(error) => FileSampledTrialStatus::Failed(error),
                DecoderComparisonStatus::Cancelled if work.numerical.entered_tokens == 0 && work.entered_sampling_calls == 0 =>
                    FileSampledTrialStatus::NotRun,
                DecoderComparisonStatus::Cancelled => FileSampledTrialStatus::Cancelled,
                _ => return Err(Error::Incomplete),
            };
            trials.push(FileSampledTrialReport { seed: *seed, status, work, pairs: cursor.completed_pairs().to_vec() });
        }
        Ok(FileSampledInvestigationReport { origin: self.origin.clone(), request: self.request.clone(),
            planned: self.planned, trials, cancelled: self.cancelled, interrupted: self.interrupted })
    }
}

impl FileOversight {
    pub fn investigate_sampled_decoder_checkpoint(&self, checkpoint: &FileDecoderCheckpoint,
        request: FileSampledInvestigationRequest) -> Result<FileSampledInvestigation, JournalError>
    {
        request.validate_shape()?;
        let (origin, source) = self.investigation_source(checkpoint)?;
        Ok(FileSampledInvestigation::prepare(origin, source, request)?)
    }
    /// Same pinned canonical read and complete-history validation as the existing
    /// deterministic investigation. Reconstruct once, then share the immutable
    /// original checkpoint across all seeds. No writer, cleanup, fence or roles.
    /// Whole-history reconstruction is additional to the paired-trial budgets.
    pub fn read_sampled_decoder_investigation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected_decoder: &FileDecoderConfig, checkpoint: u64, request: FileSampledInvestigationRequest)
        -> Result<FileSampledInvestigation, JournalError>
    {
        request.validate_shape()?;
        let (origin, source) = Self::read_investigation_source(directory.as_ref(), profile, expected_decoder, checkpoint)?;
        Ok(FileSampledInvestigation::prepare(origin, source, request)?)
    }
}
