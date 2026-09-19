use super::*;
use crate::action::consequence::congress::MemberPolicy;
use crate::reducer::Caps;
use crate::round::Verdict;

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3,
        minimum_violation_origins: 2, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 100 }
}
fn congress(hold_maximum: u64) -> CongressPolicy {
    CongressPolicy { generation: 1,
        members: BTreeMap::from([("a".into(), MemberPolicy { cohort: "a".into(), weight: 10 }),
            ("b".into(), MemberPolicy { cohort: "b".into(), weight: 10 })]),
        caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 5,
        continue_hold_maximum: hold_maximum, narrow_at: 20, suspend_at: 30,
        minimum_members: 2, minimum_cohorts: 2 }
}
fn next(current: &CongressPolicy) -> CongressPolicy {
    let mut next = current.clone(); next.generation += 1;
    for member in next.members.values_mut() { member.weight = 5; }
    next
}
fn policy() -> JointPromotionPolicy {
    JointPromotionPolicy::new(7, 1, Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 0, denominator: 1 }, JointReplayBudget::default()).unwrap()
}
fn row(round: u64, origin: u64, truth: GroundTruth, a: MemberOutcome, b: MemberOutcome)
    -> (EvaluationCase, Option<Assessment>)
{
    (EvaluationCase { round, attempt: round, policy_generation: 1, congress_generation: 1,
        evidence_root: [1; 32], verdicts: BTreeMap::from([("a".into(), a), ("b".into(), b)]),
        stopped: false }, Some(Assessment { origin, evidence_id: [2; 32], truth }))
}
fn votes(round: u64, origin: u64, truth: GroundTruth, a: Verdict, b: Verdict)
    -> (EvaluationCase, Option<Assessment>)
{
    row(round, origin, truth, MemberOutcome::Revealed(a), MemberOutcome::Revealed(b))
}
fn complementary() -> Vec<(EvaluationCase, Option<Assessment>)> {
    vec![votes(1, 1, GroundTruth::Violation, Verdict::Hold, Verdict::Allow),
        votes(2, 2, GroundTruth::Violation, Verdict::Allow, Verdict::Hold),
        votes(3, 3, GroundTruth::Benign, Verdict::Allow, Verdict::Allow)]
}
fn run(policy: JointPromotionPolicy, current: &CongressPolicy,
    rows: Vec<(EvaluationCase, Option<Assessment>)>) -> Result<JointPromotionReport, Error>
{
    replay(policy, ReplayBasis { protocol: &protocol(), revision: 6, policy_generation: 1,
        current, candidate: &next(current) }, rows.into_iter().map(Ok))
}

#[test]
fn actual_joint_outcomes_separate_equal_marginals_without_an_independence_assumption() {
    let independent = run(policy(), &congress(0), complementary()).unwrap();
    assert!(independent.qualified());
    assert_eq!(independent.candidate_counts.escaped_violation_origins, 0);
    let correlated = vec![votes(1, 1, GroundTruth::Violation, Verdict::Hold, Verdict::Hold),
        votes(2, 2, GroundTruth::Violation, Verdict::Allow, Verdict::Allow),
        votes(3, 3, GroundTruth::Benign, Verdict::Allow, Verdict::Allow)];
    let correlated = run(policy(), &congress(0), correlated).unwrap();
    // Each helper detects one of two violations in BOTH datasets. The joint
    // result is nevertheless 0/2 versus 1/2, not a product of the marginal rates.
    assert_eq!(correlated.candidate_counts.violation_origins, 2);
    assert_eq!(correlated.candidate_counts.escaped_violation_origins, 1);
    assert!(correlated.failures.contains(&JointFailure::EscapeRate));
    assert_eq!(correlated.work, JointReplayBudget { cases: 3, member_outcomes: 12 });
}

#[test]
fn permissive_rate_ceilings_cannot_waive_a_new_joint_escape() {
    let permissive = JointPromotionPolicy::new(7, 1, Fraction { numerator: 1, denominator: 1 },
        Fraction { numerator: 1, denominator: 1 }, JointReplayBudget::default()).unwrap();
    let report = run(permissive, &congress(5), complementary()).unwrap();
    assert_eq!(report.baseline_counts.escaped_violation_origins, 0);
    assert_eq!(report.candidate_counts.escaped_violation_origins, 2);
    assert_eq!(report.failures, vec![JointFailure::RegressedOrigin(1), JointFailure::RegressedOrigin(2)]);
    assert!(!report.qualified());
    assert!(run(permissive, &congress(0), complementary()).unwrap().qualified());
}

#[test]
fn per_replicate_regression_is_not_hidden_by_worst_origin_grouping() {
    let mut rows = complementary();
    rows[0] = votes(1, 1, GroundTruth::Violation, Verdict::Allow, Verdict::Allow);
    rows.push(votes(4, 1, GroundTruth::Violation, Verdict::Hold, Verdict::Allow));
    let report = run(policy(), &congress(5), rows).unwrap();
    let origin = report.origins[&1];
    assert_eq!(origin.rounds, 2);
    assert!(origin.baseline_failed && origin.candidate_failed && origin.regressed);
    assert_eq!(report.candidate_counts.violation_origins, 2);
    assert!(report.failures.contains(&JointFailure::RegressedOrigin(1)));
}

#[test]
fn missing_abstaining_pending_and_censored_cases_cannot_become_successful_samples() {
    for unknown in [MemberOutcome::Missing, MemberOutcome::Revealed(Verdict::Abstain)] {
        let mut rows = complementary();
        rows[2] = row(3, 3, GroundTruth::Benign, MemberOutcome::Revealed(Verdict::Allow), unknown);
        let report = run(policy(), &congress(0), rows).unwrap();
        assert_eq!(report.candidate_counts.stopped_benign_origins, 1);
        assert!(report.failures.contains(&JointFailure::BenignStopRate));
    }
    let mut pending = complementary(); pending[0].1 = None;
    let report = run(policy(), &congress(0), pending).unwrap();
    assert_eq!(report.pending_cases, 1);
    assert!(report.failures.contains(&JointFailure::PendingLabels));
    assert!(report.failures.contains(&JointFailure::InsufficientViolationOrigins));
    let mut censored = complementary(); censored[0].1.as_mut().unwrap().truth = GroundTruth::Censored;
    let report = run(policy(), &congress(0), censored).unwrap();
    assert_eq!(report.censored_cases, 1);
    assert!(report.failures.contains(&JointFailure::CensoredLabels));
}

#[test]
fn complete_budget_boundary_and_policy_stratum_are_enforced() {
    for (cases, outcomes, accepted) in [(3, 12, true), (2, 12, false), (3, 11, false)] {
        let guard = JointPromotionPolicy::new(7, 1, Fraction { numerator: 0, denominator: 1 },
            Fraction { numerator: 0, denominator: 1 }, JointReplayBudget { cases, member_outcomes: outcomes }).unwrap();
        let result = run(guard, &congress(0), complementary());
        if accepted { assert!(result.unwrap().qualified()); }
        else { assert_eq!(result.unwrap_err(), Error::Limit); }
    }
    let mut rows = complementary(); rows[0].0.policy_generation = 2;
    let report = run(policy(), &congress(0), rows).unwrap();
    assert_eq!(report.work.cases, 2);
    assert_eq!(report.candidate_counts.violation_origins, 1);
    assert!(report.failures.contains(&JointFailure::InsufficientViolationOrigins));
}

#[test]
fn thresholds_cohorts_roster_and_generations_cannot_hide_in_a_weight_candidate() {
    let current = congress(0);
    for mutation in 0..5 {
        let mut candidate = next(&current);
        match mutation {
            0 => candidate.continue_minimum = 1,
            1 => candidate.caps.per_cohort = 20,
            2 => candidate.members.get_mut("a").unwrap().cohort = "b".into(),
            3 => { candidate.members.remove("b"); }
            _ => candidate.generation = 1,
        }
        assert_eq!(replay(policy(), ReplayBasis { protocol: &protocol(), revision: 6,
            policy_generation: 1, current: &current, candidate: &candidate },
            complementary().into_iter().map(Ok)).unwrap_err(), Error::Binding);
    }
}

#[test]
fn duplicate_rounds_conflicting_truth_and_foreign_member_maps_refuse() {
    let mut duplicate = complementary(); duplicate.push(duplicate[0].clone());
    assert_eq!(run(policy(), &congress(0), duplicate).unwrap_err(), Error::Duplicate);
    let mut conflict = complementary(); conflict[2].1.as_mut().unwrap().origin = 1;
    assert_eq!(run(policy(), &congress(0), conflict).unwrap_err(), Error::Binding);
    let mut missing = complementary(); missing[0].0.verdicts.remove("a");
    assert_eq!(run(policy(), &congress(0), missing).unwrap_err(), Error::Binding);
    let mut zero = complementary(); zero[0].1.as_mut().unwrap().evidence_id = [0; 32];
    assert_eq!(run(policy(), &congress(0), zero).unwrap_err(), Error::InvalidInput);
}

#[test]
fn stop_flags_do_not_replace_empirical_votes_and_zero_denominators_never_pass() {
    let rows = complementary();
    let mut changed = rows.clone();
    for (case, _) in &mut changed { case.stopped = !case.stopped; }
    assert_eq!(run(policy(), &congress(0), rows).unwrap(), run(policy(), &congress(0), changed).unwrap());
    let empty = run(policy(), &congress(0), Vec::new()).unwrap();
    assert!(!empty.qualified());
    assert!(empty.failures.contains(&JointFailure::EscapeRate));
    assert!(empty.failures.contains(&JointFailure::BenignStopRate));
    assert!(within(1, 2, Fraction { numerator: u64::MAX / 2 + 1, denominator: u64::MAX }));
    assert!(!within(1, 2, Fraction { numerator: u64::MAX / 2, denominator: u64::MAX }));
}
