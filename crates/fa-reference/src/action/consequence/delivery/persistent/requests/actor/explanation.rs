//! Separately granted, bounded explanation of an ORIGINAL policy admission.
//! This historical read does not explain later holds, predict permission or run
//! an experiment. The existing investigation replays the acknowledged owner;
//! this adapter supplies neither a policy evaluator nor an authority reducer.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, FileRequestDisposition, JournalError,
    Knowledge, OwnerRef, UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::experiment::InterventionScope;
use crate::action::consequence::gate::containment::session::policy::{Evaluation, Policy};
use crate::action::{ActionState, ElapsedTick, ResolvedTarget, Scope};
use std::cell::Cell;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_EXPLANATION_QUERIES: usize = 1024;
/// A query replays at most this many retained original events. Operators may
/// grant a smaller bound. Numerical decoder and action-predictor histories also
/// require an explicitly selected cumulative numerical-transition allowance.
pub const MAX_EXPLANATION_EVENTS: usize = MAX_JOURNAL_EVENTS;

/// Two historical cuts of the same acknowledged owner. The admission policy
/// applies at `admission_revision`; the ending state applies at
/// `journal_revision`. Neither recorded tick is a fresh clock observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileExplanationBasis {
    pub scope: Scope,
    pub request: u64,
    pub attempt: u64,
    pub admission_revision: u64,
    pub admission_control_sequence: u64,
    pub admitted_at: Option<ElapsedTick>,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// Exact original predicates, trace and selected witnesses at admission, with
/// the distinct original admission and ending states. Private fields bind the
/// projection to its native basis; copying this historical data conveys no
/// authority. The grant explicitly discloses policy literals and the witnesses
/// actually selected by that policy. No directory, whole source snapshot,
/// helper input or action payload is returned.
///
/// A satisfied admission predicate is not evidence that a current hold can be
/// removed: human review, later policy, source, accounting and publication gates
/// are outside this historical explanation.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{FilePermit,
///     requests::actor::FileAdmissionExplanation};
/// fn no_approval(value: FileAdmissionExplanation) -> FilePermit { value }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAdmissionExplanation {
    basis: FileExplanationBasis,
    policy: Policy,
    evaluation: Evaluation,
    initial_state: ActionState,
    ending_state: ActionState,
    proposal_policy_epoch: u64,
    semantic_epoch: u64,
    target: Option<ResolvedTarget>,
    units: u64,
    payload_bytes: usize,
}

impl FileAdmissionExplanation {
    pub fn basis(&self) -> FileExplanationBasis {
        self.basis
    }
    pub fn policy(&self) -> &Policy {
        &self.policy
    }
    pub fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }
    pub fn initial_state(&self) -> ActionState {
        self.initial_state
    }
    pub fn ending_state(&self) -> ActionState {
        self.ending_state
    }
    pub fn proposal_policy_epoch(&self) -> u64 {
        self.proposal_policy_epoch
    }
    pub fn semantic_epoch(&self) -> u64 {
        self.semantic_epoch
    }
    pub fn target(&self) -> Option<ResolvedTarget> {
        self.target
    }
    pub fn units(&self) -> u64 {
        self.units
    }
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}

pub type FileExplanationObservation = Knowledge<FileAdmissionExplanation, FileExplanationBasis>;
pub type FileExplanationResult = Result<FileExplanationObservation, ActorError>;

struct Grant {
    remaining: Cell<usize>,
    max_replay_events: usize,
    remaining_numerical_replay_events: Option<Cell<usize>>,
}

/// Noncopyable host custody for policy-and-witness disclosure. Dropping or
/// revoking it invalidates every port clone without retaining the owner lock.
/// Existing situation-inspection grants cannot provision this stronger read.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorExplanationGrant;
/// fn no_copy(grant: FileSupervisorExplanationGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for the explicitly granted admission explanation"]
pub struct FileSupervisorExplanationGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorExplanationGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }
    pub fn max_replay_events(&self) -> usize {
        self.grant.max_replay_events
    }
    /// Shared original-history transition reservations. None means numerical
    /// reconstruction was not granted; Some(0) means the selected work was spent.
    /// These units are not arithmetic products, CPU time, memory or wall time.
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

impl fmt::Debug for FileSupervisorExplanationGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorExplanationGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("max_replay_events", &self.max_replay_events())
            .field(
                "remaining_numerical_replay_events",
                &self.remaining_numerical_replay_events(),
            )
            .finish_non_exhaustive()
    }
}

/// Weak read access to one exact original owner and shared finite grant. There
/// is no source, clock, admission, experiment, review, effect or mutable-host API.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{observed::FileOversight,
///     requests::actor::FileActorPort};
/// fn no_self_grant(actor: FileActorPort<FileOversight>) { actor.explanation_port(1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorInspectionPort;
/// fn no_upgrade(port: FileSupervisorInspectionPort) { port.explanation_port(1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorExplanationPort;
/// fn no_owner(port: FileSupervisorExplanationPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorExplanationPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorExplanationPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorExplanationPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Explicitly disclose original admission policies and their selected
    /// witnesses with a finite query allowance and the native maximum event
    /// bound. Provisioning performs no capture, source/time read or write.
    pub fn explanation_port(
        &self,
        queries: usize,
    ) -> Result<
        (
            FileSupervisorExplanationPort,
            FileSupervisorExplanationGrant,
        ),
        JournalError,
    > {
        self.explanation_port_with_budget(queries, MAX_EXPLANATION_EVENTS)
    }

    /// As `explanation_port`, with a smaller maximum original history per query.
    /// Exceeding this bound yields charged Unknown, never a prefix explanation.
    /// Numerical replay remains disabled unless explicitly selected separately.
    pub fn explanation_port_with_budget(
        &self,
        queries: usize,
        max_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorExplanationPort,
            FileSupervisorExplanationGrant,
        ),
        JournalError,
    > {
        self.explanation_port_with_limits(queries, max_replay_events, None)
    }

    /// Reconstruct ORIGINAL decoder and action-predictor admissions, including
    /// exact policy denials, under a separate cumulative work allowance. Raw
    /// frame predictors require this selection too; no caller recipe is accepted.
    ///
    /// One unit reserves one original journal transition. The entire acknowledged
    /// cut, including nonnumerical events, is reserved before reconstruction.
    /// Bootstrap, prediction, refinement and later events retain their original
    /// immutable numerical limits. This is not an arithmetic-products, CPU,
    /// memory or wall-clock bound.
    ///
    /// An insufficient allowance spends the entered query without replay. Once
    /// admitted, reconstruction, attribution, unwind or output failure cannot
    /// refund the complete reservation. Missing/unadmitted requests spend only
    /// the query. Clones share both counters; other grants and live numerical
    /// allowances are independent. Provisioning reads no source, clock or history.
    pub fn explanation_port_with_numerical_budget(
        &self,
        queries: usize,
        max_replay_events: usize,
        numerical_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorExplanationPort,
            FileSupervisorExplanationGrant,
        ),
        JournalError,
    > {
        self.explanation_port_with_limits(
            queries,
            max_replay_events,
            Some(numerical_replay_events),
        )
    }

    fn explanation_port_with_limits(
        &self,
        queries: usize,
        max_replay_events: usize,
        numerical_replay_events: Option<usize>,
    ) -> Result<
        (
            FileSupervisorExplanationPort,
            FileSupervisorExplanationGrant,
        ),
        JournalError,
    > {
        if queries == 0 || max_replay_events == 0 || numerical_replay_events == Some(0) {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_EXPLANATION_QUERIES || max_replay_events > MAX_EXPLANATION_EVENTS {
            return Err(Error::Limit.into());
        }
        let possible_replay_events = queries
            .checked_mul(max_replay_events)
            .ok_or(Error::Overflow)?;
        if numerical_replay_events.is_some_and(|events| events > possible_replay_events) {
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
            max_replay_events,
            remaining_numerical_replay_events: numerical_replay_events.map(Cell::new),
        });
        Ok((
            FileSupervisorExplanationPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorExplanationGrant { grant },
        ))
    }
}

impl FileSupervisorExplanationPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Explain an external request key in this original actor scope. Resolve
    /// its native disposition before investigating its allocated attempt; equal
    /// numeric request/attempt IDs never substitute for that mapping.
    ///
    /// The whole original acknowledged suffix must replay within the grant's
    /// event bound. The original investigation reconstructs historical inputs
    /// in memory; it does not read disk, refresh a source, sample time, consume
    /// the supervisor's one-use snapshot or run a counterfactual experiment.
    /// Numerical decoder/action-predictor histories require a separately selected
    /// cumulative allowance. The full acknowledged cut is reserved before the
    /// original investigator executes any event. Missing/unadmitted requests and
    /// reconstruction failures are Unknown, never proved absence.
    ///
    /// Every entered query spends once before lookup/replay, including unknown
    /// results and repeated requests through another clone. There is no refund
    /// for a later encoding failure or a caught unwind.
    pub fn explain_request(&self, request: u64) -> FileExplanationResult {
        if request == 0 {
            return Err(ActorError::MalformedProposal);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_explanation",
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
        let revision = owner.host.revision();
        if revision > grant.max_replay_events as u64
            || owner.host.actor_explanation_scope() != self.scope
        {
            return Ok(unknown());
        }
        let numerical = owner.host.decoder_required() || owner.host.action_consistency_required();
        if numerical && grant.remaining_numerical_replay_events.is_none() {
            return Ok(unknown());
        }
        let Ok(status) = owner.host.request_status(request) else {
            return Ok(unknown());
        };
        let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
            return Ok(unknown());
        };
        let Ok(scope) = InterventionScope::new(request, false, false, false, &[]) else {
            return Ok(unknown());
        };
        if numerical {
            // revision() is the exact retained event count. The same immutable
            // owner borrow pins that cut through the original single-pass replay.
            let Ok(events) = usize::try_from(revision) else {
                return Ok(unknown());
            };
            let allowance = grant
                .remaining_numerical_replay_events
                .as_ref()
                .expect("checked numerical explanation grant");
            let Some(remaining) = allowance.get().checked_sub(events) else {
                return Ok(unknown());
            };
            // This must precede investigator construction: its eager native
            // replay can execute numerical kernels from the first Enable event.
            allowance.set(remaining);
        }
        let Ok(investigation) = owner.host.investigate_proposal(attempt, scope) else {
            return Ok(unknown());
        };
        let origin = investigation.origin();
        let proposal = origin.proposal();
        let action = proposal.action.spec();
        let admission = origin.admission_snapshot();
        let ending = origin.journal_snapshot();
        if status.request != request
            || origin.external_request() != Some(request)
            || proposal.attempt != attempt
            || action.scope != self.scope
            || admission.control.ledger.stages.get(&attempt) != Some(&proposal.state)
            || ending.control.ledger.stages.get(&attempt) != Some(&stage)
            || admission.revision > ending.revision
            || ending.revision != owner.host.revision()
        {
            return Ok(unknown());
        }
        let basis = FileExplanationBasis {
            scope: self.scope,
            request,
            attempt,
            admission_revision: admission.revision,
            admission_control_sequence: admission.control.sequence,
            admitted_at: admission.control.ledger.elapsed,
            journal_revision: ending.revision,
            control_sequence: ending.control.sequence,
            recorded_at: ending.control.ledger.elapsed,
        };
        Ok(Knowledge::Known {
            value: FileAdmissionExplanation {
                basis,
                policy: proposal.policy.as_ref().clone(),
                evaluation: proposal.evaluation.clone(),
                initial_state: proposal.state,
                ending_state: stage,
                proposal_policy_epoch: action.policy_epoch,
                semantic_epoch: proposal.snapshot_semantic_epoch,
                target: action.target,
                units: action.units,
                payload_bytes: action.payload.len(),
            },
            basis,
        })
    }
}

#[cfg(test)]
mod tests;
