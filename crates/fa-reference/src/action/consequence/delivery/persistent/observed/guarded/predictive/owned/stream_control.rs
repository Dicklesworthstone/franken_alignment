//! Original offline stream control with complete owned-predictor guard custody.
use super::{FileLearnedResetIntent, FileOwnedPredictiveRecovery, JournalError};
use crate::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedStreamCheckpointRequest, FileLearnedStreamControlHistory, FileLearnedStreamNarrowRequest,
};

impl FileOwnedPredictiveRecovery {
    /// Persist a real original-gate restriction at the exact captured unsampled
    /// prompt. Both independent guard checks precede canonical replacement.
    /// The receipt's later control sequence can identify the reset incident;
    /// no policy generation, predictor, sample, audit or incident is fabricated.
    /// An exact old retry compares request and witness and adds zero events.
    pub fn finish_narrowed_stream_checkpoint(mut self, expected: &FileLearnedStreamNarrowRequest)
        -> Result<FileLearnedStreamControlHistory, JournalError>
    {
        self.check_ready()?;
        let (inner, receipt, appended) = self.inner.prepare_narrowed_stream_checkpoint(expected)?;
        self.inner = inner;
        self.check_ready()?;
        self.inner.finish_stream_control(receipt, appended)
    }

    /// Capture the exact original unsampled prompt and unchanged confirmed
    /// audience. The independently selected owned predictor, effective policy,
    /// guards and floors qualify before and after the SAME private preparation.
    /// One ordinary event is acknowledged; an exact retry appends none. Neither
    /// a recovery Fence nor a live owner, role or forecast is returned.
    pub fn finish_captured_stream_checkpoint(mut self, expected: FileLearnedStreamCheckpointRequest)
        -> Result<FileLearnedStreamControlHistory, JournalError>
    {
        self.check_ready()?;
        let (inner, receipt, appended) = self.inner.prepare_captured_stream_checkpoint(expected)?;
        self.inner = inner;
        self.check_ready()?;
        self.inner.finish_stream_control(receipt, appended)
    }

    /// Persist only the exact original stream-reset intent, preserving its
    /// predecessor for separately selected completion. No audit, inference,
    /// reservation refund or Fence occurs. Both independent recipes and all
    /// guards remain mandatory around preparation. A changed/interrupted retry
    /// refuses; an exact pending or completed retry adds no event or incident.
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
