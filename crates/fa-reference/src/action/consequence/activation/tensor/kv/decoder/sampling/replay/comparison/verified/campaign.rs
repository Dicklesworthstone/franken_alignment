//! Fixed-cohort, bounded execution of the original verified policy comparisons.
//!
//! This L7 runner accepts only original, unadvanced ComparisonPreparation owners.
//! It cannot accept supplied verdicts, bypass baseline replay, expose an arm or
//! promote a policy. Failures and bounded/held runs stay in the declared cohort.

use super::{ComparisonPreparation, PreparationReport};
use super::super::{ComparisonLineage, ComparisonReport, ComparisonStatus};
use super::super::super::ReplayStatus;
use super::super::super::super::monitored::{GenerationStatus, GenerationStop, MAX_GENERATION_TOKENS, MAX_GENERATION_SCORES};
use super::super::super::super::super::MAX_DECODER_PRODUCTS;
use crate::Error;
use std::collections::BTreeSet;

pub const MAX_CAMPAIGN_CASES: usize = 64;

/// Logical numerical admission, not elapsed time, memory or independent samples.
/// Paired positions count a position in BOTH arms; decoder/sampler counts already
/// include both arms plus original baseline verification. Early stops do not
/// return this reservation to the campaign or admit additional cases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CampaignCost {
    pub cases: usize,
    pub replay_positions: u64,
    pub paired_positions: u64,
    pub decoder_products: u64,
    pub vocabulary_scores: u64,
}

/// A caller-selected aggregate ceiling over the immutable case roster. Native
/// per-run telemetry and per-comparison state bounds still apply separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignBudget(pub CampaignCost);
impl Default for CampaignBudget {
    fn default() -> Self {
        Self(CampaignCost {
            cases: MAX_CAMPAIGN_CASES,
            replay_positions: MAX_CAMPAIGN_CASES as u64 * MAX_GENERATION_TOKENS as u64,
            paired_positions: MAX_CAMPAIGN_CASES as u64 * MAX_GENERATION_TOKENS as u64,
            decoder_products: MAX_CAMPAIGN_CASES as u64 * 3 * MAX_DECODER_PRODUCTS,
            vocabulary_scores: MAX_CAMPAIGN_CASES as u64 * 3 * MAX_GENERATION_SCORES,
        })
    }
}
impl CampaignCost {
    fn add(self, other: Self) -> Result<Self, Error> {
        let add = |a: u64, b: u64| a.checked_add(b).ok_or(Error::Overflow);
        Ok(Self {
            cases: self.cases.checked_add(other.cases).ok_or(Error::Overflow)?,
            replay_positions: add(self.replay_positions, other.replay_positions)?,
            paired_positions: add(self.paired_positions, other.paired_positions)?,
            decoder_products: add(self.decoder_products, other.decoder_products)?,
            vocabulary_scores: add(self.vocabulary_scores, other.vocabulary_scores)?,
        })
    }
    fn fits(self, ceiling: Self) -> bool {
        self.cases <= ceiling.cases && self.replay_positions <= ceiling.replay_positions
            && self.paired_positions <= ceiling.paired_positions
            && self.decoder_products <= ceiling.decoder_products
            && self.vocabulary_scores <= ceiling.vocabulary_scores
    }
}

/// Frozen before any campaign inference. The case ID is a local label, not an
/// authenticated dataset identity or a statement of statistical independence.
#[derive(Debug)]
pub struct CampaignCase {
    id: u64,
    preparation: ComparisonPreparation,
}
impl CampaignCase {
    pub fn new(id: u64, preparation: ComparisonPreparation) -> Result<Self, Error> {
        if id == 0 { return Err(Error::InvalidInput); }
        if !matches!(preparation.status(), ReplayStatus::Pending { compared: 0, .. })
            || preparation.report().compared_positions != 0
            || preparation.pair.status() != ComparisonStatus::Active
            || preparation.pair.position() != 0 { return Err(Error::WrongState); }
        Ok(Self { id, preparation })
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn lineage(&self) -> ComparisonLineage { self.preparation.pair.lineage }

    fn cost(&self) -> Result<CampaignCost, Error> {
        let checkpoint = &self.preparation.replay.checkpoint;
        let recipe = &checkpoint.recipe;
        let positions = self.preparation.pair.limits.positions
            .min(recipe.spec.prompt().len() + recipe.spec.max_new_tokens());
        // Use the ORIGINAL model's estimator. The original replay constructor
        // already recomputed and checked the saved reservation against this
        // recipe, including archive imports; imported counters are not trusted.
        let products = recipe.model.estimate(0, positions)?.scalar_products()?
            .checked_mul(2).ok_or(Error::Overflow)?;
        let scores = (positions.saturating_sub(recipe.spec.prompt().len()) as u64)
            .checked_mul(recipe.spec.sampling().policy.vocabulary() as u64)
            .and_then(|n| n.checked_mul(2)).ok_or(Error::Overflow)?;
        Ok(CampaignCost {
            cases: 1, replay_positions: checkpoint.positions() as u64,
            paired_positions: positions as u64,
            decoder_products: products.checked_add(checkpoint.work().reserved_decoder_products)
                .ok_or(Error::Overflow)?,
            vocabulary_scores: scores.checked_add(checkpoint.work().reserved_vocabulary_scores)
                .ok_or(Error::Overflow)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseOutcome {
    MatchedStop(GenerationStop),
    DecisionDifference { position: u64, baseline: GenerationStatus, candidate: GenerationStatus },
    BothHeld { position: u64 },
    Exhausted,
    VerificationFailed(Error),
    ComparisonFailed(Error),
}

/// Original observations, not caller-authored outcomes. No token, cache, RNG,
/// mutable verifier or executable arm is returned, including at a disagreement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseReport {
    pub id: u64,
    pub lineage: ComparisonLineage,
    pub preparation: PreparationReport,
    pub comparison: Option<ComparisonReport>,
    pub outcome: CaseOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignStatus {
    Ready { next_case: u64 },
    Complete,
    /// A caller that catches an unwind cannot retry or replace the lost case.
    Interrupted { case: u64 },
}

/// Counts cover the fixed roster, not just successful or completed experiments.
/// A distinct declared lineage is NOT evidence of independence or authenticity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignSummary {
    pub status: CampaignStatus,
    pub reserved: CampaignCost,
    pub completed: usize,
    pub pending: usize,
    pub declared_lineages: usize,
    pub matched_stops: usize,
    pub decision_differences: usize,
    pub both_held: usize,
    pub exhausted: usize,
    pub verification_failures: usize,
    pub comparison_failures: usize,
}

/// Owns a frozen roster and only original verified comparison machinery.
/// No append/retry/replacement, mutable preparation, arm or policy promotion API.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::comparison::verified::campaign::ComparisonCampaign;
/// fn escape(campaign: &mut ComparisonCampaign) { campaign.comparison_mut(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::comparison::verified::campaign::CampaignSummary;
/// fn authorize(summary: CampaignSummary) -> Permit { summary }
/// ```
#[derive(Debug)]
pub struct ComparisonCampaign {
    cases: Vec<Option<CampaignCase>>,
    ids: Vec<u64>,
    reports: Vec<CaseReport>,
    reserved: CampaignCost,
    declared_lineages: usize,
    interrupted: bool,
}
impl ComparisonCampaign {
    /// Size/admit the complete campaign before executing ANY baseline or arm.
    /// Repeated source lineages may be useful for ablations, but do not increase
    /// the reported number of distinct declared source lineages.
    pub fn required_cost(cases: &[CampaignCase]) -> Result<CampaignCost, Error> {
        if cases.is_empty() { return Err(Error::InvalidInput); }
        if cases.len() > MAX_CAMPAIGN_CASES { return Err(Error::Limit); }
        let mut ids = BTreeSet::new();
        let mut cost = CampaignCost::default();
        for case in cases {
            if !ids.insert(case.id) { return Err(Error::Duplicate); }
            cost = cost.add(case.cost()?)?;
        }
        Ok(cost)
    }

    pub fn new(cases: Vec<CampaignCase>, budget: CampaignBudget) -> Result<Self, Error> {
        let reserved = Self::required_cost(&cases)?;
        if !budget.0.fits(CampaignBudget::default().0) || !reserved.fits(budget.0) {
            return Err(Error::Limit);
        }
        let declared_lineages = cases.iter().map(|case| {
            let lineage = case.lineage();
            (lineage.stream, lineage.evaluation_origin)
        }).collect::<BTreeSet<_>>().len();
        let mut reports = Vec::new();
        reports.try_reserve_exact(cases.len()).map_err(|_| Error::Limit)?;
        Ok(Self { ids: cases.iter().map(CampaignCase::id).collect(),
            cases: cases.into_iter().map(Some).collect(), reports, reserved,
            declared_lineages, interrupted: false })
    }

    pub fn reports(&self) -> &[CaseReport] { &self.reports }
    pub fn status(&self) -> CampaignStatus {
        if self.interrupted { return CampaignStatus::Interrupted { case: self.ids[self.reports.len()] }; }
        if self.reports.len() == self.ids.len() { CampaignStatus::Complete }
        else { CampaignStatus::Ready { next_case: self.ids[self.reports.len()] } }
    }
    pub fn summary(&self) -> CampaignSummary {
        let mut summary = CampaignSummary { status: self.status(), reserved: self.reserved,
            completed: self.reports.len(), pending: self.ids.len() - self.reports.len(),
            declared_lineages: self.declared_lineages, matched_stops: 0, decision_differences: 0,
            both_held: 0, exhausted: 0, verification_failures: 0, comparison_failures: 0 };
        for report in &self.reports {
            match report.outcome {
                CaseOutcome::MatchedStop(_) => summary.matched_stops += 1,
                CaseOutcome::DecisionDifference { .. } => summary.decision_differences += 1,
                CaseOutcome::BothHeld { .. } => summary.both_held += 1,
                CaseOutcome::Exhausted => summary.exhausted += 1,
                CaseOutcome::VerificationFailed(_) => summary.verification_failures += 1,
                CaseOutcome::ComparisonFailed(_) => summary.comparison_failures += 1,
            }
        }
        summary
    }

    /// Run one declared case to its original terminal boundary. A Result failure
    /// in one experiment becomes a retained row, not permission to omit that
    /// case or stop evaluating its siblings. An unwind instead latches the owner.
    /// The expected ID prevents an accidental retry from executing the next case.
    pub fn run_next(&mut self, expected_case: u64) -> Result<CaseReport, Error> {
        match self.status() {
            CampaignStatus::Ready { next_case } if next_case != expected_case => return Err(Error::Stale),
            CampaignStatus::Ready { .. } => {}
            _ => return Err(Error::WrongState),
        }
        self.interrupted = true;
        let case = self.cases[self.reports.len()].take().ok_or(Error::WrongState)?;
        let report = execute(case);
        self.reports.push(report);
        self.interrupted = false;
        Ok(report)
    }

    /// Completion means every declared case has a row, NOT that every monitor
    /// was quiet or the candidate was safe. Inspect the outcome counts and rows.
    pub fn run_to_completion(&mut self) -> Result<CampaignSummary, Error> {
        while let CampaignStatus::Ready { next_case } = self.status() { self.run_next(next_case)?; }
        if self.interrupted { return Err(Error::WrongState); }
        Ok(self.summary())
    }
}

fn execute(case: CampaignCase) -> CaseReport {
    let CampaignCase { id, mut preparation } = case;
    let lineage = preparation.pair.lineage;
    let result = preparation.advance(usize::MAX);
    let prepared = preparation.report();
    let failed = |error| CaseReport { id, lineage, preparation: prepared,
        comparison: None, outcome: CaseOutcome::VerificationFailed(error) };
    if let Err(error) = result { return failed(error); }
    let mut pair = match preparation.finish() { Ok(pair) => pair, Err(error) => return failed(error) };
    let _result = pair.run_to_stop(); // Original terminal state, including failures, is retained below.
    let outcome = match pair.status() {
        ComparisonStatus::MatchedStop(stop) => CaseOutcome::MatchedStop(stop),
        ComparisonStatus::BothHeld { position } => CaseOutcome::BothHeld { position },
        ComparisonStatus::Exhausted => CaseOutcome::Exhausted,
        ComparisonStatus::Failed(error) => CaseOutcome::ComparisonFailed(error),
        ComparisonStatus::DecisionDifference { position } => match pair.last_step() {
            Some(step) => CaseOutcome::DecisionDifference { position,
                baseline: step.baseline_status(), candidate: step.candidate_status() },
            None => CaseOutcome::ComparisonFailed(Error::Incomplete),
        },
        ComparisonStatus::Active => CaseOutcome::ComparisonFailed(Error::Incomplete),
    };
    CaseReport { id, lineage, preparation: prepared, comparison: Some(pair.report()), outcome }
}
