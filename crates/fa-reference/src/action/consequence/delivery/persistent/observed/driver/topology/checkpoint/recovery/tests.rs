//! Real native source checks and guarded recovery. Transport is not authentication.
use super::*;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::delivery::persistent::observed::credibility::andon::tests as accounting;
use crate::action::consequence::delivery::persistent::observed::driver::topology::{
    FileDriverTopology, checkpoint::MAX_TOPOLOGY_CHECKPOINT_BYTES,
};
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRoles;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements,
    mediated::{FileMediatedBootstrap, FileMediatedRequirements, FileTopologyRequirement},
    recovery::FileGuardedRecoveryStatus,
};
use crate::action::consequence::delivery::persistent::observed::mediation::FileMediationUpdate;
use crate::action::consequence::delivery::persistent::observed::mediation::source::{
    TOPOLOGY_REFRESH_READ_BYTES, TopologySourceWork,
};
use crate::action::consequence::delivery::persistent::{
    FileDeliveryProfile, JournalIo, JournalLimits, RecoveryReserve,
};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::mediation::CutCheck;
use crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;
use crate::action::consequence::oversight::credibility::GroundTruth;
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::Verdict;
use std::{
    collections::BTreeMap,
    fs,
    io::Cursor,
    sync::atomic::{AtomicU64, Ordering},
};

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
fn recover(
    root: &Directory,
    record: &TopologyCheckpoint,
    generation: u64,
) -> (FileSupervisedDriver, FileMediationObserver) {
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
            credential_epoch: None, minimum: record.minimum() },
        topology: FileTopologyRequirement { initial: loaded(1).graph().clone(), current: loaded(generation).graph().clone(), available: record.available() },
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

fn seed_evaluated() -> (FileSupervisedDriver, Directory) {
    let root = Directory::new();
    fs::write(root.topology(), text(1)).unwrap();
    fs::write(root.inventory(), INVENTORY).unwrap();
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(
        root.store(),
        accounting::profile(),
        &guards(),
        None,
        FileMediatedBootstrap {
            topology: loaded(1).graph().clone(),
            reserve: RecoveryReserve::terminal(),
            publication: None,
        },
        None,
        Some(accounting::protocol()),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let mut source = roles
        .topology_observer
        .bind_topology_source(
            &host,
            host.revision(),
            root.topology(),
            root.inventory(),
            loaded(1),
        )
        .unwrap();
    let rev = host.revision();
    certified(source.refresh(&mut host, rev, 1).unwrap());
    // A real original full-input Hold remains pending independent evaluation.
    // No saved transcript or fabricated assessment is imported on recovery.
    accounting::case(&mut host, 1, Verdict::Hold);
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
    let (port, mut driver) = host.into_supervised_driver();
    drop(port);
    driver
        .require_topology(revision(&driver), FileDriverTopology::new(source))
        .unwrap();
    (driver, root)
}

fn recover_evaluated(
    root: &Directory,
    record: &TopologyCheckpoint,
) -> (FileOversight, FileMediatedRoles) {
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements {
            guards: guards(),
            effective_policy: accounting::profile().delivery.policy,
            credential_epoch: None,
            minimum: record.minimum(),
        },
        topology: FileTopologyRequirement {
            initial: loaded(1).graph().clone(),
            current: record.expected().graph().clone(),
            available: record.available(),
        },
        prediction: None,
        evaluation: Some(accounting::protocol()),
    };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(
        root.store(),
        accounting::profile(),
        &expected,
        record.history_anchor(),
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    run.finish().unwrap()
}

fn same_source_work(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
    assert!(!after.available());
}

#[test]
fn evaluated_checkpoint_retains_late_labels_and_resumes_original_source_work() {
    let (driver, root) = seed_evaluated();
    let original = capture(&driver);
    assert_eq!(original.work().checks, 1);
    drop(driver);
    // Historical evaluation and checkpoint export must not read next sources.
    fs::remove_file(root.topology()).unwrap();
    fs::remove_file(root.inventory()).unwrap();
    let (mut host, roles) = recover_evaluated(&root, &original);
    let observer = roles.topology_observer;
    let evaluator = roles.evaluator.unwrap();
    let fenced = observer
        .evaluated_topology_checkpoint(&host, &original)
        .unwrap();
    same_source_work(&original, &fenced);
    assert_eq!(
        fenced.history_anchor().revision(),
        original.history_anchor().revision() + 1
    );
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let control = host.inspect().control;
    assert!(accounting::assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap());
    assert_eq!(host.inspect().control, control);
    assert_eq!(host.evaluation_history(101).unwrap().len(), 1);
    assert_eq!(host.credibility_report().unwrap().pending_cases, 0);
    let before = root.bytes();
    let saved = observer
        .evaluated_topology_checkpoint(&host, &original)
        .unwrap();
    same_source_work(&original, &saved);
    assert_eq!(saved.history_anchor(), &host.history_anchor().unwrap());
    assert_eq!(decoded(&encoded(&saved)), saved);
    assert_eq!(root.bytes(), before);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(host.inspect().executions, 0);
    drop(host);

    // The exported withdrawn pair remains usable for a later real source
    // continuation. Neither the historical label nor recovery refills work.
    let (mut host, roles) = recover_evaluated(&root, &decoded(&encoded(&saved)));
    assert_eq!(host.evaluation_history(101).unwrap().len(), 1);
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    fs::write(root.topology(), text(2)).unwrap();
    fs::write(root.inventory(), INVENTORY).unwrap();
    let (port, mut driver) = host.into_supervised_driver();
    drop(port);
    let report = driver
        .resume_topology_checkpoint(
            revision(&driver),
            2,
            roles.topology_observer,
            &saved,
            selection(&root, 1),
        )
        .unwrap();
    certified(report);
    let resumed = capture(&driver);
    assert_eq!(resumed.work().checks, original.work().checks + 1);
    assert_eq!(
        resumed.work().reserved_read_bytes,
        2 * TOPOLOGY_REFRESH_READ_BYTES
    );
    assert_eq!(resumed.last_operation(), 3);
    assert_eq!(
        driver
            .supervisor()
            .host()
            .unwrap()
            .evaluation_history(101)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn evaluated_checkpoint_rejects_foreign_custody_and_a_cut_before_later_source_work() {
    let (mut driver, root) = seed();
    let old = capture(&driver);
    certified(driver.refresh_topology(revision(&driver)).unwrap());
    let current = capture(&driver);
    drop(driver);
    let (driver, observer) = recover(&root, &current, 1);
    let (other, foreign_root) = seed();
    let foreign = capture(&other);
    drop(other);
    let (_other, foreign_observer) = recover(&foreign_root, &foreign, 1);
    let host = driver.supervisor().host().unwrap();
    let before = root.bytes();
    assert!(
        foreign_observer
            .evaluated_topology_checkpoint(&host, &current)
            .is_err()
    );
    assert!(
        observer
            .evaluated_topology_checkpoint(&host, &foreign)
            .is_err()
    );
    assert!(observer.evaluated_topology_checkpoint(&host, &old).is_err());
    let saved = observer
        .evaluated_topology_checkpoint(&host, &current)
        .unwrap();
    same_source_work(&current, &saved);
    assert_eq!(saved.work().checks, 2);
    assert_eq!(root.bytes(), before);
}

#[test]
fn evaluated_checkpoint_rejects_ordinary_work_after_its_recovery_fence() {
    for change in [
        "extra-fence",
        "effect-sweep",
        "topology-update",
        "unacknowledged-time",
    ] {
        let (driver, root) = seed();
        let original = capture(&driver);
        drop(driver);
        let (mut driver, observer) = recover(&root, &original, 1);
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        assert!(
            observer
                .evaluated_topology_checkpoint(&host, &original)
                .is_ok()
        );
        let rev = host.revision();
        match change {
            "extra-fence" => host.fence(rev).unwrap(),
            "effect-sweep" => assert!(host.reconcile_pending(rev).unwrap().is_empty()),
            "unacknowledged-time" => {
                host.fail_storage_once(JournalIo::Stage);
                assert!(host.observe_time(rev, ElapsedTick(3)).is_err());
            }
            _ => {
                let update = FileMediationUpdate {
                    operation: 2,
                    expected_generation: 1,
                    expected_authority_epoch: host.inspect().control.ledger.epoch,
                    next: Some(loaded(2).graph().clone()),
                };
                observer.update(&mut host, rev, &update).unwrap();
            }
        }
        let before = root.bytes();
        assert!(
            observer
                .evaluated_topology_checkpoint(&host, &original)
                .is_err(),
            "{change}"
        );
        assert_eq!(root.bytes(), before, "{change}");
    }
}

#[test]
fn evaluated_checkpoint_preserves_original_stop_but_refuses_labels_after_stop() {
    let (driver, root) = seed_evaluated();
    let original = capture(&driver);
    drop(driver);
    let (mut host, roles) = recover_evaluated(&root, &original);
    let observer = roles.topology_observer;
    let evaluator = roles.evaluator.unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let state = host.inspect();
    let request = StopRequest {
        operation: 71,
        expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch,
    };
    host.request_stop(host.revision(), request).unwrap();
    host.fence(host.revision()).unwrap();
    host.progress_stop(host.revision(), ElapsedTick(3)).unwrap();
    assert!(host.stop_progress().unwrap().drained());
    let before = root.bytes();
    let stopped = observer
        .evaluated_topology_checkpoint(&host, &original)
        .unwrap();
    same_source_work(&original, &stopped);
    assert_eq!(stopped.history_anchor(), &host.history_anchor().unwrap());
    assert_eq!(root.bytes(), before);
    // Native history permits delayed labels, but this service's selected
    // assessment-only interval ends at Stop and cannot absorb a later label.
    assert!(accounting::assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap());
    let before = root.bytes();
    assert!(
        observer
            .evaluated_topology_checkpoint(&host, &original)
            .is_err()
    );
    assert_eq!(root.bytes(), before);
}
