//! Finite delegation of one original joint-qualified helper-weight promotion.
//!
//! The original native ledger chooses deterministic weights from all retained
//! independent labels. This grant cannot supply weights, change the roster,
//! install a joint policy, label a case, approve an effect or replenish work.
//! Only the separately held supervisor can provision it on an existing owner.

mod port;

use super::{Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::oversight::joint_credibility::JointPromotionPolicy;
use crate::action::{ElapsedTick, Purpose, Scope};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_CREDIBILITY_PROMOTION_QUERIES: usize = 1024;
/// Complete acknowledged history plus the proposed native Promote event.
pub const MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCredibilityPromotionError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}

impl fmt::Display for FileCredibilityPromotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for FileCredibilityPromotionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileCredibilityPromotionBlockReason {
    Stopped,
    Suspended,
    ClockUnavailable,
    SourceUnavailable,
    MarginalUnqualified,
    JointUnqualified,
    NoWeightChange,
    EvaluationWorkLimit,
    NumericalReplayUnsupported,
    NumericalReplayLimit,
    ReplayLimit,
    OperationLimit,
    OperationAlreadyUsed,
}

/// One complete acknowledged original-owner predecessor. Evaluation, journal,
/// control and authority coordinates remain distinct. The recorded clock is
/// historical; neither reading this basis nor submitting it observes fresh time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCredibilityPromotionBasis {
    pub scope: Scope,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub evaluation_revision: u64,
    pub congress_generation: u64,
    pub policy_generation: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// Data selecting one native operation, never a positive report or approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCredibilityPromotionRequest {
    operation: u64,
    expected: FileCredibilityPromotionBasis,
}

impl FileCredibilityPromotionRequest {
    pub fn new(
        operation: u64,
        expected: FileCredibilityPromotionBasis,
    ) -> Result<Self, FileCredibilityPromotionError> {
        if operation == 0
            || expected.scope.purpose != Purpose::Effect
            || [
                expected.scope.tenant,
                expected.scope.principal,
                expected.scope.run,
                expected.scope.branch,
                expected.scope.authority,
                expected.congress_generation,
                expected.policy_generation,
            ]
            .contains(&0)
        {
            return Err(FileCredibilityPromotionError::MalformedRequest);
        }
        Ok(Self {
            operation,
            expected,
        })
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn expected(&self) -> FileCredibilityPromotionBasis {
        self.expected
    }
}

/// The original native marginal and joint predicates at one acknowledged cut.
/// A plan reserves no label, candidate, operation or predecessor. Eligibility is
/// rechecked by the owning native promotion; no raw helper/assessment is exposed.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileCredibilityPromotionPlan,
/// };
/// fn no_effect(plan: FileCredibilityPromotionPlan) -> FilePermit { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredibilityPromotionPlan {
    basis: FileCredibilityPromotionBasis,
    operation: u64,
    block_reason: Option<FileCredibilityPromotionBlockReason>,
}

impl FileCredibilityPromotionPlan {
    pub fn basis(&self) -> FileCredibilityPromotionBasis {
        self.basis
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn can_promote(&self) -> bool {
        self.block_reason.is_none()
    }

    pub fn block_reason(&self) -> Option<FileCredibilityPromotionBlockReason> {
        self.block_reason
    }

    pub fn request(&self) -> Option<FileCredibilityPromotionRequest> {
        self.can_promote().then_some(FileCredibilityPromotionRequest {
            operation: self.operation,
            expected: self.basis,
        })
    }
}

/// Original acknowledged weight transition. Only unused reservations may be
/// cancelled/refunded; admitted and Unknown effect charges survive. A cached
/// receipt describes its historical transition, not today's congress.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileCredibilityPromotionReceipt,
/// };
/// fn no_effect(receipt: FileCredibilityPromotionReceipt) -> FilePermit { receipt }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredibilityPromotionReceipt {
    before: FileCredibilityPromotionBasis,
    after: FileCredibilityPromotionBasis,
    operation: u64,
    cancelled: Vec<u64>,
    refunded_units: u64,
}

impl FileCredibilityPromotionReceipt {
    pub fn before(&self) -> FileCredibilityPromotionBasis {
        self.before
    }

    pub fn after(&self) -> FileCredibilityPromotionBasis {
        self.after
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn cancelled(&self) -> &[u64] {
        &self.cancelled
    }

    pub fn refunded_units(&self) -> u64 {
        self.refunded_units
    }
}

pub type FileCredibilityPromotionPlanObservation =
    Knowledge<FileCredibilityPromotionPlan, FileCredibilityPromotionBasis>;
pub type FileCredibilityPromotionPlanResult =
    Result<FileCredibilityPromotionPlanObservation, FileCredibilityPromotionError>;
pub type FileCredibilityPromotionObservation =
    Knowledge<FileCredibilityPromotionReceipt, FileCredibilityPromotionBasis>;
pub type FileCredibilityPromotionResult =
    Result<FileCredibilityPromotionObservation, FileCredibilityPromotionError>;

struct Record {
    request: FileCredibilityPromotionRequest,
    // None records entered work without a returned result, including unwind.
    // It can never be mistaken for an operation that may safely start again.
    result: Option<FileCredibilityPromotionResult>,
}

struct Grant {
    joint_policy: JointPromotionPolicy,
    queries: Cell<usize>,
    operations: Cell<usize>,
    max_replay_events: usize,
    remaining_numerical_replay_events: Option<Cell<usize>>,
    record: RefCell<Option<Record>>,
}

/// Noncloneable custody of one original joint-policy selection and finite work.
/// Dropping custody revokes all weak clones, including cached-result disclosure.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorCredibilityPromotionGrant;
/// fn no_copy(grant: FileSupervisorCredibilityPromotionGrant) { grant.clone(); }
/// ```
#[must_use = "retain independent custody of the finite helper-weight promotion grant"]
pub struct FileSupervisorCredibilityPromotionGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorCredibilityPromotionGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.queries.get()
    }

    pub fn remaining_operations(&self) -> usize {
        self.grant.operations.get()
    }

    pub fn max_replay_events(&self) -> usize {
        self.grant.max_replay_events
    }

    /// None means numerical history reconstruction was not granted. Units are
    /// complete-history event reservations, not CPU time or scalar products.
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

impl fmt::Debug for FileSupervisorCredibilityPromotionGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorCredibilityPromotionGrant")
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

/// Supervisor-only weak access to the ORIGINAL owner. This role cannot provide
/// labels, change the native candidate rule, obtain its owner or mint an effect.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) {
///     actor.credibility_promotion_port(1, 64);
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorCredibilityPromotionPort;
/// fn no_owner(port: FileSupervisorCredibilityPromotionPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorCredibilityPromotionPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
    issuer: Weak<()>,
}

impl FileSupervisorCredibilityPromotionPort {
    pub(crate) fn capability_snapshot(&self) -> super::FileOperatorCapabilityObservation {
        super::capabilities::observe(
            &self.owner,
            &self.grant,
            self.scope,
            "supervisor_credibility_promotion",
            |grant| {
                let budget = super::FileOperatorCapabilityBudget::new(&[
                    ("remaining_queries", grant.queries.get()),
                    ("remaining_operations", grant.operations.get()),
                    ("replay_event_limit", grant.max_replay_events),
                ])
                .with(
                    "remaining_numerical_replay_events",
                    grant
                        .remaining_numerical_replay_events
                        .as_ref()
                        .map(Cell::get),
                );
                Some((self.issuer.clone(), budget))
            },
        )
    }
}

impl fmt::Debug for FileSupervisorCredibilityPromotionPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorCredibilityPromotionPort { .. }")
    }
}
