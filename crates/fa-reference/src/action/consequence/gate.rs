//! Consequence-aware composition of the existing in-memory action authority.
//!
//! Trusted callers supply round identities and reduced logical facts. This is
//! not an authenticated congress, durable closure store or external broker.
//! The inner authority is never exposed: Continue still requires its normal
//! witness, epoch, deadline, scope and exact-envelope checks.

pub mod containment;

use super::{Consequence, Decision, DecisionInputs, decide};
use crate::action::{
    ActionState, ElapsedTick, FrozenAction, Inspection, MAX_ATTEMPTS, Permit, ReferenceAuthority,
    ResolvedTarget, Scope, TrustedOutcome,
};
use crate::{Error, Judgment, Snapshot, State};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub const MAX_DECISIONS: usize = 4_096;

/// Exact target identities, including adapter, object, version and generation.
/// Empty means no targets; None in the gate means the initial unrestricted
/// reference ceiling. No restoration/widening operation is exposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetCeiling(BTreeSet<[u64; 5]>);

impl TargetCeiling {
    pub fn new(targets: &[ResolvedTarget]) -> Result<Self, Error> {
        if targets.len() > MAX_ATTEMPTS {
            return Err(Error::Limit);
        }
        let mut keys = BTreeSet::new();
        for target in targets {
            let key = Self::key(*target);
            if key.contains(&0) {
                return Err(Error::InvalidInput);
            }
            if !keys.insert(key) {
                return Err(Error::Duplicate);
            }
        }
        Ok(Self(keys))
    }

    pub fn contains(&self, target: ResolvedTarget) -> bool {
        self.0.contains(&Self::key(target))
    }

    pub fn intersect(&self, other: &Self) -> Self {
        Self(self.0.intersection(&other.0).copied().collect())
    }

    fn key(target: ResolvedTarget) -> [u64; 5] {
        [
            target.adapter,
            target.object,
            target.contract_version,
            target.expected_version,
            target.generation,
        ]
    }
}

/// Logical identities only; the reference does not authenticate these bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewBinding {
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub reducer_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRequest {
    pub attempt: u64,
    pub expected_control_sequence: u64,
    pub action: FrozenAction,
    pub binding: ReviewBinding,
    pub inputs: DecisionInputs,
    pub retained_targets: Option<TargetCeiling>,
}

/// Immutable reference record, not a hashed or durably appended DecisionClosure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlReceipt {
    pub sequence: u64,
    pub attempt: u64,
    pub action: FrozenAction,
    pub binding: ReviewBinding,
    pub decision: Decision,
    pub before: ActionState,
    pub after: ActionState,
    pub stopped: Vec<u64>,
    pub refunded_units: u64,
    pub ceiling: Option<TargetCeiling>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlInspection {
    pub ledger: Inspection,
    pub sequence: u64,
    pub suspended: bool,
    pub ceiling: Option<TargetCeiling>,
    pub decisions: BTreeMap<u64, Consequence>,
}

/// Owns the existing authority rather than implementing another rights ledger.
/// There is no Deref, mutable inner accessor, conversion to the inner authority,
/// Clone, or constructor accepting an externally aliased authority.
#[derive(Debug)]
pub struct ConsequenceAuthority {
    authority: ReferenceAuthority,
    sequence: u64,
    suspended: bool,
    ceiling: Option<TargetCeiling>,
    decisions: BTreeMap<u64, Consequence>,
    seen_rounds: BTreeSet<u64>,
    receipts: Vec<ControlReceipt>,
}

impl ConsequenceAuthority {
    pub fn new(scope: Scope, total: u64, max_attempts: usize) -> Result<Self, Error> {
        Ok(Self {
            authority: ReferenceAuthority::new(scope, total, max_attempts)?,
            sequence: 0,
            suspended: false,
            ceiling: None,
            decisions: BTreeMap::new(),
            seen_rounds: BTreeSet::new(),
            receipts: Vec::new(),
        })
    }

    pub fn observe_time(&mut self, elapsed: ElapsedTick) -> Result<(), Error> {
        self.authority.observe_time(elapsed)
    }

    pub fn propose(&mut self, id: u64, action: FrozenAction) -> Result<(), Error> {
        self.check_ceiling(&action)?;
        self.authority.propose(id, action)
    }

    pub fn prepare(&mut self, id: u64) -> Result<(), Error> {
        self.check_attempt_ceiling(id)?;
        self.authority.prepare(id)
    }

    pub fn begin_review(&mut self, id: u64) -> Result<(), Error> {
        self.check_attempt_ceiling(id)?;
        self.authority.begin_review(id)
    }

    pub fn authorize(
        &mut self,
        id: u64,
        judgment: &Judgment,
        snapshot: &Snapshot,
    ) -> Result<Permit, Error> {
        self.check_continue(id)?;
        self.authority.authorize(id, judgment, snapshot)
    }

    pub fn dispatch(
        &mut self,
        permit: &Permit,
        final_action: &FrozenAction,
        snapshot: &Snapshot,
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.authority.issuer, &permit.issuer) {
            return Err(Error::Binding);
        }
        self.check_continue(permit.attempt)?;
        self.check_ceiling(final_action)?;
        self.authority.dispatch(permit, final_action, snapshot)
    }

    /// One ordered, atomic reference transition. An old round cannot be replayed
    /// under a newer predecessor. A hold preserves any already-reserved units;
    /// it never downgrades an Authorized attempt into an unreserved one.
    pub fn apply_review(&mut self, request: ReviewRequest) -> Result<ControlReceipt, Error> {
        if self.suspended {
            return Err(Error::WrongState);
        }
        if request.expected_control_sequence != self.sequence {
            return Err(Error::Stale);
        }
        if request.binding.round == 0
            || request.binding.reducer_generation == 0
            || request.binding.evidence_root == [0; 32]
        {
            return Err(Error::InvalidInput);
        }
        if self.seen_rounds.contains(&request.binding.round) {
            return Err(Error::Duplicate);
        }
        if self.receipts.len() >= MAX_DECISIONS {
            return Err(Error::Limit);
        }
        let attempt = self
            .authority
            .attempts
            .get(&request.attempt)
            .ok_or(Error::Missing)?;
        if attempt.action != request.action || request.action.spec().scope != self.authority.scope {
            return Err(Error::Binding);
        }
        let before = attempt.stage;
        let decision = decide(request.inputs);
        let run_level = matches!(
            decision.consequence,
            Consequence::NarrowAuthority | Consequence::SuspendRun
        );
        if !undispatched(before) && !run_level {
            return Err(Error::WrongState);
        }
        if decision.consequence == Consequence::Continue
            && !matches!(before, ActionState::Reviewing | ActionState::Authorized)
        {
            return Err(Error::WrongState);
        }
        if (decision.consequence == Consequence::NarrowAuthority)
            != request.retained_targets.is_some()
        {
            return Err(Error::InvalidInput);
        }
        let next_sequence = self.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let next_ceiling = match (&self.ceiling, &request.retained_targets) {
            (Some(current), Some(retained)) => Some(current.intersect(retained)),
            (None, Some(retained)) => Some(retained.clone()),
            (current, None) => current.clone(),
        };
        let suspend = decision.consequence == Consequence::SuspendRun;
        let mut stops = BTreeMap::new();
        if suspend || decision.consequence == Consequence::NarrowAuthority {
            for (id, attempt) in &self.authority.attempts {
                let outside = next_ceiling.as_ref().is_some_and(|ceiling| {
                    !attempt.action.spec().target.is_some_and(|t| ceiling.contains(t))
                });
                if undispatched(attempt.stage) && (suspend || outside) {
                    stops.insert(*id, ActionState::Cancelled);
                }
            }
        }
        if undispatched(before)
            && (request.inputs.exact_disqualifier || decision.consequence == Consequence::Deny)
        {
            // A stronger run consequence must not erase the exact attempt denial.
            // Already dispatched effects keep their real disposition, even when
            // a later exact finding triggers run-level containment.
            stops.insert(request.attempt, ActionState::Denied);
        }

        // Stage only logical accounting, not an issuer or a Permit. All fallible
        // validation finishes before publishing any state or control record.
        let mut next_rights = self.authority.rights.clone();
        if !next_rights.conserved() {
            return Err(Error::WrongState);
        }
        let previous_available = next_rights.available();
        for id in stops.keys() {
            if self.authority.attempts[id].stage == ActionState::Authorized {
                if next_rights.state(*id)? != State::Reserved {
                    return Err(Error::WrongState);
                }
                next_rights.abort_before_dispatch(*id)?;
            }
        }
        if !next_rights.conserved() {
            return Err(Error::WrongState);
        }
        let refunded_units = next_rights
            .available()
            .checked_sub(previous_available)
            .ok_or(Error::WrongState)?;
        let after = stops.get(&request.attempt).copied().unwrap_or(before);
        let receipt = ControlReceipt {
            sequence: next_sequence,
            attempt: request.attempt,
            action: request.action,
            binding: request.binding,
            decision: decision.clone(),
            before,
            after,
            stopped: stops.keys().copied().collect(),
            refunded_units,
            ceiling: next_ceiling.clone(),
        };

        self.authority.rights = next_rights;
        for (id, terminal) in stops {
            self.authority.attempts.get_mut(&id).expect("checked attempt").stage = terminal;
            self.decisions.insert(id, Consequence::Deny);
        }
        self.decisions.insert(request.attempt, decision.consequence);
        self.sequence = next_sequence;
        self.ceiling = next_ceiling;
        self.suspended = suspend;
        self.seen_rounds.insert(request.binding.round);
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }

    pub fn cancel(&mut self, id: u64) -> Result<(), Error> {
        self.authority.cancel(id)
    }

    pub fn deny(&mut self, id: u64) -> Result<(), Error> {
        self.authority.deny(id)
    }

    pub fn revoke_epoch(&mut self) -> Result<(), Error> {
        self.authority.revoke_epoch()
    }

    pub fn mark_unknown(&mut self, id: u64) -> Result<(), Error> {
        self.authority.mark_unknown(id)
    }

    pub fn mark_irrecoverable(&mut self, id: u64) -> Result<(), Error> {
        self.authority.mark_irrecoverable(id)
    }

    /// Suspension never blocks reconciliation of an effect already dispatched.
    pub fn record_trusted_outcome(
        &mut self,
        id: u64,
        outcome: TrustedOutcome,
    ) -> Result<(), Error> {
        self.authority.record_trusted_outcome(id, outcome)
    }

    pub fn inspect(&self) -> ControlInspection {
        ControlInspection {
            ledger: self.authority.inspect(),
            sequence: self.sequence,
            suspended: self.suspended,
            ceiling: self.ceiling.clone(),
            decisions: self.decisions.clone(),
        }
    }

    pub fn receipts(&self) -> &[ControlReceipt] {
        &self.receipts
    }

    fn check_ceiling(&self, action: &FrozenAction) -> Result<(), Error> {
        if self.suspended {
            return Err(Error::WrongState);
        }
        if action.spec().scope != self.authority.scope {
            return Err(Error::Binding);
        }
        if self.ceiling.as_ref().is_some_and(|ceiling| {
            !action.spec().target.is_some_and(|target| ceiling.contains(target))
        }) {
            return Err(Error::Binding);
        }
        Ok(())
    }

    fn check_attempt_ceiling(&self, id: u64) -> Result<(), Error> {
        let attempt = self.authority.attempts.get(&id).ok_or(Error::Missing)?;
        self.check_ceiling(&attempt.action)
    }

    fn check_continue(&self, id: u64) -> Result<(), Error> {
        self.check_attempt_ceiling(id)?;
        match self.decisions.get(&id) {
            Some(Consequence::Continue) => Ok(()),
            Some(_) => Err(Error::WrongState),
            None => Err(Error::Incomplete),
        }
    }
}

fn undispatched(state: ActionState) -> bool {
    matches!(
        state,
        ActionState::Proposed
            | ActionState::Prepared
            | ActionState::Reviewing
            | ActionState::Authorized
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::Restriction;
    use crate::action::{ActionSpec, Purpose, VERSION};
    use crate::ReadWitness;

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

    fn action(object: u64, units: u64) -> FrozenAction {
        FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: scope(),
            target: Some(ResolvedTarget {
                adapter: 1,
                object,
                contract_version: 1,
                expected_version: 1,
                generation: 1,
            }),
            payload: vec![1, 2],
            required_witnesses: vec![ReadWitness::Exact {
                key: 7,
                value: Some(vec![9]),
            }],
            policy_epoch: 0,
            deadline: ElapsedTick(100),
            units,
        })
        .unwrap()
    }

    fn authority() -> ConsequenceAuthority {
        let mut gate = ConsequenceAuthority::new(scope(), 20, 16).unwrap();
        gate.observe_time(ElapsedTick(1)).unwrap();
        gate
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            semantic_epoch: 0,
            complete: true,
            values: BTreeMap::from([(7, vec![9])]),
        }
    }

    fn judgment(action: &FrozenAction) -> Judgment {
        Judgment::capture(&snapshot(), action.spec().required_witnesses.clone()).unwrap()
    }

    fn begin(gate: &mut ConsequenceAuthority, id: u64, action: &FrozenAction) {
        gate.propose(id, action.clone()).unwrap();
        gate.prepare(id).unwrap();
        gate.begin_review(id).unwrap();
    }

    fn request(
        gate: &ConsequenceAuthority,
        id: u64,
        action: &FrozenAction,
        empirical: Restriction,
    ) -> ReviewRequest {
        ReviewRequest {
            attempt: id,
            expected_control_sequence: gate.sequence,
            action: action.clone(),
            binding: ReviewBinding {
                round: gate.sequence + 1,
                evidence_root: [1; 32],
                reducer_generation: 1,
            },
            inputs: DecisionInputs {
                empirical,
                exact_disqualifier: false,
                mandatory_absent: false,
                contradiction: false,
            },
            retained_targets: None,
        }
    }

    fn authorize(gate: &mut ConsequenceAuthority, id: u64, action: &FrozenAction) -> Permit {
        begin(gate, id, action);
        let review = request(gate, id, action, Restriction::Continue);
        gate.apply_review(review).unwrap();
        gate.authorize(id, &judgment(action), &snapshot()).unwrap()
    }

    fn conserved(gate: &ConsequenceAuthority) {
        let inspection = gate.inspect().ledger;
        assert_eq!(inspection.available + inspection.reserved + inspection.charged, 20);
        assert!(gate.authority.rights.conserved());
    }

    #[test]
    fn continue_is_not_a_permit_and_cannot_skip_witness_checks() {
        let mut gate = authority();
        let action = action(1, 4);
        begin(&mut gate, 1, &action);
        assert_eq!(
            gate.authorize(1, &judgment(&action), &snapshot()).unwrap_err(),
            Error::Incomplete
        );
        let review = request(&gate, 1, &action, Restriction::Continue);
        gate.apply_review(review).unwrap();
        assert_eq!(gate.inspect().ledger.available, 20);
        let mut changed = snapshot();
        changed.values.insert(7, vec![8]);
        assert_eq!(
            gate.authorize(1, &judgment(&action), &changed).unwrap_err(),
            Error::Binding
        );
        let permit = gate.authorize(1, &judgment(&action), &snapshot()).unwrap();
        assert_eq!(gate.dispatch(&permit, &action, &changed), Err(Error::Binding));
        gate.dispatch(&permit, &action, &snapshot()).unwrap();
        assert_eq!(gate.dispatch(&permit, &action, &snapshot()), Err(Error::WrongState));
        conserved(&gate);
    }

    #[test]
    fn missing_specialist_keeps_effect_held() {
        let mut gate = authority();
        let action = action(1, 4);
        begin(&mut gate, 1, &action);
        let mut review = request(&gate, 1, &action, Restriction::Continue);
        review.inputs.mandatory_absent = true;
        let receipt = gate.apply_review(review).unwrap();
        assert_eq!(receipt.decision.consequence, Consequence::HoldEffect);
        assert_eq!(receipt.after, ActionState::Reviewing);
        assert_eq!(
            gate.authorize(1, &judgment(&action), &snapshot()).unwrap_err(),
            Error::WrongState
        );
        conserved(&gate);
    }

    #[test]
    fn hold_blocks_issued_permit_without_refunding_reserved_rights() {
        let mut gate = authority();
        let action = action(1, 4);
        let permit = authorize(&mut gate, 1, &action);
        let before = gate.inspect().ledger;
        let review = request(&gate, 1, &action, Restriction::HoldEffect);
        let receipt = gate.apply_review(review).unwrap();
        assert_eq!(receipt.refunded_units, 0);
        assert_eq!(gate.inspect().ledger, before);
        assert_eq!(gate.dispatch(&permit, &action, &snapshot()), Err(Error::WrongState));
        let review = request(&gate, 1, &action, Restriction::Continue);
        gate.apply_review(review).unwrap();
        gate.dispatch(&permit, &action, &snapshot()).unwrap();
        conserved(&gate);
    }

    #[test]
    fn denied_attempt_cannot_be_resurrected_or_refunded_twice() {
        let mut gate = authority();
        let action = action(1, 4);
        let permit = authorize(&mut gate, 1, &action);
        let mut review = request(&gate, 1, &action, Restriction::Continue);
        review.inputs.exact_disqualifier = true;
        review.inputs.mandatory_absent = true;
        let receipt = gate.apply_review(review).unwrap();
        assert_eq!(receipt.after, ActionState::Denied);
        assert_eq!(receipt.refunded_units, 4);
        let before = gate.inspect();
        let review = request(&gate, 1, &action, Restriction::Continue);
        assert_eq!(gate.apply_review(review), Err(Error::WrongState));
        assert_eq!(gate.deny(1), Err(Error::WrongState));
        assert!(gate.dispatch(&permit, &action, &snapshot()).is_err());
        assert_eq!(gate.inspect(), before);
        conserved(&gate);
    }

    #[test]
    fn narrowing_intersects_and_cancels_outside_undispatched_attempts() {
        let mut gate = authority();
        let first = action(1, 4);
        let second = action(2, 5);
        let first_permit = authorize(&mut gate, 1, &first);
        let second_permit = authorize(&mut gate, 2, &second);
        let mut review = request(&gate, 1, &first, Restriction::NarrowAuthority);
        review.retained_targets =
            Some(TargetCeiling::new(&[first.spec().target.unwrap()]).unwrap());
        let receipt = gate.apply_review(review).unwrap();
        assert_eq!(receipt.stopped, vec![2]);
        assert_eq!(receipt.refunded_units, 5);
        assert_eq!(gate.inspect().ledger.stages[&2], ActionState::Cancelled);
        assert!(gate.dispatch(&second_permit, &second, &snapshot()).is_err());
        assert_eq!(gate.dispatch(&first_permit, &first, &snapshot()), Err(Error::WrongState));
        let mut review = request(&gate, 1, &first, Restriction::NarrowAuthority);
        review.retained_targets = Some(
            TargetCeiling::new(&[
                first.spec().target.unwrap(),
                second.spec().target.unwrap(),
            ])
            .unwrap(),
        );
        gate.apply_review(review).unwrap();
        assert_eq!(gate.propose(3, second), Err(Error::Binding));
        let review = request(&gate, 1, &first, Restriction::Continue);
        gate.apply_review(review).unwrap();
        gate.dispatch(&first_permit, &first, &snapshot()).unwrap();
        conserved(&gate);
    }

    #[test]
    fn suspension_preserves_dispatched_and_unknown_liabilities() {
        let mut gate = authority();
        let first = action(1, 4);
        let second = action(2, 5);
        let third = action(3, 6);
        let first_permit = authorize(&mut gate, 1, &first);
        let second_permit = authorize(&mut gate, 2, &second);
        let third_permit = authorize(&mut gate, 3, &third);
        gate.dispatch(&second_permit, &second, &snapshot()).unwrap();
        gate.dispatch(&third_permit, &third, &snapshot()).unwrap();
        gate.mark_unknown(3).unwrap();
        let mut review = request(&gate, 1, &first, Restriction::SuspendRun);
        review.inputs.exact_disqualifier = true;
        let receipt = gate.apply_review(review).unwrap();
        assert_eq!(receipt.after, ActionState::Denied);
        assert_eq!(receipt.stopped, vec![1]);
        assert_eq!(receipt.refunded_units, 4);
        let state = gate.inspect();
        assert!(state.suspended);
        assert_eq!(state.ledger.available, 9);
        assert_eq!(state.ledger.charged, 11);
        assert_eq!(state.ledger.stages[&2], ActionState::Dispatching);
        assert_eq!(state.ledger.stages[&3], ActionState::Unknown);
        assert!(gate.dispatch(&first_permit, &first, &snapshot()).is_err());
        assert!(gate.propose(4, first.clone()).is_err());
        let review = request(&gate, 1, &first, Restriction::Continue);
        assert_eq!(gate.apply_review(review), Err(Error::WrongState));
        gate.record_trusted_outcome(2, TrustedOutcome::Executed).unwrap();
        gate.record_trusted_outcome(3, TrustedOutcome::NotExecuted).unwrap();
        assert_eq!(gate.inspect().ledger.available, 15);
        assert_eq!(gate.inspect().ledger.charged, 5);
        conserved(&gate);
    }

    #[test]
    fn stale_predecessor_and_replayed_round_cannot_release_a_hold() {
        let mut gate = authority();
        let action = action(1, 4);
        begin(&mut gate, 1, &action);
        let mut old = request(&gate, 1, &action, Restriction::Continue);
        let hold = request(&gate, 1, &action, Restriction::HoldEffect);
        gate.apply_review(hold).unwrap();
        let before = gate.inspect();
        assert_eq!(gate.apply_review(old.clone()), Err(Error::Stale));
        old.expected_control_sequence = gate.sequence;
        assert_eq!(gate.apply_review(old), Err(Error::Duplicate));
        assert_eq!(gate.inspect(), before);
        assert_eq!(gate.receipts().len(), 1);
    }

    #[test]
    fn rejected_control_inputs_are_atomic() {
        let mut gate = authority();
        let action = action(1, 4);
        begin(&mut gate, 1, &action);
        let mut invalid = request(&gate, 1, &action, Restriction::Continue);
        let before = gate.inspect();
        let base = invalid.clone();
        invalid.binding.evidence_root = [0; 32];
        assert_eq!(gate.apply_review(invalid), Err(Error::InvalidInput));
        let mut invalid = base.clone();
        let mut spec = action.spec().clone();
        spec.payload.push(3);
        invalid.action = FrozenAction::freeze(spec).unwrap();
        assert_eq!(gate.apply_review(invalid), Err(Error::Binding));
        let mut invalid = base;
        invalid.inputs.empirical = Restriction::NarrowAuthority;
        assert_eq!(gate.apply_review(invalid), Err(Error::InvalidInput));
        assert_eq!(gate.inspect(), before);
        assert!(gate.receipts().is_empty());
        conserved(&gate);
    }

    #[test]
    fn continue_does_not_bypass_revocation_expiry_or_issuer_binding() {
        let mut first = authority();
        let mut second = authority();
        let action = action(1, 4);
        let permit = authorize(&mut first, 1, &action);
        let other = authorize(&mut second, 1, &action);
        assert_eq!(second.dispatch(&permit, &action, &snapshot()), Err(Error::Binding));
        second.observe_time(ElapsedTick(100)).unwrap();
        assert_eq!(second.dispatch(&other, &action, &snapshot()), Err(Error::Stale));
        first.revoke_epoch().unwrap();
        assert_eq!(first.dispatch(&permit, &action, &snapshot()), Err(Error::Stale));
        conserved(&first);
        conserved(&second);
    }

    #[test]
    fn empty_ceiling_closes_all_targets_without_synthesizing_a_reset() {
        let mut gate = authority();
        let action = action(1, 4);
        let _permit = authorize(&mut gate, 1, &action);
        let mut review = request(&gate, 1, &action, Restriction::NarrowAuthority);
        review.retained_targets = Some(TargetCeiling::new(&[]).unwrap());
        gate.apply_review(review).unwrap();
        assert_eq!(gate.inspect().ledger.available, 20);
        assert_eq!(gate.propose(2, action), Err(Error::Binding));
        assert!(!gate.inspect().suspended);
        conserved(&gate);
    }

    #[test]
    fn post_dispatch_findings_can_suspend_without_rewriting_the_effect() {
        let mut gate = authority();
        let first = action(1, 4);
        let second = action(2, 5);
        let permit = authorize(&mut gate, 1, &first);
        let _other = authorize(&mut gate, 2, &second);
        gate.dispatch(&permit, &first, &snapshot()).unwrap();
        gate.mark_unknown(1).unwrap();
        let mut review = request(&gate, 1, &first, Restriction::SuspendRun);
        review.inputs.exact_disqualifier = true;
        let receipt = gate.apply_review(review).unwrap();
        assert_eq!(receipt.before, ActionState::Unknown);
        assert_eq!(receipt.after, ActionState::Unknown);
        assert_eq!(receipt.stopped, vec![2]);
        assert_eq!(receipt.refunded_units, 5);
        assert_eq!(gate.inspect().ledger.charged, 4);
        assert!(gate.inspect().suspended);
        gate.record_trusted_outcome(1, TrustedOutcome::Executed).unwrap();
        assert_eq!(gate.inspect().ledger.stages[&1], ActionState::Confirmed);
        conserved(&gate);
    }

    #[test]
    fn post_dispatch_narrowing_changes_future_authority_not_history() {
        let mut gate = authority();
        let first = action(1, 4);
        let second = action(2, 5);
        let permit = authorize(&mut gate, 1, &first);
        let _other = authorize(&mut gate, 2, &second);
        gate.dispatch(&permit, &first, &snapshot()).unwrap();
        let mut review = request(&gate, 1, &first, Restriction::NarrowAuthority);
        review.retained_targets =
            Some(TargetCeiling::new(&[first.spec().target.unwrap()]).unwrap());
        let receipt = gate.apply_review(review).unwrap();
        assert_eq!(receipt.after, ActionState::Dispatching);
        assert_eq!(receipt.stopped, vec![2]);
        assert_eq!(receipt.refunded_units, 5);
        assert_eq!(gate.propose(3, second), Err(Error::Binding));
        let review = request(&gate, 1, &first, Restriction::Continue);
        assert_eq!(gate.apply_review(review), Err(Error::WrongState));
        conserved(&gate);
    }
}
