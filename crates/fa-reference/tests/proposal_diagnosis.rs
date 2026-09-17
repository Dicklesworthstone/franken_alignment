//! The native policy controller is the admission and Boolean-evaluation oracle.
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::experiment::{Intervention, InterventionScope, Minimality, NextRequirement};
use fa_reference::action::consequence::experiment::proposal::{ProposalExperiment, search::*};
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade, RestartProfile}};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate, Truth,
    controller::{ControllerConfig, PolicyAuthority, Proposal}};
use fa_reference::reducer::Caps;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
fn target() -> ResolvedTarget { ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 } }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"bad".to_vec())]) } }
fn spec() -> ActionSpec { ActionSpec { version: VERSION, scope: scope(), target: Some(target()),
    payload: b"bad".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 10 } }
fn policy() -> Policy { Policy::new(1, vec![Predicate::PayloadIs(b"ok".to_vec()),
    Predicate::ExactValue { key: 7, value: b"ok".to_vec() }, Predicate::All(vec![0, 1])]).unwrap() }
fn native(policy: Policy, snapshot: &Snapshot) -> (PolicyAuthority, Proposal) {
    let config = ControllerConfig { scope: scope(), total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            vec![1], vec![2], vec![3], 1).unwrap(), suspend_at_incident: 3, policy,
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([
            ("member".to_owned(), MemberPolicy { cohort: "one".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[target()]).unwrap() };
    let mut host = PolicyAuthority::new(config).unwrap(); host.observe_time(ElapsedTick(1)).unwrap();
    let proposal = host.propose(1, spec(), snapshot).unwrap(); (host, proposal)
}
fn intervention_scope() -> InterventionScope { InterventionScope::new(11, true, false, true, &[7]).unwrap() }
fn menu() -> Vec<Intervention> { vec![Intervention::Payload(b"ok".to_vec()),
    Intervention::Key { key: 7, expected: Some(b"bad".to_vec()), replacement: Some(b"ok".to_vec()) }] }
fn experiment() -> ProposalExperiment {
    let (_, proposal) = native(policy(), &snapshot());
    ProposalExperiment::new(proposal, &snapshot(), intervention_scope()).unwrap()
}
fn budget() -> ProposalSearchBudget { ProposalSearchBudget {
    cases: MAX_PROPOSAL_SEARCH_CASES, retained_edit_bytes: MAX_PROPOSAL_SEARCH_EDIT_BYTES } }
fn finish(mut cursor: ProposalRepairCursor) -> ProposalRepairReport {
    while cursor.status() == ProposalSearchStatus::Running { cursor.advance().unwrap(); }
    cursor.finish().unwrap()
}

#[test]
fn denied_admissions_are_investigable_without_a_congress_and_repairs_do_not_change_authority() {
    let (host, proposal) = native(policy(), &snapshot()); let before = host.inspect();
    assert_eq!(proposal.state, ActionState::Denied);
    let experiment = ProposalExperiment::new(proposal.clone(), &snapshot(), intervention_scope()).unwrap();
    let control = experiment.run(&[]).unwrap(); assert_eq!(control.counterfactual(), Truth::Violated);
    let repair = experiment.run(&menu()).unwrap();
    assert_eq!(repair.purpose(), Purpose::Experiment); assert_eq!(repair.initial_state(), ActionState::Denied);
    assert_eq!(repair.next_requirement(), NextRequirement::FreshIndependentReview);
    assert_eq!(repair.counterfactual(), Truth::Satisfied); assert_eq!(repair.changes().len(), 3);
    assert_eq!(host.inspect(), before); assert_eq!(experiment.original(), &proposal);
    assert_eq!(experiment.run(&[]).unwrap(), control);
}

#[test]
fn deleting_a_range_witness_requires_more_evidence_unless_its_whole_domain_was_observed() {
    let snap = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(10, vec![1]), (15, vec![2])]) };
    for end in [11, 20] {
        let (_, p) = native(Policy::new(1, vec![Predicate::EmptyRange { start: 10, end }]).unwrap(), &snap);
        let e = ProposalExperiment::new(p, &snap, InterventionScope::new(1, false, false, false, &[10]).unwrap()).unwrap();
        let result = e.run(&[Intervention::Key { key: 10, expected: Some(vec![1]), replacement: None }]).unwrap();
        assert_eq!(result.counterfactual(), if end == 11 { Truth::Satisfied } else { Truth::Unknown });
        assert_eq!(result.next_requirement(), if end == 11 { NextRequirement::FreshIndependentReview } else { NextRequirement::MoreEvidence });
        // Unretained rows from the supplied historical snapshot are not smuggled
        // into the experiment, including the second member at 15.
        assert!(matches!(ProposalExperiment::new(e.original().clone(), &snap,
            InterventionScope::new(2, false, false, false, &[15]).unwrap()), Err(Error::Incomplete)));
    }
}

#[test]
fn every_menu_subset_agrees_with_independent_native_evaluation_and_retains_conflicts() {
    let e = experiment(); let mut choices = menu(); choices.push(Intervention::Units(1));
    let result = finish(e.begin_repair_search(&choices, budget()).unwrap());
    assert_eq!(result.cases().len(), 8); assert_eq!(result.work().entered_cases, 8);
    for row in result.cases() {
        let mut action = spec(); let mut snap = snapshot();
        if row.mask & 1 != 0 { action.payload = b"ok".to_vec(); }
        if row.mask & 2 != 0 { snap.values.insert(7, b"ok".to_vec()); }
        if row.mask & 4 != 0 { action.units = 1; }
        let frozen = fa_reference::action::FrozenAction::freeze(action).unwrap();
        let evaluated = policy().evaluate(&frozen, &snap).unwrap();
        let actual = row.outcome.as_ref().unwrap();
        assert_eq!(actual.counterfactual(), evaluated.result());
        assert_eq!(actual.trace(), evaluated.trace().iter().map(|s| s.result).collect::<Vec<_>>());
    }
    assert_eq!(result.sufficient_repairs().iter().map(|r| (r.mask, r.minimality)).collect::<Vec<_>>(),
        vec![(3, Minimality::EstablishedWithinMenu), (7, Minimality::NotMinimal)]);
    let conflicting = [Intervention::Payload(b"ok".to_vec()), Intervention::Payload(b"other".to_vec())];
    let result = finish(e.begin_repair_search(&conflicting, budget()).unwrap());
    assert_eq!(result.cases().len(), 4); assert_eq!(result.cases()[3].outcome, Err(Error::Duplicate));
}

#[test]
fn nonmonotone_minimality_checks_the_empty_control_not_only_one_edit_deletions() {
    let p = Policy::new(1, vec![Predicate::PayloadIs(b"ok".to_vec()), Predicate::UnitsAtMost(5),
        Predicate::All(vec![0, 1]), Predicate::Not(0), Predicate::Not(1),
        Predicate::All(vec![3, 4]), Predicate::Any(vec![2, 5])]).unwrap();
    let (_, original) = native(p, &snapshot()); assert_eq!(original.state, ActionState::Reviewing);
    let e = ProposalExperiment::new(original, &snapshot(), InterventionScope::new(1, true, false, true, &[]).unwrap()).unwrap();
    let result = finish(e.begin_repair_search(&[Intervention::Payload(b"ok".to_vec()), Intervention::Units(1)], budget()).unwrap());
    assert_eq!(result.cases()[1].outcome.as_ref().unwrap().counterfactual(), Truth::Violated);
    assert_eq!(result.cases()[2].outcome.as_ref().unwrap().counterfactual(), Truth::Violated);
    assert_eq!(result.sufficient_repairs().iter().find(|r| r.mask == 3).unwrap().minimality, Minimality::NotMinimal);
}

#[test]
fn cancellation_keeps_the_denominator_and_cannot_claim_complete_search_or_restart() {
    let e = experiment(); let mut choices = menu(); choices.push(Intervention::Units(1));
    for at in 0..8 {
        let mut cursor = e.begin_repair_search(&choices, budget()).unwrap();
        for _ in 0..at { cursor.advance().unwrap(); }
        let work = cursor.work(); cursor.cancel().unwrap(); cursor.cancel().unwrap();
        let partial = cursor.report(); assert_eq!(partial.status(), ProposalSearchStatus::Cancelled);
        assert_eq!(partial.unreported_masks().collect::<Vec<_>>(), (at as u16..8).collect::<Vec<_>>());
        assert_eq!(cursor.work(), work); assert!(cursor.finish().is_err());
        assert_eq!(cursor.advance(), Err(Error::WrongState)); assert_eq!(cursor.work(), work);
        assert!(partial.sufficient_repairs().iter().all(|r| r.minimality != Minimality::EstablishedWithinMenu));
    }
    let mut complete = e.begin_repair_search(&menu(), budget()).unwrap();
    while complete.status() == ProposalSearchStatus::Running { complete.advance().unwrap(); }
    let before = complete.finish().unwrap(); complete.advance().unwrap();
    assert_eq!(complete.finish().unwrap(), before); assert_eq!(complete.cancel(), Err(Error::WrongState));
}

#[test]
fn whole_menu_caps_admit_exact_bounds_and_refuse_one_less_before_any_branch() {
    let e = experiment(); let choices = menu(); let exact = ProposalSearchBudget { cases: 4, retained_edit_bytes: 14 };
    assert!(matches!(e.begin_repair_search(&choices, ProposalSearchBudget { cases: 3, ..exact }), Err(Error::Limit)));
    assert!(matches!(e.begin_repair_search(&choices, ProposalSearchBudget { retained_edit_bytes: 13, ..exact }), Err(Error::Limit)));
    let report = finish(e.begin_repair_search(&choices, exact).unwrap());
    assert_eq!(report.work().planned_edit_bytes, 14); assert_eq!(report.work().retained_edit_bytes, 14);
    let maximum = vec![Intervention::Units(1); 8];
    assert_eq!(finish(e.begin_repair_search(&maximum, budget()).unwrap()).cases().len(), 256);
    assert!(matches!(e.begin_repair_search(&vec![Intervention::Units(1); 9], budget()), Err(Error::Limit)));
    assert_eq!(finish(e.begin_repair_search(&[], ProposalSearchBudget { cases: 1, retained_edit_bytes: 0 }).unwrap()).cases().len(), 1);
}

#[test]
fn forged_evaluation_state_witnesses_or_snapshot_do_not_become_admission_evidence() {
    let (_, p) = native(policy(), &snapshot());
    for variant in 0..5 {
        let mut bad = p.clone(); let mut snap = snapshot();
        match variant {
            0 => bad.state = ActionState::Reviewing,
            1 => { let mut s = bad.action.spec().clone(); s.required_witnesses.clear(); bad.action = fa_reference::action::FrozenAction::freeze(s).unwrap(); }
            2 => { snap.values.insert(7, b"changed".to_vec()); }
            3 => bad.snapshot_semantic_epoch += 1,
            _ => snap.complete = false,
        }
        assert!(ProposalExperiment::new(bad, &snap, intervention_scope()).is_err(), "variant {variant}");
    }
    let e = ProposalExperiment::new(p, &snapshot(), intervention_scope()).unwrap();
    assert_eq!(e.run(&[Intervention::Target(target())]), Err(Error::Binding));
    assert_eq!(e.run(&[Intervention::Key { key: 7, expected: None, replacement: None }]), Err(Error::Stale));
    assert!(e.run(&menu()).is_ok());
}

#[test]
fn unknown_required_subsets_block_minimality_even_when_the_repaired_root_is_satisfied() {
    let snap = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(10, vec![1])]) };
    let p = Policy::new(1, vec![Predicate::Absent { key: 10 }, Predicate::EmptyRange { start: 10, end: 20 },
        Predicate::Not(1), Predicate::Absent { key: 15 }, Predicate::Any(vec![2, 3]), Predicate::All(vec![0, 4])]).unwrap();
    let (_, original) = native(p, &snap);
    let e = ProposalExperiment::new(original, &snap, InterventionScope::new(1, false, false, false, &[10, 15]).unwrap()).unwrap();
    let choices = [Intervention::Key { key: 10, expected: Some(vec![1]), replacement: None },
        Intervention::Key { key: 15, expected: None, replacement: Some(vec![2]) }];
    let result = finish(e.begin_repair_search(&choices, budget()).unwrap());
    let unresolved = result.cases()[1].outcome.as_ref().unwrap();
    assert_eq!(unresolved.counterfactual(), Truth::Satisfied);
    assert_eq!(unresolved.next_requirement(), NextRequirement::MoreEvidence);
    assert_eq!(result.cases()[3].outcome.as_ref().unwrap().next_requirement(), NextRequirement::FreshIndependentReview);
    assert_eq!(result.sufficient_repairs()[0].mask, 3);
    assert_eq!(result.sufficient_repairs()[0].minimality, Minimality::Undetermined);
}
