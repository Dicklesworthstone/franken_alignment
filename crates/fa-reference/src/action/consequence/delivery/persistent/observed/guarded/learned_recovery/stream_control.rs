//! Original stream control before recovery can interrupt its exact predecessor.
use super::{FileGuardedLearnedRecovery, FileLearnedResetIntent, JournalError};
use crate::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedStreamCheckpointRequest, FileLearnedStreamControlHistory,
};

impl FileGuardedLearnedRecovery {
    /// Capture the exact independently selected active numerical and confirmed
    /// audience cut through the ORIGINAL native capture. The complete guards,
    /// policy, credential epoch and recovery floors are checked before and after
    /// preparation. One ordinary event is acknowledged; an exact historical
    /// retry appends none. No Fence, fresh time, inference, live owner or role is
    /// issued. Lost export can be retried with the same request and new output.
    pub fn finish_captured_stream_checkpoint(mut self, expected: FileLearnedStreamCheckpointRequest)
        -> Result<FileLearnedStreamControlHistory, JournalError>
    {
        self.check_ready()?;
        let (inner, receipt, appended) = self.inner.prepare_captured_stream_checkpoint(expected)?;
        self.inner = inner;
        self.check_ready()?;
        self.inner.finish_stream_control(receipt, appended)
    }

    /// Persist only the independently specified ORIGINAL BeginStreamReset at
    /// its exact native predecessor. No reset audit, refund, rebase or Fence is
    /// performed. The uninterrupted intent remains available to the separately
    /// selected original pending-reset finalizer. A changed or interrupted
    /// retry refuses; an exact pending/completed retry appends zero events.
    ///
    /// The consumed private replay returns only an immutable receipt and anchor.
    /// No role or unfenced owner escapes even when storage fails or unwinds.
    pub fn finish_begun_stream_reset(mut self, expected: &FileLearnedResetIntent,
        first_incident_sequence: u64) -> Result<FileLearnedStreamControlHistory, JournalError>
    {
        self.check_ready()?;
        let (inner, receipt, appended) = self.inner.prepare_begun_stream_reset(
            expected, first_incident_sequence)?;
        self.inner = inner;
        self.check_ready()?;
        self.inner.finish_stream_control(receipt, appended)
    }
}
