//! Passive resolution of learned fleet visits through the same complete cursor.
use super::inputs::Cursor;
use super::*;

impl FileShutdownCoordinator {
    /// Reverify an existing learned visit without a new domain operation, clock,
    /// fence, cleanup or replacement. This explicit learned route requires the
    /// original exclusive recovery lock; ordinary nonlocking resolution is
    /// unchanged. The saved success never substitutes for this fresh replay.
    pub fn resolve_learned_shutdown_visit(
        &mut self,
        revision: u64,
        visit: usize,
        inputs: &FileShutdownLearnedInputs,
    ) -> Result<FileShutdownObservation, JournalError> {
        let mut resolution = self.begin_resolve_learned_shutdown_visit(revision, visit, inputs)?;
        while resolution.progress().status == FileStopRecoveryStatus::Replaying {
            if resolution
                .advance(resolution.progress().replayed_events, MAX_JOURNAL_EVENTS)
                .is_err()
            {
                break;
            }
        }
        resolution.finish()
    }

    /// Freeze one original guarded learned reconstruction for an existing
    /// retained visit. The same newest registered prefix is admitted before
    /// event zero. Any incomplete/failed numerical or guard state is unusable.
    /// Dropping/unwinding retains the old durable visit and quarantines this
    /// coordinator until reopen; explicit cancellation withdraws session success.
    pub fn begin_resolve_learned_shutdown_visit(
        &mut self,
        revision: u64,
        visit: usize,
        inputs: &FileShutdownLearnedInputs,
    ) -> Result<FileShutdownLearnedVisitResolution<'_>, JournalError> {
        let (index, attempt, recorded) = self.prepare_resolution(revision, visit)?;
        match inputs.begin(&self.campaign, index) {
            Ok(read) => Ok(FileShutdownLearnedVisitResolution {
                coordinator: self,
                visit,
                index,
                attempt,
                recorded,
                read,
            }),
            Err(error) => Err(self.refuse_resolution(index, attempt, error)),
        }
    }
}

/// One evidence-only learned resolution. Complete original replay, exact guards,
/// canonical confirmation and retained-visit matching precede any observation.
/// Only a pending coordinator completion may be written; domain bytes are never
/// changed. Exact already-observed refreshes write neither journal.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::FileShutdownLearnedVisitResolution;
/// fn escape(read: FileShutdownLearnedVisitResolution<'_>) { read.owner(); }
/// ```
#[must_use = "finish or cancel this resolution; dropping requires coordinator reopen"]
pub struct FileShutdownLearnedVisitResolution<'a> {
    coordinator: &'a mut FileShutdownCoordinator,
    visit: usize,
    index: usize,
    attempt: usize,
    recorded: Option<u64>,
    read: Cursor,
}
impl fmt::Debug for FileShutdownLearnedVisitResolution<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileShutdownLearnedVisitResolution")
            .field("visit", &self.visit)
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}
impl FileShutdownLearnedVisitResolution<'_> {
    pub fn progress(&self) -> FileStopRecoveryProgress {
        self.read.progress()
    }

    /// Bound original event calls, including their original numerical work.
    /// Event counts do not promise wall-clock bounds for one native operation.
    pub fn advance(
        &mut self,
        expected_events: usize,
        max_events: usize,
    ) -> Result<FileStopRecoveryProgress, JournalError> {
        self.read.advance(expected_events, max_events)
    }

    pub fn finish(self) -> Result<FileShutdownObservation, JournalError> {
        match self.progress().status {
            FileStopRecoveryStatus::Ready | FileStopRecoveryStatus::Failed(_) => {}
            FileStopRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        let result = self
            .read
            .into_terminal(self.coordinator.campaign.plan.operation)
            .and_then(|run| {
                run.finish_registered_observation(&self.coordinator.campaign, self.index)
            });
        self.coordinator.finish_resolution(
            self.visit,
            self.index,
            self.attempt,
            self.recorded,
            result,
        )
    }

    /// Session-only negative evidence. No new durable visit or native refusal
    /// is manufactured, no domain file changes, and no attempt is refunded.
    pub fn cancel(self) -> Result<(), JournalError> {
        if self.progress().status == FileStopRecoveryStatus::Interrupted {
            return Err(JournalError::Unavailable);
        }
        let error = match self.progress().status {
            FileStopRecoveryStatus::Failed(error) => error.into(),
            _ => JournalError::Unavailable,
        };
        let error = self
            .coordinator
            .refuse_resolution(self.index, self.attempt, error);
        if self.coordinator.unavailable {
            Err(error)
        } else {
            Ok(())
        }
    }
}
