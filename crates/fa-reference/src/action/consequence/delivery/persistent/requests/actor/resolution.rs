//! Finite original-owner plans and receipt reconciliation for existing requests.
//! Next is a read; Resolve invokes only the native Reconcile transition. Neither
//! can publish, resend, seal nonexecution, abandon uncertainty or mint a key.
//! Numerical histories require an explicit cumulative original-event reservation.
//! Each retained recipe keeps its own numerical limits; event units are not CPU,
//! arithmetic-products, memory or elapsed-time estimates.

mod port;
use super::{FileHoldDiagnosisBasis, Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS, Reconciliation};
use crate::action::{ActionState, Scope};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_RESOLUTION_QUERIES: usize = 1024;
pub const MAX_RESOLUTION_OPERATIONS: usize = 128;
/// Existing retained events AND the proposed Reconcile event must fit this bound.
/// This is an event-work ceiling, never a numerical-products budget or a cost.
pub const MAX_RESOLUTION_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;

/// Safe refusal classes; native source, helper and storage details are private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileResolutionError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}

impl fmt::Display for FileResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FileResolutionError {}

/// Why this finite reconciliation contract cannot offer a new operation.
/// This says nothing about other registered operations or publication permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileResolutionBlockReason {
    NotDispatched,
    Terminal,
    IrrecoverablyUnknown,
    ClockUnavailable,
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
pub struct FileResolutionRequest {
    operation: u64,
    expected: FileHoldDiagnosisBasis,
}

impl FileResolutionRequest {
    pub fn new(
        operation: u64,
        expected: FileHoldDiagnosisBasis,
    ) -> Result<Self, FileResolutionError> {
        if operation == 0 || expected.request == 0 || expected.attempt == 0 {
            return Err(FileResolutionError::MalformedRequest);
        }
        Ok(Self { operation, expected })
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn expected(&self) -> FileHoldDiagnosisBasis {
        self.expected
    }
}

/// One bounded actionable plan from the original request map and acknowledged
/// owner. At most one operation exists in this contract: query the original
/// endpoint and apply its retained receipt through FileOversight::reconcile.
///
/// can_reconcile means the recorded stage, original clock readiness, supported
/// history and this grant's remaining work permit attempting that native call.
/// Native admission, endpoint and storage gates still run at execution. The
/// owner may advance after this read, and the operation number is not reserved.
/// No fresh time is sampled; recorded time is not a freshness guarantee.
/// No cost or information-gain estimate is available, and no VOI rank is claimed.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileResolutionPlan,
/// };
/// fn no_approval(plan: FileResolutionPlan) -> FilePermit { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileResolutionPlan {
    basis: FileHoldDiagnosisBasis,
    stage: ActionState,
    operation: u64,
    block_reason: Option<FileResolutionBlockReason>,
}

impl FileResolutionPlan {
    pub fn basis(&self) -> FileHoldDiagnosisBasis {
        self.basis
    }

    pub fn stage(&self) -> ActionState {
        self.stage
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn can_reconcile(&self) -> bool {
        self.block_reason.is_none()
    }

    pub fn block_reason(&self) -> Option<FileResolutionBlockReason> {
        self.block_reason
    }

    /// An executable request for this contract, not an approval to publish.
    /// Revalidate it under the separately retained resolution custodian.
    pub fn request(&self) -> Option<FileResolutionRequest> {
        self.can_reconcile().then_some(FileResolutionRequest {
            operation: self.operation,
            expected: self.basis,
        })
    }
}

/// Acknowledgment of the original receipt-reconciliation transaction. Before
/// and after are separate native cuts; a cached retry retains these historical
/// coordinates even if the live owner later advances. An AwaitingResolution or
/// RetentionExpired result is still Unknown, charged, and never publication retry authority.
///
/// An original NotExecuted receipt may release its original charge exactly once.
/// This projection cannot create that receipt or stand in for an endpoint key.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileResolutionReceipt,
/// };
/// fn no_approval(receipt: FileResolutionReceipt) -> FilePermit { receipt }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileResolutionReceipt {
    before: FileHoldDiagnosisBasis,
    after: FileHoldDiagnosisBasis,
    operation: u64,
    outcome: Reconciliation,
    stage: ActionState,
}

impl FileResolutionReceipt {
    pub fn before(&self) -> FileHoldDiagnosisBasis {
        self.before
    }

    pub fn after(&self) -> FileHoldDiagnosisBasis {
        self.after
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn outcome(&self) -> Reconciliation {
        self.outcome
    }

    pub fn stage(&self) -> ActionState {
        self.stage
    }
}

pub type FileResolutionPlanObservation = Knowledge<FileResolutionPlan, FileHoldDiagnosisBasis>;
pub type FileResolutionPlanResult = Result<FileResolutionPlanObservation, FileResolutionError>;
pub type FileResolutionObservation = Knowledge<FileResolutionReceipt, FileHoldDiagnosisBasis>;
pub type FileResolutionResult = Result<FileResolutionObservation, FileResolutionError>;

struct Record {
    request: FileResolutionRequest,
    // None means entered but not acknowledged, including a caught unwind.
    // Retrying such an operation never repeats native work.
    result: Option<FileResolutionResult>,
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
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorResolutionGrant;
/// fn no_copy(grant: FileSupervisorResolutionGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for explicitly granted original receipt reconciliation"]
pub struct FileSupervisorResolutionGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorResolutionGrant {
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
        self.grant.remaining_numerical_replay_events.as_ref().map(Cell::get)
    }

    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorResolutionGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorResolutionGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("remaining_operations", &self.remaining_operations())
            .field("max_replay_events", &self.max_replay_events())
            .field("remaining_numerical_replay_events", &self.remaining_numerical_replay_events())
            .finish_non_exhaustive()
    }
}

/// Weak access to a separately granted reconciliation operation on the original
/// owner. There is no host, source, clock, publication or ordinary intake getter.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) {
///     actor.resolution_port(1, 1, 64);
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorResolutionPort;
/// fn no_owner(port: FileSupervisorResolutionPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorResolutionPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorResolutionPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorResolutionPort { .. }")
    }
}

#[cfg(test)]
mod tests;
