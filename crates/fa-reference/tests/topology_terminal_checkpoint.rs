//! Public native topology custody across acknowledged stop and drain operations.
//! Real source checks and journal transactions; synthetic graph data is not a
//! claim that a deployed operating-system perimeter has no bypass.
#![cfg(unix)]
#![forbid(unsafe_code)]

use fa_reference::Error;
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{StopRequest,
    persistent::{FileDeliveryProfile, JournalError, JournalLimits, RecoveryReserve,
        observed::{FileOversight, FileOversightProfile,
            driver::{FileSupervisedDriver, topology::{FileDriverTopology,
                checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES}}},
            guarded::{FileGuardSet, FileRecoveryRequirements,
                mediated::{FileMediatedBootstrap, FileMediatedRecovery,
                    FileMediatedRequirements, FileTopologyRequirement},
                recovery::FileGuardedRecoveryStatus}}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate}};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract,
    human::HumanReviewPolicy};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::reducer::Caps;
use std::{cell::Cell, collections::BTreeMap, fs, io::Cursor, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

const INVENTORY: &[u8] = include_bytes!("fixtures/authority_topology_inventory.json");
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-public-terminal-checkpoint-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("store") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("terminal checkpoint cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::AuditOnly },
            Vec::new(), vec![1], vec![2], 0).unwrap(),
        suspend_at_incident: 100, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
            MemberPolicy { cohort: "one".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"fixture".to_vec(),
            tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
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
      "gates":[2],"check_edge_budget":65536,"source_limits":{{"checks":32,"read_bytes":67108928}}}}"#)
}
fn loaded(generation: u64) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(document(generation).as_bytes(), INVENTORY).unwrap()
}
fn revision(driver: &FileSupervisedDriver) -> u64 {
    driver.supervisor().host().unwrap().revision()
}
fn refresh(driver: &mut FileSupervisedDriver) {
    let report = driver.refresh_topology(revision(driver)).unwrap();
    assert!(report.refusal.is_none());
    assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
}
fn seed(root: &Directory, at: ElapsedTick) -> FileSupervisedDriver {
    fs::write(root.0.join("topology.json"), document(1)).unwrap();
    fs::write(root.0.join("inventory.json"), INVENTORY).unwrap();
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(root.store(),
        profile(), &guards(), None, FileMediatedBootstrap { topology: loaded(1).graph().clone(),
            reserve: RecoveryReserve::terminal(), publication: None }, None, None).unwrap();
    host.observe_time(host.revision(), at).unwrap();
    let source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.0.join("topology.json"), root.0.join("inventory.json"), loaded(1)).unwrap();
    let (port, mut driver) = host.into_supervised_driver(); drop(port);
    driver.require_topology(revision(&driver), FileDriverTopology::new(source)).unwrap();
    refresh(&mut driver);
    driver
}
fn capture(driver: &FileSupervisedDriver) -> TopologyCheckpoint {
    driver.topology_checkpoint(revision(driver), document(1).as_bytes(), INVENTORY).unwrap()
}
fn active() -> (Directory, FileSupervisedDriver, TopologyCheckpoint) {
    let root = Directory::new();
    let driver = seed(&root, ElapsedTick(1));
    let saved = capture(&driver);
    assert!(saved.available()); assert_eq!(saved.work().checks, 1);
    (root, driver, saved)
}
fn ready(root: &Directory, saved: &TopologyCheckpoint) -> FileMediatedRecovery {
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements { guards: guards(),
            effective_policy: profile().delivery.policy, credential_epoch: None,
            minimum: saved.minimum() },
        topology: FileTopologyRequirement { initial: loaded(1).graph().clone(),
            current: saved.expected().graph().clone(), available: saved.available() },
        prediction: None, evaluation: None,
    };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(),
        profile(), &expected, saved.history_anchor()).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
    run
}
fn archive(saved: &TopologyCheckpoint) -> Vec<u8> {
    let mut bytes = Vec::new();
    assert_eq!(saved.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap(), bytes.len());
    bytes
}
fn imported(bytes: &[u8]) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(bytes), bytes.len()).unwrap()
}
fn same_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
    assert_eq!(&imported(&archive(after)), after);
}

#[test]
fn admission_checkpoint_drives_idempotent_drain_and_survives_missing_live_files() {
    let (root, driver, original) = active();
    let identity = fs::canonicalize(root.store()).unwrap(); drop(driver);
    let run = ready(&root, &original);
    let request = run.stop_request(900).unwrap();
    let admitted = run.finish_admission_stop(request).unwrap();
    assert_eq!(admitted.appended_events(), 2);
    let stopped = original.after_admission_stop(&profile(), &identity, &admitted).unwrap();
    same_source(&original, &stopped);
    assert!(!stopped.available());
    assert_eq!(stopped.history_anchor(), admitted.history_anchor());
    assert_eq!(stopped.minimum().journal_revision, admitted.snapshot().revision);
    assert_eq!(stopped.minimum().control_sequence, admitted.snapshot().control.sequence);
    assert_eq!(stopped.minimum().authority_epoch, admitted.snapshot().control.ledger.epoch);
    let drained = ready(&root, &stopped).finish_stopped_drain(request,
        || panic!("no outstanding effects: exact drain retry needs no clock")).unwrap();
    assert_eq!(stopped.after_stopped(&profile(), &identity, &drained).unwrap(), stopped);
    assert_eq!(original.after_stopped(&profile(), &identity, &drained).unwrap(), stopped);
    let repeated = ready(&root, &stopped).finish_admission_stop(request).unwrap();
    assert_eq!(repeated.appended_events(), 0);
    assert_eq!(stopped.after_admission_stop(&profile(), &identity, &repeated).unwrap(), stopped);
    for name in ["topology.json", "inventory.json", "store/delivery.bin"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    assert_eq!(original.after_admission_stop(&profile(), &identity, &admitted).unwrap(), stopped);
    assert_eq!(stopped.after_stopped(&profile(), &identity, &drained).unwrap(), stopped);
}

#[test]
fn combined_checkpoint_preserves_native_floors_and_original_source_spend() {
    let (root, driver, saved) = active();
    let identity = fs::canonicalize(root.store()).unwrap(); drop(driver);
    let run = ready(&root, &saved);
    let request = run.stop_request(901).unwrap();
    let history = run.try_finish_stopped_with_anchor(request, || Ok(ElapsedTick(2))).unwrap();
    assert_eq!(history.history_anchor().revision(), saved.history_anchor().revision() + 3);
    let successor = saved.after_stopped(&profile(), &identity, &history).unwrap();
    same_source(&saved, &successor);
    assert!(!successor.available());
    assert_eq!(successor.history_anchor(), history.history_anchor());
    assert_eq!(successor.minimum().control_sequence, history.stopped().snapshot().control.sequence);
    assert_eq!(successor.minimum().authority_epoch, history.stopped().snapshot().control.ledger.epoch);
    let before = root.bytes();
    let retry = ready(&root, &successor).try_finish_stopped_with_anchor(request,
        || panic!("completed retry cannot consult time")).unwrap();
    assert_eq!(successor.after_stopped(&profile(), &identity, &retry).unwrap(), successor);
    assert_eq!(root.bytes(), before);
}

#[test]
fn stale_source_accounting_and_ordinary_successors_refuse_while_latest_custody_passes() {
    for later_source in [false, true] {
        let (root, mut driver, old) = active();
        if later_source { refresh(&mut driver); }
        else {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
        }
        let latest = capture(&driver);
        let identity = fs::canonicalize(root.store()).unwrap(); drop(driver);
        let run = ready(&root, &latest);
        let request = run.stop_request(902).unwrap();
        let stopped = run.finish_admission_stop(request).unwrap();
        let before = root.bytes();
        assert_eq!(old.after_admission_stop(&profile(), &identity, &stopped),
            Err(JournalError::Contract(Error::Stale)));
        let successor = latest.after_admission_stop(&profile(), &identity, &stopped).unwrap();
        same_source(&latest, &successor);
        assert_eq!(successor.work().checks, if later_source { 2 } else { 1 });
        assert_eq!(root.bytes(), before);
    }
}

#[test]
fn original_profile_identity_and_trusted_archive_requirements_cannot_be_mixed() {
    let (root, driver, saved) = active();
    let (foreign_root, foreign_driver, foreign) = active();
    let identity = fs::canonicalize(root.store()).unwrap(); drop(driver); drop(foreign_driver);
    let run = ready(&root, &saved);
    let request = run.stop_request(903).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    let before = root.bytes();
    let mut wrong_profile = profile(); wrong_profile.delivery.total += 1;
    assert!(saved.after_admission_stop(&wrong_profile, &identity, &stopped).is_err());
    assert!(saved.after_admission_stop(&profile(), &foreign_root.store(), &stopped).is_err());
    assert!(foreign.after_admission_stop(&profile(), &identity, &stopped).is_err());
    // Importing transport-valid bytes authenticates neither custody nor the
    // fields. Native terminal advancement must still reject conflicting history
    // availability, graph and monotonic floors selected by another archive.
    for change in ["availability", "control", "epoch", "graph"] {
        let mut bytes = archive(&saved);
        match change {
            "availability" => bytes[72] = 0,
            "control" => bytes[8..16].copy_from_slice(&u64::MAX.to_le_bytes()),
            "epoch" => bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes()),
            "graph" => {
                let next = document(2);
                assert_eq!(next.len(), saved.topology_bytes().len());
                bytes[73..73 + next.len()].copy_from_slice(next.as_bytes());
            }
            _ => unreachable!(),
        }
        let changed = imported(&bytes);
        assert!(changed.after_admission_stop(&profile(), &identity, &stopped).is_err(), "{change}");
    }
    assert!(saved.after_admission_stop(&profile(), &identity, &stopped).is_ok());
    assert_eq!(root.bytes(), before);
}

#[test]
fn equal_counter_same_identity_fork_cannot_replace_original_retained_history() {
    let (root, driver, original) = active(); drop(driver);
    // Build a second genuine native history at the same canonical identity,
    // retaining the first intact. Only the original time observation differs.
    fs::rename(root.store(), root.0.join("retained-original-store")).unwrap();
    let fork_driver = seed(&root, ElapsedTick(2));
    let fork = capture(&fork_driver);
    assert_eq!(original.minimum(), fork.minimum());
    assert_eq!(original.work(), fork.work());
    assert_ne!(original.history_anchor(), fork.history_anchor());
    let identity = fs::canonicalize(root.store()).unwrap(); drop(fork_driver);
    let run = ready(&root, &fork);
    let request = run.stop_request(904).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    let before = root.bytes();
    assert_eq!(original.after_admission_stop(&profile(), &identity, &stopped),
        Err(JournalError::Contract(Error::Binding)));
    let successor = fork.after_admission_stop(&profile(), &identity, &stopped).unwrap();
    same_source(&fork, &successor);
    assert_eq!(root.bytes(), before);
}

#[test]
fn live_stop_preserves_actual_availability_and_only_recovery_fence_withdraws_it() {
    for completed_live in [false, true] {
        let (root, mut driver, saved) = active();
        let identity = fs::canonicalize(root.store()).unwrap();
        let request = {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let state = host.inspect();
            let request = StopRequest { operation: 905, expected_control_sequence: state.control.sequence,
                expected_authority_epoch: state.control.ledger.epoch };
            let revision = host.revision(); host.request_stop(revision, request).unwrap();
            if completed_live {
                let revision = host.revision();
                assert!(host.progress_stop(revision, ElapsedTick(2)).unwrap().progress.drained());
            }
            assert!(host.mediation_snapshot().unwrap().available);
            request
        };
        drop(driver);
        let admitted = ready(&root, &saved).finish_admission_stop(request).unwrap();
        assert_eq!(admitted.appended_events(), 0);
        assert_eq!(admitted.progress().endpoint_fenced, completed_live);
        assert_eq!(admitted.progress().drained(), completed_live);
        let stopped = saved.after_admission_stop(&profile(), &identity, &admitted).unwrap();
        assert!(stopped.available(), "Stop and StopProgress do not withdraw the graph");
        same_source(&saved, &stopped);
        let clocks = Cell::new(0);
        let drained = ready(&root, &stopped).try_finish_stopped_with_anchor(request,
            || { clocks.set(clocks.get() + 1); Ok(ElapsedTick(3)) }).unwrap();
        assert_eq!(clocks.get(), usize::from(!completed_live));
        assert_eq!(drained.history_anchor().revision(), stopped.history_anchor().revision()
            + if completed_live { 0 } else { 3 });
        assert!(drained.stopped().progress().drained());
        let successor = stopped.after_stopped(&profile(), &identity, &drained).unwrap();
        assert_eq!(successor.available(), completed_live,
            "only the actual recovery Fence withdraws historical availability");
        same_source(&saved, &successor);
    }
}
