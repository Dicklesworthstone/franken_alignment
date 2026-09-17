//! Actual Store barriers; not a claim about hardware power loss.
use super::*;
use super::super::{BaseEvent, Event, FileDeliveryProfile, JournalIo, Machine};
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
        let path = std::env::temp_dir().join(format!("fa-shutdown-{}-{stamp}-{}",
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
            limits: super::super::super::JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: vec![], model_epoch: 1,
                tokenizer_epoch: 1, policy_epoch: 0 }, 1, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn create(root: &Directory) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
fn campaign(host: &FileOversight) -> FileShutdownCampaign {
    FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap()], 32, MAX_SHUTDOWN_HEAD_BYTES).unwrap().start()
}

#[test]
fn stop_and_drain_faults_keep_unacknowledged_results_out_of_the_aggregate() {
    for draining in [false, true] {
        for barrier in BARRIERS {
            let root = Directory::new(); let mut host = create(&root); let mut campaign = campaign(&host);
            if draining { campaign.advance(1, &mut host, ElapsedTick(2)).unwrap(); }
            let before = host.inspect();
            host.store.fail_once(barrier);
            let error = campaign.advance(1, &mut host, ElapsedTick(2)).unwrap_err();
            let JournalError::Io(failure) = error else { panic!("expected actual storage fault"); };
            assert_eq!(failure.operation, barrier);
            assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some());
            assert!(!campaign.report().all_observed_stopped());
            assert!(!campaign.report().all_observed_drained());
            let pending = std::fs::read(root.store().join("delivery.pending")).ok();
            let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
            let read = campaign.inspect_canonical(1).unwrap();
            let visible = barrier == JournalIo::DirectorySync;
            assert_eq!(read.stop.is_some(), draining || visible);
            assert_eq!(read.stop.as_ref().is_some_and(StopProgress::drained), draining && visible);
            assert_eq!(std::fs::read(root.store().join("delivery.pending")).ok(), pending);
            assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
            assert!(host.storage_failure().is_some());
            drop(host);
            let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
            campaign.advance(1, &mut recovered, ElapsedTick(2)).unwrap();
            if !campaign.report().all_observed_drained() {
                campaign.advance(1, &mut recovered, ElapsedTick(2)).unwrap();
            }
            assert!(campaign.report().all_observed_drained());
            let revision = recovered.revision();
            campaign.advance(1, &mut recovered, ElapsedTick(0)).unwrap();
            assert_eq!(recovered.revision(), revision);
            assert_eq!(recovered.inspect().executions, 0);
        }
    }
}

#[test]
fn valid_equal_counter_fork_and_invalid_suffix_cannot_hide_behind_a_real_stop() {
    let root = Directory::new(); let mut host = create(&root); let mut campaign = campaign(&host);
    campaign.advance(1, &mut host, ElapsedTick(2)).unwrap();
    let path = root.store().join("delivery.bin"); let original = std::fs::read(&path).unwrap();
    let mut fork = host.events.clone();
    let Some(Event::Core(BaseEvent::Stop(request))) = fork.last_mut() else { panic!("real last stop"); };
    request.operation = 901;
    // It IS a valid history with equal counters, not merely corrupt framing.
    let alternate = Machine::replay(&profile(), &fork).unwrap().snapshot(fork.len());
    assert_eq!(alternate.revision, host.revision());
    assert_eq!(alternate.control.sequence, host.inspect().control.sequence);
    let bytes = journal::encode(&profile(), host.store.identity(), &fork).unwrap();
    assert_eq!(bytes.len(), original.len()); std::fs::write(&path, &bytes).unwrap();
    assert_eq!(campaign.inspect_canonical(1), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut invalid = host.events.clone(); invalid.push(Event::Core(BaseEvent::Publish(999)));
    let bytes = journal::encode(&profile(), host.store.identity(), &invalid).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(campaign.inspect_canonical(1).is_err());
    assert!(!campaign.report().all_observed_stopped());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::write(&path, &original).unwrap();
    campaign.inspect_canonical(1).unwrap(); assert!(campaign.report().all_observed_stopped());
}

#[test]
fn stopping_remains_available_during_an_interrupted_source_acquisition() {
    let root = Directory::new(); let mut host = create(&root); let mut campaign = campaign(&host);
    host.source_interrupted = true;
    campaign.advance(1, &mut host, ElapsedTick(2)).unwrap();
    assert!(host.source_interrupted); // stopping never fabricates new source evidence
    campaign.advance(1, &mut host, ElapsedTick(2)).unwrap();
    assert!(host.source_interrupted);
    assert!(campaign.report().all_observed_drained());
}
