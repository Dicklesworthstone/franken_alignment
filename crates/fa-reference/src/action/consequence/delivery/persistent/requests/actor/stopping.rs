//! One separately granted admission fence on the original durable owner.
//! This invokes the existing permanent Stop transition. It acknowledges only
//! this controller's closed admission and raised revocation floor, not endpoint
//! fencing, effect settlement, actor-process termination or a fleet-wide halt.

mod port;

use super::{Knowledge, OwnerRef};
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::{StopProgress, StopReceipt};
use crate::action::Scope;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

/// Complete acknowledged history plus the proposed original Stop event.
/// Event counts are not a CPU, numerical-products or elapsed-time estimate.
pub const MAX_FENCE_REPLAY_EVENTS: usize = MAX_JOURNAL_EVENTS;

/// Exact native predecessor. A copied observation supplies no mutation grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFenceBasis {
    pub scope: Scope,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFenceError {
    MalformedRequest,
    IdempotencyConflict,
    StalePredecessor,
    Capacity,
    Unsupported,
    Unavailable,
}

impl fmt::Display for FileFenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FileFenceError {}

/// One caller-selected operation, bound to the complete observed predecessor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFenceRequest {
    operation: u64,
    expected: FileFenceBasis,
}

impl FileFenceRequest {
    pub fn new(operation: u64, expected: FileFenceBasis) -> Result<Self, FileFenceError> {
        if operation == 0 {
            return Err(FileFenceError::MalformedRequest);
        }
        Ok(Self { operation, expected })
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn expected(&self) -> FileFenceBasis {
        self.expected
    }
}

/// Original acknowledged admission Stop and accounting at its exact ending cut.
/// Later endpoint progress does not rewrite this historical receipt. Dispatched
/// and uncertain effects stay charged until the original endpoint proves their
/// outcome; the receipt cannot be substituted for a nonexecution receipt.
///
/// Neither this value nor a successful operation proves that an actor process
/// exited, helpers were reaped, or other authority domains stopped.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileFenceReceipt;
/// fn refund(receipt: FileFenceReceipt) -> EndpointReceipt { receipt }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFenceReceipt {
    before: FileFenceBasis,
    after: FileFenceBasis,
    operation: u64,
    stop: StopReceipt,
    progress: StopProgress,
}

impl FileFenceReceipt {
    pub fn before(&self) -> FileFenceBasis {
        self.before
    }

    pub fn after(&self) -> FileFenceBasis {
        self.after
    }

    pub fn operation(&self) -> u64 {
        self.operation
    }

    pub fn stop(&self) -> &StopReceipt {
        &self.stop
    }

    pub fn progress(&self) -> &StopProgress {
        &self.progress
    }
}

pub type FileFenceObservation = Knowledge<FileFenceReceipt, FileFenceBasis>;
pub type FileFenceResult = Result<FileFenceObservation, FileFenceError>;

struct Record {
    request: FileFenceRequest,
    // None records entered but unacknowledged work, including an unwind.
    // The same operation can never enter the native transition again.
    result: Option<FileFenceResult>,
}

struct Grant {
    operations: Cell<usize>,
    max_replay_events: usize,
    remaining_numerical_replay_events: Option<Cell<usize>>,
    record: RefCell<Option<Record>>,
}

/// Nonclone custody of one original admission-stop operation. Every weak port
/// clone shares its operation, numerical reservation and historical result.
/// Dropping this custodian revokes even cached disclosure. Replacing a grant or
/// reopening the owner never reconstructs this grant's result cache.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorFenceGrant;
/// fn copy(grant: FileSupervisorFenceGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for the explicitly granted original admission fence"]
pub struct FileSupervisorFenceGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorFenceGrant {
    pub fn remaining_operations(&self) -> usize {
        self.grant.operations.get()
    }

    pub fn max_replay_events(&self) -> usize {
        self.grant.max_replay_events
    }

    pub fn remaining_numerical_replay_events(&self) -> Option<usize> {
        self.grant.remaining_numerical_replay_events.as_ref().map(Cell::get)
    }

    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorFenceGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorFenceGrant")
            .field("remaining_operations", &self.remaining_operations())
            .field("max_replay_events", &self.max_replay_events())
            .field(
                "remaining_numerical_replay_events",
                &self.remaining_numerical_replay_events(),
            )
            .finish_non_exhaustive()
    }
}

/// Weak, separately revocable access to one original-owner admission Stop.
/// This cannot keep its supervisor's authority lock alive. No clock, source,
/// helper, publication, drain or owner getter is present.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorFencePort;
/// fn escape(port: FileSupervisorFencePort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorFencePort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorFencePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorFencePort { .. }")
    }
}

#[cfg(test)]
mod tests;
