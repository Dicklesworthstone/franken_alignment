//! Recover an acknowledged native terminal suffix after both exports are lost.
//! Actual source reads, learned replay and native stopping; synthetic numerical
//! weights and graphs do not qualify a detector or deployed perimeter.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;

use fa_reference::Error;
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::learned::MAX_CHECKED_KV_BYTES, tensor::kv::experiment::KvSide,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{StopRequest,
    persistent::{FileDeliveryProfile, JournalError, JournalLimits, RecoveryReserve,
        observed::{FileOversight, FileOversightProfile,
            consistency::{FileConsistencyConfig, FileConsistencyParameters,
                learned::FileLearnedConsistencyConfig},
            decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus},
            driver::{FileSupervisedDriver, topology::{FileDriverTopology,
                checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES}}},
            guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
                mediated::{FileMediatedRequirements, FileTopologyRequirement,
                    learned::FileMediatedLearnedRecovery,
                    predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements,
                        FilePredictiveMediatedLearnedRecovery},
                    terminal::FileTerminalTopologyRecovery}},
            shutdown::recovery::FileAdmissionStoppedHistory}}};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate}};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::reducer::Caps;
use std::{cell::Cell, collections::BTreeMap, fs, io::Cursor, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

const INVENTORY: &[u8] = include_bytes!("fixtures/authority_topology_inventory.json");
const OPERATION: u64 = 901;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane { Plain, Raw, Owned }
const LANES: [Lane; 3] = [Lane::Plain, Lane::Raw, Lane::Owned];

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-public-terminal-repair-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("store") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
    fn pending(&self) { fs::write(self.store().join("delivery.pending"), b"unacknowledged candidate").unwrap(); }
    fn assert_pending(&self) {
        assert_eq!(fs::read(self.store().join("delivery.pending")).unwrap(), b"unacknowledged candidate");
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("terminal repair cleanup: {error}"); }
    }
}

fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 3,
            tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 100, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
            MemberPolicy { cohort: "one".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"fixture".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn configuration(lane: Lane) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let config = FileLearnedConfig::new_text(model.clone(), numerical::tokenizer(&model),
        numerical::config(&model), LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap();
    if lane == Lane::Plain { return config; }
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let capture = if lane == Lane::Raw { model.residual_contract(1).unwrap().profile() }
        else { model.cache_profile().layers()[&1].values().profile() };
    let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: capture, weights: vec![1.0, 0.0, 0.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"OK".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap();
    match lane {
        Lane::Raw => config.with_required_pre_output_forecast(prediction.with_hosted_residual(1)
            .unwrap().with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Owned => config.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(
            prediction, 1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
            MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
            .with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Plain => unreachable!(),
    }
}
fn guards(config: &FileLearnedConfig) -> FileGuardSet {
    FileGuardSet { stream: config.text_stream_profile(), decoder: None, decoder_stop: None,
        source: config.required_policy_source(), identity: None, campaigns: None, credential: None }
}
fn predictor(config: &FileLearnedConfig) -> FileMediatedPredictor {
    if let Some(raw) = config.required_pre_output_forecast() { FileMediatedPredictor::Raw(raw.clone()) }
    else { FileMediatedPredictor::Owned(config.required_owned_pre_output_forecast().unwrap().clone()) }
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
fn topology() -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(document(1).as_bytes(), INVENTORY).unwrap()
}
fn revision(driver: &FileSupervisedDriver) -> u64 { driver.supervisor().host().unwrap().revision() }
fn refresh(driver: &mut FileSupervisedDriver) {
    let report = driver.refresh_topology(revision(driver)).unwrap();
    assert!(report.refusal.is_none());
    assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
}
fn capture(driver: &FileSupervisedDriver) -> TopologyCheckpoint {
    driver.topology_checkpoint(revision(driver), document(1).as_bytes(), INVENTORY).unwrap()
}
fn seed(root: &Directory, lane: Lane, at: ElapsedTick, reserved: bool) -> FileSupervisedDriver {
    fs::write(root.0.join("topology.json"), document(1)).unwrap();
    fs::write(root.0.join("inventory.json"), INVENTORY).unwrap();
    let config = configuration(lane); let g = guards(&config);
    let graph = topology().graph().clone();
    let (mut host, roles) = match (lane, reserved) {
        (Lane::Plain, true) => FileOversight::create_mediated_guarded_with_learned_generation_reserved(
            root.store(), profile(), &g, None, graph, None, config, RecoveryReserve::terminal()),
        (Lane::Plain, false) => FileOversight::create_mediated_guarded_with_learned_generation(
            root.store(), profile(), &g, None, graph, None, config),
        (_, true) => FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
            root.store(), profile(), &g, None, graph, predictor(&config), None, config, RecoveryReserve::terminal()),
        (_, false) => FileOversight::create_predictive_mediated_guarded_with_learned_generation(
            root.store(), profile(), &g, None, graph, predictor(&config), None, config),
    }.unwrap();
    host.observe_time(host.revision(), at).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.advance_learned_generation(host.revision(), numerical.actor_revision, numerical.position)
        .unwrap().unwrap().sample().is_none());
    let source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.0.join("topology.json"), root.0.join("inventory.json"), topology()).unwrap();
    let (port, mut driver) = host.into_supervised_driver(); drop(port);
    driver.require_topology(revision(&driver), FileDriverTopology::new(source)).unwrap();
    refresh(&mut driver); driver
}
fn expected(lane: Lane, saved: &TopologyCheckpoint) -> FileMediatedRequirements {
    FileMediatedRequirements { oversight: FileRecoveryRequirements { guards: guards(&configuration(lane)),
        effective_policy: profile().delivery.policy, credential_epoch: None, minimum: saved.minimum() },
        topology: FileTopologyRequirement { initial: topology().graph().clone(),
            current: saved.expected().graph().clone(), available: saved.available() },
        prediction: None, evaluation: None }
}
enum Original { Plain(FileMediatedLearnedRecovery), Predictive(FilePredictiveMediatedLearnedRecovery) }
impl Original {
    fn terminal(self, saved: &TopologyCheckpoint, operation: u64)
        -> Result<FileTerminalTopologyRecovery, JournalError>
    {
        match self { Self::Plain(run) => run.require_terminal_checkpoint(saved, operation),
            Self::Predictive(run) => run.require_terminal_checkpoint(saved, operation) }
    }
    fn drive(&mut self) -> Result<(), JournalError> {
        loop {
            let progress = match self { Self::Plain(run) => run.progress(), Self::Predictive(run) => run.progress() };
            if progress.status != FileLearnedRecoveryStatus::Replaying { return Ok(()); }
            match self { Self::Plain(run) => run.advance(progress.replayed_events, 1)?,
                Self::Predictive(run) => run.advance(progress.replayed_events, 1)? };
        }
    }
    fn admission(self) -> Result<FileAdmissionStoppedHistory, JournalError> {
        match self {
            Self::Plain(run) => { let request = run.stop_request(OPERATION)?; run.finish_admission_stop(request) }
            Self::Predictive(run) => { let request = run.stop_request(OPERATION)?; run.finish_admission_stop(request) }
        }
    }
    fn fence(self) {
        match self { Self::Plain(run) => { drop(run.finish().unwrap()); }
            Self::Predictive(run) => { drop(run.finish().unwrap()); } }
    }
}
// Deliberately unanchored constructors here: the new consumer must bind the
// retained checkpoint itself, independently of an optional outer anchor gate.
fn begin(root: &Directory, lane: Lane, expected: &FileMediatedRequirements) -> Result<Original, JournalError> {
    let config = configuration(lane);
    if lane == Lane::Plain {
        return FileOversight::begin_open_mediated_guarded_with_learned_generation(
            root.store(), profile(), expected, &config).map(Original::Plain);
    }
    let expected = FilePredictiveMediatedRequirements { oversight: expected.oversight.clone(),
        topology: expected.topology.clone(), prediction: predictor(&config), evaluation: expected.evaluation.clone() };
    FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).map(Original::Predictive)
}
fn drive(run: &mut FileTerminalTopologyRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress(); let after = run.advance(before.replayed_events, 1)?;
        assert_eq!(after.replayed_events, before.replayed_events + 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready); Ok(())
}
fn ready(root: &Directory, lane: Lane, saved: &TopologyCheckpoint) -> FileTerminalTopologyRecovery {
    let before = root.bytes();
    let mut run = begin(root, lane, &expected(lane, saved)).unwrap().terminal(saved, OPERATION).unwrap();
    drive(&mut run).unwrap(); assert_eq!(root.bytes(), before); run
}
fn archive(saved: &TopologyCheckpoint) -> Vec<u8> {
    let mut bytes = Vec::new();
    saved.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap(); bytes
}
fn imported(bytes: &[u8]) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(bytes), bytes.len()).unwrap()
}
fn same_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.expected(), before.expected()); assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
    assert_eq!(&imported(&archive(after)), after);
}

#[test]
fn terminal_checkpoint_repair_recovers_lost_admission_exports_in_every_learned_mode() {
    for lane in LANES {
        let root = Directory::new(); let driver = seed(&root, lane, ElapsedTick(1), true);
        let saved = imported(&archive(&capture(&driver))); drop(driver);
        let identity = fs::canonicalize(root.store()).unwrap();
        let mut original = begin(&root, lane, &expected(lane, &saved)).unwrap();
        original.drive().unwrap();
        let acknowledged = original.admission().unwrap();
        assert_eq!(acknowledged.appended_events(), 2);
        // Drop the transient result and retain no successor archive: only the
        // original independently saved checkpoint survives this simulated crash.
        drop(acknowledged);
        fs::remove_file(root.0.join("topology.json")).unwrap();
        fs::remove_file(root.0.join("inventory.json")).unwrap();
        root.pending(); let before = root.bytes();
        let mut unchanged = begin(&root, lane, &expected(lane, &saved)).unwrap();
        assert!(unchanged.drive().is_err(), "the ordinary cursor retains exact availability");
        drop(unchanged); assert_eq!(root.bytes(), before); root.assert_pending();
        let repeated = ready(&root, lane, &saved).finish_admission_stop().unwrap();
        assert_eq!(repeated.appended_events(), 0);
        assert_eq!(repeated.snapshot().revision, saved.history_anchor().revision() + 2);
        assert_eq!(root.bytes(), before);
        let retained = saved.after_admission_stop(&profile(), &identity, &repeated).unwrap();
        same_source(&saved, &retained); assert!(!retained.available());
        let drained = ready(&root, lane, &saved).finish_stopped_drain(
            || panic!("no liabilities: exact drain retry cannot consult a clock")).unwrap();
        assert_eq!(saved.after_stopped(&profile(), &identity, &drained).unwrap(), retained);
        let again = ready(&root, lane, &retained).try_finish_stopped_with_anchor(
            || panic!("completed checkpoint retry cannot consult time")).unwrap();
        assert_eq!(again.history_anchor(), retained.history_anchor());
        assert_eq!(root.bytes(), before);
    }
}

#[test]
fn terminal_checkpoint_repair_keeps_combined_and_live_stop_availability_distinct() {
    for lane in LANES {
        for live_stop in [false, true] {
            let root = Directory::new(); let mut driver = seed(&root, lane, ElapsedTick(1), true);
            let saved = capture(&driver); let identity = fs::canonicalize(root.store()).unwrap();
            if live_stop {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                let state = host.inspect();
                let request = StopRequest { operation: OPERATION, expected_control_sequence: state.control.sequence,
                    expected_authority_epoch: state.control.ledger.epoch };
                let revision = host.revision(); host.request_stop(revision, request).unwrap();
                let revision = host.revision(); host.progress_stop(revision, ElapsedTick(2)).unwrap();
                assert!(host.mediation_snapshot().unwrap().available);
            }
            drop(driver);
            let clocks = Cell::new(0);
            let history = ready(&root, lane, &saved).try_finish_stopped_with_anchor(|| {
                clocks.set(clocks.get() + 1); Ok(ElapsedTick(3))
            }).unwrap();
            assert_eq!(clocks.get(), usize::from(!live_stop));
            let retained = saved.after_stopped(&profile(), &identity, &history).unwrap();
            same_source(&saved, &retained);
            assert_eq!(retained.available(), live_stop, "only native Fence withdraws topology");
            let before = root.bytes();
            let retry = ready(&root, lane, &saved).try_finish_stopped_with_anchor(
                || panic!("lost export retry must not reacquire time")).unwrap();
            assert_eq!(retry.history_anchor(), history.history_anchor()); assert_eq!(root.bytes(), before);
            assert!(begin(&root, lane, &expected(lane, &retained)).unwrap()
                .terminal(&retained, OPERATION + 1).is_err(), "exact stopped cut binds operation too");
        }
    }
}

#[test]
fn terminal_checkpoint_repair_rejects_unretained_work_and_ordinary_fences() {
    for lane in LANES {
        for successor in ["time", "source", "ordinary-fence", "stopped-time"] {
            let root = Directory::new(); let mut driver = seed(&root, lane, ElapsedTick(1), true);
            let old = capture(&driver);
            if successor == "source" { refresh(&mut driver); }
            if successor == "time" || successor == "stopped-time" {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                if successor == "stopped-time" {
                    let state = host.inspect();
                    let request = StopRequest { operation: OPERATION, expected_control_sequence: state.control.sequence,
                        expected_authority_epoch: state.control.ledger.epoch };
                    let revision = host.revision(); host.request_stop(revision, request).unwrap();
                }
                let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
            }
            let latest = capture(&driver); drop(driver);
            let mut original = begin(&root, lane, &expected(lane, &latest)).unwrap();
            original.drive().unwrap();
            if successor == "ordinary-fence" { original.fence(); }
            else {
                let admitted = original.admission().unwrap();
                assert_eq!(admitted.appended_events(), if successor == "stopped-time" { 0 } else { 2 });
            }
            root.pending(); let before = root.bytes();
            let result = begin(&root, lane, &expected(lane, &old)).unwrap().terminal(&old, OPERATION);
            assert!(matches!(result, Err(JournalError::Contract(Error::Stale))), "{lane:?}: {successor}");
            assert_eq!(root.bytes(), before); root.assert_pending();
            if successor != "ordinary-fence" {
                let retained = ready(&root, lane, &latest).finish_admission_stop().unwrap();
                assert_eq!(retained.appended_events(), 0); assert_eq!(root.bytes(), before);
            }
        }
    }
}

#[test]
fn terminal_checkpoint_repair_preserves_independent_floors_and_original_custody_requirements() {
    for lane in LANES {
        let root = Directory::new(); let driver = seed(&root, lane, ElapsedTick(1), true);
        let saved = capture(&driver); drop(driver); root.pending(); let before = root.bytes();
        let mut wrong = expected(lane, &saved); wrong.topology.available = !saved.available();
        assert!(begin(&root, lane, &wrong).unwrap().terminal(&saved, OPERATION).is_err());
        assert!(begin(&root, lane, &expected(lane, &saved)).unwrap().terminal(&saved, 0).is_err());
        for change in ["available", "control", "epoch", "external-floor", "policy"] {
            let mut bytes = archive(&saved);
            match change {
                "available" => bytes[72] = u8::from(!saved.available()),
                "control" => bytes[8..16].copy_from_slice(&u64::MAX.to_le_bytes()),
                "epoch" => bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes()),
                _ => {}
            }
            let changed = imported(&bytes); let mut independent = expected(lane, &changed);
            // Saved floors must be strengthened by the native adapter itself.
            independent.oversight.minimum = FileRecoveryFloor {
                journal_revision: 0, control_sequence: 0, authority_epoch: 0 };
            if change == "external-floor" { independent.oversight.minimum.journal_revision = u64::MAX; }
            if change == "policy" {
                independent.oversight.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap();
            }
            let result = begin(&root, lane, &independent).unwrap().terminal(&changed, OPERATION);
            if change == "available" { assert!(result.is_err()); }
            else { let mut run = result.unwrap(); assert!(drive(&mut run).is_err(), "{change}"); }
            assert_eq!(root.bytes(), before); root.assert_pending();
        }
        let original = begin(&root, lane, &expected(lane, &saved)).unwrap();
        let late = match original {
            Original::Plain(mut run) => { run.advance(0, 1).unwrap(); run.require_terminal_checkpoint(&saved, OPERATION) }
            Original::Predictive(mut run) => { run.advance(0, 1).unwrap(); run.require_terminal_checkpoint(&saved, OPERATION) }
        };
        assert!(matches!(late, Err(JournalError::Contract(Error::WrongState))));
        assert_eq!(root.bytes(), before); root.assert_pending();
        assert_eq!(ready(&root, lane, &saved).finish_admission_stop().unwrap().appended_events(), 2);

        let unreserved = Directory::new(); let driver = seed(&unreserved, lane, ElapsedTick(1), false);
        let saved = capture(&driver); drop(driver); unreserved.pending(); let before = unreserved.bytes();
        assert!(begin(&unreserved, lane, &expected(lane, &saved)).unwrap().terminal(&saved, OPERATION).is_err());
        assert_eq!(unreserved.bytes(), before); unreserved.assert_pending();
    }
}

#[test]
fn terminal_checkpoint_repair_rejects_equal_counter_forks_and_post_clock_replacement() {
    for lane in LANES {
        let root = Directory::new(); let driver = seed(&root, lane, ElapsedTick(1), true);
        let original = capture(&driver); drop(driver);
        fs::rename(root.store(), root.0.join("original-store")).unwrap();
        let driver = seed(&root, lane, ElapsedTick(2), true); let fork = capture(&driver); drop(driver);
        assert_eq!(original.minimum(), fork.minimum()); assert_eq!(original.work(), fork.work());
        assert_ne!(original.history_anchor(), fork.history_anchor());
        root.pending(); let before = root.bytes();
        assert!(matches!(begin(&root, lane, &expected(lane, &original)).unwrap().terminal(&original, OPERATION),
            Err(JournalError::Contract(Error::Binding))));
        assert_eq!(root.bytes(), before); root.assert_pending();
        let run = ready(&root, lane, &fork); let clocks = Cell::new(0);
        let result = run.try_finish_stopped_with_anchor(|| {
            clocks.set(clocks.get() + 1);
            fs::write(root.store().join("delivery.bin"), b"conflicting canonical replacement").unwrap();
            Ok(ElapsedTick(3))
        });
        assert!(result.is_err()); assert_eq!(clocks.get(), 1);
        assert_eq!(root.bytes(), b"conflicting canonical replacement"); root.assert_pending();
        fs::write(root.store().join("delivery.bin"), &before).unwrap();
        let completed = ready(&root, lane, &fork).try_finish_stopped_with_anchor(|| Ok(ElapsedTick(3))).unwrap();
        assert_eq!(completed.history_anchor().revision(), fork.history_anchor().revision() + 3);
        let before = root.bytes(); root.pending();
        assert!(begin(&root, lane, &expected(lane, &fork)).unwrap().terminal(&fork, OPERATION + 1).is_err());
        assert_eq!(root.bytes(), before); root.assert_pending();
    }
}
