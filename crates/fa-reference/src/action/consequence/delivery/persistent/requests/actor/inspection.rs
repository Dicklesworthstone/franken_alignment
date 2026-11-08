//! Separately granted supervisor inspection of the original acknowledged owner.
//! The actor and exact-precheck ports cannot provision this capability.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, JournalError, Knowledge, OwnerRef,
    UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::{
    FileOversight,
    replay::situation::{
        FileSituation, FileSituationAttempt, FileSituationBasis, FileSituationPageToken,
        FileSituationSummary,
    },
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_INSPECTION_QUERIES: usize = 1024;
/// Each page has the same fixed bound. A continuation always reads its original
/// immutable capture; an offset is never imported into a fresh live report.
pub const MAX_INSPECTION_ATTEMPTS: usize = 16;

// IDs distinguish pending pages even across separate grants and Tower mounts.
// Checked advancement refuses exhaustion rather than rebinding an earlier ID.
static NEXT_CURSOR_ID: AtomicU64 = AtomicU64::new(1);

fn next_cursor_id(counter: &AtomicU64) -> Option<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| {
            if id == 0 { None } else { id.checked_add(1) }
        })
        .ok()
}

/// Opaque continuation data for one page of one original inspection grant.
/// Cloning it does not copy the capture, extend owner lifetime or grant reads.
/// Every continuation still checks the original owner and revocable allowance.
/// Successful use consumes this page identity; a retry must use the returned
/// next cursor or explicitly capture a new situation.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileInspectionCursor;
/// fn forge() { let _ = FileInspectionCursor::new(1); }
/// ```
#[derive(Clone)]
pub struct FileInspectionCursor {
    id: u64,
    grant: Weak<Grant>,
    page: Weak<()>,
}

impl FileInspectionCursor {
    /// An opaque, nonzero process-wide page identifier for transport matching.
    /// It is ephemeral within this process/service lifetime, never authority or
    /// a persisted resume credential. Only the original native cursor can read
    /// its capture; an identifier cannot reconstruct one.
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl fmt::Debug for FileInspectionCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileInspectionCursor { .. }")
    }
}

impl PartialEq for FileInspectionCursor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && Weak::ptr_eq(&self.grant, &other.grant)
            && Weak::ptr_eq(&self.page, &other.page)
    }
}

impl Eq for FileInspectionCursor {}

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
    continuation: Option<FileInspectionCursor>,
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
    /// The only next page accepted by this grant. None also covers an exhausted
    /// query allowance; use `enumeration_complete` to distinguish truncation.
    pub fn continuation(&self) -> Option<&FileInspectionCursor> {
        self.continuation.as_ref()
    }
}

pub type FileInspectionObservation = Knowledge<FileInspectionPage, FileSituationBasis>;
pub type FileInspectionResult = Result<FileInspectionObservation, ActorError>;

struct Grant {
    remaining: Cell<usize>,
    capture: RefCell<Option<Capture>>,
}

/// One retained native capture per grant, shared by every port clone. The
/// original branded token stays next to its capture; callers never select an
/// offset or substitute an equal-revision report. No authoritative owner lives
/// here, and caller-held cursors have only weak references to these brands.
struct Capture {
    situation: FileSituation,
    next: FileSituationPageToken,
    page: Rc<()>,
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
            capture: RefCell::new(None),
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
    /// page is claimed complete. Capturing again invalidates the previous
    /// continuation, including through another clone. An output failure cannot
    /// refund this entered query.
    pub fn situation(&self) -> FileInspectionResult {
        self.situation_page(None)
    }

    /// Continue one original immutable situation or capture a new first page.
    /// A retained continuation keeps its historical summary, ordering and basis
    /// even after a healthy owner advances. It never recaptures and skips rows
    /// by offset. The original owner must still be healthy and available: loss,
    /// a conflicting borrow or a storage fault invalidates the retained capture.
    ///
    /// Every accepted capture/page spends once before fallible native work.
    /// Foreign, replaced and consumed cursors are refused before spending and
    /// cannot displace a valid continuation. All clones share both allowance
    /// and the single retained capture; no page can renew either. The final
    /// allowed page drops its retained capture and has no continuation, even
    /// when its explicit enumeration-complete flag remains false.
    pub fn situation_page(
        &self,
        continuation: Option<&FileInspectionCursor>,
    ) -> FileInspectionResult {
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_inspection",
            });
        };
        let mut capture = grant
            .capture
            .try_borrow_mut()
            .map_err(|_| ActorError::Unavailable)?;
        let unknown = || Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        };
        let Some(owner) = self.owner.upgrade() else {
            *capture = None;
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            *capture = None;
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some() {
            *capture = None;
            return Ok(unknown());
        }
        let remaining = grant.remaining.get();
        if remaining == 0 {
            *capture = None;
            return Err(ActorError::Capacity);
        }
        if let Some(cursor) = continuation {
            let matches = Weak::ptr_eq(&cursor.grant, &self.grant)
                && capture.as_ref().is_some_and(|current| {
                    Weak::ptr_eq(&cursor.page, &Rc::downgrade(&current.page))
                });
            if !matches {
                return Err(ActorError::IdempotencyConflict);
            }
        } else {
            // Withdraw the former capture before any new query work. A refused
            // or interrupted replacement must not revive an earlier cursor.
            *capture = None;
        }
        // Charge before the original fallible capture; a caught failure or
        // unwind does not create another allowance or reuse an earlier report.
        grant.remaining.set(remaining - 1);
        // Take before entering page construction: a failure or caught unwind
        // consumes the old page identity, never leaving it eligible to retry.
        let (situation, token) = match capture.take() {
            Some(previous) => (previous.situation, Some(previous.next)),
            None => (
                owner
                    .host
                    .situation()
                    .map_err(|_| ActorError::Unavailable)?,
                None,
            ),
        };
        let observation = situation
            .page(token.as_ref(), MAX_INSPECTION_ATTEMPTS)
            .map_err(|_| ActorError::Unavailable)?;
        let Knowledge::Known { value, basis } = observation else {
            return Err(ActorError::Unavailable);
        };
        let enumeration_complete = value.next.is_none();
        let continuation = match value.next {
            Some(next) if grant.remaining.get() > 0 => {
                let id = next_cursor_id(&NEXT_CURSOR_ID).ok_or(ActorError::Capacity)?;
                let page = Rc::new(());
                let cursor = FileInspectionCursor {
                    id,
                    grant: Rc::downgrade(&grant),
                    page: Rc::downgrade(&page),
                };
                *capture = Some(Capture {
                    situation,
                    next,
                    page,
                });
                Some(cursor)
            }
            // An exhausted allowance retains no capture or unusable cursor.
            // Incomplete enumeration remains explicit in the returned page.
            _ => None,
        };
        Ok(Knowledge::Known {
            value: FileInspectionPage {
                basis,
                summary: value.summary,
                attempts: value.attempts,
                offset: value.offset,
                enumeration_complete,
                continuation,
            },
            basis,
        })
    }
}

#[cfg(test)]
mod tests;
