//! Resolve existing shutdown bookkeeping from native evidence, never by retrying
//! a domain command or spending a second durable visit allowance.
use super::*;
use super::super::super::canonical::replay::CanonicalRead;
use crate::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecoveryProgress;

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
        let mut resolution = self.begin_resolve_shutdown_visit(revision, visit)?;
        while resolution.progress().status == FileStopRecoveryStatus::Replaying {
            if resolution.advance(resolution.progress().replayed_events, MAX_JOURNAL_EVENTS).is_err() { break; }
        }
        resolution.finish()
    }

    /// Freeze one passive canonical read for an EXISTING visit. Admission uses
    /// the original visit rules and newest acknowledged prefix. No durable visit
    /// or domain writer lock is acquired, so a live/faulted owner may coexist.
    /// A failed read withdraws session success but leaves durable intent intact.
    pub fn begin_resolve_shutdown_visit(&mut self, revision: u64, visit: usize)
        -> Result<FileShutdownVisitResolution<'_>, JournalError>
    {
        if self.unavailable { return Err(JournalError::Unavailable); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        let retained = self.visits.get(visit).ok_or(Error::Missing)?;
        if !matches!(retained.kind,
            ShutdownVisitKind::Advance { .. } | ShutdownVisitKind::RecoverStopped { .. }
                | ShutdownVisitKind::RecoverStoppedAtCompletion
                | ShutdownVisitKind::StopAdmission | ShutdownVisitKind::DrainStoppedAtCompletion)
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
        self.check_canonical_image()?;
        match self.campaign.begin_canonical_read(index) {
            Ok(read) => Ok(FileShutdownVisitResolution { coordinator: self, visit, index, attempt, recorded, read }),
            Err(error) => Err(self.refuse_resolution(index, attempt, error)),
        }
    }

    fn refuse_resolution(&mut self, index: usize, attempt: usize, error: JournalError) -> JournalError {
        // A domain read refusal or cancellation cannot clear lost coordinator
        // custody. Leave the owner quarantined and preserve both files.
        if let Err(conflict) = self.check_canonical_image() { return conflict; }
        let _ = self.campaign.complete(index, attempt, Err(error.clone()));
        self.acknowledged.current_session = self.campaign.report();
        self.unavailable = false;
        error
    }

    fn finish_resolution(&mut self, visit: usize, index: usize, attempt: usize,
        recorded: Option<u64>, result: Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>)
        -> Result<FileShutdownObservation, JournalError>
    {
        // Exact observed-cut refreshes write nothing, so they must validate
        // coordinator custody here rather than relying only on commit().
        self.check_canonical_image()?;
        let result = result.and_then(|observed| {
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

/// One borrowed, evidence-only resolution. Ready means the complete captured
/// image replayed, NOT that the stop matches the retained visit. Finish checks
/// that match and acknowledges only the original pending bookkeeping if needed.
/// No domain effect, clock, fence, cleanup or fresh command is performed.
///
/// Explicit cancellation withdraws session success without changing either
/// file or the old durable visit. Dropping/unwinding leaves the coordinator
/// unavailable; reopen before reusing it. An interrupted replay cannot be
/// cleared by finish or cancel. A valid concurrent successor does not rewrite
/// this frozen observation into a claim about the latest domain state.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::FileShutdownVisitResolution;
/// fn no_partial_observation(read: &FileShutdownVisitResolution<'_>) { read.observation(); }
/// ```
#[must_use = "finish or cancel this read; dropping requires coordinator reopen"]
pub struct FileShutdownVisitResolution<'a> {
    coordinator: &'a mut FileShutdownCoordinator,
    visit: usize,
    index: usize,
    attempt: usize,
    recorded: Option<u64>,
    read: CanonicalRead,
}
impl std::fmt::Debug for FileShutdownVisitResolution<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileShutdownVisitResolution").field("visit", &self.visit)
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}
impl FileShutdownVisitResolution<'_> {
    pub fn progress(&self) -> FileStopRecoveryProgress { self.read.progress() }

    /// Bound original event calls, not file decoding or the work inside an event.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileStopRecoveryProgress, JournalError>
    {
        self.read.advance(expected_events, max_events)
    }

    pub fn finish(self) -> Result<FileShutdownObservation, JournalError> {
        match self.progress().status {
            FileStopRecoveryStatus::Ready | FileStopRecoveryStatus::Failed(_) => {}
            FileStopRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        self.coordinator.finish_resolution(self.visit, self.index, self.attempt,
            self.recorded, self.read.finish())
    }

    /// Session-only negative observation. No new durable Refused/Observed entry
    /// is written and no spent attempt allowance is replenished.
    pub fn cancel(self) -> Result<(), JournalError> {
        if self.progress().status == FileStopRecoveryStatus::Interrupted { return Err(JournalError::Unavailable); }
        let error = match self.progress().status {
            FileStopRecoveryStatus::Failed(error) => error.into(),
            _ => JournalError::Unavailable,
        };
        let error = self.coordinator.refuse_resolution(self.index, self.attempt, error);
        if self.coordinator.unavailable { Err(error) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod cooperative_tests;
