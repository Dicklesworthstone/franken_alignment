//! Real storage barriers and capacity refusal around the original source owner.
use super::*;
use super::super::{FileSourceError, FileSourcePolicy};
use super::super::super::{FileOversightProfile, journal, machine::Machine};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits, RecoveryReserve};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-source-replacement-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("source replacement cleanup: {error}"); }
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
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn enable(host: &mut FileOversight) {
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 31, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(10).unwrap(),
    }).unwrap();
}
fn refresh(host: &mut FileOversight, root: &Directory, generation: u64, context_bytes: usize, tick: u64)
    -> Result<(), FileSourceError>
{
    let value = EvidenceSnapshot::new(EvidenceIdentity { source: 31, generation, scope: profile().delivery.scope },
        snapshot(), BTreeMap::from([("reviewer".to_owned(), vec![1; context_bytes])])).unwrap();
    let path = root.0.join("evidence.json"); let temporary = root.0.join("evidence.next");
    fs::write(&temporary, value.encode()).unwrap(); fs::rename(&temporary, &path).unwrap();
    let mut source = FileEvidenceSource::new(path, 31, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(tick)).map(|_| ())
}
fn proposal(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"candidate".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
fn request(host: &FileOversight, operation: u64) -> FileSourceReplacement {
    let generation = host.file_source_status().unwrap().capture.source.generation;
    FileSourceReplacement { operation, expected_generation: generation, next_generation: generation + 1,
        expected_authority_epoch: host.inspect().control.ledger.epoch }
}
fn canonical(host: &FileOversight) -> Machine {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}

#[test]
fn replacement_failure_exposes_no_candidate_and_reopen_uses_the_actual_canonical_generation() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
        enable(&mut host); refresh(&mut host, &root, 1, 1, 1).unwrap();
        host.propose(host.revision(), 1, proposal(&host), snapshot()).unwrap();
        let replacement = request(&host, 1); let before = host.inspect();
        assert_eq!(before.control.ledger.reserved, 16);
        host.store.fail_once(barrier);
        let error = host.replace_file_source(host.revision(), replacement).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected the selected storage barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.file_source_replacement(1), Err(JournalError::Unavailable));
        assert_eq!(host.replace_file_source(0, replacement), Err(JournalError::Unavailable));
        let disk = canonical(&host); let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.file_source_status().unwrap().capture.source.generation, if visible { 2 } else { 1 });
        assert_eq!(disk.broker.inspect().ledger.reserved, if visible { 0 } else { 16 });
        assert_eq!(disk.broker.inspect().ledger.charged, 0);
        drop(host);
        let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let revision = host.revision();
        if visible {
            let receipt = host.file_source_replacement(1).unwrap();
            assert_eq!(receipt.refunded_units, 16); assert_eq!(receipt.cancelled, vec![1]);
            assert_eq!(host.replace_file_source(0, replacement).unwrap(), receipt);
        } else {
            assert_eq!(host.file_source_replacement(1), Err(JournalError::Contract(Error::Missing)));
            assert_eq!(host.replace_file_source(revision, replacement), Err(JournalError::Contract(Error::Stale)));
        }
        assert_eq!(host.revision(), revision);
        assert_eq!(host.file_source_status().unwrap().capture.closed, None);
        assert_eq!(host.inspect().control.ledger.available, 100);
        refresh(&mut host, &root, 1, 1, 2).unwrap();
        host.propose(host.revision(), 2, proposal(&host), snapshot()).unwrap();
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn actual_capacity_interruption_can_be_replaced_but_historical_retry_cannot_clear_a_new_interruption() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.bytes = 8192;
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    enable(&mut host); refresh(&mut host, &root, 1, 1, 1).unwrap();
    let before = host.inspect();
    assert_eq!(refresh(&mut host, &root, 2, 20000, 2), Err(FileSourceError::Journal(JournalError::Contract(Error::Limit))));
    assert_eq!(host.inspect(), before); assert!(host.file_source_status().unwrap().interrupted);
    let first = request(&host, 1); let receipt = host.replace_file_source(host.revision(), first).unwrap();
    assert!(!host.file_source_status().unwrap().interrupted);
    assert_eq!(host.file_source_status().unwrap().capture.closed, None);
    assert_eq!(host.propose(host.revision(), 1, proposal(&host), snapshot()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    refresh(&mut host, &root, 3, 1, 3).unwrap();
    assert_eq!(refresh(&mut host, &root, 4, 20000, 4), Err(FileSourceError::Journal(JournalError::Contract(Error::Limit))));
    let before = host.inspect();
    assert_eq!(host.replace_file_source(0, first).unwrap(), receipt);
    assert_eq!(host.inspect(), before); assert!(host.file_source_status().unwrap().interrupted);
    assert_eq!(host.propose(host.revision(), 1, proposal(&host), snapshot()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    let second = request(&host, 2); host.replace_file_source(host.revision(), second).unwrap();
    refresh(&mut host, &root, 5, 1, 5).unwrap();
    host.propose(host.revision(), 1, proposal(&host), snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn source_replacement_cannot_spend_the_original_emergency_journal_tail() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 16;
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    enable(&mut host);
    for _ in 0..16 {
        if host.journal_capacity().unwrap().ordinary_remaining().events == 0 { break; }
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    }
    let capacity = host.journal_capacity().unwrap();
    assert_eq!(capacity.ordinary_remaining().events, 0);
    assert!(capacity.terminal_space_remaining());
    let before = host.inspect(); let replacement = request(&host, 1);
    assert_eq!(host.replace_file_source(host.revision(), replacement), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before); assert_eq!(host.journal_capacity().unwrap(), capacity);
    assert_eq!(host.file_source_replacement(1), Err(JournalError::Contract(Error::Missing)));
    host.fence(host.revision()).unwrap();
    assert_eq!(host.journal_capacity().unwrap().remaining().events, capacity.remaining().events - 1);
    assert_eq!(host.file_source_status().unwrap().capture.source.generation, 1);
    assert_eq!(host.inspect().executions, 0);
}
