//! Original finite inference, predictor, native congress and receipt lifecycle.
//! Tiny fixed weights and neutral probabilities are controls, not calibration.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanPermit,
    consistency::{FileConsistencyObserver, pre_output::{FileOwnedPreOutputStart, FileOwnedPreOutputForecast}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus},
};
use fa_reference::action::consequence::oversight::{
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget,
        receiver::native::SidecarProbeQuery},
};
#[path = "native.rs"]
mod native;

fn pinned(owned: bool) -> FileLearnedConfig {
    let base = finite_recipe(4, false, false).with_required_sidecar().unwrap();
    Forecast::new(owned).pin(base).unwrap().with_forecast_text_stream_continuation().unwrap()
}
fn start(root: &Directory, owned: bool)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver, FileLearnedConfig) {
    let config = pinned(owned);
    let (mut host, reviewer, observer) = FileOversight::create_with_pre_output_forecast(
        root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host); // ORIGINAL prompt, no sample
    (host, reviewer, observer, config)
}
fn forecast(host: &mut FileOversight, observer: &FileConsistencyObserver, owned: bool, request: u64)
    -> Result<Result<u64, Error>, JournalError> {
    let n = host.learned_generation_inspection()?.numerical;
    let revision = host.revision();
    if owned {
        observer.forecast_owned_learned_request(host, revision, request, n.actor_revision)
            .map(|result| result.and_then(|report| report.prediction()
                .map(|prediction| prediction.observation().frame().sequence)))
    } else {
        observer.forecast_hosted_request(host, revision, request, n.actor_revision)
            .map(|result| result.map(|prediction| prediction.observation().frame().sequence))
    }
}
fn begin_owned(host: &mut FileOversight, observer: &FileConsistencyObserver, request: u64)
    -> Box<FileOwnedPreOutputForecast> {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    match observer.begin_owned_pre_output_request(host, revision, request, n.actor_revision, n.position).unwrap() {
        FileOwnedPreOutputStart::Ready(run) => run,
        other => panic!("expected original complete owned prediction, got {other:?}"),
    }
}
fn generate_with_forecast(host: &mut FileOversight, observer: &FileConsistencyObserver,
    owned: bool, request: u64) -> u64 {
    let before = host.learned_generation_inspection().unwrap().numerical;
    if owned {
        let mut run = begin_owned(host, observer, request);
        assert_eq!(run.request(), request);
        assert_eq!(run.numerical(), &before);
        assert!(run.forecast_report().prediction().is_ok());
        assert_eq!(run.prediction().observation().frame().sequence, before.position);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        while run.numerical().status.is_active() {
            let revision = host.revision(); run.advance(host, revision).unwrap().unwrap();
        }
    } else {
        let revision = host.revision();
        let mut run = observer.begin_pre_output_request(host, revision, request,
            before.actor_revision, before.position).unwrap().unwrap();
        while run.numerical().status.is_active() {
            let revision = host.revision(); run.advance(host, revision).unwrap().unwrap();
        }
    }
    before.position
}
fn no_sample(host: &mut FileOversight, root: &Directory) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision(); let files = root.files();
    assert!(host.begin_learned_step(revision, n.actor_revision, n.position).is_err());
    assert!(host.prepare_learned_step_intent(revision, n.actor_revision, n.position).is_err());
    assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
    assert_eq!(host.revision(), revision); assert_eq!(root.files(), files);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
}

// Return genuine newly earned keys after the ORIGINAL native helper has read
// the complete sidecar input and produced its own committed/revealed verdict.
fn reviewed(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64,
    text: Option<&str>) -> (u64, FrozenAction, CommitteeInput, FilePermit, FileHumanPermit) {
    let (attempt, action) = super::super::request(host, request, text);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let priority = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap()
        .evidence().audit().source().groups().collect();
    let sidecar = host.begin_learned_sidecar_plan(host.revision(), attempt, n.actor_revision,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + request,
            generation: 1, transform_id: 7 }, priority, budget: SidecarCongressBudget::default() }).unwrap();
    let original = host.checked_learned_sidecar(&sidecar).unwrap();
    let rows: BTreeSet<_> = original.source().groups().map(|group| group.row).collect();
    let members = original.round().input().views().iter().enumerate().map(|(index, (name, view))| {
        let queries = rows.iter().map(|row| {
            let (frame, heads, channels) = original.source().row_shape(*row).unwrap();
            SidecarProbeQuery { row: *row, probe: LinearProbe::new(1, 1, frame.profile,
                &vec![0.0; heads * channels], 0.0, 1.0).unwrap() }
        }).collect();
        (name.clone(), NativeReviewMember {
            evaluator: native::evaluator(view.actual_input().input_profile().clone()),
            queries, salt: vec![16 + index as u8; 32],
        })
    }).collect();
    let mut review = host.begin_native_sidecar_review(host.revision(), sidecar,
        LearnedWorkerRound { round: 100 + request, evidence_root: [7; 32],
            window: ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } },
        members, NativeReviewLimits::default(), snapshot()).unwrap();
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if review.status() != NativeReviewStatus::Running { break; }
        review.advance(host, review.revision(), ElapsedTick(1), snapshot()).unwrap();
    }
    assert_eq!(review.status(), NativeReviewStatus::Finished);
    assert!(review.outcome().is_some());
    assert!(review.records().values().all(|record| record.failure.is_none()
        && record.commitment_queued && record.reveal_queued));
    let inputs = review.input().clone();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let offer = host.request_human_approval(host.revision(), 1000 + attempt,
        attempt, &inputs, ElapsedTick(40)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &offer).unwrap();
    (attempt, action, inputs, automatic, human)
}
fn effect(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64, text: Option<&str>) -> u64 {
    let (attempt, action, inputs, automatic, human) = reviewed(host, reviewer, request, text);
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    publish(host, attempt, &inputs, 1);
    host.reconcile(host.revision(), attempt).unwrap();
    action.spec().units
}
fn first(host: &mut FileOversight, reviewer: &FileHumanReviewer, observer: &FileConsistencyObserver, owned: bool) {
    assert_eq!(forecast(host, observer, owned, 71).unwrap().unwrap(), 1);
    generate(host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"A");
    effect(host, reviewer, 71, Some("A"));
}

#[test]
fn both_predictors_forecast_each_original_message_then_native_review_both_keys_and_finish() {
    for owned in [false, true] {
        let root = Directory::new(); let (mut host, reviewer, observer, config) = start(&root, owned);
        assert!(config.requires_forecast_text_stream_continuation());
        no_sample(&mut host, &root);
        assert_eq!(generate_with_forecast(&mut host, &observer, owned, 71), 1);
        effect(&mut host, &reviewer, 71, Some("A"));
        let first = host.learned_generation_inspection().unwrap().numerical;
        let charged = host.inspect().control.ledger.charged;
        continue_message(&mut host, 71, 72);
        let boundary = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(boundary.position, first.position); assert_eq!(boundary.actor_revision, first.actor_revision);
        assert_eq!(boundary.work, first.work); assert_eq!(boundary.cumulative_work, first.cumulative_work);
        assert_eq!(boundary.sampled_draws, first.sampled_draws);
        no_sample(&mut host, &root);
        assert_eq!(generate_with_forecast(&mut host, &observer, owned, 72), first.position);
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"B");
        let second_cost = effect(&mut host, &reviewer, 72, Some("B"));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.sampled_draws, 4);
        // A control uses the same original numerical model without the first
        // stop boundary. No supplied transcript or saved state is an oracle.
        let model = finite_model(); let mut source = finite_text(&model, 4, false);
        source.stop_tokens = BTreeSet::from([OTHER_CONTROL]);
        let mut control = model.observed_learned_text_generation(tokenizer(&model), source).unwrap();
        control.run_to_stop().unwrap();
        assert_eq!(numerical.work, control.work()); assert_eq!(numerical.telemetry, control.telemetry_work());
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().evidence().tokens(), control.accepted_tokens());
        assert_eq!(forecast(&mut host, &observer, owned, 73).unwrap().unwrap(), numerical.position);
        let finish_cost = effect(&mut host, &reviewer, 73, None);
        let completed = host.stream_snapshot().unwrap();
        assert_eq!(completed.confirmed, completed.published); assert!(completed.confirmed.finished());
        assert_eq!(completed.confirmed.messages().collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(host.inspect().control.ledger.charged, charged + second_cost + finish_cost);
        assert_eq!(host.inspect().executions, 3);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        assert_eq!(evidence.samples(), 3); assert!(!evidence.crossed());
        drop(host);
        let (host, _) = reopen(&root, &config);
        assert_eq!(host.stream_snapshot().unwrap().confirmed, completed.confirmed);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    }
}

#[test]
fn successor_forecast_is_bound_to_request_and_cannot_be_obtained_after_a_new_sample() {
    for owned in [false, true] {
        let root = Directory::new(); let (mut host, reviewer, observer, _) = start(&root, owned);
        first(&mut host, &reviewer, &observer, owned); continue_message(&mut host, 71, 72);
        let bytes = root.files(); let before = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(forecast(&mut host, &observer, owned, 73).err(), Some(Error::Binding.into()));
        assert_eq!(root.files(), bytes); assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        forecast(&mut host, &observer, owned, 72).unwrap().unwrap();
        step(&mut host);
        let bytes = root.files(); let before = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(forecast(&mut host, &observer, owned, 72).err(), Some(Error::WrongState.into()));
        assert_eq!(root.files(), bytes); assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        generate(&mut host); effect(&mut host, &reviewer, 72, Some("B"));
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
    }
}

#[test]
fn pending_finish_cannot_be_reclassified_as_a_message_by_continuation() {
    for owned in [false, true] {
        let root = Directory::new(); let (mut host, reviewer, observer, _) = start(&root, owned);
        first(&mut host, &reviewer, &observer, owned);
        let command = host.prepare_learned_text_stream_continuation(71, 72).unwrap();
        forecast(&mut host, &observer, owned, 72).unwrap().unwrap(); // FINISH, not a new Message
        let before = host.learned_generation_inspection().unwrap().numerical; let bytes = root.files();
        assert!(host.begin_learned_text_stream_continuation(host.revision(), command).is_err());
        assert_eq!(root.files(), bytes); assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        assert_eq!(host.pending_forecast_request().unwrap(), Some(72));
        effect(&mut host, &reviewer, 72, None);
        assert!(host.stream_snapshot().unwrap().confirmed.finished());
        assert_eq!(host.inspect().executions, 2);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
    }
}

#[test]
fn missing_new_forecast_after_recovery_preserves_original_message_boundary_and_spent_work() {
    for owned in [false, true] {
        let root = Directory::new(); let (mut host, reviewer, observer, config) = start(&root, owned);
        first(&mut host, &reviewer, &observer, owned); continue_message(&mut host, 71, 72);
        let before = host.learned_generation_inspection().unwrap().numerical;
        let command = host.current_learned_text_continuation().unwrap(); let evidence = host.action_consistency_snapshot().unwrap().evidence;
        drop(host);
        let (mut recovered, _) = reopen(&root, &config);
        assert_eq!(recovered.learned_generation_inspection().unwrap().numerical, before);
        assert_eq!(recovered.current_learned_text_continuation().unwrap(), command);
        assert_eq!(recovered.action_consistency_snapshot().unwrap().evidence, evidence);
        resume(&mut recovered, 2);
        no_sample(&mut recovered, &root);
        // Old observer custody cannot issue a replacement forecast on a new owner.
        assert_eq!(forecast(&mut recovered, &observer, owned, 72).err(), Some(Error::Binding.into()));
        assert_eq!(recovered.inspect().executions, 1);
        assert_eq!(recovered.learned_generation_inspection().unwrap().numerical.work, before.work);
    }
}
