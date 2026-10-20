//! A write-ahead fleet visit with original event-sized replay checkpoints.
//! The mutable borrow prevents concurrent visits; no domain owner escapes.
use super::*;
use super::super::super::recovery::FileStopRecoveryProgress;

/// One durably entered visit and one exclusively locked original domain. Progress
/// is not stop evidence. Only finish acknowledges a result in the coordinator.
/// Dropping leaves Entered and the coordinator unavailable until verified reopen;
/// cancel records a negative completion without changing the domain's files.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::FileShutdownVisitRecovery;
/// fn no_owner(visit: FileShutdownVisitRecovery<'_>) { visit.owner(); }
/// ```
#[must_use = "finish or cancel the visit; dropping requires coordinator recovery"]
pub struct FileShutdownVisitRecovery<'a> {
    coordinator: &'a mut FileShutdownCoordinator,
    index: usize,
    attempt: usize,
    at: ElapsedTick,
    recovery: FileStopRecovery,
}
impl fmt::Debug for FileShutdownVisitRecovery<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileShutdownVisitRecovery").field("domain", &self.domain())
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}
impl FileShutdownCoordinator {
    /// Persist original visit intent BEFORE domain acquisition. Inner errors are
    /// acknowledged native refusals; outer errors leave completion unconfirmed.
    /// Only the registered path/profile and newest retained prefix are accepted.
    /// Construction performs bounded synchronous read/decode, not event replay.
    pub fn begin_recover_and_drain(&mut self, revision: u64, domain: u64, at: ElapsedTick)
        -> Result<Result<FileShutdownVisitRecovery<'_>, JournalError>, JournalError>
    {
        let (index, attempt) = self.begin_registered_terminal(revision, domain, ShutdownVisitKind::RecoverStopped { at })?;
        let result = storage::Store::open(&self.campaign.plan.domains[index].directory)
            .and_then(|store| FileStopRecovery::begin_registered(store, &self.campaign, index));
        match result {
            Ok(recovery) => Ok(Ok(FileShutdownVisitRecovery { coordinator: self, index, attempt, at, recovery })),
            Err(error) => match self.finish(index, attempt, Err(error))? {
                Err(error) => Ok(Err(error)),
                Ok(_) => unreachable!("a refused acquisition has no observation"),
            },
        }
    }
}
impl FileShutdownVisitRecovery<'_> {
    pub fn domain(&self) -> u64 { self.coordinator.campaign.plan.domains[self.index].id }
    pub fn progress(&self) -> FileStopRecoveryProgress { self.recovery.progress() }

    /// Execute at most max_events original transitions. A stale/invalid poll
    /// does no event work. Original failures/unwinds latch; neither a later poll
    /// nor a completed progress record can publish a partially verified domain.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileStopRecoveryProgress, JournalError>
    {
        self.recovery.advance(expected_events, max_events)
    }

    /// Complete only the original fixed native terminal transaction, then the
    /// separate coordinator acknowledgment. A failed latter write cannot undo a
    /// successful domain stop. Reopen/resolve the retained intent, never resend.
    pub fn finish(self) -> Result<Result<FileShutdownObservation, JournalError>, JournalError> {
        let result = self.recovery.finish_registered(&mut self.coordinator.campaign,
            self.index, self.attempt, self.at);
        self.coordinator.finish(self.index, self.attempt, result)
    }

    /// Explicit cancellation consumes the reconstruction and acknowledges a
    /// negative visit (Unavailable), not a Stop or proof of effect nonexecution.
    /// Canonical AND pending domain bytes stay untouched. Coordinator failure
    /// still requires recovery; no hidden retry or unused visit refund occurs.
    pub fn cancel(self) -> Result<(), JournalError> {
        self.recovery.cancel();
        let _ = self.coordinator.finish(self.index, self.attempt, Err(JournalError::Unavailable))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
