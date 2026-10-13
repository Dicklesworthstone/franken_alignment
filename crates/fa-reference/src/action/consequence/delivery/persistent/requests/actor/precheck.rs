//! Explicit, revocable exact-predicate disclosure for an original durable owner.
//! A passed precheck is not admission, helper approval, a reservation or a permit.
use super::{ActorError, ActorProposal, FileActorSupervisor, FrozenAction, JournalError,
    Knowledge, OwnerRef, Scope, Snapshot, UnknownReason, ActionSpec, VERSION, MAX_PAYLOAD_BYTES};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::session::policy::{Policy, Truth};
use crate::Error;
use std::{cell::Cell, fmt, rc::{Rc, Weak}};

pub const MAX_PRECHECK_QUERIES: usize = 128;

/// Only exact constraints at a disclosed cut are checked. Both outcomes still
/// require an ordinary new submission; neither supplies any live effect key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePrecheckOutcome { DeniedByPolicy, PrecheckPassed }

/// Read-query provenance, NOT an admitted actor request or a controller sequence.
/// The supervisor chooses the disclosure label; query labels reserve no IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePrecheckBasis {
    pub disclosure: u64,
    pub query: u64,
    pub policy_epoch: u64,
    pub observed_at: ElapsedTick,
}
pub type FilePrecheckObservation = Knowledge<FilePrecheckOutcome, FilePrecheckBasis>;

// Immutable source/policy DATA, constructed only from the acknowledged original
// owner. No reducer, helper roster, role, effect key or live ledger is retained.
pub(in crate::action::consequence::delivery::persistent) struct PrecheckProjection {
    pub revision: u64,
    pub scope: Scope,
    pub target: ResolvedTarget,
    pub policy: Policy,
    pub snapshot: Snapshot,
    pub epoch: u64,
    pub now: ElapsedTick,
    pub available: u64,
    pub ceiling: Option<TargetCeiling>,
    pub stopped: bool,
}
struct Disclosure { id: u64, remaining: Cell<usize>, projection: PrecheckProjection }

/// Keep this with the supervisor. Dropping or consuming it revokes ALL copies
/// of its port. It owns only bounded disclosure data, not the authority lock.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePrecheckDisclosure;
/// fn no_copy(grant: FilePrecheckDisclosure) { let _ = grant.clone(); }
/// ```
#[must_use = "retain this custodian to keep the explicitly disclosed precheck available"]
pub struct FilePrecheckDisclosure { disclosure: Rc<Disclosure> }
impl FilePrecheckDisclosure {
    pub fn remaining_queries(&self) -> usize { self.disclosure.remaining.get() }
    pub fn revoke(self) { drop(self); }
}
impl fmt::Debug for FilePrecheckDisclosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePrecheckDisclosure").field("remaining_queries", &self.remaining_queries()).finish_non_exhaustive()
    }
}

/// A separately provisioned actor capability. The ordinary submit/poll/cancel
/// port does not acquire it. Clones share one finite allowance and revocation.
/// No policy literals, source values, helper data or control sequence is exposed.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{observed::FileOversight,
///     requests::actor::FileActorPort};
/// fn no_self_grant(actor: FileActorPort<FileOversight>) { actor.disclose_exact_precheck(1, 1, 1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPrecheckPort;
/// fn no_owner(port: FileActorPrecheckPort) { let _ = port.host_mut(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePrecheckObservation;
/// fn no_permission(observation: FilePrecheckObservation) -> FilePermit { observation }
/// ```
#[derive(Clone)]
pub struct FileActorPrecheckPort {
    owner: Weak<OwnerRef<FileOversight>>,
    disclosure: Weak<Disclosure>,
}
impl fmt::Debug for FileActorPrecheckPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("FileActorPrecheckPort { .. }") }
}

impl FileActorSupervisor<FileOversight> {
    /// Explicit exact-predicate disclosure, default absent. The mandatory source
    /// must be complete and fresh at the owner's ALREADY observed clock. This
    /// reads no file/clock, consumes no admission snapshot and writes no event.
    ///
    /// The grant is restricted to this exact acknowledged revision. Any host
    /// transition retires eligibility, even an unrelated one; there is no covert
    /// reuse of an old policy or renewal from actor input. A supervisor can issue
    /// a new grant after fresh observation, but cannot refresh the old port.
    /// Source-only ordinary publications are supported; numerical/generated and
    /// stream/predictive lanes have no raw-proposal disclosure fallback.
    pub fn disclose_exact_precheck(&self, expected_revision: u64, disclosure: u64, queries: usize)
        -> Result<(FileActorPrecheckPort, FilePrecheckDisclosure), JournalError>
    {
        if disclosure == 0 || queries == 0 { return Err(Error::InvalidInput.into()); }
        if queries > MAX_PRECHECK_QUERIES { return Err(Error::Limit.into()); }
        let owner = self.owner.try_borrow().map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if owner.host.revision() != expected_revision { return Err(Error::Stale.into()); }
        let projection = owner.host.actor_precheck_projection()?;
        let grant = Rc::new(Disclosure { id: disclosure, remaining: Cell::new(queries), projection });
        let port = FileActorPrecheckPort { owner: Rc::downgrade(&self.owner), disclosure: Rc::downgrade(&grant) };
        Ok((port, FilePrecheckDisclosure { disclosure: grant }))
    }
}

impl FileActorPrecheckPort {
    /// One bounded read, not a submission or an idempotent effect request.
    /// Labels are echoed as read provenance only; repeated labels still spend
    /// query work. Invalid proposals and evaluator failures spend their entered
    /// query too. Revoked/stale/unavailable grants perform no evaluation.
    pub fn check(&self, query: u64, proposal: &ActorProposal)
        -> Result<FilePrecheckObservation, ActorError>
    {
        let Some(grant) = self.disclosure.upgrade() else {
            return Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" });
        };
        let unknown = || Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable };
        let Some(owner) = self.owner.upgrade() else { return Ok(unknown()); };
        let Ok(state) = owner.try_borrow() else { return Ok(unknown()); };
        let projection = &grant.projection;
        if state.host.storage_failure().is_some() || state.host.revision() != projection.revision
            || state.host.capture_file_policy_state().is_err() { return Ok(unknown()); }
        let remaining = grant.remaining.get();
        if remaining == 0 { return Err(ActorError::Capacity); }
        // Charge before any fallible freeze/evaluation (including an unwind).
        grant.remaining.set(remaining - 1);
        if query == 0 { return Err(ActorError::MalformedProposal); }
        if proposal.payload.len() > MAX_PAYLOAD_BYTES || proposal.payload.len() as u64 > proposal.units {
            return Err(ActorError::Capacity);
        }
        let action = FrozenAction::freeze(ActionSpec { version: VERSION, scope: projection.scope,
            target: Some(proposal.target), payload: proposal.payload.clone(), required_witnesses: Vec::new(),
            policy_epoch: proposal.expected_policy_epoch, deadline: proposal.deadline, units: proposal.units })
            .map_err(|_| ActorError::MalformedProposal)?;
        if proposal.expected_policy_epoch != projection.epoch {
            return Ok(Knowledge::Stale { generation: proposal.expected_policy_epoch, current: projection.epoch });
        }
        // Exact resource/target screens use the captured ORIGINAL owner state.
        // They do not create a shadow admission or reserve available units.
        let denied = projection.stopped || proposal.target != projection.target
            || projection.ceiling.as_ref().is_some_and(|ceiling| !ceiling.contains(proposal.target))
            || projection.now >= proposal.deadline || proposal.units > projection.available;
        let value = if denied { FilePrecheckOutcome::DeniedByPolicy } else {
            let evaluation = match projection.policy.evaluate(&action, &projection.snapshot) {
                Ok(evaluation) => evaluation,
                Err(Error::Limit | Error::Overflow) => return Err(ActorError::Capacity),
                Err(_) => return Ok(unknown()),
            };
            if evaluation.trace().iter().any(|step| step.result == Truth::Unknown) { return Ok(unknown()); }
            if evaluation.certifiable() { FilePrecheckOutcome::PrecheckPassed }
            else { FilePrecheckOutcome::DeniedByPolicy }
        };
        Ok(Knowledge::Known { value, basis: FilePrecheckBasis {
            disclosure: grant.id, query, policy_epoch: projection.epoch, observed_at: projection.now,
        } })
    }
}

#[cfg(test)]
mod tests;
