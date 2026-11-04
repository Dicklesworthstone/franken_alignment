//! Bounded supervisor containment at an unchanged original checkpoint.
//! The only caller is the separately selected owned prompt-stream boundary.
//! This does not add a general actor-facing authority setter or an effect key.
use super::{CheckpointHandle, ContainmentAuthority, RestartGrade};
use crate::action::consequence::Consequence;
use crate::action::consequence::gate::{MAX_DECISIONS, ReviewBinding, TargetCeiling, undispatched};
use crate::action::{ActionState, Scope};
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(crate) struct CheckpointNarrowRequest {
    pub(crate) checkpoint: CheckpointHandle,
    pub(crate) expected_control_sequence: u64,
    pub(crate) expected_actor_revision: u64,
    pub(crate) expected_authority_epoch: u64,
    pub(crate) binding: ReviewBinding,
    pub(crate) retained_targets: TargetCeiling,
}

/// Historical original-gate consequence, never a permit or an approval key.
/// Narrowing fences all old undispatched attempts without resetting the actor
/// or incrementing the later reset's incident counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointNarrowReceipt {
    pub scope: Scope,
    pub sequence: u64,
    pub binding: ReviewBinding,
    pub checkpoint: u64,
    pub checkpoint_actor_revision: u64,
    pub checkpoint_control_sequence: u64,
    pub actor_revision: u64,
    pub incident_count: u64,
    pub consequence: Consequence,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
    pub revocation_floor: u64,
    pub ceiling: TargetCeiling,
}

impl ContainmentAuthority {
    pub(crate) fn check_checkpoint_narrow(&self, request: &CheckpointNarrowRequest) -> Result<(), Error> {
        if self.gate.suspended { return Err(Error::WrongState); }
        if request.expected_control_sequence != self.gate.sequence
            || request.expected_actor_revision != self.actor_revision
            || request.expected_authority_epoch != self.gate.authority.rights.epoch() {
            return Err(Error::Stale);
        }
        if !Rc::ptr_eq(&request.checkpoint.issuer, &self.gate.authority.issuer) {
            return Err(Error::Binding);
        }
        if request.binding.round == 0 || request.binding.reducer_generation == 0
            || request.binding.evidence_root == [0; 32] { return Err(Error::InvalidInput); }
        if self.gate.seen_rounds.contains(&request.binding.round) { return Err(Error::Duplicate); }
        if self.gate.seen_rounds.len() >= MAX_DECISIONS { return Err(Error::Limit); }
        let checkpoint = self.checkpoints.get(&request.checkpoint.id).ok_or(Error::Missing)?;
        if checkpoint.actor_revision != self.actor_revision || checkpoint.actor != self.actor {
            return Err(Error::Binding);
        }
        if checkpoint.actor.profile.grade == RestartGrade::AuditOnly { return Err(Error::Incomplete); }
        if checkpoint.control_sequence > self.gate.sequence { return Err(Error::Stale); }
        Ok(())
    }

    /// Restrict current authority and fence old keys at this unchanged native
    /// checkpoint. Only original pre-dispatch reservations can be refunded;
    /// dispatched, Unknown and terminal outcomes remain owned by the same gate.
    pub(crate) fn narrow_checkpoint(&mut self, request: CheckpointNarrowRequest)
        -> Result<CheckpointNarrowReceipt, Error>
    {
        self.check_checkpoint_narrow(&request)?;
        let checkpoint = self.checkpoints.get(&request.checkpoint.id).expect("checked checkpoint");
        let sequence = self.gate.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let ceiling = match &self.gate.ceiling {
            Some(current) => current.intersect(&request.retained_targets),
            None => request.retained_targets,
        };
        let cancelled: Vec<u64> = self.gate.authority.attempts.iter()
            .filter_map(|(id, attempt)| undispatched(attempt.stage).then_some(*id)).collect();
        // Accounting is staged, while issuer, actor and delivery history stay
        // with the original owner. No fallible logical operation follows publish.
        let mut rights = self.gate.authority.rights.clone();
        if !rights.conserved() { return Err(Error::WrongState); }
        let previous_available = rights.available();
        for id in &cancelled {
            if self.gate.authority.attempts[id].stage == ActionState::Authorized {
                rights.abort_before_dispatch(*id)?;
            }
        }
        rights.revoke_epoch()?;
        if !rights.conserved() { return Err(Error::WrongState); }
        let refunded_units = rights.available().checked_sub(previous_available).ok_or(Error::WrongState)?;
        let receipt = CheckpointNarrowReceipt {
            scope: self.gate.authority.scope, sequence, binding: request.binding,
            checkpoint: request.checkpoint.id, checkpoint_actor_revision: checkpoint.actor_revision,
            checkpoint_control_sequence: checkpoint.control_sequence, actor_revision: self.actor_revision,
            incident_count: rights.incident_count(), consequence: Consequence::NarrowAuthority,
            cancelled: cancelled.clone(), refunded_units, revocation_floor: rights.epoch(),
            ceiling: ceiling.clone(),
        };
        self.gate.authority.rights = rights;
        for id in cancelled {
            self.gate.authority.attempts.get_mut(&id).expect("retained attempt").stage = ActionState::Cancelled;
            self.gate.decisions.insert(id, Consequence::Deny);
        }
        self.gate.sequence = sequence;
        self.gate.ceiling = Some(ceiling);
        self.gate.seen_rounds.insert(request.binding.round);
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests;
