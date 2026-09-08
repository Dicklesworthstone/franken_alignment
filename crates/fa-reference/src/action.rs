//! FA-001 executable action/permit semantics over the existing reference ledger.
//!
//! This is an in-memory oracle, not a broker. Target resolution, clock readings,
//! snapshot completeness and trusted outcomes are caller-supplied facts. There
//! is no external effect, durability, signature, canonical wire encoding or
//! Asupersync purpose-context implementation here. Exact structural equality
//! binds an action; a process-local issuer brand prevents cross-ledger tokens.

use crate::{Effect, Error, Judgment, ReadWitness, Rights, Snapshot};
use std::collections::BTreeMap;
use std::rc::Rc;

pub const VERSION: u32 = 1;
pub const MAX_PAYLOAD_BYTES: usize = 65_536;
pub const MAX_ATTEMPTS: usize = 1_024;
pub const MAX_REQUIRED_WITNESSES: usize = 64;
pub const MAX_WITNESS_BYTES: usize = 65_536;

/// A caller-observed elapsed-clock tick, never a policy epoch or wall timestamp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ElapsedTick(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Effect,
    Experiment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub tenant: u64,
    pub principal: u64,
    pub run: u64,
    pub branch: u64,
    pub authority: u64,
    pub purpose: Purpose,
}

impl Scope {
    fn validate(self) -> Result<(), Error> {
        if [
            self.tenant,
            self.principal,
            self.run,
            self.branch,
            self.authority,
        ]
        .contains(&0)
        {
            Err(Error::InvalidInput)
        } else {
            Ok(())
        }
    }
}

/// An already resolved reference identity, not an unchecked pathname or a
/// claim that a filesystem/network adapter has actually resolved anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub adapter: u64,
    pub object: u64,
    pub contract_version: u64,
    /// Existing-resource version precondition; this profile does not model creation.
    pub expected_version: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionSpec {
    pub version: u32,
    pub scope: Scope,
    pub target: Option<ResolvedTarget>,
    pub payload: Vec<u8>,
    /// Exact declared logical dependencies, not an opaque model input view.
    pub required_witnesses: Vec<ReadWitness>,
    pub policy_epoch: u64,
    pub deadline: ElapsedTick,
    pub units: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenAction(ActionSpec);

impl FrozenAction {
    pub fn freeze(spec: ActionSpec) -> Result<Self, Error> {
        spec.scope.validate()?;
        let target = spec.target.ok_or(Error::Incomplete)?;
        if spec.version != VERSION
            || spec.units == 0
            || spec.deadline.0 == 0
            || [
                target.adapter,
                target.object,
                target.contract_version,
                target.expected_version,
                target.generation,
            ]
            .contains(&0)
        {
            return Err(Error::InvalidInput);
        }
        if spec.payload.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::Limit);
        }
        if spec.required_witnesses.len() > MAX_REQUIRED_WITNESSES {
            return Err(Error::Limit);
        }
        let mut witness_bytes = 0_usize;
        for witness in &spec.required_witnesses {
            match witness {
                ReadWitness::Exact {
                    value: Some(value), ..
                } => {
                    witness_bytes = witness_bytes.checked_add(value.len()).ok_or(Error::Limit)?;
                    if witness_bytes > MAX_WITNESS_BYTES {
                        return Err(Error::Limit);
                    }
                }
                ReadWitness::EmptyRange { start, end } if start >= end => {
                    return Err(Error::InvalidInput);
                }
                _ => {}
            }
        }
        Ok(Self(spec))
    }

    pub fn spec(&self) -> &ActionSpec {
        &self.0
    }

    // The legacy ledger checks this auxiliary representation too. It is NOT
    // the canonical binding: authorize/dispatch compare the whole frozen action.
    fn effect(&self) -> Effect {
        Effect {
            principal: self.0.scope.principal.to_string(),
            resolved_target: self
                .0
                .target
                .expect("validated frozen target")
                .object
                .to_string(),
            payload: self.0.payload.clone(),
            units: self.0.units,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionState {
    Proposed,
    Prepared,
    Reviewing,
    Authorized,
    Dispatching,
    Confirmed,
    Denied,
    Cancelled,
    Unknown,
    ConfirmedNotExecuted,
    IrrecoverablyUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustedOutcome {
    Executed,
    NotExecuted,
}

/// Inspection copies contain no permit, issuer brand or mutable ledger handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub available: u64,
    pub reserved: u64,
    pub charged: u64,
    pub epoch: u64,
    pub elapsed: Option<ElapsedTick>,
    pub stages: BTreeMap<u64, ActionState>,
}

#[derive(Debug)]
struct Attempt {
    action: FrozenAction,
    stage: ActionState,
    judgment: Option<Judgment>,
}

/// Opaque one-use reference token; only its issuing ledger can dispatch it.
/// Borrowing it twice cannot bypass the ledger's consumed state.
///
/// ```compile_fail,E0451
/// use fa_reference::action::{FrozenAction, Permit};
/// fn forge(action: FrozenAction) -> Permit {
///     Permit { issuer: std::rc::Rc::new(()), attempt: 1, action }
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::Permit;
/// fn copy_rights(permit: Permit) { let _copy = permit.clone(); }
/// ```
#[derive(Debug)]
pub struct Permit {
    issuer: Rc<()>,
    attempt: u64,
    action: FrozenAction,
}

/// Trusted bootstrap of ONE in-memory reference authority domain. Constructing
/// another instance never imports its rights or accepts its permits. This API
/// does not enforce process isolation, confer real credentials or implement Cx.
///
/// ```compile_fail,E0599
/// use fa_reference::action::ReferenceAuthority;
/// fn branch_rights(authority: ReferenceAuthority) { let _copy = authority.clone(); }
/// ```
#[derive(Debug)]
pub struct ReferenceAuthority {
    scope: Scope,
    total: u64,
    max_attempts: usize,
    elapsed: Option<ElapsedTick>,
    issuer: Rc<()>,
    rights: Rights,
    attempts: BTreeMap<u64, Attempt>,
}

impl ReferenceAuthority {
    pub fn new(scope: Scope, total: u64, max_attempts: usize) -> Result<Self, Error> {
        scope.validate()?;
        if scope.purpose != Purpose::Effect {
            return Err(Error::Binding);
        }
        if total == 0 || max_attempts == 0 || max_attempts > MAX_ATTEMPTS {
            return Err(Error::InvalidInput);
        }
        Ok(Self {
            scope,
            total,
            max_attempts,
            elapsed: None,
            issuer: Rc::new(()),
            rights: Rights::new(total),
            attempts: BTreeMap::new(),
        })
    }

    /// Record a trusted clock observation before a transition. Equality is
    /// allowed; rollback refuses. Failed transitions never roll this observation
    /// back. Until the first observation, expiry cannot be evaluated and
    /// prepare/authorization/dispatch refuse; an explicit tick zero is valid.
    pub fn observe_time(&mut self, elapsed: ElapsedTick) -> Result<(), Error> {
        if self.elapsed.is_some_and(|previous| elapsed < previous) {
            return Err(Error::Stale);
        }
        self.elapsed = Some(elapsed);
        Ok(())
    }

    pub fn propose(&mut self, id: u64, action: FrozenAction) -> Result<(), Error> {
        if id == 0 {
            return Err(Error::InvalidInput);
        }
        if self.attempts.contains_key(&id) {
            return Err(Error::Duplicate);
        }
        if self.attempts.len() >= self.max_attempts {
            return Err(Error::Limit);
        }
        self.attempts.insert(
            id,
            Attempt {
                action,
                stage: ActionState::Proposed,
                judgment: None,
            },
        );
        Ok(())
    }

    pub fn prepare(&mut self, id: u64) -> Result<(), Error> {
        let attempt = self.attempt_at(id, ActionState::Proposed)?;
        self.validate_current(&attempt.action)?;
        self.attempts.get_mut(&id).expect("checked attempt").stage = ActionState::Prepared;
        Ok(())
    }

    pub fn begin_review(&mut self, id: u64) -> Result<(), Error> {
        self.attempt_at(id, ActionState::Prepared)?;
        self.attempts.get_mut(&id).expect("checked attempt").stage = ActionState::Reviewing;
        Ok(())
    }

    /// Uses the existing exact logical read-witness model, not an opaque helper
    /// judgment. Snapshot completeness/authenticity remains a trusted input.
    pub fn authorize(
        &mut self,
        id: u64,
        judgment: &Judgment,
        snapshot: &Snapshot,
    ) -> Result<Permit, Error> {
        let attempt = self.attempt_at(id, ActionState::Reviewing)?;
        self.validate_current(&attempt.action)?;
        if judgment.witnesses != attempt.action.spec().required_witnesses {
            return Err(Error::Binding);
        }
        if !judgment.valid_at(snapshot)? {
            return Err(Error::Binding);
        }
        let action = attempt.action.clone();
        self.rights.reserve(id, action.effect())?;
        let attempt = self.attempts.get_mut(&id).expect("checked attempt");
        attempt.judgment = Some(judgment.clone());
        attempt.stage = ActionState::Authorized;
        Ok(Permit {
            issuer: Rc::clone(&self.issuer),
            attempt: id,
            action,
        })
    }

    /// The final action includes the adapter's freshly supplied resolved target
    /// and exact bytes. No irreversible external operation happens here.
    pub fn dispatch(
        &mut self,
        permit: &Permit,
        final_action: &FrozenAction,
        snapshot: &Snapshot,
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.issuer, &permit.issuer) {
            return Err(Error::Binding);
        }
        let attempt = self.attempt_at(permit.attempt, ActionState::Authorized)?;
        if attempt.action != permit.action || &attempt.action != final_action {
            return Err(Error::Binding);
        }
        self.validate_current(final_action)?;
        let judgment = attempt.judgment.as_ref().ok_or(Error::Incomplete)?;
        if !judgment.valid_at(snapshot)? {
            return Err(Error::Binding);
        }
        self.rights
            .dispatch(permit.attempt, &final_action.effect())?;
        self.attempts
            .get_mut(&permit.attempt)
            .expect("checked attempt")
            .stage = ActionState::Dispatching;
        Ok(())
    }

    pub fn cancel(&mut self, id: u64) -> Result<(), Error> {
        self.stop_before_dispatch(id, ActionState::Cancelled)
    }

    pub fn deny(&mut self, id: u64) -> Result<(), Error> {
        self.stop_before_dispatch(id, ActionState::Denied)
    }

    pub fn mark_unknown(&mut self, id: u64) -> Result<(), Error> {
        self.attempt_at(id, ActionState::Dispatching)?;
        self.rights.mark_unknown(id)?;
        self.attempts.get_mut(&id).expect("checked attempt").stage = ActionState::Unknown;
        Ok(())
    }

    /// Explicit trusted reconciliation is the only post-dispatch refund path.
    /// NotExecuted is a modeled nonexecution proof, NOT a timeout or cancellation.
    pub fn record_trusted_outcome(
        &mut self,
        id: u64,
        outcome: TrustedOutcome,
    ) -> Result<(), Error> {
        let attempt = self.attempts.get(&id).ok_or(Error::Missing)?;
        if !matches!(
            attempt.stage,
            ActionState::Dispatching | ActionState::Unknown
        ) {
            return Err(Error::WrongState);
        }
        self.rights
            .reconcile(id, outcome == TrustedOutcome::Executed)?;
        self.attempts.get_mut(&id).expect("checked attempt").stage = match outcome {
            TrustedOutcome::Executed => ActionState::Confirmed,
            TrustedOutcome::NotExecuted => ActionState::ConfirmedNotExecuted,
        };
        Ok(())
    }

    pub fn mark_irrecoverable(&mut self, id: u64) -> Result<(), Error> {
        self.attempt_at(id, ActionState::Unknown)?;
        self.attempts.get_mut(&id).expect("checked attempt").stage =
            ActionState::IrrecoverablyUnknown;
        Ok(())
    }

    pub fn revoke_epoch(&mut self) -> Result<(), Error> {
        self.rights.revoke_epoch()
    }

    pub fn inspect(&self) -> Inspection {
        let reserved = self
            .attempts
            .values()
            .filter(|a| a.stage == ActionState::Authorized)
            .map(|a| a.action.spec().units)
            .sum::<u64>();
        let available = self.rights.available();
        Inspection {
            available,
            reserved,
            charged: self.total - available - reserved,
            epoch: self.rights.epoch(),
            elapsed: self.elapsed,
            stages: self.attempts.iter().map(|(id, a)| (*id, a.stage)).collect(),
        }
    }

    fn validate_current(&self, action: &FrozenAction) -> Result<(), Error> {
        if action.spec().scope != self.scope || action.spec().scope.purpose != Purpose::Effect {
            return Err(Error::Binding);
        }
        if action.spec().policy_epoch != self.rights.epoch() {
            return Err(Error::Stale);
        }
        // The supported deadline law is strictly now < deadline.
        if self.elapsed.ok_or(Error::Incomplete)? >= action.spec().deadline {
            return Err(Error::Stale);
        }
        Ok(())
    }

    fn attempt_at(&self, id: u64, stage: ActionState) -> Result<&Attempt, Error> {
        let attempt = self.attempts.get(&id).ok_or(Error::Missing)?;
        if attempt.stage != stage {
            return Err(Error::WrongState);
        }
        Ok(attempt)
    }

    fn stop_before_dispatch(&mut self, id: u64, terminal: ActionState) -> Result<(), Error> {
        let attempt = self.attempts.get(&id).ok_or(Error::Missing)?;
        match attempt.stage {
            ActionState::Authorized => self.rights.abort_before_dispatch(id)?,
            ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing => {}
            _ => return Err(Error::WrongState),
        }
        self.attempts.get_mut(&id).expect("checked attempt").stage = terminal;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_epoch_refuses_without_restoring_an_old_floor() {
        let scope = Scope {
            tenant: 1,
            principal: 2,
            run: 3,
            branch: 4,
            authority: 5,
            purpose: Purpose::Effect,
        };
        let mut authority = ReferenceAuthority::new(scope, 10, 2).unwrap();
        // Reach the otherwise impractically distant boundary without changing
        // the transition under test. The existing Rights owns the epoch.
        authority.rights.epoch = u64::MAX;
        let before = authority.inspect();
        assert_eq!(authority.revoke_epoch(), Err(Error::Overflow));
        assert_eq!(authority.inspect(), before);
        assert!(authority.rights.conserved());
    }
}
