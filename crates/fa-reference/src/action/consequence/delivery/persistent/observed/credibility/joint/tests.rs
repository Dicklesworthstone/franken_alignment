//! Actual original reviews, separately issued labels, two keys and journal I/O.
//! Synthetic verdicts exercise enforcement, not trained-helper accuracy.
use super::*;
use super::super::{Assessment, FileCredibilityUpdate};
use super::super::super::{FileHumanReviewer, FileOversightProfile, journal};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{
    FileDeliveryProfile, FilePermit, JournalIo, JournalLimits, Reconciliation, RecoveryReserve, storage,
};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::credibility::GroundTruth;
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use crate::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use crate::action::consequence::oversight::joint_credibility::JointFailure;
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(std::env::temp_dir().join(format!("fa-joint-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.0.join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() && let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("joint journal fixture cleanup: {error}");
        }
    }
}
fn fraction(numerator: u64, denominator: u64) -> Fraction { Fraction { numerator, denominator } }
fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3,
        minimum_violation_origins: 2, minimum_benign_origins: 1,
        precision_floor: fraction(1, 2), recall_floor: fraction(1, 2),
        false_positive_ceiling: fraction(1, 2), false_stop_budget: 100 }
}
fn guard(budget: JointReplayBudget) -> JointPromotionPolicy {
    JointPromotionPolicy::new(71, 1, fraction(0, 1), fraction(0, 1), budget).unwrap()
}
fn profile(hold_maximum: u64) -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1,
        expected_version: 1, generation: 1 };
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap();
    let committee = CommitteeContract::new(["a", "b"].into_iter().map(|name| (name.to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: vec![],
            model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 }, 1, b"approve?".to_vec()).unwrap(),
    )).collect()).unwrap();
    FileOversightProfile {
        delivery: FileDeliveryProfile { scope, total: 100, max_attempts: 16, actor,
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: ["a", "b"].into_iter().map(|name|
                (name.to_owned(), MemberPolicy { cohort: name.to_owned(), weight: 10 })).collect(),
                caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 5,
                continue_hold_maximum: hold_maximum, narrow_at: 20, suspend_at: 30,
                minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 16, clock_domain: 99, limits: JournalLimits::default() },
        committee, human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 16 },
    }
}
fn ready(hold: u64, budget: JointReplayBudget)
    -> (Directory, FileOversight, FileHumanReviewer, FileIndependentEvaluator)
{
    let root = Directory::new();
    let (mut host, human) = FileOversight::create(&root.0, profile(hold)).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let evaluator = host.enable_joint_credibility(host.revision(), protocol(), guard(budget)).unwrap();
    (root, host, human, evaluator)
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn begin(host: &mut FileOversight, id: u64) -> (FrozenAction, CommitteeInput) {
    let now = ElapsedTick(id * 4);
    host.observe_time(host.revision(), now).unwrap();
    let p = profile(0);
    let action = host.propose(host.revision(), id, ActionSpec { version: VERSION,
        scope: p.delivery.scope, target: Some(host.inspect().target), payload: b"visible".to_vec(),
        required_witnesses: vec![], policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(1000), units: 16 }, snapshot()).unwrap();
    let source = EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: p.delivery.scope },
        snapshot(), ["a", "b"].into_iter().map(|name| (name.to_owned(), b"context".to_vec())).collect()).unwrap();
    let inputs = source.inputs_for(&action, &p.committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), id, id + 100, [id as u8; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 1), reveal_by: ElapsedTick(now.0 + 2) }, snapshot()).unwrap();
    (action, inputs)
}
fn review(host: &mut FileOversight, id: u64, a: Verdict, b: Verdict) -> (FrozenAction, CommitteeInput) {
    let result = begin(host, id);
    for (name, verdict) in [("a", a), ("b", b)] {
        let digest = commitment(id + 100, name, &[id as u8; 32], verdict, b"salt").unwrap();
        host.commit_review(host.revision(), id + 100, name, digest).unwrap();
    }
    host.open_reveals(host.revision(), id + 100).unwrap();
    for (name, verdict) in [("a", a), ("b", b)] {
        host.reveal_review(host.revision(), id + 100, name, verdict, b"salt".to_vec()).unwrap();
    }
    host.finish_review(host.revision(), id + 100, Some(&result.1), snapshot()).unwrap().unwrap();
    result
}
fn assess(host: &mut FileOversight, evaluator: &FileIndependentEvaluator, id: u64, origin: u64, truth: GroundTruth) {
    let ticket = host.evaluation_ticket(id + 100).unwrap();
    let revision = host.revision();
    evaluator.assess(host, revision, &ticket, Assessment { origin, evidence_id: [2; 32], truth }).unwrap();
}
fn calibrate(host: &mut FileOversight, evaluator: &FileIndependentEvaluator) {
    for (id, a, b, truth) in [
        (1, Verdict::Hold, Verdict::Allow, GroundTruth::Violation),
        (2, Verdict::Allow, Verdict::Hold, GroundTruth::Violation),
        (3, Verdict::Allow, Verdict::Allow, GroundTruth::Benign),
    ] {
        review(host, id, a, b);
        assess(host, evaluator, id, id, truth);
    }
    assert!(host.credibility_report().unwrap().qualified());
}
fn update(host: &FileOversight, operation: u64) -> FileCredibilityUpdate {
    let control = host.inspect().control;
    FileCredibilityUpdate { operation, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch,
        expected_evaluation_revision: host.credibility_report().unwrap().revision }
}
fn dispatch(host: &mut FileOversight, human: &FileHumanReviewer, id: u64,
    action: &FrozenAction, inputs: &CommitteeInput) -> FilePermit
{
    let key = host.authorize(host.revision(), id, inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1000 + id, id, inputs, ElapsedTick(id * 4 + 30)).unwrap();
    let revision = host.revision();
    let second = human.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &key, &second, action, inputs, snapshot()).unwrap();
    key
}

#[test]
fn marginally_qualified_joint_regression_is_rejected_before_and_after_reopen() {
    let (root, mut host, _, evaluator) = ready(5, JointReplayBudget::default());
    calibrate(&mut host, &evaluator);
    let request = update(&host, 41);
    let report = host.joint_credibility_report(request.expected_evaluation_revision).unwrap();
    assert_eq!(report.baseline_counts.escaped_violation_origins, 0);
    assert_eq!(report.candidate_counts.escaped_violation_origins, 2);
    assert_eq!(report.candidate.members["a"].weight, 5);
    assert!(report.failures.contains(&JointFailure::RegressedOrigin(1)));
    assert!(!report.qualified());
    let before = host.inspect();
    let bytes = root.bytes();
    assert_eq!(host.promote_credibility(host.revision(), &request), Err(Error::Incomplete.into()));
    assert_eq!(host.inspect(), before);
    assert_eq!(root.bytes(), bytes);
    drop(host);
    let (mut host, _) = FileOversight::open(&root.0, profile(5)).unwrap();
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(report.policy));
    assert_eq!(host.joint_credibility_report(request.expected_evaluation_revision).unwrap(), report);
    host.observe_time(host.revision(), ElapsedTick(50)).unwrap();
    let request = update(&host, 41);
    let bytes = root.bytes();
    assert_eq!(host.promote_credibility(host.revision(), &request), Err(Error::Incomplete.into()));
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn accepted_joint_promotion_retains_its_basis_and_publishes_with_both_fresh_keys() {
    let (root, mut host, human, evaluator) = ready(0, JointReplayBudget::default());
    calibrate(&mut host, &evaluator);
    let request = update(&host, 41);
    let report = host.joint_credibility_report(request.expected_evaluation_revision).unwrap();
    assert!(report.qualified());
    let receipt = host.promote_credibility(host.revision(), &request).unwrap();
    assert_eq!(receipt.change.current.members["a"].weight, 5);
    assert_eq!(host.joint_credibility_promotion(41).unwrap(), &report);
    let (action, inputs) = review(&mut host, 4, Verdict::Allow, Verdict::Allow);
    let before = host.inspect();
    let bytes = root.bytes();
    assert_eq!(host.promote_credibility(0, &request).unwrap(), receipt);
    assert_eq!(root.bytes(), bytes);
    assert_eq!(host.inspect(), before);
    dispatch(&mut host, &human, 4, &action, &inputs);
    let publication = host.publish_checked(host.revision(), 4, Some(&inputs), snapshot(), ElapsedTick(17)).unwrap();
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 4).unwrap();
    drop(host);
    let (host, _) = FileOversight::open(&root.0, profile(0)).unwrap();
    assert_eq!(host.joint_credibility_promotion(41).unwrap(), &report);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(!host.clock_ready());
}

#[test]
fn promotion_preserves_unresolved_liability_and_only_the_endpoint_settles_it() {
    for executed in [false, true] {
        let (root, mut host, human, evaluator) = ready(0, JointReplayBudget::default());
        calibrate(&mut host, &evaluator);
        let (action, inputs) = review(&mut host, 4, Verdict::Allow, Verdict::Allow);
        assess(&mut host, &evaluator, 4, 4, GroundTruth::Benign);
        dispatch(&mut host, &human, 4, &action, &inputs);
        if executed { host.publish_checked(host.revision(), 4, Some(&inputs), snapshot(), ElapsedTick(17)).unwrap(); }
        let (pending, current) = review(&mut host, 5, Verdict::Allow, Verdict::Allow);
        assess(&mut host, &evaluator, 5, 5, GroundTruth::Benign);
        let old_key = host.authorize(host.revision(), 5, &current, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 1005, 5, &current, ElapsedTick(50)).unwrap();
        let revision = host.revision();
        let old_human = human.approve(&mut host, revision, &request).unwrap();
        let update = update(&host, 41);
        let result = host.promote_credibility(host.revision(), &update).unwrap();
        assert_eq!(result.change.refunded_units, 16);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.human_status(1005).unwrap().disposition, HumanDisposition::Revoked);
        assert!(host.dispatch(host.revision(), &old_key, &old_human, &pending, &current, snapshot()).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open(&root.0, profile(0)).unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&4], ActionState::Unknown);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        host.observe_time(host.revision(), ElapsedTick(30)).unwrap();
        let outcome = host.reconcile(host.revision(), 4).unwrap();
        if executed {
            assert_eq!(outcome, Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
            assert_eq!(host.inspect().control.ledger.charged, 16);
        } else {
            assert_eq!(outcome, Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            host.seal_unexecuted(host.revision(), 4).unwrap();
            assert_eq!(host.inspect().control.ledger.available, 100);
        }
    }
}

#[test]
fn duplicate_late_and_incomplete_bootstrap_cannot_replace_or_downgrade_the_guard() {
    let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
    let bytes = root.bytes();
    assert!(host.enable_credibility(host.revision(), protocol()).is_err());
    assert!(host.enable_joint_credibility(host.revision(), protocol(), guard(JointReplayBudget::default())).is_err());
    assert_eq!(root.bytes(), bytes);
    calibrate(&mut host, &evaluator);
    begin(&mut host, 4); // Unfinished current-policy case cannot disappear.
    let request = update(&host, 41);
    assert_eq!(host.joint_credibility_report(request.expected_evaluation_revision), Err(Error::Incomplete.into()));
    assert_eq!(host.promote_credibility(host.revision(), &request), Err(Error::Incomplete.into()));
    let other = Directory::new();
    let (mut legacy, _) = FileOversight::create(&other.0, profile(0)).unwrap();
    begin(&mut legacy, 1);
    let bytes = other.bytes();
    assert!(legacy.enable_joint_credibility(legacy.revision(), protocol(), guard(JointReplayBudget::default())).is_err());
    assert_eq!(other.bytes(), bytes);
    assert_eq!(legacy.joint_credibility_policy().unwrap(), None);
}

#[test]
fn joint_budget_exhaustion_never_falls_back_to_marginal_qualification() {
    let budget = JointReplayBudget { cases: 3, member_outcomes: 11 };
    let (root, mut host, _, evaluator) = ready(0, budget);
    calibrate(&mut host, &evaluator);
    let request = update(&host, 41);
    let before = root.bytes();
    assert_eq!(host.joint_credibility_report(request.expected_evaluation_revision), Err(Error::Limit.into()));
    assert_eq!(host.promote_credibility(host.revision(), &request), Err(Error::Limit.into()));
    assert_eq!(root.bytes(), before);
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(guard(budget)));
}

#[test]
fn unconfigured_archives_keep_their_original_marginal_policy_without_a_joint_claim() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create(&root.0, profile(5)).unwrap();
    let evaluator = host.enable_credibility(host.revision(), protocol()).unwrap();
    calibrate(&mut host, &evaluator);
    let request = update(&host, 41);
    assert_eq!(host.joint_credibility_report(request.expected_evaluation_revision), Err(Error::Incomplete.into()));
    host.promote_credibility(host.revision(), &request).unwrap();
    assert_eq!(host.joint_credibility_promotion(41), Err(Error::Incomplete.into()));
    let bytes = root.bytes();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    assert_eq!(journal::encode(&host.profile, host.store.identity(), &events).unwrap(), bytes);
    drop(host);
    let (host, _) = FileOversight::open(&root.0, profile(5)).unwrap();
    assert_eq!(host.joint_credibility_policy().unwrap(), None);
}

#[test]
fn failed_promotion_writes_never_return_a_report_or_restore_old_keys() {
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
        calibrate(&mut host, &evaluator);
        let request = update(&host, 41);
        let report = host.joint_credibility_report(request.expected_evaluation_revision).unwrap();
        let before = host.inspect();
        host.store.fail_once(stage);
        assert!(matches!(host.promote_credibility(host.revision(), &request), Err(JournalError::Io(_))));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.joint_credibility_promotion(41), Err(JournalError::Unavailable));
        assert_eq!(host.joint_credibility_policy(), Err(JournalError::Unavailable));
        drop(host);
        let (mut host, _) = FileOversight::open(&root.0, profile(0)).unwrap();
        assert!(!host.clock_ready());
        if stage == JournalIo::DirectorySync {
            assert_eq!(host.joint_credibility_promotion(41).unwrap(), &report);
            let before = host.inspect();
            host.promote_credibility(0, &request).unwrap();
            assert_eq!(host.inspect(), before);
        } else {
            assert_eq!(host.joint_credibility_promotion(41), Err(Error::Missing.into()));
            host.observe_time(host.revision(), ElapsedTick(50)).unwrap();
            let request = update(&host, 41);
            host.promote_credibility(host.revision(), &request).unwrap();
            assert_eq!(host.joint_credibility_promotion(41).unwrap(), &report);
        }
    }
}
