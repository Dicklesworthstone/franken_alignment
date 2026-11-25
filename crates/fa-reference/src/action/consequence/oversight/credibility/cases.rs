//! Bounded discovery over the original applied evaluation records. A pending
//! label is a real applied case; a merely begun review is not one.

use super::{CredibilityLedger, GroundTruth, MAX_EVALUATION_CASES};
use crate::Error;
use std::ops::Bound::{Excluded, Unbounded};

pub const MAX_EVALUATION_CASE_PAGE: usize = 16;

/// Label progress without disclosing the label, origin, independent evidence,
/// helper verdicts or whether the original review stopped the action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationCaseStatus {
    Pending,
    Censored,
    Resolved,
}

/// Minimal identity of an original applied review. This is not an evaluation
/// ticket, an assessment, a helper decision or an effect authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationCaseSummary {
    round: u64,
    attempt: u64,
    policy_generation: u64,
    congress_generation: u64,
    evidence_root: [u8; 32],
    status: EvaluationCaseStatus,
}

impl EvaluationCaseSummary {
    pub fn round(&self) -> u64 {
        self.round
    }
    pub fn attempt(&self) -> u64 {
        self.attempt
    }
    pub fn policy_generation(&self) -> u64 {
        self.policy_generation
    }
    pub fn congress_generation(&self) -> u64 {
        self.congress_generation
    }
    pub fn evidence_root(&self) -> [u8; 32] {
        self.evidence_root
    }
    pub fn status(&self) -> EvaluationCaseStatus {
        self.status
    }
}

pub(crate) struct CasePage {
    pub(crate) revision: u64,
    pub(crate) total_cases: usize,
    pub(crate) remaining_cases: usize,
    pub(crate) unapplied_rounds: usize,
    pub(crate) cases: Vec<EvaluationCaseSummary>,
    pub(crate) next_after: Option<u64>,
}

impl CredibilityLedger {
    /// Count the bounded original suffix and copy at most `limit` summaries.
    /// Do not construct a CredibilityReport or clone raw cases or labels.
    pub(crate) fn case_page(
        &self,
        after_round: Option<u64>,
        expected_revision: Option<u64>,
        limit: usize,
    ) -> Result<CasePage, Error> {
        if limit == 0 || after_round == Some(0) {
            return Err(Error::InvalidInput);
        }
        if limit > MAX_EVALUATION_CASE_PAGE || self.records.len() > MAX_EVALUATION_CASES {
            return Err(Error::Limit);
        }
        if expected_revision.is_some_and(|revision| revision != self.revision) {
            return Err(Error::Stale);
        }
        if let Some(round) = after_round {
            if expected_revision.is_none() {
                return Err(Error::InvalidInput);
            }
            if !self.records.contains_key(&round) {
                return Err(Error::Missing);
            }
        }
        let mut records = self
            .records
            .range((after_round.map_or(Unbounded, Excluded), Unbounded));
        let remaining_cases = records.clone().count();
        let mut cases = Vec::new();
        cases
            .try_reserve_exact(limit.min(self.records.len()))
            .map_err(|_| Error::Limit)?;
        for (_, record) in records.by_ref().take(limit) {
            let case = &record.case;
            cases.push(EvaluationCaseSummary {
                round: case.round,
                attempt: case.attempt,
                policy_generation: case.policy_generation,
                congress_generation: case.congress_generation,
                evidence_root: case.evidence_root,
                status: match record.assessments.last().map(|label| label.truth) {
                    None => EvaluationCaseStatus::Pending,
                    Some(GroundTruth::Censored) => EvaluationCaseStatus::Censored,
                    Some(GroundTruth::Benign | GroundTruth::Violation) => {
                        EvaluationCaseStatus::Resolved
                    }
                },
            });
        }
        let next_after = if records.next().is_some() {
            Some(cases.last().ok_or(Error::Incomplete)?.round)
        } else {
            None
        };
        Ok(CasePage {
            revision: self.revision,
            total_cases: self.records.len(),
            remaining_cases,
            // The original broker adds the distinct unapplied-round count.
            unapplied_rounds: 0,
            cases,
            next_after,
        })
    }
}
