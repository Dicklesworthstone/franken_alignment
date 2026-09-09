//! Public archive -> counterfactual -> fresh-live-review separation.

use fa_reference::action::consequence::experiment::{
    EmpiricalStatus, Intervention, InterventionScope, MAX_SEARCH_CANDIDATES, Minimality,
    NextRequirement, PolicyExperiment,
};
use fa_reference::action::consequence::gate::containment::session::policy::controller::{
    ControllerConfig, PolicyAuthority, PolicyReview, replay::{DecisionArchive, ReviewAnchor},
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate, Truth};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION,
};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn spec() -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget {
            adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1,
        }),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100), units: 4,
    }
}

fn policy() -> Policy {
    Policy::new(1, vec![
        Predicate::ExactValue { key: 7, value: vec![9] },
        Predicate::Absent { key: 8 },
        Predicate::EmptyRange { start: 10, end: 20 },
        Predicate::PayloadAtMost(64),
        Predicate::All(vec![0, 1, 2, 3]),
    ]).unwrap()
}

fn snapshot(entries: &[(u64, u8)]) -> Snapshot {
    Snapshot {
        semantic_epoch: 3, complete: true,
        values: entries.iter().map(|(key, value)| (*key, vec![*value])).collect(),
    }
}

fn archive(
    policy: Policy,
    initial: &Snapshot,
    observed: &Snapshot,
) -> (PolicyAuthority, ReviewAnchor, DecisionArchive, PolicyReview) {
    let actor = ActorState::new(RestartProfile {
        id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::ExactRestart,
    }, vec![1], vec![2], vec![3], 1).unwrap();
    let congress = CongressPolicy {
        generation: 1,
        members: BTreeMap::from([
            ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 5 }),
            ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 5 }),
        ]),
        caps: Caps { per_member: 5, per_cohort: 5 }, continue_minimum: 8,
        continue_hold_maximum: 0, narrow_at: 8, suspend_at: 10,
        minimum_members: 2, minimum_cohorts: 2,
    };
    let mut controller = PolicyAuthority::new(ControllerConfig {
        scope: spec().scope, total: 20, max_attempts: 16, actor, suspend_at_incident: 3,
        policy, congress, narrowed_targets: TargetCeiling::new(&[spec().target.unwrap()]).unwrap(),
    }).unwrap();
    controller.observe_time(ElapsedTick(1)).unwrap();
    assert_eq!(controller.propose(1, spec(), initial).unwrap().state, ActionState::Reviewing);
    let mut session = controller.begin_review(1, 11, [1; 32], observed).unwrap();
    let anchor = session.replay_anchor();
    for member in ["alice", "bob"] {
        let commitment = session.commitment(member, Verdict::Allow, b"reference-salt").unwrap();
        session.commit(member, commitment).unwrap();
    }
    session.open_reveals().unwrap();
    for member in ["alice", "bob"] {
        session.reveal(member, Verdict::Allow, b"reference-salt").unwrap();
    }
    let review = session.finish().unwrap();
    let archive = review.replay_archive().unwrap();
    (controller, anchor, archive, review)
}

fn experiment(archive: &DecisionArchive, anchor: &ReviewAnchor, keys: &[u64]) -> PolicyExperiment {
    PolicyExperiment::from_archive(
        archive, anchor, InterventionScope::new(71, true, true, true, keys).unwrap(),
    ).unwrap()
}

fn key(key: u64, expected: Option<u8>, replacement: Option<u8>) -> Intervention {
    Intervention::Key {
        key, expected: expected.map(|value| vec![value]), replacement: replacement.map(|value| vec![value]),
    }
}

#[test]
fn repairing_exact_evidence_does_not_repair_the_live_denial_or_copy_rights() {
    let good = snapshot(&[(7, 9)]);
    let bad = snapshot(&[(7, 0), (8, 1)]);
    let (mut controller, anchor, archive, review) = archive(policy(), &good, &bad);
    let experiment = experiment(&archive, &anchor, &[7, 8]);
    let before = controller.inspect();
    let actor = controller.actor().clone();
    let control = experiment.run(&[]).unwrap();
    assert_eq!(control.counterfactual(), Truth::Violated);
    assert_eq!(experiment.baseline().decision().consequence, Consequence::Deny);
    let repaired = experiment.run(&[key(7, Some(0), Some(9)), key(8, Some(1), None)]).unwrap();
    assert_eq!(repaired.baseline(), Truth::Violated);
    assert_eq!(repaired.counterfactual(), Truth::Satisfied);
    assert_eq!(repaired.next_requirement(), NextRequirement::FreshIndependentReview);
    assert_eq!(repaired.empirical_status(), EmpiricalStatus::InvalidatedByIntervention);
    assert_eq!(repaired.changes().iter().map(|change| change.node).collect::<Vec<_>>(), vec![0, 1, 4]);
    assert_eq!(controller.inspect(), before);
    assert_eq!(controller.actor(), &actor);
    assert_eq!(controller.incident_count(), 0);
    assert_eq!(experiment.run(&[]).unwrap(), control);
    archive.verify(&anchor).unwrap();
    controller.apply_review(review, &bad).unwrap();
    assert_eq!(controller.inspect().ledger.stages[&1], ActionState::Denied);
    assert!(controller.authorize(1, &good).is_err());
    assert_eq!(controller.inspect().ledger.available, 20);
}

#[test]
fn policy_preserving_payload_edit_still_invalidates_the_old_helper_judgment() {
    let state = snapshot(&[(7, 9)]);
    let (mut controller, anchor, archive, review) = archive(policy(), &state, &state);
    let experiment = experiment(&archive, &anchor, &[]);
    controller.apply_review(review, &state).unwrap();
    let permit = controller.authorize(1, &state).unwrap();
    let before = controller.inspect();
    let report = experiment.run(&[Intervention::Payload(b"other".to_vec())]).unwrap();
    assert_eq!(report.counterfactual(), Truth::Satisfied);
    assert!(report.changes().is_empty());
    assert_eq!(report.empirical_status(), EmpiricalStatus::InvalidatedByIntervention);
    assert_eq!(report.next_requirement(), NextRequirement::FreshIndependentReview);
    let no_op = experiment.run(&[Intervention::Payload(spec().payload)]).unwrap();
    assert_eq!(no_op.empirical_status(), EmpiricalStatus::HistoricalOnly);
    assert_eq!(controller.inspect(), before);
    let mut altered = anchor.action.spec().clone();
    altered.payload = b"other".to_vec();
    let altered = FrozenAction::freeze(altered).unwrap();
    assert_eq!(controller.dispatch(&permit, &altered, &state), Err(Error::Binding));
    controller.dispatch(&permit, &anchor.action, &state).unwrap();
    assert_eq!(controller.inspect().ledger.charged, 4);
}

#[test]
fn deleting_a_range_counterexample_requires_more_observation_not_approval() {
    let good = snapshot(&[(7, 9)]);
    let bad = snapshot(&[(7, 9), (15, 1), (18, 1)]);
    let (_, anchor, archive, _) = archive(policy(), &good, &bad);
    let experiment = experiment(&archive, &anchor, &[15]);
    let report = experiment.run(&[key(15, Some(1), None)]).unwrap();
    assert_eq!(report.trace()[2], Truth::Unknown);
    assert_eq!(report.counterfactual(), Truth::Unknown);
    assert_eq!(report.next_requirement(), NextRequirement::MoreEvidence);
    // The full test world contains 18, but the archive retained only member 15.
    // The experiment must neither invent 18 nor claim there is no other member.
    assert!(matches!(PolicyExperiment::from_archive(
        &archive, &anchor, InterventionScope::new(72, false, false, false, &[18]).unwrap(),
    ), Err(Error::Incomplete)));
}

#[test]
fn unknown_negated_range_cannot_hide_under_a_satisfied_or_arm() {
    let policy = Policy::new(1, vec![
        Predicate::EmptyRange { start: 10, end: 20 },
        Predicate::Not(0), Predicate::PayloadAtMost(64), Predicate::Any(vec![1, 2]),
    ]).unwrap();
    let state = snapshot(&[(15, 1)]);
    let (_, anchor, archive, _) = archive(policy, &state, &state);
    let report = experiment(&archive, &anchor, &[15]).run(&[key(15, Some(1), None)]).unwrap();
    assert_eq!(report.trace(), &[Truth::Unknown, Truth::Unknown, Truth::Satisfied, Truth::Satisfied]);
    assert_eq!(report.counterfactual(), Truth::Satisfied);
    assert_eq!(report.next_requirement(), NextRequirement::MoreEvidence);
}

#[test]
fn closed_range_allows_insertion_but_every_branch_restarts_at_the_control() {
    let state = snapshot(&[(7, 9)]);
    let (_, anchor, archive, _) = archive(policy(), &state, &state);
    let experiment = experiment(&archive, &anchor, &[12]);
    let control = experiment.run(&[]).unwrap();
    let inserted = experiment.run(&[key(12, None, Some(1))]).unwrap();
    assert_eq!(inserted.counterfactual(), Truth::Violated);
    assert_eq!(experiment.run(&[key(12, Some(1), None)]), Err(Error::Stale));
    let no_op = experiment.run(&[key(12, None, None)]).unwrap();
    assert_eq!(no_op.counterfactual(), Truth::Satisfied);
    assert_eq!(no_op.empirical_status(), EmpiricalStatus::HistoricalOnly);
    assert_eq!(experiment.run(&[]).unwrap(), control);
}

#[test]
fn scope_preconditions_duplicate_targets_and_bad_anchors_refuse_without_mutation() {
    let state = snapshot(&[(7, 9)]);
    let (_, anchor, archive, _) = archive(policy(), &state, &state);
    let scope = InterventionScope::new(71, false, false, false, &[7]).unwrap();
    let experiment = PolicyExperiment::from_archive(&archive, &anchor, scope.clone()).unwrap();
    let before = experiment.run(&[]).unwrap();
    assert_eq!(experiment.run(&[Intervention::Payload(vec![1])]), Err(Error::Binding));
    assert_eq!(experiment.run(&[key(8, None, Some(1))]), Err(Error::Binding));
    assert_eq!(experiment.run(&[key(7, Some(0), Some(1))]), Err(Error::Stale));
    assert_eq!(experiment.run(&[key(7, Some(9), Some(1)), key(7, Some(9), None)]), Err(Error::Duplicate));
    let mut wrong = anchor.clone();
    wrong.round += 1;
    assert!(matches!(PolicyExperiment::from_archive(&archive, &wrong, scope), Err(Error::Binding)));
    assert_eq!(experiment.run(&[]).unwrap(), before);
    archive.verify(&anchor).unwrap();
}

#[test]
fn action_constructor_bounds_still_apply_to_experimental_edits() {
    let state = snapshot(&[(7, 9)]);
    let (_, anchor, archive, _) = archive(policy(), &state, &state);
    let experiment = experiment(&archive, &anchor, &[]);
    let mut invalid = spec().target.unwrap();
    invalid.expected_version = 0;
    assert_eq!(experiment.run(&[Intervention::Target(invalid)]), Err(Error::InvalidInput));
    assert_eq!(experiment.run(&[Intervention::Units(0)]), Err(Error::InvalidInput));
    assert_eq!(experiment.run(&[Intervention::Payload(vec![0; 65_537])]), Err(Error::Limit));
    let valid = experiment.run(&[Intervention::Units(1)]).unwrap();
    assert_eq!(valid.counterfactual(), Truth::Satisfied);
    assert_eq!(valid.empirical_status(), EmpiricalStatus::InvalidatedByIntervention);
}

#[test]
fn search_finds_joint_repairs_and_retains_the_failed_single_edit_controls() {
    let good = snapshot(&[(7, 9)]);
    let bad = snapshot(&[(7, 0), (8, 1)]);
    let (_, anchor, archive, _) = archive(policy(), &good, &bad);
    let experiment = experiment(&archive, &anchor, &[7, 8]);
    let candidates = [key(7, Some(0), Some(9)), key(8, Some(1), None)];
    let search = experiment.search_repairs(&candidates).unwrap();
    assert_eq!(search.cases().len(), 4);
    for case in &search.cases()[..3] {
        assert_eq!(case.outcome.as_ref().unwrap().counterfactual(), Truth::Violated);
    }
    assert_eq!(search.sufficient_repairs().len(), 1);
    assert_eq!(search.sufficient_repairs()[0].mask, 3);
    assert_eq!(search.sufficient_repairs()[0].minimality, Minimality::EstablishedWithinMenu);
    assert_eq!(search.cases()[3].outcome.as_ref().unwrap().next_requirement(), NextRequirement::FreshIndependentReview);
    assert_eq!(experiment.search_repairs(&candidates).unwrap(), search);
}

#[test]
fn minimality_checks_all_subsets_not_just_one_edit_deletions() {
    let policy = Policy::new(1, vec![
        Predicate::ExactValue { key: 1, value: vec![0] },
        Predicate::ExactValue { key: 2, value: vec![0] }, Predicate::All(vec![0, 1]),
        Predicate::ExactValue { key: 1, value: vec![1] },
        Predicate::ExactValue { key: 2, value: vec![1] }, Predicate::All(vec![3, 4]),
        Predicate::Any(vec![2, 5]),
    ]).unwrap();
    let state = snapshot(&[(1, 0), (2, 0)]);
    let (_, anchor, archive, _) = archive(policy, &state, &state);
    let experiment = experiment(&archive, &anchor, &[1, 2]);
    let search = experiment.search_repairs(&[key(1, Some(0), Some(1)), key(2, Some(0), Some(1))]).unwrap();
    assert_eq!(search.cases()[1].outcome.as_ref().unwrap().counterfactual(), Truth::Violated);
    assert_eq!(search.cases()[2].outcome.as_ref().unwrap().counterfactual(), Truth::Violated);
    assert_eq!(search.sufficient_repairs()[0].mask, 0);
    assert_eq!(search.sufficient_repairs()[1].mask, 3);
    assert_eq!(search.sufficient_repairs()[1].minimality, Minimality::NotMinimal);
}

#[test]
fn an_unknown_smaller_branch_prevents_a_minimality_claim() {
    let policy = Policy::new(1, vec![
        Predicate::Absent { key: 15 }, Predicate::EmptyRange { start: 10, end: 20 },
        Predicate::Not(1), Predicate::Absent { key: 16 }, Predicate::Not(3),
        Predicate::Any(vec![3, 4]), Predicate::All(vec![0, 2, 5]),
    ]).unwrap();
    let good = snapshot(&[(16, 1)]);
    let bad = snapshot(&[(15, 1)]);
    let (_, anchor, archive, _) = archive(policy, &good, &bad);
    let experiment = experiment(&archive, &anchor, &[15, 16]);
    let search = experiment.search_repairs(&[key(15, Some(1), None), key(16, None, Some(1))]).unwrap();
    assert_eq!(search.cases()[1].outcome.as_ref().unwrap().next_requirement(), NextRequirement::MoreEvidence);
    assert_eq!(search.cases()[2].outcome.as_ref().unwrap().counterfactual(), Truth::Violated);
    assert_eq!(search.sufficient_repairs().len(), 1);
    assert_eq!(search.sufficient_repairs()[0].mask, 3);
    assert_eq!(search.sufficient_repairs()[0].minimality, Minimality::Undetermined);
}

#[test]
fn conflicting_candidates_remain_visible_as_refused_branches() {
    let state = snapshot(&[(7, 9)]);
    let (_, anchor, archive, _) = archive(policy(), &state, &state);
    let search = experiment(&archive, &anchor, &[]).search_repairs(&[
        Intervention::Payload(b"a".to_vec()), Intervention::Payload(b"b".to_vec()),
    ]).unwrap();
    assert_eq!(search.cases().len(), 4);
    assert_eq!(search.cases()[3].outcome, Err(Error::Duplicate));
    assert_eq!(search.sufficient_repairs().iter().map(|repair| repair.mask).collect::<Vec<_>>(), vec![0, 1, 2]);
}

#[test]
fn search_runs_the_declared_finite_domain_and_refuses_one_over_capacity() {
    let state = snapshot(&[(7, 9)]);
    let (_, anchor, archive, _) = archive(policy(), &state, &state);
    let keys: Vec<_> = (10..19).collect();
    let experiment = experiment(&archive, &anchor, &keys);
    let candidates: Vec<_> = keys.iter().map(|id| key(*id, None, Some(1))).collect();
    let search = experiment.search_repairs(&candidates[..MAX_SEARCH_CANDIDATES]).unwrap();
    assert_eq!(search.cases().len(), 256);
    assert_eq!(search.sufficient_repairs().len(), 1);
    assert_eq!(search.sufficient_repairs()[0].mask, 0);
    assert!(search.cases().iter().all(|case| case.outcome.is_ok()));
    assert_eq!(experiment.search_repairs(&candidates), Err(Error::Limit));
    assert_eq!(experiment.run(&[]).unwrap().counterfactual(), Truth::Satisfied);
}
