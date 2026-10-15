//! Native durable recovery and source intake, not simulated source success.
use super::*;
use super::super::{TopologySourceRefusal, TopologyRefresh};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::delivery::persistent::observed::guarded::{FileGuardSet,
    FileRecoveryFloor, FileRecoveryRequirements};
use crate::action::consequence::delivery::persistent::observed::guarded::recovery::FileGuardedRecoveryStatus;
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::{
    FileMediatedBootstrap, FileMediatedRequirements, FileTopologyRequirement};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::{collections::BTreeMap, fs, sync::atomic::{AtomicU64, Ordering}};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-source-resume-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn topology(&self) -> PathBuf { self.0.join("topology.json") }
    fn inventory(&self) -> PathBuf { self.0.join("inventory.json") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("source resume cleanup: {error}"); }
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
            suspend_at_incident: 100,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
                MemberPolicy { cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"source-resume-fixture".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
const INVENTORY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"));
fn document(generation: u64, inventory: u64, checks: u64, budget: usize) -> String {
    let bytes = checks * TOPOLOGY_REFRESH_READ_BYTES;
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{inventory},
      "complete_inventory_generation":{inventory},
      "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
      "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
      "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
      "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
      "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
               {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
      "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
      "gates":[2],"check_edge_budget":{budget},"source_limits":{{"checks":{checks},"read_bytes":{bytes}}}}}"#)
}
fn text(generation: u64) -> String { document(generation, generation, 8, MAX_CHECK_EDGE_VISITS) }
fn loaded(text: &str) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(text.as_bytes(), INVENTORY.as_bytes()).unwrap()
}
fn requirements(initial: &LoadedAuthorityTopology, current: &LoadedAuthorityTopology,
    available: bool) -> FileMediatedRequirements {
    FileMediatedRequirements {
        oversight: FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
            credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 } },
        topology: FileTopologyRequirement { initial: initial.graph().clone(), current: current.graph().clone(), available },
        prediction: None, evaluation: None,
    }
}
struct Recovered {
    host: FileOversight,
    observer: FileMediationObserver,
    source: TopologySourceRecovery,
    root: Directory,
}
fn recovered() -> Recovered {
    let root = Directory::new(); let previous = loaded(&text(1)); let next = loaded(&text(2));
    fs::write(root.topology(), text(1)).unwrap(); fs::write(root.inventory(), INVENTORY).unwrap();
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(root.store(), profile(),
        &guards(), None, FileMediatedBootstrap { topology: previous.graph().clone(),
            reserve: RecoveryReserve::terminal(), publication: None }, None, None).unwrap();
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(1)).unwrap();
    let mut source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.topology(), root.inventory(), previous.clone()).unwrap();
    let revision = host.revision();
    assert_verified(source.refresh(&mut host, revision, 1).unwrap());
    let selected = source.recovery_requirements(next);
    let anchor = host.history_anchor().unwrap();
    drop(source); drop(host);
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(),
        &requirements(&previous, &previous, true), &anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (host, roles) = run.finish().unwrap();
    assert!(!host.mediation_snapshot().unwrap().available);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    Recovered { host, observer: roles.topology_observer, source: selected, root }
}
fn assert_verified(report: TopologyRefresh) {
    assert!(report.refusal.is_none());
    assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
}

#[test]
fn recovered_source_preserves_work_requires_actual_new_files_and_survives_another_recovery() {
    let Recovered { mut host, observer, source, root } = recovered();
    let work = source.work; let previous = source.previous.clone(); let next = source.next.clone();
    let revision = host.revision(); let units = host.inspect().control.ledger.available;
    let mut resumed = observer.resume_topology_source(&mut host, revision, 2, source).unwrap();
    assert_eq!(resumed.work(), work); assert_eq!(resumed.last_operation(), 2);
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, units);
    let current = host.mediation_snapshot().unwrap();
    assert_eq!(&current.graph, next.graph()); assert!(current.available); assert!(current.accepted.is_none());
    assert!(resumed.last_check().is_none());
    // Reinstatement is not permission: actual files still contain generation 1.
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
    fs::write(root.topology(), text(2)).unwrap();
    let revision = host.revision(); assert_verified(resumed.refresh(&mut host, revision, 3).unwrap());
    assert_eq!(resumed.work().checks, work.checks + 1);
    assert_eq!(resumed.work().reserved_read_bytes, work.reserved_read_bytes + TOPOLOGY_REFRESH_READ_BYTES);
    let again = resumed.recovery_requirements(loaded(&text(3)));
    let anchor = host.history_anchor().unwrap();
    drop(resumed); drop(host);
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(),
        &requirements(&previous, &next, true), &anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (mut host, roles) = run.finish().unwrap();
    let revision = host.revision();
    let mut resumed = roles.topology_observer.resume_topology_source(&mut host, revision, 4, again).unwrap();
    assert_eq!(resumed.work().checks, 2); assert_eq!(resumed.last_operation(), 4);
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(3)).unwrap();
    fs::write(root.topology(), text(3)).unwrap();
    let revision = host.revision(); assert_verified(resumed.refresh(&mut host, revision, 5).unwrap());
    assert_eq!(resumed.work().checks, 3); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn loss_or_substitution_after_reinstatement_uses_original_withdrawal_not_an_old_cut() {
    for missing in [false, true] {
        let Recovered { mut host, observer, source, root } = recovered();
        let revision = host.revision();
        let mut resumed = observer.resume_topology_source(&mut host, revision, 2, source).unwrap();
        if missing { fs::rename(root.inventory(), root.0.join("retained-inventory")).unwrap(); }
        let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
        let revision = host.revision(); let report = resumed.refresh(&mut host, revision, 3).unwrap();
        assert!(matches!(report.refusal, Some(TopologySourceRefusal::Input(_)) | Some(TopologySourceRefusal::ChangedInput)));
        assert_eq!(resumed.work().checks, 2);
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn invalid_continuity_and_budget_requests_do_not_modify_recovered_storage() {
    for mode in 0..11 {
        let Recovered { mut host, observer, mut source, root } = recovered();
        let mut operation = 2;
        match mode {
            0 => source.next = source.previous.clone(),
            1 => source.next = loaded(&document(2, 1, 8, MAX_CHECK_EDGE_VISITS)),
            2 => source.next = loaded(&document(2, 2, 9, MAX_CHECK_EDGE_VISITS)),
            3 => source.next = loaded(&document(2, 2, 8, MAX_CHECK_EDGE_VISITS - 1)),
            4 => source.previous = source.next.clone(),
            5 => source.work.reserved_read_bytes = 0,
            6 => source.work = TopologySourceWork { checks: 8, reserved_read_bytes: 8 * TOPOLOGY_REFRESH_READ_BYTES },
            7 => source.topology_path = source.inventory_path.clone(),
            8 => source.topology_path = PathBuf::from("relative.json"),
            9 => operation = source.last_operation,
            _ => operation = u64::MAX,
        }
        let before = root.bytes(); let revision = host.revision(); let snapshot = host.mediation_snapshot().unwrap();
        assert!(observer.resume_topology_source(&mut host, revision, operation, source).is_err(), "mode {mode}");
        assert_eq!(root.bytes(), before); assert_eq!(host.revision(), revision);
        assert_eq!(host.mediation_snapshot().unwrap(), snapshot); assert!(host.storage_failure().is_none());
    }
}

#[test]
fn stale_revision_and_foreign_observer_refuse_without_changing_either_owner() {
    let Recovered { mut host, observer, source, root } = recovered();
    let before = root.bytes(); let revision = host.revision();
    assert!(observer.resume_topology_source(&mut host, revision - 1, 2, source).is_err());
    assert_eq!(root.bytes(), before);
    let Recovered { host: other_host, observer, source, root: other_root } = recovered();
    let other_before = other_root.bytes();
    assert!(observer.resume_topology_source(&mut host, revision, 2, source).is_err());
    assert_eq!(root.bytes(), before); assert_eq!(other_root.bytes(), other_before);
    assert!(other_host.storage_failure().is_none()); assert!(host.storage_failure().is_none());
}

#[test]
fn a_live_source_cannot_be_restarted_to_reset_its_allowance() {
    let Recovered { mut host, observer, source, root } = recovered();
    let revision = host.revision();
    let resumed = observer.resume_topology_source(&mut host, revision, 2, source).unwrap();
    let requirements = resumed.recovery_requirements(loaded(&text(3)));
    let before = root.bytes(); let revision = host.revision();
    assert!(matches!(resumed.observer.resume_topology_source(&mut host, revision, 3, requirements),
        Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(root.bytes(), before); assert!(host.mediation_snapshot().unwrap().accepted.is_none());
}

mod driver;
