//! Phase selection uses the original full-roster admission and checkpoint loop.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum RegisteredPhase { Combined, Admission, Drain }

impl FileShutdownCoordinator {
    /// Withdraw admission across the complete registered roster without a time
    /// provider. Actual stop progress and charges remain visible; stopped does
    /// not imply drained. No member disappears on cancellation or refusal.
    pub fn stop_registered_admission<P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], max_events: usize, checkpoint: P)
        -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where P: FnMut(FileShutdownRecoveryCheckpoint) -> bool {
        self.admission_pass(revision, bindings, max_events, false, checkpoint)
    }

    /// Resolve eligible saved visits, with clock-free admission stop ONLY for
    /// members selected for new work. A missing/unstopped saved image refuses;
    /// it never falls through into a new stop or a drain.
    pub fn resume_registered_admission<P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], max_events: usize, checkpoint: P)
        -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where P: FnMut(FileShutdownRecoveryCheckpoint) -> bool {
        self.admission_pass(revision, bindings, max_events, true, checkpoint)
    }

    fn admission_pass<P>(&mut self, revision: u64, bindings: &[FileShutdownClockBinding],
        max_events: usize, resume: bool, checkpoint: P)
        -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where P: FnMut(FileShutdownRecoveryCheckpoint) -> bool {
        self.run_registered_pass_with_clocks(revision, bindings, max_events, (resume, RegisteredPhase::Admission),
            None::<fn(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>>, checkpoint)
    }

    /// Explicit new drain visits; never stop an active member. Each required
    /// settlement acquires its own bound time and spends one native event.
    pub fn drain_registered_stopped_with_clocks<C, P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], max_events: usize, clock: C, checkpoint: P)
        -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool {
        self.run_registered_pass_with_clocks(revision, bindings, max_events, (false, RegisteredPhase::Drain), Some(clock), checkpoint)
    }

    /// Passive resolution of retained work is still not a command retry. Only
    /// new work selected by the original resume plan may request a stopped drain.
    /// An observed but undrained member needs explicit new work to make progress.
    pub fn resume_registered_stopped_with_clocks<C, P>(&mut self, revision: u64,
        bindings: &[FileShutdownClockBinding], max_events: usize, clock: C, checkpoint: P)
        -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool {
        self.run_registered_pass_with_clocks(revision, bindings, max_events, (true, RegisteredPhase::Drain), Some(clock), checkpoint)
    }
}

#[cfg(test)]
mod tests;
