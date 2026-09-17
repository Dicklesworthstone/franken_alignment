//! Cooperative exhaustive repair search, not an optimizer or a permission path.
//! The untouched control, conflicting alternatives and unvisited masks survive.
use super::{ProposalCounterfactualReport, ProposalExperiment};
use super::super::{Intervention, MAX_SEARCH_CANDIDATES, MAX_INTERVENTION_BYTES,
    Minimality, NextRequirement, SufficientRepair, validate_edit_bounds};
use crate::Error;

pub const MAX_PROPOSAL_SEARCH_CASES: usize = 1 << MAX_SEARCH_CANDIDATES;
pub const MAX_PROPOSAL_SEARCH_EDIT_BYTES: usize = MAX_INTERVENTION_BYTES * (MAX_PROPOSAL_SEARCH_CASES / 2);

/// Bounds the WHOLE menu, not a fresh allowance for each successful branch.
/// Edit bytes count payload and key preimage/replacement bytes retained in
/// result rows. They exclude base evidence, enum/trace storage and allocator RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProposalSearchBudget { pub cases: usize, pub retained_edit_bytes: usize }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalSearchStatus { Running, Complete, Cancelled, Interrupted }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProposalSearchWork {
    pub planned_cases: usize,
    pub entered_cases: usize,
    pub completed_cases: usize,
    pub planned_edit_bytes: usize,
    pub retained_edit_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalSearchCase {
    pub mask: u16,
    pub outcome: Result<ProposalCounterfactualReport, Error>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalRepairReport {
    status: ProposalSearchStatus,
    candidates: Vec<Intervention>,
    cases: Vec<ProposalSearchCase>,
    work: ProposalSearchWork,
    repairs: Vec<SufficientRepair>,
}
impl ProposalRepairReport {
    pub fn status(&self) -> ProposalSearchStatus { self.status }
    pub fn candidates(&self) -> &[Intervention] { &self.candidates }
    pub fn cases(&self) -> &[ProposalSearchCase] { &self.cases }
    pub fn work(&self) -> ProposalSearchWork { self.work }
    pub fn sufficient_repairs(&self) -> &[SufficientRepair] { &self.repairs }
    /// NotRun includes any interrupted, unreturned case. Entered work separately
    /// distinguishes that uncertainty from cases that were never attempted.
    pub fn unreported_masks(&self) -> impl Iterator<Item = u16> + '_ {
        (self.cases.len() as u16)..(self.work.planned_cases as u16)
    }
}

/// One step evaluates one bounded branch. A caught unwind retires the cursor;
/// repeated advance cannot turn an interrupted branch into a free retry.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::experiment::proposal::search::ProposalRepairCursor;
/// fn duplicate(search: ProposalRepairCursor) { let _ = search.clone(); }
/// ```
#[derive(Debug)]
pub struct ProposalRepairCursor {
    experiment: ProposalExperiment,
    candidates: Vec<Intervention>,
    cases: Vec<ProposalSearchCase>,
    work: ProposalSearchWork,
    status: ProposalSearchStatus,
}

impl ProposalExperiment {
    pub fn begin_repair_search(&self, candidates: &[Intervention], budget: ProposalSearchBudget)
        -> Result<ProposalRepairCursor, Error>
    {
        if candidates.len() > MAX_SEARCH_CANDIDATES { return Err(Error::Limit); }
        validate_edit_bounds(candidates)?;
        let planned_cases = 1_usize << candidates.len();
        let planned_edit_bytes = edit_bytes(candidates) * (planned_cases / 2);
        if budget.cases > MAX_PROPOSAL_SEARCH_CASES || planned_cases > budget.cases
            || budget.retained_edit_bytes > MAX_PROPOSAL_SEARCH_EDIT_BYTES
            || planned_edit_bytes > budget.retained_edit_bytes { return Err(Error::Limit); }
        let mut cases = Vec::new(); cases.try_reserve_exact(planned_cases).map_err(|_| Error::Limit)?;
        Ok(ProposalRepairCursor { experiment: self.clone(), candidates: candidates.to_vec(), cases,
            status: ProposalSearchStatus::Running,
            work: ProposalSearchWork { planned_cases, entered_cases: 0, completed_cases: 0,
                planned_edit_bytes, retained_edit_bytes: 0 } })
    }
}
impl ProposalRepairCursor {
    pub fn status(&self) -> ProposalSearchStatus { self.status }
    pub fn work(&self) -> ProposalSearchWork { self.work }
    pub fn cases(&self) -> &[ProposalSearchCase] { &self.cases }
    pub fn advance(&mut self) -> Result<ProposalSearchStatus, Error> {
        match self.status {
            ProposalSearchStatus::Complete => return Ok(self.status),
            ProposalSearchStatus::Running => {}
            _ => return Err(Error::WrongState),
        }
        self.status = ProposalSearchStatus::Interrupted;
        let mask = self.cases.len() as u16;
        self.work.entered_cases += 1;
        let edits: Vec<_> = self.candidates.iter().enumerate()
            .filter(|(index, _)| mask & (1_u16 << *index) != 0)
            .map(|(_, edit)| edit.clone()).collect();
        let outcome = self.experiment.run(&edits);
        if let Ok(report) = &outcome { self.work.retained_edit_bytes += edit_bytes(report.interventions()); }
        self.cases.push(ProposalSearchCase { mask, outcome });
        self.work.completed_cases += 1;
        self.status = if self.cases.len() == self.work.planned_cases {
            ProposalSearchStatus::Complete
        } else { ProposalSearchStatus::Running };
        Ok(self.status)
    }
    pub fn cancel(&mut self) -> Result<(), Error> {
        match self.status {
            ProposalSearchStatus::Running | ProposalSearchStatus::Cancelled => {
                self.status = ProposalSearchStatus::Cancelled; Ok(())
            }
            _ => Err(Error::WrongState),
        }
    }
    /// Complete searches alone can establish within-menu minimality. All proper
    /// subsets matter: a Boolean policy need not be monotone under edits.
    pub fn report(&self) -> ProposalRepairReport {
        let repairs = self.cases.iter().filter(|case| matches!(&case.outcome,
            Ok(report) if report.next_requirement() == NextRequirement::FreshIndependentReview))
            .map(|case| {
                let mut unresolved = self.status != ProposalSearchStatus::Complete;
                let mut smaller_success = false;
                for smaller in &self.cases {
                    if smaller.mask == case.mask || smaller.mask & case.mask != smaller.mask { continue; }
                    match &smaller.outcome {
                        Ok(report) => match report.next_requirement() {
                            NextRequirement::FreshIndependentReview => smaller_success = true,
                            NextRequirement::MoreEvidence => unresolved = true,
                            NextRequirement::ExactPolicyViolation => {}
                        },
                        Err(_) => unresolved = true,
                    }
                }
                SufficientRepair { mask: case.mask, minimality: if smaller_success { Minimality::NotMinimal }
                    else if unresolved { Minimality::Undetermined } else { Minimality::EstablishedWithinMenu } }
            }).collect();
        ProposalRepairReport { status: self.status, candidates: self.candidates.clone(),
            cases: self.cases.clone(), work: self.work, repairs }
    }
    pub fn finish(&self) -> Result<ProposalRepairReport, Error> {
        if self.status != ProposalSearchStatus::Complete { return Err(Error::Incomplete); }
        Ok(self.report())
    }
}

// Called only after the original edit bounds have checked the entire menu, or
// on its successful subsets. All terms and products fit the fixed global caps.
fn edit_bytes(edits: &[Intervention]) -> usize {
    edits.iter().map(|edit| match edit {
        Intervention::Payload(value) => value.len(),
        Intervention::Key { expected, replacement, .. } =>
            expected.as_ref().map_or(0, Vec::len) + replacement.as_ref().map_or(0, Vec::len),
        _ => 0,
    }).sum()
}
