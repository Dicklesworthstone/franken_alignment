//! Caught interruption at the actual source-acquisition boundary. The injected
//! reader is private to the implementation; the public EvidenceFile stays sealed.
use super::*;
use super::super::FileOversightProfile;
use crate::action::{ActionSpec, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::evidence_source::FileEvidenceSource;
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("source interruption cleanup: {error}"); }
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
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn setup() -> (Directory, FileOversight, FileEvidenceSource, Rc<EvidenceSnapshot>, ActionSpec) {
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = Directory(std::env::temp_dir().join(format!("fa-source-interruption-{}-{time}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    std::fs::create_dir(&root.0).unwrap();
    let p = profile();
    let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) };
    let capture = Rc::new(EvidenceSnapshot::new(EvidenceIdentity { source: 9, generation: 1, scope: p.delivery.scope },
        snapshot, BTreeMap::from([("reviewer".to_owned(), b"context".to_vec())])).unwrap());
    let path = root.0.join("source.json");
    std::fs::write(&path, capture.encode()).unwrap();
    let mut reader = FileEvidenceSource::new(path, 9, p.delivery.scope, 1_048_576).unwrap();
    let action = ActionSpec { version: VERSION, scope: p.delivery.scope, target: Some(p.delivery.target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100), units: 16 };
    let (mut host, _) = FileOversight::create(root.0.join("publication"), p).unwrap();
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: action.scope, source: 9, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(20).unwrap(),
    }).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    (root, host, reader, capture, action)
}

#[test]
fn caught_read_unwind_blocks_old_snapshot_until_real_recapture() {
    let (_root, mut host, mut reader, capture, action) = setup();
    host.propose(host.revision(), 1, action.clone(), capture.snapshot().clone()).unwrap();
    let before = host.inspect();
    let revision = host.revision();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        host.refresh_source_with(revision, || panic!("interrupted acquisition"), ElapsedTick(2))
    })).is_err());
    assert_eq!(host.inspect(), before);
    assert!(host.file_source_status().unwrap().interrupted);
    assert_eq!(host.propose(host.revision(), 2, action.clone(), capture.snapshot().clone()).unwrap_err(),
        JournalError::Contract(Error::Incomplete));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.file_source_status().unwrap().interrupted);
    let reads = reader.status().read_attempts;
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)).unwrap();
    assert_eq!(reader.status().read_attempts, reads + 1);
    assert!(!host.file_source_status().unwrap().interrupted);
    host.propose(host.revision(), 2, action, capture.snapshot().clone()).unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn successful_unchanged_refresh_retains_whole_input_eligibility() {
    let (_root, mut host, mut reader, capture, spec) = setup();
    let action = host.propose(host.revision(), 1, spec, capture.snapshot().clone()).unwrap();
    let input = capture.inputs_for(&action, &profile().committee).unwrap();
    let input_revision = host.record_inputs(host.revision(), 1, 0, input).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)).unwrap();
    assert!(!host.file_source_status().unwrap().interrupted);
    assert_eq!(host.input_revision(1).unwrap(), input_revision);
    assert!(host.machine.broker.current_inputs(1).unwrap().is_some());
}

#[test]
fn ordinary_read_error_commits_withdrawal_instead_of_reusing_old_data() {
    let (_root, mut host, mut reader, capture, action) = setup();
    let failure = EvidenceError::Io(std::io::ErrorKind::NotFound);
    assert_eq!(host.refresh_source_with(host.revision(), || Err(failure), ElapsedTick(2)),
        Err(FileSourceError::Read { error: failure, withdrawal: None }));
    assert!(!host.file_source_status().unwrap().interrupted);
    assert_eq!(host.propose(host.revision(), 1, action.clone(), capture.snapshot().clone()).unwrap_err(),
        JournalError::Contract(Error::Incomplete));
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)).unwrap();
    host.propose(host.revision(), 1, action, capture.snapshot().clone()).unwrap();
}

#[test]
fn stale_preflight_neither_calls_reader_nor_withdraws_a_valid_source() {
    let (_root, mut host, _reader, capture, action) = setup();
    let called = Cell::new(false);
    assert_eq!(host.refresh_source_with(host.revision() + 1, || {
        called.set(true); Ok(Rc::clone(&capture))
    }, ElapsedTick(2)), Err(FileSourceError::Journal(JournalError::Contract(Error::Stale))));
    assert!(!called.get());
    assert!(!host.file_source_status().unwrap().interrupted);
    host.propose(host.revision(), 1, action, capture.snapshot().clone()).unwrap();
}

#[test]
fn failed_observation_replacement_never_reopens_acquisition_latch() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let (_root, mut host, mut reader, capture, action) = setup();
        let before = host.inspect();
        host.store.fail_once(barrier);
        assert!(matches!(host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)),
            Err(FileSourceError::Journal(JournalError::Io(_)))));
        assert!(host.file_source_status().unwrap().interrupted);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.propose(host.revision(), 1, action, capture.snapshot().clone()).unwrap_err(), JournalError::Unavailable);
    }
}
