//! The actual held-out promotion, original reducer and original authority.
//! Synthetic labels exercise protocol behavior, not detector qualification.
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy,
    CredibilityBinding, CredibilityRequirements};
use fa_reference::action::consequence::congress::credibility::{Campaign, CaseSpec,
    CredibilityLedger, CredibilitySnapshot, EvaluationLabel, EvaluationScope,
    HelperGeneration, LabelSource, LabelVerdict, Observation};
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade, RestartProfile}};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::{ControllerConfig, PolicyAuthority};
use fa_reference::action::consequence::gate::containment::session::policy::controller::credibility::{CredibilityActivation, CredibilityWithdrawalRequest};
use fa_reference::action::consequence::gate::containment::session::policy::controller::credibility::joint::{
    self, HeldOutJointBudget, HeldOutJointFailure, HeldOutJointPolicy,
};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, TrustedOutcome, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

const HOLD: Observation = Observation::Hold { first_sequence: 1 };
const CLEAR: Observation = Observation::Clear;
fn guard() -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0, HeldOutJointBudget::default()).unwrap()
}
fn state() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(epoch: u64) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"output".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 7 }
}
fn config(hold_maximum: u64) -> ControllerConfig {
    ControllerConfig { scope: spec(0).scope, total: 100, max_attempts: 32,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::ExactRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::Absent { key: 7 }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([
            ("a".into(), MemberPolicy { cohort: "a".into(), weight: 10 }),
            ("b".into(), MemberPolicy { cohort: "b".into(), weight: 10 }),
        ]), caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 5,
            continue_hold_maximum: hold_maximum, narrow_at: 20, suspend_at: 30,
            minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap() }
}
fn controller(hold_maximum: u64, policy: Option<HeldOutJointPolicy>) -> PolicyAuthority {
    let mut host = PolicyAuthority::new(config(hold_maximum)).unwrap();
    if let Some(policy) = policy { host.enable_held_out_joint(policy).unwrap(); }
    host.observe_time(ElapsedTick(1)).unwrap();
    host
}
fn authorized(host: &mut PolicyAuthority, id: u64) -> (FrozenAction, Permit) {
    let action = host.propose(id, spec(host.inspect().ledger.epoch), &state()).unwrap().action;
    let mut session = host.begin_review(id, 100 + id, [9; 32], &state()).unwrap();
    for member in ["a", "b"] {
        session.commit(member, session.commitment(member, Verdict::Allow, b"salt").unwrap()).unwrap();
    }
    session.open_reveals().unwrap();
    for member in ["a", "b"] { session.reveal(member, Verdict::Allow, b"salt").unwrap(); }
    host.apply_review(session.finish().unwrap(), &state()).unwrap();
    let permit = host.authorize(id, &state()).unwrap();
    (action, permit)
}
fn seed(host: &mut PolicyAuthority) {
    for id in 1..=2 { authorized(host, id); host.cancel(id).unwrap(); }
}
#[derive(Clone)]
struct Row { root: u8, stratum: &'static str, truth: Option<LabelVerdict>, votes: [Observation; 2] }
fn rows() -> Vec<Row> {
    vec![
        Row { root: 1, stratum: "effect", truth: Some(LabelVerdict::Violation), votes: [HOLD, CLEAR] },
        Row { root: 2, stratum: "effect", truth: Some(LabelVerdict::Violation), votes: [CLEAR, HOLD] },
        Row { root: 3, stratum: "effect", truth: Some(LabelVerdict::Safe), votes: [CLEAR, CLEAR] },
    ]
}
fn evidence(rows: &[Row]) -> CredibilitySnapshot {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: 1, model_generation: 1, evaluator_generation: 1, held_out_manifest: [8; 32] },
        label_owner: "independent".into(), helpers: ["a", "b"].into_iter().map(|m| (
            m.into(), HelperGeneration { generation: 1, cohort: m.into() })).collect(),
        strata: rows.iter().map(|r| r.stratum.to_owned()).collect::<BTreeSet<_>>(),
        cases: rows.iter().enumerate().map(|(i, r)| CaseSpec { id: i as u64 + 1,
            stratum: r.stratum.into(), evidence_root: [r.root; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for (i, row) in rows.iter().enumerate() {
        let id = i as u64 + 1;
        ledger.record_observations(id, ["a", "b"].into_iter().zip(row.votes)
            .map(|(m, v)| (m.into(), v)).collect()).unwrap();
        if let Some(verdict) = row.truth {
            ledger.record_label(id, EvaluationLabel { owner: "independent".into(), evaluator_generation: 1,
                source: LabelSource::IndependentEvaluation, evidence_root: [row.root; 32],
                recorded_sequence: 3, verdict }).unwrap();
        }
    }
    ledger.seal(3).unwrap()
}
fn request(host: &PolicyAuthority, operation: u64, generation: u64) -> CredibilityActivation {
    let snapshot = evidence(&rows());
    CredibilityActivation { operation, expected_control_sequence: host.inspect().sequence,
        expected_epoch: host.inspect().ledger.epoch, scope: spec(0).scope, policy_generation: host.policy().generation(),
        actor_profile: host.actor().profile(), binding: CredibilityBinding {
            scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(), reducer_generation: generation,
        }, stratum: "effect".into(), requirements: CredibilityRequirements {
            minimum_safe_cases: 1, minimum_violation_cases: 2, minimum_precision_ppm: 1_000_000,
            minimum_timely_recall_ppm: 500_000, maximum_false_positive_ppm: 0, base_weight: 10,
            lead_bonus_weight: 0, lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000,
        }, snapshot }
}
fn candidate(baseline: &CongressPolicy) -> CongressPolicy {
    let mut next = baseline.clone(); next.generation = 2;
    for member in next.members.values_mut() { member.weight = 5; }
    next
}

#[test]
fn marginally_qualified_weights_cannot_newly_admit_complementary_violations() {
    let mut host = controller(5, Some(guard())); seed(&mut host);
    let request = request(&host, 10, 2);
    let baseline = config(5).congress;
    let promoted = baseline.promote_credibility(request.snapshot.clone(), &request.requirements,
        &request.binding, host.inspect().sequence + 1).unwrap();
    assert_eq!(promoted.admitted_members()["a"].weight, 5);
    let report = joint::evaluate(guard(), &baseline, &candidate(&baseline), &request.snapshot).unwrap();
    assert_eq!(report.strata["effect"].baseline.escaped_roots, 0);
    assert_eq!(report.strata["effect"].candidate.escaped_roots, 2);
    assert!(report.failures.contains(&HeldOutJointFailure::RegressedCase(1)));
    let before = host.inspect();
    assert_eq!(host.activate_credibility(request), Err(Error::Incomplete));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.held_out_joint_report(10), Err(Error::Missing));
    assert_eq!(host.held_out_joint_policy(), Some(guard()));
}

#[test]
fn accepted_weights_use_normal_dispatch_and_historical_retry_preserves_new_work() {
    let mut host = controller(0, Some(guard())); seed(&mut host);
    let (sent, permit) = authorized(&mut host, 3);
    host.dispatch(&permit, &sent, &state()).unwrap(); host.mark_unknown(3).unwrap();
    let (pending, old_key) = authorized(&mut host, 4);
    let activation = request(&host, 10, 2);
    let receipt = host.activate_credibility(activation.clone()).unwrap();
    assert_eq!(receipt.cancelled, vec![4]); assert_eq!(receipt.refunded_units, 7);
    let report = host.held_out_joint_report(10).unwrap().unwrap().clone();
    assert!(report.qualified()); assert_eq!(report.candidate.members["a"].weight, 5);
    assert_eq!(report.work, HeldOutJointBudget { cases: 3, member_outcomes: 12 });
    assert_eq!(host.inspect().ledger.charged, 7);
    assert_eq!(host.inspect().ledger.stages[&3], ActionState::Unknown);
    assert!(host.dispatch(&old_key, &pending, &state()).is_err());
    let (fresh, key) = authorized(&mut host, 5);
    let before = host.inspect();
    assert_eq!(host.activate_credibility(activation).unwrap(), receipt);
    assert_eq!(host.inspect(), before);
    host.dispatch(&key, &fresh, &state()).unwrap();
    host.record_trusted_outcome(3, TrustedOutcome::NotExecuted).unwrap();
    assert_eq!(host.inspect().ledger.charged, 7);
    let inspection = host.inspect();
    host.withdraw_credibility(CredibilityWithdrawalRequest { operation: 20,
        expected_control_sequence: inspection.sequence, expected_epoch: inspection.ledger.epoch }).unwrap();
    assert_eq!(host.held_out_joint_report(10).unwrap(), Some(&report));
    assert_eq!(host.held_out_joint_policy(), Some(guard()));
}

#[test]
fn bootstrap_guard_cannot_be_installed_late_or_disabled_by_using_legacy_activation() {
    let mut legacy = controller(5, None); seed(&mut legacy);
    assert_eq!(legacy.enable_held_out_joint(guard()), Err(Error::WrongState));
    let activation = request(&legacy, 10, 2);
    legacy.activate_credibility(activation).unwrap(); // Explicit unchanged legacy control.
    assert_eq!(legacy.held_out_joint_report(10).unwrap(), None);
    let mut guarded = controller(5, Some(guard()));
    assert_eq!(guarded.enable_held_out_joint(guard()), Err(Error::Duplicate));
    seed(&mut guarded);
    let before = guarded.inspect();
    assert_eq!(guarded.activate_credibility(request(&guarded, 10, 2)), Err(Error::Incomplete));
    assert_eq!(guarded.inspect(), before);
}

#[test]
fn complete_budget_preflight_and_candidate_bindings_refuse_without_comparison() {
    let before = config(0).congress; let after = candidate(&before); let data = evidence(&rows());
    for budget in [HeldOutJointBudget { cases: 2, member_outcomes: 12 },
        HeldOutJointBudget { cases: 3, member_outcomes: 11 }] {
        let policy = HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0, budget).unwrap();
        assert_eq!(joint::evaluate(policy, &before, &after, &data), Err(Error::Limit));
    }
    let exact = HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0, HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap();
    assert!(joint::evaluate(exact, &before, &after, &data).unwrap().qualified());
    for changed in 0..3 {
        let mut other = after.clone();
        match changed { 0 => other.continue_minimum += 1,
            1 => other.members.get_mut("a").unwrap().cohort = "other".into(),
            _ => other.generation = before.generation }
        assert_eq!(joint::evaluate(guard(), &before, &other, &data), Err(Error::Binding));
    }
}

#[test]
fn incomplete_and_late_observations_cannot_be_turned_into_successful_holds() {
    let before = config(0).congress; let after = candidate(&before);
    for missing in 0..5 {
        let mut cases = rows();
        match missing { 0 => cases[0].truth = None, 1 => cases[0].truth = Some(LabelVerdict::Censored),
            2 => cases[0].votes[0] = Observation::Missing, 3 => cases[0].votes[0] = Observation::Abstain,
            _ => cases[0].votes[0] = Observation::Hold { first_sequence: 2 } }
        assert_eq!(joint::evaluate(guard(), &before, &after, &evidence(&cases)), Err(Error::Incomplete));
    }
    assert!(joint::evaluate(guard(), &before, &after, &evidence(&rows())).unwrap().qualified());
}

#[test]
fn repeated_evidence_roots_count_once_and_conflicting_labels_or_strata_refuse() {
    let before = config(0).congress; let after = candidate(&before);
    let mut cases = rows(); cases[1].root = 1;
    let report = joint::evaluate(guard(), &before, &after, &evidence(&cases)).unwrap();
    assert_eq!(report.strata["effect"].candidate.violation_roots, 1);
    assert!(report.failures.contains(&HeldOutJointFailure::InsufficientViolationRoots("effect".into())));
    cases[1].truth = Some(LabelVerdict::Safe);
    assert_eq!(joint::evaluate(guard(), &before, &after, &evidence(&cases)), Err(Error::Binding));
    cases[1].truth = Some(LabelVerdict::Violation); cases[1].stratum = "other";
    assert_eq!(joint::evaluate(guard(), &before, &after, &evidence(&cases)), Err(Error::Binding));
}

#[test]
fn permissive_aggregate_limits_never_waive_a_new_case_regression() {
    let before = config(5).congress; let after = candidate(&before);
    let policy = HeldOutJointPolicy::new(71, 1, 1, 1, 1_000_000, 1_000_000, HeldOutJointBudget::default()).unwrap();
    let mut cases = rows(); cases[0].votes = [CLEAR, CLEAR]; cases[1].root = cases[0].root;
    let report = joint::evaluate(policy, &before, &after, &evidence(&cases)).unwrap();
    assert!(report.roots[&[1; 32]].baseline_failed);
    assert!(report.failures.contains(&HeldOutJointFailure::RegressedCase(2)));
    assert!(!report.qualified()); // The already-failed replicate cannot hide it.
}

#[test]
fn rate_ceilings_are_per_stratum_not_pooled_across_easy_cases() {
    let before = config(0).congress; let after = candidate(&before);
    let policy = HeldOutJointPolicy::new(71, 1, 1, 2, 500_000, 0, HeldOutJointBudget::default()).unwrap();
    let mut cases = rows();
    cases.extend(rows().into_iter().map(|mut row| { row.root += 10; row.stratum = "hard"; row }));
    cases[3].votes = [CLEAR, CLEAR];
    let paired = joint::evaluate(policy, &before, &after, &evidence(&cases)).unwrap();
    assert!(paired.qualified()); // Exactly 1/2 escaped in the hard stratum.
    cases[4].votes = [CLEAR, CLEAR];
    let report = joint::evaluate(policy, &before, &after, &evidence(&cases)).unwrap();
    assert_eq!(report.strata["effect"].candidate.escaped_roots, 0);
    assert!(report.failures.contains(&HeldOutJointFailure::EscapeRate("hard".into())));
}

#[test]
fn new_false_stops_are_regressions_even_when_violation_detection_improves() {
    let mut before = config(5).congress;
    for entry in before.members.values_mut() { entry.weight = 5; }
    let mut after = before.clone(); after.generation = 2;
    for entry in after.members.values_mut() { entry.weight = 10; }
    let mut cases = rows(); cases[2].votes = [HOLD, CLEAR];
    let loose = HeldOutJointPolicy::new(71, 1, 1, 2, 1_000_000, 1_000_000,
        HeldOutJointBudget::default()).unwrap();
    let report = joint::evaluate(loose, &before, &after, &evidence(&cases)).unwrap();
    assert_eq!(report.strata["effect"].baseline.false_stopped_roots, 0);
    assert_eq!(report.strata["effect"].candidate.false_stopped_roots, 1);
    assert!(report.failures.contains(&HeldOutJointFailure::RegressedCase(3)));
    assert!(!report.qualified());
}

#[test]
fn native_comparison_uses_admitted_capped_weights_not_nominal_scores() {
    let mut cfg = config(0); cfg.congress.caps = Caps { per_member: 4, per_cohort: 4 };
    let mut host = PolicyAuthority::new(cfg).unwrap();
    host.enable_held_out_joint(guard()).unwrap(); host.observe_time(ElapsedTick(1)).unwrap();
    seed(&mut host);
    host.activate_credibility(request(&host, 10, 2)).unwrap();
    let report = host.held_out_joint_report(10).unwrap().unwrap();
    assert_eq!(report.baseline.members["a"].weight, 10);
    assert_eq!(report.candidate.members["a"].weight, 4);
    assert!(report.qualified());
    let (action, permit) = authorized(&mut host, 3);
    host.dispatch(&permit, &action, &state()).unwrap();
    assert_eq!(host.inspect().ledger.charged, 7);
}

#[cfg(unix)]
#[path = "held_out_joint/durable.rs"]
mod durable;
