//! Public congress-to-dispatch tests using immutable, authority-bound sessions.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::gate::containment::session::{
    BoundReview, ReviewSession, SessionSpec,
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, ContainmentAuthority, ResetRequest, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::gate::{ConsequenceAuthority, ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION,
};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Judgment, Snapshot};
use std::collections::BTreeMap;

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

fn target(object: u64) -> ResolvedTarget {
    ResolvedTarget {
        adapter: 1,
        object,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    }
}

fn action(epoch: u64, object: u64) -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: scope(),
        target: Some(target(object)),
        payload: vec![1, 2, 3],
        required_witnesses: vec![],
        policy_epoch: epoch,
        deadline: ElapsedTick(100),
        units: 4,
    })
    .unwrap()
}

fn spec(attempt: u64, round: u64) -> SessionSpec {
    SessionSpec {
        attempt,
        round,
        evidence_root: [7; 32],
        policy: CongressPolicy {
            generation: 1,
            members: BTreeMap::from([
                ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 2 }),
                ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 2 }),
            ]),
            caps: Caps { per_member: 2, per_cohort: 2 },
            continue_minimum: 3,
            continue_hold_maximum: 0,
            narrow_at: 3,
            suspend_at: 4,
            minimum_members: 2,
            minimum_cohorts: 2,
        },
        exact_disqualifier: false,
        contradiction: false,
        narrowed_targets: TargetCeiling::new(&[target(10)]).unwrap(),
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 0,
        complete: true,
        values: BTreeMap::new(),
    }
}

fn judgment() -> Judgment {
    Judgment::capture(&snapshot(), vec![]).unwrap()
}

fn setup() -> (ConsequenceAuthority, FrozenAction) {
    let mut gate = ConsequenceAuthority::new(scope(), 20, 16).unwrap();
    gate.observe_time(ElapsedTick(1)).unwrap();
    let action = action(0, 10);
    gate.propose(1, action.clone()).unwrap();
    gate.prepare(1).unwrap();
    gate.begin_review(1).unwrap();
    (gate, action)
}

fn complete(mut session: ReviewSession, verdict: Verdict) -> BoundReview {
    for member in ["alice", "bob"] {
        let commitment = session.commitment(member, verdict, member.as_bytes()).unwrap();
        session.commit(member, commitment).unwrap();
    }
    session.open_reveals().unwrap();
    for member in ["alice", "bob"] {
        session.reveal(member, verdict, member.as_bytes()).unwrap();
    }
    session.finish().unwrap()
}

#[test]
fn bound_approval_preserves_exact_action_policy_and_normal_permit_checks() {
    let (mut gate, action) = setup();
    let mut external = spec(1, 10);
    let session = ReviewSession::begin(&gate, external.clone()).unwrap();
    external.policy.generation = 99;
    external.policy.continue_minimum = 0;
    assert_eq!(session.action(), &action);
    assert_eq!(session.policy().generation, 1);
    let review = complete(session, Verdict::Allow);
    assert_eq!(review.action(), &action);
    assert_eq!(review.policy().continue_minimum, 3);
    assert_eq!(review.decision().consequence, Consequence::Continue);
    let receipt = review.apply(&mut gate).unwrap();
    assert_eq!(receipt.action, action);
    assert_eq!(receipt.binding.reducer_generation, 1);
    assert_eq!(gate.inspect().ledger.available, 20);
    let permit = gate.authorize(1, &judgment(), &snapshot()).unwrap();
    gate.dispatch(&permit, &action, &snapshot()).unwrap();
    assert_eq!(gate.dispatch(&permit, &action, &snapshot()), Err(Error::WrongState));
}

#[test]
fn completed_review_cannot_cross_identical_scope_and_action_issuers() {
    let (first, _) = setup();
    let (mut second, _) = setup();
    let first_before = first.inspect();
    let second_before = second.inspect();
    let session = ReviewSession::begin(&first, spec(1, 10)).unwrap();
    let review = complete(session, Verdict::Allow);
    assert_eq!(review.apply(&mut second), Err(Error::Binding));
    assert_eq!(first.inspect(), first_before);
    assert_eq!(second.inspect(), second_before);
}

#[test]
fn commitments_cannot_cross_actions_policies_evidence_or_restriction_contexts() {
    let (mut gate, _) = setup();
    let mut other = action(0, 10).spec().clone();
    other.payload.push(99);
    gate.propose(2, FrozenAction::freeze(other).unwrap()).unwrap();
    gate.prepare(2).unwrap();
    gate.begin_review(2).unwrap();
    for change in 0..5 {
        let original = ReviewSession::begin(&gate, spec(1, 10)).unwrap();
        let commitment = original.commitment("alice", Verdict::Allow, b"salt").unwrap();
        let mut changed = spec(1, 10);
        match change {
            0 => changed.attempt = 2,
            1 => changed.policy.generation += 1,
            2 => changed.evidence_root = [8; 32],
            3 => changed.narrowed_targets = TargetCeiling::new(&[]).unwrap(),
            _ => changed.exact_disqualifier = true,
        }
        let mut other = ReviewSession::begin(&gate, changed).unwrap();
        assert_eq!(other.commit("alice", commitment), Err(Error::Binding));
        let valid = other.commitment("alice", Verdict::Allow, b"salt").unwrap();
        other.commit("alice", valid).unwrap();
        other.open_reveals().unwrap();
        other.reveal("alice", Verdict::Allow, b"salt").unwrap();
        let result = other.finish().unwrap();
        assert_eq!(result.missing(), &["bob".to_owned()]);
        assert_ne!(result.decision().consequence, Consequence::Continue);
    }
    assert_eq!(gate.inspect().sequence, 0);
    assert_eq!(gate.inspect().ledger.available, 20);
}

#[test]
fn wrong_member_and_bad_reveal_leave_the_valid_commitment_usable() {
    let (mut gate, _) = setup();
    let mut session = ReviewSession::begin(&gate, spec(1, 10)).unwrap();
    let alice = session.commitment("alice", Verdict::Allow, b"a").unwrap();
    assert_eq!(session.commit("bob", alice), Err(Error::Binding));
    let alice = session.commitment("alice", Verdict::Allow, b"a").unwrap();
    let bob = session.commitment("bob", Verdict::Allow, b"b").unwrap();
    session.commit("alice", alice).unwrap();
    session.commit("bob", bob).unwrap();
    session.open_reveals().unwrap();
    assert_eq!(session.reveal("alice", Verdict::Deny, b"a"), Err(Error::Binding));
    session.reveal("alice", Verdict::Allow, b"a").unwrap();
    session.reveal("bob", Verdict::Allow, b"b").unwrap();
    assert_eq!(session.reveal("alice", Verdict::Allow, b"a"), Err(Error::Duplicate));
    session.finish().unwrap().apply(&mut gate).unwrap();
    gate.authorize(1, &judgment(), &snapshot()).unwrap();
}

#[test]
fn missing_reveal_holds_and_only_a_new_complete_round_releases() {
    let (mut gate, action) = setup();
    let mut session = ReviewSession::begin(&gate, spec(1, 10)).unwrap();
    let alice = session.commitment("alice", Verdict::Allow, b"a").unwrap();
    session.commit("alice", alice).unwrap();
    session.open_reveals().unwrap();
    session.reveal("alice", Verdict::Allow, b"a").unwrap();
    let review = session.finish().unwrap();
    assert_eq!(review.missing(), &["bob".to_owned()]);
    assert!(review.abstained().is_empty());
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    review.apply(&mut gate).unwrap();
    assert_eq!(gate.authorize(1, &judgment(), &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(ReviewSession::begin(&gate, spec(1, 10)).unwrap_err(), Error::Duplicate);
    let new_round = ReviewSession::begin(&gate, spec(1, 11)).unwrap();
    complete(new_round, Verdict::Allow).apply(&mut gate).unwrap();
    let permit = gate.authorize(1, &judgment(), &snapshot()).unwrap();
    gate.dispatch(&permit, &action, &snapshot()).unwrap();
}

#[test]
fn revocation_during_voting_cannot_be_bypassed_by_frozen_approval() {
    let (mut gate, _) = setup();
    let session = ReviewSession::begin(&gate, spec(1, 10)).unwrap();
    gate.revoke_epoch().unwrap();
    complete(session, Verdict::Allow).apply(&mut gate).unwrap();
    assert_eq!(gate.authorize(1, &judgment(), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(gate.inspect().ledger.available, 20);
}

#[test]
fn containment_reset_invalidates_a_completed_pre_reset_review() {
    let profile = RestartProfile {
        id: 1,
        generation: 1,
        host_generation: 1,
        model_generation: 1,
        tokenizer_generation: 1,
        state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart,
    };
    let actor = ActorState::new(profile, vec![1], vec![2], vec![3], 1).unwrap();
    let mut gate = ContainmentAuthority::new(scope(), 20, 16, actor, 3).unwrap();
    gate.observe_time(ElapsedTick(1)).unwrap();
    let checkpoint = gate.capture_checkpoint(1, 0).unwrap();
    gate.propose(1, action(0, 10)).unwrap();
    gate.prepare(1).unwrap();
    gate.begin_review(1).unwrap();
    let session = ReviewSession::begin_containment(&gate, spec(1, 10)).unwrap();
    let review = complete(session, Verdict::Allow);
    gate.reset(ResetRequest {
        checkpoint,
        expected_control_sequence: 0,
        expected_actor_revision: 0,
        binding: ReviewBinding {
            round: 99,
            evidence_root: [8; 32],
            reducer_generation: 1,
        },
        retained_targets: TargetCeiling::new(&[target(10)]).unwrap(),
    })
    .unwrap();
    assert_eq!(review.apply_to_containment(&mut gate), Err(Error::Stale));
    assert_eq!(gate.inspect().ledger.stages[&1], ActionState::Cancelled);
    let fresh = action(1, 10);
    gate.propose(2, fresh.clone()).unwrap();
    gate.prepare(2).unwrap();
    gate.begin_review(2).unwrap();
    let session = ReviewSession::begin_containment(&gate, spec(2, 100)).unwrap();
    complete(session, Verdict::Allow).apply_to_containment(&mut gate).unwrap();
    let permit = gate.authorize(2, &judgment(), &snapshot()).unwrap();
    gate.dispatch(&permit, &fresh, &snapshot()).unwrap();
}

#[test]
fn frozen_narrowing_cancels_outside_work_but_preserves_a_fresh_allowed_path() {
    let (mut gate, first) = setup();
    let outside = action(0, 20);
    gate.propose(2, outside.clone()).unwrap();
    gate.prepare(2).unwrap();
    gate.begin_review(2).unwrap();
    let other = ReviewSession::begin(&gate, spec(2, 10)).unwrap();
    complete(other, Verdict::Allow).apply(&mut gate).unwrap();
    let outside_permit = gate.authorize(2, &judgment(), &snapshot()).unwrap();
    let mut narrowed = spec(1, 11);
    narrowed.policy.suspend_at = 5;
    let session = ReviewSession::begin(&gate, narrowed).unwrap();
    let review = complete(session, Verdict::Hold);
    assert_eq!(review.decision().consequence, Consequence::NarrowAuthority);
    let receipt = review.apply(&mut gate).unwrap();
    assert_eq!(receipt.stopped, vec![2]);
    assert_eq!(receipt.refunded_units, 4);
    assert_eq!(gate.inspect().ledger.stages[&2], ActionState::Cancelled);
    assert!(gate.dispatch(&outside_permit, &outside, &snapshot()).is_err());
    let allowed = ReviewSession::begin(&gate, spec(1, 12)).unwrap();
    complete(allowed, Verdict::Allow).apply(&mut gate).unwrap();
    let permit = gate.authorize(1, &judgment(), &snapshot()).unwrap();
    gate.dispatch(&permit, &first, &snapshot()).unwrap();
}

#[test]
fn frozen_exact_disqualifier_cannot_be_outvoted_by_unanimous_approval() {
    let (mut gate, _) = setup();
    let mut exact = spec(1, 10);
    exact.exact_disqualifier = true;
    let session = ReviewSession::begin(&gate, exact).unwrap();
    let review = complete(session, Verdict::Allow);
    assert_eq!(review.tally().permit_weight, 4);
    assert_eq!(review.decision().consequence, Consequence::Deny);
    review.apply(&mut gate).unwrap();
    assert_eq!(gate.inspect().ledger.stages[&1], ActionState::Denied);
    assert_eq!(gate.authorize(1, &judgment(), &snapshot()).unwrap_err(), Error::WrongState);
}
