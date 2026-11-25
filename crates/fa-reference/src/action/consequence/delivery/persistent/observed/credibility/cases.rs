//! Discover original applied cases through the same independently held native
//! evaluator. No journal replay, source read, clock or authority transition.

use super::{FileIndependentEvaluator, FileOversight, JournalError};
use crate::Error;
pub use crate::action::consequence::oversight::credibility::{
    EvaluationCaseStatus as FileEvaluationCaseStatus,
    EvaluationCaseSummary as FileEvaluationCaseSummary, MAX_EVALUATION_CASE_PAGE,
};

/// Exact original acknowledged cut, not a lease or an evaluation capability.
/// Both coordinates must still match before a continuation is disclosed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileEvaluationCasesBasis {
    pub journal_revision: u64,
    pub evaluation_revision: u64,
}

/// A sealed page over APPLIED native cases, across retained policy generations.
/// `total_cases` excludes merely begun/unapplied congresses; their distinct
/// count remains explicit even when there is no discoverable case yet.
///
/// Pending means an applied case has no independent assessment. Censored and
/// Resolved reveal progress only, never truth labels, origins or verdicts.
/// This page cannot create the owner-branded ticket required for assessment.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, observed::credibility::FileEvaluationCasePage,
/// };
/// fn no_permission(page: FileEvaluationCasePage) -> FilePermit { page }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEvaluationCasePage {
    basis: FileEvaluationCasesBasis,
    after_round: Option<u64>,
    total_cases: usize,
    remaining_cases: usize,
    unapplied_rounds: usize,
    cases: Vec<FileEvaluationCaseSummary>,
    next_after: Option<u64>,
}

impl FileEvaluationCasePage {
    pub fn basis(&self) -> FileEvaluationCasesBasis {
        self.basis
    }
    pub fn after_round(&self) -> Option<u64> {
        self.after_round
    }
    pub fn total_cases(&self) -> usize {
        self.total_cases
    }
    /// Exact number of applied cases strictly after the selected boundary,
    /// including this page. A final suffix is not a claim that earlier cases
    /// were returned to this caller.
    pub fn remaining_cases(&self) -> usize {
        self.remaining_cases
    }
    pub fn unapplied_rounds(&self) -> usize {
        self.unapplied_rounds
    }
    pub fn cases(&self) -> &[FileEvaluationCaseSummary] {
        &self.cases
    }
    /// Last returned original round only when a further case exists. A caller
    /// continuing from here must retain this page's exact two-coordinate basis.
    pub fn next_after(&self) -> Option<u64> {
        self.next_after
    }
}

impl FileIndependentEvaluator {
    /// Enumerate one bounded original-case page without reading another owner's
    /// records or reissuing an evaluator, ticket or effect role. The caller's
    /// existing transport/disclosure allowance remains in force.
    ///
    /// An initial read uses `None, None`. A continuation names an actual applied
    /// boundary round and supplies BOTH original journal and evaluation revisions.
    /// Any intervening journal event, even Time or a newly begun review, makes
    /// that continuation stale. No case is silently skipped after an insertion
    /// with a lower round ID, and no new label is spliced into an old page set.
    /// A new initial read may explicitly start a fresh enumeration.
    ///
    /// Healthy stopped owners remain readable just like `evaluation_case`;
    /// the live service separately retires evaluator intake on Stop.
    pub fn evaluation_cases(
        &self,
        host: &FileOversight,
        after_round: Option<u64>,
        expected: Option<FileEvaluationCasesBasis>,
        limit: usize,
    ) -> Result<FileEvaluationCasePage, JournalError> {
        self.check_owner(host)?;
        if limit == 0 || after_round == Some(0) || (after_round.is_some() && expected.is_none()) {
            return Err(Error::InvalidInput.into());
        }
        if limit > MAX_EVALUATION_CASE_PAGE {
            return Err(Error::Limit.into());
        }
        let journal_revision = host.revision();
        if expected.is_some_and(|basis| basis.journal_revision != journal_revision) {
            return Err(Error::Stale.into());
        }
        let page = host.machine.broker.evaluation_cases(
            after_round,
            expected.map(|basis| basis.evaluation_revision),
            limit,
        )?;
        Ok(FileEvaluationCasePage {
            basis: FileEvaluationCasesBasis {
                journal_revision,
                evaluation_revision: page.revision,
            },
            after_round,
            total_cases: page.total_cases,
            remaining_cases: page.remaining_cases,
            unapplied_rounds: page.unapplied_rounds,
            cases: page.cases,
            next_after: page.next_after,
        })
    }
}
