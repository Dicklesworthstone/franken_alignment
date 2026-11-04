//! Explicit learned Unknown disposition after original numerical/guarded replay.
//! Real generation, native review and two-key dispatch provide the positive
//! history; synthetic fixture weights do not qualify detector effectiveness.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "mediated_learned_recovery/support.rs"]
mod custody;
#[path = "mediated_predictive_learned_recovery/native_fixture.rs"]
mod native_fixture;
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;
#[path = "mediated_predictive_learned_recovery/support.rs"]
#[allow(dead_code)]
mod support;
use support::*;

use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::{
    FileDriverTopology,
    checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;

fn source_inventory() -> String {
    String::from_utf8(include_bytes!("fixtures/authority_topology_inventory.json").to_vec())
        .unwrap()
        .replace(
            "Declared topology is not authenticated deployment coverage",
            "Declared graph only; no operating-system isolation claim",
        )
}
fn source_document() -> &'static str {
    r#"{"schema":"fa.authority-topology/1","generation":1,"inventory_generation":1,
      "complete_inventory_generation":1,
      "scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"},
      "target":{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1},
      "inventory_scope":{"tenant":1,"principal":2,"purpose":1},"family":"file-publication",
      "nodes":[{"id":1,"kind":"actor"},{"id":2,"kind":"enforcer"},{"id":3,"kind":"sink"}],
      "edges":[{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101},
        {"id":2,"from":2,"to":3,"channel":"dispatch","route":"publication","provenance":102}],
      "enforcers":[{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":90}],
      "gates":[2],"check_edge_budget":65536,"source_limits":{"checks":32,"read_bytes":67108928}}"#
}
use custody::{certify, fresh_identity, graph, protocol, snapshot};

use fa_reference::action::consequence::activation::{
    FrameIdentity,
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::{LinearProbe, learned::MAX_CHECKED_KV_BYTES},
    tensor::kv::{
        decoder::{
            DecoderModel,
            monitoring::restart::KvRestartBudget,
            sampling::{SampledToken, monitored::GenerationEvent},
        },
        experiment::KvSide,
        model::MAX_MODEL_KV_VALUES,
    },
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        FileDeliveryProfile, FileDeliverySnapshot, FilePermit, JournalError, JournalLimits,
        RecoveryReserve,
        abandonment::FileAbandonRequest,
        observed::{
            FileHumanRequest, FileOversight, FileOversightProfile,
            consistency::{
                FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
                learned::FileLearnedConsistencyConfig,
            },
            containment::FileResetRequest,
            decoder::learned::{
                FileLearnedConfig, FileLearnedRecovery, FileLearnedRecoveryProgress,
                FileLearnedRecoveryStatus,
                checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent},
            },
            guarded::{
                FileGuardSet, FileOversightRoles, FileRecoveryFloor, FileRecoveryRequirements,
                anchored::FileHistoryAnchor,
                learned_recovery::FileGuardedLearnedRecovery,
                mediated::{
                    FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement,
                    learned::FileMediatedLearnedRecovery,
                    predictive::{
                        FileMediatedPredictor, FilePredictiveMediatedLearnedRecovery,
                        FilePredictiveMediatedRequirements,
                    },
                },
                predictive::{
                    FilePredictiveRequirements,
                    learned::FilePredictiveLearnedRecovery,
                    owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements},
                },
                recovery::FileAbandonedHistory,
            },
            helpers::learned::native::{
                NativeReviewLimits, NativeReviewMember, NativeReviewStatus,
            },
            mediation::FileMediationObserver,
            stream::FileStreamSnapshot,
        },
        requests::{
            FileRequestDisposition,
            actor::{
                FileLearnedTextStreamActorPort, LearnedTextRelease,
                receipt::FileLearnedReceiptActorPort,
            },
        },
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::{
    ReviewBinding,
    containment::{
        ActorState, RestartGrade, RestartProfile,
        session::policy::{Policy, Predicate},
    },
};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ReviewWindow,
    actor::{ActorError, Knowledge},
    credibility::EvaluationProtocol,
    decoder_monitoring::LearnedDecoderBindingLimits,
    human::HumanReviewPolicy,
    learned_host::sidecar::{
        LearnedSidecarRequest,
        workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS},
    },
    learned_source::{LearnedEvidenceLimits, text::LearnedTextConfig},
    sidecar::{
        SidecarCongressBudget, SidecarIdentity,
        receiver::native::{SidecarDecisionBasis, SidecarProbeQuery},
    },
};
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION,
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::{Error, Snapshot};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane {
    Plain,
    Raw,
    Owned,
}

fn stream_contract() -> StreamProfile {
    StreamProfile::new(1, 1, 4, 64, 128).unwrap()
}

fn abandonment_profile() -> FileOversightProfile {
    let mut p = support::profile();
    p.delivery.initial_payload.clear();
    p.delivery.total = 4096;
    // Keep the real ordinary-capacity boundary small enough for local controls.
    p.delivery.limits.events = 128;
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    p
}

fn configuration(lane: Lane) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let config = FileLearnedConfig::new_text_stream(
        model.clone(),
        numerical::tokenizer(&model),
        source(&model),
        LearnedDecoderBindingLimits::default(),
        stream_contract(),
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
    .unwrap()
    .with_stream_messages(stream_contract())
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
        Lane::Plain => unreachable!("handled before predictor construction"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Core,
    Guarded,
    Raw,
    Owned,
    Mediated,
    MediatedRaw,
    MediatedOwned,
}
const FAMILIES: [Family; 7] = [
    Family::Core,
    Family::Guarded,
    Family::Raw,
    Family::Owned,
    Family::Mediated,
    Family::MediatedRaw,
    Family::MediatedOwned,
];
const MEDIATED: [Family; 3] = [Family::Mediated, Family::MediatedRaw, Family::MediatedOwned];
impl Family {
    fn lane(self) -> Lane {
        match self {
            Self::Raw | Self::MediatedRaw => Lane::Raw,
            Self::Owned | Self::MediatedOwned => Lane::Owned,
            _ => Lane::Plain,
        }
    }
    fn mediated(self) -> bool {
        MEDIATED.contains(&self)
    }
    fn evaluation(self) -> Option<EvaluationProtocol> {
        (self.mediated() || self.lane() != Lane::Plain).then(protocol)
    }
}
struct Roles {
    oversight: FileOversightRoles,
    prediction: Option<FileConsistencyObserver>,
    topology: Option<FileMediationObserver>,
}
fn start(
    family: Family,
    root: &Directory,
    p: FileOversightProfile,
    config: &FileLearnedConfig,
) -> (FileOversight, Roles) {
    use fa_reference::action::consequence::delivery::credential_broker::{
        BrokerRouteBinding, CredentialRotationRequest,
    };
    use fa_reference::action::consequence::delivery::persistent::observed::{
        credential::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPolicy},
        guarded::FileCredentialRegistration,
    };
    use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
    let inventory = LoadedPerimeterInventory::from_json_bytes(
        source_inventory()
            .replace("reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE)
            .as_bytes(),
    )
    .unwrap();
    let binding = BrokerRouteBinding {
        family: "file-publication".into(),
        route: "publication".into(),
    };
    let registration = Some(FileCredentialRegistration {
        inventory: &inventory,
        binding: &binding,
    });
    let mut g = guards(config);
    g.credential = Some(
        FileCredentialPolicy::from_inventory(
            &inventory,
            &binding,
            p.delivery.scope,
            p.delivery.target,
        )
        .unwrap(),
    );
    let (mut host, roles) = match family {
        Family::Core | Family::Guarded => {
            let (host, oversight) = FileOversight::create_guarded_with_learned_generation_reserved(
                root.store(),
                p,
                &g,
                registration,
                config.clone(),
                RecoveryReserve::terminal(),
            )
            .unwrap();
            (
                host,
                Roles {
                    oversight,
                    prediction: None,
                    topology: None,
                },
            )
        }
        Family::Raw => {
            let (host, roles) =
                FileOversight::create_predictive_guarded_with_learned_generation_reserved(
                    root.store(),
                    p,
                    &g,
                    registration,
                    config.required_pre_output_forecast().unwrap().clone(),
                    family.evaluation(),
                    config.clone(),
                    RecoveryReserve::terminal(),
                )
                .unwrap();
            (
                host,
                Roles {
                    oversight: roles.oversight,
                    prediction: Some(roles.consistency_observer),
                    topology: None,
                },
            )
        }
        Family::Owned => {
            let (host, roles) =
                FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(
                    root.store(),
                    p,
                    &g,
                    registration,
                    config.required_owned_pre_output_forecast().unwrap().clone(),
                    family.evaluation(),
                    config.clone(),
                    RecoveryReserve::terminal(),
                )
                .unwrap();
            (
                host,
                Roles {
                    oversight: roles.oversight,
                    prediction: Some(roles.consistency_observer),
                    topology: None,
                },
            )
        }
        Family::Mediated => {
            let (host, roles) =
                FileOversight::create_mediated_guarded_with_learned_generation_reserved(
                    root.store(),
                    p,
                    &g,
                    registration,
                    graph(1, false),
                    family.evaluation(),
                    config.clone(),
                    RecoveryReserve::terminal(),
                )
                .unwrap();
            (
                host,
                Roles {
                    oversight: roles.oversight,
                    prediction: roles.consistency_observer,
                    topology: Some(roles.topology_observer),
                },
            )
        }
        Family::MediatedRaw | Family::MediatedOwned => {
            let (host, roles) =
                FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
                    root.store(),
                    p,
                    &g,
                    registration,
                    graph(1, false),
                    prediction(config),
                    family.evaluation(),
                    config.clone(),
                    RecoveryReserve::terminal(),
                )
                .unwrap();
            (
                host,
                Roles {
                    oversight: roles.oversight,
                    prediction: roles.consistency_observer,
                    topology: Some(roles.topology_observer),
                },
            )
        }
    };
    host.rotate_credential_guard(
        host.revision(),
        CredentialRotationRequest {
            operation: 901,
            expected_generation: 1,
            next_generation: 2,
        },
    )
    .unwrap();
    (host, roles)
}

fn publication_credential(host: &FileOversight)
    -> fa_reference::action::consequence::delivery::persistent::observed::credential::FileCredentialPermit
{
    use fa_reference::action::consequence::delivery::credential_broker::{
        BrokerCredential, BrokerRouteBinding, ProviderCredential,
    };
    use fa_reference::action::consequence::delivery::persistent::observed::credential::FILE_OVERSIGHT_CREDENTIAL_PROFILE;
    use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
    let inventory = LoadedPerimeterInventory::from_json_bytes(
        source_inventory()
            .replace("reference-publication", FILE_OVERSIGHT_CREDENTIAL_PROFILE)
            .as_bytes(),
    )
    .unwrap();
    let binding = BrokerRouteBinding {
        family: "file-publication".into(),
        route: "publication".into(),
    };
    // Public synthetic fixture bytes, freshly bound to this actual generation-2
    // owner. They are never journaled or supplied by a recovery result.
    let fixture = b"learned-abandonment-fixture";
    host.bind_credential_pair(
        host.revision(),
        &inventory,
        &binding,
        BrokerCredential::new(fixture.to_vec()).unwrap(),
        ProviderCredential::new(fixture.to_vec()).unwrap(),
    )
    .unwrap()
}

enum Recovery {
    Core(FileLearnedRecovery),
    Guarded(FileGuardedLearnedRecovery),
    Raw(FilePredictiveLearnedRecovery),
    Owned(FileOwnedPredictiveRecovery),
    Mediated(FileMediatedLearnedRecovery),
    PredictiveMediated(FilePredictiveMediatedLearnedRecovery),
}
macro_rules! each_recovery {
    ($value:expr, $run:ident => $body:expr) => {
        match $value {
            Recovery::Core($run) => $body,
            Recovery::Guarded($run) => $body,
            Recovery::Raw($run) => $body,
            Recovery::Owned($run) => $body,
            Recovery::Mediated($run) => $body,
            Recovery::PredictiveMediated($run) => $body,
        }
    };
}
impl Recovery {
    fn progress(&self) -> FileLearnedRecoveryProgress {
        each_recovery!(self, run => run.progress())
    }
    fn drive(&mut self) -> Result<(), JournalError> {
        while self.progress().status == FileLearnedRecoveryStatus::Replaying {
            let completed = self.progress().replayed_events;
            let next = each_recovery!(&mut *self, run => run.advance(completed, 1))?;
            assert_eq!(next.replayed_events, completed + 1);
        }
        assert_eq!(self.progress().status, FileLearnedRecoveryStatus::Ready);
        Ok(())
    }
    fn abandon(self, request: FileAbandonRequest) -> Result<FileAbandonedHistory, JournalError> {
        each_recovery!(self, run => run.finish_abandoned_with_anchor(request))
    }
    fn abandon_source(
        self,
        request: FileAbandonRequest,
        saved: &TopologyCheckpoint,
    ) -> Result<FileAbandonedHistory, JournalError> {
        each_recovery!(self, run => run.finish_abandoned_with_checkpoint(request, saved))
    }
    fn receipt(self) -> Result<FileLearnedReceiptActorPort, JournalError> {
        each_recovery!(self, run => run.finish_receipt_port(71, None))
    }
    fn live(self) -> FileOversight {
        match self {
            Self::Core(run) => run.finish().unwrap().0,
            Self::Guarded(run) => run.finish().unwrap().0,
            Self::Raw(run) => run.finish().unwrap().0,
            Self::Owned(run) => run.finish().unwrap().0,
            Self::Mediated(run) => run.finish().unwrap().0,
            Self::PredictiveMediated(run) => run.finish().unwrap().0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Pending,
    Sent,
    Unknown,
    PublishedUnknown,
    Confirmed,
}
struct History {
    root: Directory,
    family: Family,
    profile: FileOversightProfile,
    config: FileLearnedConfig,
    expected: FileRecoveryRequirements,
    topology: Option<FileTopologyRequirement>,
    anchor: FileHistoryAnchor,
    source: Option<TopologyCheckpoint>,
    early_source: Option<TopologyCheckpoint>,
    request: FileAbandonRequest,
    original: FileDeliverySnapshot,
    stream: FileStreamSnapshot,
    action: FrozenAction,
}
impl History {
    fn new(family: Family, cut: Cut) -> Self {
        Self::build(family, cut, None, false)
    }
    fn build(family: Family, cut: Cut, remaining: Option<usize>, retain_source: bool) -> Self {
        let root = Directory::new();
        let p = abandonment_profile();
        let config = configuration(family.lane());
        let (mut host, roles) = start(family, &root, p.clone(), &config);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        assert!(step(&mut host).sample().is_none());
        assert!(step(&mut host).sample().is_none());
        let mut source = None;
        if let Some(observer) = roles.topology {
            if retain_source {
                let directory = root.store().parent().unwrap().to_owned();
                let inventory = source_inventory();
                let loaded = LoadedAuthorityTopology::from_json_bytes(
                    source_document().as_bytes(),
                    inventory.as_bytes(),
                )
                .unwrap();
                std::fs::write(directory.join("topology.json"), source_document()).unwrap();
                std::fs::write(directory.join("inventory.json"), &inventory).unwrap();
                let mut bound = observer
                    .bind_topology_source(
                        &host,
                        host.revision(),
                        directory.join("topology.json"),
                        directory.join("inventory.json"),
                        loaded,
                    )
                    .unwrap();
                let revision = host.revision();
                let refreshed = bound.refresh(&mut host, revision, 1).unwrap();
                assert!(refreshed.refusal.is_none());
                assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
                source = Some(FileDriverTopology::new(bound));
            } else {
                assert!(matches!(
                    certify(&mut host, &observer).unwrap(),
                    CutCheck::Verified(_)
                ));
            }
        } else {
            assert!(!retain_source);
        }
        let early_source = source.as_ref().map(|source| {
            source
                .checkpoint(
                    &host,
                    source_document().as_bytes(),
                    source_inventory().as_bytes(),
                )
                .unwrap()
        });
        if let Some(observer) = &roles.prediction {
            let mode = if family.lane() == Lane::Raw {
                Mode::Raw
            } else {
                Mode::Owned
            };
            mode.forecast(&mut host, observer, 71).unwrap().unwrap();
        }
        let (samples, last) = finish_generation(&mut host);
        assert_eq!(samples.len(), 3);
        assert_eq!(
            host.learned_text_message(LearnedEvidenceLimits::default())
                .unwrap()
                .bytes(),
            b"OK"
        );
        fresh_identity(&mut host, &roles.oversight, 1, 1);
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let ticket = actor
            .submit(71, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        let (attempt, action) = {
            let mut host = supervisor.host_mut().unwrap();
            let FileRequestDisposition::Admitted { attempt, .. } =
                host.request_status(71).unwrap().disposition
            else {
                panic!("original learned request must be admitted");
            };
            let action = host.request_action(71).unwrap().clone();
            if cut != Cut::Pending {
                let (same, input, automatic, offer) = prepared(&mut host, 71, 101, &last);
                assert_eq!(same, action);
                let revision = host.revision();
                let human = roles
                    .oversight
                    .human
                    .approve(&mut host, revision, &offer)
                    .unwrap();
                let revision = host.revision();
                host.dispatch(revision, &automatic, &human, &action, &input, snapshot())
                    .unwrap();
                if matches!(cut, Cut::PublishedUnknown | Cut::Confirmed) {
                    let credential = publication_credential(&host);
                    let revision = host.revision();
                    assert_eq!(
                        host.publish_checked_with_credential(
                            revision,
                            attempt,
                            Some(&input),
                            snapshot(),
                            ElapsedTick(3),
                            &credential,
                        )
                        .unwrap()
                        .outcome,
                        EndpointOutcome::Executed {
                            resulting_version: 2
                        }
                    );
                }
                match cut {
                    Cut::Unknown | Cut::Confirmed => {
                        let revision = host.revision();
                        host.reconcile(revision, attempt).unwrap();
                    }
                    // An ORIGINAL fence converts the executed-but-unaccepted
                    // send to Unknown. The new disposition must add no fence.
                    Cut::PublishedUnknown => {
                        let revision = host.revision();
                        host.fence(revision).unwrap();
                    }
                    _ => {}
                }
            }
            if let Some(remaining) = remaining {
                while host.journal_capacity().unwrap().ordinary_remaining().events > remaining {
                    let revision = host.revision();
                    host.observe_time(revision, ElapsedTick(4)).unwrap();
                }
                assert_eq!(
                    host.journal_capacity().unwrap().ordinary_remaining().events,
                    remaining
                );
            }
            (attempt, action)
        };
        if matches!(cut, Cut::Unknown | Cut::PublishedUnknown) {
            assert!(matches!(actor.poll(&ticket), Knowledge::Unknown { .. }));
        }
        let host = supervisor.host().unwrap();
        let original = host.inspect();
        let control = &original.control;
        let request = FileAbandonRequest {
            operation: 501,
            attempt,
            expected_control_sequence: control.sequence,
            expected_authority_epoch: control.ledger.epoch,
        };
        let mut g = guards(&config);
        g.credential = host.credential_policy().cloned();
        let expected = FileRecoveryRequirements { guards: g, effective_policy: p.delivery.policy.clone(),
            credential_epoch: host.credential_status().unwrap().map(|status|
                fa_reference::action::consequence::delivery::persistent::observed::guarded::FileCredentialEpoch {
                    generation: status.generation, revoked: status.revoked }),
            minimum: FileRecoveryFloor { journal_revision: original.revision,
                control_sequence: control.sequence, authority_epoch: control.ledger.epoch } };
        let topology = family.mediated().then(|| {
            let state = host.mediation_snapshot().unwrap();
            FileTopologyRequirement {
                initial: graph(1, false),
                current: state.graph,
                available: state.available,
            }
        });
        let anchor = host.history_anchor().unwrap();
        let source = source.as_ref().map(|source| {
            source
                .checkpoint(
                    &host,
                    source_document().as_bytes(),
                    source_inventory().as_bytes(),
                )
                .unwrap()
        });
        drop(host);
        drop(supervisor);
        drop(actor);
        let stream = FileOversight::read_stream_publication_with_learned_generation(
            root.store(),
            &p,
            &config,
        )
        .unwrap();
        std::fs::write(
            root.store().join("delivery.pending"),
            b"preserve learned abandonment staging",
        )
        .unwrap();
        Self {
            root,
            family,
            profile: p,
            config,
            expected,
            topology,
            anchor,
            source,
            early_source,
            request,
            original,
            stream,
            action,
        }
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.root.store())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| {
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }
    fn begin_with(
        &self,
        expected: &FileRecoveryRequirements,
        config: &FileLearnedConfig,
        topology: Option<&FileTopologyRequirement>,
        anchor: &FileHistoryAnchor,
    ) -> Result<Recovery, JournalError> {
        let directory = self.root.store();
        let p = self.profile.clone();
        match self.family {
            Family::Core => FileOversight::begin_open_with_learned_generation(directory, p, config)
                .map(Recovery::Core),
            Family::Guarded => FileOversight::begin_open_guarded_anchored_with_learned_generation(
                directory, p, expected, config, anchor,
            )
            .map(Recovery::Guarded),
            Family::Raw => {
                let expected = FilePredictiveRequirements {
                    oversight: expected.clone(),
                    prediction: config.required_pre_output_forecast().unwrap().clone(),
                    evaluation: self.family.evaluation(),
                };
                FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
                    directory, p, &expected, config, anchor,
                )
                .map(Recovery::Raw)
            }
            Family::Owned => {
                let expected = FileOwnedPredictiveRequirements {
                    oversight: expected.clone(),
                    prediction: config.required_owned_pre_output_forecast().unwrap().clone(),
                    evaluation: self.family.evaluation(),
                };
                FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
                    directory, p, &expected, config, anchor).map(Recovery::Owned)
            }
            Family::Mediated => {
                let expected = FileMediatedRequirements {
                    oversight: expected.clone(),
                    topology: topology.unwrap().clone(),
                    prediction: None,
                    evaluation: self.family.evaluation(),
                };
                FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                    directory, p, &expected, config, anchor,
                )
                .map(Recovery::Mediated)
            }
            Family::MediatedRaw | Family::MediatedOwned => {
                let expected = FilePredictiveMediatedRequirements {
                    oversight: expected.clone(),
                    topology: topology.unwrap().clone(),
                    prediction: prediction(config),
                    evaluation: self.family.evaluation(),
                };
                FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                    directory, p, &expected, config, anchor).map(Recovery::PredictiveMediated)
            }
        }
    }
    fn begin(&self) -> Recovery {
        self.begin_with(
            &self.expected,
            &self.config,
            self.topology.as_ref(),
            &self.anchor,
        )
        .unwrap()
    }
    fn ready(&self) -> Recovery {
        let mut run = self.begin();
        run.drive().unwrap();
        run
    }
    fn adopt(&mut self, host: &FileOversight) {
        let state = host.inspect();
        self.expected.minimum = FileRecoveryFloor {
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
        };
        if let Some(expected) = &mut self.topology {
            let state = host.mediation_snapshot().unwrap();
            expected.current = state.graph;
            expected.available = state.available;
        }
        self.anchor = host.history_anchor().unwrap();
    }
}

fn assert_disposition(history: &History, result: &FileAbandonedHistory) {
    assert_eq!(result.receipt().request(), history.request);
    assert_eq!(result.receipt().action(), &history.action);
    assert_eq!(
        result.receipt().charged_units(),
        history.action.spec().units
    );
    let mut expected = history.original.clone();
    expected.revision += 1;
    assert_eq!(
        expected
            .control
            .ledger
            .stages
            .insert(history.request.attempt, ActionState::IrrecoverablyUnknown),
        Some(ActionState::Unknown)
    );
    assert_eq!(result.snapshot(), &expected);
    assert_eq!(result.history_anchor().revision(), expected.revision);
    let after = FileOversight::read_stream_publication_with_learned_generation(
        history.root.store(),
        &history.profile,
        &history.config,
    )
    .unwrap();
    assert_eq!(after.publication, expected);
    assert_eq!(after.confirmed, history.stream.confirmed);
    assert_eq!(after.published, history.stream.published);
    assert_eq!(after.confirmed_target, history.stream.confirmed_target);
    assert_eq!(after.pending, history.stream.pending);
}

#[test]
fn all_original_learned_families_abandon_real_unknown_without_refund_resend_or_stream_advance() {
    for family in FAMILIES {
        for cut in [Cut::Unknown, Cut::PublishedUnknown] {
            let history = History::new(family, cut);
            assert_eq!(
                history.original.executions,
                u64::from(cut == Cut::PublishedUnknown)
            );
            let result = history.ready().abandon(history.request).unwrap();
            assert_disposition(&history, &result);
            let acknowledged = history.files();
            // The immutable result holds no writer lock or original owner.
            let retry = history.ready().abandon(history.request).unwrap();
            assert_eq!(retry, result);
            assert_eq!(history.files(), acknowledged);
            let port = history.ready().receipt().unwrap();
            let envelope = FileLearnedTextStreamActorPort::encode_release(
                71,
                LearnedTextRelease::Message,
                ElapsedTick(100),
            )
            .unwrap();
            let ticket = port.submit(71, &envelope).unwrap();
            assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
            assert_eq!(port.cancel(&ticket), Err(ActorError::Withheld));
            assert_eq!(history.files(), acknowledged);
            let mut host = history.ready().live();
            let before = host.inspect();
            assert!(host.learned_text_stream_finish_request(71, 72).is_err());
            assert!(
                host.publish(host.revision(), history.request.attempt)
                    .is_err()
            );
            assert!(
                host.cancel(host.revision(), history.request.attempt)
                    .is_err()
            );
            assert!(
                host.reconcile(host.revision(), history.request.attempt)
                    .is_err()
            );
            assert_eq!(host.inspect(), before);
            assert_eq!(
                host.abandonment_receipt(history.request.operation),
                Ok(result.receipt())
            );
        }
    }
}

#[test]
fn incomplete_replay_bad_predecessors_and_other_stages_preserve_canonical_and_staging() {
    for family in FAMILIES {
        let history = History::new(family, Cut::Unknown);
        let before = history.files();
        assert_eq!(
            history.begin().abandon(history.request).err(),
            Some(Error::Incomplete.into())
        );
        assert_eq!(history.files(), before);
        for field in 0..5 {
            let mut changed = history.request;
            match field {
                0 => changed.operation = 0,
                1 => changed.attempt = 0,
                2 => changed.attempt += 999,
                3 => changed.expected_control_sequence += 1,
                _ => changed.expected_authority_epoch += 1,
            }
            assert!(
                history.ready().abandon(changed).is_err(),
                "{family:?}, field {field}"
            );
            assert_eq!(history.files(), before);
        }
        let result = history.ready().abandon(history.request).unwrap();
        assert_disposition(&history, &result);
        let acknowledged = history.files();
        for field in 0..3 {
            let mut changed = history.request;
            match field {
                0 => changed.operation += 1,
                1 => changed.attempt += 1,
                _ => changed.expected_authority_epoch += 1,
            }
            assert!(history.ready().abandon(changed).is_err());
            assert_eq!(history.files(), acknowledged);
        }
        for cut in [Cut::Pending, Cut::Sent, Cut::Confirmed] {
            let history = History::new(family, cut);
            let before = history.files();
            assert_eq!(
                history.ready().abandon(history.request).err(),
                Some(Error::WrongState.into()),
                "{family:?}, {cut:?}"
            );
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn every_guarded_family_preserves_independent_guards_policy_credential_floors_and_topology() {
    for family in FAMILIES
        .into_iter()
        .filter(|family| *family != Family::Core)
    {
        let history = History::new(family, Cut::Unknown);
        let before = history.files();
        for field in 0..5 {
            let mut expected = history.expected.clone();
            match field {
                0 => expected.guards.identity = None,
                1 => expected.guards.campaigns = None,
                2 => {
                    expected.effective_policy =
                        Policy::new(2, vec![Predicate::PayloadAtMost(4096)]).unwrap()
                }
                3 => expected.credential_epoch.as_mut().unwrap().generation += 1,
                _ => expected.minimum.journal_revision += 1,
            }
            let result = history
                .begin_with(
                    &expected,
                    &history.config,
                    history.topology.as_ref(),
                    &history.anchor,
                )
                .and_then(|mut run| {
                    run.drive()?;
                    run.abandon(history.request)
                });
            assert!(result.is_err(), "{family:?}, independent input {field}");
            assert_eq!(history.files(), before);
        }
        if let Some(topology) = &history.topology {
            let mut wrong = topology.clone();
            wrong.available = !wrong.available;
            let result = history
                .begin_with(
                    &history.expected,
                    &history.config,
                    Some(&wrong),
                    &history.anchor,
                )
                .and_then(|mut run| {
                    run.drive()?;
                    run.abandon(history.request)
                });
            assert!(result.is_err());
            assert_eq!(history.files(), before);
        }
        let result = history.ready().abandon(history.request).unwrap();
        assert_disposition(&history, &result);
    }
}

#[test]
fn learned_abandonment_checks_the_same_canonical_image_after_readiness() {
    for family in FAMILIES {
        let history = History::new(family, Cut::Unknown);
        let canonical = history.root.bytes();
        let run = history.ready();
        let mut changed = canonical.clone();
        changed[0] ^= 1;
        std::fs::write(history.root.store().join("delivery.bin"), &changed).unwrap();
        let before = history.files();
        assert_eq!(
            run.abandon(history.request).err(),
            Some(Error::Binding.into())
        );
        assert_eq!(history.files(), before);
        std::fs::write(history.root.store().join("delivery.bin"), &canonical).unwrap();
        let result = history.ready().abandon(history.request).unwrap();
        assert_disposition(&history, &result);
    }
}

#[test]
fn ordinary_capacity_never_spends_terminal_reserve_and_exact_retry_needs_no_new_slot() {
    for family in FAMILIES {
        let full = History::build(family, Cut::Unknown, Some(0), false);
        let before = full.files();
        assert_eq!(
            full.ready().abandon(full.request).err(),
            Some(Error::Limit.into())
        );
        assert_eq!(full.files(), before);
        let last = History::build(family, Cut::Unknown, Some(1), false);
        let result = last.ready().abandon(last.request).unwrap();
        assert_disposition(&last, &result);
        let after = last.files();
        assert_eq!(last.ready().abandon(last.request).unwrap(), result);
        assert_eq!(last.files(), after);
    }
}

#[test]
fn source_custody_survives_only_exact_abandonment_with_active_or_withdrawn_topology() {
    for family in MEDIATED {
        for cut in [Cut::Unknown, Cut::PublishedUnknown] {
            let mut history = History::build(family, cut, None, true);
            let source = history.source.clone().unwrap();
            assert_eq!(source.available(), cut == Cut::Unknown);
            let before = history.files();
            assert!(
                history
                    .ready()
                    .abandon_source(history.request, history.early_source.as_ref().unwrap())
                    .is_err()
            );
            assert_eq!(history.files(), before);
            let result = history
                .ready()
                .abandon_source(history.request, &source)
                .unwrap();
            assert_disposition(&history, &result);
            let successor = source
                .after_abandoned_history(&history.profile, &history.root.store(), &result)
                .unwrap();
            assert_eq!(successor.history_anchor(), result.history_anchor());
            assert_eq!(successor.available(), source.available());
            assert_eq!(successor.expected(), source.expected());
            assert_eq!(successor.work(), source.work());
            assert_eq!(successor.last_operation(), source.last_operation());
            assert_eq!(successor.topology_bytes(), source.topology_bytes());
            assert_eq!(successor.inventory_bytes(), source.inventory_bytes());
            let mut archive = Vec::new();
            successor
                .write_to(&mut archive, MAX_TOPOLOGY_CHECKPOINT_BYTES)
                .unwrap();
            assert_eq!(
                TopologyCheckpoint::read_trusted(
                    &mut archive.as_slice(),
                    MAX_TOPOLOGY_CHECKPOINT_BYTES
                )
                .unwrap(),
                successor
            );
            let after = history.files();
            // Both original retained pairs and their acknowledged successors
            // retry without spending another event or source-work allowance.
            for saved in [&source, &successor] {
                let retry = history
                    .ready()
                    .abandon_source(history.request, saved)
                    .unwrap();
                assert_eq!(retry, result);
                assert_eq!(
                    saved
                        .after_abandoned_history(&history.profile, &history.root.store(), &retry)
                        .unwrap(),
                    successor
                );
                assert_eq!(history.files(), after);
            }
            // A normal recovery appends its own Fence. Even if all CURRENT
            // wrapper requirements qualify, an old source cut cannot certify
            // that unrelated suffix through an abandonment retry.
            let mut host = history.ready().live();
            host.observe_time(host.revision(), ElapsedTick(4)).unwrap();
            history.adopt(&host);
            drop(host);
            std::fs::write(
                history.root.store().join("delivery.pending"),
                b"preserve unrelated suffix",
            )
            .unwrap();
            let after = history.files();
            assert_eq!(
                history
                    .ready()
                    .abandon_source(history.request, &source)
                    .err(),
                Some(Error::Stale.into())
            );
            assert_eq!(history.files(), after);
            assert!(
                source
                    .after_abandoned_history(
                        &history.profile,
                        &history.root.store(),
                        &history.ready().abandon(history.request).unwrap()
                    )
                    .is_err()
            );
        }
    }
}
