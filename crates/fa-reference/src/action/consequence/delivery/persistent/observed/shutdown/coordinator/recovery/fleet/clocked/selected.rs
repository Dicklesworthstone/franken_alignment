//! Explicit new terminal work for selected members; fresh evidence for EVERY other member.
//! This selects original operations, not a reduced roster or an automatic retry.
use super::*;

impl FileShutdownCoordinator {
    /// Run original combined recovery only for the named registered domains.
    /// Every unselected member must have an eligible latest retained visit and
    /// is passively resolved in THIS pass. Missing/conflicting/unstopped evidence
    /// never falls through into a command. No historical success is imported.
    ///
    /// The complete independent roster is mandatory. Selection must be nonempty,
    /// distinct and known; order is irrelevant. All identity, latest-visit and
    /// aggregate visit/session/revision admission precedes callbacks and writes.
    /// Only selected members spend new durable visits; all members spend a
    /// session read/attempt. Existing reserve, custody and liability rules apply.
    /// A selected pending intent is an explicit new recovery request, not proof
    /// that its prior operation failed; native idempotency still decides the cut.
    pub fn recover_selected_pass_with_clocks<C, P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], selected: &[u64], max_events: usize,
        clock: C, checkpoint: P) -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool,
    {
        self.run_selected_pass(revision, bindings, max_events,
            (VisitSelection::Explicit(selected), RegisteredPhase::Combined), Some(clock), checkpoint)
    }

    /// Clock-free admission stop only for selected members. All others undergo
    /// the same fresh passive resolution as recover_selected_pass_with_clocks.
    /// No member can be omitted or treated as drained because it was unselected.
    pub fn stop_selected_admission<P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], selected: &[u64], max_events: usize,
        checkpoint: P) -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where P: FnMut(FileShutdownRecoveryCheckpoint) -> bool,
    {
        self.run_selected_pass(revision, bindings, max_events,
            (VisitSelection::Explicit(selected), RegisteredPhase::Admission),
            None::<fn(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>>, checkpoint)
    }

    /// Explicit stopped-drain work only for selected members; active selected
    /// domains still refuse. Unselected undrained receipts are reported as such,
    /// never drained implicitly. Each required selected settlement acquires its
    /// own clock. Cancellation and acknowledgment failure retain an unvisited tail.
    pub fn drain_selected_stopped_with_clocks<C, P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], selected: &[u64], max_events: usize,
        clock: C, checkpoint: P) -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool,
    {
        self.run_selected_pass(revision, bindings, max_events,
            (VisitSelection::Explicit(selected), RegisteredPhase::Drain), Some(clock), checkpoint)
    }
}

#[cfg(test)]
mod tests;
