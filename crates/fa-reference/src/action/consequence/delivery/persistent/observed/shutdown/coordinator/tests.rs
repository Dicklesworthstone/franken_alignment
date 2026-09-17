//! Actual Store barriers; not a claim about hardware power loss.
use super::*;
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo};
use crate::action::{Purpose, ResolvedTarget};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
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
        let path = std::env::temp_dir().join(format!("fa-shutdown-coordinator-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("shutdown test cleanup: {error}"); }
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
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99,
            limits: crate::action::consequence::delivery::persistent::JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"shutdown-fixture".to_vec(), model_epoch: 1,
                tokenizer_epoch: 1, policy_epoch: 0 }, 1, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn create(root: &Directory) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}

fn plan(host: &FileOversight) -> FileShutdownPlan {
    FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap()], 32, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}

#[test]
fn every_coordinator_intent_and_completion_barrier_preserves_native_stop_truth() {
    for completion in [false, true] {
        for barrier in BARRIERS {
            let root = Directory::new(); let control = Directory::new(); let mut h = create(&root);
            let p = plan(&h); let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
            if completion {
                let (index, attempt) = c.begin(0, 1, ShutdownVisitKind::Advance { at: ElapsedTick(2) }).unwrap();
                let result = c.campaign.advance_inner(index, attempt, &mut h, ElapsedTick(2));
                assert!(h.inspect().stop.is_some());
                c.store.fail_once(barrier);
                assert!(c.finish(index, attempt, result).is_err());
                assert_eq!(c.report().revision, 1);
            } else {
                c.store.fail_once(barrier);
                assert!(c.advance(0, 1, &mut h, ElapsedTick(2)).is_err());
                assert!(h.inspect().stop.is_none()); assert_eq!(c.report().revision, 0);
            }
            assert!(c.report().unavailable); assert!(!c.report().all_observed_drained());
            assert_eq!(c.advance(c.revision(), 1, &mut h, ElapsedTick(2)), Err(JournalError::Unavailable));
            drop(c);
            let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 0).unwrap();
            let visible = barrier == JournalIo::DirectorySync;
            assert_eq!(c.report().visits.len(), usize::from(completion || visible));
            assert_eq!(c.revision(), if completion { if visible { 2 } else { 1 } } else { u64::from(visible) });
            assert!(!c.report().all_observed_drained());
            c.inspect_canonical(c.revision(), 1).unwrap().unwrap();
            assert_eq!(c.report().current_session.all_observed_stopped(), completion);
            while !c.report().all_observed_drained() {
                c.advance(c.revision(), 1, &mut h, ElapsedTick(2)).unwrap().unwrap();
            }
            assert_eq!(h.inspect().executions, 0);
        }
    }
}

#[test]
fn caught_interruption_requires_reopen_without_repeating_a_native_stop() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root); let p = plan(&h);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (index, attempt) = c.begin(0, 1, ShutdownVisitKind::Advance { at: ElapsedTick(2) }).unwrap();
        c.campaign.advance_inner(index, attempt, &mut h, ElapsedTick(2)).unwrap();
        panic!("simulate controller interruption after actual domain acknowledgment");
    }));
    assert!(interrupted.is_err()); assert!(c.report().unavailable);
    assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
    let stopped = h.inspect(); drop(c);
    let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 1).unwrap();
    assert!(!c.report().current_session.all_observed_stopped());
    c.inspect_canonical(c.revision(), 1).unwrap().unwrap(); assert_eq!(h.inspect(), stopped);
    assert!(c.report().current_session.all_observed_stopped());
    c.advance(c.revision(), 1, &mut h, ElapsedTick(2)).unwrap().unwrap();
    assert!(c.report().all_observed_drained());
    assert_eq!(h.inspect().stop, stopped.stop);
}

#[test]
fn coordinator_image_rejects_every_truncation_false_revision_and_changed_plan() {
    let root = Directory::new(); let control = Directory::new(); let mut h = create(&root); let p = plan(&h);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    c.advance(0, 1, &mut h, ElapsedTick(2)).unwrap().unwrap();
    let bytes = c.encoded(c.revision()).unwrap();
    for end in 0..bytes.len() {
        assert!(codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 0, &bytes[..end]).is_err());
    }
    assert!(codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 3, &bytes).is_err());
    let false_revision = c.encoded(99).unwrap();
    assert!(codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 0, &false_revision).is_err());
    let mut suffix = bytes.clone(); suffix.push(0);
    assert!(codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 0, &suffix).is_err());
    let restored = codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 2, &bytes).unwrap();
    assert_eq!(restored.visits.len(), 1); assert_eq!(restored.revision, 2);
    assert!(!restored.campaign.report().all_observed_stopped());
    assert_eq!(restored.campaign.slots[0].revision, h.revision() as usize);
}

#[test]
fn completion_capacity_exhaustion_keeps_the_durable_intent_not_a_false_success() {
    for extra in [0, 4096] {
        let root = Directory::new(); let control = Directory::new(); let mut h = create(&root); let p = plan(&h);
        let mut staged = p.start(); staged.enter(1, FileShutdownStep::InspectOwner).unwrap();
        let visit = ShutdownVisit { domain: 1, kind: ShutdownVisitKind::Advance { at: ElapsedTick(2) },
            result: ShutdownVisitResult::Entered };
        let required = codec::encode(&control.store(), &p.encode().unwrap(), MAX_JOURNAL_BYTES,
            1, &[visit], &staged).unwrap().len();
        let cap = required + extra;
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        let result = c.advance(0, 1, &mut h, ElapsedTick(2));
        assert!(h.inspect().stop.is_some());
        if extra == 0 {
            assert_eq!(result, Err(JournalError::Contract(Error::Limit)));
            assert!(c.report().unavailable); assert_eq!(c.revision(), 1);
            assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
            drop(c);
            let c = FileShutdownCoordinator::open(control.store(), &p, cap, 1).unwrap();
            assert_eq!(c.report().visits.len(), 1); assert!(!c.report().all_observed_drained());
            // A separate original read can see the native stop. Coordinator
            // capacity exhaustion never reverses it or manufactures a drain.
            let mut inspect = p.start(); inspect.inspect_canonical(1).unwrap();
            assert!(inspect.report().all_observed_stopped()); assert!(!inspect.report().all_observed_drained());
        } else {
            result.unwrap().unwrap(); assert_eq!(c.revision(), 2);
            assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Observed { .. }));
            c.advance(c.revision(), 1, &mut h, ElapsedTick(2)).unwrap().unwrap();
            assert!(c.report().all_observed_drained());
        }
    }
}
