//! Restart progress without a surviving in-memory domain owner. Coordinator
//! intent precedes domain I/O; the ORIGINAL registered history remains authority.
pub(super) mod resolution;
pub(super) mod cooperative;
pub(super) mod clocked;
mod phases;
mod reserve;
pub(super) mod fleet;
#[cfg(test)]
mod test_support;
use super::*;
#[cfg(test)]
use super::super::super::{BaseEvent, Event, Machine};
use super::super::recovery::{FileStopRecovery, FileStopRecoveryStatus};
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;

impl FileShutdownCoordinator {
    /// Recover and terminally drain ONE registered local domain. No arbitrary
    /// path/profile, caller-supplied receipt, helper or reviewer is accepted.
    /// The independently registered profile and newest acknowledged whole prefix
    /// are checked under the original exclusive Store lock BEFORE any mutation.
    /// Existing passive numerical-bootstrap admission is preserved before replay.
    ///
    /// Durable visit intent comes first. Stop, fencing and drain then use the
    /// same FileOversight terminal cut; coordinator completion is a separate
    /// acknowledgment. An outer error may therefore follow a successful domain
    /// shutdown. Reopen the coordinator and inspect/recover again, never infer
    /// nonexecution or resend a publication from that error.
    ///
    /// An already drained, matching canonical stop is only reread and confirmed:
    /// it needs no new domain event, time observation, key or recovery capacity.
    /// Otherwise at must be fresh in the registered domain's clock system. All
    /// missing/busy/refused members remain in the original fixed denominator;
    /// other members may be visited after a durably recorded domain refusal.
    pub fn recover_and_drain(
        &mut self, revision: u64, domain: u64, at: ElapsedTick,
    ) -> Result<Result<FileShutdownObservation, JournalError>, JournalError> {
        let (index, attempt) = self.begin_registered_terminal(revision, domain, ShutdownVisitKind::RecoverStopped { at })?;
        let result = self.recover_domain(index, attempt, at);
        self.finish(index, attempt, result)
    }

    fn recover_domain(&mut self, index: usize, attempt: usize, at: ElapsedTick)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        let store = storage::Store::open(&self.campaign.plan.domains[index].directory)?;
        self.recover_locked(index, attempt, at, store)
    }

    // The private split permits deterministic Store barrier injection. There is
    // no external callback and no way for an actor to supply a replacement owner.
    fn recover_locked(&mut self, index: usize, attempt: usize, at: ElapsedTick, store: storage::Store)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        let mut recovery = FileStopRecovery::begin_registered(store, &self.campaign, index)?;
        while recovery.progress().status == FileStopRecoveryStatus::Replaying {
            recovery.advance(recovery.progress().replayed_events, MAX_JOURNAL_EVENTS)?;
        }
        recovery.finish_registered(&mut self.campaign, index, attempt, at)
    }
}

#[cfg(test)]
mod tests;
