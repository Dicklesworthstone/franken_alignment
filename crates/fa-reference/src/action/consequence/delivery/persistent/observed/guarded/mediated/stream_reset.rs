//! Original guarded replay sealed to one independently selected stream reset.
use super::super::super::{
    FileOversight,
    decoder::learned::{FileLearnedRecovery, FileLearnedRecoveryProgress,
        FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent},
    driver::topology::checkpoint::TopologyCheckpoint,
};
use super::super::{FileRecoveryFloor, learned_recovery::check_recovery_reserve};
use super::{FileMediatedRoles, FileTopologyRequirement, JournalError,
    learned::FileMediatedLearnedRecovery};
use crate::{Error, action::consequence::delivery::persistent::RecoveryReserve};
use std::fmt;

/// Replay the complete original numerical history, guards, policy, current
/// credential epoch and source custody. Only the selected StreamReset and Fence
/// suffix may explain lost source-checkpoint export. Availability may withdraw;
/// source bytes, work, graph and independently supplied floors cannot weaken.
///
/// The bound command cannot be replaced after admission. No ordinary finish or
/// live observer can escape before its exact completion. A checked terminal Stop
/// can consume the SAME Ready cursor without issuing roles or restoring numbers.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::stream_reset::FileStreamResetTopologyRecovery;
/// fn live(run: FileStreamResetTopologyRecovery) { run.finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::stream_reset::FileStreamResetTopologyRecovery;
/// fn escape(run: FileStreamResetTopologyRecovery) { run.into_inner(); }
/// ```
#[must_use = "complete the selected reset or drop without changing storage"]
pub struct FileStreamResetTopologyRecovery {
    inner: FileMediatedLearnedRecovery,
    intent: FileLearnedResetIntent,
    first_incident_sequence: u64,
}
impl fmt::Debug for FileStreamResetTopologyRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileStreamResetTopologyRecovery")
            .field("operation", &self.intent.control().operation)
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}

pub(super) fn prepare(inner: &FileLearnedRecovery, progress: FileLearnedRecoveryProgress,
    topology: &mut FileTopologyRequirement, minimum: &mut FileRecoveryFloor,
    saved: &TopologyCheckpoint, intent: &FileLearnedResetIntent, first_incident_sequence: u64,
) -> Result<(), JournalError> {
    if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
        return Err(Error::WrongState.into());
    }
    check_recovery_reserve(inner, RecoveryReserve::terminal())?;
    if &topology.current != saved.expected().graph() || topology.available != saved.available() {
        return Err(Error::Binding.into());
    }
    let (profile, events, identity) = inner.guarded_history();
    let available = saved.stream_reset_successor_available(profile, identity, events,
        intent, first_incident_sequence)?;
    let floor = saved.minimum();
    minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
    minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
    minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
    topology.available = available;
    Ok(())
}

impl FileStreamResetTopologyRecovery {
    pub(super) fn new(inner: FileMediatedLearnedRecovery, intent: &FileLearnedResetIntent,
        first_incident_sequence: u64) -> Self {
        Self { inner, intent: intent.clone(), first_incident_sequence }
    }
    pub fn progress(&self) -> FileLearnedRecoveryProgress { self.inner.progress() }
    /// Advance the original bounded replay under every independent requirement.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError> {
        self.inner.advance(expected_events, max_events)
    }
    /// A checked peer retains priority before the original reset-plus-Fence
    /// transaction. Ready and every original guard are rechecked by the native
    /// Stop handoff; no reset audit, ordinary finish or live role occurs first.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(self, operation: u64)
        -> Result<crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop, JournalError> {
        self.inner.into_recovery_stop(operation)
    }
    /// Complete only the frozen intent and incident frontier, atomically with
    /// the original recovery Fence. An already-completed exact retry adds only
    /// the Fence; it cannot repeat the audit, refund, incident or source work.
    pub fn finish_pending_stream_reset(self) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
        self.inner.finish_pending_stream_reset(&self.intent, self.first_incident_sequence)
    }
}
