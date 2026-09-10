//! Fenced, weight-only congress replacement used by independent-label oversight.
//! No roster, cohort, threshold, quota, exact policy or target-ceiling change is
//! expressible through this transition. There is no external raw setter.

use super::super::{PolicyAuthority, validate_congress};
use crate::action::ActionState;
use crate::action::consequence::congress::CongressPolicy;
use crate::action::consequence::gate::undispatched;
use crate::action::consequence::oversight::credibility::CongressChange;
use crate::Error;

impl PolicyAuthority {
    pub(crate) fn congress_policy(&self) -> &CongressPolicy { &self.congress }

    pub(crate) fn replace_congress_weights(
        &mut self, expected_sequence: u64, expected_epoch: u64, next: CongressPolicy,
    ) -> Result<CongressChange, Error> {
        let gate = &mut self.host.gate;
        if gate.sequence != expected_sequence || gate.authority.rights.epoch() != expected_epoch { return Err(Error::Stale); }
        validate_congress(&next)?;
        if next.generation != self.congress.generation.checked_add(1).ok_or(Error::Overflow)? { return Err(Error::Stale); }
        let mut allowed = self.congress.clone();
        allowed.generation = next.generation;
        for (member, entry) in &mut allowed.members {
            let candidate = next.members.get(member).ok_or(Error::Binding)?;
            if candidate.weight > self.congress.caps.per_member { return Err(Error::Limit); }
            entry.weight = candidate.weight;
        }
        if allowed != next { return Err(Error::Binding); }
        let sequence = gate.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let mut rights = gate.authority.rights.clone();
        if !rights.conserved() { return Err(Error::WrongState); }
        let previous_available = rights.available();
        rights.revoke_epoch()?;
        let cancelled: Vec<_> = gate.authority.attempts.iter()
            .filter(|(_, attempt)| undispatched(attempt.stage)).map(|(id, _)| *id).collect();
        for id in &cancelled {
            if gate.authority.attempts[id].stage == ActionState::Authorized { rights.abort_before_dispatch(*id)?; }
        }
        if !rights.conserved() { return Err(Error::WrongState); }
        let change = CongressChange {
            previous: self.congress.clone(), current: next.clone(), sequence, revocation_floor: rights.epoch(),
            refunded_units: rights.available().checked_sub(previous_available).ok_or(Error::WrongState)?, cancelled,
        };
        // All fallible operations precede publication. The original authority,
        // dispatched history and suspended/narrowed restrictions remain intact.
        gate.authority.rights = rights;
        for id in &change.cancelled {
            gate.authority.attempts.get_mut(id).expect("validated attempt").stage = ActionState::Cancelled;
            gate.decisions.remove(id);
        }
        gate.sequence = sequence;
        self.congress = next;
        Ok(change)
    }
}

/// Result of the existing authority's exact admission-stop transaction. This
/// contains no fleet messages, endpoint outcomes, actor restore or new rights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthorityFence {
    pub(crate) sequence: u64,
    pub(crate) previous_epoch: u64,
    pub(crate) revocation_floor: u64,
    pub(crate) cancelled: Vec<u64>,
    pub(crate) refunded_units: u64,
}

impl PolicyAuthority {
    pub(crate) fn fence_authority(
        &mut self, expected_sequence: u64, expected_epoch: u64, minimum_epoch: u64,
    ) -> Result<AuthorityFence, Error> {
        let gate = &mut self.host.gate;
        if gate.sequence != expected_sequence || gate.authority.rights.epoch() != expected_epoch {
            return Err(Error::Stale);
        }
        if minimum_epoch == 0 { return Err(Error::InvalidInput); }
        let sequence = gate.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let mut rights = gate.authority.rights.clone();
        if !rights.conserved() { return Err(Error::WrongState); }
        let available = rights.available();
        rights.revoke_epoch()?;
        rights.epoch = rights.epoch().max(minimum_epoch);
        let cancelled: Vec<_> = gate.authority.attempts.iter()
            .filter(|(_, attempt)| undispatched(attempt.stage)).map(|(id, _)| *id).collect();
        for id in &cancelled {
            if gate.authority.attempts[id].stage == ActionState::Authorized {
                rights.abort_before_dispatch(*id)?;
            }
        }
        if !rights.conserved() { return Err(Error::WrongState); }
        let result = AuthorityFence {
            sequence, previous_epoch: expected_epoch, revocation_floor: rights.epoch(), cancelled,
            refunded_units: rights.available().checked_sub(available).ok_or(Error::WrongState)?,
        };
        // Preserve every post-dispatch disposition and every external liability.
        // All Result-producing operations finish before publishing this stop.
        gate.authority.rights = rights;
        for id in &result.cancelled {
            gate.authority.attempts.get_mut(id).expect("validated attempt").stage = ActionState::Cancelled;
            gate.decisions.remove(id);
        }
        gate.sequence = sequence;
        gate.suspended = true;
        Ok(result)
    }
}
