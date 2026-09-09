//! Exhaustive search within a small registered intervention menu.
//!
//! This proves no global optimality and predicts no helper votes. Every subset,
//! including the untouched control, retains its result or refusal. Unknown or
//! failed smaller branches prevent an unsupported minimality claim.

use super::{
    CounterfactualReport, Intervention, NextRequirement, PolicyExperiment, validate_edit_bounds,
};
use crate::Error;

pub const MAX_SEARCH_CANDIDATES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Minimality {
    /// Every proper subset has an established exact-policy violation.
    EstablishedWithinMenu,
    /// At least one proper subset also satisfies the exact policy.
    NotMinimal,
    /// No smaller success was established, but a smaller branch is unknown or refused.
    Undetermined,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchCase {
    pub mask: u16,
    pub outcome: Result<CounterfactualReport, Error>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SufficientRepair {
    pub mask: u16,
    pub minimality: Minimality,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepairSearch {
    candidates: Vec<Intervention>,
    cases: Vec<SearchCase>,
    repairs: Vec<SufficientRepair>,
}

impl RepairSearch {
    pub fn candidates(&self) -> &[Intervention] { &self.candidates }
    pub fn cases(&self) -> &[SearchCase] { &self.cases }
    pub fn sufficient_repairs(&self) -> &[SufficientRepair] { &self.repairs }
}

impl PolicyExperiment {
    /// Enumerate ALL subsets of at most eight candidates, at most 256 branches.
    /// Candidate order defines mask bits, not execution authority. Conflicting
    /// alternatives remain explicit refused branches; they are never dropped.
    /// A sufficient repair means exact predicates only, never a new permission.
    pub fn search_repairs(&self, candidates: &[Intervention]) -> Result<RepairSearch, Error> {
        if candidates.len() > MAX_SEARCH_CANDIDATES { return Err(Error::Limit); }
        validate_edit_bounds(candidates)?;
        let mut cases = Vec::with_capacity(1_usize << candidates.len());
        for mask in 0_u16..(1_u16 << candidates.len()) {
            let edits: Vec<_> = candidates.iter().enumerate()
                .filter(|(index, _)| mask & (1_u16 << *index) != 0)
                .map(|(_, edit)| edit.clone()).collect();
            cases.push(SearchCase { mask, outcome: self.run(&edits) });
        }
        let repairs = cases.iter().filter(|case| sufficient(&case.outcome))
            .map(|case| {
                // Inspect ALL proper subsets, not merely one-edit deletions:
                // arbitrary Boolean policies need not be monotone in edits.
                let mut unresolved = false;
                let mut smaller_success = false;
                for smaller in &cases {
                    if smaller.mask == case.mask || smaller.mask & case.mask != smaller.mask {
                        continue;
                    }
                    match &smaller.outcome {
                        Ok(report) => match report.next_requirement() {
                            NextRequirement::FreshIndependentReview => smaller_success = true,
                            NextRequirement::MoreEvidence => unresolved = true,
                            NextRequirement::ExactPolicyViolation => {}
                        },
                        Err(_) => unresolved = true,
                    }
                }
                SufficientRepair {
                    mask: case.mask,
                    minimality: if smaller_success {
                        Minimality::NotMinimal
                    } else if unresolved {
                        Minimality::Undetermined
                    } else {
                        Minimality::EstablishedWithinMenu
                    },
                }
            }).collect();
        Ok(RepairSearch { candidates: candidates.to_vec(), cases, repairs })
    }
}

fn sufficient(outcome: &Result<CounterfactualReport, Error>) -> bool {
    matches!(outcome, Ok(report) if report.next_requirement() == NextRequirement::FreshIndependentReview)
}
