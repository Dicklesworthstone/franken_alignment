//! Separately granted diagnostics from one ORIGINAL acknowledged publication cut.
//! These are recorded necessary conditions, never a current dispatch decision.
//! No source, clock, helper, replay, admission snapshot or authority is consumed.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, FileRequestDisposition, JournalError,
    Knowledge, OwnerRef, UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::{ActionState, ElapsedTick, Scope};
use std::cell::Cell;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_HOLD_DIAGNOSIS_QUERIES: usize = 1024;

/// One acknowledged native cut. The request generation is request-local; it is
/// neither the journal revision nor the authority control sequence. Recorded
/// time is historical data, not a new clock observation or a current lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileHoldDiagnosisBasis {
    pub scope: Scope,
    pub request: u64,
    pub request_generation: u64,
    pub attempt: u64,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// A fixed, bounded set of observations. False recorded conditions are obstacles
/// at this cut, not the causal explanation of a historical terminal outcome.
/// Even all true recorded conditions do not establish live publication eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileHoldCondition {
    RecordedAdmissionOpen,
    RecordedNotSuspended,
    RecordedPolicyEpochMatches,
    RecordedDeadlineOpen,
    RecordedStageAuthorized,
    /// Whether this attempt's recorded lifecycle has not admitted dispatch.
    /// This is never an independent observation of external nonexecution.
    DispatchNotRecorded,
    CurrentSourceEvidence,
    CurrentApprovals,
    CurrentClock,
    FinalPublicationEligibility,
}

/// One condition, retaining its exact native basis when known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHoldPredicate {
    condition: FileHoldCondition,
    satisfied: Knowledge<bool, FileHoldDiagnosisBasis>,
}

impl FileHoldPredicate {
    pub fn condition(&self) -> FileHoldCondition {
        self.condition
    }

    pub fn satisfied(&self) -> &Knowledge<bool, FileHoldDiagnosisBasis> {
        &self.satisfied
    }
}

/// Sealed recorded state of an admitted original request. No policy literals,
/// payload, helper input, human key or usable permit is disclosed. Unknown
/// outcomes stay charged in the original owner; this value offers no retry or
/// refund authority. Terminal stages are reported as terminal history, not as
/// held effects that can be resumed by satisfying these conditions.
///
/// The final four conditions remain Unknown even when a recorded Authorized
/// stage, unchanged epoch and unexpired recorded time are all present. Their
/// controller observations are unavailable to this read. The existing
/// ControllerUnavailable reason does not imply that the owner itself died.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileHoldDiagnosis,
/// };
/// fn no_approval(value: FileHoldDiagnosis) -> FilePermit { value }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHoldDiagnosis {
    basis: FileHoldDiagnosisBasis,
    stage: ActionState,
    proposal_policy_epoch: u64,
    current_policy_epoch: u64,
    deadline: ElapsedTick,
    predicates: Vec<FileHoldPredicate>,
}

impl FileHoldDiagnosis {
    pub fn basis(&self) -> FileHoldDiagnosisBasis {
        self.basis
    }

    pub fn stage(&self) -> ActionState {
        self.stage
    }

    pub fn proposal_policy_epoch(&self) -> u64 {
        self.proposal_policy_epoch
    }

    pub fn current_policy_epoch(&self) -> u64 {
        self.current_policy_epoch
    }

    pub fn deadline(&self) -> ElapsedTick {
        self.deadline
    }

    pub fn predicates(&self) -> &[FileHoldPredicate] {
        &self.predicates
    }
}

pub type FileHoldDiagnosisObservation = Knowledge<FileHoldDiagnosis, FileHoldDiagnosisBasis>;
pub type FileHoldDiagnosisResult = Result<FileHoldDiagnosisObservation, ActorError>;

struct Grant {
    remaining: Cell<usize>,
}

/// Noncopyable custody of one finite shared disclosure allowance. Dropping or
/// revoking it invalidates every weak port clone without retaining the owner.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorHoldDiagnosisGrant;
/// fn no_copy(grant: FileSupervisorHoldDiagnosisGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for the explicitly granted recorded-state diagnosis"]
pub struct FileSupervisorHoldDiagnosisGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorHoldDiagnosisGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }

    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorHoldDiagnosisGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorHoldDiagnosisGrant")
            .field("remaining_queries", &self.remaining_queries())
            .finish_non_exhaustive()
    }
}

/// Weak supervisor read access to one exact original owner. Ordinary actor,
/// precheck, inspection and explanation ports cannot create this grant.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) { actor.hold_diagnosis_port(1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorHoldDiagnosisPort;
/// fn no_owner(port: FileSupervisorHoldDiagnosisPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorHoldDiagnosisPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorHoldDiagnosisPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorHoldDiagnosisPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Grant bounded recorded-state disclosure without reading a source, clock,
    /// replay history or admission snapshot. The grant follows only this live
    /// acknowledged owner; it cannot adopt a later reopened incarnation.
    pub fn hold_diagnosis_port(
        &self,
        queries: usize,
    ) -> Result<
        (
            FileSupervisorHoldDiagnosisPort,
            FileSupervisorHoldDiagnosisGrant,
        ),
        JournalError,
    > {
        if queries == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_HOLD_DIAGNOSIS_QUERIES {
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
        });
        Ok((
            FileSupervisorHoldDiagnosisPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorHoldDiagnosisGrant { grant },
        ))
    }
}

impl FileSupervisorHoldDiagnosisPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Inspect an external request through its original request-to-attempt map
    /// and one immutable borrow of the acknowledged owner. This never searches
    /// a partial situation page, reads a canonical file or replays model work.
    ///
    /// A missing or NotAdmitted request is Unknown, never a fabricated attempt
    /// or closed-domain absence. Every entered lookup spends one shared query,
    /// including unknown results and retries through another clone. No later
    /// serialization failure refunds the work. Malformed zero, revoked grants,
    /// lost owners, conflicting borrows and faulted storage enter no lookup.
    pub fn why_held(&self, request: u64) -> FileHoldDiagnosisResult {
        if request == 0 {
            return Err(ActorError::MalformedProposal);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_hold_diagnosis",
            });
        };
        let unknown = || Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some() {
            return Ok(unknown());
        }
        let remaining = grant.remaining.get();
        if remaining == 0 {
            return Err(ActorError::Capacity);
        }
        grant.remaining.set(remaining - 1);
        if owner.host.actor_explanation_scope() != self.scope {
            return Ok(unknown());
        }
        let Ok(status) = owner.host.request_status(request) else {
            return Ok(unknown());
        };
        let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
            return Ok(unknown());
        };
        let Ok(action) = owner.host.request_action(request) else {
            return Ok(unknown());
        };
        let action = action.spec();
        let snapshot = owner.host.inspect();
        if status.request != request
            || attempt == 0
            || action.scope != self.scope
            || snapshot.control.ledger.stages.get(&attempt) != Some(&stage)
            || snapshot.revision != owner.host.revision()
        {
            return Ok(unknown());
        }
        let basis = FileHoldDiagnosisBasis {
            scope: self.scope,
            request,
            request_generation: status.generation,
            attempt,
            journal_revision: snapshot.revision,
            control_sequence: snapshot.control.sequence,
            recorded_at: snapshot.control.ledger.elapsed,
        };
        let recorded = |condition, value| FileHoldPredicate {
            condition,
            satisfied: Knowledge::Known { value, basis },
        };
        let unavailable = |condition| FileHoldPredicate {
            condition,
            satisfied: Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable,
            },
        };
        use FileHoldCondition::*;
        let deadline = match basis.recorded_at {
            Some(now) => recorded(RecordedDeadlineOpen, now < action.deadline),
            None => unavailable(RecordedDeadlineOpen),
        };
        let dispatch_not_recorded = match stage {
            ActionState::Proposed
            | ActionState::Prepared
            | ActionState::Reviewing
            | ActionState::Authorized
            | ActionState::Denied
            | ActionState::Cancelled => true,
            ActionState::Dispatching
            | ActionState::Unknown
            | ActionState::IrrecoverablyUnknown
            | ActionState::Confirmed
            | ActionState::ConfirmedNotExecuted => false,
        };
        let predicates = vec![
            recorded(RecordedAdmissionOpen, snapshot.stop.is_none()),
            recorded(RecordedNotSuspended, !snapshot.control.suspended),
            recorded(
                RecordedPolicyEpochMatches,
                action.policy_epoch == snapshot.control.ledger.epoch,
            ),
            deadline,
            recorded(RecordedStageAuthorized, stage == ActionState::Authorized),
            recorded(DispatchNotRecorded, dispatch_not_recorded),
            unavailable(CurrentSourceEvidence),
            unavailable(CurrentApprovals),
            unavailable(CurrentClock),
            unavailable(FinalPublicationEligibility),
        ];
        Ok(Knowledge::Known {
            value: FileHoldDiagnosis {
                basis,
                stage,
                proposal_policy_epoch: action.policy_epoch,
                current_policy_epoch: snapshot.control.ledger.epoch,
                deadline: action.deadline,
                predicates,
            },
            basis,
        })
    }
}

#[cfg(test)]
mod tests;
