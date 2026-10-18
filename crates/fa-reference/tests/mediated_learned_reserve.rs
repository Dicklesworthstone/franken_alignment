//! Reserved mediated startup and terminal recovery through original numerical,
//! topology, congress, human-key, journal and endpoint owners. Synthetic weights,
//! ballots and topology fixtures are controls, not deployment qualification.
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
use custody::{graph, protocol, certify, snapshot};

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction,
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
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits,
    requests::FileRequestDisposition,
    observed::{FileHumanRequest, FileOversight, FileOversightProfile,
        consistency::{FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig},
        containment::FileResetRequest,
        decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus,
            checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent}},
        guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement,
                predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements,
                    FilePredictiveMediatedLearnedRecovery}}},
        helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus},
    },
};
use fa_reference::action::consequence::gate::{ReviewBinding,
    containment::{ActorState, RestartGrade, RestartProfile,
        session::policy::{Policy, Predicate}},
};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, credibility::EvaluationProtocol,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_source::{LearnedEvidenceLimits, text::LearnedTextConfig},
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarDecisionBasis, SidecarProbeQuery}},
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use fa_reference::action::consequence::delivery::{StopRequest, stream::StreamProfile,
    persistent::{RecoveryReserve, observed::{
        guarded::{anchored::FileHistoryAnchor, mediated::learned::FileMediatedLearnedRecovery},
        decoder::learned::FileLearnedRecoveryProgress,
        shutdown::recovery::{FileStoppedRecovery, FileStoppedHistory},
    }}};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane { Plain, Raw, Owned }
const LANES: [Lane; 3] = [Lane::Plain, Lane::Raw, Lane::Owned];

fn stream_contract() -> StreamProfile { StreamProfile::new(1, 1, 4, 64, 128).unwrap() }

fn boundary_profile(stream: bool) -> FileOversightProfile {
    let mut p = profile();
    if stream {
        p.delivery.initial_payload.clear();
        p.delivery.total = 4096;
        p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    }
    p
}

fn configuration(lane: Lane, stream: bool) -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let source = source(&model);
    let config = if stream {
        FileLearnedConfig::new_text_stream(model.clone(), numerical::tokenizer(&model), source,
            LearnedDecoderBindingLimits::default(), stream_contract()).unwrap()
    } else {
        FileLearnedConfig::new_text(model.clone(), numerical::tokenizer(&model), source,
            LearnedDecoderBindingLimits::default()).unwrap()
    }.with_required_sidecar().unwrap();
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
    let prediction = if stream { prediction.with_stream_messages(stream_contract()).unwrap() }
        else { prediction };
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

fn create_case(lane: Lane, root: &Directory, p: FileOversightProfile, config: &FileLearnedConfig,
    reserve: Option<RecoveryReserve>) -> Result<(FileOversight, FileMediatedRoles), JournalError>
{
    let g = guards(config);
    match (lane, reserve) {
        (Lane::Plain, None) => FileOversight::create_mediated_guarded_with_learned_generation(
            root.store(), p, &g, None, graph(1, false), Some(protocol()), config.clone()),
        (Lane::Plain, Some(reserve)) => FileOversight::create_mediated_guarded_with_learned_generation_reserved(
            root.store(), p, &g, None, graph(1, false), Some(protocol()), config.clone(), reserve),
        (_, None) => FileOversight::create_predictive_mediated_guarded_with_learned_generation(
            root.store(), p, &g, None, graph(1, false), prediction(config), Some(protocol()), config.clone()),
        (_, Some(reserve)) => FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
            root.store(), p, &g, None, graph(1, false), prediction(config), Some(protocol()), config.clone(), reserve),
    }
}

fn expected(host: &FileOversight, p: &FileOversightProfile, config: &FileLearnedConfig)
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
    fn require(self, reserve: RecoveryReserve) -> Result<Self, JournalError> {
        match self { Self::Plain(run) => run.require_recovery_reserve(reserve).map(Self::Plain),
            Self::Predictive(run) => run.require_recovery_reserve(reserve).map(Self::Predictive) }
    }
    fn progress(&self) -> FileLearnedRecoveryProgress {
        match self { Self::Plain(run) => run.progress(), Self::Predictive(run) => run.progress() }
    }
    fn advance(&mut self, expected: usize, count: usize) -> Result<FileLearnedRecoveryProgress, JournalError> {
        match self { Self::Plain(run) => run.advance(expected, count),
            Self::Predictive(run) => run.advance(expected, count) }
    }
    fn drive(&mut self) -> Result<(), JournalError> {
        while self.progress().status == FileLearnedRecoveryStatus::Replaying {
            let before = self.progress(); let after = self.advance(before.replayed_events, 1)?;
            assert_eq!(after.replayed_events, before.replayed_events + 1);
        }
        Ok(())
    }
    fn finish(self) -> Result<(FileOversight, FileMediatedRoles), JournalError> {
        match self { Self::Plain(run) => run.finish(), Self::Predictive(run) => run.finish() }
    }
    fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        match self { Self::Plain(run) => run.stop_request(operation),
            Self::Predictive(run) => run.stop_request(operation) }
    }
    fn finish_stopped(self, request: StopRequest, clock: impl FnOnce() -> ElapsedTick)
        -> Result<FileStoppedRecovery, JournalError>
    {
        match self { Self::Plain(run) => run.finish_stopped(request, clock),
            Self::Predictive(run) => run.finish_stopped(request, clock) }
    }
    fn finish_stopped_with_anchor(self, request: StopRequest, clock: impl FnOnce() -> ElapsedTick)
        -> Result<FileStoppedHistory, JournalError>
    {
        match self { Self::Plain(run) => run.finish_stopped_with_anchor(request, clock),
            Self::Predictive(run) => run.finish_stopped_with_anchor(request, clock) }
    }
}

fn begin(lane: Lane, root: &Directory, p: &FileOversightProfile, config: &FileLearnedConfig,
    expected: &FileMediatedRequirements, anchor: Option<&FileHistoryAnchor>) -> Result<Recovery, JournalError>
{
    if lane == Lane::Plain {
        return match anchor {
            Some(anchor) => FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                root.store(), p.clone(), expected, config, anchor),
            None => FileOversight::begin_open_mediated_guarded_with_learned_generation(
                root.store(), p.clone(), expected, config),
        }.map(Recovery::Plain);
    }
    let expected = FilePredictiveMediatedRequirements { oversight: expected.oversight.clone(),
        topology: expected.topology.clone(), prediction: prediction(config), evaluation: expected.evaluation.clone() };
    match anchor {
        Some(anchor) => FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            root.store(), p.clone(), &expected, config, anchor),
        None => FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
            root.store(), p.clone(), &expected, config),
    }.map(Recovery::Predictive)
}

fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    std::fs::read_dir(root.store()).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file()).map(|path| {
            let bytes = std::fs::read(&path).unwrap(); (path, bytes)
        }).collect()
}

fn prefill(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(step(host).sample().is_none());
    assert!(step(host).sample().is_none());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 2);
}

fn forecast(lane: Lane, host: &mut FileOversight, roles: &FileMediatedRoles, request: u64) {
    match lane {
        Lane::Plain => assert!(roles.consistency_observer.is_none()),
        Lane::Raw | Lane::Owned => {
            let mode = if lane == Lane::Raw { Mode::Raw } else { Mode::Owned };
            mode.forecast(host, roles.consistency_observer.as_ref().unwrap(), request).unwrap().unwrap();
        }
    }
}

#[test]
fn original_terminal_reserve_preserves_numerical_output_and_separate_mediation_roles() {
    for lane in LANES {
        let root = Directory::new(); let control = Directory::new(); let p = boundary_profile(false);
        let config = configuration(lane, false);
        let (mut host, roles) = create_case(lane, &root, p.clone(), &config, Some(RecoveryReserve::terminal())).unwrap();
        let (mut baseline, baseline_roles) = create_case(lane, &control, p, &config, None).unwrap();
        assert_eq!(host.revision(), baseline.revision() + 1);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert_eq!(baseline.journal_capacity().unwrap().reserve(), None);
        assert!(!host.clock_ready()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert!(roles.oversight.identity_observer.is_some() && roles.oversight.policy_governor.is_some()
            && roles.evaluator.is_some());
        assert_eq!(roles.consistency_observer.is_some(), lane != Lane::Plain);
        let mut results = Vec::new();
        for (current, custody) in [(&mut host, &roles), (&mut baseline, &baseline_roles)] {
            prefill(current);
            if lane != Lane::Plain {
                let n = current.learned_generation_inspection().unwrap().numerical;
                let revision = current.revision();
                assert_eq!(current.advance_learned_generation(revision, n.actor_revision, n.position).err(),
                    Some(Error::Incomplete.into()));
                assert_eq!(current.revision(), revision);
            }
            forecast(lane, current, custody, 71);
            let (samples, _) = finish_generation(current);
            assert_eq!(current.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
            assert_eq!(samples.len(), 3); results.push(samples);
            assert!(matches!(certify(current, &custody.topology_observer).unwrap(), CutCheck::Verified(_)));
            assert_eq!(current.inspect().executions, 0);
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            baseline.learned_generation_inspection().unwrap().numerical);
        assert_eq!(host.inspect().control.ledger.charged, 0);
    }
}

#[test]
fn invalid_reserve_or_insufficient_bootstrap_capacity_creates_no_partial_owner() {
    for lane in LANES {
        let config = configuration(lane, false);
        for reserve in [RecoveryReserve { events: 2, bytes: 50 }, RecoveryReserve { events: 3, bytes: 49 }] {
            let root = Directory::new();
            assert_eq!(create_case(lane, &root, boundary_profile(false), &config, Some(reserve)).err(),
                Some(Error::InvalidInput.into()));
            assert!(!root.store().exists());
        }
        let root = Directory::new(); let mut limited = boundary_profile(false); limited.delivery.limits.events = 5;
        assert_eq!(create_case(lane, &root, limited, &config, Some(RecoveryReserve::terminal())).err(),
            Some(Error::Limit.into()));
        assert!(!root.store().exists());
        std::fs::write(root.store(), b"original non-directory").unwrap();
        assert!(create_case(lane, &root, boundary_profile(false), &config, Some(RecoveryReserve::terminal())).is_err());
        assert_eq!(std::fs::read(root.store()).unwrap(), b"original non-directory");
        let good = Directory::new();
        let (host, _) = create_case(lane, &good, boundary_profile(false), &config, Some(RecoveryReserve::terminal())).unwrap();
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn missing_changed_or_late_reserve_and_unmet_topology_leave_original_history_untouched() {
    for lane in LANES {
        let config = configuration(lane, false); let p = boundary_profile(false);
        for reserve in [None, Some(RecoveryReserve { events: 4, bytes: 60 })] {
            let root = Directory::new(); let (host, _) = create_case(lane, &root, p.clone(), &config, reserve).unwrap();
            let expected = expected(&host, &p, &config); drop(host);
            std::fs::write(root.store().join("delivery.pending"), b"unconfirmed candidate").unwrap();
            let before = files(&root);
            let run = begin(lane, &root, &p, &config, &expected, None).unwrap();
            assert_eq!(run.require(RecoveryReserve::terminal()).err(), Some(Error::Binding.into()));
            assert_eq!(files(&root), before);
        }
        let root = Directory::new();
        let (mut host, _) = create_case(lane, &root, p.clone(), &config, Some(RecoveryReserve::terminal())).unwrap();
        prefill(&mut host); let n = host.learned_generation_inspection().unwrap().numerical;
        let expected = expected(&host, &p, &config); let anchor = host.history_anchor().unwrap(); drop(host);
        std::fs::write(root.store().join("delivery.pending"), b"do not clean before validation").unwrap();
        let before = files(&root);
        let mut late = begin(lane, &root, &p, &config, &expected, Some(&anchor)).unwrap();
        late.advance(0, 1).unwrap();
        assert_eq!(late.require(RecoveryReserve::terminal()).err(), Some(Error::WrongState.into()));
        assert_eq!(files(&root), before);
        for field in 0..4 {
            let mut wrong = expected.clone();
            match field { 0 => wrong.topology.current = graph(1, true),
                1 => wrong.topology.available = !wrong.topology.available,
                2 => wrong.oversight.minimum.journal_revision += 1,
                _ => wrong.evaluation = None }
            match begin(lane, &root, &p, &config, &wrong, Some(&anchor)) {
                Err(_) => {},
                Ok(run) => {
                    let mut run = run.require(RecoveryReserve::terminal()).unwrap();
                    assert!(run.drive().is_err());
                    assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
                    assert!(run.stop_request(91).is_err());
                    assert!(run.finish_stopped(StopRequest { operation: 91,
                        expected_control_sequence: expected.oversight.minimum.control_sequence,
                        expected_authority_epoch: expected.oversight.minimum.authority_epoch },
                        || panic!("unmet independent guards cannot acquire time")).is_err());
                }
            }
            assert_eq!(files(&root), before, "{lane:?}, changed field {field}");
        }
        let mut run = begin(lane, &root, &p, &config, &expected, Some(&anchor)).unwrap()
            .require(RecoveryReserve::terminal()).unwrap();
        run.drive().unwrap(); assert_eq!(files(&root), before);
        let (host, roles) = run.finish().unwrap();
        assert_eq!(host.revision(), expected.oversight.minimum.journal_revision + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready() && !host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(roles.consistency_observer.is_some(), lane != Lane::Plain);
        assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
        assert!(!root.store().join("delivery.pending").exists());
    }
}

#[test]
fn mediated_stop_uses_only_the_reserved_tail_and_completed_retry_does_not_fence_again() {
    for lane in LANES {
        let root = Directory::new(); let mut p = boundary_profile(false); p.delivery.limits.events = 32;
        let ordinary_limit = p.delivery.limits.events - RecoveryReserve::terminal().events;
        let config = configuration(lane, false);
        let (mut host, roles) = create_case(lane, &root, p.clone(), &config, Some(RecoveryReserve::terminal())).unwrap();
        prefill(&mut host);
        forecast(lane, &mut host, &roles, 71);
        let mut tick = 2;
        while host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
            assert!(tick <= 32, "fixture must fill only its bounded ordinary allowance");
            host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); tick += 1;
        }
        assert_eq!(host.revision(), ordinary_limit as u64);
        assert_eq!(host.journal_capacity().unwrap().remaining().events, 3);
        let before = host.inspect(); let mut expected = expected(&host, &p, &config);
        let anchor = host.history_anchor().unwrap(); let canonical = root.bytes();
        assert_eq!(host.observe_time(host.revision(), ElapsedTick(tick)).err(), Some(Error::Limit.into()));
        assert_eq!(root.bytes(), canonical); drop(host);
        std::fs::write(root.store().join("delivery.pending"), b"unacknowledged candidate").unwrap();
        let untouched = files(&root);
        let mut run = begin(lane, &root, &p, &config, &expected, Some(&anchor)).unwrap()
            .require(RecoveryReserve::terminal()).unwrap();
        assert_eq!(run.stop_request(91), Err(Error::Incomplete.into()));
        run.drive().unwrap(); assert_eq!(files(&root), untouched);
        let stop = run.stop_request(91).unwrap(); let calls = Cell::new(0);
        let result = run.finish_stopped_with_anchor(stop, || {
            calls.set(calls.get() + 1); ElapsedTick(tick)
        }).unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(result.stopped().snapshot().revision, before.revision + 3);
        assert_eq!(result.stopped().snapshot().revision, 32);
        assert_eq!(result.history_anchor().revision(), result.stopped().snapshot().revision);
        assert!(result.stopped().progress().drained());
        assert_eq!(result.stopped().snapshot().executions, 0);
        assert_eq!(result.stopped().snapshot().control.ledger.charged, 0);
        let completed = root.bytes();
        expected.topology.available = false;
        std::fs::write(root.store().join("delivery.pending"), canonical).unwrap();
        let mut run = begin(lane, &root, &p, &config, &expected, Some(result.history_anchor())).unwrap()
            .require(RecoveryReserve::terminal()).unwrap();
        run.drive().unwrap();
        assert_eq!(run.stop_request(91).unwrap(), stop);
        assert_eq!(run.stop_request(92), Err(Error::Duplicate.into()));
        let retry = run.finish_stopped(stop, || panic!("acknowledged drained stop must not read time")).unwrap();
        assert!(matches!(retry, FileStoppedRecovery::AlreadyDrained { .. }));
        assert_eq!(retry.snapshot(), result.stopped().snapshot());
        assert_eq!(retry.progress(), result.stopped().progress());
        assert_eq!(root.bytes(), completed);
        assert!(!root.store().join("delivery.pending").exists());
    }
}

