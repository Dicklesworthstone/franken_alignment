//! Actual two-key publication and restart using the original filesystem journal.
use super::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit, JournalLimits, Reconciliation, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanReviewer, FileOversight, FileOversightProfile};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow};
use fa_reference::action::consequence::oversight::credibility::{EvaluationProtocol, Fraction};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::round::commitment;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("fa-held-out-joint-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() && let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("held-out joint cleanup: {error}"); }
    }
}
fn profile(hold_maximum: u64) -> FileOversightProfile {
    let config = config(hold_maximum);
    FileOversightProfile {
        delivery: FileDeliveryProfile { scope: config.scope, total: config.total, max_attempts: config.max_attempts,
            actor: config.actor, suspend_at_incident: config.suspend_at_incident, policy: config.policy,
            congress: config.congress, narrowed_targets: vec![spec(0).target.unwrap()], target: spec(0).target.unwrap(),
            initial_payload: Vec::new(), retention_ticks: 200, max_deliveries: 32, clock_domain: 1,
            limits: JournalLimits::default() },
        committee: CommitteeContract::new(["a", "b"].into_iter().map(|m| (m.into(),
            HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: vec![],
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 1, b"approve?".to_vec()).unwrap())).collect()).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 100, max_requests: 32 },
    }
}
fn reviewed(host: &mut FileOversight, p: &FileOversightProfile, id: u64) -> (FrozenAction, CommitteeInput) {
    let mut proposal = spec(host.inspect().control.ledger.epoch); proposal.target = Some(host.inspect().target);
    let action = host.propose(host.revision(), id, proposal, state()).unwrap();
    let source = EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: p.delivery.scope },
        state(), ["a", "b"].into_iter().map(|m| (m.into(), b"context".to_vec())).collect()).unwrap();
    let inputs = source.inputs_for(&action, &p.committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = 100 + id;
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, state()).unwrap();
    for member in ["a", "b"] {
        host.commit_review(host.revision(), round, member,
            commitment(round, member, &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    }
    host.open_reveals(host.revision(), round).unwrap();
    for member in ["a", "b"] { host.reveal_review(host.revision(), round, member, Verdict::Allow, b"salt".to_vec()).unwrap(); }
    host.finish_review(host.revision(), round, Some(&inputs), state()).unwrap().unwrap();
    (action, inputs)
}
fn prepared(host: &mut FileOversight, human: &FileHumanReviewer, p: &FileOversightProfile, id: u64)
    -> (FrozenAction, CommitteeInput, FilePermit, FileHumanPermit)
{
    let (action, inputs) = reviewed(host, p, id);
    let automatic = host.authorize(host.revision(), id, &inputs, state()).unwrap();
    let request = host.request_human_approval(host.revision(), 1000 + id, id, &inputs, ElapsedTick(80)).unwrap();
    let revision = host.revision();
    let key = human.approve(host, revision, &request).unwrap();
    (action, inputs, automatic, key)
}
fn activation(host: &FileOversight, p: &FileOversightProfile, operation: u64, generation: u64) -> CredibilityActivation {
    let snapshot = evidence(&rows());
    CredibilityActivation { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch, scope: p.delivery.scope,
        policy_generation: p.delivery.policy.generation(), actor_profile: p.delivery.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(), reducer_generation: generation },
        stratum: "effect".into(), requirements: CredibilityRequirements {
            minimum_safe_cases: 1, minimum_violation_cases: 2, minimum_precision_ppm: 1_000_000,
            minimum_timely_recall_ppm: 500_000, maximum_false_positive_ppm: 0, base_weight: 10,
            lead_bonus_weight: 0, lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}
fn ready(hold_maximum: u64) -> (Directory, FileOversight, FileHumanReviewer, FileOversightProfile) {
    let root = Directory::new(); let p = profile(hold_maximum);
    let (mut host, human) = FileOversight::create_with_held_out_joint(&root.0, p.clone(), guard()).unwrap();
    assert_eq!(host.revision(), 1);
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for id in 1..=2 { reviewed(&mut host, &p, id); host.cancel(host.revision(), id).unwrap(); }
    (root, host, human, p)
}
fn publish(host: &mut FileOversight, human: &FileHumanReviewer, p: &FileOversightProfile, id: u64) {
    let (action, inputs, automatic, key) = prepared(host, human, p, id);
    host.dispatch(host.revision(), &automatic, &key, &action, &inputs, state()).unwrap();
    let result = host.publish_checked(host.revision(), id, Some(&inputs), state(), ElapsedTick(2)).unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    host.reconcile(host.revision(), id).unwrap();
}

#[test]
fn durable_activation_pairs_joint_refusal_with_real_two_key_publication() {
    for hold_maximum in [5, 0] {
        let (root, mut host, human, p) = ready(hold_maximum);
        let request = activation(&host, &p, 10, 2);
        let before = host.inspect(); let bytes = root.bytes();
        let outcome = host.activate_credibility(host.revision(), request, &p.committee);
        if hold_maximum == 5 {
            assert_eq!(outcome, Err(Error::Incomplete.into()));
            assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
            assert!(host.held_out_joint_report(10).is_err());
        } else {
            outcome.unwrap();
            let report = host.held_out_joint_report(10).unwrap().unwrap().clone();
            assert!(report.qualified()); assert_eq!(report.candidate.members["a"].weight, 5);
            publish(&mut host, &human, &p, 3);
            assert_eq!(host.inspect().executions, 1);
            assert_eq!(host.inspect().control.ledger.charged, 7);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            let bytes = root.bytes();
            let read = FileOversight::read_held_out_joint(&root.0, &p, guard()).unwrap();
            assert_eq!(read.journal, host.inspect()); assert_eq!(read.promotions, vec![(10, report)]);
            assert_eq!(root.bytes(), bytes); // The writable owner still holds its lock.
        }
    }
}

#[test]
fn recovery_preserves_guard_and_history_but_requires_new_qualification_and_keys() {
    for executed in [false, true] {
        let (root, mut host, human, p) = ready(0);
        let first = activation(&host, &p, 10, 2);
        let receipt = host.activate_credibility(host.revision(), first.clone(), &p.committee).unwrap();
        let (action, inputs, old_auto, old_human) = prepared(&mut host, &human, &p, 3);
        host.dispatch(host.revision(), &old_auto, &old_human, &action, &inputs, state()).unwrap();
        if executed { host.publish_checked(host.revision(), 3, Some(&inputs), state(), ElapsedTick(2)).unwrap(); }
        let report = host.held_out_joint_report(10).unwrap().unwrap().clone();
        drop(host);
        let (mut host, human) = FileOversight::open_with_held_out_joint(&root.0, p.clone(), guard()).unwrap();
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(guard()));
        assert_eq!(host.held_out_joint_report(10).unwrap(), Some(&report));
        assert_eq!(host.check_credibility(), Err(Error::Stale.into())); assert!(!host.clock_ready());
        assert_eq!(host.inspect().control.ledger.charged, 7);
        let before = host.inspect(); let bytes = root.bytes();
        assert_eq!(host.activate_credibility(0, first.clone(), &p.committee).unwrap(), receipt);
        assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
        assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        assert!(host.dispatch(host.revision(), &old_auto, &old_human, &action, &inputs, state()).is_err());
        let resolved = host.reconcile(host.revision(), 3).unwrap();
        if executed {
            assert!(matches!(resolved, Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
        } else {
            assert_eq!(resolved, Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.charged, 7);
            assert_eq!(host.seal_unexecuted(host.revision(), 3).unwrap(), Reconciliation::Resolved(
                EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
        }
        let next = activation(&host, &p, 11, 3);
        host.activate_credibility(host.revision(), next, &p.committee).unwrap();
        let (fresh, current, automatic, key) = prepared(&mut host, &human, &p, 4);
        let before = host.inspect();
        host.activate_credibility(0, first, &p.committee).unwrap();
        assert_eq!(host.inspect(), before); // Historical retry does not revoke newer human approval.
        host.dispatch(host.revision(), &automatic, &key, &fresh, &current, state()).unwrap();
        host.publish_checked(host.revision(), 4, Some(&current), state(), ElapsedTick(4)).unwrap();
        host.reconcile(host.revision(), 4).unwrap();
        assert_eq!(host.inspect().executions, u64::from(executed) + 1);
        assert_eq!(host.inspect().control.ledger.charged, if executed { 14 } else { 7 });
    }
}

#[test]
fn exact_policy_pinning_refuses_before_cleanup_and_generic_open_keeps_the_guard() {
    let (root, host, _, p) = ready(0); drop(host);
    let pending = root.0.join("delivery.pending"); fs::write(&pending, b"do not clean on mismatch").unwrap();
    let bytes = root.bytes();
    for changed in [
        HeldOutJointPolicy::new(72, 1, 1, 2, 0, 0, HeldOutJointBudget::default()).unwrap(),
        HeldOutJointPolicy::new(71, 1, 1, 2, 1, 0, HeldOutJointBudget::default()).unwrap(),
        HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0, HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap(),
    ] {
        assert!(FileOversight::open_with_held_out_joint(&root.0, p.clone(), changed).is_err());
        assert_eq!(root.bytes(), bytes); assert_eq!(fs::read(&pending).unwrap(), b"do not clean on mismatch");
    }
    let (mut host, _) = FileOversight::open(&root.0, p.clone()).unwrap();
    assert_eq!(host.held_out_joint_policy().unwrap(), Some(guard()));
    assert!(!pending.exists());
    assert!(host.enable_held_out_joint(host.revision(), guard()).is_err());
    let request = activation(&host, &p, 10, 2);
    host.activate_credibility(host.revision(), request, &p.committee).unwrap();
    assert!(host.held_out_joint_report(10).unwrap().unwrap().qualified());
}

#[test]
fn durable_evaluation_lanes_are_exclusive_and_legacy_cannot_claim_the_new_guard() {
    let protocol = EvaluationProtocol { domain: 1, stratum: 1, period: 1,
        minimum_violation_origins: 2, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 };
    let root = Directory::new(); let p = profile(0);
    let (mut host, _) = FileOversight::create_with_held_out_joint(&root.0, p.clone(), guard()).unwrap();
    let before = root.bytes();
    assert!(host.enable_credibility(host.revision(), protocol.clone()).is_err());
    assert_eq!(root.bytes(), before);
    drop(host);
    let other = Directory::new();
    let (mut host, _) = FileOversight::create(&other.0, p.clone()).unwrap();
    host.enable_credibility(host.revision(), protocol).unwrap();
    let before = other.bytes();
    assert!(host.enable_held_out_joint(host.revision(), guard()).is_err());
    assert_eq!(other.bytes(), before);
    drop(host);
    assert!(FileOversight::open_with_held_out_joint(&other.0, p, guard()).is_err());
    assert_eq!(other.bytes(), before);
}

#[test]
fn joint_qualification_never_bypasses_final_exact_policy_revalidation() {
    for changed in [false, true] {
        let (_root, mut host, human, p) = ready(0);
        let request = activation(&host, &p, 10, 2);
        host.activate_credibility(host.revision(), request, &p.committee).unwrap();
        let (action, inputs, auto, key) = prepared(&mut host, &human, &p, 3);
        host.dispatch(host.revision(), &auto, &key, &action, &inputs, state()).unwrap();
        let mut current = state();
        if changed { current.values.insert(7, vec![1]); } else { current.values.insert(8, vec![1]); }
        let outcome = host.publish_checked(host.revision(), 3, Some(&inputs), current, ElapsedTick(2)).unwrap();
        assert_eq!(matches!(outcome.outcome, EndpointOutcome::Executed { .. }), !changed);
        host.reconcile(host.revision(), 3).unwrap();
        assert_eq!(host.inspect().executions, u64::from(!changed));
        assert_eq!(host.inspect().control.ledger.charged, if changed { 0 } else { 7 });
    }
}
