//! /4 loaded-owner continuation through actual native inference and both keys.
//! The original fixture model naturally samples A/stop/A/stop; identical visible
//! bytes remain distinct receipt-bound messages with cumulative source evidence.
use super::*;
use super::super::{continuation, finish};
use fa_reference::action::consequence::delivery::{stream::ReleaseFrame,
    persistent::observed::{FileHumanPermit, decoder::learned::sidecar::FileLearnedSidecarFinish}};
use fa_reference::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use std::path::PathBuf;

#[path = "continuation/native.rs"] mod native;

pub(super) fn write(root: &Root) -> PathBuf {
    let path = fixture::write(root, b"allow");
    native::provision(root);
    let recipe = fs::read_to_string(&path).unwrap()
        .replacen("fa.learned-publication/1", "fa.learned-publication/4", 1)
        .replacen("\"max_messages\":1", "\"max_messages\":2", 1)
        .replacen("\"max_total_bytes\":64", "\"max_total_bytes\":128", 1)
        .replacen("\"max_new_tokens\":2", "\"max_new_tokens\":4", 1)
        .replacen("\"vocabulary_scores\":1024", "\"vocabulary_scores\":1028", 1)
        .replacen("\"sidecar\":{", "\"sidecar\":{\"priority\":[],", 1)
        .replacen("\"native_review\":{", "\"native_review\":{\"round_ids\":[1],", 1);
    fs::write(&path, recipe).unwrap(); path
}

pub(super) fn next(root: &Root, config: &Config, request: u64) -> (PathBuf, Loaded) {
    let path = root.0.join(format!("request-{request}.json"));
    let original = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    let ids = [request * 10 + 1, request * 10 + 2, request * 10 + 3];
    // The frozen rank-one codec has a wide coarse key envelope at position5.
    // Independently provision the real residual ladder before either later
    // review starts. A later round must consume it before native inference.
    let priority = r#"[{"layer":1,"side":"key","position":5,"head":0},{"layer":1,"side":"value","position":5,"head":0}]"#;
    fs::write(&path, original.replacen("\"request\":1", &format!("\"request\":{request}"), 1)
        .replacen("\"object_id\":1001", &format!("\"object_id\":{}", 1000 + request), 1)
        .replacen("\"priority\":[]", &format!("\"priority\":{priority}"), 1)
        .replacen("\"rounds\":1", "\"rounds\":3", 1)
        .replacen("\"round_ids\":[1]", &format!("\"round_ids\":{ids:?}"), 1)
        .replacen("\"evaluations\":2", "\"evaluations\":6", 1)
        .replacen(&format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 2),
            &format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 6), 1)
        .replacen("\"sampling_entries\":8192", "\"sampling_entries\":24576", 1)).unwrap();
    let loaded = recipe::load(&path, config, true).unwrap();
    assert_eq!(loaded.request, request); assert_eq!(loaded.rounds, ids);
    assert!(loaded.generation.allows_text_stream_continuation());
    (path, loaded)
}

fn prepared(root: &Root) -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline) {
    let mut config = configured(root); let path = write(root);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
    let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    (config, loaded, reviewer, driver, wire, deadline)
}

pub(super) fn review_current(config: &mut Config, loaded: &Loaded, reviewer: &FileHumanReviewer,
    driver: FileSupervisedDriver, deadline: &Deadline) -> (FileSupervisedDriver, FileNativeSidecarSequence) {
    let mut control = Control::new(config, loaded.request, None).unwrap();
    let reviewed = review::run(driver, config, loaded, reviewer, &mut control,
        deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let history = reviewed.history.unwrap();
    assert_eq!(history.status(), NativeReviewStatus::Finished);
    assert_eq!(history.records().keys().copied().collect::<Vec<_>>(), loaded.rounds);
    assert_eq!(native_records(&history).filter(|record|
        record.progress.basis == Some(SidecarDecisionBasis::NativeModel)).count(), 2,
        "request {} must finish with two actual native reviewers: {:?}", loaded.request, history.records());
    for record in native_records(&history) {
        assert_eq!(record.progress.completed_probes, 2);
        if record.progress.basis == Some(SidecarDecisionBasis::NativeModel) {
            assert_eq!(record.progress.native.reviewed_prompt_tokens, record.progress.native.requested_prompt_tokens);
            assert!(record.progress.native.requested_prompt_tokens > 0);
            assert_eq!(record.progress.native.work.decoder.tokens,
                record.progress.native.requested_prompt_tokens as u64 + 2);
            assert_eq!(record.progress.native.work.sampled_draws, 2);
        } else {
            assert!(loaded.rounds.len() > 1);
            assert_eq!(record.progress.basis, Some(SidecarDecisionBasis::NumericalUncertainty),
                "request {}: {:?}", loaded.request, record.progress);
            assert!(!record.progress.native_started);
            assert_eq!(record.progress.native.work.decoder.tokens, 0);
            assert_eq!(record.progress.native.work.sampled_draws, 0);
        }
        assert!(record.commitment_queued && record.reveal_queued);
        assert!(record.failure.is_none() && !record.interrupted);
    }
    if loaded.request != 1 {
        assert!(history.history().iter().any(|result| matches!(result,
            FileLearnedSidecarFinish::Refined { group, .. } if group.row.position == 5)));
    }
    assert_eq!(reviewed.driver.phase(), FileDriverPhase::AwaitingDispatch { request: loaded.request });
    (reviewed.driver, history)
}

pub(super) fn approve(driver: &mut FileSupervisedDriver, config: &mut Config,
    reviewer: &FileHumanReviewer, request: u64) -> FileHumanPermit {
    let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
        700 + request, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
    let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &offer).unwrap()
}

pub(super) fn publish(driver: &mut FileSupervisedDriver, config: &mut Config, human: &FileHumanPermit) {
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), Some(human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
}

fn reopen(config: &Config, loaded: &Loaded, driver: FileSupervisedDriver)
    -> (FileHumanReviewer, FileSupervisedDriver, Wire) {
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let (host, reviewer) = recovery::open(config, loaded).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    (reviewer, FileSupervisedDriver::new(supervisor), ActorWire::new(port))
}

#[test]
fn loaded_continuation_replays_two_messages_with_fresh_native_human_reviews_and_explicit_finish() {
    let root = Root::new();
    let (mut config, first, reviewer, driver, mut wire, deadline) = prepared(&root);
    let (mut driver, history) = review_current(&mut config, &first, &reviewer, driver, &deadline);
    let first_human = approve(&mut driver, &mut config, &reviewer, 1);
    publish(&mut driver, &mut config, &first_human); assert!(executed(&mut wire, 1));
    let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    assert_eq!(before.position, 4); assert_eq!(before.sampled_draws, 2);
    let (_, second) = next(&root, &config, 2); assert_eq!(first.generation, second.generation);
    let (reviewer, mut driver, mut wire) = reopen(&config, &second, driver); drop(history);
    let command = driver.supervisor().host().unwrap().prepare_learned_text_stream_continuation(1, 2).unwrap();
    let mut control = Control::new(&config, 2, None).unwrap();
    assert!(continuation::prepare(&mut driver, &reviewer, &mut config, command,
        &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
    {
        let current = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        assert_eq!(current.position, before.position); assert_eq!(current.work, before.work);
        assert_eq!(current.telemetry, before.telemetry); assert_eq!(current.sampled_draws, before.sampled_draws);
    }
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    {
        let host = driver.supervisor().host().unwrap();
        let message = host.learned_text_message(second.evidence).unwrap();
        assert_eq!(message.bytes(), b"A");
        assert_eq!(message.evidence().tokens(), &[120, 121, 65, 256, 65, 256]);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 4);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
        assert!(host.prepare_learned_text_stream_continuation(1, 3).is_err());
    }
    submit(&mut driver, &mut wire, &mut config, &second, &deadline, &mut || ElapsedTick(1000)).unwrap();
    assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
        702, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
    let (mut driver, history) = review_current(&mut config, &second, &reviewer, driver, &deadline);
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 2 }));
    let human = approve(&mut driver, &mut config, &reviewer, 2);
    let published = driver.supervisor().host().unwrap().stream_snapshot().unwrap().published;
    assert!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), Some(&first_human)).result.is_err());
    assert_eq!(driver.supervisor().host().unwrap().stream_snapshot().unwrap().published, published);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
    publish(&mut driver, &mut config, &human); assert!(executed(&mut wire, 2));
    let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    let (path, last) = next(&root, &config, 3); assert_eq!(last.generation, second.generation);
    let (reviewer, mut driver, mut wire) = reopen(&config, &last, driver); drop(history);
    let mut control = Control::new(&config, 3, None).unwrap();
    assert!(finish::prepare(&mut driver, &reviewer, &mut config, &last, 2,
        &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
    {
        let host = driver.supervisor().host().unwrap();
        let message = host.learned_text_message(last.evidence).unwrap();
        assert_eq!(message.bytes(), b"A", "recovery must retain the second message boundary");
        assert_eq!(message.evidence().tokens(), &[120, 121, 65, 256, 65, 256]);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    }
    let submitted = submit_release(&mut driver, &mut wire, &mut config, &last, LearnedTextRelease::Finish,
        &deadline, &mut || ElapsedTick(1000));
    assert!(submitted.is_ok(), "{submitted:?}; source capture: {:?}; source reader: {:?}; request: {:?}; journal revision: {}; journal bytes: {}",
        driver.supervisor().host().unwrap().file_source_status(), config.source.status(),
        driver.supervisor().host().unwrap().request_status(last.request),
        driver.supervisor().host().unwrap().revision(), fs::metadata(config.store.join("delivery.bin")).unwrap().len());
    let (mut driver, history) = review_current(&mut config, &last, &reviewer, driver, &deadline);
    let human = approve(&mut driver, &mut config, &reviewer, 3);
    publish(&mut driver, &mut config, &human); assert!(executed(&mut wire, 3));
    {
        let host = driver.supervisor().host().unwrap(); let stream = host.stream_snapshot().unwrap();
        assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
        assert!(stream.confirmed.finished()); assert_eq!(stream.confirmed, stream.published);
        assert_eq!(stream.publication.executions, 3); assert_eq!(stream.pending, None);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        let charge: u64 = (1..=3).map(|request| host.request_action(request).unwrap().spec().units).sum();
        assert_eq!(stream.publication.control.ledger.charged, charge);
        let second_frame = ReleaseFrame::decode(&host.request_action(2).unwrap().spec().payload).unwrap();
        assert_eq!(second_frame.prior_messages(), &["A"]); assert_eq!(second_frame.message(), Some("A"));
        assert_eq!(host.learned_text_stream_finish_request(2, 3).unwrap().deadline, deadline.logical);
        assert!(host.learned_text_stream_finish_request(1, 3).is_err());
        let captured = host.file_source_status().unwrap().capture;
        assert!(captured.retained_events < config.source_policy.limits.events && captured.fault.is_none(), "{captured:?}");
    }
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
    for file in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors",
        "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
        fs::remove_file(root.0.join(file)).unwrap();
    }
    let loaded = recipe::load(&path, &config, false).unwrap();
    let result = recovery::resume_finish(config, loaded, 2,
        || panic!("confirmed finish must not resume generation or reviews")).unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
}

#[test]
fn continuation_recipe_pins_original_caps_and_does_not_upgrade_legacy_stream_history() {
    let root = Root::new(); let config = configured(&root); let path = write(&root);
    let admitted = recipe::load(&path, &config, false).unwrap();
    let (host, _) = recovery::create(&config, &admitted).unwrap(); drop(host);
    let disk = fs::read(config.store.join("delivery.bin")).unwrap();
    let original = fs::read_to_string(&path).unwrap();
    for changed in [original.replace("fa.learned-publication/4", "fa.learned-publication/3"),
        original.replace("\"max_new_tokens\":4", "\"max_new_tokens\":5")
            .replace("\"vocabulary_scores\":1028", "\"vocabulary_scores\":1285"),
        original.replace("\"vocabulary_scores\":1028", "\"vocabulary_scores\":1285")] {
        fs::write(&path, changed).unwrap();
        let changed = recipe::load(&path, &config, false).unwrap();
        assert!(recovery::open(&config, &changed).is_err());
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
    }
    fs::write(&path, original.replace("fa.learned-publication/4", "fa.learned-publication/3")).unwrap();
    let legacy = recipe::load(&path, &config, false).unwrap();
    assert!(!legacy.generation.allows_text_stream_continuation());
    assert_ne!(legacy.generation, admitted.generation);
}

#[test]
fn continuation_options_require_one_explicit_nonzero_predecessor_before_any_io() {
    let valid = ["create-learned-generated", "config", "recipe", "reviewer", "--after-message", "71"].map(str::to_owned);
    let (base, after) = continuation::take_option(&valid).unwrap();
    assert_eq!(base, &valid[..4]); assert_eq!(after, Some(71));
    for args in [
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--after-message"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--after-message", "0"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--after-message", "-1"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--after-message", "1.0"],
        vec!["create-learned-generated", "config", "recipe", "--resume", "--after-message", "71"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--continue-generation", "--after-message", "71"],
        vec!["create-learned-generated", "config", "recipe", "reviewer", "--after-message", "71", "--finish-after-message", "71"],
    ] {
        assert_eq!(command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>(), None).unwrap_err(), USAGE);
    }
    assert!(continuation::check_ids(72, 71).is_ok());
    for (request, after) in [(0, 71), (72, 0), (71, 71)] {
        assert!(continuation::check_ids(request, after).is_err());
    }
}
