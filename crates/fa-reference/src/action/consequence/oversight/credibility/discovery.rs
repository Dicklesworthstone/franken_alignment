//! Bounded discovery over the original applied cases that still need a final
//! independent assessment. No member verdict, label or labeling ticket escapes.
use super::{CredibilityLedger, GroundTruth, Record};
use crate::Error;

pub const MAX_PENDING_EVALUATION_PAGE: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingEvaluationStatus {
    Unassessed,
    Censored,
}

/// Minimal identity of an original applied review. Retaining this observation
/// supplies neither the evaluator's owner brand nor a ticket for an assessment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingEvaluationCase {
    round: u64,
    attempt: u64,
    policy_generation: u64,
    congress_generation: u64,
    evidence_root: [u8; 32],
    status: PendingEvaluationStatus,
}

impl PendingEvaluationCase {
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
    pub fn status(&self) -> PendingEvaluationStatus {
        self.status
    }
}

/// One exact, bounded page of retained APPLIED cases across all policy
/// generations. Unapplied reviews have no evaluation ticket and are counted
/// separately. An empty applied domain is not an all-reviews-complete claim.
///
/// Continuation is `(evaluation_revision, next_after_round)`. Every new applied
/// case or changed assessment advances the native revision and invalidates an
/// older continuation, even when the changed case sorts before its boundary.
/// A changed journal time or an unfinished review can update the accompanying
/// observation basis without changing this applied-case inventory revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingEvaluationPage {
    evaluation_revision: u64,
    after_round: Option<u64>,
    cases: Vec<PendingEvaluationCase>,
    next_after_round: Option<u64>,
    total_pending: usize,
    unfinished_reviews: usize,
}

impl PendingEvaluationPage {
    pub fn evaluation_revision(&self) -> u64 {
        self.evaluation_revision
    }
    pub fn after_round(&self) -> Option<u64> {
        self.after_round
    }
    pub fn cases(&self) -> &[PendingEvaluationCase] {
        &self.cases
    }
    /// None closes only the pending applied-case suffix strictly after
    /// `after_round` at `evaluation_revision`, never unseen or future history.
    pub fn next_after_round(&self) -> Option<u64> {
        self.next_after_round
    }
    /// Complete original applied pending/censored count, including cases before
    /// this page's boundary and older policy generations.
    pub fn total_pending(&self) -> usize {
        self.total_pending
    }
    /// Started reviews with no successfully applied case. These cannot be
    /// independently labeled until the original native review is applied.
    pub fn unfinished_reviews(&self) -> usize {
        self.unfinished_reviews
    }
}

fn pending(record: &Record) -> Option<PendingEvaluationStatus> {
    match record.assessments.last().map(|assessment| assessment.truth) {
        None => Some(PendingEvaluationStatus::Unassessed),
        Some(GroundTruth::Censored) => Some(PendingEvaluationStatus::Censored),
        Some(GroundTruth::Benign | GroundTruth::Violation) => None,
    }
}

impl CredibilityLedger {
    pub(crate) fn pending_cases(
        &self,
        continuation: Option<(u64, u64)>,
        unfinished_reviews: usize,
    ) -> Result<PendingEvaluationPage, Error> {
        let after_round = if let Some((revision, round)) = continuation {
            if revision != self.revision {
                return Err(Error::Stale);
            }
            if round == 0 {
                return Err(Error::InvalidInput);
            }
            if self.records.get(&round).and_then(pending).is_none() {
                return Err(Error::Binding);
            }
            Some(round)
        } else {
            None
        };
        let mut cases = Vec::new();
        cases
            .try_reserve(MAX_PENDING_EVALUATION_PAGE)
            .map_err(|_| Error::Limit)?;
        let mut total_pending = 0;
        let mut more = false;
        // The native ledger retains at most MAX_EVALUATION_CASES records.
        // Scan the whole bounded domain so the total cannot hide final-label
        // gaps or older policy strata; retain only this page's minimal entries.
        for record in self.records.values() {
            let Some(status) = pending(record) else {
                continue;
            };
            total_pending += 1;
            if after_round.is_some_and(|round| record.case.round <= round) {
                continue;
            }
            if cases.len() == MAX_PENDING_EVALUATION_PAGE {
                more = true;
                continue;
            }
            cases.push(PendingEvaluationCase {
                round: record.case.round,
                attempt: record.case.attempt,
                policy_generation: record.case.policy_generation,
                congress_generation: record.case.congress_generation,
                evidence_root: record.case.evidence_root,
                status,
            });
        }
        let next_after_round = more.then(|| {
            cases
                .last()
                .expect("another case follows a full bounded page")
                .round
        });
        Ok(PendingEvaluationPage {
            evaluation_revision: self.revision,
            after_round,
            cases,
            next_after_round,
            total_pending,
            unfinished_reviews,
        })
    }
}
