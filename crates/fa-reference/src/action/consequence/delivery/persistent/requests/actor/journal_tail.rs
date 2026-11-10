//! Separately custodied, bounded metadata pages from the original live journal.
//! Source bytes, numerical state, helper secrets and approval keys never enter
//! this grant. A retained page is historical evidence, not a fresh observation.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, JournalError, Knowledge, OwnerRef,
    UnknownReason,
};
use crate::Error;
pub use crate::action::consequence::delivery::persistent::observed::replay::tail::{
    FileJournalEventKind, FileJournalTailEvent,
};
use crate::action::consequence::delivery::persistent::{
    MAX_JOURNAL_EVENTS,
    observed::{
        FileOversight,
        replay::{situation::FileSituationBasis, tail::FileJournalTailCapture},
    },
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_JOURNAL_TAIL_QUERIES: usize = 1024;
pub const MAX_JOURNAL_TAIL_PAGE_EVENTS: usize = 64;
/// The original journal ceiling, checked before any capture allocation. No
/// truncation, filtering or extra retained history can enlarge this contract.
pub const MAX_JOURNAL_TAIL_CAPTURE_EVENTS: usize = MAX_JOURNAL_EVENTS;

static NEXT_CURSOR_ID: AtomicU64 = AtomicU64::new(1);

fn next_cursor_id() -> Option<u64> {
    NEXT_CURSOR_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| {
            if id == 0 { None } else { id.checked_add(1) }
        })
        .ok()
}

/// One opaque, single-use continuation of an exact native capture. The scalar
/// ID supports transport correlation only; it cannot reconstruct a cursor or
/// retain the owner, grant or captured data after their custodians are gone.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileJournalTailCursor;
/// fn forge() { let _ = FileJournalTailCursor::new(1); }
/// ```
#[derive(Clone)]
pub struct FileJournalTailCursor {
    id: u64,
    grant: Weak<Grant>,
    page: Weak<()>,
}

impl FileJournalTailCursor {
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl fmt::Debug for FileJournalTailCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileJournalTailCursor { .. }")
    }
}

impl PartialEq for FileJournalTailCursor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && Weak::ptr_eq(&self.grant, &other.grant)
            && Weak::ptr_eq(&self.page, &other.page)
    }
}
impl Eq for FileJournalTailCursor {}

/// One sealed metadata page. Coverage is exactly the original half-open suffix
/// `(after_revision, basis.journal_revision]`, never a later appended suffix.
/// `offset` is within that capture, while `through_revision` is the last row
/// returned on this page (or the selected start for a complete empty page).
/// A complete page says only that every metadata row through that cut was read;
/// it does not make the withheld raw journal replayable or prove external effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileJournalTailPage {
    basis: FileSituationBasis,
    after_revision: u64,
    offset: usize,
    through_revision: u64,
    events_per_page: usize,
    events: Vec<FileJournalTailEvent>,
    enumeration_complete: bool,
    continuation: Option<FileJournalTailCursor>,
}

impl FileJournalTailPage {
    pub fn basis(&self) -> FileSituationBasis {
        self.basis
    }
    pub fn after_revision(&self) -> u64 {
        self.after_revision
    }
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn through_revision(&self) -> u64 {
        self.through_revision
    }
    pub fn events_per_page(&self) -> usize {
        self.events_per_page
    }
    pub fn events(&self) -> &[FileJournalTailEvent] {
        &self.events
    }
    pub fn enumeration_complete(&self) -> bool {
        self.enumeration_complete
    }
    /// None also covers exhausted custody; consult enumeration_complete rather
    /// than interpreting a missing cursor as an observed end of history.
    pub fn continuation(&self) -> Option<&FileJournalTailCursor> {
        self.continuation.as_ref()
    }
}

pub type FileJournalTailObservation = Knowledge<FileJournalTailPage, FileSituationBasis>;
pub type FileJournalTailResult = Result<FileJournalTailObservation, ActorError>;

struct Capture {
    data: FileJournalTailCapture,
    next: usize,
    cursor_id: u64,
    page: Rc<()>,
}

struct Grant {
    remaining: Cell<usize>,
    events_per_page: usize,
    capture: RefCell<Option<Capture>>,
}

/// Noncloneable custody of one finite shared query allowance and at most one
/// redacted capture. Revocation also withdraws every existing continuation.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorJournalTailGrant;
/// fn no_copy(grant: FileSupervisorJournalTailGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for the explicitly granted journal metadata disclosure"]
pub struct FileSupervisorJournalTailGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorJournalTailGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }
    pub fn events_per_page(&self) -> usize {
        self.grant.events_per_page
    }
    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorJournalTailGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorJournalTailGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("events_per_page", &self.events_per_page())
            .finish_non_exhaustive()
    }
}

/// Weak read access to the exact original native incarnation. No mutable host,
/// raw journal exporter, effect capability or disclosure constructor is exposed.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) { actor.journal_tail_port(1, 16); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorJournalTailPort;
/// fn no_authority(port: FileSupervisorJournalTailPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorJournalTailPort {
    owner: Weak<OwnerRef<FileOversight>>,
    issuer: Weak<()>,
    grant: Weak<Grant>,
}

impl fmt::Debug for FileSupervisorJournalTailPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorJournalTailPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Provisioning performs no capture, replay, source/clock read, I/O or
    /// admission snapshot consumption. The host fixes both work bounds once;
    /// actor bytes, reconnects and port clones cannot increase them.
    pub fn journal_tail_port(
        &self,
        queries: usize,
        events_per_page: usize,
    ) -> Result<
        (
            FileSupervisorJournalTailPort,
            FileSupervisorJournalTailGrant,
        ),
        JournalError,
    > {
        if queries == 0 || events_per_page == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_JOURNAL_TAIL_QUERIES || events_per_page > MAX_JOURNAL_TAIL_PAGE_EVENTS {
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
            events_per_page,
            capture: RefCell::new(None),
        });
        Ok((
            FileSupervisorJournalTailPort {
                owner: Rc::downgrade(&self.owner),
                issuer: owner.host.journal_tail_identity(),
                grant: Rc::downgrade(&grant),
            },
            FileSupervisorJournalTailGrant { grant },
        ))
    }
}

impl FileSupervisorJournalTailPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner)
    }

    /// Capture every acknowledged metadata row after this original journal
    /// revision, then return its first finite page. Starting at the exact end
    /// returns an observed empty suffix; starting beyond it is charged Unknown.
    /// A fresh capture always retires the previous continuation, even on failure.
    pub fn tail(&self, after_revision: u64) -> FileJournalTailResult {
        self.read(Some(after_revision), None)
    }

    /// Consume one next page of the original capture, without recapturing a
    /// newer journal or trusting a caller-selected offset. Foreign, replaced or
    /// consumed cursors refuse before work is charged. Accepted pages consume
    /// one query before fallible work and cannot be retried after output loss.
    pub fn continue_tail(&self, cursor: &FileJournalTailCursor) -> FileJournalTailResult {
        self.read(None, Some(cursor))
    }

    fn read(
        &self,
        after_revision: Option<u64>,
        cursor: Option<&FileJournalTailCursor>,
    ) -> FileJournalTailResult {
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_journal_tail",
            });
        };
        let mut capture = grant
            .capture
            .try_borrow_mut()
            .map_err(|_| ActorError::Unavailable)?;
        let Some(owner) = self.owner.upgrade() else {
            *capture = None;
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            *capture = None;
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some()
            || !Weak::ptr_eq(&self.issuer, &owner.host.journal_tail_identity())
        {
            *capture = None;
            return Ok(unknown());
        }
        if let Some(cursor) = cursor {
            if !Weak::ptr_eq(&cursor.grant, &self.grant)
                || !capture.as_ref().is_some_and(|stored| {
                    stored.cursor_id == cursor.id
                        && Weak::ptr_eq(&cursor.page, &Rc::downgrade(&stored.page))
                })
            {
                return Err(ActorError::IdempotencyConflict);
            }
        }
        let remaining = grant.remaining.get();
        if remaining == 0 {
            *capture = None;
            return Err(ActorError::Capacity);
        }
        // The former identity and complete charge are consumed before any work
        // that can fail or unwind. Neither is restored by failed output later.
        grant.remaining.set(remaining - 1);
        let previous = capture.take();
        let (data, offset) = match after_revision {
            Some(after) => {
                drop(previous);
                match owner.host.capture_journal_tail(after) {
                    Ok(Some(data)) => (data, 0),
                    Ok(None) => return Ok(unknown()),
                    Err(JournalError::Contract(Error::Limit | Error::Overflow)) => {
                        return Err(ActorError::Capacity);
                    }
                    Err(_) => return Err(ActorError::Unavailable),
                }
            }
            None => {
                let previous = previous.ok_or(ActorError::IdempotencyConflict)?;
                (previous.data, previous.next)
            }
        };
        let end = offset
            .checked_add(grant.events_per_page)
            .ok_or(ActorError::Capacity)?
            .min(data.events.len());
        if offset > end || data.events.len() > MAX_JOURNAL_TAIL_CAPTURE_EVENTS {
            return Err(ActorError::Unavailable);
        }
        let mut events = Vec::new();
        events
            .try_reserve_exact(end - offset)
            .map_err(|_| ActorError::Capacity)?;
        events.extend_from_slice(&data.events[offset..end]);
        let basis = data.basis;
        let after_revision = data.after_revision;
        let through_revision = after_revision
            .checked_add(end as u64)
            .ok_or(ActorError::Capacity)?;
        let enumeration_complete = end == data.events.len();
        let continuation = if !enumeration_complete && grant.remaining.get() > 0 {
            let id = next_cursor_id().ok_or(ActorError::Capacity)?;
            let page = Rc::new(());
            let cursor = FileJournalTailCursor {
                id,
                grant: Rc::downgrade(&grant),
                page: Rc::downgrade(&page),
            };
            *capture = Some(Capture {
                data,
                next: end,
                cursor_id: id,
                page,
            });
            Some(cursor)
        } else {
            None
        };
        Ok(Knowledge::Known {
            value: FileJournalTailPage {
                basis,
                after_revision,
                offset,
                through_revision,
                events_per_page: grant.events_per_page,
                events,
                enumeration_complete,
                continuation,
            },
            basis,
        })
    }
}

fn unknown() -> FileJournalTailObservation {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}

#[cfg(test)]
mod tests;
