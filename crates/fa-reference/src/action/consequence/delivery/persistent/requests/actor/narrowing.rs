//! Separately granted monotone restriction of the original ordinary run.
//! Query plans are observation; only narrow enters the original durable gate.
mod port;
use super::{Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS, observed::FileOversight};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::{ElapsedTick, MAX_ATTEMPTS, ResolvedTarget, Scope};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_NARROW_QUERIES: usize = 1024;
pub const MAX_NARROW_OPERATIONS: usize = 128;
pub const MAX_NARROW_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;
pub const MAX_NARROW_TARGETS: usize = MAX_ATTEMPTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileNarrowError {
    MalformedRequest, IdempotencyConflict, StalePredecessor, Capacity, Unsupported, Unavailable,
}
impl fmt::Display for FileNarrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for FileNarrowError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileNarrowBlockReason {
    Stopped, Suspended, UnsupportedHistory, ReplayLimit, OperationLimit, OperationAlreadyUsed,
}

/// One exact acknowledged original-owner cut. The recorded clock is historical
/// evidence, never a fresh observation or a lease for subsequent work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileNarrowBasis {
    pub scope: Scope,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub recorded_at: Option<ElapsedTick>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileNarrowRequest {
    operation: u64,
    expected: FileNarrowBasis,
    retained_targets: Vec<ResolvedTarget>,
}
impl FileNarrowRequest {
    pub fn new(operation: u64, expected: FileNarrowBasis, targets: Vec<ResolvedTarget>)
        -> Result<Self, FileNarrowError>
    {
        if operation == 0 { return Err(FileNarrowError::MalformedRequest); }
        let retained_targets = canonical(&targets)?;
        Ok(Self { operation, expected, retained_targets })
    }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn expected(&self) -> FileNarrowBasis { self.expected }
    pub fn retained_targets(&self) -> &[ResolvedTarget] { &self.retained_targets }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileNarrowPlan {
    basis: FileNarrowBasis,
    operation: u64,
    retained_targets: Vec<ResolvedTarget>,
    current_ceiling: Option<Vec<ResolvedTarget>>,
    effective_targets: Vec<ResolvedTarget>,
    block_reason: Option<FileNarrowBlockReason>,
}
impl FileNarrowPlan {
    pub fn basis(&self) -> FileNarrowBasis { self.basis }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn retained_targets(&self) -> &[ResolvedTarget] { &self.retained_targets }
    pub fn current_ceiling(&self) -> Option<&[ResolvedTarget]> { self.current_ceiling.as_deref() }
    pub fn effective_targets(&self) -> &[ResolvedTarget] { &self.effective_targets }
    pub fn can_narrow(&self) -> bool { self.block_reason.is_none() }
    pub fn block_reason(&self) -> Option<FileNarrowBlockReason> { self.block_reason }
    pub fn request(&self) -> Option<FileNarrowRequest> {
        self.can_narrow().then(|| FileNarrowRequest { operation: self.operation,
            expected: self.basis, retained_targets: self.retained_targets.clone() })
    }
}

/// Acknowledged original transition. Requested targets and the actual effective
/// intersection remain distinct, so a later wider selection cannot claim wider
/// authority. Refunded units belong only to cancelled pre-dispatch reservations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileNarrowReceipt {
    before: FileNarrowBasis,
    after: FileNarrowBasis,
    operation: u64,
    retained_targets: Vec<ResolvedTarget>,
    previous_ceiling: Option<Vec<ResolvedTarget>>,
    effective_targets: Vec<ResolvedTarget>,
    cancelled: Vec<u64>,
    refunded_units: u64,
}
impl FileNarrowReceipt {
    pub fn before(&self) -> FileNarrowBasis { self.before }
    pub fn after(&self) -> FileNarrowBasis { self.after }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn retained_targets(&self) -> &[ResolvedTarget] { &self.retained_targets }
    pub fn previous_ceiling(&self) -> Option<&[ResolvedTarget]> { self.previous_ceiling.as_deref() }
    pub fn effective_targets(&self) -> &[ResolvedTarget] { &self.effective_targets }
    pub fn cancelled(&self) -> &[u64] { &self.cancelled }
    pub fn refunded_units(&self) -> u64 { self.refunded_units }
}
pub type FileNarrowPlanObservation = Knowledge<FileNarrowPlan, FileNarrowBasis>;
pub type FileNarrowPlanResult = Result<FileNarrowPlanObservation, FileNarrowError>;
pub type FileNarrowObservation = Knowledge<FileNarrowReceipt, FileNarrowBasis>;
pub type FileNarrowResult = Result<FileNarrowObservation, FileNarrowError>;

struct Record { request: FileNarrowRequest, result: Option<FileNarrowResult> }
struct Grant {
    queries: Cell<usize>, operations: Cell<usize>, max_replay_events: usize,
    records: RefCell<BTreeMap<u64, Record>>,
}

/// Dropping original nonclone custody revokes all weak port clones. Neither a
/// reconnect nor a new query can replenish this grant or keep its owner alive.
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorNarrowGrant;
/// fn duplicate(grant: FileSupervisorNarrowGrant) { grant.clone(); }
/// ```
#[must_use = "retain custody for the independently granted authority restriction"]
pub struct FileSupervisorNarrowGrant { grant: Rc<Grant> }
impl FileSupervisorNarrowGrant {
    pub fn remaining_queries(&self) -> usize { self.grant.queries.get() }
    pub fn remaining_operations(&self) -> usize { self.grant.operations.get() }
    pub fn max_replay_events(&self) -> usize { self.grant.max_replay_events }
    pub fn revoke(self) { drop(self); }
}
impl fmt::Debug for FileSupervisorNarrowGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorNarrowGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("remaining_operations", &self.remaining_operations()).finish_non_exhaustive()
    }
}

/// No actor, inspector or precheck capability can mint this independently
/// scoped role, acquire its owner or upgrade a read into a mutation.
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{observed::FileOversight, requests::actor::FileActorPort};
/// fn grant(actor: FileActorPort<FileOversight>) { actor.narrow_port(1, 1, 64); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorNarrowPort;
/// fn host(port: FileSupervisorNarrowPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorNarrowPort {
    owner: Weak<OwnerRef<FileOversight>>, grant: Weak<Grant>, scope: Scope, issuer: Weak<()>,
}
impl fmt::Debug for FileSupervisorNarrowPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("FileSupervisorNarrowPort { .. }") }
}

fn canonical(targets: &[ResolvedTarget]) -> Result<Vec<ResolvedTarget>, FileNarrowError> {
    TargetCeiling::new(targets).map(|ceiling| ceiling.targets()).map_err(|error| match error {
        crate::Error::Limit | crate::Error::Overflow => FileNarrowError::Capacity,
        _ => FileNarrowError::MalformedRequest,
    })
}

#[cfg(test)]
mod tests;
