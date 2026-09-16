//! Full-input and deadline-aware replay of an observed reference congress.
//!
//! The policy archive's original verifier still owns vote and decision replay.
//! This layer checks the actual helper views and accepted phase times against a
//! separately retained pre-vote basis. Equality is NOT authentication: replacing
//! both the archive and its expected anchor remains outside this reference claim.

use super::{CommitteeInput, ObservedReceipt, ReviewWindow};
use crate::action::ElapsedTick;
use crate::action::consequence::gate::containment::session::policy::controller::{
    DecisionArchive, ReplayedDecision, ReviewAnchor,
};
use crate::reducer::MAX_VOTES;
use crate::Error;
use std::rc::Rc;

pub const OBSERVED_ARCHIVE_VERSION: u32 = 1;

/// Obtain from the original session before voting and retain independently.
/// Includes every submitted byte, transformation, projection and input profile;
/// an actor's explanation of which paragraphs mattered is not a dependency set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedReviewAnchor {
    pub policy: ReviewAnchor,
    pub inputs: Rc<CommitteeInput>,
    pub input_revision: u64,
    pub window: ReviewWindow,
    pub started_at: ElapsedTick,
}

/// Untrusted replay data, not a live review, approval, or execution receipt.
/// Timestamp vectors align with the original transcript's acceptance order.
/// Invalid attempted votes are not accepted events and get no timestamp slot.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::oversight::replay::ObservedDecisionArchive;
/// fn promote(archive: ObservedDecisionArchive) -> Permit { archive }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedDecisionArchive {
    pub version: u32,
    pub policy: DecisionArchive,
    pub inputs: Rc<CommitteeInput>,
    pub input_revision: u64,
    pub window: ReviewWindow,
    pub started_at: ElapsedTick,
    pub commit_times: Vec<ElapsedTick>,
    pub reveals_opened_at: ElapsedTick,
    pub reveal_times: Vec<ElapsedTick>,
    pub completed_at: ElapsedTick,
}

impl ObservedDecisionArchive {
    /// Replays evidence consistency only. No broker, provider, elapsed clock or
    /// effect endpoint is read, and the result has no conversion to authority.
    /// Host clocks and the provenance of the expected anchor remain assumptions.
    pub fn verify(&self, expected: &ObservedReviewAnchor) -> Result<ReplayedDecision, Error> {
        if self.version != OBSERVED_ARCHIVE_VERSION { return Err(Error::InvalidInput); }
        if self.commit_times.len() > MAX_VOTES || self.reveal_times.len() > MAX_VOTES {
            return Err(Error::Limit);
        }
        // The original verifier bounds public policy/transcript collections and
        // reconstructs votes, observations, exact disqualifiers and the reducer.
        let replayed = self.policy.verify(&expected.policy)?;
        if self.inputs != expected.inputs || self.input_revision != expected.input_revision
            || self.window != expected.window || self.started_at != expected.started_at
            || self.input_revision == 0 || self.inputs.action() != &expected.policy.action
            || !self.inputs.views().keys().eq(expected.policy.congress.members.keys())
        { return Err(Error::Binding); }
        let transcript = &self.policy.transcript;
        if self.commit_times.len() != transcript.commits.len()
            || self.reveal_times.len() != transcript.reveals.len()
            || transcript.members.len() != self.inputs.views().len()
            || transcript.members.iter().any(|name| !self.inputs.views().contains_key(name))
        { return Err(Error::Binding); }
        if !(self.started_at < self.window.commit_by
            && self.window.commit_by < self.window.reveal_by
            && self.window.reveal_by <= self.inputs.action().spec().deadline)
        { return Err(Error::InvalidInput); }

        let mut previous = self.started_at;
        for &at in &self.commit_times {
            if at < previous || at >= self.window.commit_by { return Err(Error::Stale); }
            previous = at;
        }
        if self.reveals_opened_at < previous { return Err(Error::Stale); }
        // Early reveal is legal only after the complete fixed roster committed.
        // Missing members cannot disappear merely by shrinking a denominator.
        let members = self.inputs.views().len();
        if transcript.commits.len() != members && self.reveals_opened_at < self.window.commit_by {
            return Err(Error::Incomplete);
        }
        previous = self.reveals_opened_at;
        for &at in &self.reveal_times {
            if at < previous || at >= self.window.reveal_by { return Err(Error::Stale); }
            previous = at;
        }
        if self.completed_at < previous { return Err(Error::Stale); }
        if transcript.reveals.len() != members && self.completed_at < self.window.reveal_by {
            return Err(Error::Incomplete);
        }
        // Late completion is a valid HISTORICAL observation. It does not extend
        // an action/key deadline or establish current publication eligibility.
        Ok(replayed)
    }

    /// Bind replay to an actual reported application, checking whole inputs and
    /// timing as well as the original policy/control receipt. Accounting and the
    /// external effect outcome are still outside this receipt's replay claim.
    pub fn verify_receipt(&self, expected: &ObservedReviewAnchor, receipt: &ObservedReceipt)
        -> Result<ReplayedDecision, Error>
    {
        let replayed = self.verify(expected)?;
        receipt.policy.verify_replay(&self.policy, &expected.policy)?;
        if receipt.inputs != self.inputs || receipt.input_revision != self.input_revision
            || receipt.window != self.window || receipt.started_at != self.started_at
            || receipt.completed_at != self.completed_at
        { return Err(Error::Binding); }
        Ok(replayed)
    }
}
