//! Real imported generation through exact capture and initiation command files.
use super::*;
use super::super::super::stream_checkpoint as checkpoint_control;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedStreamCheckpointRequest, FileLearnedStreamControlReceipt,
    checkpoint::FileLearnedCheckpointInfo,
};

struct CaptureCase {
    config: Config,
    recipe: PathBuf,
    loaded: Loaded,
    request: FileLearnedStreamCheckpointRequest,
    numerical: HostedLearnedInspection,
}

fn capture_case(root: &Root) -> CaptureCase {
    let mut config = configured(root);
    let original = &config.profile.delivery.actor;
    let mut declared = original.profile(); declared.grade = RestartGrade::FunctionalRestart;
    config.profile.delivery.actor = ActorState::new(declared, original.tokens().to_vec(),
        original.cache().to_vec(), original.sampler().to_vec(), original.next_position()).unwrap();
    let recipe_path = super::super::continuation::write(root);
    evidence(root, &config, 1, true, false);
    let mut loaded = recipe::load(&recipe_path, &config, true).unwrap();
    let (mut host, _reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let initial = host.learned_generation_inspection().unwrap().numerical;
    let event = host.advance_learned_generation(host.revision(), initial.actor_revision,
        initial.position).unwrap().unwrap();
    let stream = event.audit().source().descriptor().layers().values().next().unwrap().stream;
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let current = host.inspect();
    let request = FileLearnedStreamCheckpointRequest::new(host.revision(), FileLearnedCheckpointInfo {
        checkpoint: 1, actor_revision: numerical.actor_revision,
        authority_epoch: current.control.ledger.epoch, control_sequence: current.control.sequence,
        stream, position: numerical.position, sampled_draws: numerical.sampled_draws,
    }).unwrap();
    loaded.recovery_anchor = Some(host.history_anchor().unwrap());
    loaded.anchor_output = Some(root.0.join("capture-output.anchor"));
    drop(host);
    CaptureCase { config, recipe: recipe_path, loaded, request, numerical }
}

fn capture_document(request: FileLearnedStreamCheckpointRequest) -> Vec<u8> {
    let info = request.checkpoint();
    format!(concat!(
        "{{\"schema\":\"fa.learned-stream-checkpoint/1\",\"checkpoint\":\"{}\",",
        "\"expected_journal_revision\":\"{}\",\"expected_actor_revision\":\"{}\",",
        "\"expected_authority_epoch\":\"{}\",\"expected_control_sequence\":\"{}\",",
        "\"expected_stream\":\"{}\",\"expected_position\":\"{}\",\"expected_sampled_draws\":\"{}\"}}"),
        info.checkpoint, request.revision(), info.actor_revision, info.authority_epoch,
        info.control_sequence, info.stream, info.position, info.sampled_draws).into_bytes()
}

fn capture_args(root: &Root, case: &CaptureCase) -> Vec<String> {
    let request = root.0.join("independent-capture.json");
    fs::write(&request, capture_document(case.request)).unwrap();
    let input = root.0.join("capture-input.anchor");
    custody::export_acknowledged(case.loaded.recovery_anchor.as_ref().unwrap(), &input).unwrap();
    vec!["create-learned-generated".into(), config_file(root, &case.config).display().to_string(),
        case.recipe.display().to_string(), "--capture-stream-checkpoint".into(), request.display().to_string(),
        "--history-anchor".into(), input.display().to_string(), "--retain-history-anchor".into(),
        case.loaded.anchor_output.as_ref().unwrap().display().to_string()]
}

fn decoded(bytes: &[u8]) -> Json {
    let json = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(json.get("schema").and_then(Json::as_str), Some("fa.learned-stream-control-result/1"));
    assert_eq!(json.get("authority").and_then(Json::as_str), Some("none"));
    json
}

#[test]
fn command_captures_the_exact_native_prefix_and_lost_report_retry_adds_no_event() {
    let root = Root::new(); let case = capture_case(&root); let mut command = capture_args(&root, &case);
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("lost output")) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let result = checkpoint_control::command(&command, None, || ElapsedTick(1000), &mut Broken);
    assert!(result.unwrap_err().contains("acknowledged"));
    let archive = custody::load(case.loaded.anchor_output.as_deref().unwrap()).unwrap();
    assert_eq!(archive.revision(), case.request.revision() + 1);
    let before = fs::read(case.config.store.join("delivery.bin")).unwrap();
    command[8] = root.0.join("capture-retry.anchor").display().to_string();
    let mut output = Vec::new();
    checkpoint_control::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    let report = decoded(&output);
    assert_eq!(report.get("outcome").and_then(Json::as_str), Some("already_captured"));
    assert_eq!(report.get("appended_events").and_then(Json::as_str), Some("0"));
    assert_eq!(report.get("restart_attempts").and_then(Json::as_str), Some("0"));
    assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), before);
    let result = recovery::stream_checkpoint::execute(&case.config, &case.loaded,
        &checkpoint_control::Request::Capture(case.request), &mut |_| Ok(())).unwrap();
    assert_eq!(result.numerical(), &case.numerical);
    assert_eq!(result.receipt(), &FileLearnedStreamControlReceipt::Captured(case.request.checkpoint()));
}

#[test]
fn command_begins_an_independent_intent_then_existing_completion_restores_the_original_prefix() {
    let root = Root::new(); let case = prepared_control(&root, false, false, false);
    let mut command = args(&root, &case); command[3] = "--begin-stream-reset".into();
    let mut output = Vec::new();
    checkpoint_control::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    let report = decoded(&output);
    assert_eq!(report.get("outcome").and_then(Json::as_str), Some("pending"));
    assert_eq!(report.get("appended_events").and_then(Json::as_str), Some("1"));
    assert_eq!(report.get("restart_attempts").and_then(Json::as_str), Some("0"));
    let pending = FileOversight::read_learned_reset_result(&case.config.store,
        &case.config.profile, &case.loaded.generation, 900).unwrap();
    assert_eq!(pending.journal_revision(), case.before.revision + 1);
    assert!(matches!(pending.record(), FileLearnedResetRecord::Pending(value)
        if value.intent == case.request.intent && !value.interrupted));
    let original = disk(&case);
    command[8] = root.0.join("begin-retry.anchor").display().to_string();
    output.clear(); checkpoint_control::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    assert_eq!(decoded(&output).get("outcome").and_then(Json::as_str), Some("already_pending"));
    assert_eq!(disk(&case), original);
    command[3] = "--complete-stream-reset".into();
    command[8] = root.0.join("completed-after-begin.anchor").display().to_string();
    output.clear(); reset::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    assert_eq!(super::report(&output).get("outcome").and_then(Json::as_str), Some("restored"));
    let (host, _) = recovery::open(&case.config, &case.loaded).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.cumulative_work,
        case.numerical.cumulative_work);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
}

#[test]
fn capture_decoder_and_closed_argv_refuse_near_misses_before_mutation() {
    let root = Root::new(); let case = capture_case(&root); let command = capture_args(&root, &case);
    let good = capture_document(case.request);
    assert_eq!(checkpoint_control::decode_capture(&good).unwrap(), case.request);
    let text = String::from_utf8(good).unwrap();
    for invalid in [text.replace("\"checkpoint\":\"1\"", "\"checkpoint\":1"),
        text.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"01\""),
        text.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"0\""),
        text.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"18446744073709551616\""),
        text.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"1\",\"unexpected\":\"1\""),
        text.replace("\"checkpoint\":\"1\"", "\"checkpoint\":\"1\",\"checkpoint\":\"1\"") ] {
        assert!(checkpoint_control::decode_capture(invalid.as_bytes()).is_err());
    }
    let before = fs::read(case.config.store.join("delivery.bin")).unwrap();
    for flag in ["--topology-profile", "--stop-profile", "--continue-generation", "--resume", "--complete-stream-reset"] {
        let mut changed = command.clone(); changed.push(flag.into());
        assert!(checkpoint_control::command(&changed, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
        assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), before);
    }
    let mut changed = case.request.checkpoint(); changed.position += 1;
    let changed = FileLearnedStreamCheckpointRequest::new(case.request.revision(), changed).unwrap();
    fs::write(&command[4], capture_document(changed)).unwrap();
    assert!(checkpoint_control::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), before);
    assert!(!case.loaded.anchor_output.as_ref().unwrap().exists());
}

#[test]
fn ready_deadline_withholds_capture_and_existing_request_cannot_be_rebased() {
    let root = Root::new(); let case = capture_case(&root);
    let before = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let result = recovery::stream_checkpoint::execute(&case.config, &case.loaded,
        &checkpoint_control::Request::Capture(case.request), &mut |progress| {
            if progress.is_some_and(|value| value.status ==
                fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedRecoveryStatus::Ready) {
                return Err("deadline at Ready".into());
            }
            Ok(())
        });
    assert_eq!(result.unwrap_err(), "deadline at Ready");
    assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), before);
    let result = recovery::stream_checkpoint::execute(&case.config, &case.loaded,
        &checkpoint_control::Request::Capture(case.request), &mut |_| Ok(())).unwrap();
    assert_eq!(result.appended_events(), 1);
}
