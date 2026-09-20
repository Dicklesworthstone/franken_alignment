//! Source-free recovery of one original domain without retaining a live owner.
//! An exact completed retry confirms the same canonical image, not another fence.
use super::{FileOversight, FileOversightProfile, FileStopSweep, JournalError, StopProgress, StopRequest};
use super::super::{Machine, journal, storage};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::Error;
use std::path::Path;

/// Observed native state, never a recovered reviewer, automatic key or owner.
/// A completed historical stop may still contain charges for executed effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileStoppedRecovery {
    AlreadyDrained { snapshot: FileDeliverySnapshot, progress: StopProgress },
    Advanced { snapshot: FileDeliverySnapshot, sweep: FileStopSweep },
}
impl FileStoppedRecovery {
    pub fn snapshot(&self) -> &FileDeliverySnapshot {
        match self { Self::AlreadyDrained { snapshot, .. } | Self::Advanced { snapshot, .. } => snapshot }
    }
    pub fn progress(&self) -> &StopProgress {
        match self { Self::AlreadyDrained { progress, .. } => progress,
            Self::Advanced { sweep, .. } => &sweep.progress }
    }
}

impl FileOversight {
    /// Recover into a permanently stopped domain using the ORIGINAL fixed
    /// Stop -> Fence -> StopProgress transaction. The independently supplied
    /// profile and original stop preconditions must match canonical history.
    /// There is no saved submit document, helper evidence, new human offer,
    /// publication retry, live owner or reviewer role in the returned value.
    ///
    /// If this exact stop is already drained, hold the cooperating writer lock,
    /// confirm canonical durability and return its native progress WITHOUT a new
    /// event, fence, clock call or replacement. This works at the journal ceiling
    /// and after a lost reply or an ambiguous final directory sync. A pending
    /// image is never promoted; canonical verification precedes its cleanup.
    ///
    /// Otherwise call the supplied trusted clock once and use the existing
    /// terminal transaction and recovery reserve. Pending/expired liabilities
    /// remain explicit and charged. Clock or persistence failure returns no
    /// candidate receipt. A failure does not imply that no replacement occurred.
    /// This is one local reference sink, not a remote or process-stop guarantee;
    /// storage integrity and anti-rollback remain independent host assumptions.
    pub fn recover_stopped<F>(directory: impl AsRef<Path>, profile: FileOversightProfile,
        request: StopRequest, clock: F) -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        profile.delivery.limits.check()?;
        if request.operation == 0 { return Err(Error::InvalidInput.into()); }
        let store = storage::Store::open(directory.as_ref())?;
        recover_locked(store, profile, request, clock)
    }
}

// Private split for deterministic native Store fault tests. It cannot be used to
// substitute a caller's snapshot, unlock between read/write, or import an owner.
fn recover_locked<F>(store: storage::Store, profile: FileOversightProfile,
    request: StopRequest, clock: F) -> Result<FileStoppedRecovery, JournalError>
where F: FnOnce() -> ElapsedTick {
    let bytes = store.read(profile.delivery.limits.bytes)?;
    let events = journal::decode(&profile, store.identity(), &bytes)?;
    let machine = Machine::replay(&profile, &events)?;
    let snapshot = machine.snapshot(events.len());
    if let Some(receipt) = &snapshot.stop {
        let previous = receipt.request();
        if previous != request {
            return Err(if previous.operation == request.operation { Error::Binding } else { Error::Duplicate }.into());
        }
        let progress = machine.broker.stop_progress()?;
        if progress.drained() {
            store.confirm_and_cleanup()?;
            return Ok(FileStoppedRecovery::AlreadyDrained { snapshot, progress });
        }
    } else if snapshot.control.sequence != request.expected_control_sequence
        || snapshot.control.ledger.epoch != request.expected_authority_epoch {
        return Err(Error::Stale.into());
    }
    // No current evidence is acquired. A noncompleted stop still needs a trusted
    // current clock and the original finite terminal capacity. This local owner
    // never escapes, including on failure or unwind.
    store.confirm_and_cleanup()?;
    let (mut host, _reviewer) = FileOversight::owner(profile, store, events, machine);
    let sweep = host.finish_stopped_recovery(request, clock())?;
    Ok(FileStoppedRecovery::Advanced { snapshot: host.inspect(), sweep })
}

#[cfg(test)]
mod tests;
