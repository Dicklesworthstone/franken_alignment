//! Cooperative operation-bounded execution and cancellation of a frozen campaign.
//! Each operation calls one ORIGINAL replay step or one ORIGINAL paired step.
//! No executable arm, partial verified owner or changed policy can escape.

use super::{CampaignCase, CampaignStatus, CampaignSummary, CaseOutcome, CaseReport,
    ComparisonCampaign, ComparisonLineage, ComparisonPreparation, ComparisonReport,
    ComparisonStatus, PolicyComparison, PreparationReport, ReplayStatus, completed_report};
use crate::Error;

pub const MAX_STEP_OPERATIONS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CasePhase { Verification, Comparison }

/// Read-only observations from the current original owner. During verification
/// there is no paired report, even if saved bytes claim a successful result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseProgress {
    pub id: u64,
    pub lineage: ComparisonLineage,
    pub phase: CasePhase,
    pub preparation: PreparationReport,
    pub comparison: Option<ComparisonReport>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SteppedProgress {
    pub revision: u64,
    /// Counts calls into the original engines, including zero-token finalization
    /// and an exhausted paired step. This is not a wall-clock or FLOP measure.
    pub operations: u64,
    pub summary: CampaignSummary,
    pub active: Option<CaseProgress>,
}

#[derive(Debug)]
enum Stage {
    Verification(Box<ComparisonPreparation>),
    Comparison(Box<PolicyComparison>),
}
#[derive(Debug)]
struct ActiveCase {
    id: u64,
    lineage: ComparisonLineage,
    prepared: PreparationReport,
    stage: Stage,
}
impl ActiveCase {
    fn new(case: CampaignCase) -> Self {
        let lineage = case.lineage();
        let prepared = case.preparation.report();
        Self { id: case.id, lineage, prepared, stage: Stage::Verification(Box::new(case.preparation)) }
    }
    fn progress(&self) -> CaseProgress {
        let (phase, preparation, comparison) = match &self.stage {
            Stage::Verification(owner) => (CasePhase::Verification, owner.report(), None),
            Stage::Comparison(owner) => (CasePhase::Comparison, self.prepared, Some(owner.report())),
        };
        CaseProgress { id: self.id, lineage: self.lineage, phase, preparation, comparison }
    }
    fn cancelled(self) -> CaseReport {
        let progress = self.progress();
        CaseReport { id: self.id, lineage: self.lineage, preparation: progress.preparation,
            comparison: progress.comparison, outcome: CaseOutcome::Cancelled }
    }
}

/// Consumes only an UNSTARTED campaign. No conversion back can discard the active
/// case and expose run_next as a route around its partial verification.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::comparison::verified::campaign::stepped::SteppedCampaign;
/// fn skip(owner: SteppedCampaign) { owner.into_campaign(); }
/// ```
#[derive(Debug)]
pub struct SteppedCampaign {
    campaign: ComparisonCampaign,
    active: Option<ActiveCase>,
    revision: u64,
    operations: u64,
}
impl ComparisonCampaign {
    pub fn into_stepped(self) -> Result<SteppedCampaign, Error> {
        if self.interrupted || !self.reports.is_empty() { return Err(Error::WrongState); }
        Ok(SteppedCampaign { campaign: self, active: None, revision: 0, operations: 0 })
    }
}
impl SteppedCampaign {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn reports(&self) -> &[CaseReport] { self.campaign.reports() }
    pub fn progress(&self) -> SteppedProgress {
        SteppedProgress { revision: self.revision, operations: self.operations,
            summary: self.campaign.summary(), active: self.active.as_ref().map(ActiveCase::progress) }
    }

    /// At most max_operations original calls, never a hidden run-to-stop. One
    /// paired call can compute one token in EACH arm; replay calls compute at
    /// most one baseline token. Final comparison/encoding remains native-bounded.
    /// Zero is a read and cannot even certify an empty checkpoint. Invalid/stale
    /// calls do no work. Each nonempty accepted call advances the revision once.
    pub fn advance(&mut self, expected_revision: u64, max_operations: usize)
        -> Result<SteppedProgress, Error>
    {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if max_operations > MAX_STEP_OPERATIONS { return Err(Error::Limit); }
        if self.campaign.interrupted { return Err(Error::WrongState); }
        if max_operations == 0 { return Ok(self.progress()); }
        if self.campaign.status() == CampaignStatus::Complete { return Err(Error::WrongState); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        for _ in 0..max_operations {
            if self.campaign.reports.len() == self.campaign.ids.len() { break; }
            // Latch BEFORE taking a case or entering original code. A caught
            // unwind cannot retry a partially computed position or hide its case.
            self.campaign.interrupted = true;
            self.operations = self.operations.checked_add(1).ok_or(Error::Overflow)?;
            self.operation()?;
            self.campaign.interrupted = false;
        }
        Ok(self.progress())
    }

    fn operation(&mut self) -> Result<(), Error> {
        let active = match self.active.take() {
            Some(active) => active,
            None => ActiveCase::new(self.campaign.cases[self.campaign.reports.len()]
                .take().ok_or(Error::WrongState)?),
        };
        let ActiveCase { id, lineage, prepared, stage } = active;
        match stage {
            Stage::Verification(mut owner) => {
                let result = owner.advance(1);
                let prepared = owner.report();
                let failed = |error| CaseReport { id, lineage, preparation: prepared,
                    comparison: None, outcome: CaseOutcome::VerificationFailed(error) };
                match result {
                    Err(error) | Ok(ReplayStatus::Failed(error)) => self.campaign.reports.push(failed(error)),
                    Ok(ReplayStatus::Pending { .. }) => {
                        self.active = Some(ActiveCase { id, lineage, prepared, stage: Stage::Verification(owner) });
                    }
                    Ok(ReplayStatus::Verified) => match (*owner).finish() {
                        Err(error) => self.campaign.reports.push(failed(error)),
                        Ok(pair) => {
                            self.active = Some(ActiveCase { id, lineage, prepared,
                                stage: Stage::Comparison(Box::new(pair)) });
                        }
                    },
                }
            }
            Stage::Comparison(mut owner) => {
                let position = owner.position();
                let result = owner.advance(position);
                if owner.status() == ComparisonStatus::Active {
                    match result {
                        Ok(_) => self.active = Some(ActiveCase { id, lineage, prepared,
                            stage: Stage::Comparison(owner) }),
                        Err(error) => self.campaign.reports.push(CaseReport { id, lineage,
                            preparation: prepared, comparison: Some(owner.report()),
                            outcome: CaseOutcome::ComparisonFailed(error) }),
                    }
                } else {
                    self.campaign.reports.push(completed_report(id, lineage, prepared, &owner));
                }
            }
        }
        Ok(())
    }

    /// Explicitly stop all remaining work, retaining a cancellation row for the
    /// active case AND every queued case. Original partial work stays visible;
    /// no verification, quiet outcome, budget refund or new authority is implied.
    /// Completed rows never change. Repeating cancellation at its current
    /// revision is a read; a stale predecessor is still rejected.
    pub fn cancel(&mut self, expected_revision: u64) -> Result<SteppedProgress, Error> {
        if expected_revision != self.revision { return Err(Error::Stale); }
        if self.campaign.interrupted { return Err(Error::WrongState); }
        if self.campaign.status() == CampaignStatus::Complete { return Ok(self.progress()); }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.campaign.interrupted = true;
        if let Some(active) = self.active.take() { self.campaign.reports.push(active.cancelled()); }
        while self.campaign.reports.len() < self.campaign.ids.len() {
            let case = self.campaign.cases[self.campaign.reports.len()].take().ok_or(Error::WrongState)?;
            let lineage = case.lineage();
            self.campaign.reports.push(CaseReport { id: case.id, lineage,
                preparation: case.preparation.report(), comparison: None, outcome: CaseOutcome::Cancelled });
        }
        self.campaign.interrupted = false;
        Ok(self.progress())
    }

    /// Explicit convenience loop over the bounded step API. As in the original
    /// campaign, completion includes failed/held/cancelled cases, never approval.
    pub fn run_to_completion(&mut self) -> Result<CampaignSummary, Error> {
        while matches!(self.campaign.status(), CampaignStatus::Ready { .. }) {
            self.advance(self.revision, MAX_STEP_OPERATIONS)?;
        }
        if self.campaign.interrupted { return Err(Error::WrongState); }
        Ok(self.campaign.summary())
    }
}
