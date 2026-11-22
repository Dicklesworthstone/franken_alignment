//! Ordinary mediated BeginReset is a complete command path. It retains the
//! exact independent source/history cut through reset and fresh publication.
use super::*;
use crate::workflow::actor_service::learned::{
    stream_checkpoint as checkpoint_control,
    tests::{functional_creation, peer_intake},
};
use fa_reference::action::consequence::delivery::{
    persistent::observed::reviewer::wire::ReviewDecision, stream::ReleaseFrame,
};

fn control_report(bytes: &[u8]) -> Json {
    let result = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(result.get("schema").and_then(Json::as_str), Some("fa.learned-stream-control-result/1"));
    assert_eq!(result.get("authority").and_then(Json::as_str), Some("none"));
    assert_eq!(result.get("history_anchor_retained").and_then(Json::as_bool), Some(true));
    assert_eq!(result.get("topology_checkpoint_retained").and_then(Json::as_bool), Some(true));
    result
}

fn same_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
}

#[test]
fn ordinary_begin_reset_command_completes_then_requires_fresh_source_native_and_human_publication() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let case = prepared_control(&root, false, false);
        assert!(case.case.loaded.generation.required_owned_pre_output_forecast().is_none());
        assert!(case.case.loaded.generation.required_pre_output_forecast().is_none());
        let mut begin = arguments(&root, &case);
        begin[3] = "--begin-stream-reset".into();
        let old_history = fs::read(&begin[6]).unwrap();
        let old_source = fs::read(root.0.join("source-1.bin")).unwrap();
        // Initial independent requirements remain available. The current and
        // next source files are absent throughout the offline control work.
        for name in ["live-1.json", "inventory-1.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let mut output = Vec::new();
        checkpoint_control::command(&begin, None, || ElapsedTick(1000), &mut output).unwrap();
        let pending = control_report(&output);
        assert_eq!(pending.get("outcome").and_then(Json::as_str), Some("pending"));
        assert_eq!(pending.get("appended_events").and_then(Json::as_str), Some("1"));
        assert_eq!(pending.get("restart_attempts").and_then(Json::as_str), Some("0"));
        let saved = checkpoint(&root.0.join("source-reset.bin"));
        same_source(&case.saved, &saved);
        assert_eq!(saved.available(), case.saved.available());
        assert_eq!(saved.history_anchor(), &custody::load(Path::new(&begin[8])).unwrap());
        assert_eq!(saved.history_anchor().revision(), case.case.before.revision + 1);
        let original = disk(&case.case);
        begin[8] = root.0.join("begin-retry.anchor").display().to_string();
        profile(&root, &case.saved, "source-1.bin", Some("source-retry.bin"));
        output.clear();
        checkpoint_control::command(&begin, None, || ElapsedTick(1000), &mut output).unwrap();
        let retry = control_report(&output);
        assert_eq!(retry.get("outcome").and_then(Json::as_str), Some("already_pending"));
        assert_eq!(retry.get("appended_events").and_then(Json::as_str), Some("0"));
        assert_eq!(disk(&case.case), original);
        assert_eq!(checkpoint(&root.0.join("source-retry.bin")), saved);
        assert_eq!(fs::read(&begin[6]).unwrap(), old_history);
        assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), old_source);

        let mut complete = begin.clone();
        complete[3] = "--complete-stream-reset".into();
        complete[6] = begin[8].clone();
        complete[8] = root.0.join("completed.anchor").display().to_string();
        profile(&root, &saved, "source-retry.bin", Some("source-completed.bin"));
        output.clear();
        reset::command(&complete, None, || ElapsedTick(1000), &mut output).unwrap();
        let restored = report(&output);
        assert_eq!(restored.get("outcome").and_then(Json::as_str), Some("restored"));
        assert_eq!(restored.get("restart_attempts").and_then(Json::as_str), Some("1"));
        assert_eq!(restored.get("incident_count").and_then(Json::as_str), Some("1"));
        let completed = checkpoint(&root.0.join("source-completed.bin"));
        same_source(&case.saved, &completed);
        assert!(!completed.available());
        assert_eq!(completed.history_anchor(), &custody::load(Path::new(&complete[8])).unwrap());
        assert_eq!(completed.history_anchor().revision(), case.case.before.revision + 3);
        assert_eq!(snapshot(&case.case).executions, 0);

        let recipe = root.0.join("fresh-request.json");
        let original_recipe = fs::read_to_string(&case.case.recipe).unwrap();
        fs::write(&recipe, original_recipe
            .replacen("\"request\":1", "\"request\":2", 1)
            .replacen("\"object_id\":1001", "\"object_id\":1002", 1)
            .replacen("\"round_ids\":[1]", "\"round_ids\":[2]", 1)).unwrap();
        let fresh = recipe::load(&recipe, &case.case.config, true).unwrap();
        assert_eq!(fresh.generation, case.case.loaded.generation);
        assert_eq!(fresh.request, 2); assert_eq!(fresh.rounds, [2]);
        let next = fs::read_to_string(root.0.join("initial.json")).unwrap()
            .replacen("\"generation\":1", "\"generation\":2", 1)
            .replacen("\"inventory_generation\":1", "\"inventory_generation\":2", 1)
            .replacen("\"complete_inventory_generation\":1", "\"complete_inventory_generation\":2", 1);
        fs::write(root.0.join("missing-next.json"), next).unwrap();
        fs::copy(root.0.join("initial-inventory.json"), root.0.join("missing-next-inventory.json")).unwrap();
        let live_profile = profile(&root, &completed, "source-completed.bin", Some("source-published.bin"));
        let (peers_path, peers) = functional_creation::peers_file(&root, &case.case.config);
        let live = vec!["create-learned-generated".into(), complete[1].clone(),
            recipe.display().to_string(), peers_path.display().to_string(),
            "--continue-generation".into(), "--history-anchor".into(), complete[8].clone(),
            "--retain-history-anchor".into(), root.0.join("published.anchor").display().to_string(),
            "--topology-profile".into(), live_profile.display().to_string()];
        let mut human = None;
        let result = crate::workflow::actor_service::learned::command_with_clock(&live, None, || {
            if human.is_none() && peers.socket(2).exists() {
                human = Some(peer_intake::human(peers.clone(), 2, decision));
            }
            ElapsedTick(1001)
        });
        let packet = human.expect("reset receipt must not replace independent human review")
            .join().unwrap();
        assert_eq!(result.is_ok(), decision == ReviewDecision::Approve, "{result:?}");
        assert_eq!(packet.views().len(), 2);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        let state = snapshot(&case.case);
        assert_eq!(state.executions, u64::from(decision == ReviewDecision::Approve));
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.charged, if decision == ReviewDecision::Approve {
            packet.action().spec().units
        } else { 0 });
        let published = checkpoint(&root.0.join("source-published.bin"));
        assert_eq!(published.history_anchor(), &custody::load(Path::new(&live[8])).unwrap());
        assert!(published.work().checks > completed.work().checks);
        assert_eq!(published.expected().source_limits(), completed.expected().source_limits());
        assert_eq!(published.expected().graph().spec().generation, 2);
        assert!(!peers.socket(2).exists());
    }
}

#[test]
fn ordinary_begin_reset_keeps_ready_deadline_and_exact_incident_intent() {
    let root = Root::new(); let case = prepared_control(&root, false, false);
    let original = disk(&case.case);
    let request = checkpoint_control::Request::Begin(case.case.request.clone());
    let error = recovery::stream_checkpoint::execute_paired(&case.case.config, &case.case.loaded,
        &request, &mut |progress| {
            if progress.is_some_and(|value| value.status == FileLearnedRecoveryStatus::Ready) {
                return Err("original deadline before reset intent".into());
            }
            Ok(())
        }).unwrap_err();
    assert_eq!(error, "original deadline before reset intent");
    assert_eq!(disk(&case.case), original);
    assert!(!root.0.join("source-reset.bin").exists());
    assert!(!case.case.loaded.anchor_output.as_ref().unwrap().exists());
    let mut wrong = case.case.request.clone();
    wrong.first_incident_sequence += 1;
    assert!(recovery::stream_checkpoint::execute_paired(&case.case.config, &case.case.loaded,
        &checkpoint_control::Request::Begin(wrong), &mut |_| Ok(())).is_err());
    assert_eq!(disk(&case.case), original);
    let mut command = arguments(&root, &case); command[3] = "--begin-stream-reset".into();
    let mut output = Vec::new();
    checkpoint_control::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    assert_eq!(control_report(&output).get("outcome").and_then(Json::as_str), Some("pending"));
    assert_eq!(snapshot(&case.case).revision, case.case.before.revision + 1);
}
