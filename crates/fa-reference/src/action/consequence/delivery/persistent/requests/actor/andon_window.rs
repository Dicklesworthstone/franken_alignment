//! Finite delegation of one independently selected native Andon window.
//!
//! The supervisor fixes the complete immutable budget before handing out weak
//! access. A request asserts the operation, exact original-owner predecessor
//! and that same budget; it cannot change limits, submit labels, alter helpers, obtain
//! effect rights, refresh time or replace the accounting journal.

mod port;

use super::{Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::oversight::credibility::native_andon::NativeAndonBinding;
use crate::action::{ElapsedTick, Purpose, Scope};
use crate::round::review::andon::{AndonBudget, LedgerIdentity};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_ANDON_WINDOW_QUERIES: usize = 1024;
/// Complete acknowledged history plus the proposed original AndonWindow event.
pub const MAX_ANDON_WINDOW_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAndonWindowError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}

impl fmt::Display for FileAndonWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FileAndonWindowError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAndonWindowBlockReason {
    WindowConflict,
    WindowOverlap,
    WindowLimit,
    NativeAdmissionUnavailable,
    NumericalReplayUnsupported,
    NumericalReplayLimit,
    ReplayLimit,
    OperationLimit,
    OperationAlreadyUsed,
}

/// Exact original native accounting/owner cut. Clock and evaluation coordinates
/// retain their own domains. Reading or registering a window observes no time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileAndonWindowBasis {
    pub scope: Scope,
    pub accounting: LedgerIdentity,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub evaluation_revision: u64,
    pub congress_generation: u64,
    pub policy_generation: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// Select one preprovisioned operation and assert its exact frozen budget.
/// The assertion cannot choose or replace the independently granted window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileAndonWindowRequest {
    operation: u64,
    expected: FileAndonWindowBasis,
    expected_budget: AndonBudget,
}
impl FileAndonWindowRequest {
    pub fn new(
        operation: u64,
        expected: FileAndonWindowBasis,
        expected_budget: AndonBudget,
    ) -> Result<Self, FileAndonWindowError> {
        if operation == 0
            || expected_budget.window == 0
            || expected_budget.start >= expected_budget.end
            || expected.scope.purpose != Purpose::Effect
            || [
                expected.scope.tenant,
                expected.scope.principal,
                expected.scope.run,
                expected.scope.branch,
                expected.scope.authority,
                expected.accounting.deployment,
                expected.accounting.evaluator,
                expected.congress_generation,
                expected.policy_generation,
            ]
            .contains(&0)
        {
            return Err(FileAndonWindowError::MalformedRequest);
        }
        Ok(Self {
            operation,
            expected,
            expected_budget,
        })
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn expected(&self) -> FileAndonWindowBasis {
        self.expected
    }
    pub fn expected_budget(&self) -> AndonBudget {
        self.expected_budget
    }
}

/// A finite preview of the original window admission. It reserves no journal
/// event, predecessor, label or effect. The owning transition rechecks admission.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileAndonWindowPlan,
/// };
/// fn no_effect(plan: FileAndonWindowPlan) -> FilePermit { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAndonWindowPlan {
    basis: FileAndonWindowBasis,
    operation: u64,
    budget: AndonBudget,
    already_registered: bool,
    block_reason: Option<FileAndonWindowBlockReason>,
}
impl FileAndonWindowPlan {
    pub fn basis(&self) -> FileAndonWindowBasis {
        self.basis
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn budget(&self) -> AndonBudget {
        self.budget
    }
    pub fn already_registered(&self) -> bool {
        self.already_registered
    }
    pub fn can_register(&self) -> bool {
        self.block_reason.is_none()
    }
    pub fn block_reason(&self) -> Option<FileAndonWindowBlockReason> {
        self.block_reason
    }
    pub fn request(&self) -> Option<FileAndonWindowRequest> {
        self.can_register().then_some(FileAndonWindowRequest {
            operation: self.operation,
            expected: self.basis,
            expected_budget: self.budget,
        })
    }
}

/// Original durable window registration, or an exact existing-window no-op.
/// This records accounting configuration only. It never closes an incident,
/// changes a label, refunds a false stop or renews effect authority.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileAndonWindowReceipt,
/// };
/// fn no_effect(receipt: FileAndonWindowReceipt) -> FilePermit { receipt }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAndonWindowReceipt {
    before: FileAndonWindowBasis,
    after: FileAndonWindowBasis,
    operation: u64,
    budget: AndonBudget,
    registered: bool,
}
impl FileAndonWindowReceipt {
    pub fn before(&self) -> FileAndonWindowBasis {
        self.before
    }
    pub fn after(&self) -> FileAndonWindowBasis {
        self.after
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn budget(&self) -> AndonBudget {
        self.budget
    }
    pub fn registered(&self) -> bool {
        self.registered
    }
}

pub type FileAndonWindowPlanObservation = Knowledge<FileAndonWindowPlan, FileAndonWindowBasis>;
pub type FileAndonWindowPlanResult = Result<FileAndonWindowPlanObservation, FileAndonWindowError>;
pub type FileAndonWindowObservation = Knowledge<FileAndonWindowReceipt, FileAndonWindowBasis>;
pub type FileAndonWindowResult = Result<FileAndonWindowObservation, FileAndonWindowError>;

struct Record {
    request: FileAndonWindowRequest,
    // None records entered work with no returned result, including an unwind.
    // Lost or uncertain completion cannot become a fresh registration attempt.
    result: Option<FileAndonWindowResult>,
}
struct Grant {
    binding: NativeAndonBinding,
    budget: AndonBudget,
    queries: Cell<usize>,
    operations: Cell<usize>,
    max_replay_events: usize,
    remaining_numerical_replay_events: Option<Cell<usize>>,
    record: RefCell<Option<Record>>,
}

/// Independent noncloneable custody of one exact budget and finite native work.
/// Revoking custody also revokes disclosure of cached results on weak clones.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorAndonWindowGrant;
/// fn no_copy(grant: FileSupervisorAndonWindowGrant) { grant.clone(); }
/// ```
#[must_use = "retain independent custody of the finite native Andon window grant"]
pub struct FileSupervisorAndonWindowGrant {
    grant: Rc<Grant>,
}
impl FileSupervisorAndonWindowGrant {
    pub fn budget(&self) -> AndonBudget {
        self.grant.budget
    }
    pub fn remaining_queries(&self) -> usize {
        self.grant.queries.get()
    }
    pub fn remaining_operations(&self) -> usize {
        self.grant.operations.get()
    }
    pub fn max_replay_events(&self) -> usize {
        self.grant.max_replay_events
    }
    /// None grants no numerical replay. Units reserve complete-history events;
    /// the original immutable recipe continues to own all numerical ceilings.
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
impl fmt::Debug for FileSupervisorAndonWindowGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorAndonWindowGrant")
            .field("budget", &self.budget())
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

/// Weak supervisor-only access to one original live owner and selected budget.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) {
///     actor.andon_window_port(1, 64);
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorAndonWindowPort;
/// fn no_owner(port: FileSupervisorAndonWindowPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorAndonWindowPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
    issuer: Weak<()>,
}
impl FileSupervisorAndonWindowPort {
    pub(crate) fn capability_snapshot(&self) -> super::FileOperatorCapabilityObservation {
        super::capabilities::observe(
            &self.owner,
            &self.grant,
            self.scope,
            "supervisor_andon_window",
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
impl fmt::Debug for FileSupervisorAndonWindowPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorAndonWindowPort { .. }")
    }
}
