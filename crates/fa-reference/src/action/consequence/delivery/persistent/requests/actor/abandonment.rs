//! Separately granted irreversible disposition of one ORIGINAL unknown request.
//! Abandonment records a retained liability; it does not establish execution or
//! nonexecution, release its charge, seal an endpoint or advance a text stream.
//! Next observes only. Abandon consumes the original full-input owner's durable
//! transition, with separately bounded numerical reconstruction when required.

mod port;
use super::{FileHoldDiagnosisBasis, Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::{ActionState, Scope};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_ABANDONMENT_QUERIES: usize = 1024;
pub const MAX_ABANDONMENT_OPERATIONS: usize = 128;
/// Existing retained events AND the proposed AbandonUnknown must fit this bound.
/// These are event-work units, not numerical products or wall-clock costs.
pub const MAX_ABANDONMENT_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAbandonmentError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}

impl fmt::Display for FileAbandonmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FileAbandonmentError {}

/// Why this finite contract cannot offer a new abandonment. None of these
/// observations grants permission to resend, reconcile or refund an effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAbandonmentBlockReason {
    NotUnknown,
    Terminal,
    IrrecoverablyUnknown,
    NumericalReplayUnsupported,
    NumericalReplayLimit,
    ReplayLimit,
    OperationLimit,
    OperationAlreadyUsed,
}

/// The exact original external request cut and a durable abandonment operation
/// identity. Constructing or copying it supplies no supervisor custody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileAbandonmentRequest {
    operation: u64,
    expected: FileHoldDiagnosisBasis,
}

impl FileAbandonmentRequest {
    pub fn new(
        operation: u64,
        expected: FileHoldDiagnosisBasis,
    ) -> Result<Self, FileAbandonmentError> {
        if operation == 0 || expected.request == 0 || expected.attempt == 0 {
            return Err(FileAbandonmentError::MalformedRequest);
        }
        Ok(Self {
            operation,
            expected,
        })
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn expected(&self) -> FileHoldDiagnosisBasis {
        self.expected
    }
}

/// A plan from the original request map and acknowledged owner. The expected
/// predecessor is rechecked at execution; Next reserves no operation or work.
/// No clock, source, helper, human key or publication permission is acquired.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileAbandonmentPlan,
/// };
/// fn no_approval(plan: FileAbandonmentPlan) -> FilePermit { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAbandonmentPlan {
    basis: FileHoldDiagnosisBasis,
    stage: ActionState,
    operation: u64,
    block_reason: Option<FileAbandonmentBlockReason>,
}

impl FileAbandonmentPlan {
    pub fn basis(&self) -> FileHoldDiagnosisBasis {
        self.basis
    }

    pub fn stage(&self) -> ActionState {
        self.stage
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn can_abandon(&self) -> bool {
        self.block_reason.is_none()
    }

    pub fn block_reason(&self) -> Option<FileAbandonmentBlockReason> {
        self.block_reason
    }

    pub fn request(&self) -> Option<FileAbandonmentRequest> {
        self.can_abandon().then_some(FileAbandonmentRequest {
            operation: self.operation,
            expected: self.basis,
        })
    }
}

/// Acknowledgment of the ORIGINAL Unknown -> IrrecoverablyUnknown transition.
/// Its selected effect's full charge remains a liability. This is not an
/// endpoint receipt or evidence of nonexecution, and does not clear a Stop.
/// Before and after name separate historical native cuts; an exact cached retry
/// preserves them even after later owner progress.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileAbandonmentReceipt;
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// fn no_refund(receipt: FileAbandonmentReceipt) -> EndpointReceipt { receipt }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAbandonmentReceipt {
    before: FileHoldDiagnosisBasis,
    after: FileHoldDiagnosisBasis,
    operation: u64,
    before_stage: ActionState,
    charged_units: u64,
    stage: ActionState,
}

impl FileAbandonmentReceipt {
    pub fn before(&self) -> FileHoldDiagnosisBasis {
        self.before
    }

    pub fn after(&self) -> FileHoldDiagnosisBasis {
        self.after
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn before_stage(&self) -> ActionState {
        self.before_stage
    }

    /// The selected original effect's conservative charge, not a refund or
    /// the owner's whole-ledger total.
    pub fn charged_units(&self) -> u64 {
        self.charged_units
    }

    pub fn stage(&self) -> ActionState {
        self.stage
    }
}

pub type FileAbandonmentPlanObservation = Knowledge<FileAbandonmentPlan, FileHoldDiagnosisBasis>;
pub type FileAbandonmentPlanResult = Result<FileAbandonmentPlanObservation, FileAbandonmentError>;
pub type FileAbandonmentObservation = Knowledge<FileAbandonmentReceipt, FileHoldDiagnosisBasis>;
pub type FileAbandonmentResult = Result<FileAbandonmentObservation, FileAbandonmentError>;

struct Record {
    request: FileAbandonmentRequest,
    // Entered but unacknowledged, including a caught unwind, never reruns.
    result: Option<FileAbandonmentResult>,
}

struct Grant {
    queries: Cell<usize>,
    operations: Cell<usize>,
    max_replay_events: usize,
    remaining_numerical_replay_events: Option<Cell<usize>>,
    records: RefCell<BTreeMap<u64, Record>>,
}

/// Nonclone custody of separate finite read and mutation allowances. Weak
/// clones share counters and original-result cache. A replacement grant or
/// reopened owner cannot recover this cache; native durable operation IDs also
/// cannot be reused as new operations.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorAbandonmentGrant;
/// fn no_copy(grant: FileSupervisorAbandonmentGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for explicitly granted unknown-liability abandonment"]
pub struct FileSupervisorAbandonmentGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorAbandonmentGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.queries.get()
    }

    pub fn remaining_operations(&self) -> usize {
        self.grant.operations.get()
    }

    pub fn max_replay_events(&self) -> usize {
        self.grant.max_replay_events
    }

    /// Original-event reservations, not CPU time, products or model quality.
    pub fn remaining_numerical_replay_events(&self) -> Option<usize> {
        self.grant
            .remaining_numerical_replay_events
            .as_ref()
            .map(Cell::get)
    }

    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorAbandonmentGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorAbandonmentGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("remaining_operations", &self.remaining_operations())
            .field("max_replay_events", &self.max_replay_events())
            .field(
                "remaining_numerical_replay_events",
                &self.remaining_numerical_replay_events(),
            )
            .finish_non_exhaustive()
    }
}

/// Weak access to a separately granted operation on the original owner. It has
/// no source, clock, publication, ordinary intake or mutable host accessor.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) {
///     actor.abandonment_port(1, 1, 64);
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorAbandonmentPort;
/// fn no_owner(port: FileSupervisorAbandonmentPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorAbandonmentPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorAbandonmentPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorAbandonmentPort { .. }")
    }
}

#[cfg(test)]
mod tests;
