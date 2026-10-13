//! Cooperatively interrupt an ORIGINAL fixed-roster recovery pass, not a new plan.
use super::*;
use crate::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecoveryProgress;

/// None is the checkpoint BEFORE this domain's intent or I/O. Some is the
/// original replay count, including zero and Ready. No partial stop evidence,
/// authority role, current-source claim or shared clock is carried here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileShutdownRecoveryCheckpoint {
    pub domain: u64,
    pub replay: Option<FileStopRecoveryProgress>,
}

impl FileShutdownCoordinator {
    /// Visit the ENTIRE independently registered roster with operator deadline/
    /// cancellation checks before each visit, between replay quanta and at Ready.
    /// Returning false stops the pass. The caller can use one cumulative Instant;
    /// neither progress nor a new member resets any budget in this implementation.
    ///
    /// Admission (including the full clocks/visit allowance) precedes callbacks.
    /// Before an intent, cancellation leaves that member NotVisited. During
    /// replay it records an Unavailable refusal without touching domain files.
    /// Subsequent members remain explicitly NotVisited; old successes never count
    /// for this pass. A failed cancellation write is Unacknowledged, not a stop.
    ///
    /// A callback unwind is not swallowed. An entered visit remains unavailable
    /// until coordinator reopen; its domain lock is released without a Stop.
    /// Native failures are acknowledged and later members may proceed. Any
    /// coordinator acknowledgment failure terminates the pass immediately.
    ///
    /// This is explicit NEW recovery work, not resolution of retained visits;
    /// resume_registered_pass keeps its separate, read-only-resolution semantics.
    /// Decode, an original event and final storage barriers remain synchronous.
    /// No hard real-time, parallel or simultaneous fleet-stop guarantee is made.
    pub fn recover_registered_pass_with_checkpoints<F>(&mut self, revision: u64,
        clocks: &[FileShutdownClock], max_events: usize, mut checkpoint: F)
        -> Result<FileShutdownRecoveryPass, JournalError>
    where F: FnMut(FileShutdownRecoveryCheckpoint) -> bool {
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        let (mut pass, _) = self.prepare_registered_pass(revision, clocks, false)?;
        for member in &mut pass.members {
            if !checkpoint(FileShutdownRecoveryCheckpoint { domain: member.clock.domain, replay: None }) { break; }
            let mut visit = match self.begin_recover_and_drain(self.revision(), member.clock.domain, member.clock.at) {
                Ok(Ok(visit)) => visit,
                Ok(Err(error)) => { member.result = FileShutdownPassResult::Refused(error); continue; }
                Err(error) => { member.result = FileShutdownPassResult::Unacknowledged(error); break; }
            };
            let mut cancelled = false;
            let result = loop {
                let progress = visit.progress();
                if !checkpoint(FileShutdownRecoveryCheckpoint { domain: member.clock.domain, replay: Some(progress) }) {
                    cancelled = true;
                    break visit.cancel().map(|()| Err(JournalError::Unavailable));
                }
                if progress.status == FileStopRecoveryStatus::Ready { break visit.finish(); }
                if visit.advance(progress.replayed_events, max_events).is_err() {
                    // The native cursor retains the EXACT failure; finish records
                    // that refusal, never a forged cancellation or partial success.
                    break visit.finish();
                }
            };
            match result {
                Ok(Ok(observation)) => member.result = FileShutdownPassResult::Observed(Box::new(observation)),
                Ok(Err(error)) => member.result = FileShutdownPassResult::Refused(error),
                Err(error) => { member.result = FileShutdownPassResult::Unacknowledged(error); break; }
            }
            if cancelled { break; }
        }
        pass.final_revision = self.revision();
        Ok(pass)
    }
}

#[cfg(test)]
mod tests;
