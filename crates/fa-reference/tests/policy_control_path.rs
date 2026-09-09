//! Public-API campaigns for exact policies composed with congress and containment.
//! These exercise only the in-memory reference host, not a publication adapter.

use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{
    ActorState, ResetRequest, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::gate::containment::session::policy::{
    Policy, Predicate, Truth,
};
use fa_reference::action::consequence::gate::containment::session::policy::controller::{
    ControllerConfig, MAX_POLICY_CHANGES, PolicyAuthority, PolicyReview, PolicySession,
};
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope,
    TrustedOutcome, VERSION,
};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, ReadWitness, Snapshot};
use std::collections::BTreeMap;

fn target() -> ResolvedTarget {
    ResolvedTarget {
        adapter: 1,
        object: 2,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    }
}

fn action(epoch: u64) -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: Scope {
            tenant: 1,
            principal: 2,
            run: 3,
            branch: 4,
            authority: 5,
            purpose: Purpose::Effect,
        },
        target: Some(target()),
        payload: b"publish:reviewed-artifact".to_vec(),
        required_witnesses: Vec::new(),
        policy_epoch: epoch,
        deadline: ElapsedTick(100),
        units: 5,
    }
}

fn policy(generation: u64) -> Policy {
    Policy::new(generation, vec![
        Predicate::TargetIs(target()),
        Predicate::PayloadIs(b"publish:reviewed-artifact".to_vec()),
        Predicate::UnitsAtMost(5),
        Predicate::ExactValue { key: 100, value: b"approved".to_vec() },
        Predicate::Absent { key: 200 },
        Predicate::EmptyRange { start: 300, end: 400 },
        Predicate::All((0..6).collect()),
    ]).unwrap()
}

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 10,
        complete: true,
        values: BTreeMap::from([(100, b"approved".to_vec())]),
    }
}

fn controller() -> PolicyAuthority {
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
    let congress = CongressPolicy {
        generation: 1,
        members: BTreeMap::from([
            ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 }),
            ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 1 }),
        ]),
        caps: Caps { per_member: 1, per_cohort: 1 },
        continue_minimum: 2,
        continue_hold_maximum: 0,
        narrow_at: 1,
        suspend_at: 2,
        minimum_members: 2,
        minimum_cohorts: 2,
    };
    let mut controller = PolicyAuthority::new(ControllerConfig {
        scope: action(0).scope,
        total: 100,
        max_attempts: 32,
        actor,
        suspend_at_incident: 2,
        policy: policy(1),
        congress,
        narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }).unwrap();
    controller.observe_time(ElapsedTick(1)).unwrap();
    controller
}

fn finish(mut session: PolicySession, bob: Option<Verdict>) -> PolicyReview {
    for (member, verdict) in [("alice", Verdict::Allow), ("bob", bob.unwrap_or(Verdict::Allow))] {
        let commitment = session.commitment(member, verdict, b"reference-only-salt").unwrap();
        session.commit(member, commitment).unwrap();
    }
    session.open_reveals().unwrap();
    session.reveal("alice", Verdict::Allow, b"reference-only-salt").unwrap();
    if let Some(verdict) = bob {
        session.reveal("bob", verdict, b"reference-only-salt").unwrap();
    }
    session.finish().unwrap()
}

fn approve(controller: &mut PolicyAuthority, id: u64, round: u64) {
    let session = controller.begin_review(id, round, [1; 32], &snapshot()).unwrap();
    let review = finish(session, Some(Verdict::Allow));
    assert_eq!(review.decision().consequence, Consequence::Continue);
    let receipt = controller.apply_review(review, &snapshot()).unwrap();
    assert!(receipt.evaluation.certifiable());
    assert_eq!(receipt.control.binding.round, round);
    assert_eq!(receipt.snapshot_semantic_epoch, 10);
}

fn conserved(controller: &PolicyAuthority) {
    let ledger = controller.inspect().ledger;
    assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
}

#[test]
fn public_policy_path_dispatches_only_after_independent_complete_review() {
    let mut controller = controller();
    let proposal = controller.propose(1, action(0), &snapshot()).unwrap();
    assert_eq!(proposal.state, ActionState::Reviewing);
    assert_eq!(proposal.action.spec().required_witnesses, vec![
        ReadWitness::Exact { key: 100, value: Some(b"approved".to_vec()) },
        ReadWitness::Exact { key: 200, value: None },
        ReadWitness::EmptyRange { start: 300, end: 400 },
    ]);
    assert_eq!(controller.authorize(1, &snapshot()).unwrap_err(), Error::Incomplete);
    let session = controller.begin_review(1, 1, [1; 32], &snapshot()).unwrap();
    let review = finish(session, None);
    assert_eq!(review.missing(), &["bob".to_owned()]);
    let receipt = controller.apply_review(review, &snapshot()).unwrap();
    assert_eq!(receipt.control.decision.consequence, Consequence::HoldEffect);
    assert_eq!(controller.authorize(1, &snapshot()).unwrap_err(), Error::WrongState);
    approve(&mut controller, 1, 2);
    let permit = controller.authorize(1, &snapshot()).unwrap();
    let mut unrelated = snapshot();
    unrelated.values.insert(400, b"outside half-open interval".to_vec());
    controller.dispatch(&permit, &proposal.action, &unrelated).unwrap();
    controller.record_trusted_outcome(1, TrustedOutcome::Executed).unwrap();
    assert_eq!(controller.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(controller.inspect().ledger.charged, 5);
    conserved(&controller);
}

#[test]
fn exact_action_and_state_violations_have_independent_causal_controls() {
    let mut controller = controller();
    for case in 0..6 {
        let mut spec = action(0);
        let mut state = snapshot();
        match case {
            0 => spec.target.as_mut().unwrap().generation += 1,
            1 => spec.payload.push(0),
            2 => spec.units += 1,
            3 => { state.values.insert(100, b"not-approved".to_vec()); }
            4 => { state.values.insert(200, b"revoked".to_vec()); }
            5 => { state.values.insert(350, b"new-route".to_vec()); }
            _ => unreachable!(),
        }
        let rejected = controller.propose(case + 1, spec, &state).unwrap();
        assert_eq!(rejected.state, ActionState::Denied);
        assert_eq!(rejected.evaluation.trace()[case as usize].result, Truth::Violated);
        assert_eq!(controller.inspect().ledger.available, 100);
        conserved(&controller);
    }
    let permitted = controller.propose(7, action(0), &snapshot()).unwrap();
    assert_eq!(permitted.state, ActionState::Reviewing);
    approve(&mut controller, 7, 1);
    let permit = controller.authorize(7, &snapshot()).unwrap();
    controller.dispatch(&permit, &permitted.action, &snapshot()).unwrap();
    conserved(&controller);
}

#[test]
fn loss_of_coverage_or_changed_semantics_blocks_an_already_issued_permit() {
    let mut controller = controller();
    let proposal = controller.propose(1, action(0), &snapshot()).unwrap();
    approve(&mut controller, 1, 1);
    let permit = controller.authorize(1, &snapshot()).unwrap();
    let before = controller.inspect();
    let mut unavailable = snapshot();
    unavailable.complete = false;
    assert_eq!(controller.dispatch(&permit, &proposal.action, &unavailable), Err(Error::Incomplete));
    let mut other_semantics = snapshot();
    other_semantics.semantic_epoch += 1;
    assert_eq!(controller.dispatch(&permit, &proposal.action, &other_semantics), Err(Error::Binding));
    assert_eq!(controller.inspect(), before);
    controller.observe_time(ElapsedTick(100)).unwrap();
    assert_eq!(controller.dispatch(&permit, &proposal.action, &snapshot()), Err(Error::Stale));
    assert_eq!(controller.inspect().ledger.reserved, 5);
    controller.cancel(1).unwrap();
    assert_eq!(controller.inspect().ledger.available, 100);
}

#[test]
fn new_disqualifier_keeps_original_and_later_evidence_distinct() {
    let mut controller = controller();
    let proposal = controller.propose(1, action(0), &snapshot()).unwrap();
    let mut violated = snapshot();
    violated.values.insert(200, b"revoked".to_vec());
    let session = controller.begin_review(1, 1, [2; 32], &violated).unwrap();
    assert_eq!(session.evaluation().result(), Truth::Violated);
    let review = finish(session, Some(Verdict::Allow));
    assert_eq!(review.tally().permit_weight, 2);
    assert_eq!(review.decision().consequence, Consequence::Deny);
    let mut unavailable = snapshot();
    unavailable.complete = false;
    let receipt = controller.apply_review(review, &unavailable).unwrap();
    assert!(proposal.evaluation.certifiable());
    assert_eq!(receipt.evaluation.result(), Truth::Violated);
    assert!(receipt.evaluation.witnesses().contains(&ReadWitness::Exact {
        key: 200, value: Some(b"revoked".to_vec()),
    }));
    let state = controller.inspect();
    controller.replace_policy(state.sequence, state.ledger.epoch, policy(2)).unwrap();
    assert_eq!(controller.review_receipts()[0], receipt);
    assert_eq!(receipt.policy.generation(), 1);
    assert_eq!(controller.policy().generation(), 2);
    assert_eq!(controller.evaluated_policy(1).unwrap().nodes(), proposal.policy.nodes());
}

#[test]
fn held_reservations_and_unknown_effects_survive_policy_and_actor_changes_correctly() {
    let mut controller = controller();
    let checkpoint = controller.capture_checkpoint(1, 0).unwrap();
    let first = controller.propose(1, action(0), &snapshot()).unwrap();
    approve(&mut controller, 1, 1);
    let pending = controller.authorize(1, &snapshot()).unwrap();
    let hold = finish(controller.begin_review(1, 2, [1; 32], &snapshot()).unwrap(), None);
    controller.apply_review(hold, &snapshot()).unwrap();
    assert_eq!(controller.inspect().ledger.reserved, 5);
    let second = controller.propose(2, action(0), &snapshot()).unwrap();
    approve(&mut controller, 2, 3);
    let dispatched = controller.authorize(2, &snapshot()).unwrap();
    controller.dispatch(&dispatched, &second.action, &snapshot()).unwrap();
    controller.mark_unknown(2).unwrap();
    let state = controller.inspect();
    let change = controller.replace_policy(state.sequence, state.ledger.epoch, policy(2)).unwrap();
    assert_eq!(change.cancelled, vec![1]);
    assert_eq!(change.refunded_units, 5);
    assert_eq!(controller.inspect().ledger.stages[&2], ActionState::Unknown);
    let receipt = controller.reset(ResetRequest {
        checkpoint,
        expected_control_sequence: controller.inspect().sequence,
        expected_actor_revision: controller.actor_revision(),
        binding: ReviewBinding { round: 4, evidence_root: [3; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[target()]).unwrap(),
    }).unwrap();
    assert!(receipt.restored);
    assert_eq!(receipt.refunded_units, 0);
    assert_eq!(controller.inspect().ledger.charged, 5);
    assert!(controller.dispatch(&pending, &first.action, &snapshot()).is_err());
    let fresh = controller.propose(3, action(receipt.revocation_floor), &snapshot()).unwrap();
    approve(&mut controller, 3, 5);
    let permit = controller.authorize(3, &snapshot()).unwrap();
    controller.dispatch(&permit, &fresh.action, &snapshot()).unwrap();
    controller.record_trusted_outcome(2, TrustedOutcome::NotExecuted).unwrap();
    assert_eq!(controller.inspect().ledger.charged, 5);
    conserved(&controller);
}

#[test]
fn bounded_three_valued_truth_tables_match_an_independent_table() {
    use Truth::{Satisfied as T, Unknown as U, Violated as F};
    let values = [T, F, U];
    let all = [[T, F, U], [F, F, F], [U, F, U]];
    let any = [[T, T, T], [T, F, U], [T, U, U]];
    let leaf = |value| match value {
        T => Predicate::UnitsAtMost(5),
        F => Predicate::UnitsAtMost(0),
        U => Predicate::Absent { key: 999 },
    };
    let action = FrozenAction::freeze(action(0)).unwrap();
    let mut unavailable = snapshot();
    unavailable.complete = false;
    for (i, left) in values.into_iter().enumerate() {
        for (j, right) in values.into_iter().enumerate() {
            for (root, expected) in [(Predicate::All(vec![0, 1]), all[i][j]), (Predicate::Any(vec![0, 1]), any[i][j])] {
                let policy = Policy::new(1, vec![leaf(left), leaf(right), root]).unwrap();
                let evaluation = policy.evaluate(&action, &unavailable).unwrap();
                assert_eq!(evaluation.result(), expected);
                assert!(!evaluation.certifiable());
            }
        }
        let policy = Policy::new(1, vec![leaf(left), Predicate::Not(0)]).unwrap();
        assert_eq!(policy.evaluate(&action, &unavailable).unwrap().result(), [F, T, U][i]);
    }
}

#[test]
fn policy_history_capacity_refuses_without_changing_authority() {
    let mut controller = controller();
    for index in 0..MAX_POLICY_CHANGES {
        let state = controller.inspect();
        controller.replace_policy(state.sequence, state.ledger.epoch, policy(index as u64 + 2)).unwrap();
    }
    let state = controller.inspect();
    assert_eq!(controller.policy_changes().len(), MAX_POLICY_CHANGES);
    assert_eq!(controller.replace_policy(state.sequence, state.ledger.epoch, policy(MAX_POLICY_CHANGES as u64 + 2)), Err(Error::Limit));
    assert_eq!(controller.inspect(), state);
    assert_eq!(controller.policy().generation(), MAX_POLICY_CHANGES as u64 + 1);
    let mut old = action(0);
    assert_eq!(controller.propose(1, old.clone(), &snapshot()), Err(Error::Stale));
    old.policy_epoch = state.ledger.epoch;
    let fresh = controller.propose(1, old, &snapshot()).unwrap();
    approve(&mut controller, 1, 1);
    let permit = controller.authorize(1, &snapshot()).unwrap();
    controller.dispatch(&permit, &fresh.action, &snapshot()).unwrap();
    conserved(&controller);
}
