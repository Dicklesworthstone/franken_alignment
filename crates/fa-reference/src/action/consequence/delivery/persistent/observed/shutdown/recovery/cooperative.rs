//! Stop-only cooperative replay of one original, exclusively locked history.
//! No partial state, live authority, helper session or reviewer role can escape.
use super::{FileHistoryAnchor, FileOversight, FileOversightProfile, FileStoppedHistory,
    FileStoppedRecovery, JournalError, Machine, StopRequest, journal, storage};
use super::super::super::{BaseEvent, Event, machine::Transition};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS, RecoveryReserve};
use crate::Error;
use std::fmt;
use std::path::Path;
use super::super::FileShutdownDrain;

mod registered;
mod admission;
pub use admission::FileAdmissionStoppedHistory;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStopRecoveryStatus {
    Replaying,
    /// Every original transition passed. No stop or fence has been written yet.
    Ready,
    Interrupted,
    Failed(Error),
    Cancelled,
}

/// Original event calls, not a wall-time, I/O or numerical-work measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStopRecoveryProgress {
    pub replayed_events: usize,
    pub attempted_events: usize,
    pub total_events: usize,
    pub status: FileStopRecoveryStatus,
}

/// A terminal-only reconstruction. Unlike live guarded recovery, this object
/// completes only fixed terminal operations: Stop/Fence, stopped drain, or
/// the original combined Stop/Fence/StopProgress transaction.
/// It cannot return a live owner or provision roles from a recorded guard set.
/// Learned/numerical histories requiring an external reconstruction recipe keep
/// their own recovery APIs; this cursor uses the original Machine::new/apply.
///
/// Construction reads and decodes one bounded canonical file synchronously.
/// Polling bounds original event calls, not the work inside an event. Dropping
/// or cancelling at any point before completion preserves canonical AND pending
/// bytes and releases the cooperating writer lock. No clock is read by replay.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecovery;
/// fn no_live_owner(recovery: FileStopRecovery) { recovery.owner(); }
/// ```
/// ```compile_fail,E0382
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecovery;
/// fn no_resume(mut recovery: FileStopRecovery) { recovery.cancel(); recovery.advance(0, 1); }
/// ```
#[must_use = "replay and finish a stop, or drop without changing the journal"]
pub struct FileStopRecovery {
    profile: FileOversightProfile,
    store: storage::Store,
    canonical: Vec<u8>,
    events: Vec<Event>,
    machine: Machine,
    last_drain: Option<FileShutdownDrain>,
    replayed: usize,
    attempted: usize,
    status: FileStopRecoveryStatus,
}

impl fmt::Debug for FileStopRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileStopRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Retain exclusive canonical custody before executing any historical event.
    /// No source read, saved-clock refresh, ordinary fence or role issuance occurs.
    pub fn begin_stopped_recovery(directory: impl AsRef<Path>, profile: FileOversightProfile)
        -> Result<FileStopRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        FileStopRecovery::begin_store(store, profile, None)
    }

    /// Match an independently retained exact prefix before original event replay.
    /// An invalid suffix still refuses; the anchor is never a saved state importer.
    pub fn begin_stopped_recovery_anchored(directory: impl AsRef<Path>, profile: FileOversightProfile,
        anchor: &FileHistoryAnchor) -> Result<FileStopRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        FileStopRecovery::begin_store(store, profile, Some(anchor))
    }
}

impl FileStopRecovery {
    pub(super) fn begin_store(store: storage::Store, profile: FileOversightProfile,
        anchor: Option<&FileHistoryAnchor>) -> Result<Self, JournalError>
    {
        let canonical = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &canonical)?;
        if let Some(anchor) = anchor { anchor.check(&profile, store.identity(), &events)?; }
        let machine = Machine::new(&profile)?;
        Ok(Self { profile, store, canonical, events, machine, replayed: 0, attempted: 0,
            status: FileStopRecoveryStatus::Replaying, last_drain: None })
    }

    /// Optional independent reserve pin, checked before replay. This neither
    /// installs a reserve nor makes an insufficient terminal tail sufficient.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        if self.attempted != 0 || self.status != FileStopRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        let mut reserves = self.events.iter().filter_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(actual)) => Some(*actual), _ => None,
        });
        if reserves.next() != Some(expected) || reserves.next().is_some() {
            return Err(Error::Binding.into());
        }
        Ok(self)
    }

    pub fn progress(&self) -> FileStopRecoveryProgress {
        FileStopRecoveryProgress { replayed_events: self.replayed, attempted_events: self.attempted,
            total_events: self.events.len(), status: self.status }
    }

    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileStopRecoveryProgress, JournalError>
    {
        self.advance_with(expected_events, max_events, Machine::apply)
    }

    fn advance_with(&mut self, expected_events: usize, max_events: usize,
        mut apply: impl FnMut(&mut Machine, &Event) -> Result<Transition, Error>)
        -> Result<FileStopRecoveryProgress, JournalError>
    {
        match self.status {
            FileStopRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            FileStopRecoveryStatus::Failed(error) => return Err(error.into()),
            FileStopRecoveryStatus::Cancelled => return Err(Error::WrongState.into()),
            _ => {}
        }
        if expected_events != self.replayed { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        let end = self.replayed + max_events.min(self.events.len() - self.replayed);
        while self.replayed < end {
            self.status = FileStopRecoveryStatus::Interrupted;
            self.attempted += 1; // At most one attempt per bounded decoded event.
            let result = match apply(&mut self.machine, &self.events[self.replayed]) {
                Ok(result) => result,
                Err(error) => {
                    self.status = FileStopRecoveryStatus::Failed(error);
                    return Err(error.into());
                }
            };
            self.replayed += 1;
            if let Transition::StopProgressed(sweep) = result {
                self.last_drain = Some(FileShutdownDrain { journal_revision: self.replayed as u64, sweep });
            }
        }
        self.status = if self.replayed == self.events.len() { FileStopRecoveryStatus::Ready }
            else { FileStopRecoveryStatus::Replaying };
        Ok(self.progress())
    }

    pub fn cancel(self) -> FileStopRecoveryProgress {
        let mut progress = self.progress();
        if matches!(progress.status, FileStopRecoveryStatus::Replaying | FileStopRecoveryStatus::Ready) {
            progress.status = FileStopRecoveryStatus::Cancelled;
        }
        progress
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        match self.status {
            FileStopRecoveryStatus::Ready => Ok(()),
            FileStopRecoveryStatus::Interrupted => Err(JournalError::Unavailable),
            FileStopRecoveryStatus::Failed(error) => Err(error.into()),
            _ => Err(Error::Incomplete.into()),
        }
    }

    /// Select the EXACT original predecessor under the same lock, only after
    /// complete replay. A recorded operation cannot be replaced by another one.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        if operation == 0 { return Err(Error::InvalidInput.into()); }
        let snapshot = self.machine.snapshot(self.events.len());
        if let Some(receipt) = snapshot.stop {
            let request = receipt.request();
            if request.operation != operation { return Err(Error::Duplicate.into()); }
            return Ok(request);
        }
        Ok(StopRequest { operation, expected_control_sequence: snapshot.control.sequence,
            expected_authority_epoch: snapshot.control.ledger.epoch })
    }

    /// No historical event is replayed again. Only the original terminal
    /// transaction may mutate the journal, and only stopped observations escape.
    pub fn finish_stopped<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.finish_stopped_with_anchor(request, clock).map(|history| history.into_parts().0)
    }

    /// Retain the acknowledged stopped cut, never speculative candidate bytes.
    /// Exact drained retries need no clock, event, fence or replacement, including
    /// at capacity. Intent and canonical identity are checked before the clock;
    /// the original finalizer repeats canonical comparison after preflight and
    /// before cleanup/replacement. I/O failure may still leave a visible result.
    pub fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        if request.operation == 0 { return Err(Error::InvalidInput.into()); }
        let snapshot = self.machine.snapshot(self.events.len());
        if let Some(receipt) = &snapshot.stop {
            let previous = receipt.request();
            if previous != request {
                return Err(if previous.operation == request.operation { Error::Binding }
                    else { Error::Duplicate }.into());
            }
            let progress = self.machine.broker.stop_progress()?;
            if progress.drained() {
                if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
                    return Err(Error::Binding.into());
                }
                self.store.confirm_and_cleanup()?;
                let anchor = FileHistoryAnchor::from_acknowledged(self.events.len(), self.canonical);
                return Ok(FileStoppedHistory::new(
                    FileStoppedRecovery::AlreadyDrained { snapshot, progress }, anchor));
            }
        } else if snapshot.control.sequence != request.expected_control_sequence
            || snapshot.control.ledger.epoch != request.expected_authority_epoch {
            return Err(Error::Stale.into());
        }
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        let now = clock();
        let Self { profile, store, events, machine, canonical, .. } = self;
        let (host, _reviewer) = FileOversight::owner(profile, store, events, machine);
        host.finish_verified_stopped(request, now, &canonical)
    }

    pub(super) fn finish_synchronously(mut self, request: StopRequest,
        clock: impl FnOnce() -> ElapsedTick) -> Result<FileStoppedRecovery, JournalError>
    {
        while self.status == FileStopRecoveryStatus::Replaying {
            self.advance(self.replayed, MAX_JOURNAL_EVENTS)?;
        }
        self.finish_stopped(request, clock)
    }
}

#[cfg(test)]
mod tests;
