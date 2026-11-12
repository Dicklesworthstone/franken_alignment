//! Explicit, finite requests to reopen an ordinary held congress. Queueing is
//! not a review, an approval, a new observation or an execution permit.
mod port;

use super::{FileActorSupervisor, Knowledge, OwnerRef};
use crate::Error;
use crate::action::consequence::delivery::persistent::{JournalError, observed::FileOversight};
use crate::action::{ElapsedTick, Scope};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_CONVENE_QUERIES: usize = 1024;
pub const MAX_CONVENE_OPERATIONS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileConveneError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}
impl fmt::Display for FileConveneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FileConveneError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileConveneBlockReason {
    NotHeld,
    Stopped,
    Suspended,
    UnsupportedHistory,
    ActiveReview,
    StaleReview,
    RoundAlreadyUsed,
    RoundLimit,
    OperationLimit,
    PendingOperation,
    DeadlineElapsed,
}

/// One exact acknowledged original-owner cut. The clock is historical and the
/// deadline belongs to the original action; neither is a fresh lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileConveneBasis {
    pub scope: Scope,
    pub request: u64,
    pub request_generation: u64,
    pub attempt: u64,
    pub held_round: u64,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub input_revision: u64,
    pub recorded_at: Option<ElapsedTick>,
    pub deadline: ElapsedTick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileConveneRequest {
    request: u64,
    round: u64,
    expected: FileConveneBasis,
}
impl FileConveneRequest {
    pub fn new(
        request: u64,
        round: u64,
        expected: FileConveneBasis,
    ) -> Result<Self, FileConveneError> {
        if request == 0
            || round == 0
            || expected.request != request
            || expected.attempt == 0
            || expected.held_round == 0
            || expected.control_sequence == 0
            || expected.journal_revision == 0
        {
            return Err(FileConveneError::MalformedRequest);
        }
        Ok(Self {
            request,
            round,
            expected,
        })
    }
    pub fn request(&self) -> u64 {
        self.request
    }
    pub fn round(&self) -> u64 {
        self.round
    }
    pub fn expected(&self) -> FileConveneBasis {
        self.expected
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileConvenePlan {
    basis: FileConveneBasis,
    round: u64,
    block_reason: Option<FileConveneBlockReason>,
}
impl FileConvenePlan {
    pub fn basis(&self) -> FileConveneBasis {
        self.basis
    }
    pub fn request(&self) -> u64 {
        self.basis.request
    }
    pub fn round(&self) -> u64 {
        self.round
    }
    pub fn block_reason(&self) -> Option<FileConveneBlockReason> {
        self.block_reason
    }
    pub fn can_convene(&self) -> bool {
        self.block_reason.is_none()
    }
    pub fn convene_request(&self) -> Option<FileConveneRequest> {
        self.can_convene().then(|| FileConveneRequest {
            request: self.basis.request,
            round: self.round,
            expected: self.basis,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileConveneOutcome {
    /// Accepted by the original finite grant; no source or helper work yet.
    Pending,
    /// The original durable Begin and the selected helper launch succeeded.
    Opened,
    /// The one attempted handoff failed. It may have committed Begin or retained
    /// children for cleanup; it never refunds this operation or authorizes retry.
    Failed,
    /// The selected one-use handoff was dropped without completing its launch.
    Abandoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileConveneOpened {
    pub(crate) journal_revision: u64,
    pub(crate) input_revision: u64,
    pub(crate) started_at: ElapsedTick,
}
impl FileConveneOpened {
    pub fn journal_revision(&self) -> u64 {
        self.journal_revision
    }
    pub fn input_revision(&self) -> u64 {
        self.input_revision
    }
    pub fn started_at(&self) -> ElapsedTick {
        self.started_at
    }
}

/// Historical disposition of one supervisor request, never a congress decision.
/// Its basis stays the held predecessor, even after the original owner advances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileConveneReceipt {
    request: FileConveneRequest,
    outcome: FileConveneOutcome,
    opened: Option<FileConveneOpened>,
}
impl FileConveneReceipt {
    pub fn request(&self) -> &FileConveneRequest {
        &self.request
    }
    pub fn outcome(&self) -> FileConveneOutcome {
        self.outcome
    }
    pub fn opened(&self) -> Option<&FileConveneOpened> {
        self.opened.as_ref()
    }
}
pub type FileConvenePlanObservation = Knowledge<FileConvenePlan, FileConveneBasis>;
pub type FileConvenePlanResult = Result<FileConvenePlanObservation, FileConveneError>;
pub type FileConveneObservation = Knowledge<FileConveneReceipt, FileConveneBasis>;
pub type FileConveneResult = Result<FileConveneObservation, FileConveneError>;

struct Record {
    receipt: FileConveneReceipt,
    taken: bool,
}
struct Grant {
    queries: Cell<usize>,
    operations: Cell<usize>,
    records: RefCell<BTreeMap<u64, Record>>,
    scope: Scope,
    issuer: Weak<()>,
}
pub(super) struct Selection {
    grant: Weak<Grant>,
    // Reserved round IDs survive revocation without retaining authority or its
    // receipt cache. At most the original 128 operation slots can enter here.
    rounds: BTreeSet<u64>,
}
impl Grant {
    fn healthy(&self, host: &FileOversight) -> bool {
        host.storage_failure().is_none()
            && host.actor_explanation_scope() == self.scope
            && Weak::ptr_eq(&self.issuer, &host.journal_tail_identity())
    }
}

/// Nonclone original custody. Dropping it withdraws every weak port and pending
/// ticket. A reconnect, facade or later request cannot refill its allowance.
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorConveneGrant;
/// fn duplicate(grant: FileSupervisorConveneGrant) { grant.clone(); }
/// ```
#[must_use = "retain the original finite convening grant"]
pub struct FileSupervisorConveneGrant {
    grant: Rc<Grant>,
}
impl FileSupervisorConveneGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.queries.get()
    }
    pub fn remaining_operations(&self) -> usize {
        self.grant.operations.get()
    }
    pub fn revoke(self) {
        drop(self);
    }
}
impl fmt::Debug for FileSupervisorConveneGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorConveneGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("remaining_operations", &self.remaining_operations())
            .finish_non_exhaustive()
    }
}

/// Independently granted admission only. It has no source, helper, clock, host,
/// actor submission or ability to lift the held decision.
#[derive(Clone)]
pub struct FileSupervisorConvenePort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
}
impl fmt::Debug for FileSupervisorConvenePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorConvenePort { .. }")
    }
}

/// A sealed, one-use handoff to the ORIGINAL file driver. This is not clonable,
/// serializable or reconstructible from a Pending receipt or public basis.
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneTicket;
/// fn duplicate(ticket: FileConveneTicket) { ticket.clone(); }
/// ```
#[must_use = "consume the one-use held-review handoff or explicitly abandon it"]
pub struct FileConveneTicket {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    input: FileConveneRequest,
    completed: bool,
}
impl fmt::Debug for FileConveneTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileConveneTicket")
            .field("request", &self.request())
            .field("round", &self.round())
            .finish_non_exhaustive()
    }
}
impl FileConveneTicket {
    pub fn request(&self) -> u64 {
        self.input.request
    }
    pub fn round(&self) -> u64 {
        self.input.round
    }
    pub fn expected(&self) -> FileConveneBasis {
        self.input.expected
    }

    pub(crate) fn belongs_to(&self, supervisor: &FileActorSupervisor<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &Rc::downgrade(&supervisor.owner))
    }

    pub(crate) fn check(&self, host: &FileOversight) -> Result<(), JournalError> {
        let grant = self.grant.upgrade().ok_or(JournalError::Unavailable)?;
        if self.completed || !grant.healthy(host) || self.owner.upgrade().is_none() {
            return Err(JournalError::Unavailable);
        }
        let records = grant
            .records
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let record = records.get(&self.round()).ok_or(Error::Missing)?;
        if record.receipt.request != self.input
            || !record.taken
            || record.receipt.outcome != FileConveneOutcome::Pending
        {
            return Err(Error::Binding.into());
        }
        let (basis, blocked) = host.convene_status(self.request())?;
        if basis != self.expected() {
            return Err(Error::Stale.into());
        }
        if blocked.is_some() || host.convene_round_used(self.round()) {
            return Err(Error::WrongState.into());
        }
        Ok(())
    }

    pub(crate) fn finish(&mut self, opened: Option<FileConveneOpened>) {
        self.record(
            if opened.is_some() {
                FileConveneOutcome::Opened
            } else {
                FileConveneOutcome::Failed
            },
            opened,
        );
    }

    fn record(&mut self, outcome: FileConveneOutcome, opened: Option<FileConveneOpened>) {
        if self.completed {
            return;
        }
        self.completed = true;
        let Some(grant) = self.grant.upgrade() else {
            return;
        };
        let Ok(mut records) = grant.records.try_borrow_mut() else {
            return;
        };
        let Some(record) = records.get_mut(&self.round()) else {
            return;
        };
        if record.taken
            && record.receipt.request == self.input
            && record.receipt.outcome == FileConveneOutcome::Pending
        {
            record.receipt.outcome = outcome;
            record.receipt.opened = opened;
        }
    }
}
impl Drop for FileConveneTicket {
    fn drop(&mut self) {
        self.record(FileConveneOutcome::Abandoned, None);
    }
}
