//! The original campaign finalizer under complete mediated recovery and exact
//! independent source custody. Only immutable acknowledged records can escape.
use super::{FileLearnedRecoveryProgress, FileLearnedRecoveryStatus,
    FileMediatedLearnedRecovery, JournalError, check_recovery_reserve};
use crate::Error;
use crate::action::consequence::delivery::persistent::{
    RecoveryReserve,
    observed::{
        decoder::learned::{FileLearnedPolicyPromotionHistory, FileLearnedPolicyPromotionRequest,
            FileLearnedPolicyPromotionReview},
        driver::topology::checkpoint::TopologyCheckpoint,
    },
};
use std::fmt;

/// One original numerical replay sealed to an exact governor request and source
/// checkpoint. Every independent mediated requirement remains mandatory. Ready
/// grants neither effect authority nor a governor, topology observer or owner.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::learned::FileMediatedPolicyPromotionRecovery;
/// fn escape(run: FileMediatedPolicyPromotionRecovery) { run.finish(); }
/// ```
#[must_use = "complete the selected promotion or drop without changing storage"]
pub struct FileMediatedPolicyPromotionRecovery {
    inner: FileMediatedLearnedRecovery,
    saved: TopologyCheckpoint,
    request: FileLearnedPolicyPromotionRequest,
}

impl fmt::Debug for FileMediatedPolicyPromotionRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMediatedPolicyPromotionRecovery")
            .field("operation", &self.request.update().operation())
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

/// Paired immutable records of the SAME acknowledged cut. Source expenditure,
/// original documents and last source operation are preserved, never inferred
/// from the policy report or refreshed from current files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediatedPolicyPromotionHistory {
    promotion: FileLearnedPolicyPromotionHistory,
    topology: TopologyCheckpoint,
}
impl FileMediatedPolicyPromotionHistory {
    pub fn promotion(&self) -> &FileLearnedPolicyPromotionHistory { &self.promotion }
    pub fn topology_checkpoint(&self) -> &TopologyCheckpoint { &self.topology }
    pub fn into_parts(self) -> (FileLearnedPolicyPromotionHistory, TopologyCheckpoint) {
        (self.promotion, self.topology)
    }
}

impl FileMediatedLearnedRecovery {
    /// Bind an independently retained complete source cut before event zero.
    /// Only its unchanged cut or the exact original campaign triple may precede
    /// this retry. No unrelated source, numerical, time or Fence event is waived.
    /// The original campaign guard and terminal reserve must already exist.
    pub fn require_policy_promotion_checkpoint(
        mut self,
        request: FileLearnedPolicyPromotionRequest,
        saved: &TopologyCheckpoint,
    ) -> Result<FileMediatedPolicyPromotionRecovery, JournalError> {
        let progress = self.progress();
        if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        let campaigns = self.expected.oversight.guards.campaigns.ok_or(Error::Incomplete)?;
        check_recovery_reserve(&self.inner, RecoveryReserve::terminal())?;
        if &self.expected.topology.current != saved.expected().graph()
            || self.expected.topology.available != saved.available()
        {
            return Err(Error::Binding.into());
        }
        let (profile, events, identity) = self.inner.guarded_history();
        saved.policy_promotion_successor_available(profile, identity, events, &request)?;
        let floor = saved.minimum();
        let minimum = &mut self.expected.oversight.minimum;
        minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
        minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
        minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
        self.inner = self.inner.select_policy_promotion(request.clone(), campaigns.limits)?;
        Ok(FileMediatedPolicyPromotionRecovery { inner: self, saved: saved.clone(), request })
    }
}

impl FileMediatedPolicyPromotionRecovery {
    pub fn progress(&self) -> FileLearnedRecoveryProgress { self.inner.progress() }

    /// Replay only the original events under the original complete independent
    /// guard, policy, graph, credential and recovery-floor requirements.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        self.inner.advance(expected_events, max_events)
    }

    /// The complete original comparison is immutable evidence, not approval.
    /// A trusted governor must compare it with independent reviewed expectations.
    pub fn policy_promotion_review(&self)
        -> Result<&FileLearnedPolicyPromotionReview, JournalError>
    {
        self.inner.check_ready()?;
        self.inner.inner.policy_promotion_review()
    }

    /// Execute the original native Request/Approve/Promote in one canonical
    /// replacement, after and before complete mediated guard checks. It cancels
    /// only undispatched reservations and preserves all admitted/Unknown charges.
    /// Exact retries bind the original review and acceptance bit and add no event.
    /// Current effective policy must be independently selected even on a retry.
    ///
    /// No clock, source refresh, extra numerical replay, recovery Fence, live
    /// owner or role escapes. Allocation or archive failure after acknowledgment
    /// cannot undo promotion; retry the same request with the old retained pair.
    pub fn finish_promoted_policy(
        mut self,
        reviewed: &FileLearnedPolicyPromotionReview,
        accept_newly_reviewable: bool,
    ) -> Result<FileMediatedPolicyPromotionHistory, JournalError> {
        self.check_ready_custody()?;
        let (inner, receipt, appended) = self.inner.inner.prepare_policy_promotion(
            reviewed, accept_newly_reviewable)?;
        self.inner.inner = inner;
        if appended != 0 {
            self.inner.expected.oversight.effective_policy = receipt.update().policy().clone();
        }
        self.check_ready_custody()?;
        let (profile, _, identity) = self.inner.inner.guarded_history();
        let profile = profile.clone();
        let identity = identity.to_path_buf();
        let promotion = self.inner.inner.finish_policy_promotion(
            receipt, appended, accept_newly_reviewable)?;
        let topology = self.saved.after_promoted_policy(&profile, &identity, &promotion)?;
        Ok(FileMediatedPolicyPromotionHistory { promotion, topology })
    }

    fn check_ready_custody(&self) -> Result<(), JournalError> {
        self.inner.check_ready()?;
        let (profile, events, identity) = self.inner.inner.guarded_history();
        let snapshot = self.inner.inner.verified_guarded_machine()?.snapshot(events.len());
        self.saved.check_policy_promotion_start(profile, identity, events, &snapshot, &self.request)
    }
}
