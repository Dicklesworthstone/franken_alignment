//! Original ByteBpe output, native helper models, actor tickets and both keys.
//! Probabilities and native response weights are causal controls, not calibration.
use super::FilePreOutputForecast;
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, FileHumanReviewer, FileHumanPermit, decoder::learned::FileLearnedConfig,
    consistency::{FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver},
    driver::{FileSupervisedDriver, FileDriverEvent, native_learned::{FileNativeDriverLaunch,
        FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}};
use crate::action::consequence::delivery::persistent::requests::{FileRequestDisposition,
    actor::{FileActorSupervisor, LearnedTextProposal, LearnedTextRelease}};
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::oversight::{ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits, learned_source::LearnedEvidenceLimits,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget}, actor::{ActorOutcome, Knowledge}};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::{Snapshot, Error};
use std::collections::BTreeSet;
#[path = "../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn predictor(crossing: bool, stream: Option<StreamProfile>) -> FileConsistencyConfig {
    let (model, _, _) = recipe(); let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let config = FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model.residual_contract(1).unwrap().profile(), weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral,
            positive: if crossing { BinaryForecast::new(16_384, 49_152).unwrap() } else { neutral } },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap();
    match stream { Some(stream) => config.with_stream_messages(stream).unwrap(), None => config }
}
fn start(root: &Directory, crossing: bool, stream: Option<StreamProfile>)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver, FilePreOutputForecast,
        FileLearnedConfig, FileOversightProfile)
{
    let mut profile = profile();
    let config = match stream {
        Some(stream) => {
            profile.delivery.initial_payload.clear(); profile.delivery.total = 1000;
            let (model, tokenizer, source) = recipe();
            FileLearnedConfig::new_text_stream(model, tokenizer, source,
                LearnedDecoderBindingLimits::default(), stream).unwrap().with_required_sidecar().unwrap()
        }
        None => config(),
    };
    let (mut host, reviewer) = match stream {
        Some(_) => FileOversight::create_with_learned_text_stream(root.store(), profile.clone(), config.clone()),
        None => FileOversight::create_with_learned_text(root.store(), profile.clone(), config.clone()),
    }.unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let role = host.enable_action_consistency(host.revision(), predictor(crossing, stream)).unwrap();
    step(&mut host); // the actual tokenizer's one-token prompt, not generated output
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    let run = role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).unwrap().unwrap();
    assert_eq!(run.numerical().cumulative_work.sampling_attempts, 0);
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
    (host, reviewer, role, run, config, profile)
}
fn generate(run: &mut FilePreOutputForecast, host: &mut FileOversight) {
    while run.numerical().status.is_active() {
        let revision = host.revision(); run.advance(host, revision).unwrap().unwrap();
    }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
}

#[test]
fn prompt_forecast_scores_only_later_source_derived_output_once_with_crossing_and_permitting_controls() {
    for crossing in [false, true] {
        let root = Directory::new(); let (host, _, _, mut run, config, profile) = start(&root, crossing, None);
        let deadline = run.deadline(); let prediction = run.prediction().clone();
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot());
        assert!(port.submit(71, proposal()).is_err(), "a forecast is not complete output");
        assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
        { let mut host = supervisor.host_mut().unwrap(); generate(&mut run, &mut host); }
        observe(&mut supervisor, snapshot());
        let ticket = port.submit(71, proposal()).unwrap();
        let evidence = {
            let host = supervisor.host().unwrap();
            let observed = host.action_consistency_observation(1).unwrap();
            assert_eq!(observed.prediction(), &prediction);
            assert_eq!(observed.forecast_actor_revision(), deadline.actor_revision);
            assert!(observed.observed_actor_revision() > observed.forecast_actor_revision());
            assert_eq!(observed.prediction().observation().frame().position, 0);
            assert!(observed.event()); assert_eq!(observed.crossed(), crossing);
            assert_eq!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::Admitted { .. }), !crossing);
            host.action_consistency_snapshot().unwrap().evidence
        };
        let outcome = port.poll(&ticket); let retry = port.submit(71, proposal()).unwrap();
        assert_eq!(port.poll(&retry), outcome);
        assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(evidence.samples(), 1); assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
        drop(supervisor);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile, &config).unwrap();
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert!(host.learned_generation_inspection().unwrap().paused);
    }
}

fn review(mut driver: FileSupervisedDriver, request: u64, round: u64) -> FileNativeSupervisedDriver {
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("original request must be admitted"); };
        let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
        let options = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + request,
            generation: 1, transform_id: 7 }, priority: evidence.audit().source().groups().collect(),
            budget: SidecarCongressBudget::default() };
        let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, n.actor_revision, options).unwrap();
        let original = host.checked_learned_sidecar(&sidecar).unwrap();
        let rows: BTreeSet<_> = original.source().groups().map(|group| group.row).collect();
        let roster = original.round().input().views().iter().enumerate().map(|(index, (name, view))| {
            let queries = rows.iter().map(|row| {
                let (frame, heads, channels) = original.source().row_shape(*row).unwrap();
                crate::action::consequence::oversight::sidecar::receiver::native::SidecarProbeQuery {
                    row: *row, probe: LinearProbe::new(1, 1, frame.profile,
                        &vec![0.0; heads * channels], 0.0, 1.0).unwrap() }
            }).collect();
            (name.clone(), NativeReviewMember { evaluator: native_fixture::evaluator(
                view.actual_input().input_profile().clone(), b"allow"), queries, salt: vec![16 + index as u8; 32] })
        }).collect();
        FileNativeDriverLaunch { journal_revision: host.revision(), request, sidecar,
            rounds: vec![LearnedWorkerRound { round, evidence_root: [7; 32],
                window: ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) } }],
            rosters: [(round, roster)].into_iter().collect(), limits: NativeReviewLimits::default() }
    };
    let mut run = driver.start_native_learned_sequence(launch, snapshot(), ElapsedTick(1)).unwrap();
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { break; }
        assert!(matches!(run.step(|| ElapsedTick(1), || Ok(snapshot()), None).unwrap(), FileNativeDriverEvent::Review(_)));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    run
}
fn effect(run: &mut FileNativeSupervisedDriver, human: Option<&FileHumanPermit>) -> FileDriverEvent {
    match run.step(|| ElapsedTick(1), || Ok(snapshot()), human).unwrap() {
        FileNativeDriverEvent::Driver(event) => event,
        other => panic!("expected original effect lifecycle, got {other:?}"),
    }
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer, key: u64) -> FileHumanPermit {
    let request = run.request_human_approval(key, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn pre_output_message_forecast_and_later_finish_forecast_preserve_sequence_floor_and_both_keys() {
    let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let (mut host, reviewer, role, mut predicted, _, _) = start(&root, false, Some(stream));
    let prompt_sequence = predicted.deadline().source_sequence;
    generate(&mut predicted, &mut host); let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    let message = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert!(supervisor.host().unwrap().action_consistency_observation(1).unwrap().event());
    let message_cost = supervisor.host().unwrap().request_action(71).unwrap().spec().units;
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101);
    assert!(matches!(effect(&mut run, None), FileDriverEvent::AwaitingHuman { request: 71 }));
    let human = approve(&mut run, &reviewer, 1001);
    assert!(matches!(effect(&mut run, Some(&human)), FileDriverEvent::Dispatched { request: 71, .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&message), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        // Closing an already generated message is a separate pre-action forecast,
        // not another pre-output continuation. No sequence-floor relaxation.
        assert_eq!(role.begin_pre_output_request(&mut host, revision, 72,
            numerical.actor_revision, numerical.position).err(), Some(Error::WrongState.into()));
        let prediction = role.forecast_hosted_request(&mut host, revision, 72, numerical.actor_revision).unwrap().unwrap();
        assert!(prediction.observation().frame().sequence > prompt_sequence);
    }
    observe(driver.supervisor_mut(), snapshot());
    let finish = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let finish_cost = {
        let host = driver.supervisor().host().unwrap();
        let observation = host.action_consistency_observation(2).unwrap();
        assert!(!observation.event()); assert_eq!(observation.sample(), 2);
        let spec = host.request_action(72).unwrap().spec();
        assert!(ReleaseFrame::decode(&spec.payload).unwrap().is_finish()); spec.units
    };
    let mut run = review(driver, 72, 201);
    assert!(matches!(effect(&mut run, None), FileDriverEvent::AwaitingHuman { request: 72 }));
    assert!(run.step(|| ElapsedTick(1), || Ok(snapshot()), Some(&human)).is_err());
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 1);
    let finish_human = approve(&mut run, &reviewer, 1002);
    assert!(matches!(effect(&mut run, Some(&finish_human)), FileDriverEvent::Dispatched { request: 72, .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&finish), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let host = run.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
    assert!(view.confirmed.finished()); assert_eq!(view.confirmed.visible(), b"aa");
    assert_eq!(view.publication.executions, 2);
    assert_eq!(view.publication.control.ledger.charged, message_cost + finish_cost);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(handoff.review.status(), NativeReviewStatus::Finished);
}
