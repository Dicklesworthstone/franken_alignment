//! Recover exact credential maintenance after both independent exports are lost.
//! Actual source reads, learned replay and native credential writes; synthetic numerical
//! weights and graphs do not qualify a detector or deployed perimeter.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;

use fa_reference::Error;
use fa_reference::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::experiment::KvSide,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{
    StopRequest,
    credential_broker::{
        BrokerRouteBinding, CredentialRevocationRequest, CredentialRotationRequest,
    },
    persistent::{
        FileDeliveryProfile, JournalError, JournalLimits, RecoveryReserve,
        observed::{
            FileOversight, FileOversightProfile,
            consistency::{
                FileConsistencyConfig, FileConsistencyParameters,
                learned::FileLearnedConsistencyConfig,
            },
            credential::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPolicy},
            decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus},
            driver::{
                FileSupervisedDriver,
                topology::{
                    FileDriverTopology,
                    checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
                },
            },
            guarded::{
                FileCredentialEpoch, FileCredentialRegistration, FileGuardSet, FileRecoveryFloor,
                FileRecoveryRequirements,
                mediated::{
                    FileMediatedRequirements, FileTopologyRequirement,
                    credential::{
                        FileCredentialMaintenanceRequest, FileCredentialTopologyRecovery,
                    },
                    learned::FileMediatedLearnedRecovery,
                    predictive::{
                        FileMediatedPredictor, FilePredictiveMediatedLearnedRecovery,
                        FilePredictiveMediatedRequirements,
                    },
                },
            },
        },
    },
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate},
};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, HelperContract, decoder_monitoring::LearnedDecoderBindingLimits,
    human::HumanReviewPolicy,
};
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::perimeter_inventory::{
    LoadedPerimeterInventory, topology::LoadedAuthorityTopology,
};
use fa_reference::reducer::Caps;
use std::{
    collections::BTreeMap,
    fs,
    io::Cursor,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn inventory() -> String {
    String::from_utf8(include_bytes!("fixtures/authority_topology_inventory.json").to_vec())
        .unwrap()
        .replace("reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE)
}
fn binding() -> BrokerRouteBinding {
    BrokerRouteBinding {
        family: "file-publication".into(),
        route: "publication".into(),
    }
}
const OPERATION: u64 = 901;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane {
    Plain,
    Raw,
    Owned,
}
const LANES: [Lane; 3] = [Lane::Plain, Lane::Raw, Lane::Owned];

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fa-public-credential-repair-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf {
        self.0.join("store")
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(self.store().join("delivery.bin")).unwrap()
    }
    fn pending(&self) {
        fs::write(
            self.store().join("delivery.pending"),
            b"unacknowledged candidate",
        )
        .unwrap();
    }
    fn assert_pending(&self) {
        assert_eq!(
            fs::read(self.store().join("delivery.pending")).unwrap(),
            b"unacknowledged candidate"
        );
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("credential repair cleanup: {error}");
        }
    }
}

fn profile() -> FileOversightProfile {
    let target = ResolvedTarget {
        adapter: 10,
        object: 11,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope {
                tenant: 1,
                principal: 2,
                run: 3,
                branch: 4,
                authority: 5,
                purpose: Purpose::Effect,
            },
            total: 100,
            max_attempts: 8,
            actor: ActorState::new(
                RestartProfile {
                    id: 1,
                    generation: 1,
                    host_generation: 1,
                    model_generation: 3,
                    tokenizer_generation: 4,
                    state_schema_generation: 1,
                    grade: RestartGrade::FunctionalRestart,
                },
                Vec::new(),
                vec![0],
                vec![0],
                0,
            )
            .unwrap(),
            suspend_at_incident: 100,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy {
                generation: 1,
                members: BTreeMap::from([(
                    "reviewer".into(),
                    MemberPolicy {
                        cohort: "one".into(),
                        weight: 1,
                    },
                )]),
                caps: Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                continue_minimum: 1,
                continue_hold_maximum: 0,
                narrow_at: 2,
                suspend_at: 3,
                minimum_members: 1,
                minimum_cohorts: 1,
            },
            narrowed_targets: vec![target],
            target,
            initial_payload: b"initial".to_vec(),
            retention_ticks: 1000,
            max_deliveries: 8,
            clock_domain: 99,
            limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([(
            "reviewer".into(),
            HelperContract::new(
                InputProfileBinding {
                    profile_id: 1,
                    profile_bytes: b"fixture".to_vec(),
                    tokenizer_epoch: 4,
                    policy_epoch: 0,
                    model_epoch: 3,
                },
                7,
                b"approve?".to_vec(),
            )
            .unwrap(),
        )]))
        .unwrap(),
        human: HumanReviewPolicy {
            reviewer_id: 77,
            max_validity_ticks: 50,
            max_requests: 8,
        },
    }
}
fn configuration(lane: Lane) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let config = FileLearnedConfig::new_text(
        model.clone(),
        numerical::tokenizer(&model),
        numerical::config(&model),
        LearnedDecoderBindingLimits::default(),
    )
    .unwrap()
    .with_required_sidecar()
    .unwrap();
    if lane == Lane::Plain {
        return config;
    }
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let capture = if lane == Lane::Raw {
        model.residual_contract(1).unwrap().profile()
    } else {
        model.cache_profile().layers()[&1].values().profile()
    };
    let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91,
        probe_generation: 1,
        profile: capture,
        weights: vec![1.0, 0.0, 0.0, 0.0],
        bias: 0.0,
        threshold: 0.0,
        forecast: ForecastRegistration {
            domain: 71,
            generation: 1,
            policy_generation: 1,
            event_prefix: b"OK".to_vec(),
            negative: neutral,
            at_threshold: neutral,
            positive: neutral,
        },
        alpha: ErrorBudget::new(1, 2).unwrap(),
        stream: 21,
        max_predictions: 8,
        max_prediction_age_ticks: 50,
    })
    .unwrap();
    match lane {
        Lane::Raw => config
            .with_required_pre_output_forecast(
                prediction
                    .with_hosted_residual(1)
                    .unwrap()
                    .with_pre_output_forecast()
                    .unwrap(),
            )
            .unwrap(),
        Lane::Owned => config
            .with_required_owned_pre_output_forecast(
                FileLearnedConsistencyConfig::new(
                    prediction,
                    1,
                    KvSide::Value,
                    LearnedMonitorBudget::default(),
                    LearnedMonitorBudget::default(),
                    MAX_CHECKED_KV_BYTES,
                )
                .unwrap()
                .with_owned_generation()
                .unwrap()
                .with_pre_output_forecast()
                .unwrap(),
            )
            .unwrap(),
        Lane::Plain => unreachable!(),
    }
}
fn guards(config: &FileLearnedConfig) -> FileGuardSet {
    FileGuardSet {
        stream: config.text_stream_profile(),
        decoder: None,
        decoder_stop: None,
        source: config.required_policy_source(),
        identity: None,
        campaigns: None,
        credential: Some(
            FileCredentialPolicy::from_inventory(
                &LoadedPerimeterInventory::from_json_bytes(inventory().as_bytes()).unwrap(),
                &binding(),
                profile().delivery.scope,
                profile().delivery.target,
            )
            .unwrap(),
        ),
    }
}
fn predictor(config: &FileLearnedConfig) -> FileMediatedPredictor {
    if let Some(raw) = config.required_pre_output_forecast() {
        FileMediatedPredictor::Raw(raw.clone())
    } else {
        FileMediatedPredictor::Owned(config.required_owned_pre_output_forecast().unwrap().clone())
    }
}
fn document(generation: u64) -> String {
    format!(
        r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
      "complete_inventory_generation":{generation},
      "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
      "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
      "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
      "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
      "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
      "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
      "gates":[2],"check_edge_budget":65536,"source_limits":{{"checks":32,"read_bytes":67108928}}}}"#
    )
}
fn topology() -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(document(1).as_bytes(), inventory().as_bytes())
        .unwrap()
}
fn revision(driver: &FileSupervisedDriver) -> u64 {
    driver.supervisor().host().unwrap().revision()
}
fn refresh(driver: &mut FileSupervisedDriver) {
    let report = driver.refresh_topology(revision(driver)).unwrap();
    assert!(report.refusal.is_none());
    assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
}
fn capture(driver: &FileSupervisedDriver) -> TopologyCheckpoint {
    driver
        .topology_checkpoint(
            revision(driver),
            document(1).as_bytes(),
            inventory().as_bytes(),
        )
        .unwrap()
}
fn seed(root: &Directory, lane: Lane, at: ElapsedTick, reserved: bool) -> FileSupervisedDriver {
    fs::write(root.0.join("topology.json"), document(1)).unwrap();
    fs::write(root.0.join("inventory.json"), inventory().as_bytes()).unwrap();
    let config = configuration(lane);
    let g = guards(&config);
    let graph = topology().graph().clone();
    let inventory = LoadedPerimeterInventory::from_json_bytes(inventory().as_bytes()).unwrap();
    let binding = binding();
    let (mut host, roles) = match (lane, reserved) {
        (Lane::Plain, true) => {
            FileOversight::create_mediated_guarded_with_learned_generation_reserved(
                root.store(),
                profile(),
                &g,
                Some(FileCredentialRegistration {
                    inventory: &inventory,
                    binding: &binding,
                }),
                graph,
                None,
                config,
                RecoveryReserve::terminal(),
            )
        }
        (Lane::Plain, false) => FileOversight::create_mediated_guarded_with_learned_generation(
            root.store(),
            profile(),
            &g,
            Some(FileCredentialRegistration {
                inventory: &inventory,
                binding: &binding,
            }),
            graph,
            None,
            config,
        ),
        (_, true) => {
            FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
                root.store(),
                profile(),
                &g,
                Some(FileCredentialRegistration {
                    inventory: &inventory,
                    binding: &binding,
                }),
                graph,
                predictor(&config),
                None,
                config,
                RecoveryReserve::terminal(),
            )
        }
        (_, false) => FileOversight::create_predictive_mediated_guarded_with_learned_generation(
            root.store(),
            profile(),
            &g,
            Some(FileCredentialRegistration {
                inventory: &inventory,
                binding: &binding,
            }),
            graph,
            predictor(&config),
            None,
            config,
        ),
    }
    .unwrap();
    host.observe_time(host.revision(), at).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert!(
        host.advance_learned_generation(
            host.revision(),
            numerical.actor_revision,
            numerical.position
        )
        .unwrap()
        .unwrap()
        .sample()
        .is_none()
    );
    let source = roles
        .topology_observer
        .bind_topology_source(
            &host,
            host.revision(),
            root.0.join("topology.json"),
            root.0.join("inventory.json"),
            topology(),
        )
        .unwrap();
    let (port, mut driver) = host.into_supervised_driver();
    drop(port);
    driver
        .require_topology(revision(&driver), FileDriverTopology::new(source))
        .unwrap();
    refresh(&mut driver);
    driver
}
fn expected(
    lane: Lane,
    saved: &TopologyCheckpoint,
    generation: u64,
    revoked: bool,
) -> FileMediatedRequirements {
    FileMediatedRequirements {
        oversight: FileRecoveryRequirements {
            guards: guards(&configuration(lane)),
            effective_policy: profile().delivery.policy,
            credential_epoch: Some(FileCredentialEpoch {
                generation,
                revoked,
            }),
            minimum: saved.minimum(),
        },
        topology: FileTopologyRequirement {
            initial: topology().graph().clone(),
            current: saved.expected().graph().clone(),
            available: saved.available(),
        },
        prediction: None,
        evaluation: None,
    }
}
enum Original {
    Plain(FileMediatedLearnedRecovery),
    Predictive(FilePredictiveMediatedLearnedRecovery),
}
impl Original {
    fn maintenance(
        self,
        saved: &TopologyCheckpoint,
        request: FileCredentialMaintenanceRequest,
    ) -> Result<FileCredentialTopologyRecovery, JournalError> {
        match self {
            Self::Plain(run) => run.require_credential_checkpoint(saved, request),
            Self::Predictive(run) => run.require_credential_checkpoint(saved, request),
        }
    }
    fn advance_one(&mut self) {
        match self {
            Self::Plain(run) => {
                run.advance(0, 1).unwrap();
            }
            Self::Predictive(run) => {
                run.advance(0, 1).unwrap();
            }
        }
    }
}
// Deliberately unanchored constructors here: the new consumer must bind the
// retained checkpoint itself, independently of an optional outer anchor gate.
fn begin(
    root: &Directory,
    lane: Lane,
    expected: &FileMediatedRequirements,
) -> Result<Original, JournalError> {
    let config = configuration(lane);
    if lane == Lane::Plain {
        return FileOversight::begin_open_mediated_guarded_with_learned_generation(
            root.store(),
            profile(),
            expected,
            &config,
        )
        .map(Original::Plain);
    }
    let expected = FilePredictiveMediatedRequirements {
        oversight: expected.oversight.clone(),
        topology: expected.topology.clone(),
        prediction: predictor(&config),
        evaluation: expected.evaluation.clone(),
    };
    FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
        root.store(),
        profile(),
        &expected,
        &config,
    )
    .map(Original::Predictive)
}
fn drive(run: &mut FileCredentialTopologyRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1)?;
        assert_eq!(after.replayed_events, before.replayed_events + 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    Ok(())
}
fn ready(
    root: &Directory,
    lane: Lane,
    saved: &TopologyCheckpoint,
    generation: u64,
    revoked: bool,
    request: FileCredentialMaintenanceRequest,
) -> FileCredentialTopologyRecovery {
    let before = root.bytes();
    let mut run = begin(root, lane, &expected(lane, saved, generation, revoked))
        .unwrap()
        .maintenance(saved, request)
        .unwrap();
    drive(&mut run).unwrap();
    assert_eq!(root.bytes(), before);
    run
}
fn rotation(operation: u64, generation: u64) -> FileCredentialMaintenanceRequest {
    FileCredentialMaintenanceRequest::Rotate(CredentialRotationRequest {
        operation,
        expected_generation: generation,
        next_generation: generation + 1,
    })
}
fn revocation(operation: u64, generation: u64) -> FileCredentialMaintenanceRequest {
    FileCredentialMaintenanceRequest::Revoke(CredentialRevocationRequest {
        operation,
        expected_generation: generation,
    })
}
fn archive(saved: &TopologyCheckpoint) -> Vec<u8> {
    let mut bytes = Vec::new();
    saved
        .write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES)
        .unwrap();
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
fn credential_checkpoint_repair_restores_lost_rotation_and_revocation_custody_in_all_modes() {
    for lane in LANES {
        for revoke in [false, true] {
            let root = Directory::new();
            let driver = seed(&root, lane, ElapsedTick(1), true);
            let saved = imported(&archive(&capture(&driver)));
            drop(driver);
            let request = if revoke {
                revocation(OPERATION, 1)
            } else {
                rotation(OPERATION, 1)
            };
            let run = ready(&root, lane, &saved, 1, false, request);
            assert!(run.check_request().unwrap().is_none());
            let mut owner = run.finish_maintenance().unwrap();
            let fence = owner.topology_checkpoint().unwrap();
            same_source(&saved, &fence);
            assert!(!fence.available());
            assert_eq!(
                fence.history_anchor().revision(),
                saved.history_anchor().revision() + 1
            );
            let receipt = owner.apply().unwrap();
            assert_eq!(owner.revision(), saved.history_anchor().revision() + 2);
            // Simulated crash after acknowledgment: no successor archive survives.
            drop(owner);
            fs::remove_file(root.0.join("topology.json")).unwrap();
            fs::remove_file(root.0.join("inventory.json")).unwrap();
            root.pending();
            let before = root.bytes();
            let generation = if revoke { 1 } else { 2 };
            let run = ready(&root, lane, &saved, generation, revoke, request);
            assert_eq!(run.check_request().unwrap(), Some(receipt));
            let status = run.credential_status().unwrap().unwrap();
            assert_eq!(status.generation, generation);
            assert_eq!(status.revoked, revoke);
            assert_eq!(root.bytes(), before);
            root.assert_pending();
            let mut owner = run.finish_maintenance().unwrap();
            assert_eq!(owner.revision(), saved.history_anchor().revision() + 3);
            let fenced = root.bytes();
            assert_eq!(owner.apply().unwrap(), receipt);
            assert_eq!(
                root.bytes(),
                fenced,
                "exact retry must not repeat credential event"
            );
            let retained = owner.topology_checkpoint().unwrap();
            same_source(&saved, &retained);
            assert!(!retained.available());
            assert_eq!(
                owner.history_anchor_after(saved.history_anchor()).unwrap(),
                *retained.history_anchor()
            );
            assert_eq!(
                owner.credential_status().unwrap().unwrap().retained_changes,
                1
            );
            drop(owner);
            let mut again = ready(&root, lane, &retained, generation, revoke, request)
                .finish_maintenance()
                .unwrap();
            assert_eq!(again.apply().unwrap(), receipt);
            same_source(&retained, &again.topology_checkpoint().unwrap());
        }
    }
}

#[test]
fn credential_checkpoint_repair_admits_a_fence_only_interruption_without_refunding_source_work() {
    for lane in LANES {
        for revoke in [false, true] {
            let root = Directory::new();
            let driver = seed(&root, lane, ElapsedTick(1), true);
            let saved = capture(&driver);
            drop(driver);
            let request = if revoke {
                revocation(OPERATION, 1)
            } else {
                rotation(OPERATION, 1)
            };
            let owner = ready(&root, lane, &saved, 1, false, request)
                .finish_maintenance()
                .unwrap();
            assert_eq!(owner.revision(), saved.history_anchor().revision() + 1);
            drop(owner);
            root.pending();
            let run = ready(&root, lane, &saved, 1, false, request);
            assert!(run.check_request().unwrap().is_none());
            root.assert_pending();
            let mut owner = run.finish_maintenance().unwrap();
            let fence = owner.topology_checkpoint().unwrap();
            same_source(&saved, &fence);
            assert_eq!(
                fence.history_anchor().revision(),
                saved.history_anchor().revision() + 2
            );
            let receipt = owner.apply().unwrap();
            assert_eq!(receipt.revoked, revoke);
            assert_eq!(owner.revision(), saved.history_anchor().revision() + 3);
            assert_eq!(
                owner.credential_status().unwrap().unwrap().retained_changes,
                1
            );
            same_source(&saved, &owner.topology_checkpoint().unwrap());
        }
    }
}

#[test]
fn credential_checkpoint_repair_rejects_unrelated_events_and_accepts_their_actual_source_cut() {
    for lane in LANES {
        for successor in [
            "time",
            "source",
            "other-rotation",
            "other-revocation",
            "stop",
        ] {
            let root = Directory::new();
            let mut driver = seed(&root, lane, ElapsedTick(1), true);
            let old = capture(&driver);
            let mut generation = 1;
            let mut revoked = false;
            if successor == "source" {
                refresh(&mut driver);
            } else {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                match successor {
                    "time" => {
                        host.observe_time(revision, ElapsedTick(2)).unwrap();
                    }
                    "other-rotation" => {
                        host.rotate_credential_guard(
                            revision,
                            CredentialRotationRequest {
                                operation: OPERATION + 1,
                                expected_generation: 1,
                                next_generation: 2,
                            },
                        )
                        .unwrap();
                        generation = 2;
                    }
                    "other-revocation" => {
                        host.revoke_credential_guard(
                            revision,
                            CredentialRevocationRequest {
                                operation: OPERATION + 1,
                                expected_generation: 1,
                            },
                        )
                        .unwrap();
                        revoked = true;
                    }
                    "stop" => {
                        let state = host.inspect();
                        host.request_stop(
                            revision,
                            StopRequest {
                                operation: OPERATION + 1,
                                expected_control_sequence: state.control.sequence,
                                expected_authority_epoch: state.control.ledger.epoch,
                            },
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
            }
            let latest = capture(&driver);
            drop(driver);
            root.pending();
            let before = root.bytes();
            let request = if revoked {
                revocation(OPERATION + 1, 1)
            } else {
                rotation(OPERATION, generation)
            };
            // Even another operation's valid credential event cannot borrow an older cut.
            let rejected = if successor == "other-revocation" {
                rotation(OPERATION, 1)
            } else {
                request
            };
            let result = begin(&root, lane, &expected(lane, &old, generation, revoked))
                .unwrap()
                .maintenance(&old, rejected);
            assert!(
                matches!(result, Err(JournalError::Contract(Error::Stale))),
                "{lane:?}: {successor}"
            );
            assert_eq!(root.bytes(), before);
            root.assert_pending();
            if successor != "stop" {
                let run = ready(&root, lane, &latest, generation, revoked, request);
                let mut owner = run.finish_maintenance().unwrap();
                owner.apply().unwrap();
                let retained = owner.topology_checkpoint().unwrap();
                same_source(&latest, &retained);
                assert_eq!(
                    retained.work().checks,
                    if successor == "source" { 2 } else { 1 }
                );
            }
        }
    }
}

#[test]
fn credential_checkpoint_repair_preserves_operation_epoch_floors_guards_and_reserve() {
    for lane in LANES {
        let root = Directory::new();
        let driver = seed(&root, lane, ElapsedTick(1), true);
        let saved = capture(&driver);
        drop(driver);
        let request = rotation(OPERATION, 1);
        let mut owner = ready(&root, lane, &saved, 1, false, request)
            .finish_maintenance()
            .unwrap();
        owner.apply().unwrap();
        drop(owner);
        root.pending();
        let before = root.bytes();
        for changed in [
            rotation(OPERATION + 1, 1),
            rotation(OPERATION, 2),
            revocation(OPERATION, 1),
        ] {
            assert!(
                begin(&root, lane, &expected(lane, &saved, 2, false))
                    .unwrap()
                    .maintenance(&saved, changed)
                    .is_err()
            );
            assert_eq!(root.bytes(), before);
            root.assert_pending();
        }
        for change in [
            "epoch",
            "revoked",
            "policy",
            "external-floor",
            "saved-control",
            "saved-epoch",
        ] {
            let mut bytes = archive(&saved);
            if change == "saved-control" {
                bytes[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
            }
            if change == "saved-epoch" {
                bytes[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
            }
            let changed = imported(&bytes);
            let mut independent = expected(lane, &changed, 2, false);
            independent.oversight.minimum = FileRecoveryFloor {
                journal_revision: 0,
                control_sequence: 0,
                authority_epoch: 0,
            };
            match change {
                "epoch" => {
                    independent
                        .oversight
                        .credential_epoch
                        .as_mut()
                        .unwrap()
                        .generation = 1
                }
                "revoked" => {
                    independent
                        .oversight
                        .credential_epoch
                        .as_mut()
                        .unwrap()
                        .revoked = true
                }
                "policy" => {
                    independent.oversight.effective_policy =
                        Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()
                }
                "external-floor" => independent.oversight.minimum.journal_revision = u64::MAX,
                _ => {}
            }
            let mut run = begin(&root, lane, &independent)
                .unwrap()
                .maintenance(&changed, request)
                .unwrap();
            assert!(drive(&mut run).is_err(), "{lane:?}: {change}");
            drop(run);
            assert_eq!(root.bytes(), before);
            root.assert_pending();
        }
        let mut wrong = expected(lane, &saved, 2, false);
        wrong.topology.available = !saved.available();
        assert!(
            begin(&root, lane, &wrong)
                .unwrap()
                .maintenance(&saved, request)
                .is_err()
        );
        let mut original = begin(&root, lane, &expected(lane, &saved, 2, false)).unwrap();
        original.advance_one();
        assert!(matches!(
            original.maintenance(&saved, request),
            Err(JournalError::Contract(Error::WrongState))
        ));
        assert!(
            begin(&root, lane, &expected(lane, &saved, 2, false))
                .unwrap()
                .maintenance(&saved, rotation(0, 1))
                .is_err()
        );
        assert_eq!(root.bytes(), before);
        root.assert_pending();
        assert!(
            ready(&root, lane, &saved, 2, false, request)
                .check_request()
                .unwrap()
                .is_some()
        );

        let unreserved = Directory::new();
        let driver = seed(&unreserved, lane, ElapsedTick(1), false);
        let saved = capture(&driver);
        drop(driver);
        unreserved.pending();
        let before = unreserved.bytes();
        assert!(
            begin(&unreserved, lane, &expected(lane, &saved, 1, false))
                .unwrap()
                .maintenance(&saved, request)
                .is_err()
        );
        assert_eq!(unreserved.bytes(), before);
        unreserved.assert_pending();
    }
}

#[test]
fn credential_checkpoint_repair_drop_at_each_quantum_and_equal_counter_forks_leave_no_candidate() {
    for lane in LANES {
        let root = Directory::new();
        let driver = seed(&root, lane, ElapsedTick(1), true);
        let original = capture(&driver);
        drop(driver);
        fs::rename(root.store(), root.0.join("original-store")).unwrap();
        let driver = seed(&root, lane, ElapsedTick(2), true);
        let saved = capture(&driver);
        drop(driver);
        assert_eq!(original.minimum(), saved.minimum());
        assert_eq!(original.work(), saved.work());
        assert_ne!(original.history_anchor(), saved.history_anchor());
        root.pending();
        let before = root.bytes();
        let request = rotation(OPERATION, 1);
        assert!(matches!(
            begin(&root, lane, &expected(lane, &original, 1, false))
                .unwrap()
                .maintenance(&original, request),
            Err(JournalError::Contract(Error::Binding))
        ));
        assert_eq!(root.bytes(), before);
        root.assert_pending();
        let mut quantum = 0;
        loop {
            let mut run = begin(&root, lane, &expected(lane, &saved, 1, false))
                .unwrap()
                .maintenance(&saved, request)
                .unwrap();
            for _ in 0..quantum {
                let progress = run.progress();
                run.advance(progress.replayed_events, 1).unwrap();
            }
            let ready = run.progress().status == FileLearnedRecoveryStatus::Ready;
            drop(run);
            assert_eq!(root.bytes(), before);
            root.assert_pending();
            if ready {
                break;
            }
            quantum += 1;
        }
        assert!(quantum > 0);
        let mut owner = ready(&root, lane, &saved, 1, false, request)
            .finish_maintenance()
            .unwrap();
        owner.apply().unwrap();
        same_source(&saved, &owner.topology_checkpoint().unwrap());
    }
}
