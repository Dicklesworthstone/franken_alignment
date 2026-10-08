//! Actual native congress, original stream receipts and deterministic Store faults.
//! Probabilities and helper weights are synthetic controls, never imported votes.
use super::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    decoder::learned::FileLearnedStepPreparationStatus,
    driver::{FileSupervisedDriver, FileDriverEvent, FileDriverPhase,
        native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}};
use crate::action::consequence::delivery::persistent::requests::actor::LearnedTextRelease;
use crate::action::consequence::delivery::stream::ReleaseFrame;
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::oversight::{ReviewWindow, actor::{ActorOutcome, Knowledge},
    decoder_monitoring::LearnedDecoderBindingLimits,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarDecisionBasis, SidecarProbeQuery}}};
use std::collections::BTreeSet;
#[path = "../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn fresh(root: &Directory, crossing: bool, stream: Option<StreamProfile>)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver, FileLearnedConfig,
        FileLearnedConsistencyConfig, FileOversightProfile)
{
    let mut profile = profile();
    let generation = match stream {
        Some(stream) => {
            let (model, tokenizer, source) = recipe();
            profile.delivery.initial_payload.clear(); profile.delivery.total = 1000;
            FileLearnedConfig::new_text_stream(model, tokenizer, source,
                LearnedDecoderBindingLimits::default(), stream).unwrap().with_required_sidecar().unwrap()
        }
        None => config(),
    };
    let pair = if crossing { BinaryForecast::new(8192, 57344).unwrap() }
        else { BinaryForecast::new(32768, 32768).unwrap() };
    let predictor = prediction_config(true, stream, MAX_CHECKED_KV_BYTES, pair);
    let (mut host, human) = match stream {
        Some(_) => FileOversight::create_with_learned_text_stream(root.store(), profile.clone(), generation.clone()),
        None => FileOversight::create_with_learned_text(root.store(), profile.clone(), generation.clone()),
    }.unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let observer = host.enable_learned_action_consistency(host.revision(), predictor.clone()).unwrap();
    step(&mut host);
    (host, human, observer, generation, predictor, profile)
}
fn generate(host: &mut FileOversight) {
    step(host); step(host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
}
fn review(mut driver: FileSupervisedDriver, request: u64, round: u64, spelling: &[u8])
    -> FileNativeSupervisedDriver
{
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("original request must be admitted before congress"); };
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
                SidecarProbeQuery { row: *row, probe: LinearProbe::new(1, 1, frame.profile,
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
        other => panic!("expected original publication, got {other:?}"),
    }
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer, key: u64) -> FileHumanPermit {
    let request = run.request_human_approval(key, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn required_kv_forecast_still_needs_actual_native_allow_and_the_independent_human_key() {
    for (crossing, spelling) in [(false, b"allow".as_slice()), (false, b"deny".as_slice()), (true, b"allow".as_slice())] {
        let root = Directory::new();
        let (mut host, human, role, generation, predictor, profile) = fresh(&root, crossing, None);
        forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap(); generate(&mut host);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
        assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.crossed(), crossing);
        if crossing {
            let host = supervisor.host().unwrap();
            assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
            assert_eq!(host.inspect().executions, 0); continue;
        }
        let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, spelling);
        if spelling == b"deny" {
            assert_eq!(run.progress().phase, FileDriverPhase::Idle);
            assert!(run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).is_err());
            assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0); continue;
        }
        assert!(matches!(effect(&mut run, None), FileDriverEvent::AwaitingHuman { request: 71 }));
        assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
        let permit = approve(&mut run, &human, 1001);
        assert!(matches!(effect(&mut run, Some(&permit)), FileDriverEvent::Dispatched { request: 71, .. }));
        assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
        assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(run.supervisor().host().unwrap().inspect().payload, b"aa");
        let before = run.supervisor().host().unwrap().learned_action_consistency_snapshot().unwrap();
        drop(run);
        let (reopened, _, _) = FileOversight::open_with_owned_learned_consistency(
            root.store(), profile, &generation, &predictor).unwrap();
        assert_eq!(reopened.inspect().executions, 1);
        assert_eq!(reopened.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(reopened.learned_action_consistency_snapshot().unwrap().work, before.work);
        assert_eq!(reopened.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    }
}

fn reject_early_finish(run: &mut FileNativeSupervisedDriver, role: &FileConsistencyObserver) {
    let mut host = run.supervisor_mut().host_mut().unwrap();
    let before = host.action_consistency_snapshot().unwrap(); let bytes = canonical(&host);
    assert!(forecast(&mut host, role, 72).is_err());
    assert_eq!(host.action_consistency_snapshot().unwrap(), before); assert_eq!(canonical(&host), bytes);
}
#[test]
fn stream_finish_requires_receipt_confirmation_a_later_kv_forecast_and_new_review_keys() {
    let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let (mut host, human, role, generation, predictor, profile) = fresh(&root, false, Some(stream));
    forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap(); generate(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let message = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let message_spec = supervisor.host().unwrap().request_action(71).unwrap().spec().clone();
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
    reject_early_finish(&mut run, &role);
    let first_key = approve(&mut run, &human, 1001);
    assert!(matches!(effect(&mut run, Some(&first_key)), FileDriverEvent::Dispatched { .. }));
    reject_early_finish(&mut run, &role);
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(run.supervisor().host().unwrap().stream_snapshot().unwrap().confirmed.visible().is_empty());
    reject_early_finish(&mut run, &role);
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&message), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        forecast(&mut host, &role, 72).unwrap().unwrap().prediction().unwrap();
        let deadline = host.machine.broker.consistency_deadline().unwrap().unwrap();
        assert_eq!(deadline.source_sequence, numerical.position);
        let before = host.action_consistency_snapshot().unwrap(); let bytes = canonical(&host); let revision = host.revision();
        assert_eq!(host.submit_request(revision, 72, message_spec.clone(), snapshot()).err(), Some(Error::Binding.into()));
        assert_eq!(host.action_consistency_snapshot().unwrap(), before); assert_eq!(canonical(&host), bytes);
    }
    observe(driver.supervisor_mut(), snapshot()); let finish = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let finish_cost = {
        let host = driver.supervisor().host().unwrap(); let spec = host.request_action(72).unwrap().spec();
        assert!(ReleaseFrame::decode(&spec.payload).unwrap().is_finish());
        assert!(!host.learned_action_consistency_observation(2).unwrap().event()); spec.units
    };
    let mut run = review(driver, 72, 201, b"allow");
    assert!(matches!(effect(&mut run, None), FileDriverEvent::AwaitingHuman { .. }));
    assert!(run.step(|| ElapsedTick(1), || Ok(snapshot()), Some(&first_key)).is_err());
    let finish_key = approve(&mut run, &human, 1002);
    assert!(matches!(effect(&mut run, Some(&finish_key)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&finish), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    {
        let host = run.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
        assert!(view.confirmed.finished()); assert_eq!(view.confirmed.visible(), b"aa");
        assert_eq!(view.publication.executions, 2);
        assert_eq!(view.publication.control.ledger.charged, message_spec.units + finish_cost);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    }
    drop(run);
    let (recovered, _, _) = FileOversight::open_with_owned_learned_consistency(
        root.store(), profile, &generation, &predictor).unwrap();
    assert!(recovered.stream_snapshot().unwrap().confirmed.finished());
    assert_eq!(recovered.inspect().executions, 2);
    assert_eq!(recovered.action_consistency_snapshot().unwrap().evidence.samples(), 2);
}

#[test]
fn recovery_rejects_a_weaker_timing_config_and_settles_unknown_effects_without_generation() {
    let root = Directory::new();
    let (mut host, human, role, generation, required, profile) = fresh(&root, false, None);
    forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap(); generate(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
    let human_key = approve(&mut run, &human, 1001);
    assert!(matches!(effect(&mut run, Some(&human_key)), FileDriverEvent::Dispatched { .. }));
    let bytes = canonical(&run.supervisor().host().unwrap());
    assert!(FileOversight::read_publication_with_owned_learned_consistency(
        root.store(), &profile, &generation, &predictor(false)).is_err());
    assert_eq!(canonical(&run.supervisor().host().unwrap()), bytes);
    drop(run); assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    assert!(FileOversight::open_with_owned_learned_consistency(
        root.store(), profile.clone(), &generation, &predictor(false)).is_err());
    let (host, _, _) = FileOversight::open_with_owned_learned_consistency(
        root.store(), profile, &generation, &required).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    let (port, supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let ticket = port.submit(71, proposal()).unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); driver.resume_reconciliation(71).unwrap();
    assert!(matches!(driver.step_with_evidence(|| ElapsedTick(2),
        |_, _| panic!("settlement must not read evidence or generate output"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 0);
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
}

#[test]
fn all_three_write_boundaries_return_no_speculative_sample_and_retain_canonical_forecasts() {
    for stage in 0..3 {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let required = predictor(true);
            let (mut host, _, role, generation) = setup(&root, &required); step(&mut host);
            if stage > 0 { forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap(); }
            let n = host.learned_generation_inspection().unwrap().numerical;
            if stage == 2 { host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap(); }
            let before = host.inspect(); host.store.fail_once(barrier);
            let failed = match stage {
                0 => forecast(&mut host, &role, 71).err(),
                1 => host.begin_learned_step(host.revision(), n.actor_revision, n.position).err(),
                _ => host.complete_learned_step(host.revision(), n.actor_revision, n.position).err(),
            };
            assert!(matches!(failed, Some(JournalError::Io(ref failure)) if failure.operation == barrier));
            assert_eq!(host.inspect(), before); assert!(host.learned_generation_inspection().is_err());
            let read = replay(&host, &generation, &required);
            let visible = barrier == JournalIo::DirectorySync;
            let count = u64::from(stage == 2 && visible);
            assert_eq!(read.broker.hosted_learned_generation().unwrap().work.sampling_attempts, count);
            let had_forecast = stage > 0 || visible;
            assert_eq!(read.consistency_request.map(|(request, _)| request), had_forecast.then_some(71));
            drop(host);
            let (mut recovered, _, observer) = FileOversight::open_with_owned_learned_consistency(
                root.store(), profile(), &generation, &required).unwrap();
            assert_eq!(recovered.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, count);
            assert_eq!(recovered.action_consistency_snapshot().unwrap().coverage_lost, had_forecast);
            assert!(recovered.learned_generation_inspection().unwrap().paused);
            assert!(forecast(&mut recovered, &observer, 72).is_err());
            assert_eq!(recovered.inspect().control.ledger.available, 100);
            assert_eq!(recovered.inspect().executions, 0);
        }
    }
}

#[test]
fn cooperative_catch_up_cannot_extend_forecast_validity_but_can_finish_before_expiry() {
    for tick in [10, 11] {
        let root = Directory::new(); let required = predictor(true);
        let (mut host, _, role, _) = setup(&root, &required); step(&mut host);
        forecast(&mut host, &role, 71).unwrap().unwrap().prediction().unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
        let mut completion = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
        completion.advance(&host, 0, 1).unwrap();
        host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
        let done = completion.progress().replayed_events;
        let result = completion.catch_up(&host, host.revision(), done, 1);
        if tick == 11 {
            assert!(result.is_err());
            assert!(completion.finish(&mut host).is_err());
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 0);
            assert!(host.learned_generation_inspection().unwrap().pending.is_some());
        } else {
            result.unwrap();
            while completion.progress().status == FileLearnedStepPreparationStatus::Replaying {
                let done = completion.progress().replayed_events;
                completion.advance(&host, done, 1).unwrap();
            }
            completion.finish(&mut host).unwrap().unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.sampling_attempts, 1);
            assert_eq!(host.machine.broker.consistency_deadline().unwrap().unwrap().expires_at, ElapsedTick(11));
        }
    }
}
