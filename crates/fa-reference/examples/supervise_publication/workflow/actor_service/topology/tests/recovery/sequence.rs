//! One anchored owner, one actor connection and one cumulative topology source
//! across scheduled effects. Synthetic helper votes test enforcement, not quality.
use super::*;

fn scheduled_arguments(root: &Root, checked: bool, requests: &str) -> Vec<String> {
    let mut args = arguments(root, checked, true, false);
    args.extend(["--requests".into(), requests.into()]);
    args
}

fn scheduled_actor(root: &Root) -> Profile {
    one_connection_actor(root, 8)
}

fn one_connection_actor(root: &Root, request: u64) -> Profile {
    actor_profile(root, request);
    let text = fs::read_to_string(root.path("actor.json"))
        .unwrap()
        .replace("\"connection_limit\":4", "\"connection_limit\":1")
        .replace("\"candidate_limit\":8", "\"candidate_limit\":1");
    fs::write(root.path("actor.json"), &text).unwrap();
    Profile::decode(text.as_bytes()).unwrap()
}

fn finish(socket: &mut UnixStream, submit: &str, poll: &str) -> Json {
    let response = exchange(socket, submit);
    if field(knowledge(&response), "state").as_str() == Some("pending") {
        terminal(socket, poll)
    } else {
        response
    }
}

fn assert_executed(response: &Json) {
    assert_eq!(field(knowledge(response), "state").as_str(), Some("known"));
    assert_eq!(
        field(knowledge(response), "value").as_str(),
        Some("executed")
    );
}

fn third_capture(config: &Config) -> FilePublicationCapture {
    let mut target = config.profile.delivery.target;
    target.expected_version = 3;
    let action = FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: config.profile.delivery.scope,
        target: Some(target),
        payload: b"second".to_vec(),
        required_witnesses: vec![ReadWitness::Exact {
            key: 7,
            value: Some(b"ok".to_vec()),
        }],
        policy_epoch: 3,
        deadline: ElapsedTick(100000),
        units: 6,
    })
    .unwrap();
    let key = ProjectionKey {
        source: 40,
        branch: action.spec().scope.branch,
        projection: 7,
        source_epoch: 1,
    };
    let close = TrustedClosingMarker {
        key,
        final_sequence: 1,
        marker_generation: 1,
    };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(key, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(
        4,
        4,
        20,
        AdapterDomainInput::new(
            DomainProjection::new(40, 1, key),
            DomainClosure::Closed(close),
        ),
        [0, 2, 4]
            .into_iter()
            .map(|key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap())
            .collect(),
        &frontiers,
    )
    .unwrap();
    FilePublicationCapture::new(
        3,
        FileCaptureIdentity {
            source: 91,
            generation: 4,
        },
        &action,
        FilePublicationInputs::new(Some(input), None),
    )
    .unwrap()
}

fn replace_capture(path: &Path, bytes: &[u8]) {
    let pending = path.with_extension("pending");
    fs::write(&pending, bytes).unwrap();
    fs::rename(pending, path).unwrap();
}

fn retained_pair(root: &Root, checkpoint: &str, anchor: &str) -> TopologyCheckpoint {
    let saved = read_checkpoint(root, checkpoint);
    let history = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(root.path(anchor)).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    assert_eq!(saved.history_anchor(), &history);
    saved
}

fn continuation_after(root: &Root, checkpoint: &TopologyCheckpoint, source: &str, output: &str) {
    continuation_from(root, source, Some(output));
    // Historical profile admission still checks the declared operation floor,
    // even when no next source will be opened or operation spent.
    let next = checkpoint.last_operation().checked_add(1).unwrap();
    let text = fs::read_to_string(root.path("continuation.json"))
        .unwrap()
        .replace("\"operation\":100", &format!("\"operation\":{next}"));
    fs::write(root.path("continuation.json"), text).unwrap();
}

fn publish_pair(root: &Root, checked: bool) -> (Config, PeerProfile, TopologyCheckpoint) {
    let (_, reviewer, before) = seed(root, checked, 32);
    let config = helper_epoch(root, 3);
    let actor = scheduled_actor(root);
    next_graph(root, 2, 32);
    let third = if checked {
        second_captures(root, &config);
        Some(third_capture(&config).to_bytes().unwrap())
    } else {
        None
    };
    let original = root.path("original.bin");
    let current = root.path("current.bin");
    let first = proposal(&config, 8, 3, 2);
    let second = proposal(&config, 9, 3, 3);
    let first_poll = request_document(&config, 8, "get");
    let second_poll = request_document(&config, 9, "get");
    let next_control = crate::workflow::control::socket_path(&reviewer, 9);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile.socket);
        profile.supervisor.verify(&socket).unwrap();
        assert_executed(&finish(&mut socket, &first, &first_poll));
        // This endpoint appears only after the first request's direct children
        // retire. The next actual capture binds native attempt three, not key 9.
        wait(&next_control);
        if let Some(bytes) = third {
            replace_capture(&original, &bytes);
            replace_capture(&current, &bytes);
        }
        assert_executed(&finish(&mut socket, &second, &second_poll));
        assert_executed(&exchange(&mut socket, &first));
        assert_executed(&exchange(&mut socket, &first_poll));
    });
    let first_human = review(reviewer.clone(), 8, None, None);
    let second_human = review(reviewer.clone(), 9, None, None);
    let result = command_with_clock(&scheduled_arguments(root, checked, "8,9"), None, || {
        ElapsedTick(1001)
    });
    peer.join().unwrap();
    first_human.join().unwrap();
    second_human.join().unwrap();
    assert!(result.is_ok(), "checked={checked}: {result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 3);
    assert_eq!(state.target.expected_version, 4);
    assert_eq!(state.payload, b"second");
    assert_eq!(state.control.ledger.stages.len(), 3);
    assert!(
        state
            .control
            .ledger
            .stages
            .values()
            .all(|stage| *stage == ActionState::Confirmed)
    );
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (19, 0)
    );
    assert_eq!(
        state.control.ledger.available + 19,
        config.profile.delivery.total
    );
    assert_eq!(state.control.ledger.epoch, 3);
    assert!(state.stop.is_none());
    let after = retained_pair(root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(after.history_anchor().revision(), state.revision);
    assert_eq!(after.work().checks, before.work().checks + 3);
    assert_eq!(
        after.work().reserved_read_bytes,
        before.work().reserved_read_bytes + 3 * TOPOLOGY_REFRESH_READ_BYTES
    );
    assert!(after.last_operation() > before.last_operation());
    assert_eq!(after.expected().graph().spec().generation, 2);
    assert_eq!(after.expected().graph().spec().inventory_generation, 2);
    assert!(!actor.socket.exists());
    for request in [8, 9] {
        assert!(!reviewer.socket(request).exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, request).exists());
    }
    (config, reviewer, after)
}

#[test]
fn recovered_plain_and_checked_schedules_publish_twice_on_the_same_owner_and_connection() {
    for checked in [false, true] {
        publish_pair(&Root::new(), checked);
    }
}

#[test]
fn mixed_recorded_and_new_schedule_rebinds_once_without_replaying_the_recorded_effect() {
    let root = Root::new();
    let (_, reviewer, before) = seed(&root, false, 32);
    let config = helper_epoch(&root, 3);
    let actor = one_connection_actor(&root, 7);
    next_graph(&root, 2, 32);
    let recorded = proposal(&config, 7, 0, 1);
    let fresh = proposal(&config, 8, 3, 2);
    let recorded_poll = request_document(&config, 7, "get");
    let fresh_poll = request_document(&config, 8, "get");
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile.socket);
        profile.supervisor.verify(&socket).unwrap();
        assert_executed(&finish(&mut socket, &recorded, &recorded_poll));
        wait(&next_control);
        assert_executed(&finish(&mut socket, &fresh, &fresh_poll));
        assert_executed(&exchange(&mut socket, &recorded));
    });
    // No new human or helper round is supplied for the historical request.
    let human = review(reviewer.clone(), 8, None, None);
    let result = command_with_clock(&scheduled_arguments(&root, false, "7,8"), None, || {
        ElapsedTick(1001)
    });
    peer.join().unwrap();
    human.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 2);
    assert_eq!(state.target.expected_version, 3);
    assert_eq!(state.control.ledger.stages.len(), 2);
    assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(state.control.ledger.stages[&2], ActionState::Confirmed);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (13, 0)
    );
    assert_eq!(
        state.control.ledger.available + 13,
        config.profile.delivery.total
    );
    assert_eq!(state.control.ledger.epoch, 3);
    assert!(state.stop.is_none());
    let after = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(after.history_anchor().revision(), state.revision);
    assert_eq!(after.work().checks, before.work().checks + 2);
    assert_eq!(
        after.work().reserved_read_bytes,
        before.work().reserved_read_bytes + 2 * TOPOLOGY_REFRESH_READ_BYTES
    );
    assert_eq!(after.expected().graph().spec().generation, 2);
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn recovered_schedule_rechecks_changed_input_and_conserves_source_work_at_the_later_effect() {
    for mode in ["funded", "exhausted", "changed"] {
        let checks = if mode == "exhausted" { 4 } else { 5 };
        let root = Root::new();
        let (_, reviewer, before) = seed(&root, false, checks);
        let config = helper_epoch(&root, 3);
        let actor = scheduled_actor(&root);
        next_graph(&root, 2, checks);
        let first = proposal(&config, 8, 3, 2);
        let second = proposal(&config, 9, 3, 3);
        let first_poll = request_document(&config, 8, "get");
        let second_poll = request_document(&config, 9, "get");
        let next_control = crate::workflow::control::socket_path(&reviewer, 9);
        let profile = actor.clone();
        let peer = std::thread::spawn(move || {
            let mut socket = connect(&profile.socket);
            profile.supervisor.verify(&socket).unwrap();
            assert_executed(&finish(&mut socket, &first, &first_poll));
            wait(&next_control);
            assert_eq!(
                field(knowledge(&exchange(&mut socket, &second)), "state").as_str(),
                Some("pending")
            );
            if mode == "funded" {
                assert_executed(&terminal(&mut socket, &second_poll));
            }
        });
        let first_human = review(reviewer.clone(), 8, None, None);
        let second_human = review(reviewer.clone(), 9, None, None);
        let mut dispatched = false;
        let result = command_with_clock(&scheduled_arguments(&root, false, "8,9"), None, || {
            if !dispatched
                && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
                && state.control.ledger.charged == 19
                && state.executions == 2
            {
                dispatched = true;
                if mode == "changed" {
                    // Change the actual source only AFTER both original keys
                    // and charged dispatch. No earlier review can catch this.
                    let text = fs::read_to_string(root.path("topology.json"))
                        .unwrap()
                        .replace("\"inventory_generation\":2", "\"inventory_generation\":1")
                        .replace(
                            "\"complete_inventory_generation\":2",
                            "\"complete_inventory_generation\":1",
                        );
                    fs::write(root.path("topology.json"), text).unwrap();
                }
            }
            ElapsedTick(1001)
        });
        peer.join().unwrap();
        first_human.join().unwrap();
        second_human.join().unwrap();
        assert!(dispatched, "{mode}: {result:?}");
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(
            state.executions,
            if mode == "funded" { 3 } else { 2 },
            "{mode}: {result:?}"
        );
        assert_eq!(
            state.target.expected_version,
            if mode == "funded" { 4 } else { 3 }
        );
        assert_eq!(state.payload, b"second");
        assert_eq!(state.control.ledger.stages.len(), 3);
        assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
        assert_eq!(state.control.ledger.stages[&2], ActionState::Confirmed);
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(
            state.control.ledger.available + state.control.ledger.charged,
            config.profile.delivery.total
        );
        if mode == "funded" {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(state.control.ledger.stages[&3], ActionState::Confirmed);
            assert_eq!(state.control.ledger.charged, 19);
        } else {
            match state.control.ledger.stages[&3] {
                ActionState::ConfirmedNotExecuted => assert_eq!(state.control.ledger.charged, 13),
                ActionState::Dispatching
                | ActionState::Unknown
                | ActionState::IrrecoverablyUnknown => {
                    assert_eq!(state.control.ledger.charged, 19);
                }
                stage => panic!("{mode}: forbidden later effect ended as {stage:?}: {result:?}"),
            }
        }
        let after = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
        assert_eq!(after.history_anchor().revision(), state.revision);
        assert_eq!(after.work().checks, checks);
        assert_eq!(
            after.work().reserved_read_bytes,
            checks * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert!(after.last_operation() > before.last_operation());
        assert_eq!(after.available(), mode == "funded");
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(9).exists());
    }
}

#[test]
fn recorded_schedule_retains_exact_source_custody_without_any_live_assets() {
    let root = Root::new();
    let (config, reviewer, mut before) = publish_pair(&root, true);
    for name in [
        "topology.json",
        "inventory.json",
        "evidence.json",
        "original.bin",
        "current.bin",
    ] {
        fs::remove_file(root.path(name)).unwrap();
    }
    let text = fs::read_to_string(root.path("config.json"))
        .unwrap()
        .replace(
            &format!(
                "{:?}",
                std::env::current_exe().unwrap().display().to_string()
            ),
            &format!("{:?}", root.path("absent-helper").display().to_string()),
        );
    fs::write(root.path("config.json"), text).unwrap();
    let absent_credibility = root.path("absent-credibility");
    let mut receipts: Option<Vec<Json>> = None;
    for (input, output, anchor, tick) in [
        (
            "second-checkpoint.bin",
            "third-checkpoint.bin",
            "third.anchor",
            1000000,
        ),
        (
            "third-checkpoint.bin",
            "fourth-checkpoint.bin",
            "fourth.anchor",
            1000001,
        ),
    ] {
        guard_profile(&root, &before, anchor);
        continuation_after(&root, &before, input, output);
        let actor = scheduled_actor(&root);
        let first = proposal(&config, 8, 3, 2);
        let second = proposal(&config, 9, 3, 3);
        let first_poll = request_document(&config, 8, "get");
        let second_poll = request_document(&config, 9, "get");
        let profile = actor.clone();
        let peer = std::thread::spawn(move || {
            let mut socket = connect(&profile.socket);
            profile.supervisor.verify(&socket).unwrap();
            let responses = [
                finish(&mut socket, &first, &first_poll),
                finish(&mut socket, &second, &second_poll),
            ];
            responses.iter().for_each(assert_executed);
            responses
                .iter()
                .map(|response| knowledge(response).clone())
                .collect::<Vec<_>>()
        });
        let result = command_with_clock(
            &scheduled_arguments(&root, true, "8,9"),
            Some(&absent_credibility),
            || ElapsedTick(tick),
        );
        let observed = peer.join().unwrap();
        assert!(result.is_ok(), "{input}: {result:?}");
        if let Some(original) = &receipts {
            assert_eq!(&observed, original);
        } else {
            receipts = Some(observed);
        }
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 3);
        assert_eq!(state.target.expected_version, 4);
        assert_eq!(state.payload, b"second");
        assert_eq!(state.control.ledger.stages.len(), 3);
        assert_eq!(
            (state.control.ledger.charged, state.control.ledger.reserved),
            (19, 0)
        );
        assert!(state.stop.is_none());
        let after = retained_pair(&root, output, anchor);
        assert_eq!(after.history_anchor().revision(), state.revision);
        assert!(after.history_anchor().revision() > before.history_anchor().revision());
        assert_eq!(after.work(), before.work());
        assert_eq!(after.last_operation(), before.last_operation());
        assert_eq!(after.topology_bytes(), before.topology_bytes());
        assert_eq!(after.inventory_bytes(), before.inventory_bytes());
        assert!(!after.available());
        for name in [
            "topology.json",
            "inventory.json",
            "evidence.json",
            "original.bin",
            "current.bin",
            "absent-helper",
            "absent-credibility",
        ] {
            assert!(!root.path(name).exists(), "{name}");
        }
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(8).exists());
        assert!(!reviewer.socket(9).exists());
        before = after;
    }
}

#[test]
fn unresolved_first_scheduled_dispatch_blocks_the_later_effect_and_cannot_refund_it() {
    let root = Root::new();
    let (_, reviewer, _) = seed(&root, false, 32);
    let config = helper_epoch(&root, 3);
    let actor = scheduled_actor(&root);
    next_graph(&root, 2, 32);
    let submit = proposal(&config, 8, 3, 2);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile.socket);
        profile.supervisor.verify(&socket).unwrap();
        assert_eq!(
            field(knowledge(&exchange(&mut socket, &submit)), "state").as_str(),
            Some("pending")
        );
    });
    let human = review(reviewer.clone(), 8, None, None);
    let mut expired = false;
    let result = command_with_clock(&scheduled_arguments(&root, false, "8,9"), None, || {
        if !expired
            && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
            && state.control.ledger.charged == 13
            && state.executions == 1
        {
            expired = true;
        }
        // The endpoint's own retention has expired after charged dispatch; a
        // native Stop sweep cannot manufacture evidence of nonexecution.
        ElapsedTick(if expired { 5_000_000 } else { 1001 })
    });
    peer.join().unwrap();
    human.join().unwrap();
    assert!(expired && result.is_err(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.stages.len(), 2);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (13, 0)
    );
    assert!(matches!(
        state.control.ledger.stages[&2],
        ActionState::Unknown | ActionState::IrrecoverablyUnknown
    ));
    let saved = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(saved.work().checks, 3);
    guard_profile(&root, &saved, "third.anchor");
    continuation_after(
        &root,
        &saved,
        "second-checkpoint.bin",
        "third-checkpoint.bin",
    );
    for name in ["topology.json", "inventory.json", "evidence.json"] {
        fs::remove_file(root.path(name)).unwrap();
    }
    // The first key is recorded; looking only at that key must not allow the
    // later new key to escape the original unsettled-delivery barrier.
    let error = command_with_clock(&scheduled_arguments(&root, false, "8,9"), None, || {
        ElapsedTick(5_000_001)
    })
    .unwrap_err();
    assert!(error.contains("unresolved prior deliveries"), "{error}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"network");
    assert_eq!(state.control.ledger.stages.len(), 2);
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (13, 0)
    );
    assert_eq!(
        state.control.ledger.available + 13,
        config.profile.delivery.total
    );
    let after = retained_pair(&root, "third-checkpoint.bin", "third.anchor");
    assert_eq!(after.history_anchor().revision(), state.revision);
    assert_eq!(after.work(), saved.work());
    assert_eq!(after.last_operation(), saved.last_operation());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(9).exists());
}

#[test]
fn recovered_schedule_rejects_archive_aliases_of_later_reviewer_and_stop_endpoints_before_time() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, false, 32);
    let actor = scheduled_actor(&root);
    let journal = fs::read(config.store.join("delivery.bin")).unwrap();
    let alias = root.path("reviewer-alias");
    std::os::unix::fs::symlink(root.path("reviewers"), &alias).unwrap();
    for checkpoint in [false, true] {
        for endpoint in [
            reviewer.socket(9),
            crate::workflow::control::socket_path(&reviewer, 9),
        ] {
            guard_profile(&root, &before, "second.anchor");
            continuation_profile(&root, Some("second-checkpoint.bin"));
            let alias = alias.join(endpoint.file_name().unwrap());
            let (profile, original) = if checkpoint {
                (
                    root.path("continuation.json"),
                    root.path("second-checkpoint.bin"),
                )
            } else {
                (root.path("guarded.json"), root.path("second.anchor"))
            };
            let text = fs::read_to_string(&profile).unwrap().replace(
                &format!("\"{}\"", original.display()),
                &format!("\"{}\"", alias.display()),
            );
            fs::write(profile, text).unwrap();
            let error = command_with_clock(&scheduled_arguments(&root, false, "8,9"), None, || {
                panic!("archive alias reached the clock or recovery")
            })
            .unwrap_err();
            assert!(
                error.contains("recovery archives") && error.contains("must be distinct"),
                "{error}"
            );
            assert_eq!(
                fs::read(config.store.join("delivery.bin")).unwrap(),
                journal
            );
            assert_eq!(read_checkpoint(&root, "first-checkpoint.bin"), before);
            assert!(!root.path("second.anchor").exists());
            assert!(!root.path("second-checkpoint.bin").exists());
            assert!(!endpoint.exists());
            assert!(!actor.socket.exists());
        }
    }
}
