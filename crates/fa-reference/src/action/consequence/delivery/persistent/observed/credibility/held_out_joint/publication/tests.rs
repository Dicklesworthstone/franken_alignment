//! Actual original journals, key custody and publication; synthetic helper votes.
use super::*;
use super::super::super::super::publication::PublicationBasis;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits,
    RecoveryReserve, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationEvidence, FilePublicationInputs,
};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("fa-joint-publication-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() && let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("joint publication cleanup: {error}");
        }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::ExactRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(32)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("a".into(), MemberPolicy { cohort: "a".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: vec![], retention_ticks: 200,
            max_deliveries: 8, clock_domain: 1, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("a".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: vec![], tokenizer_epoch: 1,
                policy_epoch: 0, model_epoch: 1 }, 1, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 100, max_requests: 8 },
    }
}
fn selection(level: usize) -> JointPublicationProfile {
    JointPublicationProfile {
        joint: super::super::HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0,
            super::super::HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap(),
        validation: (level > 0).then_some(PublicationLimits {
            bindings: 8, validation: RefinementBudget { steps: 10_000, value_bytes: 1_048_576 },
        }),
        feed: (level > 1).then_some(JointPublicationFeed {
            changes: PublicationChangePolicy { source: 41, after: 0,
                lookup: RoutingBudget { steps: 10_000, bytes: 1_048_576 } },
            freshness: PublicationFreshnessPolicy { clock_domain: 1, max_age_ticks: 100 },
            snapshot_fallback: level == 3,
        }),
    }
}
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];

#[test]
fn every_supported_composition_is_in_the_first_image_and_recovery_is_one_fence() {
    for level in 0..4 {
        let root = Directory::new(); let expected = selection(level);
        let (mut host, human) = FileOversight::create_with_joint_publication(&root.0, profile(), expected).unwrap();
        assert_eq!(human.reviewer_id(), 77);
        assert!(host.publication_guard_required());
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(expected.joint));
        assert_eq!(host.publication_validation_profile().unwrap(), expected.validation);
        assert_eq!(host.revision(), [1, 2, 4, 5][level]);
        let before = root.bytes();
        let image = FileOversight::read_joint_publication(&root.0, &profile(), expected).unwrap();
        assert_eq!(image.journal, host.inspect()); assert!(image.promotions.is_empty());
        assert_eq!(root.bytes(), before); // Works beside the live exclusive lock.
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        let before = host.inspect(); drop(host);
        let (host, _) = FileOversight::open_with_joint_publication(&root.0, profile(), expected).unwrap();
        assert_eq!(host.revision(), before.revision + 1);
        assert_eq!(host.inspect().control.ledger.epoch, before.control.ledger.epoch + 1);
        assert!(!host.clock_ready()); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn all_policy_fields_and_absent_gates_are_pinned_before_cleanup_or_recovery() {
    let root = Directory::new(); let expected = selection(3);
    let (host, _) = FileOversight::create_with_joint_publication(&root.0, profile(), expected).unwrap();
    drop(host); let bytes = root.bytes();
    let pending = root.0.join("delivery.pending"); fs::write(&pending, b"must survive mismatch").unwrap();
    let mut invalid = vec![selection(0), selection(1), selection(2)];
    let mut p = expected; p.validation.as_mut().unwrap().validation.steps -= 1; invalid.push(p);
    let mut p = expected; p.validation.as_mut().unwrap().bindings -= 1; invalid.push(p);
    let mut p = expected; p.feed.as_mut().unwrap().changes.source += 1; invalid.push(p);
    let mut p = expected; p.feed.as_mut().unwrap().changes.after += 1; invalid.push(p);
    let mut p = expected; p.feed.as_mut().unwrap().changes.lookup.bytes -= 1; invalid.push(p);
    let mut p = expected; p.feed.as_mut().unwrap().freshness.max_age_ticks += 1; invalid.push(p);
    let mut p = expected;
    p.joint = super::super::HeldOutJointPolicy::new(72, 1, 1, 2, 0, 0, p.joint.budget()).unwrap(); invalid.push(p);
    for wrong in invalid {
        assert!(matches!(FileOversight::open_with_joint_publication(&root.0, profile(), wrong),
            Err(JournalError::Contract(Error::Binding))));
        assert!(FileOversight::read_joint_publication(&root.0, &profile(), wrong).is_err());
        assert_eq!(root.bytes(), bytes); assert_eq!(fs::read(&pending).unwrap(), b"must survive mismatch");
    }
    let (host, _) = FileOversight::open_with_joint_publication(&root.0, profile(), expected).unwrap();
    assert!(!pending.exists()); assert_eq!(host.revision(), 6);
    drop(host);
    // Absence is also exact: a configured caller cannot accept a legacy store.
    let legacy = Directory::new(); let (host, _) = FileOversight::create(&legacy.0, profile()).unwrap();
    drop(host); let before = legacy.bytes();
    assert!(FileOversight::open_with_joint_publication(&legacy.0, profile(), selection(0)).is_err());
    assert_eq!(legacy.bytes(), before);
}

#[test]
fn invalid_composition_never_creates_storage_and_subtree_is_not_silently_reinterpreted() {
    let mut invalid = Vec::new();
    let mut p = selection(2); p.validation = None; invalid.push(p);
    let mut p = selection(2); p.feed.as_mut().unwrap().freshness.clock_domain += 1; invalid.push(p);
    let mut p = selection(2); p.feed.as_mut().unwrap().changes.source = 0; invalid.push(p);
    let mut p = selection(1); p.validation.as_mut().unwrap().bindings = 0; invalid.push(p);
    for p in invalid {
        let root = Directory::new();
        assert!(FileOversight::create_with_joint_publication(&root.0, profile(), p).is_err());
        assert!(!root.0.exists());
    }
    let expected = selection(2);
    let mut prepared = expected.prepare(profile()).unwrap();
    let event = Event::PublicationWitness(WitnessEvent::SubtreeRouting);
    prepared.machine.apply(&event).unwrap(); prepared.events.push(event);
    let root = Directory::new(); let (host, _) = prepared.publish(storage::Store::create(&root.0).unwrap()).unwrap();
    drop(host); let before = root.bytes();
    assert!(matches!(FileOversight::open_with_joint_publication(&root.0, profile(), expected),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(root.bytes(), before);
}

#[test]
fn no_role_escapes_a_failed_first_image_or_recovery_replacement() {
    let expected = selection(3);
    for stage in BARRIERS {
        let root = Directory::new(); let prepared = expected.prepare(profile()).unwrap();
        let store = storage::Store::create(&root.0).unwrap(); store.fail_once(stage);
        assert!(matches!(prepared.publish(store), Err(JournalError::Io(ref failure)) if failure.operation == stage));
        if stage == JournalIo::DirectorySync {
            let image = FileOversight::read_joint_publication(&root.0, &profile(), expected).unwrap();
            assert_eq!(image.journal.revision, 5);
        } else { assert!(!root.0.join(storage::CANONICAL).exists()); }
        let root = Directory::new();
        let (host, _) = FileOversight::create_with_joint_publication(&root.0, profile(), expected).unwrap();
        drop(host); let store = storage::Store::open(&root.0).unwrap(); store.fail_once(stage);
        assert!(matches!(FileOversight::open_joint_publication_store(store, profile(), expected),
            Err(JournalError::Io(ref failure)) if failure.operation == stage));
        let image = FileOversight::read_joint_publication(&root.0, &profile(), expected).unwrap();
        assert_eq!(image.journal.revision, if stage == JournalIo::DirectorySync { 6 } else { 5 });
        assert_eq!(image.journal.control.ledger.charged, 0);
        let (host, _) = FileOversight::open_with_joint_publication(&root.0, profile(), expected).unwrap();
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(expected.joint));
        assert!(!host.clock_ready());
    }
}

#[test]
fn original_two_key_publication_still_rechecks_whole_input_after_dispatch() {
    for changed in [false, true] {
        let root = Directory::new(); let p = profile();
        let (mut host, human) = FileOversight::create_with_joint_publication(&root.0, p.clone(), selection(1)).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let source = EvidenceSnapshot::new(EvidenceIdentity { source: 21, generation: 1, scope: p.delivery.scope },
            Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() },
            BTreeMap::from([("a".into(), b"review evidence".to_vec())])).unwrap();
        let action = host.propose(host.revision(), 1, ActionSpec {
            version: VERSION, scope: p.delivery.scope, target: Some(p.delivery.target), payload: b"visible".to_vec(),
            required_witnesses: vec![], policy_epoch: 0, deadline: ElapsedTick(100), units: 7,
        }, source.snapshot().clone()).unwrap();
        let inputs = source.inputs_for(&action, &p.committee).unwrap();
        let original = FilePublicationInputs::new(None, Some(inputs.views()["a"].actual_input().clone()));
        host.bind_publication_evidence(host.revision(), 1, FilePublicationEvidence::new(original.clone(), vec![]).unwrap()).unwrap();
        host.record_publication_inputs(host.revision(), 1, 0, Some(original)).unwrap();
        host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
        host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, source.snapshot().clone()).unwrap();
        host.commit_review(host.revision(), 101, "a", commitment(101, "a", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
        host.open_reveals(host.revision(), 101).unwrap();
        host.reveal_review(host.revision(), 101, "a", Verdict::Allow, b"salt".to_vec()).unwrap();
        host.finish_review(host.revision(), 101, Some(&inputs), source.snapshot().clone()).unwrap().unwrap();
        let automatic = host.authorize(host.revision(), 1, &inputs, source.snapshot().clone()).unwrap();
        let request = host.request_human_approval(host.revision(), 11, 1, &inputs, ElapsedTick(50)).unwrap();
        let revision = host.revision(); let key = human.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &key, &action, &inputs, source.snapshot().clone()).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, 7);
        if changed {
            let revision = host.publication_input_revision(1).unwrap();
            host.record_publication_inputs(host.revision(), 1, revision, None).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, 7); // Loss is not nonexecution.
        }
        let published = host.publish_checked(host.revision(), 1, Some(&inputs), source.snapshot().clone(), ElapsedTick(2)).unwrap();
        let outcome = if changed { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
            else { EndpointOutcome::Executed { resulting_version: 2 } };
        assert_eq!(published.outcome, outcome);
        assert!(matches!(published.basis, PublicationBasis::Rejected(_)) == changed);
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
        assert_eq!(host.inspect().executions, u64::from(!changed));
        assert_eq!(host.inspect().control.ledger.charged, if changed { 0 } else { 7 });
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(selection(1).joint));
    }
}
