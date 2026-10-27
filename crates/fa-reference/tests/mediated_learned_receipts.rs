//! Actual learned stream publication and sealed mediated receipt recovery.
//! Original numerical/native review owners supply the positive outcomes; the
//! synthetic models, probability tables and graphs do not qualify detection.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;
#[path = "mediated_learned_recovery/support.rs"]
mod custody;
#[path = "mediated_predictive_learned_recovery/native_fixture.rs"]
mod native_fixture;
#[path = "mediated_predictive_learned_recovery/support.rs"]
#[allow(dead_code)]
mod support;
use support::*;
#[path = "mediated_learned_receipts/custody.rs"]
mod retained_custody;

use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileReconciledLearnedReceipt,
    driver::topology::{FileDriverTopology, checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES}},
};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;

fn source_inventory() -> String {
    String::from_utf8(include_bytes!("fixtures/authority_topology_inventory.json").to_vec()).unwrap()
        .replace("Declared topology is not authenticated deployment coverage",
            "Declared graph only; no operating-system isolation claim")
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
use custody::{graph, protocol, certify, fresh_identity, snapshot};

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction,
    Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::{FrameIdentity,
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::{LinearProbe, learned::MAX_CHECKED_KV_BYTES},
    tensor::kv::{experiment::KvSide, model::MAX_MODEL_KV_VALUES,
        decoder::{DecoderModel, monitoring::restart::KvRestartBudget,
            sampling::{SampledToken, monitored::GenerationEvent}}},
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, stream::StreamProfile, persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits, RecoveryReserve,
    requests::{FileRequestDisposition, actor::{FileLearnedTextStreamActorPort,
        LearnedTextRelease, receipt::FileLearnedReceiptActorPort}},
    observed::{FileHumanRequest, FileOversight, FileOversightProfile,
        consistency::{FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig},
        containment::FileResetRequest,
        decoder::learned::{FileLearnedConfig, FileLearnedInspection,
            FileLearnedRecoveryProgress, FileLearnedRecoveryStatus,
            checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent}},
        guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            anchored::FileHistoryAnchor,
            mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement,
                learned::FileMediatedLearnedRecovery,
                predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements,
                    FilePredictiveMediatedLearnedRecovery}}},
        helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus},
    },
}};
use fa_reference::action::consequence::gate::{ReviewBinding,
    containment::{ActorState, RestartGrade, RestartProfile,
        session::policy::{Policy, Predicate}},
};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, credibility::EvaluationProtocol,
    actor::{ActorError, ActorOutcome, Knowledge},
    actor_wire::{ActorWire, Command, WireError, encode_command},
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_source::{LearnedEvidenceLimits, text::LearnedTextConfig},
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarDecisionBasis, SidecarProbeQuery}},
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::{cell::Cell, collections::{BTreeMap, BTreeSet}, path::PathBuf,
    rc::Rc, sync::atomic::{AtomicU64, Ordering}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane { Plain, Raw, Owned }
const LANES: [Lane; 3] = [Lane::Plain, Lane::Raw, Lane::Owned];

fn stream_contract() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }

fn receipt_profile() -> FileOversightProfile {
    let mut p = support::profile();
    p.delivery.initial_payload.clear();
    p.delivery.total = 4096;
    // Bound the real full-capacity receipt control without thousands of padding writes.
    p.delivery.limits.events = 128;
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    p
}

fn configuration(lane: Lane) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let config = FileLearnedConfig::new_text_stream(model.clone(), numerical::tokenizer(&model),
        source(&model), LearnedDecoderBindingLimits::default(), stream_contract()).unwrap()
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
    }).unwrap().with_stream_messages(stream_contract()).unwrap();
    match lane {
        Lane::Raw => config.with_required_pre_output_forecast(prediction
            .with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Owned => config.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(
            prediction, 1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
            MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap()
            .with_pre_output_forecast().unwrap()).unwrap(),
        Lane::Plain => unreachable!("handled before predictor construction"),
    }
}

fn start(lane: Lane, root: &Directory, p: FileOversightProfile, config: &FileLearnedConfig)
    -> (FileOversight, FileMediatedRoles)
{
    let g = guards(config);
    if lane == Lane::Plain {
        FileOversight::create_mediated_guarded_with_learned_generation_reserved(
            root.store(), p, &g, None, graph(1, false), Some(protocol()), config.clone(),
            RecoveryReserve::terminal()).unwrap()
    } else {
        FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
            root.store(), p, &g, None, graph(1, false), prediction(config), Some(protocol()), config.clone(),
            RecoveryReserve::terminal()).unwrap()
    }
}

fn requirements_for(host: &FileOversight, p: &FileOversightProfile, config: &FileLearnedConfig)
    -> FileMediatedRequirements
{
    let control = host.inspect().control;
    let topology = host.mediation_snapshot().unwrap();
    FileMediatedRequirements { oversight: FileRecoveryRequirements {
        guards: guards(config), effective_policy: p.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch },
    }, topology: FileTopologyRequirement { initial: graph(1, false), current: topology.graph,
        available: topology.available }, prediction: None, evaluation: Some(protocol()) }
}

enum Recovery { Plain(FileMediatedLearnedRecovery), Predictive(FilePredictiveMediatedLearnedRecovery) }
impl Recovery {
    fn require(self) -> Result<Self, JournalError> {
        match self { Self::Plain(run) => run.require_recovery_reserve(RecoveryReserve::terminal()).map(Self::Plain),
            Self::Predictive(run) => run.require_recovery_reserve(RecoveryReserve::terminal()).map(Self::Predictive) }
    }
    fn progress(&self) -> FileLearnedRecoveryProgress {
        match self { Self::Plain(run) => run.progress(), Self::Predictive(run) => run.progress() }
    }
    fn advance(&mut self) -> Result<(), JournalError> {
        let completed = self.progress().replayed_events;
        match self { Self::Plain(run) => { run.advance(completed, 1)?; },
            Self::Predictive(run) => { run.advance(completed, 1)?; } }
        Ok(())
    }
    fn drive(&mut self) -> Result<(), JournalError> {
        while self.progress().status == FileLearnedRecoveryStatus::Replaying { self.advance()?; }
        assert_eq!(self.progress().status, FileLearnedRecoveryStatus::Ready);
        Ok(())
    }
    fn port(self, request: u64, after: Option<u64>) -> Result<FileLearnedReceiptActorPort, JournalError> {
        match self { Self::Plain(run) => run.finish_receipt_port(request, after),
            Self::Predictive(run) => run.finish_receipt_port(request, after) }
    }
    fn reconcile(self, request: u64, clock: impl FnOnce() -> ElapsedTick)
        -> Result<FileLearnedReceiptActorPort, JournalError>
    {
        match self { Self::Plain(run) => run.finish_reconciled_receipt_port(request, None, clock),
            Self::Predictive(run) => run.finish_reconciled_receipt_port(request, None, clock) }
    }

    fn receipt_history<F>(self, request: u64, after: Option<u64>,
        source: Option<&TopologyCheckpoint>, clock: F)
        -> Result<FileReconciledLearnedReceipt, JournalError>
    where F: FnOnce() -> ElapsedTick {
        match (self, source) {
            (Self::Plain(run), None) => run.finish_reconciled_receipt_with_history(request, after, clock),
            (Self::Predictive(run), None) => run.finish_reconciled_receipt_with_history(request, after, clock),
            (Self::Plain(run), Some(source)) => run.finish_reconciled_receipt_with_checkpoint(request, after, source, clock),
            (Self::Predictive(run), Some(source)) => run.finish_reconciled_receipt_with_checkpoint(request, after, source, clock),
        }
    }
    fn port_history<F>(self, source: &TopologyCheckpoint, clock: F)
        -> Result<(FileLearnedReceiptActorPort, FileReconciledLearnedReceipt), JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self {
            Self::Plain(run) => run.finish_reconciled_receipt_port_with_checkpoint(71, None, source, clock),
            Self::Predictive(run) => run.finish_reconciled_receipt_port_with_checkpoint(71, None, source, clock),
        }
    }
    fn finish(self) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
        match self { Self::Plain(run) => run.finish(), Self::Predictive(run) => run.finish() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut { Sent, Published, Confirmed }
struct History {
    root: Directory, lane: Lane, profile: FileOversightProfile, config: FileLearnedConfig,
    expected: FileMediatedRequirements, anchor: FileHistoryAnchor,
    original: Knowledge<ActorOutcome>, numerical: FileLearnedInspection,
    attempt: u64, charge: u64,
    topology: Option<TopologyCheckpoint>, early_topology: Option<TopologyCheckpoint>,
}
impl History {
    fn new(lane: Lane, cut: Cut, full: bool) -> Self {
        Self::build(lane, cut, full, false)
    }
    fn with_source(lane: Lane, cut: Cut) -> Self { Self::build(lane, cut, false, true) }
    fn build(lane: Lane, cut: Cut, full: bool, retain_source: bool) -> Self {
        let root = Directory::new(); let p = receipt_profile(); let config = configuration(lane);
        let (mut host, roles) = start(lane, &root, p.clone(), &config);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        assert!(step(&mut host).sample().is_none());
        assert!(step(&mut host).sample().is_none());
        let mut source = None;
        if retain_source {
            let directory = root.store().parent().unwrap().to_owned();
            let inventory = source_inventory();
            let loaded = LoadedAuthorityTopology::from_json_bytes(source_document().as_bytes(), inventory.as_bytes()).unwrap();
            assert_eq!(loaded.graph(), &graph(1, false));
            std::fs::write(directory.join("topology.json"), source_document()).unwrap();
            std::fs::write(directory.join("inventory.json"), &inventory).unwrap();
            let mut bound = roles.topology_observer.bind_topology_source(&host, host.revision(),
                directory.join("topology.json"), directory.join("inventory.json"), loaded).unwrap();
            let revision = host.revision();
            let refreshed = bound.refresh(&mut host, revision, 1).unwrap();
            assert!(refreshed.refusal.is_none());
            assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
            source = Some(FileDriverTopology::new(bound));
        } else {
            assert!(matches!(certify(&mut host, &roles.topology_observer).unwrap(), CutCheck::Verified(_)));
        }
        let early_topology = source.as_ref().map(|source| source.checkpoint(&host,
            source_document().as_bytes(), source_inventory().as_bytes()).unwrap());
        match lane {
            Lane::Plain => assert!(roles.consistency_observer.is_none()),
            Lane::Raw | Lane::Owned => {
                let mode = if lane == Lane::Raw { Mode::Raw } else { Mode::Owned };
                mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71).unwrap().unwrap();
            }
        }
        let (samples, last) = finish_generation(&mut host);
        assert_eq!(samples.len(), 3);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
        fresh_identity(&mut host, &roles.oversight, 1, 1);
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let ticket = actor.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        let (attempt, charge) = {
            let mut host = supervisor.host_mut().unwrap();
            let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(71).unwrap().disposition
                else { panic!("original stream request must be admitted"); };
            let (action, input, automatic, request) = support::prepared(&mut host, 71, 101, &last);
            assert_eq!(host.inspect().executions, 0);
            let revision = host.revision();
            assert!(host.publish_checked(revision, attempt, Some(&input), snapshot(), ElapsedTick(2)).is_err());
            let revision = host.revision();
            let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
            let revision = host.revision();
            host.dispatch(revision, &automatic, &human, &action, &input, snapshot()).unwrap();
            let charge = action.spec().units;
            assert_eq!(host.inspect().control.ledger.charged, charge);
            assert_eq!(host.delivery_mediation(attempt).unwrap().unwrap().graph(), &graph(1, false));
            if cut != Cut::Sent {
                let revision = host.revision();
                let receipt = host.publish_checked(revision, attempt, Some(&input), snapshot(), ElapsedTick(3)).unwrap();
                assert_eq!(receipt.outcome, EndpointOutcome::Executed { resulting_version: 2 });
                assert_eq!(host.inspect().executions, 1);
                if cut == Cut::Confirmed { let revision = host.revision(); host.reconcile(revision, attempt).unwrap(); }
            }
            (attempt, charge)
        };
        if full {
            let mut host = supervisor.host_mut().unwrap();
            while host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
                let revision = host.revision(); host.observe_time(revision, ElapsedTick(4)).unwrap();
            }
            assert_eq!(host.journal_capacity().unwrap().remaining().events, RecoveryReserve::terminal().events);
        }
        let original = actor.poll(&ticket);
        if cut == Cut::Confirmed {
            assert!(matches!(original, Knowledge::Known { value: ActorOutcome::Executed, .. }));
        } else { assert!(matches!(original, Knowledge::Unknown { .. })); }
        let host = supervisor.host().unwrap();
        let expected = requirements_for(&host, &p, &config);
        let anchor = host.history_anchor().unwrap();
        let numerical = host.learned_generation_inspection().unwrap();
        let topology = source.as_ref().map(|source| source.checkpoint(&host,
            source_document().as_bytes(), source_inventory().as_bytes()).unwrap());
        drop(host); drop(supervisor); drop(actor);
        std::fs::write(root.store().join("delivery.pending"), b"preserve receipt staging").unwrap();
        Self { root, lane, profile: p, config, expected, anchor, original, numerical, attempt, charge, topology, early_topology }
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.root.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = std::fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
    fn begin_with(&self, expected: &FileMediatedRequirements, anchor: &FileHistoryAnchor)
        -> Result<Recovery, JournalError>
    {
        let run = if self.lane == Lane::Plain {
            FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                self.root.store(), self.profile.clone(), expected, &self.config, anchor).map(Recovery::Plain)?
        } else {
            let expected = FilePredictiveMediatedRequirements { oversight: expected.oversight.clone(),
                topology: expected.topology.clone(), prediction: prediction(&self.config),
                evaluation: expected.evaluation.clone() };
            FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                self.root.store(), self.profile.clone(), &expected, &self.config, anchor).map(Recovery::Predictive)?
        };
        run.require()
    }
    fn begin(&self) -> Recovery { self.begin_with(&self.expected, &self.anchor).unwrap() }
    fn ready(&self) -> Recovery {
        let mut run = self.begin(); run.drive().unwrap(); run
    }
}
fn envelope() -> fa_reference::action::consequence::oversight::actor::ActorProposal {
    FileLearnedTextStreamActorPort::encode_release(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap()
}
fn observe(port: &FileLearnedReceiptActorPort) -> Knowledge<ActorOutcome> {
    let ticket = port.submit(71, &envelope()).unwrap();
    assert_eq!(port.cancel(&ticket), Err(ActorError::Withheld));
    port.poll(&ticket)
}

#[test]
fn original_mediated_ports_preserve_real_outcomes_at_full_ordinary_capacity_without_a_lock() {
    for lane in LANES {
        for cut in [Cut::Sent, Cut::Published, Cut::Confirmed] {
            let history = History::new(lane, cut, true); let before = history.files();
            // The terminal reserve cannot fund an ordinary Time/Sweep. The
            // matching immutable receipt remains available without those writes.
            assert_eq!(history.ready().reconcile(71, || ElapsedTick(5)).err(), Some(Error::Limit.into()));
            assert_eq!(history.files(), before);
            let port = history.ready().port(71, None).unwrap();
            assert_eq!(observe(&port), history.original);
            assert_eq!(history.files(), before);
            // Reopen the same exclusive original numerical history while the
            // immutable receipt lives. Its port cannot retain a writer or roles.
            drop(history.begin());
            let other = history.ready().port(71, None).unwrap();
            let ticket = port.submit(71, &envelope()).unwrap();
            assert!(matches!(other.poll(&ticket), Knowledge::Withheld { .. }));
            for _ in 0..3 { assert_eq!(observe(&port), history.original); }
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn original_mediated_receipts_bind_every_source_envelope_field_without_new_admission() {
    for lane in LANES {
        let history = History::new(lane, Cut::Confirmed, false); let before = history.files();
        let port = history.ready().port(71, None).unwrap(); let original = envelope();
        for field in 0..9 {
            let mut changed = original.clone();
            match field {
                0 => changed.target.adapter += 1,
                1 => changed.target.object += 1,
                2 => changed.target.contract_version += 1,
                3 => changed.target.expected_version += 1,
                4 => changed.target.generation += 1,
                5 => changed.expected_policy_epoch += 1,
                6 => changed.units += 1,
                7 => changed.deadline.0 += 1,
                _ => changed.payload[0] ^= 1,
            }
            assert_eq!(port.submit(71, &changed).err(), Some(ActorError::IdempotencyConflict));
            assert_eq!(observe(&port), history.original);
        }
        assert_eq!(port.submit(72, &original).err(), Some(ActorError::Withheld));
        let mut wire = ActorWire::new(port);
        let submit = encode_command(&Command::Submit { request: 71, proposal: original }).unwrap();
        assert_eq!(wire.exchange(&submit).result, Ok(history.original.clone()));
        assert_eq!(wire.exchange(&encode_command(&Command::Cancel { request: 71 }).unwrap()).result, Err(WireError::Withheld));
        assert_eq!(wire.exchange(&submit).result, Ok(history.original.clone()));
        assert_eq!(history.files(), before);
    }
}

#[test]
fn missing_request_incomplete_replay_and_wrong_independent_guards_never_issue_a_port_or_read_time() {
    for lane in LANES {
        let history = History::new(lane, Cut::Confirmed, false); let before = history.files();
        assert_eq!(history.begin().port(71, None).err(), Some(Error::Incomplete.into()));
        assert!(history.ready().port(72, None).is_err());
        assert!(history.ready().port(71, Some(70)).is_err());
        assert!(history.ready().reconcile(72, || panic!("missing request must not read time")).is_err());
        let foreign = History::new(lane, Cut::Confirmed, false);
        assert!(history.begin_with(&history.expected, &foreign.anchor).is_err());
        for field in 0..5 {
            let mut wrong = history.expected.clone();
            match field {
                0 => wrong.oversight.guards.stream = None,
                1 => wrong.topology.current = graph(1, true),
                2 => wrong.topology.available = !wrong.topology.available,
                3 => wrong.oversight.minimum.journal_revision += 1,
                _ => wrong.evaluation = None,
            }
            if let Ok(mut run) = history.begin_with(&wrong, &history.anchor) {
                assert!(run.drive().is_err());
                assert!(run.reconcile(71, || panic!("failed independent guard must not read time")).is_err());
            }
            assert_eq!(history.files(), before, "{lane:?}: changed guard {field}");
        }
        let port = history.ready().port(71, None).unwrap();
        assert_eq!(observe(&port), history.original);
        assert_eq!(history.files(), before);
    }
}

#[test]
fn original_reconciliation_confirms_only_real_publication_and_preserves_sent_unknown_charges() {
    for lane in LANES {
        for cut in [Cut::Sent, Cut::Published] {
            let history = History::new(lane, cut, false); let before = history.files();
            assert!(matches!(history.original, Knowledge::Unknown { .. }));
            assert!(history.ready().reconcile(71, || ElapsedTick(0)).is_err());
            assert_eq!(history.files(), before);
            let calls = Cell::new(0);
            let port = history.ready().reconcile(71, || { calls.set(calls.get() + 1); ElapsedTick(5) }).unwrap();
            assert_eq!(calls.get(), 1);
            let outcome = observe(&port);
            if cut == Cut::Published {
                assert!(matches!(outcome, Knowledge::Known { value: ActorOutcome::Executed, .. }));
            } else { assert!(matches!(outcome, Knowledge::Unknown { .. })); }
            let committed = history.files();
            for _ in 0..3 { assert_eq!(observe(&port), outcome); }
            assert_eq!(history.files(), committed);
            // The independently selected post-fence requirement is explicit.
            // A stale available=true requirement must not infer a new value.
            assert!(history.begin().drive().is_err());
            let mut expected = history.expected.clone(); expected.topology.available = false;
            let mut run = history.begin_with(&expected, &history.anchor).unwrap();
            assert_eq!(run.progress().total_events as u64, history.expected.oversight.minimum.journal_revision + 3);
            run.drive().unwrap();
            // A separate acknowledged reopen supplies the original ledger view
            // without giving the receipt port access to this live owner.
            let (host, _) = run.finish().unwrap();
            assert_eq!(host.revision(), history.expected.oversight.minimum.journal_revision + 4);
            assert_eq!(host.inspect().control.ledger.charged, history.charge);
            assert_eq!(host.inspect().executions, u64::from(cut == Cut::Published));
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, history.numerical.numerical);
            assert!(host.learned_generation_inspection().unwrap().paused);
            assert!(!host.mediation_snapshot().unwrap().available);
            if cut == Cut::Sent { assert_eq!(host.inspect().control.ledger.stages[&history.attempt], ActionState::Unknown); }
            if lane != Lane::Plain { assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1); }
            assert_eq!(observe(&port), outcome);
        }
    }
}

#[test]
fn a_changed_canonical_cut_refuses_before_port_issuance_and_after_the_reconciliation_clock() {
    for lane in LANES {
        let history = History::new(lane, Cut::Published, false);
        let path = history.root.store().join("delivery.bin"); let original = history.root.bytes();
        let mut changed = original.clone(); let last = changed.len() - 1; changed[last] ^= 1;
        let run = history.ready();
        std::fs::write(&path, &changed).unwrap();
        assert_eq!(run.port(71, None).err(), Some(Error::Binding.into()));
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        assert_eq!(std::fs::read(history.root.store().join("delivery.pending")).unwrap(), b"preserve receipt staging");
        std::fs::write(&path, &original).unwrap();
        let run = history.ready(); let calls = Cell::new(0);
        assert_eq!(run.reconcile(71, || {
            calls.set(calls.get() + 1); std::fs::write(&path, &changed).unwrap(); ElapsedTick(5)
        }).err(), Some(Error::Binding.into()));
        assert_eq!(calls.get(), 1); assert_eq!(std::fs::read(&path).unwrap(), changed);
        assert_eq!(std::fs::read(history.root.store().join("delivery.pending")).unwrap(), b"preserve receipt staging");
        std::fs::write(&path, original).unwrap();
        let port = history.ready().reconcile(71, || ElapsedTick(5)).unwrap();
        assert!(matches!(observe(&port), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    }
}
