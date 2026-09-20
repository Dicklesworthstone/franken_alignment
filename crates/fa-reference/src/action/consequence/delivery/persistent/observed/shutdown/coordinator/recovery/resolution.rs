//! Resolve existing shutdown bookkeeping from native evidence, never by retrying
//! a domain command or spending a second durable visit allowance.
use super::*;

impl FileShutdownCoordinator {
    /// Resolve a retained Advance/RecoverStopped visit by reading the ORIGINAL
    /// canonical domain. `visit` is its zero-based index in report().visits.
    /// No domain is opened as an owner, stopped, fenced, drained or resent here;
    /// the old intent's tick is never reused as a current clock observation.
    ///
    /// An Entered visit becomes Observed only after a matching native stop has
    /// been seen and this coordinator's completion replacement is acknowledged.
    /// The original progress may still contain unresolved/charged liabilities;
    /// resolving bookkeeping is NOT evidence that every effect was drained.
    /// No additional durable visit is allocated, even at the visit ceiling.
    ///
    /// An already Observed visit may be rechecked without a journal write ONLY
    /// when the canonical image is still its exact retained cut. This lets a
    /// restarted, fully spent coordinator establish fresh observations after a
    /// lost completion acknowledgment. A newer image requires an ordinary visit;
    /// this method never observes a newer head without retaining its new floor.
    /// A later successful visit for this domain must be used instead, so resolving
    /// an older intent cannot rewrite the ordering of acknowledged history cuts.
    ///
    /// Missing, substituted, unstopped or conflicting evidence leaves Entered
    /// untouched and withdraws this domain's current-session success. Such a read
    /// refusal permits other domains to progress. A caught unwind or failed
    /// coordinator replacement quarantines the coordinator until reopening.
    /// Each read still consumes the campaign's bounded in-memory attempt budget.
    pub fn resolve_shutdown_visit(&mut self, revision: u64, visit: usize)
        -> Result<FileShutdownObservation, JournalError>
    {
        if self.unavailable { return Err(JournalError::Unavailable); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        let retained = self.visits.get(visit).ok_or(Error::Missing)?;
        if !matches!(retained.kind,
            ShutdownVisitKind::Advance { .. } | ShutdownVisitKind::RecoverStopped { .. })
        { return Err(Error::WrongState.into()); }
        let recorded = match &retained.result {
            ShutdownVisitResult::Entered => None,
            ShutdownVisitResult::Observed { domain_revision } => Some(*domain_revision),
            ShutdownVisitResult::Refused { .. } => return Err(Error::WrongState.into()),
        };
        let domain = retained.domain;
        if self.visits[visit + 1..].iter().any(|later| later.domain == domain
            && matches!(later.result, ShutdownVisitResult::Observed { .. }))
        { return Err(Error::WrongState.into()); }
        if recorded.is_none() { self.revision.checked_add(1).ok_or(Error::Overflow)?; }
        let (index, attempt) = self.campaign.enter(domain, FileShutdownStep::InspectCanonical)?;
        // Before native read/replay: an unwind cannot reuse older success.
        self.unavailable = true;
        let result = self.campaign.read_canonical(index).and_then(|observed| {
            let stop = observed.0.stop.as_ref().ok_or(Error::Incomplete)?;
            if stop.receipt.request().operation != self.campaign.plan.operation {
                return Err(Error::Binding.into());
            }
            if recorded.is_some_and(|cut| cut != observed.0.journal_revision) {
                return Err(Error::Stale.into());
            }
            Ok(observed)
        });
        let observation = match self.campaign.complete(index, attempt, result) {
            Ok(observation) => observation,
            Err(error) => {
                // The durable intent is still pending (or the old observation is
                // still historical). Only this session's failed read is updated.
                self.acknowledged.current_session = self.campaign.report();
                self.unavailable = false;
                return Err(error);
            }
        };
        if recorded.is_none() {
            self.visits[visit].result = ShutdownVisitResult::Observed {
                domain_revision: observation.journal_revision,
            };
            self.commit()?;
        } else {
            // Byte-identical retained head: no new journal fact or revision.
            self.acknowledged.current_session = self.campaign.report();
        }
        self.unavailable = false;
        Ok(observation)
    }
}

#[cfg(test)]
mod tests;
