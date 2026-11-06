//! Separately granted supervisor inspection of the original acknowledged owner.
//! The actor and exact-precheck ports cannot provision this capability.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, JournalError, Knowledge, OwnerRef,
    UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::{
    FileOversight,
    replay::situation::{FileSituationAttempt, FileSituationBasis, FileSituationSummary},
};
use std::cell::Cell;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_INSPECTION_QUERIES: usize = 1024;
/// One bounded first page. Further native pages remain an explicit supervisor
/// operation; an offset is never imported into a newly captured live report.
pub const MAX_INSPECTION_ATTEMPTS: usize = 16;

/// A projection of one original native situation page, with its exact basis.
/// Fields are private so public response reconstruction cannot substitute a
/// different origin or clock for this acknowledged capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInspectionPage {
    basis: FileSituationBasis,
    summary: FileSituationSummary,
    attempts: Vec<FileSituationAttempt>,
    offset: usize,
    enumeration_complete: bool,
}

impl FileInspectionPage {
    pub fn basis(&self) -> FileSituationBasis {
        self.basis
    }
    pub fn summary(&self) -> FileSituationSummary {
        self.summary
    }
    pub fn attempts(&self) -> &[FileSituationAttempt] {
        &self.attempts
    }
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn enumeration_complete(&self) -> bool {
        self.enumeration_complete
    }
}

pub type FileInspectionObservation = Knowledge<FileInspectionPage, FileSituationBasis>;
pub type FileInspectionResult = Result<FileInspectionObservation, ActorError>;

struct Grant {
    remaining: Cell<usize>,
}

/// Host custody for one finite, shared read allowance. Dropping or consuming
/// this custodian revokes every port clone without retaining the authority lock.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorInspectionGrant;
/// fn no_copy(grant: FileSupervisorInspectionGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for the explicitly granted supervisor inspection"]
pub struct FileSupervisorInspectionGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorInspectionGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }
    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorInspectionGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorInspectionGrant")
            .field("remaining_queries", &self.remaining_queries())
            .finish_non_exhaustive()
    }
}

/// A supervisor-only read capability. It has no effect, source, clock or mutable
/// host API. Clones share the original allowance and revocation custody.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{observed::FileOversight,
///     requests::actor::FileActorPort};
/// fn no_self_grant(actor: FileActorPort<FileOversight>) { actor.inspection_port(1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPrecheckPort;
/// fn no_upgrade(actor: FileActorPrecheckPort) { actor.inspection_port(1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorInspectionPort;
/// fn no_owner(port: FileSupervisorInspectionPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorInspectionPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
}

impl fmt::Debug for FileSupervisorInspectionPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorInspectionPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Explicitly grant privileged live inspection, separately from actor
    /// submission and policy disclosure. No source/time read, admission snapshot
    /// consumption, immutable situation capture or journal write occurs here.
    /// The grant follows this owner across later acknowledged transitions; it
    /// cannot follow a reopened owner or renew itself through a port clone.
    pub fn inspection_port(
        &self,
        queries: usize,
    ) -> Result<(FileSupervisorInspectionPort, FileSupervisorInspectionGrant), JournalError> {
        if queries == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_INSPECTION_QUERIES {
            return Err(Error::Limit.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let grant = Rc::new(Grant {
            remaining: Cell::new(queries),
        });
        Ok((
            FileSupervisorInspectionPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
            },
            FileSupervisorInspectionGrant { grant },
        ))
    }
}

impl FileSupervisorInspectionPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner)
    }

    /// Read the first bounded page of a fresh original acknowledged situation.
    /// The original report supplies its entire summary and historical basis;
    /// no action stage is inferred, no disk fallback is made, and no remaining
    /// page is claimed complete. Repeated requests spend again, including through
    /// other clones. An output failure cannot refund this entered query.
    pub fn situation(&self) -> FileInspectionResult {
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_inspection",
            });
        };
        let unknown = || Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some() {
            return Ok(unknown());
        }
        let remaining = grant.remaining.get();
        if remaining == 0 {
            return Err(ActorError::Capacity);
        }
        // Charge before the original fallible capture; a caught failure or
        // unwind does not create another allowance or reuse an earlier report.
        grant.remaining.set(remaining - 1);
        let situation = owner
            .host
            .situation()
            .map_err(|_| ActorError::Unavailable)?;
        let observation = situation
            .page(None, MAX_INSPECTION_ATTEMPTS)
            .map_err(|_| ActorError::Unavailable)?;
        let Knowledge::Known { value, basis } = observation else {
            return Err(ActorError::Unavailable);
        };
        Ok(Knowledge::Known {
            value: FileInspectionPage {
                basis,
                summary: value.summary,
                attempts: value.attempts,
                offset: value.offset,
                enumeration_complete: value.next.is_none(),
            },
            basis,
        })
    }
}
