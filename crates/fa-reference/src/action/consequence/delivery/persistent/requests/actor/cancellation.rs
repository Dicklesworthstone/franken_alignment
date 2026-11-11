//! Separately granted cancellation of one ORIGINAL undispatched request.
//! This is the native Cancel law, never Hold, Stop, endpoint sealing or a refund
//! of an uncertain effect. Next only observes; Cancel consumes the original
//! full-input owner's transition and preserves its dependent-buffer cleanup.
//! Numerical replay requires its own cumulative original-event reservation.

mod port;
use super::{FileHoldDiagnosisBasis, Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::{ActionState, Scope};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_CANCELLATION_QUERIES: usize = 1024;
pub const MAX_CANCELLATION_OPERATIONS: usize = 128;
/// Existing retained events AND the proposed Cancel event must fit this bound.
/// This is an event-work ceiling, never a numerical-products budget or a cost.
pub const MAX_CANCELLATION_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;

/// Safe refusal classes; native source, helper and storage details are private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCancellationError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}

impl fmt::Display for FileCancellationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FileCancellationError {}

/// Why this finite cancellation contract cannot offer a new operation.
/// This says nothing about other registered operations or publication permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCancellationBlockReason {
    AlreadyDispatched,
    Terminal,
    IrrecoverablyUnknown,
    NumericalReplayUnsupported,
    NumericalReplayLimit,
    ReplayLimit,
    OperationLimit,
    OperationAlreadyUsed,
}

/// Caller-selected operation and the complete observed predecessor. Constructing
/// or copying this request supplies no custody, current clock, receipt or permit.
/// The original owner rechecks every coordinate before entering native work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCancellationRequest {
    operation: u64,
    expected: FileHoldDiagnosisBasis,
}

impl FileCancellationRequest {
    pub fn new(
        operation: u64,
        expected: FileHoldDiagnosisBasis,
    ) -> Result<Self, FileCancellationError> {
        if operation == 0 || expected.request == 0 || expected.attempt == 0 {
            return Err(FileCancellationError::MalformedRequest);
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

/// One bounded plan from the original request map and acknowledged owner.
/// can_cancel means the recorded undispatched stage and this grant's remaining
/// work permit attempting the native cancellation. The full predecessor is
/// rechecked at execution; Next reserves neither operation identity nor work.
/// No clock, source, helper, human key or publication permission is acquired.
/// No cost or information-gain estimate is available, and no VOI rank is claimed.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileCancellationPlan,
/// };
/// fn no_approval(plan: FileCancellationPlan) -> FilePermit { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCancellationPlan {
    basis: FileHoldDiagnosisBasis,
    stage: ActionState,
    operation: u64,
    block_reason: Option<FileCancellationBlockReason>,
}

impl FileCancellationPlan {
    pub fn basis(&self) -> FileHoldDiagnosisBasis {
        self.basis
    }

    pub fn stage(&self) -> ActionState {
        self.stage
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn can_cancel(&self) -> bool {
        self.block_reason.is_none()
    }

    pub fn block_reason(&self) -> Option<FileCancellationBlockReason> {
        self.block_reason
    }

    /// An executable request for this contract, not an approval to publish.
    /// Revalidate it under the separately retained cancellation custodian.
    pub fn request(&self) -> Option<FileCancellationRequest> {
        self.can_cancel().then_some(FileCancellationRequest {
            operation: self.operation,
            expected: self.basis,
        })
    }
}

/// Acknowledgment of the ORIGINAL Cancel transaction. Before and after are
/// separate native cuts; a cached retry keeps those historical coordinates even
/// after later progress. The selected undispatched request becomes Cancelled.
/// This is terminal cancellation, not a temporary Hold or a run-wide Stop.
/// Only native undispatched reservations may be released; charged effects are
/// unchanged. Dependent generated buffers follow the original Cancel reducer.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileCancellationReceipt,
/// };
/// fn no_approval(receipt: FileCancellationReceipt) -> FilePermit { receipt }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCancellationReceipt {
    before: FileHoldDiagnosisBasis,
    after: FileHoldDiagnosisBasis,
    operation: u64,
    before_stage: ActionState,
    refunded_units: u64,
    stage: ActionState,
}

impl FileCancellationReceipt {
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

    pub fn refunded_units(&self) -> u64 {
        self.refunded_units
    }

    pub fn stage(&self) -> ActionState {
        self.stage
    }
}

pub type FileCancellationPlanObservation = Knowledge<FileCancellationPlan, FileHoldDiagnosisBasis>;
pub type FileCancellationPlanResult =
    Result<FileCancellationPlanObservation, FileCancellationError>;
pub type FileCancellationObservation = Knowledge<FileCancellationReceipt, FileHoldDiagnosisBasis>;
pub type FileCancellationResult = Result<FileCancellationObservation, FileCancellationError>;

struct Record {
    request: FileCancellationRequest,
    // None means entered but not acknowledged, including a caught unwind.
    // Retrying such an operation never repeats native work.
    result: Option<FileCancellationResult>,
}

struct Grant {
    queries: Cell<usize>,
    operations: Cell<usize>,
    max_replay_events: usize,
    remaining_numerical_replay_events: Option<Cell<usize>>,
    records: RefCell<BTreeMap<u64, Record>>,
}

/// Nonclone custody of independent finite read and mutation allowances. All
/// weak port clones share the counters and bounded original-result cache.
/// The cache belongs to this live grant only. A replacement grant or reopened
/// owner cannot recover its results; old predecessors are rechecked and refused.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorCancellationGrant;
/// fn no_copy(grant: FileSupervisorCancellationGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for explicitly granted original undispatched-request cancellation"]
pub struct FileSupervisorCancellationGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorCancellationGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.queries.get()
    }

    pub fn remaining_operations(&self) -> usize {
        self.grant.operations.get()
    }

    pub fn max_replay_events(&self) -> usize {
        self.grant.max_replay_events
    }

    /// Remaining reservations shared by every clone of this grant's port.
    /// None means numerical reconstruction was not granted. These units cover
    /// original journal transitions, not products, CPU time or model quality.
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

impl fmt::Debug for FileSupervisorCancellationGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorCancellationGrant")
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

/// Weak access to a separately granted cancellation operation on the original
/// owner. There is no host, source, clock, publication or ordinary intake getter.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) {
///     actor.cancellation_port(1, 1, 64);
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorCancellationPort;
/// fn no_owner(port: FileSupervisorCancellationPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorCancellationPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorCancellationPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorCancellationPort { .. }")
    }
}

#[cfg(test)]
mod tests;
