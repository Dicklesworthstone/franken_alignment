//! Terminal recovery retains the complete mediated profile, not a weaker cursor.
//! Reuse the original stop request, terminal transaction and no-clock retry.
use super::{FileMediatedRecovery, JournalError, check_machine};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{StopRequest,
    persistent::observed::shutdown::recovery::{FileStoppedHistory, FileStoppedRecovery}};

impl FileMediatedRecovery {
    /// Select one native stop only after the entire original history and ALL
    /// independent mediation requirements have passed. This includes initial and
    /// current graphs, availability, prediction/evaluation, guards and floors.
    /// No topology source is read, clock sampled or authority returned here.
    /// A retained stop reuses its exact request; a different operation refuses.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.inner.stop_request_checked(operation,
            |run| check_machine(&run.profile, &run.machine, &run.events, &self.expected))
    }

    /// Finish through the ORIGINAL Stop -> Fence -> StopProgress transaction.
    /// No ordinary recovery fence or second replay occurs first. Only a stopped
    /// observation escapes; no live owner, helper, reviewer or observer role does.
    /// Dispatched/unknown obligations are settled only by the native endpoint;
    /// missing or expired receipts keep their original charges.
    pub fn finish_stopped<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.finish_stopped_with_anchor(request, clock).map(|history| history.into_parts().0)
    }

    /// Return the stopped observation and exact acknowledged history together.
    /// Recheck the SAME full mediated requirements before intent, canonical or
    /// clock processing. Existing reserve selection and original capacity limits
    /// are retained: ordinary exhaustion can use an already provisioned terminal
    /// reserve, never create one or widen the journal during recovery.
    ///
    /// The original finalizer checks canonical bytes both before the clock and
    /// after terminal preflight. Failure/unwind returns no candidate receipt or
    /// anchor, even if a failed final directory sync left the replacement visible.
    /// Exact drained retries require the correct post-stop requirements and
    /// canonical history, but no new clock, event or replacement at full capacity.
    /// Old graph availability is not silently substituted for the stopped image.
    ///
    /// Import/export custody, trusted storage/clock and declared-topology limits
    /// are unchanged. This does not load learned recipes, stop another process,
    /// authenticate graph completeness or establish external exactly-once effects.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::consequence::delivery::persistent::observed::{
    ///     FileOversight, shutdown::recovery::FileStoppedHistory};
    /// fn no_live_owner(history: FileStoppedHistory) -> FileOversight {
    ///     history.into_parts().0
    /// }
    /// ```
    pub fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        let expected = self.expected;
        self.inner.finish_stopped_checked_with_anchor(request, clock,
            |run| check_machine(&run.profile, &run.machine, &run.events, &expected))
    }
}

#[cfg(test)]
#[path = "stopped_tests.rs"]
mod tests;
