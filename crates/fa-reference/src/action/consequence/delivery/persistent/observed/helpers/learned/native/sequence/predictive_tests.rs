//! Original forecast and native Begin boundaries after a real learned Hold.
//! Public operator/ticket lifecycle coverage lives in the integration suite.
use super::*;
use crate::action::ResolvedTarget;
use crate::action::consequence::activation::consistency::{
    BinaryForecast, ErrorBudget, ForecastRegistration, LikelihoodEvidence,
};
use crate::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedMonitorWork,
};
use crate::action::consequence::activation::probe::{LinearProbe, learned::MAX_CHECKED_KV_BYTES};
use crate::action::consequence::activation::tensor::kv::experiment::KvSide;
use crate::action::consequence::delivery::persistent::{
    RecoveryReserve,
    observed::{
        FileHumanReviewer, FileOversightProfile,
        consistency::{
            FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig,
        },
        decoder::learned::FileLearnedConfig,
        source::FileSourcePolicy,
    },
    requests::{
        FileRequestDisposition,
        actor::{FileActorSupervisor, LearnedTextProposal, LearnedTextRelease},
    },
};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::{
    consistency::{ConsistencyObservation, LearnedConsistencyObservation},
    decoder_monitoring::LearnedDecoderBindingLimits,
    evidence_source::{
        EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
    },
    learned_host::sidecar::LearnedSidecarRequest,
    learned_source::LearnedEvidenceLimits,
    policy_state::{StateFreshness, StateLimits, StateSource},
    sidecar::{SidecarCongressBudget, SidecarIdentity},
};

#[allow(dead_code)]
#[path = "../../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

const REQUEST: u64 = 71;
const FIRST: u64 = 101;
const NEXT: u64 = 102;

fn stream() -> StreamProfile {
    StreamProfile::new(61, 1, 4, 32, 128).unwrap()
}
fn policy() -> FileSourcePolicy {
    FileSourcePolicy {
        source: StateSource {
            scope: profile().delivery.scope,
            source: 42,
            generation: 1,
        },
        limits: StateLimits::default(),
        freshness: StateFreshness::new(100).unwrap(),
    }
}
fn prediction(owned: bool, age: u64) -> FileConsistencyConfig {
    let (model, _, _) = recipe();
    let pair = BinaryForecast::new(32_768, 32_768).unwrap();
    let config = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91,
        probe_generation: 1,
        profile: if owned {
            model.cache_profile().layers()[&1].values().profile()
        } else {
            model.residual_contract(1).unwrap().profile()
        },
        weights: vec![1.0, 0.0],
        bias: 0.0,
        threshold: 0.0,
        forecast: ForecastRegistration {
            domain: 71,
            generation: 1,
            policy_generation: 1,
            event_prefix: b"aa".to_vec(),
            negative: pair,
            at_threshold: pair,
            positive: pair,
        },
        alpha: ErrorBudget::new(1, 2).unwrap(),
        stream: 21,
        max_predictions: 8,
        max_prediction_age_ticks: age,
    })
    .unwrap();
    if owned {
        config.with_stream_messages(stream()).unwrap()
    } else {
        config
            .with_hosted_residual(1)
            .unwrap()
            .with_stream_messages(stream())
            .unwrap()
            .with_pre_output_forecast()
            .unwrap()
    }
}
fn owned_prediction(age: u64) -> FileLearnedConsistencyConfig {
    FileLearnedConsistencyConfig::new(
        prediction(true, age),
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
    .unwrap()
}
fn configured(owned: bool) -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    let config = FileLearnedConfig::new_text_stream(
        model,
        tokenizer,
        source,
        LearnedDecoderBindingLimits::default(),
        stream(),
    )
    .unwrap()
    .with_required_sidecar()
    .unwrap()
    .with_required_policy_source(policy())
    .unwrap();
    if owned {
        config
            .with_required_owned_pre_output_forecast(owned_prediction(100))
            .unwrap()
    } else {
        config
            .with_required_pre_output_forecast(prediction(false, 100))
            .unwrap()
    }
}

struct Pending {
    root: Directory,
    host: FileOversight,
    observer: FileConsistencyObserver,
}
fn pending(owned: bool) -> Pending {
    let root = Directory::new();
    let mut selected = profile();
    selected.delivery.initial_payload.clear();
    selected.delivery.total = 1000;
    // Two substantive native Hold judgments remain below Narrow. Abstentions
    // would instead let the original planner buy automatic residual refinement.
    selected.delivery.congress.narrow_at = 3;
    selected.delivery.congress.suspend_at = 4;
    let (mut host, _, observer) = FileOversight::create_with_pre_output_forecast_with_reserve(
        root.store(),
        selected,
        configured(owned),
        RecoveryReserve::terminal(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let path = root.store().with_extension("policy");
    let captured = EvidenceSnapshot::new(
        EvidenceIdentity {
            scope: profile().delivery.scope,
            source: 42,
            generation: 1,
        },
        snapshot(),
        ["alpha", "beta"]
            .into_iter()
            .map(|name| (name.to_owned(), Vec::new()))
            .collect(),
    )
    .unwrap();
    std::fs::write(&path, captured.encode()).unwrap();
    let mut source =
        FileEvidenceSource::new(&path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES)
            .unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    step(&mut host);
    let revision = host.revision();
    let actor = host
        .learned_generation_inspection()
        .unwrap()
        .numerical
        .actor_revision;
    if owned {
        observer
            .forecast_owned_learned_request(&mut host, revision, REQUEST, actor)
            .unwrap()
            .unwrap()
            .prediction()
            .unwrap();
    } else {
        observer
            .forecast_hosted_request(&mut host, revision, REQUEST, actor)
            .unwrap()
            .unwrap();
    }
    Pending {
        root,
        host,
        observer,
    }
}
fn submit(host: &mut FileOversight) -> u64 {
    step(host);
    step(host);
    let spec = host
        .prepare_learned_text_stream_request(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    host.submit_request(host.revision(), REQUEST, spec, snapshot())
        .unwrap();
    let FileRequestDisposition::Admitted {
        attempt,
        stage: ActionState::Reviewing,
    } = host.request_status(REQUEST).unwrap().disposition
    else {
        panic!("real predictive learned request was not admitted");
    };
    assert_eq!(
        host.action_consistency_snapshot()
            .unwrap()
            .evidence
            .samples(),
        1
    );
    assert_eq!(host.machine.check_native_learned_convening(attempt), Ok(()));
    attempt
}
fn round(id: u64) -> LearnedWorkerRound {
    LearnedWorkerRound {
        round: id,
        evidence_root: [7; 32],
        window: crate::action::consequence::oversight::ReviewWindow {
            commit_by: ElapsedTick(if id == FIRST { 10 } else { 30 }),
            reveal_by: ElapsedTick(if id == FIRST { 15 } else { 35 }),
        },
    }
}
fn members(
    host: &FileOversight,
    sidecar: &FileLearnedSidecar,
    spelling: &[u8],
    salt: u8,
) -> BTreeMap<String, NativeReviewMember> {
    let original = host.checked_learned_sidecar(sidecar).unwrap();
    let rows: BTreeSet<_> = original.source().groups().map(|group| group.row).collect();
    original
        .round()
        .input()
        .views()
        .iter()
        .enumerate()
        .map(|(index, (name, view))| {
            let queries = rows
                .iter()
                .map(|row| {
                    let (frame, heads, channels) = original.source().row_shape(*row).unwrap();
                    SidecarProbeQuery {
                        row: *row,
                        probe: LinearProbe::new(
                            1,
                            1,
                            frame.profile,
                            &vec![0.0; heads * channels],
                            0.0,
                            1.0,
                        )
                        .unwrap(),
                    }
                })
                .collect();
            (
                name.clone(),
                NativeReviewMember {
                    evaluator: native_fixture::evaluator(
                        view.actual_input().input_profile().clone(),
                        spelling,
                    ),
                    queries,
                    salt: vec![salt + index as u8; 32],
                },
            )
        })
        .collect()
}
fn held(owned: bool, reserved: bool) -> (Pending, FileNativeSidecarSequence, u64) {
    let mut case = pending(owned);
    let host = &mut case.host;
    let attempt = submit(host);
    let source = host
        .machine
        .broker
        .learned_decoder_evidence(attempt)
        .unwrap()
        .unwrap();
    let request = LearnedSidecarRequest {
        identity: SidecarIdentity {
            object_id: 1001,
            generation: 1,
            transform_id: 7,
        },
        priority: source.audit().source().groups().collect(),
        budget: SidecarCongressBudget::default(),
    };
    let actor = host
        .learned_generation_inspection()
        .unwrap()
        .numerical
        .actor_revision;
    let sidecar = host
        .begin_learned_sidecar_plan(host.revision(), attempt, actor, request)
        .unwrap();
    let mut rosters = BTreeMap::from([(FIRST, members(host, &sidecar, b"hold", 16))]);
    let mut rounds = vec![round(FIRST)];
    let next = members(host, &sidecar, b"allow", 32);
    let successors = if reserved {
        vec![next, members(host, &sidecar, b"allow", 48)]
    } else {
        rosters.insert(NEXT, next);
        rounds.push(round(NEXT));
        Vec::new()
    };
    let mut run = host
        .begin_native_sidecar_sequence_with_convening(
            host.revision(),
            sidecar,
            rounds,
            rosters,
            NativeReviewLimits::default(),
            snapshot(),
            successors,
        )
        .unwrap();
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.status() != NativeReviewStatus::Running {
            break;
        }
        let revision = run.revision();
        run.advance(host, revision, ElapsedTick(1), snapshot())
            .unwrap();
    }
    assert_eq!(run.status(), NativeReviewStatus::Finished);
    assert!(matches!(run.history().last(),
        Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. })
            if receipt.policy.control.decision.consequence == Consequence::HoldEffect));
    assert_eq!(host.native_convene_status(REQUEST).unwrap().1, None);
    (case, run, attempt)
}
fn bytes(case: &Pending) -> Vec<u8> {
    std::fs::read(case.root.store().join("delivery.bin")).unwrap()
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Observation {
    Raw(ConsistencyObservation),
    Owned(LearnedConsistencyObservation),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ForecastState {
    evidence: LikelihoodEvidence,
    pending: Option<u64>,
    lost: bool,
    owned_work: Option<(LearnedMonitorWork, usize, bool)>,
    observation: Observation,
}
fn forecast_state(host: &FileOversight, attempt: u64) -> ForecastState {
    let current = host.action_consistency_snapshot().unwrap();
    let (owned_work, observation) = if host.owned_learned_action_consistency_required() {
        let owned = host.learned_action_consistency_snapshot().unwrap();
        (
            Some((
                owned.work,
                owned.retained_source_bytes,
                owned.has_unreported_work,
            )),
            Observation::Owned(
                host.learned_action_consistency_observation(attempt)
                    .unwrap()
                    .clone(),
            ),
        )
    } else {
        (
            None,
            Observation::Raw(
                host.action_consistency_observation(attempt)
                    .unwrap()
                    .clone(),
            ),
        )
    };
    ForecastState {
        evidence: current.evidence,
        pending: current.pending_attempt,
        lost: current.coverage_lost,
        owned_work,
        observation,
    }
}

#[test]
fn required_configuration_cannot_ignore_missing_or_different_installed_prediction() {
    for owned in [false, true] {
        let (mut case, run, _) = held(owned, false);
        let raw = case.host.machine.consistency.clone();
        let learned = case.host.machine.learned_consistency.clone();
        let canonical = bytes(&case);
        assert!(case.host.machine.native_learned_convening_supported());
        assert!(run.check_held_successor(&case.host, ElapsedTick(1)).is_ok());
        // Exercise the structural equality boundary directly. These mismatches
        // are never installed by a public role or written into a journal.
        for changed in 0..3 {
            case.host.machine.consistency = raw.clone();
            case.host.machine.learned_consistency = learned.clone();
            match changed {
                0 => case.host.machine.consistency = None,
                1 => case.host.machine.consistency = Some(Rc::new(prediction(owned, 101))),
                _ => case.host.machine.learned_consistency = Some(Rc::new(owned_prediction(101))),
            }
            assert!(!case.host.machine.native_learned_convening_supported());
            assert!(
                run.check_held_successor(&case.host, ElapsedTick(1))
                    .is_err()
            );
            assert_eq!(bytes(&case), canonical);
        }
        case.host.machine.consistency = raw;
        case.host.machine.learned_consistency = learned;
        assert!(run.check_held_successor(&case.host, ElapsedTick(1)).is_ok());
    }
}

#[test]
fn a_real_pending_pre_output_forecast_is_not_a_settled_native_successor_basis() {
    for owned in [false, true] {
        let mut case = pending(owned);
        let before = case.host.action_consistency_snapshot().unwrap();
        let attempt = before.pending_attempt.unwrap();
        assert_eq!(before.evidence.samples(), 0);
        assert!(case.host.machine.native_learned_convening_supported());
        let canonical = bytes(&case);
        assert_eq!(
            case.host.machine.check_native_learned_convening(attempt),
            Err(Error::Incomplete)
        );
        assert_eq!(case.host.action_consistency_snapshot().unwrap(), before);
        assert_eq!(bytes(&case), canonical);
        assert_eq!(submit(&mut case.host), attempt);
        assert_eq!(
            case.host.machine.check_native_learned_convening(attempt),
            Ok(())
        );
        assert_eq!(
            case.host
                .machine
                .check_native_learned_convening(attempt + 1),
            Err(Error::Incomplete)
        );
    }
}

#[test]
fn fixed_successor_rechecks_original_coverage_immediately_before_native_begin() {
    for owned in [false, true] {
        for lose_coverage in [false, true] {
            let (mut case, mut run, attempt) = held(owned, false);
            assert_eq!(
                run.check_held_successor(&case.host, ElapsedTick(1))
                    .unwrap(),
                round(NEXT)
            );
            let reservation = run.reservation();
            let polls = run.polls();
            let prior = run.records()[&FIRST].clone();
            if lose_coverage {
                let revision = case.host.revision();
                case.observer.unavailable(&mut case.host, revision).unwrap();
            }
            let original = forecast_state(&case.host, attempt);
            let numerical = case.host.learned_generation_inspection().unwrap().numerical;
            let canonical = bytes(&case);
            let revision = case.host.revision();
            let selected = run.revision();
            let result = run.convene_held(&mut case.host, selected, ElapsedTick(1), snapshot());
            if lose_coverage {
                assert_eq!(result, Err(Error::Incomplete.into()));
                assert_eq!(bytes(&case), canonical);
                assert!(!run.records()[&NEXT].started);
                assert!(
                    run.check_held_successor(&case.host, ElapsedTick(1))
                        .is_err()
                );
            } else {
                assert_eq!(result, Ok(NativeReviewStatus::Running));
                assert_eq!(case.host.revision(), revision + 1);
                assert_eq!(run.records()[&NEXT].polls, 0);
                assert!(run.records()[&NEXT].started);
            }
            assert_eq!(run.reservation(), reservation);
            assert_eq!(run.polls(), polls);
            assert_eq!(run.records()[&FIRST], prior);
            assert_eq!(forecast_state(&case.host, attempt), original);
            assert_eq!(
                case.host.learned_generation_inspection().unwrap().numerical,
                numerical
            );
            assert_eq!(case.host.inspect().executions, 0);
        }
    }
}

#[test]
fn reserved_successor_rechecks_after_consuming_owners_without_refunding_or_reviving_them() {
    for owned in [false, true] {
        for lose_coverage in [false, true] {
            let (mut case, mut run, attempt) = held(owned, true);
            let (basis, blocked) = case.host.native_convene_status(REQUEST).unwrap();
            assert_eq!(blocked, None);
            assert_eq!(run.pending_convene_rosters(), 2);
            let reservation = run.reservation();
            let polls = run.polls();
            let prior = run.records()[&FIRST].clone();
            let selected = run
                .take_reserved_successor(&case.host, &basis, NEXT)
                .unwrap();
            assert_eq!(run.pending_convene_rosters(), 0);
            assert!(!run.reserved_successor_available());
            // This is the actual final-boundary state after the paid owners
            // leave the sequence. No public host or observer escape is added.
            if lose_coverage {
                let revision = case.host.revision();
                case.observer.unavailable(&mut case.host, revision).unwrap();
            }
            let original = forecast_state(&case.host, attempt);
            let numerical = case.host.learned_generation_inspection().unwrap().numerical;
            let canonical = bytes(&case);
            let revision = case.host.revision();
            let result =
                run.begin_reserved_successor(&mut case.host, selected, round(NEXT), snapshot());
            if lose_coverage {
                assert_eq!(result, Err(Error::Incomplete.into()));
                assert_eq!(bytes(&case), canonical);
                assert_eq!(run.status(), NativeReviewStatus::Failed);
                assert_eq!(run.pending_convene_rosters(), 0);
                assert!(!run.records()[&NEXT].started);
                assert!(
                    run.take_reserved_successor(&case.host, &basis, NEXT + 1)
                        .is_err()
                );
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(case.host.revision(), revision + 1);
                assert_eq!(run.status(), NativeReviewStatus::Running);
                assert_eq!(run.pending_convene_rosters(), 1);
                assert_eq!(run.records()[&NEXT].polls, 0);
                assert!(run.records()[&NEXT].started);
            }
            assert_eq!(run.reservation(), reservation);
            assert_eq!(run.polls(), polls);
            assert_eq!(run.records()[&FIRST], prior);
            assert_eq!(forecast_state(&case.host, attempt), original);
            assert_eq!(
                case.host.learned_generation_inspection().unwrap().numerical,
                numerical
            );
            assert_eq!(case.host.inspect().executions, 0);
        }
    }
}
