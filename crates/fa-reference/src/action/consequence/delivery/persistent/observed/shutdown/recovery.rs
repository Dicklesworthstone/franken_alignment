//! Source-free recovery of one original domain without retaining a live owner.
//! An exact completed retry confirms the same canonical image, not another fence.
use super::{FileOversight, FileOversightProfile, FileStopSweep, JournalError, StopProgress, StopRequest};
use super::super::{Machine, journal, storage};
use super::super::guarded::anchored::FileHistoryAnchor;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::Error;
use std::path::Path;

mod anchored;
mod cooperative;
pub use cooperative::{FileStopRecovery, FileStopRecoveryProgress, FileStopRecoveryStatus,
    FileAdmissionStoppedHistory};

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

/// One acknowledged stopped cut and its exact original history prefix.
/// This contains private operator evidence, never a live owner or role. The
/// anchor covers the same revision as the stop observation, including unresolved
/// and executed charges. It is not an assertion that the domain is drained.
///
/// Capture is under the original exclusive recovery lock. External retention is
/// a separate operation: keep the old archive until the new one is durably
/// acknowledged. Export or report failure cannot undo this acknowledged stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStoppedHistory {
    stopped: FileStoppedRecovery,
    anchor: FileHistoryAnchor,
}
impl FileStoppedHistory {
    pub fn stopped(&self) -> &FileStoppedRecovery { &self.stopped }
    pub fn history_anchor(&self) -> &FileHistoryAnchor { &self.anchor }
    pub fn into_parts(self) -> (FileStoppedRecovery, FileHistoryAnchor) {
        (self.stopped, self.anchor)
    }

    pub(in super::super) fn new(stopped: FileStoppedRecovery, anchor: FileHistoryAnchor) -> Self {
        Self { stopped, anchor }
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
    FileStopRecovery::begin_store(store, profile, None)?.finish_synchronously(request, clock)
}

#[cfg(test)]
mod tests;
