//! Real domain and coordinator files; synthetic policy fixtures, no inference.
use super::*;
use crate::action::{Purpose, ResolvedTarget};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-shutdown-resolution-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn image(&self) -> Vec<u8> { std::fs::read(self.store().join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("resolution test cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1,
        expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
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
fn create(root: &Directory, events: usize) -> FileOversight {
    let mut p = profile(); p.delivery.limits.events = events;
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
fn plan(host: &FileOversight, visits: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap()], visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
fn reopen(root: &Directory, p: &FileShutdownPlan, floor: u64) -> FileShutdownCoordinator {
    FileShutdownCoordinator::open(root.store(), p, MAX_JOURNAL_BYTES, floor).unwrap()
}
fn request(host: &FileOversight, operation: u64) -> StopRequest {
    let before = host.inspect();
    StopRequest { operation, expected_control_sequence: before.control.sequence,
        expected_authority_epoch: before.control.ledger.epoch }
}
fn enter(c: &mut FileShutdownCoordinator) -> (usize, usize) {
    c.begin(c.revision(), 1, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }).unwrap()
}

#[test]
fn final_visit_and_final_native_events_can_resolve_then_refresh_without_new_writes() {
    let root = Directory::new(); let control = Directory::new(); let h = create(&root, 5);
    let p = plan(&h, 1); let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    drop(h);
    let (index, attempt) = enter(&mut c);
    c.recover_domain(index, attempt, ElapsedTick(2)).unwrap();
    let native = root.image(); drop(c);
    let mut c = reopen(&control, &p, 1);
    assert_eq!(c.inspect_canonical(c.revision(), 1), Err(Error::Limit.into()));
    let observed = c.resolve_shutdown_visit(1, 0).unwrap();
    assert_eq!(observed.source, FileShutdownSource::CanonicalImage);
    assert_eq!(observed.journal_revision, 5);
    assert!(observed.stop.unwrap().drained());
    assert_eq!(c.revision(), 2); assert_eq!(c.report().visits.len(), 1);
    assert!(c.report().all_observed_drained()); assert_eq!(root.image(), native);
    let completed = control.image(); drop(c);
    let mut c = reopen(&control, &p, 2);
    assert!(!c.report().all_observed_drained());
    c.resolve_shutdown_visit(2, 0).unwrap();
    assert!(c.report().all_observed_drained());
    assert_eq!(c.revision(), 2); assert_eq!(control.image(), completed); assert_eq!(root.image(), native);
}

#[test]
fn a_pending_intent_cannot_stop_an_unstopped_domain_or_consume_failed_evidence() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root, 20);
    let p = plan(&h, 2); let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    enter(&mut c); drop(c); let intent = control.image(); let native = root.image();
    let mut c = reopen(&control, &p, 1);
    assert_eq!(c.resolve_shutdown_visit(1, 0), Err(Error::Incomplete.into()));
    assert!(!c.report().unavailable); assert!(!c.report().all_observed_drained());
    assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
    assert_eq!(control.image(), intent); assert_eq!(root.image(), native);
    h.stop_and_drain(h.revision(), request(&h, 900), ElapsedTick(2)).unwrap();
    let native = root.image();
    assert!(c.resolve_shutdown_visit(1, 0).unwrap().stop.unwrap().drained());
    assert_eq!(root.image(), native); // Even beside the still-locked live owner.
}

#[test]
fn every_completion_barrier_preserves_pending_or_complete_truth_at_the_visit_ceiling() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new(); let h = create(&root, 5);
        let p = plan(&h, 1); let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        drop(h); let (index, attempt) = enter(&mut c);
        c.recover_domain(index, attempt, ElapsedTick(2)).unwrap(); drop(c);
        let native = root.image(); let mut c = reopen(&control, &p, 1);
        c.store.fail_once(barrier);
        assert!(matches!(c.resolve_shutdown_visit(1, 0), Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert!(c.report().unavailable); assert!(!c.report().all_observed_drained());
        assert_eq!(c.resolve_shutdown_visit(c.revision(), 0), Err(JournalError::Unavailable));
        assert_eq!(root.image(), native); drop(c);
        let mut c = reopen(&control, &p, 1);
        assert_eq!(c.revision(), if barrier == JournalIo::DirectorySync { 2 } else { 1 });
        assert!(!c.report().all_observed_drained());
        c.resolve_shutdown_visit(c.revision(), 0).unwrap();
        assert_eq!(c.revision(), 2); assert_eq!(c.report().visits.len(), 1);
        assert!(c.report().all_observed_drained()); assert_eq!(root.image(), native);
    }
}

#[test]
fn exact_cut_refresh_withdraws_success_on_a_missing_or_newer_image() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root, 20);
    let p = plan(&h, 8); let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    let (index, attempt) = c.begin(0, 1, ShutdownVisitKind::Advance { at: ElapsedTick(2) }).unwrap();
    let result = c.campaign.advance_inner(index, attempt, &mut h, ElapsedTick(2));
    c.finish(index, attempt, result).unwrap().unwrap();
    let persisted = control.image(); let native = root.image();
    let hidden = root.0.join("hidden-canonical");
    std::fs::rename(root.store().join(storage::CANONICAL), &hidden).unwrap();
    assert!(c.resolve_shutdown_visit(c.revision(), 0).is_err());
    assert!(!c.report().current_session.all_observed_stopped()); assert!(!c.report().unavailable);
    std::fs::rename(&hidden, root.store().join(storage::CANONICAL)).unwrap();
    c.resolve_shutdown_visit(c.revision(), 0).unwrap();
    assert!(c.report().current_session.all_observed_stopped());
    assert_eq!(control.image(), persisted); assert_eq!(root.image(), native);
    h.progress_stop(h.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(c.resolve_shutdown_visit(c.revision(), 0), Err(Error::Stale.into()));
    assert!(!c.report().current_session.all_observed_stopped()); assert_eq!(control.image(), persisted);
    c.inspect_canonical(c.revision(), 1).unwrap().unwrap(); // Ordinary retention of a NEW head.
    assert!(c.report().all_observed_drained());
}

#[test]
fn fresh_resolution_preserves_the_latest_independent_prefix_and_original_stop_identity() {
    for conflicting in [false, true] {
        let root = Directory::new(); let control = Directory::new(); let mut h = create(&root, 20);
        let p = plan(&h, 8); let old = root.image();
        let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
        c.inspect_canonical(0, 1).unwrap().unwrap();
        enter(&mut c); let intent = control.image(); drop(c);
        h.stop_and_drain(h.revision(), request(&h, if conflicting { 901 } else { 900 }), ElapsedTick(3)).unwrap();
        let latest = root.image(); drop(h);
        let mut c = reopen(&control, &p, 3);
        std::fs::write(root.store().join(storage::CANONICAL), &old).unwrap();
        assert_eq!(c.resolve_shutdown_visit(3, 1), Err(Error::Stale.into()));
        assert_eq!(control.image(), intent);
        std::fs::write(root.store().join(storage::CANONICAL), &latest).unwrap();
        let result = c.resolve_shutdown_visit(3, 1);
        if conflicting {
            assert_eq!(result, Err(Error::Binding.into()));
            assert_eq!(control.image(), intent);
        } else { assert!(result.unwrap().stop.unwrap().drained()); }
        assert_eq!(root.image(), latest);
    }
}

#[test]
fn preconditions_and_later_success_cannot_rewrite_old_visit_history() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root, 20);
    let p = plan(&h, 8); let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    enter(&mut c); drop(c); let mut c = reopen(&control, &p, 1);
    let before = c.report(); let bytes = control.image();
    assert_eq!(c.resolve_shutdown_visit(0, 0), Err(Error::Stale.into()));
    assert_eq!(c.resolve_shutdown_visit(1, usize::MAX), Err(Error::Missing.into()));
    assert_eq!(c.report(), before); assert_eq!(control.image(), bytes);
    h.stop_and_drain(h.revision(), request(&h, 900), ElapsedTick(2)).unwrap();
    c.inspect_canonical(c.revision(), 1).unwrap().unwrap();
    let before = c.report(); let bytes = control.image();
    assert_eq!(c.resolve_shutdown_visit(c.revision(), 0), Err(Error::WrongState.into()));
    assert_eq!(c.resolve_shutdown_visit(c.revision(), 1), Err(Error::WrongState.into()));
    assert_eq!(c.report(), before); assert_eq!(control.image(), bytes);
}

#[test]
fn original_stopped_but_undrained_progress_is_not_promoted_to_drained() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root, 20);
    let p = plan(&h, 1); let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    let (index, attempt) = c.begin(0, 1, ShutdownVisitKind::Advance { at: ElapsedTick(2) }).unwrap();
    let native = c.campaign.advance_inner(index, attempt, &mut h, ElapsedTick(2)).unwrap().0;
    assert!(!native.stop.unwrap().drained()); drop(c);
    let mut c = reopen(&control, &p, 1); let bytes = root.image();
    let observed = c.resolve_shutdown_visit(1, 0).unwrap();
    assert!(!observed.stop.unwrap().drained());
    assert!(c.report().current_session.all_observed_stopped()); assert!(!c.report().all_observed_drained());
    assert_eq!(root.image(), bytes);
}
