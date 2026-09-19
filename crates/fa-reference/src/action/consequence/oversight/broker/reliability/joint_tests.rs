//! Full native proposal/whole-input congress/independent-label/promotion path.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::MemberPolicy;
use crate::action::consequence::delivery::PublicationEndpoint;
use crate::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade, RestartProfile}};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate, controller::ControllerConfig};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::credibility::{Fraction, GroundTruth};
use crate::action::consequence::oversight::joint_credibility::{JointFailure, JointReplayBudget};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;

fn guard(budget: JointReplayBudget) -> JointPromotionPolicy {
    JointPromotionPolicy::new(71, 1, Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 0, denominator: 1 }, budget).unwrap()
}
fn setup(hold_maximum: u64, joint: Option<JointPromotionPolicy>)
    -> (OversightBroker, IndependentEvaluator, PublicationEndpoint)
{
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap();
    let members = BTreeMap::from([("a".into(), MemberPolicy { cohort: "a".into(), weight: 10 }),
        ("b".into(), MemberPolicy { cohort: "b".into(), weight: 10 })]);
    let contracts = CommitteeContract::new(["a", "b"].into_iter().map(|name| (name.to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: vec![],
            tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 1, b"approve?".to_vec()).unwrap())).collect()).unwrap();
    let mut endpoint = PublicationEndpoint::new(target, b"initial".to_vec(), 1000, 8).unwrap();
    let mut host = OversightBroker::new(ControllerConfig { scope, total: 100, max_attempts: 8,
        actor, suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members, caps: Caps { per_member: 10, per_cohort: 10 },
            continue_minimum: 5, continue_hold_maximum: hold_maximum, narrow_at: 20,
            suspend_at: 30, minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[target]).unwrap() }, &mut endpoint, contracts).unwrap();
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    let evaluator = host.enable_credibility(EvaluationProtocol { domain: 1, stratum: 2, period: 3,
        minimum_violation_origins: 2, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 100 }).unwrap();
    if let Some(policy) = joint { host.enable_joint_credibility(policy).unwrap(); }
    (host, evaluator, endpoint)
}
fn review(host: &mut OversightBroker, evaluator: &IndependentEvaluator, id: u64, a: Verdict, b: Verdict, truth: GroundTruth) {
    let now = ElapsedTick(id * 4);
    host.observe_time(now).unwrap();
    let source = EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: host.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() },
        BTreeMap::from([("a".into(), b"context".to_vec()), ("b".into(), b"context".to_vec())])).unwrap();
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    let action = host.propose(id, ActionSpec { version: VERSION, scope: host.scope, target: Some(target),
        payload: b"output".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }, source.snapshot()).unwrap().action;
    let inputs = source.inputs_for(&action, host.contracts()).unwrap();
    host.record_inputs(id, 0, inputs.clone()).unwrap();
    let round = id + 100;
    let mut session = host.begin_review(id, round, source.reference_root(), ReviewWindow {
        commit_by: ElapsedTick(now.0 + 1), reveal_by: ElapsedTick(now.0 + 2) }, source.snapshot()).unwrap();
    for (name, verdict) in [("a", a), ("b", b)] {
        let digest = session.commitment(name, verdict, b"actual-review-salt").unwrap();
        session.commit(name, digest, now).unwrap();
    }
    session.open_reveals(now).unwrap();
    for (name, verdict) in [("a", a), ("b", b)] {
        session.reveal(name, verdict, b"actual-review-salt", now).unwrap();
    }
    let finished = session.finish(now).unwrap();
    host.apply_review(finished, Some(&inputs), source.snapshot()).unwrap();
    let ticket = host.evaluation_ticket(round).unwrap();
    let label = evaluator.assess(&ticket, Assessment { origin: id, evidence_id: [2; 32], truth }).unwrap();
    host.record_evaluation(label).unwrap();
}
fn calibrate(host: &mut OversightBroker, evaluator: &IndependentEvaluator) {
    review(host, evaluator, 1, Verdict::Hold, Verdict::Allow, GroundTruth::Violation);
    review(host, evaluator, 2, Verdict::Allow, Verdict::Hold, GroundTruth::Violation);
    review(host, evaluator, 3, Verdict::Allow, Verdict::Allow, GroundTruth::Benign);
    assert!(host.credibility_report().unwrap().qualified());
}

#[test]
fn joint_regression_blocks_the_original_promotion_before_any_control_mutation() {
    let (mut host, evaluator, endpoint) = setup(5, Some(guard(JointReplayBudget::default())));
    calibrate(&mut host, &evaluator);
    let report = host.credibility_report().unwrap();
    let preview = host.joint_credibility_report(report.revision).unwrap();
    assert_eq!(preview.baseline_counts.escaped_violation_origins, 0);
    assert_eq!(preview.candidate_counts.escaped_violation_origins, 2);
    assert_eq!(preview.candidate.members["a"].weight, 5);
    assert!(preview.failures.contains(&JointFailure::RegressedOrigin(1)));
    let before = host.inspect();
    assert_eq!(host.promote_credibility(before.sequence, before.ledger.epoch, report.revision), Err(Error::Incomplete));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.credibility_report().unwrap(), report);
    assert!(host.credibility_promotions().unwrap().is_empty());
    assert!(host.joint_credibility_promotions().unwrap().is_empty());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn paired_permitted_configuration_reweights_and_retains_the_joint_basis() {
    let (mut host, evaluator, endpoint) = setup(0, Some(guard(JointReplayBudget::default())));
    calibrate(&mut host, &evaluator);
    let revision = host.credibility_report().unwrap().revision;
    let preview = host.joint_credibility_report(revision).unwrap();
    assert!(preview.qualified());
    let before = host.inspect();
    let result = host.promote_credibility(before.sequence, before.ledger.epoch, revision).unwrap();
    assert_eq!(result.change.current.members["a"].weight, 5);
    assert_eq!(result.change.current.members["b"].weight, 5);
    assert!(result.change.revocation_floor > before.ledger.epoch);
    assert_eq!(host.joint_credibility_promotions().unwrap(), &[preview]);
    assert_eq!(host.credibility_promotions().unwrap().len(), 1);
    assert_eq!(host.inspect().ledger.charged, before.ledger.charged);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(host.joint_credibility_report(revision), Err(Error::Stale));
}

#[test]
fn legacy_profiles_keep_their_original_semantics_without_implicitly_claiming_joint_validation() {
    let (mut host, evaluator, _) = setup(5, None);
    calibrate(&mut host, &evaluator);
    let revision = host.credibility_report().unwrap().revision;
    assert_eq!(host.joint_credibility_report(revision), Err(Error::Incomplete));
    let before = host.inspect();
    host.promote_credibility(before.sequence, before.ledger.epoch, revision).unwrap();
    assert!(host.joint_credibility_promotions().unwrap().is_empty());
    assert_eq!(host.joint_credibility_policy(), None);
}

#[test]
fn guard_cannot_be_changed_after_bootstrap_and_exhaustion_does_not_promote() {
    let limited = guard(JointReplayBudget { cases: 3, member_outcomes: 11 });
    let (mut host, evaluator, _) = setup(0, Some(limited));
    assert_eq!(host.enable_joint_credibility(guard(JointReplayBudget::default())), Err(Error::Duplicate));
    calibrate(&mut host, &evaluator);
    assert_eq!(host.enable_joint_credibility(guard(JointReplayBudget::default())), Err(Error::WrongState));
    let before = host.inspect(); let revision = host.credibility_report().unwrap().revision;
    assert_eq!(host.joint_credibility_report(revision), Err(Error::Limit));
    assert_eq!(host.promote_credibility(before.sequence, before.ledger.epoch, revision), Err(Error::Limit));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.joint_credibility_policy(), Some(limited));
}
