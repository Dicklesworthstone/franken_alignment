//! Operation-bound maintenance after an acknowledged credential write outlives
//! independent checkpoint export. The original learned replay keeps its one lock.
use super::super::super::{
    FileOversight,
    credential::FileCredentialStatus,
    decoder::learned::{
        FileLearnedRecovery, FileLearnedRecoveryProgress, FileLearnedRecoveryStatus,
    },
    driver::topology::checkpoint::TopologyCheckpoint,
};
use super::super::{
    FileRecoveryFloor, anchored::FileHistoryAnchor, learned_recovery::check_recovery_reserve,
};
use super::{
    FileTopologyRequirement, JournalError, learned::FileMediatedLearnedRecovery,
    predictive::FilePredictiveMediatedLearnedRecovery,
};
use crate::Error;
use crate::action::consequence::delivery::{
    credential_broker::{
        CredentialChangeReceipt, CredentialRevocationRequest, CredentialRotationRequest,
    },
    persistent::{JournalFailure, RecoveryReserve},
};
use std::fmt;

/// One exact native maintenance operation, never an actor-supplied effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCredentialMaintenanceRequest {
    Rotate(CredentialRotationRequest),
    Revoke(CredentialRevocationRequest),
}
impl FileCredentialMaintenanceRequest {
    pub(crate) fn operation(self) -> u64 {
        match self {
            Self::Rotate(request) => request.operation,
            Self::Revoke(request) => request.operation,
        }
    }
}
enum Inner {
    Mediated(FileMediatedLearnedRecovery),
    Predictive(FilePredictiveMediatedLearnedRecovery),
}

/// The original guarded learned replay, restricted to one credential operation.
/// Only its matching Rotate/Revoke and recovery Fence suffix may follow the
/// independently retained source checkpoint. The original exact prefix, guards,
/// current credential epoch, predictor, policy, floors and terminal reserve
/// remain mandatory. No live observer, publication role or general owner escapes.
///
/// Ready establishes historical validity only. Acknowledgement performs the
/// original single recovery Fence and returns a maintenance-only owner. A lost
/// successor export cannot refund source work or recertify unavailable topology.
/// The checkpoint and independent history anchor remain trusted custody, not
/// authentication or protection against their joint rollback.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::credential::FileCredentialTopologyRecovery;
/// fn live(run: FileCredentialTopologyRecovery) { run.finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::credential::FileCredentialTopologyRecovery;
/// fn escape(run: FileCredentialTopologyRecovery) { run.into_inner(); }
/// ```
#[must_use = "complete the selected credential maintenance or drop without changing storage"]
pub struct FileCredentialTopologyRecovery {
    inner: Inner,
    saved: TopologyCheckpoint,
    request: FileCredentialMaintenanceRequest,
}
impl fmt::Debug for FileCredentialTopologyRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCredentialTopologyRecovery")
            .field("operation", &self.request.operation())
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

// Admission changes only the availability explained by the exact operation's
// suffix. Independent floors are kept or strengthened, never replaced by an
// older checkpoint. Full original replay still validates every event.
pub(super) fn prepare(
    inner: &FileLearnedRecovery,
    progress: FileLearnedRecoveryProgress,
    topology: &mut FileTopologyRequirement,
    minimum: &mut FileRecoveryFloor,
    saved: &TopologyCheckpoint,
    request: FileCredentialMaintenanceRequest,
) -> Result<(), JournalError> {
    if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
        return Err(Error::WrongState.into());
    }
    check_recovery_reserve(inner, RecoveryReserve::terminal())?;
    if &topology.current != saved.expected().graph() || topology.available != saved.available() {
        return Err(Error::Binding.into());
    }
    let (profile, events, identity) = inner.guarded_history();
    let available = saved.credential_successor_available(profile, identity, events, request)?;
    let floor = saved.minimum();
    minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
    minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
    minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
    topology.available = available;
    Ok(())
}

impl FileCredentialTopologyRecovery {
    pub(super) fn mediated(
        inner: FileMediatedLearnedRecovery,
        saved: &TopologyCheckpoint,
        request: FileCredentialMaintenanceRequest,
    ) -> Self {
        Self {
            inner: Inner::Mediated(inner),
            saved: saved.clone(),
            request,
        }
    }
    pub(super) fn predictive(
        inner: FilePredictiveMediatedLearnedRecovery,
        saved: &TopologyCheckpoint,
        request: FileCredentialMaintenanceRequest,
    ) -> Self {
        Self {
            inner: Inner::Predictive(inner),
            saved: saved.clone(),
            request,
        }
    }
    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        match &self.inner {
            Inner::Mediated(run) => run.progress(),
            Inner::Predictive(run) => run.progress(),
        }
    }
    /// Replay the original bounded event quantum under every original guard.
    /// Ready creates no live authority and acknowledges no journal change.
    pub fn advance(
        &mut self,
        expected_events: usize,
        max_events: usize,
    ) -> Result<FileLearnedRecoveryProgress, JournalError> {
        match &mut self.inner {
            Inner::Mediated(run) => run.advance(expected_events, max_events),
            Inner::Predictive(run) => run.advance(expected_events, max_events),
        }
    }
    /// Read the actual current credential state only after full guarded replay.
    pub fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError> {
        match &self.inner {
            Inner::Mediated(run) => run.credential_status(),
            Inner::Predictive(run) => run.credential_status(),
        }
    }
    /// Native preflight for the one bound operation. An exact retry returns its
    /// original receipt; today's current epoch is checked independently above.
    pub fn check_request(&self) -> Result<Option<CredentialChangeReceipt>, JournalError> {
        match (&self.inner, self.request) {
            (Inner::Mediated(run), FileCredentialMaintenanceRequest::Rotate(request)) => {
                run.check_credential_rotation(request)
            }
            (Inner::Mediated(run), FileCredentialMaintenanceRequest::Revoke(request)) => {
                run.check_credential_revocation(request)
            }
            (Inner::Predictive(run), FileCredentialMaintenanceRequest::Rotate(request)) => {
                run.check_credential_rotation(request)
            }
            (Inner::Predictive(run), FileCredentialMaintenanceRequest::Revoke(request)) => {
                run.check_credential_revocation(request)
            }
        }
    }
    /// Check the bound request before the original recovery Fence, discard all
    /// provisioned roles and return only operation-bound maintenance custody.
    /// The same locked native recovery performs canonical revalidation and
    /// storage acknowledgement. A failure exposes no candidate owner.
    pub fn finish_maintenance(self) -> Result<FileCredentialMaintenanceOwner, JournalError> {
        self.check_request()?;
        if self.credential_status()?.is_none() {
            return Err(Error::Missing.into());
        }
        let (host, _roles) = match self.inner {
            Inner::Mediated(run) => run.finish()?,
            Inner::Predictive(run) => run.finish()?,
        };
        Ok(FileCredentialMaintenanceOwner {
            host,
            saved: self.saved,
            request: self.request,
        })
    }
}

/// An acknowledged owner limited to one selected credential rotation/revocation
/// and independent checkpoint export. It grants no topology observer, effect,
/// generation control, clock, receipt reconciliation or arbitrary owner access.
/// The request cannot be changed after recovery admission. Exact retries use
/// the original native receipt and never repeat the credential transition.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::credential::FileCredentialMaintenanceOwner;
/// fn escape(owner: FileCredentialMaintenanceOwner) { owner.into_inner(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::credential::FileCredentialMaintenanceOwner;
/// fn publish(owner: FileCredentialMaintenanceOwner) { owner.into_supervised_driver(); }
/// ```
#[must_use = "apply or retain the selected maintenance cut before dropping custody"]
pub struct FileCredentialMaintenanceOwner {
    host: FileOversight,
    saved: TopologyCheckpoint,
    request: FileCredentialMaintenanceRequest,
}
impl fmt::Debug for FileCredentialMaintenanceOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCredentialMaintenanceOwner")
            .field("operation", &self.request.operation())
            .field("revision", &self.revision())
            .finish_non_exhaustive()
    }
}
impl FileCredentialMaintenanceOwner {
    pub fn revision(&self) -> u64 {
        self.host.revision()
    }
    pub fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError> {
        self.host.credential_status()
    }
    /// A poisoned owner cannot certify a successor. Previously captured exact
    /// acknowledged checkpoints remain suitable for independent retention.
    pub fn storage_failure(&self) -> Option<&JournalFailure> {
        self.host.storage_failure()
    }
    /// Perform only the admitted native request, or return its exact receipt.
    pub fn apply(&mut self) -> Result<CredentialChangeReceipt, JournalError> {
        let revision = self.host.revision();
        match self.request {
            FileCredentialMaintenanceRequest::Rotate(request) => {
                self.host.rotate_credential_guard(revision, request)
            }
            FileCredentialMaintenanceRequest::Revoke(request) => {
                self.host.revoke_credential_guard(revision, request)
            }
        }
    }
    pub fn history_anchor_after(
        &self,
        previous: &FileHistoryAnchor,
    ) -> Result<FileHistoryAnchor, JournalError> {
        self.host.history_anchor_after(previous)
    }
    /// Preserve original source bytes, expenditure and last operation while
    /// capturing only the actual healthy acknowledged maintenance cut.
    pub fn topology_checkpoint(&self) -> Result<TopologyCheckpoint, JournalError> {
        self.saved.after_credential_lifecycle(&self.host)
    }
}
