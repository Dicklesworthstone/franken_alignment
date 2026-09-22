//! Fixed reference wire vectors and real original Store failure barriers.
use super::*;
use super::super::codec;
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("fa-heldout-bootstrap-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() && let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("heldout bootstrap cleanup: {error}"); }
    }
}
fn policy() -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0, HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap()
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::ExactRestart },
                vec![1], vec![2], vec![3], 1).unwrap(),
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
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];

#[test]
fn held_out_policy_vector_is_distinct_strict_and_fully_bounded() {
    let mut expected = vec![6];
    for field in [71_u64, 1, 1, 2, 0, 0, 3, 12] { expected.extend_from_slice(&field.to_be_bytes()); }
    let mut w = Writer::new(65);
    codec::write(&mut w, &CredibilityEvent::EnableHeldOutJoint(policy())).unwrap();
    assert_eq!(w.finish(), expected);
    for end in 0..expected.len() { assert!(codec::read(&mut Reader::new(&expected[..end])).is_err()); }
    let mut r = Reader::new(&expected);
    assert!(matches!(codec::read(&mut r).unwrap(), CredibilityEvent::EnableHeldOutJoint(p) if p == policy()));
    r.end().unwrap();
    for (field, invalid) in [(0, 0_u64), (1, 0), (2, 0), (3, 0), (4, 1_000_001),
        (5, 1_000_001), (6, 4097), (7, u64::MAX)] {
        let mut bytes = expected.clone(); let start = 1 + field * 8;
        bytes[start..start + 8].copy_from_slice(&invalid.to_be_bytes());
        assert!(codec::read(&mut Reader::new(&bytes)).is_err());
    }
    expected[0] = 7;
    assert!(codec::read(&mut Reader::new(&expected)).is_err());
}

#[test]
fn bootstrap_returns_roles_only_after_the_complete_original_canonical_write() {
    let root = Directory::new();
    let (host, human) = FileOversight::create_with_held_out_joint(&root.0, profile(), policy()).unwrap();
    assert_eq!(human.reviewer_id(), 77); assert_eq!(host.revision(), 1);
    assert!(host.publication_guard_required());
    assert_eq!(host.held_out_joint_policy().unwrap(), Some(policy()));
    let bytes = fs::read(root.0.join(storage::CANONICAL)).unwrap();
    let read = FileOversight::read_held_out_joint(&root.0, &profile(), policy()).unwrap();
    assert_eq!(read.journal, host.inspect()); assert!(read.promotions.is_empty());
    assert_eq!(fs::read(root.0.join(storage::CANONICAL)).unwrap(), bytes);
    drop(host);
    for barrier in BARRIERS {
        let root = Directory::new(); let prepared = Prepared::new(profile(), policy()).unwrap();
        let store = storage::Store::create(&root.0).unwrap(); store.fail_once(barrier);
        let failed = prepared.publish(store);
        assert!(matches!(failed, Err(JournalError::Io(ref f)) if f.operation == barrier));
        if barrier == JournalIo::DirectorySync {
            let read = FileOversight::read_held_out_joint(&root.0, &profile(), policy()).unwrap();
            assert_eq!(read.policy, policy()); assert_eq!(read.journal.revision, 1);
            let (host, _) = FileOversight::open_with_held_out_joint(&root.0, profile(), policy()).unwrap();
            assert_eq!(host.revision(), 2); assert!(!host.clock_ready());
        } else {
            assert!(!root.0.join(storage::CANONICAL).exists());
            assert!(FileOversight::open_with_held_out_joint(&root.0, profile(), policy()).is_err());
        }
    }
}

#[test]
fn failed_recovery_never_returns_old_roles_or_drops_the_joint_policy() {
    for barrier in BARRIERS {
        let root = Directory::new();
        let (host, _) = FileOversight::create_with_held_out_joint(&root.0, profile(), policy()).unwrap();
        let before = host.inspect(); drop(host);
        let store = storage::Store::open(&root.0).unwrap(); store.fail_once(barrier);
        let failed = FileOversight::open_held_out_joint_store(store, profile(), policy());
        assert!(matches!(failed, Err(JournalError::Io(ref f)) if f.operation == barrier));
        let disk = FileOversight::read_held_out_joint(&root.0, &profile(), policy()).unwrap();
        assert_eq!(disk.policy, policy());
        assert_eq!(disk.journal.control.ledger.available, before.control.ledger.available);
        assert_eq!(disk.journal.control.ledger.charged, 0);
        assert_eq!(disk.journal.revision, if barrier == JournalIo::DirectorySync { 2 } else { 1 });
        let (host, _) = FileOversight::open_with_held_out_joint(&root.0, profile(), policy()).unwrap();
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(policy()));
        assert!(!host.clock_ready()); assert_eq!(host.inspect().executions, 0);
    }
}
