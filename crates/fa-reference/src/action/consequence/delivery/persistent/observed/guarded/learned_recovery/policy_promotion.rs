//! Trusted offline policy governance without releasing an unfenced role.
use super::{FileGuardedLearnedRecovery, JournalError};
use crate::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedPolicyPromotionHistory, FileLearnedPolicyPromotionRequest,
    FileLearnedPolicyPromotionReview,
};
use crate::Error;

impl FileGuardedLearnedRecovery {
    /// Select one exact campaign operation before event zero. Replay captures
    /// its original full corpus/cut in the SAME numerical pass. This does not
    /// request, approve or persist a campaign. The complete independently
    /// selected campaign inventory must already be present in the original
    /// history; neither this request nor ordinary recovery may enable it late.
    pub fn require_policy_promotion(mut self, request: FileLearnedPolicyPromotionRequest)
        -> Result<Self, JournalError>
    {
        let campaigns = self.expected.guards.campaigns.ok_or(Error::Incomplete)?;
        self.inner = self.inner.select_policy_promotion(request, campaigns.limits)?;
        Ok(self)
    }

    /// Immutable original review after complete numerical and independent guard
    /// verification. A trusted governor must compare this complete value with
    /// independently reviewed expectations before selecting promotion. Cloning
    /// it issues no approval, role, permission or live effect capability.
    pub fn policy_promotion_review(&self) -> Result<&FileLearnedPolicyPromotionReview, JournalError> {
        self.check_ready()?;
        self.inner.policy_promotion_review()
    }

    /// Explicit trusted governor action, separate from actor/helper/human
    /// effect review. The caller authenticates this maintenance authority;
    /// possession of descriptive policy-replay output is not authorization.
    /// The original campaign governor checks shadow coverage and the explicit
    /// newly-reviewable acceptance, then the original promotion cancels only
    /// undispatched reservations and preserves admitted/Unknown liabilities.
    ///
    /// Three ORIGINAL campaign events are committed in one canonical write,
    /// spending ordinary capacity. No Fence, clock, source, helper, inference,
    /// governor, approval key or live owner escapes. Exact acknowledged retries
    /// append zero events and bind the original acceptance bit. A retry must
    /// still independently select the CURRENT effective policy in its recovery
    /// requirements, even if this operation's historical policy is now older.
    pub fn finish_promoted_policy(mut self, reviewed: &FileLearnedPolicyPromotionReview,
        accept_newly_reviewable: bool) -> Result<FileLearnedPolicyPromotionHistory, JournalError>
    {
        self.check_ready()?;
        let (inner, receipt, appended) = self.inner.prepare_policy_promotion(
            reviewed, accept_newly_reviewable)?;
        self.inner = inner;
        if appended != 0 {
            // Only this verified original promotion changes the independently
            // expected policy. Every other guard, credential and floor remains.
            self.expected.effective_policy = receipt.update().policy().clone();
        }
        self.check_ready()?;
        self.inner.finish_policy_promotion(receipt, appended, accept_newly_reviewable)
    }
}
