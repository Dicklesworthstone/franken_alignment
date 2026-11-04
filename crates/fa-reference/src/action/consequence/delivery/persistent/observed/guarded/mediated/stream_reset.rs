//! Original guarded replay sealed to one independently selected stream reset.
use super::super::super::{
    FileOversight,
    decoder::learned::{FileLearnedRecovery, FileLearnedRecoveryProgress,
        FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent},
    driver::topology::checkpoint::TopologyCheckpoint,
};
use super::super::{FileRecoveryFloor, learned_recovery::check_recovery_reserve};
use super::{FileMediatedRoles, FileTopologyRequirement, JournalError,
    learned::FileMediatedLearnedRecovery, predictive::FilePredictiveMediatedLearnedRecovery};
use crate::{Error, action::consequence::delivery::persistent::RecoveryReserve};
use std::fmt;

enum Inner {
    Mediated(FileMediatedLearnedRecovery),
    Predictive(FilePredictiveMediatedLearnedRecovery),
}
impl Inner {
    fn progress(&self) -> FileLearnedRecoveryProgress {
        match self { Self::Mediated(run) => run.progress(), Self::Predictive(run) => run.progress() }
    }
    fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        match self {
            Self::Mediated(run) => run.advance(expected_events, max_events),
            Self::Predictive(run) => run.advance(expected_events, max_events),
        }
    }
    #[cfg(target_os = "linux")]
    fn into_recovery_stop(self, operation: u64)
        -> Result<crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop, JournalError>
    {
        match self {
            Self::Mediated(run) => run.into_recovery_stop(operation),
            Self::Predictive(run) => run.into_recovery_stop(operation),
        }
    }
    fn finish_pending_stream_reset(self, intent: &FileLearnedResetIntent, first_incident_sequence: u64)
        -> Result<(FileOversight, FileMediatedRoles), JournalError>
    {
        match self {
            Self::Mediated(run) => run.finish_pending_stream_reset(intent, first_incident_sequence),
            Self::Predictive(run) => run.finish_pending_stream_reset(intent, first_incident_sequence),
        }
    }
    fn finish_admission_stop(self, operation: u64)
        -> Result<super::super::super::shutdown::recovery::FileAdmissionStoppedHistory, JournalError>
    {
        match self {
            Self::Mediated(run) => { let request = run.stop_request(operation)?; run.finish_admission_stop(request) },
            Self::Predictive(run) => { let request = run.stop_request(operation)?; run.finish_admission_stop(request) },
        }
    }
}

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
    inner: Inner,
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
        Self { inner: Inner::Mediated(inner), intent: intent.clone(), first_incident_sequence }
    }
    pub(super) fn predictive(inner: FilePredictiveMediatedLearnedRecovery, intent: &FileLearnedResetIntent,
        first_incident_sequence: u64) -> Self {
        Self { inner: Inner::Predictive(inner), intent: intent.clone(), first_incident_sequence }
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


/// The original complete guarded replay, restricted to confirming a matching
/// Stop already recorded after the independently selected stream-reset intent.
/// This repairs paired custody after completion and Stop exports were both lost.
/// Exact source bytes/work and every independent guard remain mandatory.
///
/// The pre-replay admission proves that Stop is already recorded. Its native
/// exact retry therefore acknowledges the same history without adding reset,
/// Stop, Fence, time or source events. It exposes only a sealed stopped history,
/// never a live owner, observer, reset finalizer, drain clock or first-Stop path.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::stream_reset::FileStoppedStreamResetRecovery;
/// fn resume(run: FileStoppedStreamResetRecovery) { run.finish_pending_stream_reset(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::stream_reset::FileStoppedStreamResetRecovery;
/// fn escape(run: FileStoppedStreamResetRecovery) { run.finish(); }
/// ```
#[must_use = "confirm the selected acknowledged Stop or drop without changing storage"]
pub struct FileStoppedStreamResetRecovery {
    inner: Inner,
    operation: u64,
}
impl fmt::Debug for FileStoppedStreamResetRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileStoppedStreamResetRecovery")
            .field("operation", &self.operation)
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}

pub(super) struct StoppedStreamResetSelection<'a> {
    pub(super) intent: &'a FileLearnedResetIntent,
    pub(super) first_incident_sequence: u64,
    pub(super) operation: u64,
}

pub(super) fn prepare_stopped(inner: &FileLearnedRecovery,
    progress: FileLearnedRecoveryProgress, topology: &mut FileTopologyRequirement,
    minimum: &mut FileRecoveryFloor, saved: &TopologyCheckpoint,
    selection: StoppedStreamResetSelection<'_>,
) -> Result<(), JournalError> {
    if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
        return Err(Error::WrongState.into());
    }
    check_recovery_reserve(inner, RecoveryReserve::terminal())?;
    if &topology.current != saved.expected().graph() || topology.available != saved.available() {
        return Err(Error::Binding.into());
    }
    let (profile, events, identity) = inner.guarded_history();
    let available = saved.stopped_stream_reset_successor_available(profile, identity,
        events, selection.intent, selection.first_incident_sequence, selection.operation)?;
    let floor = saved.minimum();
    minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
    minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
    minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
    topology.available = available;
    Ok(())
}

impl FileStoppedStreamResetRecovery {
    pub(super) fn new(inner: FileMediatedLearnedRecovery, operation: u64) -> Self {
        Self { inner: Inner::Mediated(inner), operation }
    }
    pub(super) fn predictive(inner: FilePredictiveMediatedLearnedRecovery, operation: u64) -> Self {
        Self { inner: Inner::Predictive(inner), operation }
    }
    pub fn progress(&self) -> FileLearnedRecoveryProgress { self.inner.progress() }
    /// Advance the SAME original bounded replay under all independent guards.
    /// Incomplete, failed and dropped cursors acknowledge nothing.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError> {
        self.inner.advance(expected_events, max_events)
    }
    /// Confirm only the original native Stop already proved by admission and
    /// full guarded replay. Native canonical-byte revalidation and staging
    /// confirmation still must succeed before any acknowledged history escapes.
    /// No additional Stop, Fence, clock, reset audit or source work occurs.
    ///
    /// Pair the sealed result with the original source checkpoint through
    /// TopologyCheckpoint::after_stream_reset_admission_stop, then independently
    /// retain both archives. Export failure cannot undo the existing Stop.
    pub fn finish_admission_stop(self)
        -> Result<super::super::super::shutdown::recovery::FileAdmissionStoppedHistory, JournalError>
    {
        self.inner.finish_admission_stop(self.operation)
    }
}
