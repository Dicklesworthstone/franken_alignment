//! Finite delegation of ONE independently approved original policy campaign.
//! The custodian owns the nonclone native key. No caller-supplied policy,
//! replay report, candidate ID or query can manufacture a governor approval.

mod port;

use super::{Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::{
    FileOversight, governance::campaigns::FilePolicyPromotionPermit,
};
use crate::action::{ElapsedTick, Scope};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_POLICY_PROMOTION_QUERIES: usize = 1024;
/// The complete retained journal plus its proposed Promote event must fit.
/// This ordinary-history bound grants no numerical reconstruction allowance.
pub const MAX_POLICY_PROMOTION_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePolicyPromotionError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}
impl fmt::Display for FilePolicyPromotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FilePolicyPromotionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePolicyPromotionBlockReason {
    Stopped,
    CampaignNotApproved,
    StaleCampaign,
    SourceUnavailable,
    NumericalReplayUnsupported,
    ReplayLimit,
    OperationLimit,
    OperationAlreadyUsed,
}

/// The complete current cut of the selected original owner and campaign.
/// The recorded clock is historical; observing it acquires no fresh lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePolicyPromotionBasis {
    pub scope: Scope,
    pub campaign: u64,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub policy_generation: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// Data selecting an operation at an exact predecessor, never an approval key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePolicyPromotionRequest {
    operation: u64,
    expected: FilePolicyPromotionBasis,
}
impl FilePolicyPromotionRequest {
    pub fn new(
        operation: u64,
        expected: FilePolicyPromotionBasis,
    ) -> Result<Self, FilePolicyPromotionError> {
        if operation == 0 || expected.campaign == 0 {
            return Err(FilePolicyPromotionError::MalformedRequest);
        }
        Ok(Self {
            operation,
            expected,
        })
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn expected(&self) -> FilePolicyPromotionBasis {
        self.expected
    }
}

/// Observed native eligibility for this one already-approved candidate. The
/// original complete-corpus stamp is checked without replay or mutation. A
/// later Promote still requires the retained key and a current predecessor.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{FilePermit,
///     requests::actor::FilePolicyPromotionPlan};
/// fn no_effect(plan: FilePolicyPromotionPlan) -> FilePermit { plan }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePolicyPromotionPlan {
    basis: FilePolicyPromotionBasis,
    operation: u64,
    candidate_policy_generation: u64,
    block_reason: Option<FilePolicyPromotionBlockReason>,
}
impl FilePolicyPromotionPlan {
    pub fn basis(&self) -> FilePolicyPromotionBasis {
        self.basis
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn candidate_policy_generation(&self) -> u64 {
        self.candidate_policy_generation
    }
    pub fn can_promote(&self) -> bool {
        self.block_reason.is_none()
    }
    pub fn block_reason(&self) -> Option<FilePolicyPromotionBlockReason> {
        self.block_reason
    }
    pub fn request(&self) -> Option<FilePolicyPromotionRequest> {
        self.can_promote().then_some(FilePolicyPromotionRequest {
            operation: self.operation,
            expected: self.basis,
        })
    }
}

/// An acknowledged original policy transition. Old undispatched reservations
/// may be refunded; admitted/Unknown effects and their charges are unchanged.
/// A historical retry does not assert that this remains the current policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePolicyPromotionReceipt {
    before: FilePolicyPromotionBasis,
    after: FilePolicyPromotionBasis,
    operation: u64,
    candidate_policy_generation: u64,
    cancelled: Vec<u64>,
    refunded_units: u64,
}
impl FilePolicyPromotionReceipt {
    pub fn before(&self) -> FilePolicyPromotionBasis {
        self.before
    }
    pub fn after(&self) -> FilePolicyPromotionBasis {
        self.after
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn candidate_policy_generation(&self) -> u64 {
        self.candidate_policy_generation
    }
    pub fn cancelled(&self) -> &[u64] {
        &self.cancelled
    }
    pub fn refunded_units(&self) -> u64 {
        self.refunded_units
    }
}

pub type FilePolicyPromotionPlanObservation =
    Knowledge<FilePolicyPromotionPlan, FilePolicyPromotionBasis>;
pub type FilePolicyPromotionPlanResult =
    Result<FilePolicyPromotionPlanObservation, FilePolicyPromotionError>;
pub type FilePolicyPromotionObservation =
    Knowledge<FilePolicyPromotionReceipt, FilePolicyPromotionBasis>;
pub type FilePolicyPromotionResult =
    Result<FilePolicyPromotionObservation, FilePolicyPromotionError>;

struct Record {
    request: FilePolicyPromotionRequest,
    // Entered without acknowledgment, including a caught unwind, never reruns.
    result: Option<FilePolicyPromotionResult>,
}
struct Grant {
    permit: FilePolicyPromotionPermit,
    candidate_policy_generation: u64,
    queries: Cell<usize>,
    operations: Cell<usize>,
    max_replay_events: usize,
    record: RefCell<Option<Record>>,
}

/// Nonclone custody of one original approval and its finite work. All weak
/// clones share one entered operation and one exact-result cache. Revocation
/// suppresses even cached disclosure; neither reconnect nor a new operation ID
/// can replenish the native approval or this allowance.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorPolicyPromotionGrant;
/// fn duplicate(grant: FileSupervisorPolicyPromotionGrant) { grant.clone(); }
/// ```
#[must_use = "retain custody of the independently approved policy campaign"]
pub struct FileSupervisorPolicyPromotionGrant {
    grant: Rc<Grant>,
}
impl FileSupervisorPolicyPromotionGrant {
    pub fn campaign(&self) -> u64 {
        self.grant.permit.campaign()
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
    pub fn revoke(self) {
        drop(self);
    }
}
impl fmt::Debug for FileSupervisorPolicyPromotionGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorPolicyPromotionGrant")
            .field("campaign", &self.campaign())
            .field("remaining_queries", &self.remaining_queries())
            .field("remaining_operations", &self.remaining_operations())
            .finish_non_exhaustive()
    }
}

/// This port cannot request a campaign, approve/reapprove it, select another
/// policy, obtain a governor or create an effect permit. Owner and approval
/// custody must both survive independently of all port clones.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorPolicyPromotionPort;
/// fn no_owner(port: FileSupervisorPolicyPromotionPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorPolicyPromotionPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
    issuer: Weak<()>,
}
impl fmt::Debug for FileSupervisorPolicyPromotionPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorPolicyPromotionPort { .. }")
    }
}

#[cfg(test)]
mod tests;
