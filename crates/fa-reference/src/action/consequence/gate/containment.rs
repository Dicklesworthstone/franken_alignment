//! FA-108 containment reset over the existing consequence/action authority.
//!
//! This is a full-prefix, in-memory reference host. Cache and sampler bytes and
//! restart qualification are supplied by a trusted host/controller; no tensor
//! engine, authentication, durable storage or real model restart is claimed.
//! Checkpoints contain actor state, never permits, reservations or live rights.

use super::{
    ConsequenceAuthority, ControlInspection, ControlReceipt, MAX_DECISIONS, ReviewBinding,
    ReviewRequest, TargetCeiling, undispatched,
};
use crate::action::consequence::Consequence;
use crate::action::{
    ActionState, ElapsedTick, FrozenAction, Permit, Scope, TrustedOutcome,
};
use crate::{Error, Judgment, Snapshot};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_TOKENS: usize = 65_536;
pub const MAX_CACHE_BYTES: usize = 1_048_576;
pub const MAX_SAMPLER_BYTES: usize = 4_096;
pub const MAX_CHECKPOINTS: usize = 32;
pub const MAX_RETAINED_STATE_BYTES: usize = 8_388_608;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestartGrade {
    AuditOnly,
    FunctionalRestart,
    ExactRestart,
}

/// Exact caller-qualified profile identity, not a claim inferred from dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestartProfile {
    pub id: u64,
    pub generation: u64,
    pub host_generation: u64,
    pub model_generation: u64,
    pub tokenizer_generation: u64,
    pub state_schema_generation: u64,
    pub grade: RestartGrade,
}

/// Complete supplied state for this full-prefix profile. No mutable field or
/// authority-bearing object crosses the actor-state boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorState {
    profile: RestartProfile,
    tokens: Vec<u32>,
    cache: Vec<u8>,
    sampler: Vec<u8>,
    next_position: u64,
}

impl ActorState {
    pub fn new(
        profile: RestartProfile,
        tokens: Vec<u32>,
        cache: Vec<u8>,
        sampler: Vec<u8>,
        next_position: u64,
    ) -> Result<Self, Error> {
        if [
            profile.id,
            profile.generation,
            profile.host_generation,
            profile.model_generation,
            profile.tokenizer_generation,
            profile.state_schema_generation,
        ]
        .contains(&0)
        {
            return Err(Error::InvalidInput);
        }
        if tokens.len() > MAX_TOKENS
            || cache.len() > MAX_CACHE_BYTES
            || sampler.len() > MAX_SAMPLER_BYTES
        {
            return Err(Error::Limit);
        }
        if cache.is_empty() || sampler.is_empty() {
            return Err(Error::Incomplete);
        }
        if u64::try_from(tokens.len()).map_err(|_| Error::Overflow)? != next_position {
            return Err(Error::Binding);
        }
        Ok(Self {
            profile,
            tokens,
            cache,
            sampler,
            next_position,
        })
    }

    pub fn profile(&self) -> RestartProfile {
        self.profile
    }

    pub fn tokens(&self) -> &[u32] {
        &self.tokens
    }

    pub fn cache(&self) -> &[u8] {
        &self.cache
    }

    pub fn sampler(&self) -> &[u8] {
        &self.sampler
    }

    pub fn next_position(&self) -> u64 {
        self.next_position
    }

    fn retained_bytes(&self) -> usize {
        // Constructor bounds make this sum safe even on a 32-bit target.
        self.tokens.len() * 4 + self.cache.len() + self.sampler.len()
    }
}

/// Copyable reference to data, not authority. The process-local issuer brand
/// prevents a same-number checkpoint from another controller being substituted.
#[derive(Clone, Debug)]
pub struct CheckpointHandle {
    issuer: Rc<()>,
    id: u64,
}

impl CheckpointHandle {
    pub fn id(&self) -> u64 {
        self.id
    }
}

#[derive(Debug)]
struct Checkpoint {
    actor: ActorState,
    actor_revision: u64,
    control_sequence: u64,
}

#[derive(Clone, Debug)]
pub struct ResetRequest {
    pub checkpoint: CheckpointHandle,
    pub expected_control_sequence: u64,
    pub expected_actor_revision: u64,
    pub binding: ReviewBinding,
    pub retained_targets: TargetCeiling,
}

/// A logical reset record. Actor contents are not copied into the control log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetReceipt {
    pub scope: Scope,
    pub sequence: u64,
    pub binding: ReviewBinding,
    pub checkpoint: u64,
    pub checkpoint_actor_revision: u64,
    pub checkpoint_control_sequence: u64,
    pub actor_revision: u64,
    pub incident_count: u64,
    pub consequence: Consequence,
    pub restored: bool,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
    pub revocation_floor: u64,
    pub ceiling: TargetCeiling,
}

/// Trusted controller owning both the reference host and its authority domain.
/// It never exports a mutable gate, a cloned authority or a checkpoint containing
/// rights. Production purpose-typed controller authentication remains separate.
#[derive(Debug)]
pub struct ContainmentAuthority {
    gate: ConsequenceAuthority,
    actor: ActorState,
    actor_revision: u64,
    suspend_at_incident: u64,
    checkpoints: BTreeMap<u64, Checkpoint>,
    checkpoint_bytes: usize,
    resets: Vec<ResetReceipt>,
}

impl ContainmentAuthority {
    pub fn new(
        scope: Scope,
        total: u64,
        max_attempts: usize,
        actor: ActorState,
        suspend_at_incident: u64,
    ) -> Result<Self, Error> {
        if suspend_at_incident == 0 {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            gate: ConsequenceAuthority::new(scope, total, max_attempts)?,
            actor,
            actor_revision: 0,
            suspend_at_incident,
            checkpoints: BTreeMap::new(),
            checkpoint_bytes: 0,
            resets: Vec::new(),
        })
    }

    pub fn actor(&self) -> &ActorState {
        &self.actor
    }

    pub fn actor_revision(&self) -> u64 {
        self.actor_revision
    }

    pub fn incident_count(&self) -> u64 {
        self.gate.authority.rights.incident_count()
    }

    /// Trusted host update. A delayed pre-reset update cannot overwrite a rewind.
    /// Profile changes need a separate qualification transition, not this method.
    pub fn replace_actor_state(
        &mut self,
        expected_actor_revision: u64,
        actor: ActorState,
    ) -> Result<(), Error> {
        if self.gate.suspended {
            return Err(Error::WrongState);
        }
        if expected_actor_revision != self.actor_revision {
            return Err(Error::Stale);
        }
        if actor.profile != self.actor.profile {
            return Err(Error::Binding);
        }
        let revision = self.actor_revision.checked_add(1).ok_or(Error::Overflow)?;
        self.actor = actor;
        self.actor_revision = revision;
        Ok(())
    }

    pub fn capture_checkpoint(
        &mut self,
        id: u64,
        expected_actor_revision: u64,
    ) -> Result<CheckpointHandle, Error> {
        if self.gate.suspended {
            return Err(Error::WrongState);
        }
        if expected_actor_revision != self.actor_revision {
            return Err(Error::Stale);
        }
        if id == 0 {
            return Err(Error::InvalidInput);
        }
        if self.checkpoints.contains_key(&id) {
            return Err(Error::Duplicate);
        }
        if self.actor.profile.grade == RestartGrade::AuditOnly {
            return Err(Error::Incomplete);
        }
        if self.checkpoints.len() >= MAX_CHECKPOINTS {
            return Err(Error::Limit);
        }
        let bytes = self
            .checkpoint_bytes
            .checked_add(self.actor.retained_bytes())
            .ok_or(Error::Limit)?;
        if bytes > MAX_RETAINED_STATE_BYTES {
            return Err(Error::Limit);
        }
        let checkpoint = Checkpoint {
            actor: self.actor.clone(),
            actor_revision: self.actor_revision,
            control_sequence: self.gate.sequence,
        };
        self.checkpoints.insert(id, checkpoint);
        self.checkpoint_bytes = bytes;
        Ok(CheckpointHandle {
            issuer: Rc::clone(&self.gate.authority.issuer),
            id,
        })
    }

    /// Atomically rewind only actor state, fence old authority, and cancel all
    /// undispatched attempts from the abandoned continuation. Dispatched, unknown
    /// and terminal outcomes are never rewritten or refunded by the rewind.
    /// Reaching the fixed incident threshold suspends instead of restoring.
    pub fn reset(&mut self, request: ResetRequest) -> Result<ResetReceipt, Error> {
        if self.gate.suspended {
            return Err(Error::WrongState);
        }
        if request.expected_control_sequence != self.gate.sequence
            || request.expected_actor_revision != self.actor_revision
        {
            return Err(Error::Stale);
        }
        if !Rc::ptr_eq(&request.checkpoint.issuer, &self.gate.authority.issuer) {
            return Err(Error::Binding);
        }
        if request.binding.round == 0
            || request.binding.reducer_generation == 0
            || request.binding.evidence_root == [0; 32]
        {
            return Err(Error::InvalidInput);
        }
        if self.gate.seen_rounds.contains(&request.binding.round) {
            return Err(Error::Duplicate);
        }
        if self.gate.seen_rounds.len() >= MAX_DECISIONS {
            return Err(Error::Limit);
        }
        let checkpoint = self.checkpoints.get(&request.checkpoint.id).ok_or(Error::Missing)?;
        if checkpoint.actor.profile != self.actor.profile {
            return Err(Error::Binding);
        }
        if checkpoint.actor.profile.grade == RestartGrade::AuditOnly {
            return Err(Error::Incomplete);
        }
        let sequence = self.gate.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let actor_revision = self.actor_revision.checked_add(1).ok_or(Error::Overflow)?;
        let ceiling = match &self.gate.ceiling {
            Some(current) => current.intersect(&request.retained_targets),
            None => request.retained_targets,
        };
        let cancelled: Vec<u64> = self
            .gate
            .authority
            .attempts
            .iter()
            .filter_map(|(id, attempt)| undispatched(attempt.stage).then_some(*id))
            .collect();

        // Stage accounting, not a second authority. The original issuer, tokens
        // and effect history remain owned by the same gate throughout.
        let mut rights = self.gate.authority.rights.clone();
        let previous_available = rights.available();
        rights.reset_to_checkpoint(&BTreeSet::new())?;
        rights.revoke_epoch()?;
        if !rights.conserved() {
            return Err(Error::WrongState);
        }
        let refunded_units = rights
            .available()
            .checked_sub(previous_available)
            .ok_or(Error::WrongState)?;
        let incident_count = rights.incident_count();
        let suspended = incident_count >= self.suspend_at_incident;
        let restored_actor = (!suspended).then(|| checkpoint.actor.clone());
        let receipt = ResetReceipt {
            scope: self.gate.authority.scope,
            sequence,
            binding: request.binding,
            checkpoint: request.checkpoint.id,
            checkpoint_actor_revision: checkpoint.actor_revision,
            checkpoint_control_sequence: checkpoint.control_sequence,
            actor_revision,
            incident_count,
            consequence: if suspended {
                Consequence::SuspendRun
            } else {
                Consequence::ResetToCheckpoint
            },
            restored: !suspended,
            cancelled: cancelled.clone(),
            refunded_units,
            revocation_floor: rights.epoch(),
            ceiling: ceiling.clone(),
        };

        // No fallible logical operation follows publication of the staged state.
        self.gate.authority.rights = rights;
        for id in cancelled {
            self.gate.authority.attempts.get_mut(&id).expect("retained attempt").stage =
                ActionState::Cancelled;
            self.gate.decisions.insert(id, Consequence::Deny);
        }
        self.gate.sequence = sequence;
        self.gate.ceiling = Some(ceiling);
        self.gate.suspended = suspended;
        self.gate.seen_rounds.insert(request.binding.round);
        if let Some(actor) = restored_actor {
            self.actor = actor;
        }
        self.actor_revision = actor_revision;
        self.resets.push(receipt.clone());
        Ok(receipt)
    }

    pub fn inspect(&self) -> ControlInspection {
        self.gate.inspect()
    }

    pub fn reset_receipts(&self) -> &[ResetReceipt] {
        &self.resets
    }

    pub fn review_receipts(&self) -> &[ControlReceipt] {
        self.gate.receipts()
    }

    pub fn observe_time(&mut self, elapsed: ElapsedTick) -> Result<(), Error> {
        self.gate.observe_time(elapsed)
    }

    pub fn propose(&mut self, id: u64, action: FrozenAction) -> Result<(), Error> {
        self.gate.propose(id, action)
    }

    pub fn prepare(&mut self, id: u64) -> Result<(), Error> {
        self.gate.prepare(id)
    }

    pub fn begin_review(&mut self, id: u64) -> Result<(), Error> {
        self.gate.begin_review(id)
    }

    pub fn apply_review(&mut self, request: ReviewRequest) -> Result<ControlReceipt, Error> {
        if self.gate.seen_rounds.len() >= MAX_DECISIONS {
            return Err(Error::Limit);
        }
        self.gate.apply_review(request)
    }

    pub fn authorize(
        &mut self,
        id: u64,
        judgment: &Judgment,
        snapshot: &Snapshot,
    ) -> Result<Permit, Error> {
        self.gate.authorize(id, judgment, snapshot)
    }

    pub fn dispatch(
        &mut self,
        permit: &Permit,
        action: &FrozenAction,
        snapshot: &Snapshot,
    ) -> Result<(), Error> {
        self.gate.dispatch(permit, action, snapshot)
    }

    pub fn cancel(&mut self, id: u64) -> Result<(), Error> {
        self.gate.cancel(id)
    }

    pub fn deny(&mut self, id: u64) -> Result<(), Error> {
        self.gate.deny(id)
    }

    pub fn revoke_epoch(&mut self) -> Result<(), Error> {
        self.gate.revoke_epoch()
    }

    pub fn mark_unknown(&mut self, id: u64) -> Result<(), Error> {
        self.gate.mark_unknown(id)
    }

    pub fn mark_irrecoverable(&mut self, id: u64) -> Result<(), Error> {
        self.gate.mark_irrecoverable(id)
    }

    pub fn record_trusted_outcome(
        &mut self,
        id: u64,
        outcome: TrustedOutcome,
    ) -> Result<(), Error> {
        self.gate.record_trusted_outcome(id, outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> RestartProfile {
        RestartProfile {
            id: 1,
            generation: 1,
            host_generation: 1,
            model_generation: 1,
            tokenizer_generation: 1,
            state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart,
        }
    }

    #[test]
    fn full_prefix_state_requires_cache_sampler_and_exact_position() {
        assert_eq!(
            ActorState::new(profile(), vec![1], vec![], vec![2], 1),
            Err(Error::Incomplete)
        );
        assert_eq!(
            ActorState::new(profile(), vec![1], vec![2], vec![], 1),
            Err(Error::Incomplete)
        );
        assert_eq!(
            ActorState::new(profile(), vec![1], vec![2], vec![3], 2),
            Err(Error::Binding)
        );
        let state = ActorState::new(profile(), vec![1], vec![2], vec![3], 1).unwrap();
        assert_eq!(state.tokens(), &[1]);
        assert_eq!(state.cache(), &[2]);
        assert_eq!(state.sampler(), &[3]);
        assert_eq!(state.next_position(), 1);
    }

    #[test]
    fn state_bounds_admit_exact_limits_and_refuse_one_over() {
        let state = ActorState::new(
            profile(),
            vec![1; MAX_TOKENS],
            vec![2; MAX_CACHE_BYTES],
            vec![3; MAX_SAMPLER_BYTES],
            MAX_TOKENS as u64,
        )
        .unwrap();
        assert_eq!(state.tokens().len(), MAX_TOKENS);
        assert_eq!(state.cache().len(), MAX_CACHE_BYTES);
        assert_eq!(state.sampler().len(), MAX_SAMPLER_BYTES);
        for (tokens, cache, sampler) in [
            (MAX_TOKENS + 1, 1, 1),
            (1, MAX_CACHE_BYTES + 1, 1),
            (1, 1, MAX_SAMPLER_BYTES + 1),
        ] {
            assert_eq!(
                ActorState::new(
                    profile(), vec![1; tokens], vec![2; cache], vec![3; sampler], tokens as u64
                ),
                Err(Error::Limit)
            );
        }
    }
}
