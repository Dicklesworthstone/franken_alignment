//! Shared original-profile fixtures for recovery execution and evidence resolution.
use super::*;
use crate::action::{Purpose, ResolvedTarget};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
pub(super) use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
pub(super) struct Directory(pub(super) PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-shutdown-resolution-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn image(&self) -> Vec<u8> { std::fs::read(self.store().join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("resolution test cleanup: {error}"); }
    }
}
pub(super) fn profile() -> FileOversightProfile {
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
pub(super) fn create(root: &Directory, events: usize) -> FileOversight {
    let mut p = profile(); p.delivery.limits.events = events;
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
pub(super) fn plan(host: &FileOversight, visits: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap()], visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
pub(super) fn reopen(root: &Directory, p: &FileShutdownPlan, floor: u64) -> FileShutdownCoordinator {
    FileShutdownCoordinator::open(root.store(), p, MAX_JOURNAL_BYTES, floor).unwrap()
}
pub(super) fn request(host: &FileOversight, operation: u64) -> StopRequest {
    let before = host.inspect();
    StopRequest { operation, expected_control_sequence: before.control.sequence,
        expected_authority_epoch: before.control.ledger.epoch }
}
pub(super) fn enter(c: &mut FileShutdownCoordinator) -> (usize, usize) {
    c.begin(c.revision(), 1, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }).unwrap()
}

pub(super) fn member(root: &Directory, id: u64, clock_domain: u64) -> FileOversight {
    let mut p = profile();
    p.delivery.scope.run = id;
    p.delivery.scope.authority = 100 + id;
    p.delivery.clock_domain = clock_domain;
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host
}
