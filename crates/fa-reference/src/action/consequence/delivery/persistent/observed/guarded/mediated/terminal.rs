//! Restricted recovery after an acknowledged terminal write outlives export.
//! The original learned cursor retains its one lock, replay and native checks.
use super::{FileTopologyRequirement, JournalError,
    learned::FileMediatedLearnedRecovery,
    predictive::FilePredictiveMediatedLearnedRecovery};
use super::super::{FileRecoveryFloor, learned_recovery::check_recovery_reserve};
use super::super::super::{
    decoder::learned::{FileLearnedRecovery, FileLearnedRecoveryProgress, FileLearnedRecoveryStatus},
    driver::topology::checkpoint::TopologyCheckpoint,
    shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory},
};
use crate::{Error, action::ElapsedTick};
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use std::fmt;

enum Inner {
    Mediated(FileMediatedLearnedRecovery),
    Predictive(FilePredictiveMediatedLearnedRecovery),
}

/// The SAME guarded learned recovery, restricted to one selected stop operation.
/// A retained source checkpoint may precede only its Stop/Fence/StopProgress
/// suffix. Full original numerical, guard, predictor, policy, floor and native
/// stop verification still precede acknowledgement. No live continuation,
/// topology observer, receipt reconciliation or owner can escape this type.
///
/// The exact retained cut may start its initial stop. A later cut must already
/// contain the matching Stop: ordinary recovery or source work cannot be priced
/// from old counters. The checkpoint and independent history anchor remain
/// trusted custody, not authentication or protection against joint rollback.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::terminal::FileTerminalTopologyRecovery;
/// fn live(run: FileTerminalTopologyRecovery) { run.finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::terminal::FileTerminalTopologyRecovery;
/// fn escape(run: FileTerminalTopologyRecovery) { run.into_inner(); }
/// ```
#[must_use = "complete the selected native stop or drop without changing storage"]
pub struct FileTerminalTopologyRecovery {
    inner: Inner,
    operation: u64,
}

impl fmt::Debug for FileTerminalTopologyRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTerminalTopologyRecovery")
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}

// Both guarded profiles call this before exposing the restricted wrapper. Its
// only changed expectation is availability explained by the checked terminal
// suffix. Every independently supplied floor is kept or strengthened.
pub(super) fn prepare(inner: &FileLearnedRecovery, progress: FileLearnedRecoveryProgress,
    topology: &mut FileTopologyRequirement, minimum: &mut FileRecoveryFloor,
    saved: &TopologyCheckpoint, operation: u64) -> Result<(), JournalError>
{
    if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
        return Err(Error::WrongState.into());
    }
    check_recovery_reserve(inner, RecoveryReserve::terminal())?;
    if &topology.current != saved.expected().graph() || topology.available != saved.available() {
        return Err(Error::Binding.into());
    }
    let (profile, events, identity) = inner.guarded_history();
    let available = saved.terminal_successor_available(profile, identity, events, operation)?;
    let floor = saved.minimum();
    minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
    minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
    minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
    topology.available = available;
    Ok(())
}

impl FileTerminalTopologyRecovery {
    // Only fleet internals may inspect the selected framing or consume this
    // same restricted cursor. The operation restriction and all guards survive.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn shutdown_history(&self)
        -> (&super::super::super::FileOversightProfile,
            &[super::super::super::Event], &std::path::Path, usize)
    {
        match &self.inner {
            Inner::Mediated(run) => run.shutdown_history(),
            Inner::Predictive(run) => run.shutdown_history(),
        }
    }

    pub(in crate::action::consequence::delivery::persistent::observed)
    fn into_shutdown_terminal(self, operation: u64)
        -> Result<super::super::super::shutdown::recovery::FileStopRecovery, JournalError>
    {
        if self.operation != operation { return Err(Error::Binding.into()); }
        match self.inner {
            Inner::Mediated(run) => run.into_shutdown_terminal(),
            Inner::Predictive(run) => run.into_shutdown_terminal(),
        }
    }

    pub(super) fn mediated(inner: FileMediatedLearnedRecovery, operation: u64) -> Self {
        Self { inner: Inner::Mediated(inner), operation }
    }
    pub(super) fn predictive(inner: FilePredictiveMediatedLearnedRecovery, operation: u64) -> Self {
        Self { inner: Inner::Predictive(inner), operation }
    }

    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        match &self.inner {
            Inner::Mediated(run) => run.progress(), Inner::Predictive(run) => run.progress(),
        }
    }

    /// Replay the original bounded event quantum under all original guards.
    /// Ready reports historical verification only; it creates no live authority.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        match &mut self.inner {
            Inner::Mediated(run) => run.advance(expected_events, max_events),
            Inner::Predictive(run) => run.advance(expected_events, max_events),
        }
    }

    /// Acknowledge only the bound operation's original Stop/Fence, or confirm
    /// its exact previous admission. No clock, live owner or new source is used.
    pub fn finish_admission_stop(self) -> Result<FileAdmissionStoppedHistory, JournalError> {
        match self.inner {
            Inner::Mediated(run) => {
                let request = run.stop_request(self.operation)?;
                run.finish_admission_stop(request)
            }
            Inner::Predictive(run) => {
                let request = run.stop_request(self.operation)?;
                run.finish_admission_stop(request)
            }
        }
    }

    /// Drain only the same acknowledged Stop. Native preflight and canonical
    /// checks surround the fallible clock; an already drained retry needs none.
    pub fn finish_stopped_drain<F>(self, clock: F) -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        match self.inner {
            Inner::Mediated(run) => {
                let request = run.stop_request(self.operation)?;
                run.finish_stopped_drain(request, clock)
            }
            Inner::Predictive(run) => {
                let request = run.stop_request(self.operation)?;
                run.finish_stopped_drain(request, clock)
            }
        }
    }

    /// Complete the original combined native stop without an ordinary recovery
    /// fence or live authority first. A failed clock never selects another phase.
    pub fn try_finish_stopped_with_anchor<F>(self, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        match self.inner {
            Inner::Mediated(run) => {
                let request = run.stop_request(self.operation)?;
                run.try_finish_stopped_with_anchor(request, clock)
            }
            Inner::Predictive(run) => {
                let request = run.stop_request(self.operation)?;
                run.try_finish_stopped_with_anchor(request, clock)
            }
        }
    }
}
