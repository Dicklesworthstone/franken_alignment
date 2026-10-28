//! Two real native reviews, original durable receipts and explicit stream close.
//! The synthetic model fixtures establish execution paths, not detector quality.
use super::*;
use super::super::finish;
use fa_reference::action::consequence::delivery::{stream::ReleaseFrame,
    persistent::observed::FileHumanPermit};
use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluationStatus;
use std::path::PathBuf;

fn finish_recipe(root: &Root, config: &Config) -> (PathBuf, Loaded) {
    let original = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    let path = root.0.join("finish.json");
    fs::write(&path, original.replace("\"request\":1", "\"request\":2")
        .replace("\"object_id\":1001", "\"object_id\":1002")).unwrap();
    let loaded = recipe::load(&path, config, true).unwrap();
    assert_eq!(loaded.request, 2); assert_eq!(loaded.rounds, [2]);
    (path, loaded)
}

fn native_review(config: &mut Config, loaded: &Loaded, reviewer: &FileHumanReviewer,
    driver: FileSupervisedDriver, deadline: &Deadline) -> (FileSupervisedDriver, FileNativeSidecarSequence)
{
    let mut control = Control::new(config, loaded.request, None).unwrap();
    let reviewed = review::run(driver, config, loaded, reviewer, &mut control,
        deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let history = reviewed.history.unwrap();
    assert_eq!(history.status(), NativeReviewStatus::Finished);
    assert_eq!(native_records(&history).count(), 2);
    for record in native_records(&history) {
        assert_eq!(record.progress.basis, Some(SidecarDecisionBasis::NativeModel));
        assert_eq!(record.progress.completed_probes, 2);
        assert_eq!(record.progress.native.reviewed_prompt_tokens, record.progress.native.requested_prompt_tokens);
        assert_eq!(record.progress.native.work.decoder.tokens,
            record.progress.native.requested_prompt_tokens as u64 + 2);
        assert_eq!(record.progress.native.work.sampled_draws, 2);
        assert!(record.commitment_queued && record.reveal_queued);
        assert!(record.failure.is_none() && !record.interrupted);
    }
    assert_eq!(reviewed.driver.phase(), FileDriverPhase::AwaitingDispatch { request: loaded.request });
    (reviewed.driver, history)
}

fn approval(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
    config: &mut Config, request: u64) -> FileHumanPermit
{
    let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
        700 + request, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
    let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &offer).unwrap()
}

fn dispatch(driver: &mut FileSupervisedDriver, config: &mut Config, human: &FileHumanPermit) {
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), Some(human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
}

fn publication(driver: &mut FileSupervisedDriver, config: &mut Config) {
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
}

fn reconcile(driver: &mut FileSupervisedDriver, config: &mut Config) {
    let reads = config.source.status().read_attempts;
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
    assert_eq!(config.source.status().read_attempts, reads);
}

pub(super) fn confirmed(root: &Root) -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline) {
    confirm_prepared(super::prepared(root, b"allow"))
}

fn confirmed_at_attempt_limit(root: &Root)
    -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline)
{
    let mut config = configured(root);
    config.profile.delivery.max_attempts = 1;
    let path = fixture::write(root, b"allow");
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
    let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    confirm_prepared((config, loaded, reviewer, driver, wire, deadline))
}

fn confirm_prepared(state: (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline))
    -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline)
{
    let (mut config, loaded, reviewer, driver, mut wire, deadline) = state;
    let (mut driver, history) = native_review(&mut config, &loaded, &reviewer, driver, &deadline);
    assert_native(&history);
    let human = approval(&mut driver, &reviewer, &mut config, 1);
    dispatch(&mut driver, &mut config, &human); publication(&mut driver, &mut config);
    reconcile(&mut driver, &mut config); assert!(executed(&mut wire, 1));
    assert_eq!(driver.supervisor().host().unwrap().stream_snapshot().unwrap().confirmed.message_count(), 1);
    drop(history);
    (config, loaded, reviewer, driver, wire, deadline)
}

fn reopen(config: &Config, loaded: &Loaded, driver: FileSupervisedDriver)
    -> (FileHumanReviewer, FileSupervisedDriver, Wire)
{
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let (host, reviewer) = recovery::open(config, loaded).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    (reviewer, FileSupervisedDriver::new(supervisor), ActorWire::new(port))
}

fn prepare_finish(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
    config: &mut Config, loaded: &Loaded, wire: &mut Wire, deadline: &Deadline)
{
    let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    let mut control = Control::new(config, 2, None).unwrap();
    assert!(finish::prepare(driver, reviewer, config, loaded, 1, &mut control,
        deadline, &mut || ElapsedTick(1000)).unwrap());
    assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical, before);
    submit_release(driver, wire, config, loaded, LearnedTextRelease::Finish,
        deadline, &mut || ElapsedTick(1000)).unwrap();
}

fn remove_live_inputs(root: &Root) {
    for name in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors",
        "helper-tokenizer.bbpe", "helper-sampling.json", "helper-monitor.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
}

#[test]
fn learned_finish_reopens_full_stream_runs_fresh_native_and_both_keys_without_generation() {
    for approve in [true, false] {
        let root = Root::new();
        let (mut config, _, _, driver, _, deadline) = confirmed(&root);
        let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        let message_charge = driver.supervisor().host().unwrap().request_action(1).unwrap().spec().units;
        let (path, loaded) = finish_recipe(&root, &config);
        assert_eq!(loaded.stream.max_messages(), 1); // Already full; finish consumes no slot.
        let (reviewer, mut driver, mut wire) = reopen(&config, &loaded, driver);
        prepare_finish(&mut driver, &reviewer, &mut config, &loaded, &mut wire, &deadline);
        assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
            702, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
        let (mut driver, history) = native_review(&mut config, &loaded, &reviewer, driver, &deadline);
        assert_eq!(history.records().keys().copied().collect::<Vec<_>>(), [2]);
        for view in history.input().views().values() {
            assert_eq!(view.actual_input().submitted_bytes().len(), NATIVE_INPUT_BYTES + 4);
        }
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 2 }));
        let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
            702, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
        let finish_charge = {
            let host = driver.supervisor().host().unwrap();
            let action = host.request_action(2).unwrap();
            let frame = ReleaseFrame::decode(&action.spec().payload).unwrap();
            assert!(frame.is_finish()); assert_eq!(frame.prior_messages(), &["A"]);
            assert_eq!(action.spec().units, action.spec().payload.len() as u64);
            assert!(!host.stream_snapshot().unwrap().confirmed.finished());
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            action.spec().units
        };
        if approve {
            let human = {
                let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
                reviewer.approve(&mut host, revision, &offer).unwrap()
            };
            dispatch(&mut driver, &mut config, &human); publication(&mut driver, &mut config);
            assert!(!driver.supervisor().host().unwrap().stream_snapshot().unwrap().confirmed.finished());
            reconcile(&mut driver, &mut config);
        } else {
            let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            reviewer.reject(&mut host, revision, &offer).unwrap(); let revision = host.revision();
            host.cancel_request(revision, 2).unwrap();
        }
        assert_eq!(executed(&mut wire, 2), approve);
        {
            let host = driver.supervisor().host().unwrap();
            let cut = host.stream_snapshot().unwrap();
            assert_eq!(cut.confirmed, cut.published); assert_eq!(cut.confirmed.finished(), approve);
            assert_eq!(cut.confirmed.messages().collect::<Vec<_>>(), ["A"]);
            assert_eq!(cut.publication.executions, 1 + u64::from(approve));
            assert_eq!(cut.publication.control.ledger.charged, message_charge + if approve { finish_charge } else { 0 });
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            let captured = host.file_source_status().unwrap().capture;
            assert!(captured.retained_events < 1024 && captured.fault.is_none(), "{captured:?}");
            assert!(config.source.status().read_attempts < 1024);
            assert_eq!(host.learned_text_stream_finish_request(1, 2).unwrap().deadline, deadline.logical);
            assert!(host.learned_text_stream_finish_request(2, 2).is_err());
            assert!(host.learned_text_stream_finish_request(999, 2).is_err());
        }
        assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
        remove_live_inputs(&root);
        let mut receipt = recipe::load(&path, &config, false).unwrap(); receipt.ttl_ms = 1;
        let generation = receipt.generation.clone(); let profile = config.profile.clone(); let store = config.store.clone();
        let result = recovery::resume_finish(config, receipt, 1,
            || panic!("terminal finish receipt cannot acquire time or evidence")).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(matches!(result.response.result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), approve);
        let before_output = fs::read(store.join("delivery.bin")).unwrap();
        assert_eq!(emit(result, &mut Vec::new()).is_ok(), approve);
        assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), before_output);
        let cut = FileOversight::read_stream_publication_with_learned_generation(&store, &profile, &generation).unwrap();
        assert_eq!(cut.confirmed.finished(), approve); assert_eq!(cut.confirmed.message_count(), 1);
    }
}

#[test]
fn learned_finish_refuses_unconfirmed_message_and_wrong_predecessor_without_new_work() {
    let root = Root::new();
    let (mut config, message, reviewer, driver, _, deadline) = super::prepared(&root, b"allow");
    let (_, loaded) = finish_recipe(&root, &config);
    let (mut driver, history) = native_review(&mut config, &message, &reviewer, driver, &deadline);
    let human = approval(&mut driver, &reviewer, &mut config, 1);
    let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    for phase in 0..3 {
        let host = driver.supervisor().host().unwrap(); let revision = host.revision();
        let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
        assert!(finish::check_fresh(&host, &loaded, 1).is_err());
        assert_eq!(host.revision(), revision); assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        drop(host);
        match phase { 0 => dispatch(&mut driver, &mut config, &human),
            1 => publication(&mut driver, &mut config), _ => {} }
    }
    reconcile(&mut driver, &mut config);
    {
        let host = driver.supervisor().host().unwrap();
        finish::check_fresh(&host, &loaded, 1).unwrap();
        for after in [0, 2, 999] { assert!(finish::check_fresh(&host, &loaded, after).is_err()); }
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    }
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn learned_finish_retains_admission_capacity_refusal_and_original_deadline() {
    let root = Root::new();
    let (mut config, _, _, driver, _, deadline) = confirmed_at_attempt_limit(&root);
    let (path, loaded) = finish_recipe(&root, &config);
    let (reviewer, mut driver, mut wire) = reopen(&config, &loaded, driver);
    prepare_finish(&mut driver, &reviewer, &mut config, &loaded, &mut wire, &deadline);
    let original = {
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.request_status(2).unwrap().disposition,
            FileRequestDisposition::NotAdmitted(Error::Limit));
        assert!(host.request_action(2).is_err()); // No invented FrozenAction for a refusal.
        assert_eq!(host.request_resolution(2).unwrap(), None);
        let revision = host.revision();
        let original = host.learned_text_stream_finish_request(1, 2).unwrap();
        assert_eq!(original.deadline, deadline.logical); assert_eq!(host.revision(), revision);
        assert_eq!(host.inspect().executions, 1); assert!(!host.stream_snapshot().unwrap().confirmed.finished());
        original
    };
    let retained = response(&mut wire, 2);
    assert_eq!(cleanup(driver, 1000, 1), 0); remove_live_inputs(&root);
    let mut receipt = recipe::load(&path, &config, false).unwrap(); receipt.ttl_ms = 1;
    let result = recovery::resume_finish(config, receipt, 1,
        || panic!("refused original finish cannot acquire fresh time")).unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.response, retained);
    assert!(original.message.is_none());
    assert!(emit(result, &mut Vec::new()).is_err());
}

#[test]
fn learned_finish_changed_policy_after_both_reviews_keeps_the_original_prefix_open() {
    let root = Root::new();
    let (mut config, _, _, driver, _, deadline) = confirmed(&root);
    let (_, loaded) = finish_recipe(&root, &config);
    let (reviewer, mut driver, mut wire) = reopen(&config, &loaded, driver);
    prepare_finish(&mut driver, &reviewer, &mut config, &loaded, &mut wire, &deadline);
    let (mut driver, history) = native_review(&mut config, &loaded, &reviewer, driver, &deadline);
    let human = approval(&mut driver, &reviewer, &mut config, 2);
    evidence(&root, &config, 2, false, false);
    let result = driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1001), Some(&human)).result;
    assert!(!matches!(result, Ok(FileDriverEvent::Dispatched { .. })));
    let host = driver.supervisor().host().unwrap(); let cut = host.stream_snapshot().unwrap();
    assert_eq!(cut.publication.executions, 1); assert_eq!(cut.confirmed.visible(), b"A");
    assert!(!cut.confirmed.finished() && !cut.published.finished());
    assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 2);
    assert!(!executed(&mut wire, 2));
    drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn learned_finish_cancelled_native_work_is_retained_without_closing_or_regenerating() {
    let root = Root::new();
    let (mut config, _, _, driver, _, deadline) = confirmed(&root);
    let (_, loaded) = finish_recipe(&root, &config);
    let (reviewer, mut driver, mut wire) = reopen(&config, &loaded, driver);
    prepare_finish(&mut driver, &reviewer, &mut config, &loaded, &mut wire, &deadline);
    let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    let mut run = review::start(driver, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    for _ in 0..loaded.native_limits.polls {
        if native_records(run.review()).any(|record| record.progress.native.work.decoder.tokens > 0
            && record.progress.native.status == NativeEvaluationStatus::Running) { break; }
        run.step_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap();
    }
    let reservation = run.review().reservation();
    let work: Vec<_> = native_records(run.review()).map(|record| record.progress.native.work).collect();
    assert!(work.iter().any(|work| work.decoder.tokens > 0));
    run.cancel(run.review().revision()).unwrap();
    assert_eq!(run.review().reservation(), reservation);
    assert_eq!(native_records(run.review()).map(|record| record.progress.native.work).collect::<Vec<_>>(), work);
    let handoff = run.into_handoff().unwrap();
    assert_eq!(handoff.review.status(), NativeReviewStatus::Cancelled);
    let host = handoff.driver.supervisor().host().unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 1); assert!(!host.stream_snapshot().unwrap().confirmed.finished());
    assert!(!executed(&mut wire, 2));
    drop(host); assert_eq!(cleanup(handoff.driver, 1000, 1), 0); drop(handoff.review);
}

#[test]
fn learned_finish_deadline_boundary_cannot_resume_an_expired_preparation() {
    for now in [60_999, 61_000] {
        let root = Root::new();
        let (mut config, _, _, driver, _, deadline) = confirmed(&root);
        let (_, loaded) = finish_recipe(&root, &config);
        let (reviewer, mut driver, _) = reopen(&config, &loaded, driver);
        let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
        let mut control = Control::new(&config, 2, None).unwrap();
        let result = finish::prepare(&mut driver, &reviewer, &mut config, &loaded, 1,
            &mut control, &deadline, &mut || ElapsedTick(now));
        assert_eq!(result.is_ok(), now < 61_000, "{result:?}");
        let host = driver.supervisor().host().unwrap();
        let current = host.learned_generation_inspection().unwrap();
        assert_eq!(current.numerical, before.numerical);
        assert_eq!(current.paused, now == 61_000);
        assert_eq!(host.inspect().executions, 1); assert!(!host.stream_snapshot().unwrap().confirmed.finished());
        assert!(host.request_status(2).is_err());
        if now == 61_000 { assert_eq!(current.journal_revision, before.journal_revision); }
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[test]
fn learned_finish_exact_recipe_mismatch_precedes_recovery_writes() {
    let root = Root::new();
    let (config, _, _, driver, _, _) = confirmed(&root);
    let (path, _) = finish_recipe(&root, &config); assert_eq!(cleanup(driver, 1000, 1), 0);
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    fs::write(root.0.join("prompt.txt"), b"xz").unwrap();
    let changed = recipe::load(&path, &config, false).unwrap();
    assert!(recovery::resume_finish(config, changed, 1, || panic!("changed recipe cannot read time")).is_err());
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
}

#[test]
fn learned_finish_command_options_are_closed_and_do_not_select_continued_generation() {
    for actor in ["reviewer", "--resume"] {
        let args: Vec<_> = ["create-learned-generated", "config", "recipe", actor,
            "--finish-after-message", "17"].into_iter().map(str::to_owned).collect();
        let (ordinary, after) = finish::take_option(&args).unwrap();
        assert_eq!(ordinary, &args[..4]); assert_eq!(after, Some(17));
    }
    for tail in [vec!["--finish-after-message"], vec!["--finish-after-message", "0"],
        vec!["--finish-after-message", "-1"], vec!["--finish-after-message", "1x"],
        vec!["--finish-after-message", "18446744073709551616"],
        vec!["--continue-generation", "--finish-after-message", "1"],
        vec!["--finish-after-message", "1", "--continue-generation"],
        vec!["--finish-after-message", "1", "--finish-after-message", "2"]] {
        let args: Vec<_> = ["create-learned-generated", "config", "recipe", "reviewer"]
            .into_iter().chain(tail).map(str::to_owned).collect();
        assert_eq!(command(&args, None).unwrap_err(), USAGE);
    }
    for (request, after) in [(0, 1), (1, 0), (1, 1)] { assert!(finish::check_ids(request, after).is_err()); }
    finish::check_ids(2, 1).unwrap();
}
