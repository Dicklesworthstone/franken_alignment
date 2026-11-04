//! Original mediated recovery, real source files and the native cut checker.
//! These declarations do not prove operating-system containment.
use super::*;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{FileOversightProfile,
    guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        mediated::{FileMediatedBootstrap, FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement},
        recovery::FileGuardedRecoveryStatus},
    mediation::{FileMediationObserver, FileMediationUpdate},
    driver::topology::FileDriverTopology};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::full_input::InputProfileBinding;
use crate::perimeter_inventory::topology::LoadedAuthorityTopology;
use crate::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use crate::reducer::Caps;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

const INVENTORY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-withdrawn-source-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn topology(&self) -> PathBuf { self.0.join("topology.json") }
    fn inventory(&self) -> PathBuf { self.0.join("inventory.json") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
    fn install(&self, generation: u64) {
        fs::write(self.topology(), document(generation)).unwrap();
        fs::write(self.inventory(), INVENTORY).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("withdrawn topology cleanup: {error}"); }
    }
}
fn document(generation: u64) -> String {
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":8,"read_bytes":67108864}}}}"#)
}
fn loaded(generation: u64) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(document(generation).as_bytes(), INVENTORY.as_bytes()).unwrap()
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::AuditOnly }, vec![], vec![2], vec![3], 0).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
                MemberPolicy { cohort: "one".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
            max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"source-recovery".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn recovered(root: &Directory, operation: u64) -> (FileOversight, FileMediatedRoles, FileMediationObserver) {
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(root.store(), profile(), &guards(), None,
        FileMediatedBootstrap { topology: loaded(1).graph().clone(), reserve: RecoveryReserve::terminal(), publication: None },
        None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let revision = host.revision();
    let update = FileMediationUpdate { operation, expected_generation: 1,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next: Some(loaded(2).graph().clone()) };
    roles.topology_observer.update(&mut host, revision, &update).unwrap();
    let state = host.inspect();
    let expected = FileMediatedRequirements { oversight: FileRecoveryRequirements {
        guards: guards(), effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: state.revision, control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch },
    }, topology: FileTopologyRequirement { initial: loaded(1).graph().clone(), current: loaded(2).graph().clone(), available: true },
        prediction: None, evaluation: None };
    let anchor = host.history_anchor().unwrap(); drop(host);
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying { run.advance(run.progress().replayed_events, 1).unwrap(); }
    let (host, current) = run.finish().unwrap();
    (host, current, roles.topology_observer)
}
fn bind(root: &Directory, host: &FileOversight, role: FileMediationObserver) -> FileTopologySource {
    role.bind_withdrawn_topology_source(host, host.revision(), root.topology(), root.inventory(), loaded(2)).unwrap()
}
#[test]
fn recovered_custody_requires_new_evidence_and_transfers_unchanged_to_the_driver() {
    let root = Directory::new(); root.install(3);
    let (mut host, roles, _) = recovered(&root, 40); let before = root.bytes();
    let mut source = bind(&root, &host, roles.topology_observer);
    assert_eq!(source.last_operation(), 40);
    assert_eq!(root.bytes(), before); assert!(!host.mediation_snapshot().unwrap().available);
    let revision = host.revision();
    let report = source.refresh(&mut host, revision, 41).unwrap();
    assert_eq!(report.refusal, Some(super::super::TopologySourceRefusal::CoverageUnavailable));
    assert_eq!(source.work(), super::super::TopologySourceWork::default());
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    source.rebind(&mut host, revision, 42, loaded(3)).unwrap();
    assert_eq!(host.inspect().control.ledger.epoch, epoch + 1);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    let (port, mut driver) = host.into_supervised_driver();
    let revision = driver.supervisor().host().unwrap().revision();
    driver.require_topology(revision, FileDriverTopology::new(source)).unwrap();
    let checked = driver.refresh_topology(revision).unwrap();
    assert!(matches!(checked.checked, Some(Ok(CutCheck::Verified(_)))));
    assert_eq!(driver.topology().unwrap().source().last_operation(), 43);
    assert_eq!(driver.topology().unwrap().source().work().checks, 1);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    drop(port);
}
#[test]
fn source_loss_after_rebind_uses_a_fresh_native_operation_not_a_pre_recovery_id() {
    let root = Directory::new(); root.install(3);
    let (mut host, roles, _) = recovered(&root, 40);
    let mut source = bind(&root, &host, roles.topology_observer); let revision = host.revision();
    source.rebind(&mut host, revision, 41, loaded(3)).unwrap();
    fs::remove_file(root.inventory()).unwrap(); let revision = host.revision();
    let report = source.refresh_next(&mut host, revision).unwrap();
    assert!(report.refusal.is_some()); assert!(report.withdrawal.is_some());
    assert!(host.mediation_update(42).unwrap().is_some());
    assert!(!host.mediation_snapshot().unwrap().available);
    assert!(host.storage_failure().is_none()); assert_eq!(source.last_operation(), 42);
}
#[test]
fn foreign_roles_wrong_graphs_bad_paths_and_stale_revisions_leave_original_history_unchanged() {
    for mode in 0..5 {
        let root = Directory::new(); let (host, roles, old) = recovered(&root, 40); let before = root.bytes();
        let role = if mode == 0 { old } else { roles.topology_observer };
        let expected = loaded(if mode == 1 { 1 } else { 2 });
        let revision = host.revision() + u64::from(mode == 2);
        let path = if mode == 3 { PathBuf::from("relative.json") } else { root.topology() };
        let inventory = if mode == 4 { path.clone() } else { root.inventory() };
        assert!(role.bind_withdrawn_topology_source(&host, revision, path, inventory, expected).is_err());
        assert_eq!(root.bytes(), before); assert!(host.storage_failure().is_none());
    }
}
#[test]
fn old_generations_and_reused_operations_refuse_but_the_corrected_rebind_succeeds() {
    let root = Directory::new(); root.install(3); let (mut host, roles, _) = recovered(&root, 40);
    let mut source = bind(&root, &host, roles.topology_observer); let before = root.bytes(); let revision = host.revision();
    assert!(source.rebind(&mut host, revision, 40, loaded(3)).is_err());
    assert!(source.rebind(&mut host, revision, 41, loaded(2)).is_err());
    assert_eq!(root.bytes(), before); assert_eq!(source.last_operation(), 40);
    let receipt = source.rebind(&mut host, revision, 41, loaded(3)).unwrap();
    let bytes = root.bytes();
    assert_eq!(source.rebind(&mut host, revision, 41, loaded(3)).unwrap(), receipt);
    assert_eq!(root.bytes(), bytes);
    let revision = host.revision();
    assert!(matches!(source.refresh_next(&mut host, revision).unwrap().checked, Some(Ok(CutCheck::Verified(_)))));
}
#[test]
fn native_rebind_persistence_faults_never_restore_old_eligibility() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); root.install(3); let (mut host, roles, _) = recovered(&root, 40);
        let mut source = bind(&root, &host, roles.topology_observer); let before = root.bytes(); let revision = host.revision();
        host.store.fail_once(barrier);
        assert!(matches!(source.rebind(&mut host, revision, 41, loaded(3)), Err(JournalError::Io(error)) if error.operation == barrier));
        assert!(host.storage_failure().is_some()); assert!(host.history_anchor().is_err());
        assert!(source.refresh_next(&mut host, revision).is_err());
        assert_eq!(source.last_operation(), 40);
        if barrier != JournalIo::DirectorySync { assert_eq!(root.bytes(), before); }
    }
}
#[test]
fn an_exhausted_native_operation_space_cannot_be_restarted_at_zero() {
    let root = Directory::new(); let (host, roles, _) = recovered(&root, u64::MAX); let before = root.bytes();
    assert!(matches!(roles.topology_observer.bind_withdrawn_topology_source(&host, host.revision(),
        root.topology(), root.inventory(), loaded(2)), Err(JournalError::Contract(Error::Overflow))));
    assert_eq!(root.bytes(), before); assert!(host.storage_failure().is_none());
}
