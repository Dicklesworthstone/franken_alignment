//! An ordinary prepared prompt can retain another exact offline capture with
//! the original source/history pair, without a predictor or a live source read.
use super::*;
use crate::workflow::actor_service::learned::{
    checkpoint_prepare as preparation, stream_checkpoint as control,
};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedRecoveryStatus, FileLearnedStreamCheckpointRequest,
};
use fa_reference::strict_json::{self, Limits};
use std::io::{self, Write};

fn prepared_request(root: &Root, case: &Case) -> FileLearnedStreamCheckpointRequest {
    let mut output = Vec::new();
    preparation::command(&preparation_args(root, case), None, || ElapsedTick(1000),
        &mut output).unwrap();
    let value = report(&output);
    prepared(case, &value);
    let control::Request::Capture(original) = capture_request(&value) else {
        panic!("original preparation must identify its native capture");
    };
    let mut info = original.checkpoint();
    info.checkpoint = 72;
    // A distinct explicit capture at the same numerical cut is a new original
    // event; it cannot stand in for renewed prompt work or a review key.
    FileLearnedStreamCheckpointRequest::new(number(&value, "revision"), info).unwrap()
}

fn command(root: &Root, case: &Case, request: FileLearnedStreamCheckpointRequest,
    label: &str) -> Vec<String> {
    let info = request.checkpoint();
    let path = root.0.join(format!("{label}.capture.json"));
    fs::write(&path, format!(concat!(
        "{{\"schema\":\"fa.learned-stream-checkpoint/1\",\"checkpoint\":\"{}\",",
        "\"expected_journal_revision\":\"{}\",\"expected_actor_revision\":\"{}\",",
        "\"expected_authority_epoch\":\"{}\",\"expected_control_sequence\":\"{}\",",
        "\"expected_stream\":\"{}\",\"expected_position\":\"{}\",\"expected_sampled_draws\":\"{}\"}}"),
        info.checkpoint, request.revision(), info.actor_revision, info.authority_epoch,
        info.control_sequence, info.stream, info.position, info.sampled_draws)).unwrap();
    let profile = recovery_profile(root, &root.0.join("source-1.bin"),
        Some(&root.0.join(format!("{label}.topology"))), label);
    vec!["create-learned-generated".into(), case.config_path.display().to_string(),
        case.recipe.display().to_string(), "--capture-stream-checkpoint".into(),
        path.display().to_string(), "--history-anchor".into(), case.anchor.display().to_string(),
        "--retain-history-anchor".into(), root.0.join(format!("{label}.anchor")).display().to_string(),
        "--topology-profile".into(), profile.display().to_string()]
}

fn result(bytes: &[u8]) -> Json {
    let value = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(value.get("schema").and_then(Json::as_str), Some("fa.learned-stream-control-result/1"));
    assert_eq!(value.get("authority").and_then(Json::as_str), Some("none"));
    assert_eq!(value.get("topology_checkpoint_retained").and_then(Json::as_bool), Some(true));
    assert_eq!(value.get("history_anchor_retained").and_then(Json::as_bool), Some(true));
    value
}

#[test]
fn ordinary_prepared_capture_command_retains_both_archives_after_lost_report_and_exact_retry() {
    let root = Root::new(); let case = selected(&root);
    let request = prepared_request(&root, &case);
    let original = checkpoint(&root.0.join("source-1.bin"));
    let original_archive = fs::read(&case.anchor).unwrap();
    let original_source = fs::read(root.0.join("source-1.bin")).unwrap();
    for name in ["live-1.json", "inventory-1.json", "helper-weights.safetensors"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    let first = command(&root, &case, request, "captured");
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("lost report")) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let error = control::command(&first, None, || ElapsedTick(1000), &mut Broken).unwrap_err();
    assert!(error.contains("acknowledged"), "{error}");
    let captured = checkpoint(&root.0.join("captured.topology"));
    assert_eq!(captured.history_anchor(), &custody::load(Path::new(&first[8])).unwrap());
    assert_eq!(captured.history_anchor().revision(), request.revision() + 1);
    assert_eq!(captured.work(), original.work());
    assert_eq!(captured.last_operation(), original.last_operation());
    assert_eq!(captured.expected(), original.expected());
    let disk = case.disk();
    let retry = command(&root, &case, request, "retried");
    let mut output = Vec::new();
    control::command(&retry, None, || ElapsedTick(1000), &mut output).unwrap();
    let value = result(&output);
    assert_eq!(value.get("outcome").and_then(Json::as_str), Some("already_captured"));
    assert_eq!(number(&value, "appended_events"), 0);
    assert_eq!(number(&value, "position"), 2);
    assert_eq!(number(&value, "sampled_draws"), 0);
    assert_eq!(number(&value, "restart_attempts"), 0);
    assert_eq!(checkpoint(&root.0.join("retried.topology")), captured);
    assert_eq!(case.disk(), disk);
    assert_eq!(fs::read(&case.anchor).unwrap(), original_archive);
    assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), original_source);
    assert_eq!(view(&case).executions, 0);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[test]
fn ordinary_mediated_capture_keeps_ready_deadline_exact_pair_and_request_boundaries() {
    let root = Root::new(); let case = selected(&root);
    let request = prepared_request(&root, &case);
    let args = command(&root, &case, request, "selected");
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
    loaded.anchor_output = Some(PathBuf::from(&args[8]));
    topology::admit(&case.config, &mut loaded, Some(Path::new(&args[10])), topology::Mode::Live).unwrap();
    let disk = case.disk();
    let selection = control::Request::Capture(request);
    let error = recovery::stream_checkpoint::execute_paired(&case.config, &loaded, &selection,
        &mut |progress| {
            if progress.is_some_and(|value| value.status == FileLearnedRecoveryStatus::Ready) {
                return Err("original deadline at Ready".into());
            }
            Ok(())
        }).unwrap_err();
    assert_eq!(error, "original deadline at Ready");
    assert_eq!(case.disk(), disk);
    assert!(!Path::new(&args[8]).exists());
    assert!(!root.0.join("selected.topology").exists());
    let foreign_root = Root::new(); let foreign_case = selected(&foreign_root);
    prepared_request(&foreign_root, &foreign_case);
    loaded.recovery_anchor = Some(custody::load(&foreign_case.anchor).unwrap());
    assert!(recovery::stream_checkpoint::execute_paired(&case.config, &loaded, &selection,
        &mut |_| Ok(())).unwrap_err().contains("different cuts"));
    assert_eq!(case.disk(), disk);
    let mut wrong = request.checkpoint(); wrong.position += 1;
    let wrong = FileLearnedStreamCheckpointRequest::new(request.revision(), wrong).unwrap();
    let rejected = command(&root, &case, wrong, "wrong");
    assert!(control::command(&rejected, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(case.disk(), disk);
    let mut output = Vec::new();
    control::command(&args, None, || ElapsedTick(1000), &mut output).unwrap();
    let value = result(&output);
    assert_eq!(value.get("outcome").and_then(Json::as_str), Some("captured"));
    assert_eq!(number(&value, "appended_events"), 1);
    assert_eq!(number(&value, "checkpoints"), 2);
    assert_eq!(number(&value, "sampled_draws"), 0);
    assert_eq!(view(&case).executions, 0);
}

#[test]
fn offline_capture_preserves_missing_next_sources_before_acknowledging_a_new_checkpoint() {
    let root = Root::new();
    let case = selected(&root);
    let request = prepared_request(&root, &case);
    let before = case.disk();
    let next_graph = root.0.join("live-2.json");
    let next_inventory = root.0.join("inventory-2.json");
    assert!(!next_graph.exists() && !next_inventory.exists());
    let mut args = command(&root, &case, request, "isolated");
    let history = args[8].clone();
    let profile = PathBuf::from(&args[10]);
    let source = root.0.join("isolated.topology");
    let original = fs::read_to_string(&profile).unwrap();

    args[8] = next_graph.display().to_string();
    let error = control::command(&args, None, || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("output conflicts with an independently selected input"), "{error}");
    assert_eq!(case.disk(), before);
    assert!(!next_graph.exists() && !source.exists());

    args[8] = history;
    fs::write(&profile, original.replace(source.to_str().unwrap(), next_inventory.to_str().unwrap())).unwrap();
    let error = control::command(&args, None, || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("output conflicts with an independently selected input"), "{error}");
    assert_eq!(case.disk(), before);
    assert!(!next_inventory.exists() && !Path::new(&args[8]).exists());

    // Restore only the independent output. Neither next source has to exist
    // to capture a second checkpoint under the retained historical pair.
    fs::write(profile, original).unwrap();
    let mut output = Vec::new();
    control::command(&args, None, || ElapsedTick(1000), &mut output).unwrap();
    let value = result(&output);
    assert_eq!(number(&value, "appended_events"), 1);
    assert_eq!(number(&value, "sampled_draws"), 0);
    assert_eq!(checkpoint(&source).history_anchor(), &custody::load(Path::new(&args[8])).unwrap());
    assert!(!next_graph.exists() && !next_inventory.exists());
}
