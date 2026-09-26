//! Fixed-seed source-intervention campaigns with no success-only denominator.
//! The question, seeds, order and resource envelope freeze before computation.
use super::{GenerationSpec, GenerationStatus, PromptArm, PromptComparison,
    PromptComparisonBudget, PromptComparisonReport, PromptComparisonStatus, PromptIntervention};
use crate::Error;
use std::collections::BTreeSet;

pub const MAX_PROMPT_TRIALS: usize = 64;
pub const MAX_OUTCOME_TOKENS: usize = 64;

/// Presence/absence is about the exact registered continuation-token pattern,
/// not harmfulness, causality, or tokens suppressed by a held/failed monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenObservation {
    Present { first_generated_offset: usize },
    AbsentWithinCompletedHorizon,
    Censored { stopped: GenerationStatus },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeClass { Present, Absent, Censored }
impl OutcomeClass {
    fn index(self) -> usize {
        match self { Self::Present => 0, Self::Absent => 1, Self::Censored => 2 }
    }
}
impl TokenObservation {
    pub fn class(self) -> OutcomeClass {
        match self {
            Self::Present { .. } => OutcomeClass::Present,
            Self::AbsentWithinCompletedHorizon => OutcomeClass::Absent,
            Self::Censored { .. } => OutcomeClass::Censored,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutcomeTable { counts: [[usize; 3]; 3] }
impl OutcomeTable {
    pub fn count(&self, baseline: OutcomeClass, treated: OutcomeClass) -> usize {
        self.counts[baseline.index()][treated.index()]
    }
    pub fn observed_pairs(&self) -> usize { self.counts.iter().flatten().sum() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptCampaignBudget {
    pub trials: usize,
    pub positions: usize,
    pub decoder_products: u64,
    pub vocabulary_scores: u64,
    /// Worst-case token equalities in the frozen outcome query over BOTH arms.
    pub outcome_comparisons: u64,
}
impl Default for PromptCampaignBudget {
    fn default() -> Self {
        let pair = PromptComparisonBudget::default();
        Self { trials: MAX_PROMPT_TRIALS, positions: MAX_PROMPT_TRIALS * pair.positions,
            decoder_products: MAX_PROMPT_TRIALS as u64 * pair.decoder_products,
            vocabulary_scores: MAX_PROMPT_TRIALS as u64 * pair.vocabulary_scores,
            outcome_comparisons: (MAX_PROMPT_TRIALS * pair.positions * MAX_OUTCOME_TOKENS) as u64 }
    }
}
impl PromptCampaignBudget {
    fn admits(self, need: Self) -> Result<(), Error> {
        let maximum = Self::default();
        if self.trials > maximum.trials || self.positions > maximum.positions
            || self.decoder_products > maximum.decoder_products || self.vocabulary_scores > maximum.vocabulary_scores
            || self.outcome_comparisons > maximum.outcome_comparisons || need.trials > self.trials
            || need.positions > self.positions || need.decoder_products > self.decoder_products
            || need.vocabulary_scores > self.vocabulary_scores || need.outcome_comparisons > self.outcome_comparisons {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrialDisposition { Observed, AdmissionFailed(Error), Interrupted(Error) }
/// All attempted seeds retain a row, including constructor failures and cancelled
/// partial runs. Comparison rows preserve both arms' detailed numerical/telemetry
/// accounting and stop reasons. Public callers cannot replace the owner's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptTrial {
    seed: u64,
    disposition: TrialDisposition,
    comparison: Option<PromptComparisonReport>,
    outcomes: Option<(TokenObservation, TokenObservation)>,
    outcome_comparisons: u64,
}
impl PromptTrial {
    pub fn seed(&self) -> u64 { self.seed }
    pub fn disposition(&self) -> TrialDisposition { self.disposition }
    pub fn comparison(&self) -> Option<&PromptComparisonReport> { self.comparison.as_ref() }
    pub fn outcomes(&self) -> Option<(TokenObservation, TokenObservation)> { self.outcomes }
    pub fn outcome_comparisons(&self) -> u64 { self.outcome_comparisons }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptCampaignStatus {
    Active,
    /// The declared schedule is exhausted, NOT a claim that all trials succeeded.
    Finished,
    Cancelled,
    Failed(Error),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptCampaignProgress {
    pub campaign: u64,
    pub revision: u64,
    pub status: PromptCampaignStatus,
    pub declared_trials: usize,
    pub started_trials: usize,
    pub recorded_trials: usize,
    /// Includes a started attempt interrupted by an unwind before a row exists.
    pub unrecorded_started_trials: usize,
    pub unstarted_trials: usize,
    pub admission_failures: usize,
    pub interrupted_trials: usize,
    pub outcomes: OutcomeTable,
    pub outcome_comparisons: u64,
}

/// One schedule and one frozen token-pattern question. Seeds are paired within
/// trials, not assumed independent across trials or representative of tasks.
/// At most one comparison's two generators is retained at a time.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::prompt::campaign::PromptCampaign;
/// fn retune(campaign: &mut PromptCampaign) { campaign.replace_question(vec![1]); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::prompt::campaign::PromptCampaign;
/// fn escape(campaign: &mut PromptCampaign) { campaign.comparison_mut(); }
/// ```
#[derive(Debug)]
pub struct PromptCampaign {
    id: u64,
    plan: PromptIntervention,
    seeds: Vec<u64>,
    needle: Vec<u32>,
    reservation: PromptCampaignBudget,
    status: PromptCampaignStatus,
    revision: u64,
    started: usize,
    active: Option<PromptComparison>,
    trials: Vec<PromptTrial>,
    outcome_comparisons: u64,
}
impl PromptIntervention {
    pub fn estimate_campaign(&self, trials: usize, outcome_tokens: usize) -> Result<PromptCampaignBudget, Error> {
        if trials == 0 || outcome_tokens == 0 { return Err(Error::InvalidInput); }
        if trials > MAX_PROMPT_TRIALS || outcome_tokens > MAX_OUTCOME_TOKENS { return Err(Error::Limit); }
        let pair = self.reservation();
        let horizon = self.config().original.max_new_tokens();
        let windows = if horizon >= outcome_tokens { horizon - outcome_tokens + 1 } else { 0 };
        let comparisons = windows.checked_mul(outcome_tokens).and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_mul(trials)).ok_or(Error::Overflow)?;
        Ok(PromptCampaignBudget { trials,
            positions: pair.positions.checked_mul(trials).ok_or(Error::Overflow)?,
            decoder_products: pair.decoder_products.checked_mul(trials as u64).ok_or(Error::Overflow)?,
            vocabulary_scores: pair.vocabulary_scores.checked_mul(trials as u64).ok_or(Error::Overflow)?,
            outcome_comparisons: u64::try_from(comparisons).map_err(|_| Error::Overflow)? })
    }
    pub fn begin_campaign(&self, id: u64, seeds: Vec<u64>, needle: Vec<u32>, budget: PromptCampaignBudget)
        -> Result<PromptCampaign, Error>
    {
        if id == 0 { return Err(Error::InvalidInput); }
        let reservation = self.estimate_campaign(seeds.len(), needle.len())?;
        budget.admits(reservation)?;
        if needle.iter().any(|token| *token as usize >= self.config().original.sampling().policy.vocabulary()) {
            return Err(Error::InvalidInput);
        }
        let mut distinct = BTreeSet::new();
        if seeds.iter().any(|seed| !distinct.insert(*seed)) { return Err(Error::Duplicate); }
        let mut trials = Vec::new();
        trials.try_reserve_exact(seeds.len()).map_err(|_| Error::Limit)?;
        Ok(PromptCampaign { id, plan: self.clone(), seeds, needle, reservation,
            status: PromptCampaignStatus::Active, revision: 0, started: 0,
            active: None, trials, outcome_comparisons: 0 })
    }
    fn at_seed(&self, seed: u64) -> Result<PromptIntervention, Error> {
        let mut config = self.config().clone();
        let mut sampling = config.original.sampling().clone();
        sampling.seed = seed;
        config.original = GenerationSpec::new(config.original.prompt().to_vec(), config.original.max_new_tokens(),
            config.original.stop_tokens().clone(), sampling)?;
        self.data.model.prompt_intervention(self.id(), config, self.edit().clone())
    }
}
impl PromptCampaign {
    pub fn seeds(&self) -> &[u64] { &self.seeds }
    pub fn question(&self) -> &[u32] { &self.needle }
    pub fn plan(&self) -> &PromptIntervention { &self.plan }
    pub fn reservation(&self) -> PromptCampaignBudget { self.reservation }
    pub fn trials(&self) -> &[PromptTrial] { &self.trials }
    pub fn active_report(&self) -> Option<PromptComparisonReport> { self.active.as_ref().map(PromptComparison::report) }
    pub fn progress(&self) -> PromptCampaignProgress {
        let mut outcomes = OutcomeTable::default();
        let mut admission_failures = 0;
        let mut interrupted_trials = 0;
        for trial in &self.trials {
            if let Some((a, b)) = trial.outcomes {
                outcomes.counts[a.class().index()][b.class().index()] += 1;
            }
            match trial.disposition {
                TrialDisposition::AdmissionFailed(_) => admission_failures += 1,
                TrialDisposition::Interrupted(_) => interrupted_trials += 1,
                TrialDisposition::Observed => {}
            }
        }
        PromptCampaignProgress { campaign: self.id, revision: self.revision, status: self.status,
            declared_trials: self.seeds.len(), started_trials: self.started, recorded_trials: self.trials.len(),
            unrecorded_started_trials: self.started - self.trials.len(), unstarted_trials: self.seeds.len() - self.started,
            admission_failures, interrupted_trials, outcomes, outcome_comparisons: self.outcome_comparisons }
    }

    /// At most one paired numerical step, or one refused trial admission. Every
    /// seed gets exactly its registered slot; errors cannot reroll it or skip it
    /// in reporting. Construction and inference execute only after poisoning.
    pub fn advance(&mut self, expected_revision: u64) -> Result<PromptCampaignProgress, Error> {
        self.preflight(expected_revision)?;
        self.status = PromptCampaignStatus::Failed(Error::Incomplete);
        self.revision += 1;
        if self.active.is_none() {
            let seed = self.seeds[self.started];
            self.started += 1;
            match self.plan.at_seed(seed).and_then(|plan| plan.begin(plan.reservation())) {
                Ok(pair) => self.active = Some(pair),
                Err(error) => {
                    self.trials.push(PromptTrial { seed, disposition: TrialDisposition::AdmissionFailed(error),
                        comparison: None, outcomes: None, outcome_comparisons: 0 });
                    self.set_progress();
                    return Ok(self.progress());
                }
            }
        }
        let pair = self.active.as_mut().ok_or(Error::Incomplete)?;
        let result = pair.advance(pair.revision());
        if let Err(error) = result {
            self.record_interrupted(error);
        } else if pair.status() == PromptComparisonStatus::Stopped {
            let report = pair.report();
            let (a, left) = observe(pair.generated_tokens(PromptArm::Baseline), report.baseline.status, &self.needle);
            let (b, right) = observe(pair.generated_tokens(PromptArm::Treated), report.treated.status, &self.needle);
            let visits = left.checked_add(right).ok_or(Error::Overflow)?;
            self.outcome_comparisons = self.outcome_comparisons.checked_add(visits).ok_or(Error::Overflow)?;
            if self.outcome_comparisons > self.reservation.outcome_comparisons { return Err(Error::Binding); }
            self.trials.push(PromptTrial { seed: self.seeds[self.started - 1], disposition: TrialDisposition::Observed,
                comparison: Some(report), outcomes: Some((a, b)), outcome_comparisons: visits });
            self.active = None;
        }
        self.set_progress();
        Ok(self.progress())
    }
    fn preflight(&self, revision: u64) -> Result<(), Error> {
        if self.status != PromptCampaignStatus::Active { return Err(Error::WrongState); }
        if revision != self.revision { return Err(Error::Stale); }
        self.revision.checked_add(1).ok_or(Error::Overflow)?;
        Ok(())
    }
    fn set_progress(&mut self) {
        self.status = if self.trials.len() == self.seeds.len() { PromptCampaignStatus::Finished }
            else { PromptCampaignStatus::Active };
    }
    fn record_interrupted(&mut self, error: Error) {
        if let Some(pair) = self.active.take() {
            self.trials.push(PromptTrial { seed: self.seeds[self.started - 1],
                disposition: TrialDisposition::Interrupted(error), comparison: Some(pair.report()),
                outcomes: None, outcome_comparisons: 0 });
        }
    }
    /// Cancellation records a partial current trial as interrupted and leaves
    /// every unstarted seed in the denominator explicitly unobserved.
    pub fn cancel(&mut self, expected_revision: u64) -> Result<PromptCampaignProgress, Error> {
        self.preflight(expected_revision)?;
        self.status = PromptCampaignStatus::Failed(Error::Incomplete);
        self.revision += 1;
        self.record_interrupted(Error::Incomplete);
        self.status = PromptCampaignStatus::Cancelled;
        Ok(self.progress())
    }
    pub fn run_to_stop(&mut self) -> Result<PromptCampaignProgress, Error> {
        while self.status == PromptCampaignStatus::Active { self.advance(self.revision)?; }
        match self.status {
            PromptCampaignStatus::Failed(error) => Err(error),
            _ => Ok(self.progress()),
        }
    }
}

fn observe(tokens: &[u32], status: GenerationStatus, needle: &[u32]) -> (TokenObservation, u64) {
    let mut comparisons = 0;
    for (offset, window) in tokens.windows(needle.len()).enumerate() {
        let mut matched = true;
        for (a, b) in window.iter().zip(needle) {
            comparisons += 1;
            if a != b { matched = false; break; }
        }
        if matched { return (TokenObservation::Present { first_generated_offset: offset }, comparisons); }
    }
    (if matches!(status, GenerationStatus::Finished(_)) { TokenObservation::AbsentWithinCompletedHorizon }
        else { TokenObservation::Censored { stopped: status } }, comparisons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::super::sampling::monitored::GenerationStop;

    #[test]
    fn exact_pattern_queries_preserve_overlap_censoring_and_comparison_counts() {
        let finished = GenerationStatus::Finished(GenerationStop::TokenLimit);
        assert_eq!(observe(&[1, 1, 2], finished, &[1, 2]),
            (TokenObservation::Present { first_generated_offset: 1 }, 4));
        assert_eq!(observe(&[1], finished, &[1, 2]), (TokenObservation::AbsentWithinCompletedHorizon, 0));
        let failed = GenerationStatus::Failed(Error::Limit);
        assert_eq!(observe(&[1], failed, &[1, 2]), (TokenObservation::Censored { stopped: failed }, 0));
        assert_eq!(observe(&[1, 2], failed, &[1, 2]),
            (TokenObservation::Present { first_generated_offset: 0 }, 2));
    }
}
