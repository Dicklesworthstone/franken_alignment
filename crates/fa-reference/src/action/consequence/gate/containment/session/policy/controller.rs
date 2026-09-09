//! Evidence-backed controller composing the existing containment authority.
//!
//! A proposal cannot supply its own read witnesses, a fabricated Judgment, or
//! an exact-disqualifier Boolean. The controller derives these from its owned
//! policy and supplied snapshot. Snapshots, evidence roots, governance calls
//! and host bytes remain trusted reference inputs, not authenticated providers.

mod review;
pub use review::{PolicyReceipt, PolicyReview, PolicySession};
pub use review::replay::{
    ArchivedPolicyReceipt, DecisionArchive, MAX_ARCHIVE_BYTES, ReplayedDecision, ReviewAnchor,
};

use super::{Evaluation, Policy, Truth};
use super::super::{ReviewSession, SessionSpec};
use crate::action::consequence::Consequence;
use crate::action::consequence::congress::CongressPolicy;
use crate::action::consequence::gate::{ControlInspection, TargetCeiling, undispatched};
use crate::action::consequence::gate::containment::{
    ActorState, CheckpointHandle, ContainmentAuthority, ResetReceipt, ResetRequest,
};
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Scope, TrustedOutcome};
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use crate::{Error, Judgment, Snapshot};
use std::collections::BTreeMap;
use std::rc::Rc;

pub const MAX_POLICY_CHANGES: usize = 64;

/// Trusted bootstrap, not configuration accepted from the proposed actor action.
#[derive(Debug)]
pub struct ControllerConfig {
    pub scope: Scope,
    pub total: u64,
    pub max_attempts: usize,
    pub actor: ActorState,
    pub suspend_at_incident: u64,
    pub policy: Policy,
    pub congress: CongressPolicy,
    pub narrowed_targets: TargetCeiling,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub attempt: u64,
    pub action: FrozenAction,
    pub policy: Rc<Policy>,
    pub evaluation: Evaluation,
    pub snapshot_semantic_epoch: u64,
    pub state: ActionState,
}

#[derive(Debug)]
struct Record {
    judgment: Judgment,
    policy: Rc<Policy>,
    evaluation: Evaluation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyChange {
    pub previous_generation: u64,
    pub generation: u64,
    pub previous_policy: Rc<Policy>,
    pub policy: Rc<Policy>,
    pub sequence: u64,
    pub revocation_floor: u64,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

/// Owns the host/controller, not a second ledger. There is no mutable host or
/// gate accessor, raw review application, or caller-supplied judgment interface.
/// Shared policy references contain immutable data, never authority.
#[derive(Debug)]
pub struct PolicyAuthority {
    host: ContainmentAuthority,
    policy: Rc<Policy>,
    congress: CongressPolicy,
    narrowed_targets: TargetCeiling,
    records: BTreeMap<u64, Record>,
    changes: Vec<PolicyChange>,
    reviews: Vec<PolicyReceipt>,
}

impl PolicyAuthority {
    pub fn new(config: ControllerConfig) -> Result<Self, Error> {
        validate_congress(&config.congress)?;
        Ok(Self {
            host: ContainmentAuthority::new(
                config.scope, config.total, config.max_attempts, config.actor,
                config.suspend_at_incident,
            )?,
            policy: Rc::new(config.policy),
            congress: config.congress,
            narrowed_targets: config.narrowed_targets,
            records: BTreeMap::new(),
            changes: Vec::new(),
            reviews: Vec::new(),
        })
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub fn observe_time(&mut self, tick: ElapsedTick) -> Result<(), Error> {
        self.host.gate.observe_time(tick)
    }

    /// Compile exact dependencies into the action before it enters the ledger.
    /// Incomplete evidence refuses without creating an attempt. A known policy
    /// violation creates a terminal Denied attempt without congress expenditure
    /// or reservation. Satisfied policy starts Reviewing, never Authorized.
    pub fn propose(
        &mut self,
        id: u64,
        spec: ActionSpec,
        snapshot: &Snapshot,
    ) -> Result<Proposal, Error> {
        if !spec.required_witnesses.is_empty() {
            return Err(Error::InvalidInput);
        }
        let candidate = FrozenAction::freeze(spec)?;
        self.host.gate.check_ceiling(&candidate)?;
        self.host.gate.authority.validate_current(&candidate)?;
        let evaluation = self.policy.evaluate(&candidate, snapshot)?;
        if !evaluation.complete || evaluation.trace().iter().any(|step| step.result == Truth::Unknown) {
            return Err(Error::Incomplete);
        }
        let mut spec = candidate.spec().clone();
        spec.required_witnesses = evaluation.witnesses().to_vec();
        let action = FrozenAction::freeze(spec)?;
        let judgment = Judgment::capture(snapshot, evaluation.witnesses().to_vec())?;
        let state = if evaluation.certifiable() { ActionState::Reviewing } else { ActionState::Denied };

        // All data/currentness validation above is complete before insertion.
        // The following transitions cannot fail after this successful fresh
        // proposal: no clock, epoch, scope or stage can change under &mut self.
        self.host.gate.propose(id, action.clone())?;
        if state == ActionState::Denied {
            self.host.gate.deny(id).expect("fresh undispatched proposal");
        } else {
            self.host.gate.prepare(id).expect("prevalidated current action");
            self.host.gate.begin_review(id).expect("fresh prepared action");
        }
        self.records.insert(id, Record {
            judgment, policy: Rc::clone(&self.policy), evaluation: evaluation.clone(),
        });
        Ok(Proposal {
            attempt: id, action, policy: Rc::clone(&self.policy), evaluation,
            snapshot_semantic_epoch: snapshot.semantic_epoch, state,
        })
    }

    /// Fresh exact violations become real disqualifiers, even when the proposal
    /// originally passed. Satisfied evidence must still match the frozen read
    /// set. No actor-supplied disqualifier or contradiction flag is accepted.
    pub fn begin_review(
        &self,
        id: u64,
        round: u64,
        evidence_root: [u8; 32],
        snapshot: &Snapshot,
    ) -> Result<PolicySession, Error> {
        let action = &self.host.gate.authority.attempts.get(&id).ok_or(Error::Missing)?.action;
        self.records.get(&id).ok_or(Error::Missing)?;
        let current = self.policy.evaluate(action, snapshot)?;
        if !current.complete || current.trace().iter().any(|step| step.result == Truth::Unknown) {
            return Err(Error::Incomplete);
        }
        let exact_disqualifier = current.result() == Truth::Violated;
        if !exact_disqualifier {
            self.recheck(id, snapshot)?;
        }
        let session = ReviewSession::begin_containment(&self.host, SessionSpec {
            attempt: id,
            round,
            evidence_root,
            policy: self.congress.clone(),
            exact_disqualifier,
            contradiction: false,
            narrowed_targets: self.narrowed_targets.clone(),
        })?;
        Ok(PolicySession::new(
            session, Rc::clone(&self.policy), current, snapshot.semantic_epoch,
        ))
    }

    pub fn apply_review(
        &mut self,
        review: PolicyReview,
        snapshot: &Snapshot,
    ) -> Result<PolicyReceipt, Error> {
        if !Rc::ptr_eq(&review.review.context.authority_issuer, &self.host.gate.authority.issuer) {
            return Err(Error::Binding);
        }
        if !Rc::ptr_eq(&review.policy, &self.policy) {
            return Err(Error::Stale);
        }
        review.verify_replay()?;
        if review.decision().consequence == Consequence::Continue {
            self.recheck(review.review.context.spec.attempt, snapshot)?;
        }
        // Missing or changed evidence must not obstruct a restrictive review.
        // Its frozen evaluation remains the basis; it is not replaced by the
        // proposal's earlier passing evaluation or the unavailable current one.
        let receipt = PolicyReceipt {
            control: review.review.apply_to_containment(&mut self.host)?,
            policy: review.policy,
            evaluation: review.evaluation,
            snapshot_semantic_epoch: review.snapshot_semantic_epoch,
        };
        // The underlying gate bounds successful reviews by MAX_DECISIONS.
        self.reviews.push(receipt.clone());
        Ok(receipt)
    }

    pub fn authorize(&mut self, id: u64, snapshot: &Snapshot) -> Result<Permit, Error> {
        self.recheck(id, snapshot)?;
        let judgment = &self.records.get(&id).ok_or(Error::Missing)?.judgment;
        self.host.gate.authorize(id, judgment, snapshot)
    }

    pub fn dispatch(
        &mut self,
        permit: &Permit,
        final_action: &FrozenAction,
        snapshot: &Snapshot,
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&permit.issuer, &self.host.gate.authority.issuer) {
            return Err(Error::Binding);
        }
        self.recheck(permit.attempt, snapshot)?;
        self.host.gate.dispatch(permit, final_action, snapshot)
    }

    pub fn evaluation(&self, id: u64) -> Result<&Evaluation, Error> {
        Ok(&self.records.get(&id).ok_or(Error::Missing)?.evaluation)
    }

    pub fn evaluated_policy(&self, id: u64) -> Result<&Policy, Error> {
        Ok(&self.records.get(&id).ok_or(Error::Missing)?.policy)
    }

    pub fn review_receipts(&self) -> &[PolicyReceipt] {
        &self.reviews
    }

    pub fn inspect(&self) -> ControlInspection {
        self.host.gate.inspect()
    }

    pub fn cancel(&mut self, id: u64) -> Result<(), Error> {
        self.host.gate.cancel(id)
    }

    pub fn deny(&mut self, id: u64) -> Result<(), Error> {
        self.host.gate.deny(id)
    }

    pub fn revoke_epoch(&mut self) -> Result<(), Error> {
        self.host.gate.revoke_epoch()
    }

    pub fn mark_unknown(&mut self, id: u64) -> Result<(), Error> {
        self.host.gate.mark_unknown(id)
    }

    pub fn mark_irrecoverable(&mut self, id: u64) -> Result<(), Error> {
        self.host.gate.mark_irrecoverable(id)
    }

    pub fn record_trusted_outcome(&mut self, id: u64, outcome: TrustedOutcome) -> Result<(), Error> {
        self.host.gate.record_trusted_outcome(id, outcome)
    }

    pub fn actor(&self) -> &ActorState {
        self.host.actor()
    }

    pub fn actor_revision(&self) -> u64 {
        self.host.actor_revision()
    }

    pub fn incident_count(&self) -> u64 {
        self.host.incident_count()
    }

    pub fn replace_actor_state(&mut self, expected: u64, actor: ActorState) -> Result<(), Error> {
        self.host.replace_actor_state(expected, actor)
    }

    pub fn capture_checkpoint(&mut self, id: u64, expected: u64) -> Result<CheckpointHandle, Error> {
        self.host.capture_checkpoint(id, expected)
    }

    /// The exact policy and its history are never in the actor checkpoint.
    /// Existing reset fences all old attempts; resumed work gets a new policy
    /// evaluation, bound congress round and permit under the new epoch.
    pub fn reset(&mut self, request: ResetRequest) -> Result<ResetReceipt, Error> {
        self.host.reset(request)
    }

    /// Explicit trusted governance transition. It changes no actor state or
    /// dispatched history, and cannot reopen a suspended run or widen a target
    /// ceiling. Every old undispatched attempt is cancelled, even when the new
    /// policy would still accept it. New work must use the advanced epoch.
    pub fn replace_policy(
        &mut self,
        expected_sequence: u64,
        expected_epoch: u64,
        next: Policy,
    ) -> Result<PolicyChange, Error> {
        let gate = &mut self.host.gate;
        if gate.sequence != expected_sequence || gate.authority.rights.epoch() != expected_epoch {
            return Err(Error::Stale);
        }
        if next.generation() <= self.policy.generation() {
            return Err(Error::Stale);
        }
        if self.changes.len() >= MAX_POLICY_CHANGES {
            return Err(Error::Limit);
        }
        let sequence = gate.sequence.checked_add(1).ok_or(Error::Overflow)?;
        let mut rights = gate.authority.rights.clone();
        if !rights.conserved() {
            return Err(Error::WrongState);
        }
        let previous_available = rights.available();
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
        let next = Rc::new(next);
        let change = PolicyChange {
            previous_generation: self.policy.generation(),
            generation: next.generation(),
            previous_policy: Rc::clone(&self.policy),
            policy: Rc::clone(&next),
            sequence,
            revocation_floor: rights.epoch(),
            cancelled,
            refunded_units: rights.available().checked_sub(previous_available).ok_or(Error::WrongState)?,
        };
        gate.authority.rights = rights;
        for id in &change.cancelled {
            gate.authority.attempts.get_mut(id).expect("validated attempt").stage = ActionState::Cancelled;
            gate.decisions.remove(id);
        }
        gate.sequence = sequence;
        self.policy = next;
        self.changes.push(change.clone());
        Ok(change)
    }

    pub fn policy_changes(&self) -> &[PolicyChange] {
        &self.changes
    }

    fn recheck(&self, id: u64, snapshot: &Snapshot) -> Result<(), Error> {
        let record = self.records.get(&id).ok_or(Error::Missing)?;
        if !Rc::ptr_eq(&record.policy, &self.policy) {
            return Err(Error::Stale);
        }
        let action = &self.host.gate.authority.attempts.get(&id).ok_or(Error::Missing)?.action;
        self.host.gate.check_ceiling(action)?;
        self.host.gate.authority.validate_current(action)?;
        let current = self.policy.evaluate(action, snapshot)?;
        if !current.complete || current.trace().iter().any(|step| step.result == Truth::Unknown) {
            return Err(Error::Incomplete);
        }
        if !current.certifiable() || !record.judgment.valid_at(snapshot)?
            || current.witnesses() != action.spec().required_witnesses.as_slice()
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

fn validate_congress(policy: &CongressPolicy) -> Result<(), Error> {
    if policy.members.len() > MAX_VOTES {
        return Err(Error::Limit);
    }
    if policy.generation == 0 || policy.members.is_empty()
        || policy.caps.per_member == 0 || policy.caps.per_cohort == 0
        || policy.continue_minimum == 0 || policy.continue_hold_maximum >= policy.narrow_at
        || policy.narrow_at >= policy.suspend_at || policy.minimum_members == 0
        || policy.minimum_cohorts == 0 || policy.minimum_members > policy.members.len()
        || policy.minimum_cohorts > policy.minimum_members
    {
        return Err(Error::InvalidInput);
    }
    for (member, profile) in &policy.members {
        if member.is_empty() || profile.cohort.is_empty() || profile.weight == 0 {
            return Err(Error::InvalidInput);
        }
        if member.len() > MAX_IDENTIFIER_BYTES || profile.cohort.len() > MAX_IDENTIFIER_BYTES {
            return Err(Error::Limit);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Predicate;
    use crate::action::consequence::congress::MemberPolicy;
    use crate::action::consequence::gate::containment::{RestartGrade, RestartProfile};
    use crate::action::{Purpose, ResolvedTarget, VERSION};
    use crate::reducer::Caps;
    use crate::round::Verdict;

    fn actor() -> ActorState {
        ActorState::new(RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::ExactRestart,
        }, vec![1], vec![2], vec![3], 1).unwrap()
    }

    fn spec(epoch: u64) -> ActionSpec {
        ActionSpec {
            version: VERSION,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
            payload: b"publish".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: epoch, deadline: ElapsedTick(100), units: 4,
        }
    }

    fn policy(generation: u64) -> Policy {
        Policy::new(generation, vec![
            Predicate::ExactValue { key: 7, value: vec![9] }, Predicate::Absent { key: 8 },
            Predicate::EmptyRange { start: 10, end: 20 }, Predicate::All(vec![0, 1, 2]),
        ]).unwrap()
    }

    fn snapshot() -> Snapshot {
        Snapshot { semantic_epoch: 3, complete: true, values: BTreeMap::from([(7, vec![9])]) }
    }

    fn controller() -> PolicyAuthority {
        let congress = CongressPolicy {
            generation: 1,
            members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1,
        };
        let mut controller = PolicyAuthority::new(ControllerConfig {
            scope: spec(0).scope, total: 20, max_attempts: 32, actor: actor(),
            suspend_at_incident: 3, policy: policy(1), congress,
            narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap(),
        }).unwrap();
        controller.observe_time(ElapsedTick(1)).unwrap();
        controller
    }

    fn finish(mut session: PolicySession) -> PolicyReview {
        let commitment = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
        session.commit("helper", commitment).unwrap();
        session.open_reveals().unwrap();
        session.reveal("helper", Verdict::Allow, b"salt").unwrap();
        session.finish().unwrap()
    }

    fn authorize(controller: &mut PolicyAuthority, id: u64, round: u64) -> (FrozenAction, Permit) {
        let proposal = controller.propose(id, spec(controller.inspect().ledger.epoch), &snapshot()).unwrap();
        let session = controller.begin_review(id, round, [1; 32], &snapshot()).unwrap();
        controller.apply_review(finish(session), &snapshot()).unwrap();
        let permit = controller.authorize(id, &snapshot()).unwrap();
        (proposal.action, permit)
    }

    #[test]
    fn complete_policy_round_permit_dispatch_path_uses_derived_witnesses() {
        let mut controller = controller();
        let (action, permit) = authorize(&mut controller, 1, 11);
        assert_eq!(action.spec().required_witnesses.len(), 3);
        assert!(controller.evaluation(1).unwrap().certifiable());
        let mut unrelated = snapshot();
        unrelated.values.insert(21, vec![1]);
        controller.dispatch(&permit, &action, &unrelated).unwrap();
        assert_eq!(controller.inspect().ledger.charged, 4);
        assert_eq!(controller.dispatch(&permit, &action, &unrelated), Err(Error::WrongState));
    }

    #[test]
    fn violated_policy_is_denied_before_voting_and_unknown_creates_no_attempt() {
        let mut controller = controller();
        let mut missing = snapshot();
        missing.complete = false;
        let before = controller.inspect();
        assert_eq!(controller.propose(1, spec(0), &missing), Err(Error::Incomplete));
        assert_eq!(controller.inspect(), before);
        let mut violated = snapshot();
        violated.values.insert(8, vec![1]);
        let proposal = controller.propose(1, spec(0), &violated).unwrap();
        assert_eq!(proposal.state, ActionState::Denied);
        assert_eq!(proposal.evaluation.result(), Truth::Violated);
        assert_eq!(controller.inspect().ledger.available, 20);
        assert!(controller.authorize(1, &snapshot()).is_err());
        let mut supplied = spec(0);
        supplied.required_witnesses = vec![crate::ReadWitness::Exact { key: 100, value: None }];
        assert_eq!(controller.propose(2, supplied, &snapshot()), Err(Error::InvalidInput));
        assert!(!controller.inspect().ledger.stages.contains_key(&2));
    }

    #[test]
    fn changed_reads_block_every_positive_boundary() {
        for key in [7, 8, 15] {
            let mut controller = controller();
            controller.propose(1, spec(0), &snapshot()).unwrap();
            let session = controller.begin_review(1, 11, [1; 32], &snapshot()).unwrap();
            let mut changed = snapshot();
            changed.values.insert(key, vec![8]);
            let before = controller.inspect();
            assert_eq!(controller.apply_review(finish(session), &changed), Err(Error::Binding));
            assert_eq!(controller.inspect(), before);
            let session = controller.begin_review(1, 12, [1; 32], &snapshot()).unwrap();
            controller.apply_review(finish(session), &snapshot()).unwrap();
            assert_eq!(controller.authorize(1, &changed).unwrap_err(), Error::Binding);
            let permit = controller.authorize(1, &snapshot()).unwrap();
            let action = controller.host.gate.authority.attempts[&1].action.clone();
            assert_eq!(controller.dispatch(&permit, &action, &changed), Err(Error::Binding));
            assert_eq!(controller.inspect().ledger.reserved, 4);
            controller.dispatch(&permit, &action, &snapshot()).unwrap();
        }
    }

    #[test]
    fn newly_observed_violation_supplies_disqualifier_and_retains_its_own_basis() {
        let mut controller = controller();
        let proposal = controller.propose(1, spec(0), &snapshot()).unwrap();
        let mut changed = snapshot();
        changed.values.insert(8, vec![1]);
        let session = controller.begin_review(1, 11, [1; 32], &changed).unwrap();
        let review = finish(session);
        assert_eq!(review.decision().consequence, Consequence::Deny);
        let mut unavailable = snapshot();
        unavailable.complete = false;
        let receipt = controller.apply_review(review, &unavailable).unwrap();
        assert_eq!(controller.inspect().ledger.stages[&1], ActionState::Denied);
        assert!(proposal.evaluation.certifiable());
        assert_eq!(receipt.evaluation.result(), Truth::Violated);
        assert_eq!(receipt.snapshot_semantic_epoch, 3);
        assert!(receipt.evaluation.witnesses().contains(&crate::ReadWitness::Exact {
            key: 8, value: Some(vec![1]),
        }));
        let before = controller.inspect();
        controller.replace_policy(before.sequence, before.ledger.epoch, policy(2)).unwrap();
        assert_eq!(controller.policy().generation(), 2);
        assert_eq!(controller.evaluated_policy(1).unwrap().generation(), 1);
        assert_eq!(receipt.policy.generation(), 1);
        assert_eq!(controller.review_receipts(), &[receipt]);
    }

    #[test]
    fn foreign_reviews_permits_and_action_substitution_are_refused() {
        let mut first = controller();
        let mut second = controller();
        let (action, permit) = authorize(&mut first, 1, 11);
        second.propose(1, spec(0), &snapshot()).unwrap();
        let review = finish(second.begin_review(1, 12, [1; 32], &snapshot()).unwrap());
        assert_eq!(first.apply_review(review, &snapshot()), Err(Error::Binding));
        assert_eq!(second.dispatch(&permit, &action, &snapshot()), Err(Error::Binding));
        let mut replacement = action.spec().clone();
        replacement.payload.push(1);
        let replacement = FrozenAction::freeze(replacement).unwrap();
        assert_eq!(first.dispatch(&permit, &replacement, &snapshot()), Err(Error::Binding));
        first.dispatch(&permit, &action, &snapshot()).unwrap();
    }

    #[test]
    fn policy_change_cancels_pending_work_and_preserves_unknown_liabilities() {
        let mut controller = controller();
        let (first, pending) = authorize(&mut controller, 1, 11);
        let (second, dispatched) = authorize(&mut controller, 2, 12);
        controller.dispatch(&dispatched, &second, &snapshot()).unwrap();
        controller.mark_unknown(2).unwrap();
        let before = controller.inspect();
        assert_eq!(controller.replace_policy(before.sequence + 1, before.ledger.epoch, policy(2)), Err(Error::Stale));
        assert_eq!(controller.inspect(), before);
        let change = controller.replace_policy(before.sequence, before.ledger.epoch, policy(2)).unwrap();
        assert_eq!(change.cancelled, vec![1]);
        assert_eq!(change.refunded_units, 4);
        assert_eq!(change.revocation_floor, 1);
        assert_eq!(change.previous_policy.generation(), 1);
        assert_eq!(change.policy.generation(), 2);
        assert_eq!(controller.inspect().ledger.available, 16);
        assert_eq!(controller.inspect().ledger.charged, 4);
        assert_eq!(controller.inspect().ledger.stages[&2], ActionState::Unknown);
        assert_eq!(controller.dispatch(&pending, &first, &snapshot()), Err(Error::Stale));
        let (fresh, permit) = authorize(&mut controller, 3, 13);
        assert_eq!(fresh.spec().policy_epoch, 1);
        controller.dispatch(&permit, &fresh, &snapshot()).unwrap();
        controller.record_trusted_outcome(2, TrustedOutcome::NotExecuted).unwrap();
        assert_eq!(controller.inspect().ledger.available, 16);
        assert_eq!(controller.inspect().ledger.charged, 4);
    }

    #[test]
    fn reset_rechecks_new_work_and_never_rolls_policy_back() {
        let mut controller = controller();
        let checkpoint = controller.capture_checkpoint(1, 0).unwrap();
        let (old_action, old_permit) = authorize(&mut controller, 1, 11);
        let before = controller.inspect();
        controller.replace_policy(before.sequence, before.ledger.epoch, policy(2)).unwrap();
        let request = ResetRequest {
            checkpoint,
            expected_control_sequence: controller.inspect().sequence,
            expected_actor_revision: controller.actor_revision(),
            binding: crate::action::consequence::gate::ReviewBinding {
                round: 12, evidence_root: [2; 32], reducer_generation: 1,
            },
            retained_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap(),
        };
        let reset = controller.reset(request).unwrap();
        assert!(reset.restored);
        assert_eq!(controller.policy().generation(), 2);
        assert_eq!(controller.incident_count(), 1);
        assert!(controller.dispatch(&old_permit, &old_action, &snapshot()).is_err());
        let (fresh, permit) = authorize(&mut controller, 2, 13);
        assert_eq!(fresh.spec().policy_epoch, 2);
        controller.dispatch(&permit, &fresh, &snapshot()).unwrap();
    }

    #[test]
    fn overflow_during_policy_change_is_atomic() {
        let mut controller = controller();
        controller.host.gate.authority.rights.epoch = u64::MAX;
        let before = controller.inspect();
        assert_eq!(controller.replace_policy(before.sequence, u64::MAX, policy(2)), Err(Error::Overflow));
        assert_eq!(controller.inspect(), before);
        assert_eq!(controller.policy().generation(), 1);
        assert!(controller.policy_changes().is_empty());
    }

    #[test]
    fn a_completed_old_policy_review_cannot_survive_policy_rotation() {
        let mut controller = controller();
        controller.propose(1, spec(0), &snapshot()).unwrap();
        let review = finish(controller.begin_review(1, 11, [1; 32], &snapshot()).unwrap());
        controller.replace_policy(0, 0, policy(2)).unwrap();
        let before = controller.inspect();
        assert_eq!(controller.apply_review(review, &snapshot()), Err(Error::Stale));
        assert_eq!(controller.inspect(), before);
        assert!(controller.review_receipts().is_empty());
    }
}
