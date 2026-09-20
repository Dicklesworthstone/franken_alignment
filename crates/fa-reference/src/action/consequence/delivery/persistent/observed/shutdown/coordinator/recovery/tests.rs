//! Actual two-store shutdown/recovery, including ambiguous acknowledgments.
//! These deterministic injected barriers are not hardware power-cut experiments.
use super::*;
use crate::action::{ActionSpec, Purpose, ResolvedTarget, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits, RecoveryReserve};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-shutdown-recovery-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn image(&self) -> Vec<u8> { std::fs::read(self.store().join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("shutdown recovery cleanup: {error}"); }
    }
}
fn profile(id: u64) -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: id, branch: 4, authority: 100 + id, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(64)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
                continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: Vec::new(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"shutdown-fixture".to_vec(), model_epoch: 1,
                tokenizer_epoch: 1, policy_epoch: 0 }, 1, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn owner(root: &Directory, id: u64, events: Option<usize>) -> FileOversight {
    let mut p = profile(id);
    if let Some(events) = events { p.delivery.limits.events = events; }
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    if events.is_some() { host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap(); }
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
fn plan(hosts: &[&FileOversight], max_head_bytes: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, hosts.iter().map(|host| {
        host.shutdown_domain(host.profile.delivery.scope.run).unwrap()
    }).collect(), 32, max_head_bytes).unwrap()
}
fn coordinator(root: &Directory, plan: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), plan, MAX_JOURNAL_BYTES).unwrap()
}

#[test]
fn crashed_guarded_domain_uses_exact_tail_and_repeated_recovery_never_rewrites_it() {
    let root = Directory::new(); let control = Directory::new();
    let mut h = owner(&root, 1, Some(7));
    h.propose(h.revision(), 1, ActionSpec { version: VERSION, scope: h.profile.delivery.scope,
        target: Some(h.inspect().target), payload: b"reserved".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: h.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 },
        Snapshot { complete: true, semantic_epoch: 1, ..Snapshot::default() }).unwrap();
    assert_eq!(h.inspect().control.ledger.reserved, 16);
    assert_eq!(h.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let native = h.profile.clone();
    let before = h.inspect(); let mut c = coordinator(&control, &p); drop(h);
    let observation = c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(observation.source, FileShutdownSource::AcknowledgedOwner);
    assert_eq!(observation.journal_revision, before.revision + 3);
    assert!(observation.stop.unwrap().drained());
    assert!(observation.last_drain.unwrap().sweep.progress.drained());
    let after = FileOversight::read_publication(root.store(), &native).unwrap();
    assert_eq!(after.revision, 7); assert_eq!(after.control.ledger.available, 100);
    assert_eq!(after.control.ledger.reserved, 0); assert_eq!(after.executions, 0);
    let complete = root.image();
    // No new current-time claim or writable owner is returned by this reread.
    let repeated = c.recover_and_drain(c.revision(), 1, ElapsedTick(0)).unwrap().unwrap();
    assert_eq!(repeated.source, FileShutdownSource::CanonicalImage);
    assert_eq!(repeated.journal_revision, after.revision);
    assert_eq!(root.image(), complete); assert!(c.report().all_observed_drained());
}

#[test]
fn busy_and_missing_members_remain_visible_without_blocking_other_registered_domains() {
    let first = Directory::new(); let second = Directory::new(); let control = Directory::new();
    let h1 = owner(&first, 1, None); let h2 = owner(&second, 2, None);
    let p = plan(&[&h1, &h2], MAX_SHUTDOWN_HEAD_BYTES); let mut c = coordinator(&control, &p);
    drop(h1);
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(c.recover_and_drain(c.revision(), 2, ElapsedTick(2)).unwrap(), Err(JournalError::Busy));
    assert_eq!(c.report().current_session.unobserved_stops(), vec![2]);
    assert!(!c.report().all_observed_drained()); assert!(h2.inspect().stop.is_none());
    drop(h2);
    let hidden = second.0.join("unreachable"); std::fs::rename(second.store(), &hidden).unwrap();
    assert!(c.recover_and_drain(c.revision(), 2, ElapsedTick(2)).unwrap().is_err());
    assert_eq!(c.report().current_session.domains.len(), 2);
    assert_eq!(c.report().current_session.unobserved_stops(), vec![2]);
    std::fs::rename(hidden, second.store()).unwrap();
    c.recover_and_drain(c.revision(), 2, ElapsedTick(2)).unwrap().unwrap();
    assert!(c.report().all_observed_drained());
}

#[test]
fn latest_acknowledged_prefix_rejects_rollback_and_same_length_valid_substitutions() {
    let root = Directory::new(); let control = Directory::new(); let mut h = owner(&root, 1, None);
    let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let native = h.profile.clone(); let old = root.image();
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap(); let current = root.image();
    let mut c = coordinator(&control, &p); c.inspect_canonical(0, 1).unwrap().unwrap(); drop(h);
    std::fs::write(root.store().join(storage::CANONICAL), &old).unwrap();
    assert_eq!(c.recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap(), Err(Error::Stale.into()));
    assert_eq!(root.image(), old);
    let mut alternative = journal::decode(&native, &root.store(), &current).unwrap();
    *alternative.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    Machine::replay(&native, &alternative).unwrap();
    let substituted = journal::encode(&native, &root.store(), &alternative).unwrap();
    assert_eq!(substituted.len(), current.len());
    std::fs::write(root.store().join(storage::CANONICAL), &substituted).unwrap();
    assert_eq!(c.recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap(), Err(Error::Binding.into()));
    assert_eq!(root.image(), substituted);
    std::fs::write(root.store().join(storage::CANONICAL), &current).unwrap();
    c.recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap().unwrap();
    assert!(c.report().all_observed_drained());
}

#[test]
fn comparison_head_capacity_is_preflighted_before_native_shutdown() {
    for enough in [false, true] {
        let root = Directory::new(); let control = Directory::new(); let h = owner(&root, 1, None);
        let before = root.image();
        let p = plan(&[&h], before.len() + 50 - usize::from(!enough));
        let mut c = coordinator(&control, &p); drop(h);
        let result = c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap();
        if enough {
            assert!(result.unwrap().stop.unwrap().drained());
            assert_eq!(root.image().len(), before.len() + 50);
        } else {
            assert_eq!(result, Err(Error::Limit.into()));
            assert_eq!(root.image(), before); assert!(!c.report().all_observed_drained());
        }
    }
}

#[test]
fn coordinator_interruption_after_native_commit_needs_no_second_recovery_tail() {
    let root = Directory::new(); let control = Directory::new(); let h = owner(&root, 1, Some(6));
    let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let mut c = coordinator(&control, &p); drop(h);
    let (index, attempt) = c.begin(0, 1, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }).unwrap();
    let (observed, _, _) = c.recover_domain(index, attempt, ElapsedTick(2)).unwrap();
    assert_eq!(observed.journal_revision, 6); let completed = root.image();
    assert!(c.report().unavailable); drop(c); // No coordinator completion acknowledgment.
    let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 1).unwrap();
    assert!(!c.report().all_observed_drained());
    assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
    c.recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap().unwrap();
    assert_eq!(root.image(), completed); assert!(c.report().all_observed_drained());
    assert_eq!(c.report().visits.len(), 2);
    assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
}

#[test]
fn coordinator_intent_storage_failures_never_begin_domain_recovery() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new(); let h = owner(&root, 1, Some(6));
        let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let mut c = coordinator(&control, &p);
        let before = root.image(); drop(h); c.store.fail_once(barrier);
        assert!(matches!(c.recover_and_drain(0, 1, ElapsedTick(2)), Err(JournalError::Io(_))));
        assert_eq!(root.image(), before); assert!(c.report().unavailable); drop(c);
        let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 0).unwrap();
        assert!(!c.report().all_observed_drained());
        c.recover_and_drain(c.revision(), 1, ElapsedTick(2)).unwrap().unwrap();
        assert!(c.report().all_observed_drained());
    }
}

#[test]
fn coordinator_completion_storage_failures_cannot_repeat_a_completed_domain_write() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new(); let h = owner(&root, 1, Some(6));
        let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let mut c = coordinator(&control, &p); drop(h);
        let (index, attempt) = c.begin(0, 1, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }).unwrap();
        let result = c.recover_domain(index, attempt, ElapsedTick(2)); assert!(result.is_ok());
        let complete = root.image(); c.store.fail_once(barrier);
        assert!(matches!(c.finish(index, attempt, result), Err(JournalError::Io(_))));
        assert!(c.report().unavailable); assert!(!c.report().all_observed_drained()); drop(c);
        let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 1).unwrap();
        assert!(!c.report().all_observed_drained());
        c.recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap().unwrap();
        assert!(c.report().all_observed_drained()); assert_eq!(root.image(), complete);
    }
}

#[test]
fn native_storage_failures_are_recorded_and_retry_resolves_the_actual_canonical_cut() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new(); let h = owner(&root, 1, Some(6));
        let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let native = h.profile.clone();
        let mut c = coordinator(&control, &p); drop(h);
        let (index, attempt) = c.begin(0, 1, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }).unwrap();
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let result = c.recover_locked(index, attempt, ElapsedTick(2), store);
        assert!(matches!(&result, Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert!(c.finish(index, attempt, result).unwrap().is_err());
        assert!(!c.report().unavailable); assert!(!c.report().all_observed_drained());
        let disk = FileOversight::read_publication(root.store(), &native).unwrap();
        assert_eq!(disk.revision, if barrier == JournalIo::DirectorySync { 6 } else { 3 });
        c.recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap().unwrap();
        let disk = FileOversight::read_publication(root.store(), &native).unwrap();
        assert_eq!(disk.revision, 6); assert_eq!(disk.executions, 0);
        assert!(c.report().all_observed_drained());
    }
}

#[test]
fn recovery_intent_format_is_explicit_and_downgrading_its_header_refuses() {
    let root = Directory::new(); let control = Directory::new(); let h = owner(&root, 1, None);
    let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let mut c = coordinator(&control, &p); drop(h);
    let old = c.encoded(0).unwrap(); assert_eq!(&old[..8], b"FASHCOO\x01");
    let mut false_upgrade = old.clone(); false_upgrade[7] = 2;
    assert!(codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 0, &false_upgrade).is_err());
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    let bytes = c.encoded(c.revision()).unwrap(); assert_eq!(&bytes[..8], b"FASHCOO\x02");
    let restored = codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 2, &bytes).unwrap();
    assert!(matches!(restored.visits[0].kind, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }));
    assert!(!restored.campaign.report().all_observed_stopped());
    assert_eq!(codec::encode(c.store.identity(), &p.encode().unwrap(), MAX_JOURNAL_BYTES,
        restored.revision, &restored.visits, &restored.campaign).unwrap(), bytes);
    let mut downgrade = bytes.clone(); downgrade[7] = 1;
    assert!(codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 0, &downgrade).is_err());
}

#[test]
fn a_different_native_stop_cannot_be_claimed_as_this_campaigns_shutdown() {
    let root = Directory::new(); let control = Directory::new(); let mut h = owner(&root, 1, None);
    let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let mut c = coordinator(&control, &p);
    let before = h.inspect();
    h.stop_and_drain(h.revision(), StopRequest { operation: 901,
        expected_control_sequence: before.control.sequence, expected_authority_epoch: before.control.ledger.epoch },
        ElapsedTick(2)).unwrap(); let complete = root.image(); drop(h);
    assert_eq!(c.recover_and_drain(0, 1, ElapsedTick(3)).unwrap(), Err(Error::Binding.into()));
    assert_eq!(root.image(), complete); assert!(!c.report().all_observed_drained());
}

#[test]
fn unknown_domain_and_stale_coordinator_revision_cannot_acquire_or_mutate_a_domain() {
    let root = Directory::new(); let control = Directory::new(); let h = owner(&root, 1, None);
    let p = plan(&[&h], MAX_SHUTDOWN_HEAD_BYTES); let mut c = coordinator(&control, &p);
    let before = root.image(); let report = c.report(); drop(h);
    assert_eq!(c.recover_and_drain(1, 1, ElapsedTick(2)), Err(Error::Stale.into()));
    assert_eq!(c.recover_and_drain(0, 999, ElapsedTick(2)), Err(Error::Missing.into()));
    assert_eq!(c.report(), report); assert_eq!(root.image(), before);
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    assert!(c.report().all_observed_drained());
}
