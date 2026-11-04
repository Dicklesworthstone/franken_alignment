//! Explicit recovery metadata through real operator entry points. A completed
//! native reset must support a fresh numerical request and both publication
//! keys, while the declaration alone cannot upgrade a different history.
use super::*;
use fa_reference::action::consequence::delivery::{
    persistent::observed::{
        decoder::learned::sidecar::FileLearnedSidecarFinish,
        replay::FileReviewReplayStatus,
        reviewer::wire::ReviewDecision,
    },
    stream::ReleaseFrame,
};
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use std::os::unix::net::UnixStream;

fn completed(root: &Root) -> (Case, Vec<String>, Json) {
    let case = super::prepared(root, false, false);
    let arguments = super::args(root, &case);
    let mut output = Vec::new();
    reset::command(&arguments, None, || ElapsedTick(1000), &mut output).unwrap();
    let result = report(&output);
    assert_eq!(result.get("restored").and_then(Json::as_bool), Some(true));
    assert_eq!(case.numerical.cumulative_work.sampling_attempts, 2);
    (case, arguments, result)
}

fn fresh_request(root: &Root, case: &Case) -> PathBuf {
    let original = fs::read_to_string(&case.recipe).unwrap();
    let next = original
        .replacen("\"request\":1", "\"request\":2", 1)
        .replacen("\"object_id\":1001", "\"object_id\":1002", 1)
        .replacen("\"round_ids\":[1]", "\"round_ids\":[2]", 1);
    let path = root.0.join("request-2.json");
    fs::write(&path, next).unwrap();
    let loaded = recipe::load(&path, &case.config, true).unwrap();
    assert_eq!(loaded.request, 2);
    assert_eq!(loaded.rounds, [2]);
    assert_eq!(loaded.generation, case.loaded.generation);
    assert_eq!(loaded.stream, case.loaded.stream);
    assert!(loaded.native.is_some() && loaded.process.is_none());
    // Numerical allowance was provisioned before the abandoned generation.
    // Changing request identity cannot refill its four-draw lifetime budget.
    assert!(original.contains("\"max_new_tokens\":4"));
    assert!(original.contains("\"vocabulary_scores\":1028"));
    path
}

fn peer_file(root: &Root, config: &Config) -> (PathBuf, PeerProfile) {
    let (socket, _client) = UnixStream::pair().unwrap();
    let p = PeerCredentials::observe(&socket).unwrap();
    let s = config.profile.delivery.scope;
    let text = format!(
        r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":{},"socket_directory":"{}/peers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority,
        config.profile.human.reviewer_id, root.0.display(),
        p.uid(), p.gid(), p.pid(), p.uid(), p.gid(), p.pid(),
    );
    let path = root.0.join("independent-reviewer.json");
    fs::write(&path, text).unwrap();
    let peers = PeerProfile::read(&path).unwrap();
    peers.check_host(config).unwrap();
    (path, peers)
}

fn live_args(root: &Root, case: &Case, reset_args: &[String]) -> (Vec<String>, PeerProfile) {
    let request = fresh_request(root, case);
    let (peer_path, peers) = peer_file(root, &case.config);
    (
        vec![
            "create-learned-generated".into(), reset_args[1].clone(),
            request.display().to_string(), peer_path.display().to_string(),
            "--continue-generation".into(), "--history-anchor".into(),
            reset_args[8].clone(), "--retain-history-anchor".into(),
            root.0.join("publication.anchor").display().to_string(),
        ],
        peers,
    )
}

#[test]
fn completed_reset_can_publish_a_fresh_cli_request_then_recover_its_original_receipt() {
    let root = Root::new();
    let (case, reset_args, restored) = completed(&root);
    let (arguments, peers) = live_args(&root, &case, &reset_args);
    let reset_anchor = fs::read(&reset_args[8]).unwrap();
    let mut human = None;
    let result = super::super::super::command_with_clock(&arguments, None, || {
        if human.is_none() && peers.socket(2).exists() {
            human = Some(super::super::peer_intake::human(
                peers.clone(), 2, ReviewDecision::Approve,
            ));
        }
        ElapsedTick(1001)
    });
    let packet = human.expect("fresh request reached its authenticated human endpoint")
        .join().unwrap();
    result.unwrap();
    assert_eq!(packet.created_at(), ElapsedTick(1001));
    assert_eq!(packet.views().len(), 2);
    assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
    assert_eq!(fs::read(&reset_args[8]).unwrap(), reset_anchor);
    assert!(!peers.socket(2).exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    let published = snapshot(&case);
    assert_eq!(published.payload, b"A");
    assert_eq!(published.executions, 1);
    assert_eq!(published.control.ledger.reserved, 0);
    assert_eq!(published.control.ledger.charged,
        case.before.control.ledger.charged + packet.action().spec().units);

    let mut loaded = recipe::load(Path::new(&arguments[2]), &case.config, false).unwrap();
    loaded.recovery_anchor = Some(custody::load(Path::new(&arguments[8])).unwrap());
    let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 4);
    assert_eq!(numerical.sampled_draws, 2);
    assert_eq!(numerical.cumulative_work.sampling_attempts,
        case.numerical.cumulative_work.sampling_attempts + 2);
    assert_eq!(numerical.cumulative_work.reserved_vocabulary_scores, 1028);
    assert!(numerical.cumulative_work.admitted_tokens > case.numerical.cumulative_work.admitted_tokens);
    let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(usage.restart_attempts, 1);
    assert_eq!(usage.reserved_cache_values,
        restored.get("reserved_cache_values").and_then(Json::as_str).unwrap().parse::<u64>().unwrap());
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(host.request_action(2).unwrap(), packet.action());

    // The new automatic key comes from the actual fresh native sidecar round.
    // No ordinary helper executable exists at the serialized /operator paths.
    let native = host.learned_sidecar_outcome(2).unwrap();
    assert_eq!(native.round, 2);
    assert!(native.actor_revision > case.numerical.actor_revision);
    let FileLearnedSidecarFinish::Applied { receipt, archive, .. } = native.result else {
        panic!("fresh native review did not apply its original decision");
    };
    assert!(receipt.is_ok());
    assert_eq!(archive.inputs.action(), packet.action());
    assert_eq!(archive.inputs.views().len(), 2);
    assert_eq!(archive.commit_times.len(), 2);
    assert_eq!(archive.reveal_times.len(), 2);
    assert_eq!(archive.started_at, ElapsedTick(1001));
    assert!(archive.input_revision > host.review_replay(1).unwrap().archive().input_revision);
    let mut cursor = host.begin_review_replay(2).unwrap();
    while cursor.progress().status == FileReviewReplayStatus::Replaying {
        cursor.advance(cursor.progress().replayed_events, 1).unwrap();
    }
    let audit = cursor.finish_with_effects().unwrap();
    assert!(audit.effect().selected_review_dispatched());
    assert_eq!(audit.effect().dispatch().unwrap().authorization().round(), 2);
    assert!(audit.effect().outcome().is_some() && audit.effect().reconciliation().is_some());
    drop(host);

    // Both executable recovery paths must work after all present-day helper
    // material disappears. Receipt inspection preserves even unrelated staging.
    for file in ["evidence.json", "native-roster.json", "helper-model.json",
        "helper-weights.safetensors", "helper-monitor.json", "helper-tokenizer.bbpe",
        "helper-sampling.json"] {
        fs::remove_file(root.0.join(file)).unwrap();
    }
    let mut receipt = vec![
        "create-learned-generated".into(), arguments[1].clone(), arguments[2].clone(),
        "--resume".into(), "--receipts-only".into(), "--history-anchor".into(),
        arguments[8].clone(),
    ];
    let staging = case.config.store.join("delivery.pending");
    fs::write(&staging, b"read-only receipt preserves staging").unwrap();
    let before = disk(&case);
    let before_state = snapshot(&case);
    crate::command_with_output(receipt.clone(), &mut Vec::new()).unwrap();
    assert_eq!(disk(&case), before);
    assert_eq!(fs::read(&staging).unwrap(), b"read-only receipt preserves staging");
    receipt.remove(4);
    crate::command_with_output(receipt, &mut Vec::new()).unwrap();
    let recovered = snapshot(&case);
    assert_eq!(recovered.revision, before_state.revision + 1);
    assert_eq!(recovered.executions, 1);
    assert_eq!(recovered.control.ledger.charged, before_state.control.ledger.charged);
    assert_eq!(recovered.payload, published.payload);
    assert!(!staging.exists());
}

#[test]
fn independently_declared_profile_mismatches_cannot_fence_or_clean_reset_history() {
    let root = Root::new();
    let (case, reset_args, _) = completed(&root);
    let (arguments, _) = live_args(&root, &case, &reset_args);
    let config = fs::read_to_string(&arguments[1]).unwrap();
    let before = disk(&case);
    let staging = case.config.store.join("delivery.pending");
    fs::write(&staging, b"must survive wrong recovery expectation").unwrap();
    for changed in [
        config.replace("\"functional_restart\"", "\"audit_only\""),
        config.replace("\"model_generation\": 1", "\"model_generation\": 2"),
        config.replace("\"functional_restart\"", "\"bit_exact\""),
    ] {
        assert_ne!(changed, config);
        fs::write(&arguments[1], changed).unwrap();
        assert!(super::super::super::command_with_clock(&arguments, None, || ElapsedTick(1001)).is_err());
        assert_eq!(disk(&case), before);
        assert_eq!(fs::read(&staging).unwrap(), b"must survive wrong recovery expectation");
        assert!(!Path::new(&arguments[8]).exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn completed_functional_history_accepts_terminal_admission_without_current_evidence() {
    let root = Root::new();
    let (case, arguments, _) = completed(&root);
    let before = snapshot(&case);
    let reset_before = FileOversight::read_learned_reset_result(
        &case.config.store, &case.config.profile, &case.loaded.generation, 900,
    ).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    fs::remove_file(root.0.join("native-roster.json")).unwrap();
    let terminal_anchor = root.0.join("terminal.anchor");
    crate::command_with_output(vec![
        "create-learned-generated".into(), arguments[1].clone(), arguments[2].clone(),
        "--stop-admission".into(), "901".into(), "--history-anchor".into(),
        arguments[8].clone(), "--retain-history-anchor".into(),
        terminal_anchor.display().to_string(),
    ], &mut Vec::new()).unwrap();
    let after = snapshot(&case);
    assert_eq!(after.stop.as_ref().unwrap().request().operation, 901);
    assert_eq!(after.revision, before.revision + 2);
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(custody::load(&terminal_anchor).unwrap().revision(), after.revision);
    let reset_after = FileOversight::read_learned_reset_result(
        &case.config.store, &case.config.profile, &case.loaded.generation, 900,
    ).unwrap();
    let (FileLearnedResetRecord::Completed { intent: first, result: Ok(first_result) },
        FileLearnedResetRecord::Completed { intent: last, result: Ok(last_result) }) =
        (reset_before.record(), reset_after.record()) else { panic!("original reset result lost"); };
    assert_eq!(first, last);
    assert!(first_result.control.restored && last_result.control.restored);
    assert_eq!(first_result.actor_revision, last_result.actor_revision);
    assert_eq!(first_result.position, last_result.position);
    assert_eq!(first_result.sampled_draws, last_result.sampled_draws);
}

#[test]
fn learned_and_resident_creation_cannot_replace_history_and_generic_profiles_remain_audit_only() {
    let root = Root::new();
    let (case, arguments, _) = completed(&root);
    let (peer, _) = peer_file(&root, &case.config);
    let before = disk(&case);
    assert!(Config::read(Path::new(&arguments[1])).is_err());
    let create = vec!["create-learned-generated".into(), arguments[1].clone(),
        arguments[2].clone(), peer.display().to_string()];
    // The explicit learned constructor can now provision a new functional
    // owner, but a create invocation can never replace this existing history.
    assert!(super::super::super::command_with_clock(&create, None, || ElapsedTick(1001)).is_err());
    assert_eq!(disk(&case), before);
    // A fully admitted resident declaration now accepts the functional grade,
    // but must reach the same create-new storage refusal instead of reopening
    // this completed reset or replacing its original canonical bytes.
    let next = fresh_request(&root, &case);
    let mut entries = Vec::new();
    for (request, recipe, release) in [(1, &case.recipe, "message"), (2, &next, "finish")] {
        let mut actor = super::super::peer_intake::actor_profile(&root, &case.config, request);
        actor.socket = root.0.join(format!("peers/resident-{request}.sock"));
        let scope = actor.scope;
        let audience = format!(r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
            scope.tenant, scope.principal, scope.run, scope.branch, scope.authority);
        let identity = format!(r#"{{"uid":{},"gid":{},"pid":{}}}"#,
            actor.actor.uid(), actor.actor.gid(), actor.actor.pid().unwrap());
        let path = root.0.join(format!("resident-actor-{request}.json"));
        fs::write(&path, format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":{request},"scope":{audience},"socket":"{}","supervisor":{identity},"actor":{identity},"candidate_limit":4,"connection_limit":4,"exchange_limit":4096,"runtime_ms":60000,"poll_ms":1,"reply_ms":500}}"#,
            actor.socket.display())).unwrap();
        entries.push(format!(r#"{{"recipe":"{}","actor_profile":"{}","release":"{release}"}}"#,
            recipe.display(), path.display()));
    }
    let sequence = root.0.join("resident-sequence.json");
    fs::write(&sequence, format!(r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{}]}}"#,
        entries.join(","))).unwrap();
    let resident = vec!["serve-learned-generated".into(), arguments[1].clone(),
        sequence.display().to_string(), peer.display().to_string()];
    let clocks = Cell::new(0);
    let error = super::super::super::sequence::command_with_clock(
        &resident, None, || { clocks.set(clocks.get() + 1); ElapsedTick(1001) },
    ).unwrap_err();
    assert!(clocks.get() > 0, "the complete functional declaration passed admission");
    assert!(error.contains("AlreadyExists"), "{error}");
    assert_eq!(disk(&case), before);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[test]
fn recovery_declaration_cannot_upgrade_an_original_audit_only_journal() {
    let root = Root::new();
    let mut config = configured(&root);
    let path = super::super::continuation::write(&root);
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer, identity) = recovery::create_selected(&config, &loaded).unwrap();
    let anchor_path = root.0.join("audit-only.anchor");
    let mut anchor = Vec::new();
    host.history_anchor().unwrap().write_to(&mut anchor, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    fs::write(&anchor_path, anchor).unwrap();
    drop((host, reviewer, observer, identity));
    let canonical = config.store.join("delivery.bin");
    let before = fs::read(&canonical).unwrap();
    let staging = config.store.join("delivery.pending");
    fs::write(&staging, b"audit-only original must not be upgraded").unwrap();
    // Change only the independently supplied expectation, never stored history.
    let actor = &config.profile.delivery.actor;
    let mut declared = actor.profile();
    declared.grade = RestartGrade::FunctionalRestart;
    config.profile.delivery.actor = ActorState::new(declared, actor.tokens().to_vec(),
        actor.cache().to_vec(), actor.sampler().to_vec(), actor.next_position()).unwrap();
    let config_path = config_file(&root, &config);
    let (peers, _) = peer_file(&root, &config);
    let output = root.0.join("must-not-retain.anchor");
    let arguments = vec![
        "create-learned-generated".into(), config_path.display().to_string(),
        path.display().to_string(), peers.display().to_string(),
        "--continue-generation".into(), "--history-anchor".into(),
        anchor_path.display().to_string(), "--retain-history-anchor".into(),
        output.display().to_string(),
    ];
    assert!(super::super::super::command_with_clock(&arguments, None, || ElapsedTick(1001)).is_err());
    assert_eq!(fs::read(&canonical).unwrap(), before);
    assert_eq!(fs::read(&staging).unwrap(), b"audit-only original must not be upgraded");
    assert!(!output.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}
