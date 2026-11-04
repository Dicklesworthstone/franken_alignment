//! Bounded retirement of expired or revoked, never-dispatched action attempts.
//!
//! This consumes the original authority's logical state, not a second ledger.
//! Time and epoch facts have the same trust boundary as normal authorization.
//! Reports are inspection evidence, not permits, remote receipts or durability.

use super::{ActionState, ElapsedTick, FrozenAction, ReferenceAuthority};
use crate::{Error, Rights, State};

/// Both causes are retained when an attempt has expired and its epoch is old.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetiredAttempt {
    pub attempt: u64,
    pub action: FrozenAction,
    pub before: ActionState,
    pub deadline_reached: bool,
    pub epoch_revoked: bool,
    pub refunded_units: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetirementBatch {
    pub elapsed: ElapsedTick,
    pub policy_epoch: u64,
    /// Ascending attempt order, independent of proposal insertion order.
    pub retired: Vec<RetiredAttempt>,
    pub refunded_units: u64,
}

/// Stages only accounting. No issuer or live authority can be copied out.
/// Ancestor-controlled composition may preflight its journal before applying.
pub(super) struct PendingRetirement {
    rights: Rights,
    pub(super) report: RetirementBatch,
}

impl ReferenceAuthority {
    /// Cancel invalidated pre-dispatch work and release only its reservations.
    ///
    /// The complete batch is validated before any mutation. At most the
    /// authority's configured attempt bound is visited. Attempts and IDs remain
    /// historical: this releases units, not audit capacity or replay identities.
    /// Future epochs are not treated as revoked. Expiry uses now >= deadline.
    /// No clock observation means Incomplete, without partial epoch cleanup.
    /// Dispatched, unknown, irrecoverable and terminal outcomes are untouched.
    pub fn retire_invalidated(&mut self) -> Result<RetirementBatch, Error> {
        let pending = self.prepare_retirement()?;
        Ok(self.apply_retirement(pending))
    }

    pub(super) fn prepare_retirement(&self) -> Result<PendingRetirement, Error> {
        let elapsed = self.elapsed.ok_or(Error::Incomplete)?;
        if !self.rights.conserved() {
            return Err(Error::WrongState);
        }
        let policy_epoch = self.rights.epoch();
        let mut rights = self.rights.clone();
        let mut retired = Vec::new();
        let mut refunded_units = 0_u64;
        for (&id, attempt) in &self.attempts {
            if !matches!(
                attempt.stage,
                ActionState::Proposed
                    | ActionState::Prepared
                    | ActionState::Reviewing
                    | ActionState::Authorized
            ) {
                continue;
            }
            let spec = attempt.action.spec();
            let deadline_reached = elapsed >= spec.deadline;
            let epoch_revoked = spec.policy_epoch < policy_epoch;
            if !deadline_reached && !epoch_revoked {
                continue;
            }
            let refund = if attempt.stage == ActionState::Authorized {
                let reservation = rights.reservations.get(&id).ok_or(Error::Missing)?;
                if reservation.state != State::Reserved
                    || reservation.epoch != spec.policy_epoch
                    || reservation.effect != attempt.action.effect()
                {
                    return Err(Error::WrongState);
                }
                rights.abort_before_dispatch(id)?;
                spec.units
            } else {
                if rights.reservations.contains_key(&id) {
                    return Err(Error::WrongState);
                }
                0
            };
            refunded_units = refunded_units.checked_add(refund).ok_or(Error::Overflow)?;
            retired.push(RetiredAttempt {
                attempt: id,
                action: attempt.action.clone(),
                before: attempt.stage,
                deadline_reached,
                epoch_revoked,
                refunded_units: refund,
            });
        }
        if !rights.conserved()
            || rights.available().checked_sub(self.rights.available()) != Some(refunded_units)
        {
            return Err(Error::WrongState);
        }
        Ok(PendingRetirement {
            rights,
            report: RetirementBatch {
                elapsed,
                policy_epoch,
                retired,
                refunded_units,
            },
        })
    }

    // Only called synchronously on the same owner, after all fallible checks.
    pub(super) fn apply_retirement(&mut self, pending: PendingRetirement) -> RetirementBatch {
        self.rights = pending.rights;
        for row in &pending.report.retired {
            let attempt = self.attempts.get_mut(&row.attempt).expect("staged attempt");
            attempt.stage = ActionState::Cancelled;
            attempt.judgment = None;
        }
        pending.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{
        ActionSpec, MAX_ATTEMPTS, Permit, Purpose, ResolvedTarget, Scope, TrustedOutcome, VERSION,
    };
    use crate::{Judgment, Snapshot};

    fn scope() -> Scope {
        Scope {
            tenant: 1,
            principal: 2,
            run: 3,
            branch: 4,
            authority: 5,
            purpose: Purpose::Effect,
        }
    }

    fn action(units: u64, deadline: u64, epoch: u64) -> FrozenAction {
        FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: scope(),
            target: Some(ResolvedTarget {
                adapter: 1,
                object: 2,
                contract_version: 1,
                expected_version: 1,
                generation: 1,
            }),
            payload: vec![1],
            required_witnesses: vec![],
            policy_epoch: epoch,
            deadline: ElapsedTick(deadline),
            units,
        })
        .unwrap()
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            complete: true,
            ..Snapshot::default()
        }
    }

    fn authorize(owner: &mut ReferenceAuthority, id: u64, action: &FrozenAction) -> Permit {
        owner.propose(id, action.clone()).unwrap();
        owner.prepare(id).unwrap();
        owner.begin_review(id).unwrap();
        let judgment = Judgment::capture(&snapshot(), vec![]).unwrap();
        owner.authorize(id, &judgment, &snapshot()).unwrap()
    }

    fn owner(total: u64) -> ReferenceAuthority {
        let mut owner = ReferenceAuthority::new(scope(), total, MAX_ATTEMPTS).unwrap();
        owner.observe_time(ElapsedTick(0)).unwrap();
        owner
    }

    #[test]
    fn deadline_retirement_reuses_units_but_not_a_permit_or_attempt_id() {
        let mut owner = owner(10);
        let old = action(10, 100, 0);
        let permit = authorize(&mut owner, 1, &old);
        owner.observe_time(ElapsedTick(99)).unwrap();
        let before = owner.inspect();
        assert!(owner.retire_invalidated().unwrap().retired.is_empty());
        assert_eq!(owner.inspect(), before);
        owner.observe_time(ElapsedTick(100)).unwrap();
        let report = owner.retire_invalidated().unwrap();
        assert_eq!(report.refunded_units, 10);
        assert_eq!(report.retired.len(), 1);
        assert!(report.retired[0].deadline_reached);
        assert!(!report.retired[0].epoch_revoked);
        assert_eq!(report.retired[0].action, old);
        assert_eq!(report.retired[0].before, ActionState::Authorized);
        assert!(owner.attempts[&1].judgment.is_none());
        assert_eq!(
            owner.dispatch(&permit, &old, &snapshot()),
            Err(Error::WrongState)
        );
        assert_eq!(owner.propose(1, old), Err(Error::Duplicate));
        let next = action(10, 200, 0);
        let next_permit = authorize(&mut owner, 2, &next);
        owner.dispatch(&next_permit, &next, &snapshot()).unwrap();
        let before_retry = owner.inspect();
        assert_eq!(owner.retire_invalidated().unwrap().refunded_units, 0);
        assert_eq!(owner.inspect(), before_retry);
        assert_eq!(before_retry.charged, 10);
        assert!(owner.rights.conserved());
    }

    #[test]
    fn revocation_and_expiry_never_refund_dispatched_or_unknown_effects() {
        let mut owner = owner(20);
        let action = action(2, 100, 0);
        for id in 1..=7 {
            let permit = authorize(&mut owner, id, &action);
            if id >= 2 {
                owner.dispatch(&permit, &action, &snapshot()).unwrap();
            }
            if matches!(id, 3 | 4) {
                owner.mark_unknown(id).unwrap();
            }
            if id == 4 {
                owner.mark_irrecoverable(id).unwrap();
            }
            if id == 5 {
                owner
                    .record_trusted_outcome(id, TrustedOutcome::Executed)
                    .unwrap();
            }
            if id == 6 {
                owner
                    .record_trusted_outcome(id, TrustedOutcome::NotExecuted)
                    .unwrap();
            }
        }
        owner.propose(8, action.clone()).unwrap();
        owner.propose(9, action.clone()).unwrap();
        owner.prepare(9).unwrap();
        owner.propose(10, action).unwrap();
        owner.prepare(10).unwrap();
        owner.begin_review(10).unwrap();
        owner.observe_time(ElapsedTick(100)).unwrap();
        owner.revoke_epoch().unwrap();
        let before = owner.inspect();
        let report = owner.retire_invalidated().unwrap();
        assert_eq!(report.refunded_units, 2);
        assert_eq!(
            report.retired.iter().map(|r| r.attempt).collect::<Vec<_>>(),
            vec![1, 8, 9, 10]
        );
        assert!(
            report
                .retired
                .iter()
                .all(|r| r.deadline_reached && r.epoch_revoked)
        );
        let after = owner.inspect();
        assert_eq!(after.charged, before.charged);
        for id in 2..=7 {
            assert_eq!(after.stages[&id], before.stages[&id]);
        }
        assert_eq!(owner.cancel(3), Err(Error::WrongState));
        owner
            .record_trusted_outcome(3, TrustedOutcome::NotExecuted)
            .unwrap();
        assert_eq!(owner.inspect().available, after.available + 2);
        assert!(owner.rights.conserved());
    }

    #[test]
    fn future_epoch_is_not_revoked_and_missing_clock_changes_nothing() {
        let mut owner = ReferenceAuthority::new(scope(), 10, 3).unwrap();
        owner.propose(1, action(1, 100, 0)).unwrap();
        owner.propose(2, action(1, 100, 1)).unwrap();
        owner.revoke_epoch().unwrap();
        let before = owner.inspect();
        assert_eq!(owner.retire_invalidated(), Err(Error::Incomplete));
        assert_eq!(owner.inspect(), before);
        owner.observe_time(ElapsedTick(0)).unwrap();
        let report = owner.retire_invalidated().unwrap();
        assert_eq!(report.retired.len(), 1);
        assert_eq!(report.retired[0].attempt, 1);
        assert!(report.retired[0].epoch_revoked);
        assert!(!report.retired[0].deadline_reached);
        assert_eq!(owner.inspect().stages[&2], ActionState::Proposed);
        owner.propose(3, action(1, 100, 2)).unwrap();
        assert!(owner.retire_invalidated().unwrap().retired.is_empty());
        assert_eq!(owner.inspect().stages[&3], ActionState::Proposed);
        owner.revoke_epoch().unwrap();
        let report = owner.retire_invalidated().unwrap();
        assert_eq!(report.retired[0].attempt, 2);
        assert_eq!(owner.inspect().epoch, 2);
    }

    #[test]
    fn invalid_second_reservation_cannot_partially_refund_first() {
        let mut owner = owner(10);
        for id in 1..=2 {
            authorize(&mut owner, id, &action(2, 100, 0));
        }
        owner.observe_time(ElapsedTick(100)).unwrap();
        owner.rights.reservations.get_mut(&2).unwrap().state = State::Dispatched;
        let before = owner.inspect();
        let rights_before = owner.rights.clone();
        assert_eq!(owner.retire_invalidated(), Err(Error::WrongState));
        assert_eq!(owner.inspect(), before);
        assert_eq!(owner.rights, rights_before);
        assert!(owner.attempts[&1].judgment.is_some());
    }

    #[test]
    fn full_bound_is_deterministic_and_retains_history_capacity() {
        let mut owner = owner(1);
        for id in (1..=MAX_ATTEMPTS as u64).rev() {
            owner.propose(id, action(1, 1, 0)).unwrap();
        }
        owner.observe_time(ElapsedTick(1)).unwrap();
        let report = owner.retire_invalidated().unwrap();
        assert_eq!(report.retired.len(), MAX_ATTEMPTS);
        assert_eq!(report.refunded_units, 0);
        for (index, row) in report.retired.iter().enumerate() {
            assert_eq!(row.attempt, index as u64 + 1);
            assert_eq!(row.before, ActionState::Proposed);
        }
        assert_eq!(
            owner.propose(MAX_ATTEMPTS as u64 + 1, action(1, 2, 0)),
            Err(Error::Limit)
        );
        assert!(owner.retire_invalidated().unwrap().retired.is_empty());
    }

    #[test]
    fn full_u64_budget_is_conserved_without_refund_overflow() {
        let mut owner = owner(u64::MAX);
        authorize(&mut owner, 1, &action(1, 1, 0));
        authorize(&mut owner, 2, &action(u64::MAX - 1, 1, 0));
        owner.observe_time(ElapsedTick(1)).unwrap();
        assert_eq!(
            owner.retire_invalidated().unwrap().refunded_units,
            u64::MAX
        );
        assert_eq!(owner.inspect().available, u64::MAX);
        assert!(owner.rights.conserved());
    }
}
