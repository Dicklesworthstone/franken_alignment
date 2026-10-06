//! Actual native congress, stream receipts and storage faults under required timing.
//! Reuses original synthetic model fixtures, never supplied worker verdicts.
use super::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    decoder::learned::FileLearnedStepPreparationStatus,
    driver::{FileSupervisedDriver, FileDriverEvent, FileDriverPhase,
        native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}};
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::oversight::{ReviewWindow, actor::{ActorOutcome, Knowledge},
    decoder_monitoring::LearnedDecoderBindingLimits,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::SidecarDecisionBasis}};
use crate::action::consequence::delivery::persistent::requests::actor::LearnedTextRelease;
use std::collections::BTreeSet;
#[path = "../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn fresh(root: &Directory, crossing: bool, stream: Option<StreamProfile>)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver, FileLearnedConfig, FileOversightProfile)
{
    let mut profile = profile();
    let (model, tokenizer, source) = recipe();
    let learned = match stream {
        Some(stream) => {
            profile.delivery.initial_payload.clear(); profile.delivery.total = 1000;
            FileLearnedConfig::new_text_stream(model.clone(), tokenizer, source,
                LearnedDecoderBindingLimits::default(), stream).unwrap().with_required_sidecar().unwrap()
        }
        None => config(),
    };
    let pair = if crossing { BinaryForecast::new(16_384, 49_152).unwrap() }
        else { BinaryForecast::new(32_768, 32_768).unwrap() };
    let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: model.residual_contract(1).unwrap().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap();
    let prediction = match stream { Some(p) => prediction.with_stream_messages(p).unwrap(), None => prediction };
    let (mut host, reviewer) = match stream {
        Some(_) => FileOversight::create_with_learned_text_stream(root.store(), profile.clone(), learned.clone()),
        None => FileOversight::create_with_learned_text(root.store(), profile.clone(), learned.clone()),
    }.unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let role = host.enable_action_consistency(host.revision(), prediction).unwrap();
    step(&mut host);
    (host, reviewer, role, learned, profile)
}
fn generate(host: &mut FileOversight) {
    step(host); step(host); // ordinary API, deliberately not the optional handle
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
}
fn review(mut driver: FileSupervisedDriver, request: u64, round: u64, spelling: &[u8])
    -> FileNativeSupervisedDriver
{
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("the original request must be admitted before congress"); };
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
                view.actual_input().input_profile().clone(), spelling), queries, salt: vec![16 + index as u8; 32] })
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
    assert!(run.review().records().values().flat_map(|round| round.members.values().filter_map(Option::as_ref))
        .any(|record| record.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
    run
}
fn effect(run: &mut FileNativeSupervisedDriver, human: Option<&FileHumanPermit>) -> FileDriverEvent {
    match run.step(|| ElapsedTick(1), || Ok(snapshot()), human).unwrap() {
        FileNativeDriverEvent::Driver(event) => event,
        other => panic!("expected original publication state machine, got {other:?}"),
    }
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer, key: u64) -> FileHumanPermit {
    let request = run.request_human_approval(key, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn required_forecast_never_substitutes_for_native_model_judgment_or_the_human_key() {
    for (crossing, spelling) in [(false, b"allow".as_slice()), (false, b"deny".as_slice()), (true, b"allow".as_slice())] {
        let root = Directory::new(); let (mut host, reviewer, role, config, profile) = fresh(&root, crossing, None);
        forecast(&mut host, &role, 71); generate(&mut host);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
        assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.crossed(), crossing);
        if crossing {
            assert!(matches!(supervisor.host().unwrap().request_status(71).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
            assert_eq!(supervisor.host().unwrap().inspect().executions, 0); continue;
        }
        let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, spelling);
        if spelling == b"deny" {
            assert_eq!(run.progress().phase, FileDriverPhase::Idle);
            assert!(run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).is_err());
            assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0); continue;
        }
        assert!(matches!(effect(&mut run, None), FileDriverEvent::AwaitingHuman { request: 71 }));
        assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
        let human = approve(&mut run, &reviewer, 1001);
        assert!(matches!(effect(&mut run, Some(&human)), FileDriverEvent::Dispatched { request: 71, .. }));
        assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
        assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(run.supervisor().host().unwrap().inspect().payload, b"aa");
        drop(run);
        let (reopened, _) = FileOversight::open_with_learned_generation(root.store(), profile, &config).unwrap();
        assert!(reopened.machine.consistency.as_ref().unwrap().requires_pre_output_forecast());
        assert_eq!(reopened.inspect().executions, 1);
        assert_eq!(reopened.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    }
}

fn reject_early_finish(run: &mut FileNativeSupervisedDriver, role: &FileConsistencyObserver) {
    let mut host = run.supervisor_mut().host_mut().unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    let before = host.action_consistency_snapshot().unwrap(); let bytes = disk(&host);
    assert!(role.forecast_hosted_request(&mut host, revision, 72, n.actor_revision).is_err());
    assert_eq!(host.action_consistency_snapshot().unwrap(), before); assert_eq!(disk(&host), bytes);
}
#[test]
fn required_stream_finish_waits_for_receipt_then_uses_a_new_forecast_review_and_human_key() {
    let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let (mut host, reviewer, role, _, _) = fresh(&root, false, Some(stream));
    forecast(&mut host, &role, 71); generate(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let message = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let message_spec = supervisor.host().unwrap().request_action(71).unwrap().spec().clone();
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
    reject_early_finish(&mut run, &role);
    let human = approve(&mut run, &reviewer, 1001);
    assert!(matches!(effect(&mut run, Some(&human)), FileDriverEvent::Dispatched { .. }));
    reject_early_finish(&mut run, &role);
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(run.supervisor().host().unwrap().stream_snapshot().unwrap().confirmed.visible().is_empty());
    reject_early_finish(&mut run, &role);
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&message), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); forecast(&mut host, &role, 72);
        let pending = host.machine.broker.consistency_deadline().unwrap().unwrap();
        assert_eq!(pending.source_sequence, numerical.position);
        let before = host.action_consistency_snapshot().unwrap(); let bytes = disk(&host);
        let revision = host.revision();
        assert_eq!(host.submit_request(revision, 72, message_spec.clone(), snapshot()).err(), Some(Error::Binding.into()));
        assert_eq!(host.action_consistency_snapshot().unwrap(), before); assert_eq!(disk(&host), bytes);
    }
    observe(driver.supervisor_mut(), snapshot()); let finish = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let finish_cost = {
        let host = driver.supervisor().host().unwrap(); let spec = host.request_action(72).unwrap().spec();
        assert!(ReleaseFrame::decode(&spec.payload).unwrap().is_finish());
        assert!(!host.action_consistency_observation(2).unwrap().event()); spec.units
    };
    let mut run = review(driver, 72, 201, b"allow");
    assert!(matches!(effect(&mut run, None), FileDriverEvent::AwaitingHuman { .. }));
    assert!(run.step(|| ElapsedTick(1), || Ok(snapshot()), Some(&human)).is_err());
    let finish_human = approve(&mut run, &reviewer, 1002);
    assert!(matches!(effect(&mut run, Some(&finish_human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&finish), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let host = run.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
    assert!(view.confirmed.finished()); assert_eq!(view.confirmed.visible(), b"aa");
    assert_eq!(view.publication.executions, 2);
    assert_eq!(view.publication.control.ledger.charged, message_spec.units + finish_cost);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
}

#[test]
fn unknown_dispatch_recovery_needs_neither_a_replacement_forecast_nor_generation_resume() {
    let root = Directory::new(); let (mut host, reviewer, role, config, profile) = fresh(&root, false, None);
    forecast(&mut host, &role, 71); generate(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
    let human = approve(&mut run, &reviewer, 1001);
    assert!(matches!(effect(&mut run, Some(&human)), FileDriverEvent::Dispatched { .. }));
    drop(run); assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile, &config).unwrap();
    assert!(host.machine.consistency.as_ref().unwrap().requires_pre_output_forecast());
    let (port, supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let ticket = port.submit(71, proposal()).unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); driver.resume_reconciliation(71).unwrap();
    assert!(matches!(driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("query must not capture evidence"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    let host = driver.supervisor().host().unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 84);
}

#[test]
fn all_forecast_and_both_numerical_write_barriers_recover_without_bypassing_required_timing() {
    for stage in 0..3 {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let (mut host, _, role, config, profile) = fresh(&root, false, None);
            if stage > 0 { forecast(&mut host, &role, 71); }
            let n = host.learned_generation_inspection().unwrap().numerical;
            if stage == 2 { host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap(); }
            let before = host.inspect(); let revision = host.revision(); host.store.fail_once(barrier);
            let result = match stage {
                0 => role.forecast_hosted_request(&mut host, revision, 71, n.actor_revision).map(|_| ()),
                1 => host.begin_learned_step(revision, n.actor_revision, n.position),
                _ => host.complete_learned_step(revision, n.actor_revision, n.position).map(|_| ()),
            };
            assert!(matches!(result, Err(JournalError::Io(failure)) if failure.operation == barrier));
            assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some()); drop(host);
            let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile, &config).unwrap();
            assert!(host.machine.consistency.as_ref().unwrap().requires_pre_output_forecast());
            assert!(host.learned_generation_inspection().unwrap().paused);
            let n = host.learned_generation_inspection().unwrap().numerical;
            assert!(n.work.sampling_attempts <= 1); assert_eq!(host.inspect().executions, 0);
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
            let revision = host.revision();
            assert_eq!(role.forecast_hosted_request(&mut host, revision, 71, n.actor_revision).err(), Some(Error::Binding.into()));
            assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        }
    }
}

#[test]
fn prepared_completion_cannot_catch_up_past_expiry_but_can_finish_before_the_boundary() {
    for tick in [10, 11] {
        let root = Directory::new(); let (mut host, _, role, _, _) = fresh(&root, false, None);
        forecast(&mut host, &role, 71); let n = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
        let mut task = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
        while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
            task.advance(&host, task.progress().replayed_events, 1).unwrap();
        }
        let ready = task.progress(); host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
        let bytes = disk(&host);
        let result = task.catch_up(&host, host.revision(), ready.replayed_events, 1);
        if tick == 10 {
            assert_eq!(result.unwrap().status, FileLearnedStepPreparationStatus::Ready);
            assert!(task.finish(&mut host).unwrap().unwrap().accepted().is_some());
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 1);
        } else {
            assert!(result.is_err()); assert_eq!(task.progress(), ready);
            assert!(task.finish(&mut host).is_err()); assert_eq!(disk(&host), bytes);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
            assert!(host.learned_generation_inspection().unwrap().pending.is_some());
        }
    }
}
