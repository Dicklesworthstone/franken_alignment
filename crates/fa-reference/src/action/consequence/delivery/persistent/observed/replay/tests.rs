//! Canonical-image inspection after real replacement faults, without recovery writes.
use super::*;
use super::super::{FileHumanReviewer, JournalIo};
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot}, human::HumanReviewPolicy};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-review-export-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("review-export cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("judge".into(),
                MemberPolicy { cohort: "one".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("judge".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view".to_vec(), tokenizer_epoch: 1,
                policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn prepared(root: &Directory) -> (FileOversight, FileHumanReviewer, CommitteeInput, ObservedReviewAnchor) {
    let p = profile(); let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION,
        scope: p.delivery.scope, target: Some(p.delivery.target), payload: b"publish".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 16,
    }, snapshot()).unwrap();
    let capture = EvidenceSnapshot::new(EvidenceIdentity { source: 7, generation: 1, scope: p.delivery.scope },
        snapshot(), BTreeMap::from([("judge".into(), b"original context".to_vec())])).unwrap();
    let inputs = capture.inputs_for(&action, &p.committee).unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    let anchor = host.review_anchor(101).unwrap();
    let digest = commitment(101, "judge", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 101, "judge", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "judge", Verdict::Allow, b"salt".to_vec()).unwrap();
    (host, reviewer, inputs, anchor)
}

#[test]
fn all_replacement_faults_preserve_acknowledged_vs_visible_review_distinctions() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync]
    {
        let root = Directory::new(); let (mut host, _, inputs, anchor) = prepared(&root);
        let before = host.inspect(); host.store.fail_once(barrier);
        let error = host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap_err();
        assert!(matches!(error, JournalError::Io(ref failure) if failure.operation == barrier));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.review_replay(101).unwrap_err(), JournalError::Unavailable);
        assert_eq!(host.review_anchor(101).unwrap_err(), JournalError::Unavailable);
        let pending = root.store().join("delivery.pending"); let staged = std::fs::read(&pending).ok();
        let disk = FileOversight::read_review_replay(root.store(), &profile(), 101);
        if barrier == JournalIo::DirectorySync {
            let replay = disk.unwrap();
            replay.archive().verify_receipt(&anchor, replay.application().as_ref().unwrap()).unwrap();
            assert_eq!(replay.completed_snapshot().revision, before.revision + 1);
            assert_eq!(replay.journal_snapshot().executions, 0);
        } else { assert_eq!(disk.unwrap_err(), JournalError::Contract(Error::Incomplete)); }
        assert_eq!(std::fs::read(&pending).ok(), staged);
        drop(host);
        let (recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(recovered.review_replay(101).is_ok(), barrier == JournalIo::DirectorySync);
        assert_eq!(recovered.inspect().executions, 0);
    }
}

#[test]
fn a_well_encoded_but_semantically_invalid_suffix_is_not_ignored() {
    let root = Directory::new(); let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    assert!(FileOversight::read_review_replay(root.store(), &profile(), 101).is_ok());
    let mut events = host.events.clone(); events.push(Event::OpenReveals(999));
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    std::fs::write(root.store().join(storage::CANONICAL), bytes).unwrap();
    assert!(FileOversight::read_review_replay(root.store(), &profile(), 101).is_err());
}
