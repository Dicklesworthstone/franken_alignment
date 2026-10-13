//! Acquire native settlement time after replay, never from a saved visit or a
//! pass-wide timestamp. Receipt-only completion deliberately does not call time.
use super::*;
use super::super::super::recovery::FileStopRecoveryProgress;

/// Independent clock identity, not an observation of time or proof of freshness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileShutdownClockBinding {
    pub domain: u64,
    pub clock_domain: u64,
}

impl FileShutdownCoordinator {
    /// Persist a timestamp-free recovery intent, replay under the original domain
    /// lock, then sample the bound clock exactly once if terminal work needs it.
    /// Completed matching stops, acquisition/replay failures and cancellation do
    /// not call clock. Ready is checked BEFORE acquiring time, not after using an
    /// early pass-wide tick. A callback error is an acknowledged native refusal;
    /// an unwind retains Entered and leaves this coordinator unavailable.
    ///
    /// Progress includes zero, bounded event quanta and Ready. Returning false
    /// acknowledges Unavailable without any domain write or staging cleanup.
    /// The clock callback can perform its own elapsed-deadline check and refuse.
    /// Neither callback authenticates a clock, returns a receipt or issues rights.
    /// Domain settlement and coordinator completion are separate acknowledgments.
    pub fn recover_and_drain_with_clock<C, P>(&mut self, revision: u64,
        binding: FileShutdownClockBinding, max_events: usize, clock: C, checkpoint: P)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool,
    {
        if self.unavailable { return Err(JournalError::Unavailable); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        let index = self.campaign.plan.domains.binary_search_by_key(&binding.domain,
            |domain| domain.id()).map_err(|_| Error::Missing)?;
        if self.campaign.plan.domains[index].clock_domain() != binding.clock_domain {
            return Err(Error::Binding.into());
        }
        let pair = self.begin(revision, binding.domain, ShutdownVisitKind::RecoverStoppedAtCompletion)?;
        let result = storage::Store::open(&self.campaign.plan.domains[index].directory)
            .and_then(|store| self.recover_clocked_locked(pair, store, max_events, clock, checkpoint));
        self.finish(pair.0, pair.1, result)
    }

    // The only alternate Store is the existing deterministic fault-injection
    // seam in tests. No caller can supply a different domain or replay machine.
    fn recover_clocked_locked<C, P>(&mut self, pair: (usize, usize), store: storage::Store,
        max_events: usize, clock: C, mut checkpoint: P)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    where C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool,
    {
        let (index, attempt) = pair;
        let domain = &self.campaign.plan.domains[index];
        let binding = FileShutdownClockBinding { domain: domain.id(), clock_domain: domain.clock_domain() };
        let mut recovery = FileStopRecovery::begin_registered(store, &self.campaign, index)?;
        loop {
            let progress = recovery.progress();
            if !checkpoint(progress) { return Err(JournalError::Unavailable); }
            if progress.status == FileStopRecoveryStatus::Ready {
                return recovery.finish_registered_with_clock(&mut self.campaign, index, attempt,
                    || clock(binding));
            }
            recovery.advance(progress.replayed_events, max_events)?;
        }
    }
}

#[cfg(test)]
mod tests;
