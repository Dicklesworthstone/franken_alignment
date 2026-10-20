//! Separate registered admission withdrawal from time-dependent settlement.
use super::*;
use super::super::super::recovery::FileStopRecoveryProgress;

impl FileShutdownCoordinator {
    /// Stop the registered domain without acquiring time or settling its effects.
    /// The original Stop/Fence operation preserves charges and the last real
    /// sweep. A matching existing stop is confirmed without appending events.
    /// Completion means admission stopped, not that every liability drained.
    pub fn recover_admission_stop<P>(&mut self, revision: u64,
        binding: FileShutdownClockBinding, max_events: usize, checkpoint: P)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where P: FnMut(FileStopRecoveryProgress) -> bool {
        self.recover_phase(revision, binding, max_events,
            None::<fn(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>>, checkpoint)
    }

    /// Drain only an already-stopped registered domain. Acquire current time
    /// after original replay, then use one StopProgress event, not another
    /// Stop/Fence. Matching drained retries require no clock or domain write.
    /// An active domain refuses before calling time. A failed clock cannot undo
    /// the earlier admission stop or refund an unresolved effect.
    pub fn drain_stopped_with_clock<C, P>(&mut self, revision: u64,
        binding: FileShutdownClockBinding, max_events: usize, clock: C, checkpoint: P)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool {
        self.recover_phase(revision, binding, max_events, Some(clock), checkpoint)
    }

    fn recover_phase<C, P>(&mut self, revision: u64, binding: FileShutdownClockBinding,
        max_events: usize, clock: Option<C>, checkpoint: P)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    where C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool {
        if self.unavailable { return Err(JournalError::Unavailable); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        let index = self.campaign.plan.domains.binary_search_by_key(&binding.domain,
            |domain| domain.id()).map_err(|_| Error::Missing)?;
        if self.campaign.plan.domains[index].clock_domain() != binding.clock_domain {
            return Err(Error::Binding.into());
        }
        let kind = if clock.is_some() { ShutdownVisitKind::DrainStoppedAtCompletion }
            else { ShutdownVisitKind::StopAdmission };
        let pair = self.begin(revision, binding.domain, kind)?;
        let result = storage::Store::open(&self.campaign.plan.domains[index].directory)
            .and_then(|store| self.recover_phase_locked(pair, store, max_events, clock, checkpoint));
        self.finish(pair.0, pair.1, result)
    }

    // Shares original locked replay and acknowledgment with other visit kinds.
    // Only tests can supply the Store for the existing barrier-injection seam.
    fn recover_phase_locked<C, P>(&mut self, pair: (usize, usize), store: storage::Store,
        max_events: usize, clock: Option<C>, mut checkpoint: P)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    where C: FnOnce(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileStopRecoveryProgress) -> bool {
        let (index, attempt) = pair;
        let domain = &self.campaign.plan.domains[index];
        let binding = FileShutdownClockBinding { domain: domain.id(), clock_domain: domain.clock_domain() };
        let mut recovery = FileStopRecovery::begin_registered(store, &self.campaign, index)?;
        loop {
            let progress = recovery.progress();
            if !checkpoint(progress) { return Err(JournalError::Unavailable); }
            if progress.status == FileStopRecoveryStatus::Ready {
                return recovery.finish_registered_phase(&mut self.campaign, index, attempt,
                    clock.map(|clock| move || clock(binding)));
            }
            recovery.advance(progress.replayed_events, max_events)?;
        }
    }
}

#[cfg(test)]
mod tests;
