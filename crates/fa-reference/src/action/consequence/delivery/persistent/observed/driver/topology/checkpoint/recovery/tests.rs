//! Real native source checks and guarded recovery. Transport is not authentication.
use super::*;
use crate::action::consequence::delivery::persistent::observed::driver::topology::{
    FileDriverTopology, checkpoint::MAX_TOPOLOGY_CHECKPOINT_BYTES};
use crate::action::consequence::delivery::persistent::observed::mediation::source::{
    TopologySourceWork, TOPOLOGY_REFRESH_READ_BYTES};
use crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements,
    mediated::{FileMediatedBootstrap, FileMediatedRequirements, FileTopologyRequirement},
    recovery::FileGuardedRecoveryStatus};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::mediation::CutCheck;
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::{collections::BTreeMap, fs, io::Cursor, sync::atomic::{AtomicU64, Ordering}};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-accounted-topology-{}-{stamp}-{}",
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
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("checkpoint recovery cleanup: {error}"); }
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
            InputProfileBinding { profile_id: 1, profile_bytes: b"accounted-source-fixture".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"));
fn text(generation: u64) -> String {
    let bytes = 8 * TOPOLOGY_REFRESH_READ_BYTES;
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
      "complete_inventory_generation":{generation},
      "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
      "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
      "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
      "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
      "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
               {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
      "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
      "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":8,"read_bytes":{bytes}}}}}"#)
}
fn loaded(generation: u64) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(text(generation).as_bytes(), INVENTORY).unwrap()
}
fn seed() -> (FileSupervisedDriver, Directory) {
    let root = Directory::new(); let expected = loaded(1);
    fs::write(root.topology(), text(1)).unwrap(); fs::write(root.inventory(), INVENTORY).unwrap();
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(root.store(), profile(),
        &guards(), None, FileMediatedBootstrap { topology: expected.graph().clone(),
            reserve: RecoveryReserve::terminal(), publication: None }, None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.topology(), root.inventory(), expected).unwrap();
    let (port, mut driver) = host.into_supervised_driver(); drop(port);
    driver.require_topology(revision(&driver), FileDriverTopology::new(source)).unwrap();
    certified(driver.refresh_topology(revision(&driver)).unwrap());
    (driver, root)
}
fn capture(driver: &FileSupervisedDriver) -> TopologyCheckpoint {
    let generation = driver.topology().unwrap().source().expected().graph().spec().generation;
    driver.topology_checkpoint(revision(driver), text(generation).as_bytes(), INVENTORY).unwrap()
}
fn revision(driver: &FileSupervisedDriver) -> u64 { driver.supervisor().host().unwrap().revision() }
fn certified(report: TopologyRefresh) {
    assert!(report.refusal.is_none()); assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
}
fn encoded(record: &TopologyCheckpoint) -> Vec<u8> {
    let mut bytes = Vec::new(); record.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap(); bytes
}
fn decoded(bytes: &[u8]) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(bytes), bytes.len()).unwrap()
}
fn selection(root: &Directory, generation: u64) -> TopologyCheckpointResume {
    TopologyCheckpointResume { topology_path: root.topology(), inventory_path: root.inventory(),
        next: loaded(generation + 1) }
}
fn recover(root: &Directory, record: &TopologyCheckpoint, generation: u64)
    -> (FileSupervisedDriver, FileMediationObserver)
{
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
            credential_epoch: None, minimum: record.minimum() },
        topology: FileTopologyRequirement { initial: loaded(1).graph().clone(), current: loaded(generation).graph().clone(), available: true },
        prediction: None, evaluation: None,
    };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(),
        &expected, record.history_anchor()).unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (mut host, roles) = run.finish().unwrap();
    host.observe_time(host.revision(), ElapsedTick(generation + 1)).unwrap();
    let (port, driver) = host.into_supervised_driver(); drop(port);
    (driver, roles.topology_observer)
}

#[test]
fn checkpoint_round_trip_drives_two_native_recoveries_without_resetting_work() {
    let (mut driver, root) = seed();
    for (generation, operation) in [(1, 2), (2, 4)] {
        let before = root.bytes(); let record = capture(&driver);
        let bytes = encoded(&record); assert_eq!(decoded(&bytes), record);
        assert_eq!(record.work().checks, generation); assert_eq!(root.bytes(), before);
        drop(driver);
        let (mut recovered, observer) = recover(&root, &decoded(&bytes), generation);
        fs::write(root.topology(), text(generation + 1)).unwrap();
        let report = recovered.resume_topology_checkpoint(revision(&recovered), operation, observer,
            &decoded(&bytes), selection(&root, generation)).unwrap();
        certified(report);
        let after = capture(&recovered);
        assert_eq!(after.work().checks, generation + 1); assert_eq!(after.last_operation(), operation + 1);
        assert_eq!(after.work().reserved_read_bytes, (generation + 1) * TOPOLOGY_REFRESH_READ_BYTES);
        assert_eq!(recovered.supervisor().host().unwrap().inspect().executions, 0);
        driver = recovered;
    }
}

#[test]
fn retention_needs_the_original_healthy_owner_but_does_not_reread_missing_files() {
    let (driver, root) = seed(); let (foreign, _foreign_root) = seed();
    let before = root.bytes(); let rev = revision(&driver);
    let record = capture(&driver);
    fs::remove_file(root.inventory()).unwrap();
    assert_eq!(capture(&driver), record); assert_eq!(root.bytes(), before);
    assert!(driver.topology_checkpoint(rev - 1, text(1).as_bytes(), INVENTORY).is_err());
    assert!(driver.topology().unwrap().checkpoint(&foreign.supervisor().host().unwrap(), text(1).as_bytes(), INVENTORY).is_err());
    assert_eq!(root.bytes(), before);
}

#[test]
fn stale_checkpoint_cannot_hide_later_native_source_work_but_latest_record_resumes() {
    for latest in [false, true] {
        let (mut driver, root) = seed();
        let old = capture(&driver);
        certified(driver.refresh_topology(revision(&driver)).unwrap());
        let record = if latest { capture(&driver) } else { old };
        drop(driver);
        // Native anchored recovery legitimately accepts the later history. Only
        // source accounting must reject the old expenditure for that successor.
        let (mut driver, observer) = recover(&root, &record, 1); let before = root.bytes();
        fs::write(root.topology(), text(2)).unwrap();
        let result = driver.resume_topology_checkpoint(revision(&driver), 3, observer, &record, selection(&root, 1));
        if latest {
            certified(result.unwrap()); assert_eq!(driver.topology().unwrap().source().work().checks, 3);
        } else {
            assert_eq!(result, Err(JournalError::Contract(Error::Stale)));
            assert!(driver.topology().is_none()); assert_eq!(root.bytes(), before);
        }
    }
}

#[test]
fn misbound_or_exhausted_records_refuse_before_source_installation_or_storage_change() {
    for mode in ["foreign-history", "wrong-generation", "wrong-limits", "exhausted", "stale-revision", "wrong-floors"] {
        let (driver, root) = seed(); let mut record = capture(&driver);
        drop(driver); let (mut driver, observer) = recover(&root, &record, 1);
        let mut selected = selection(&root, 1);
        match mode {
            "foreign-history" => {
                let (foreign, _root) = seed(); record = capture(&foreign);
            }
            "wrong-generation" => selected.next = loaded(1),
            "wrong-limits" => selected.next = LoadedAuthorityTopology::from_json_bytes(
                text(2).replace("\"checks\":8", "\"checks\":9").as_bytes(), INVENTORY).unwrap(),
            "wrong-floors" => record.control_sequence = u64::MAX,
            "exhausted" => {
                record.work = TopologySourceWork { checks: 8, reserved_read_bytes: 8 * TOPOLOGY_REFRESH_READ_BYTES };
                record.last_operation = 8;
            }
            _ => {}
        }
        let before = root.bytes(); let rev = revision(&driver);
        let selected_revision = if mode == "stale-revision" { rev - 1 } else { rev };
        let result = driver.resume_topology_checkpoint(selected_revision, 9, observer, &record, selected);
        assert!(result.is_err(), "{mode}"); assert_eq!(root.bytes(), before, "{mode}");
        assert!(driver.topology().is_none()); assert_eq!(revision(&driver), rev);
    }
}

#[test]
fn post_recovery_source_loss_remains_withdrawn_and_spent_work_can_be_retained() {
    let (driver, root) = seed(); let record = decoded(&encoded(&capture(&driver)));
    drop(driver); let (mut driver, observer) = recover(&root, &record, 1);
    fs::remove_file(root.inventory()).unwrap();
    let report = driver.resume_topology_checkpoint(revision(&driver), 2, observer, &record, selection(&root, 1)).unwrap();
    assert!(report.refusal.is_some());
    let host = driver.supervisor().host().unwrap();
    assert!(!host.mediation_snapshot().unwrap().available); assert_eq!(host.inspect().executions, 0);
    let after = capture(&driver);
    assert_eq!(after.work().checks, 2); assert_eq!(after.last_operation(), 3);
    assert_eq!(decoded(&encoded(&after)), after);
}
