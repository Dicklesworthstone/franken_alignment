//! Explicit offline credibility activation in the ORIGINAL policy authority.
//!
//! Governance, evaluator identity and the mapping of campaign positions into this
//! authority's control-sequence domain remain trusted inputs. This is not label
//! authentication, statistical calibration or durable storage. Once installed,
//! the ordinary positive paths cannot fall back to unqualified weights.

use super::{ActionState, Error, PolicyAuthority, Scope, undispatched};
use crate::action::consequence::congress::{
    CredibilityBinding, CredibilityRequirements, PromotedCongressPolicy,
};
use crate::action::consequence::congress::credibility::CredibilitySnapshot;
use crate::action::consequence::gate::containment::RestartProfile;

pub const MAX_CREDIBILITY_ACTIVATIONS: usize = 64;
/// Aggregate case/helper observations retained by this authority's history.
/// This counts logical evidence, not allocator bytes or transient caller copies.
pub const MAX_CREDIBILITY_OBSERVATIONS: u64 = 65_536;

/// An explicit trusted governance request, never accepted from actor proposals.
/// The expected predecessor is part of the idempotency binding, not rewritten
/// when a retry arrives. The campaign must use this authority's sequence domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityActivation {
    pub operation: u64,
    pub expected_control_sequence: u64,
    pub expected_epoch: u64,
    pub scope: Scope,
    pub policy_generation: u64,
    pub actor_profile: RestartProfile,
    pub binding: CredibilityBinding,
    pub stratum: String,
    pub requirements: CredibilityRequirements,
    pub snapshot: CredibilitySnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActivationContext {
    operation: u64,
    expected_control_sequence: u64,
    expected_epoch: u64,
    scope: Scope,
    policy_generation: u64,
    actor_profile: RestartProfile,
    binding: CredibilityBinding,
    stratum: String,
    requirements: CredibilityRequirements,
}

impl CredibilityActivation {
    fn context(&self) -> ActivationContext {
        ActivationContext {
            operation: self.operation,
            expected_control_sequence: self.expected_control_sequence,
            expected_epoch: self.expected_epoch,
            scope: self.scope,
            policy_generation: self.policy_generation,
            actor_profile: self.actor_profile,
            binding: self.binding.clone(),
            stratum: self.stratum.clone(),
            requirements: self.requirements.clone(),
        }
    }
}

/// Native transition evidence, not an endpoint receipt or a permit. History
/// retains its promoted snapshot separately, even after the next activation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityChange {
    pub operation: u64,
    pub sequence: u64,
    pub revocation_floor: u64,
    pub previous_reducer_generation: u64,
    pub reducer_generation: u64,
    pub policy_generation: u64,
    pub campaign: u64,
    pub valid_through: u64,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

/// Trusted notification that the admitted credibility evidence is unavailable.
/// An operation is unique within this authority's withdrawal history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityWithdrawalRequest {
    pub operation: u64,
    pub expected_control_sequence: u64,
    pub expected_epoch: u64,
}

/// Evidence-loss transition, never proof that a dispatched effect did not occur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilityWithdrawal {
    pub request: CredibilityWithdrawalRequest,
    pub sequence: u64,
    pub revocation_floor: u64,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

#[derive(Debug)]
pub(super) struct ActivationRecord {
    context: ActivationContext,
    promoted: PromotedCongressPolicy,
    observations: u64,
    receipt: CredibilityChange,
    withdrawal: Option<CredibilityWithdrawal>,
}

impl PolicyAuthority {
    /// All validation and rights accounting precede mutation. Every pending
    /// attempt is invalidated, including completed reviews and unspent permits.
    /// Dispatched/unknown effects keep their existing accounting and outcomes.
    /// No policy predicate, threshold, ceiling, actor state or suspension changes.
    pub fn activate_credibility(
        &mut self,
        request: CredibilityActivation,
    ) -> Result<CredibilityChange, Error> {
        if request.operation == 0 {
            return Err(Error::InvalidInput);
        }
        // Bound request-derived maps before cloning any caller-supplied binding.
        if request.binding.helpers.len() > crate::reducer::MAX_VOTES
            || request.binding.strata.len()
                > crate::action::consequence::congress::credibility::MAX_STRATA
            || request.stratum.len() > crate::reducer::MAX_IDENTIFIER_BYTES
            || request.binding.label_owner.len() > crate::reducer::MAX_IDENTIFIER_BYTES
            || request.binding.helpers.iter().any(|(id, profile)| {
                id.len() > crate::reducer::MAX_IDENTIFIER_BYTES
                    || profile.cohort.len() > crate::reducer::MAX_IDENTIFIER_BYTES
            })
            || request.binding.strata.iter().any(|stratum| {
                stratum.len() > crate::reducer::MAX_IDENTIFIER_BYTES
            })
        {
            return Err(Error::Limit);
        }
        let context = request.context();
        if let Some(previous) = self.credibility_history.iter()
            .find(|entry| entry.context.operation == request.operation)
        {
            return if previous.context == context && previous.promoted.snapshot() == &request.snapshot {
                Ok(previous.receipt.clone())
            } else {
                Err(Error::Binding)
            };
        }
        if self.credibility_history.len() >= MAX_CREDIBILITY_ACTIVATIONS {
            return Err(Error::Limit);
        }
        let gate = &self.host.gate;
        if gate.sequence != request.expected_control_sequence
            || gate.authority.rights.epoch() != request.expected_epoch
        {
            return Err(Error::Stale);
        }
        if gate.suspended {
            return Err(Error::WrongState);
        }
        if request.scope != gate.authority.scope
            || request.policy_generation != self.policy.generation()
            || request.actor_profile != self.host.actor().profile()
            || request.binding.scope.model_generation != request.actor_profile.model_generation
            || !request.binding.strata.contains(&request.stratum)
        {
            return Err(Error::Binding);
        }
        if let Some(previous) = self.credibility_history.last() {
            // A refresh cannot silently relax the evidence gate, drop a hard
            // stratum, change evaluator ownership or roll helper identity back.
            let old = &previous.context;
            if request.requirements != old.requirements || request.stratum != old.stratum
                || request.binding.strata != old.binding.strata
                || request.binding.label_owner != old.binding.label_owner
                || !request.binding.helpers.keys().eq(old.binding.helpers.keys())
            {
                return Err(Error::Binding);
            }
            if request.snapshot.sealed_sequence() < previous.promoted.snapshot().sealed_sequence()
                || request.snapshot.oldest_case_sequence() < previous.promoted.snapshot().oldest_case_sequence()
                || request.binding.scope.evaluator_generation < old.binding.scope.evaluator_generation
                || request.binding.helpers.iter().any(|(id, helper)| {
                    helper.generation < old.binding.helpers[id].generation
                })
            {
                return Err(Error::Stale);
            }
        }
        let observations = request.snapshot.scores().values()
            .flat_map(|strata| strata.values())
            .try_fold(0_u64, |total, score| total.checked_add(score.cases))
            .ok_or(Error::Overflow)?;
        let retained = self.credibility_history.iter()
            .try_fold(observations, |total, entry| total.checked_add(entry.observations))
            .ok_or(Error::Overflow)?;
        if retained > MAX_CREDIBILITY_OBSERVATIONS {
            return Err(Error::Limit);
        }
        let sequence = gate.sequence.checked_add(1).ok_or(Error::Overflow)?;
        // Promotion is checked at its resulting predecessor. It cannot be born
        // already expired after its own activation event advances the journal.
        let promoted = self.congress.promote_credibility(
            request.snapshot, &request.requirements, &request.binding, sequence,
        )?;
        let mut next_congress = self.congress.clone();
        next_congress.generation = request.binding.reducer_generation;
        next_congress.members = promoted.admitted_members().clone();
        next_congress.validate()?;

        let mut rights = gate.authority.rights.clone();
        if !rights.conserved() {
            return Err(Error::WrongState);
        }
        let available = rights.available();
        rights.revoke_epoch()?;
        let cancelled: Vec<_> = gate.authority.attempts.iter()
            .filter(|(_, attempt)| undispatched(attempt.stage))
            .map(|(id, _)| *id).collect();
        for id in &cancelled {
            if gate.authority.attempts[id].stage == ActionState::Authorized {
                rights.abort_before_dispatch(*id)?;
            }
        }
        if !rights.conserved() {
            return Err(Error::WrongState);
        }
        let receipt = CredibilityChange {
            operation: request.operation, sequence, revocation_floor: rights.epoch(),
            previous_reducer_generation: self.congress.generation,
            reducer_generation: next_congress.generation,
            policy_generation: self.policy.generation(),
            campaign: request.binding.scope.campaign, valid_through: promoted.valid_through(),
            cancelled, refunded_units: rights.available().checked_sub(available).ok_or(Error::WrongState)?,
        };
        let gate = &mut self.host.gate;
        gate.authority.rights = rights;
        for id in &receipt.cancelled {
            gate.authority.attempts.get_mut(id).expect("validated attempt").stage = ActionState::Cancelled;
            gate.decisions.remove(id);
        }
        gate.sequence = sequence;
        self.congress = next_congress;
        self.credibility_invalidated = false;
        self.credibility_history.push(ActivationRecord {
            context, promoted, observations, receipt: receipt.clone(), withdrawal: None,
        });
        Ok(receipt)
    }

    /// Withdraw positive use of the currently admitted evidence. The original
    /// host keeps its immutable actor-profile contract; observed evidence loss
    /// does not require accepting a substituted actor or synthesizing new labels.
    /// At most one withdrawal is retained per activation. An exact historical
    /// retry returns its receipt without withdrawing a later fresh activation.
    pub fn withdraw_credibility(
        &mut self,
        request: CredibilityWithdrawalRequest,
    ) -> Result<CredibilityWithdrawal, Error> {
        if request.operation == 0 {
            return Err(Error::InvalidInput);
        }
        if let Some(previous) = self.credibility_withdrawals()
            .find(|entry| entry.request.operation == request.operation)
        {
            return if previous.request == request {
                Ok(previous.clone())
            } else {
                Err(Error::Binding)
            };
        }
        let gate = &self.host.gate;
        if gate.sequence != request.expected_control_sequence
            || gate.authority.rights.epoch() != request.expected_epoch
        {
            return Err(Error::Stale);
        }
        let active = self.credibility_history.last().ok_or(Error::Incomplete)?;
        if active.withdrawal.is_some() {
            return Err(Error::WrongState);
        }
        let sequence = gate.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let mut rights = gate.authority.rights.clone();
        if !rights.conserved() {
            return Err(Error::WrongState);
        }
        let available = rights.available();
        rights.revoke_epoch()?;
        let cancelled: Vec<_> = gate.authority.attempts.iter()
            .filter(|(_, attempt)| undispatched(attempt.stage))
            .map(|(id, _)| *id).collect();
        for id in &cancelled {
            if gate.authority.attempts[id].stage == ActionState::Authorized {
                rights.abort_before_dispatch(*id)?;
            }
        }
        if !rights.conserved() {
            return Err(Error::WrongState);
        }
        let receipt = CredibilityWithdrawal {
            request, sequence, revocation_floor: rights.epoch(), cancelled,
            refunded_units: rights.available().checked_sub(available).ok_or(Error::WrongState)?,
        };
        let gate = &mut self.host.gate;
        gate.authority.rights = rights;
        for id in &receipt.cancelled {
            gate.authority.attempts.get_mut(id).expect("validated attempt").stage = ActionState::Cancelled;
            gate.decisions.remove(id);
        }
        gate.sequence = sequence;
        self.credibility_invalidated = true;
        self.credibility_history.last_mut().expect("validated activation").withdrawal = Some(receipt.clone());
        Ok(receipt)
    }

    pub fn credibility_withdrawals(&self) -> impl Iterator<Item = &CredibilityWithdrawal> {
        self.credibility_history.iter().filter_map(|entry| entry.withdrawal.as_ref())
    }

    pub fn credibility_changes(&self) -> impl ExactSizeIterator<Item = &CredibilityChange> {
        self.credibility_history.iter().map(|entry| &entry.receipt)
    }

    /// Historical qualification DATA remains inspectable after invalidation.
    pub fn active_credibility(&self) -> Option<&PromotedCongressPolicy> {
        self.credibility_history.last().map(|entry| &entry.promoted)
    }

    /// Read-only currentness check, also used by ordinary controller boundaries.
    /// Ok does not authorize any action. None installed retains legacy semantics;
    /// once installed, neither expiry nor identity loss can select that fallback.
    pub fn check_credibility(&self) -> Result<(), Error> {
        let Some(active) = self.credibility_history.last() else {
            return Ok(());
        };
        let context = &active.context;
        if self.credibility_invalidated || self.policy.generation() != context.policy_generation
            || self.host.actor().profile() != context.actor_profile
            || self.host.gate.sequence < active.receipt.sequence
            || self.host.gate.sequence > active.promoted.valid_through()
            || self.congress.generation != active.receipt.reducer_generation
        {
            return Err(Error::Stale);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
