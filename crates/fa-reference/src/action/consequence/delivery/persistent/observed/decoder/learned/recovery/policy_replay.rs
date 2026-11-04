//! Complete retained-policy replay from the original verified learned history.
use super::{FileLearnedRecovery, JournalError};
use crate::Error;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::policy_campaign::{PolicyReplayReport, ReplayLimits};

impl FileLearnedRecovery {
    /// Compare a candidate against ALL retained proposal and review cases under
    /// the effective policy, including exact denials. The SAME machine that
    /// verified the complete original numerical history supplies the corpus;
    /// callers cannot select requests or import favorable archives.
    ///
    /// This is a historical exact-policy comparison, not an effect permit,
    /// campaign approval, policy promotion or new helper judgment. Requirements
    /// outside retained observations remain explicitly shadow-required. No
    /// source refresh, helper execution, clock, fence, sync, cleanup, or journal
    /// append occurs. Numerical work to verify history has already run.
    ///
    /// The canonical image is re-read under the SAME exclusive recovery lock
    /// after the bounded comparison. Changed bytes, incomplete/failed replay,
    /// stale candidates, empty corpora or exhausted limits return no report.
    /// The consumed private machine and lock are dropped on either outcome.
    /// Full journals and staged evidence remain untouched. Access authorization
    /// and any independently retained guard/anchor requirements are the caller's
    /// responsibility; guarded wrappers enforce their original contracts.
    pub fn finish_policy_replay(
        self,
        candidate: Policy,
        limits: ReplayLimits,
    ) -> Result<(FileDeliverySnapshot, PolicyReplayReport), JournalError> {
        let machine = self.verified_guarded_machine()?;
        let report = machine.broker.replay_candidate_policy(candidate, limits)?;
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        Ok((machine.snapshot(self.events.len()), report))
    }
}
