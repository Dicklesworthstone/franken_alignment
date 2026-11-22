//! Real recovered schedules use the original source, request journal and keys.
//! Fixed helper votes and explicit human choices test enforcement, not quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{
        PrecheckRequest, PrecheckWireError, PrecheckWireResponse, client::PrecheckPeerClient,
    },
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;

fn profile_path(root: &Root, request: u64) -> PathBuf {
    root.path(&format!("precheck-{request}.json"))
}

fn endpoint(root: &Root, request: u64) -> PathBuf {
    root.path(&format!("actors/precheck-{request}.sock"))
}

fn schedule(root: &Root, rows: &[(u64, Option<u64>)]) {
    let entries = rows.iter().map(|(request, epoch)| {
        let profile = if let Some(epoch) = epoch {
            let path = profile_path(root, *request);
            fs::write(&path, format!(
                r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":{epoch},"disclosure_id":{},"query_limit":1,"exchange_limit":4,"candidate_limit":4,"connection_limit":2,"runtime_ms":15000}}"#,
                endpoint(root, *request).display(), 80 + request,
            )).unwrap();
            format!("\"{}\"", path.display())
        } else { "null".into() };
        format!(r#"{{"request":{request},"precheck_profile":{profile}}}"#)
    }).collect::<Vec<_>>().join(",");
    fs::write(
        root.path("precheck-schedule.json"),
        format!(r#"{{"schema":"fa.actor-precheck-schedule/1","entries":[{entries}]}}"#,),
    )
    .unwrap();
}

fn args(root: &Root, checked: bool, requests: &str, operator_selected: bool) -> Vec<String> {
    let mut args = arguments(root, checked, true, operator_selected);
    args.extend([
        "--requests".into(),
        requests.into(),
        "--precheck-schedule".into(),
        root.path("precheck-schedule.json").display().to_string(),
    ]);
    args
}

fn candidate(config: &Config, version: u64) -> ActorProposal {
    let mut target = config.profile.delivery.target;
    target.expected_version = version;
    ActorProposal {
        target,
        payload: b"second".to_vec(),
        units: 6,
        deadline: ElapsedTick(100000),
        expected_policy_epoch: 3,
    }
}

fn query(actor: &Profile, socket: &Path, proposal: &ActorProposal) -> PrecheckWireResponse {
    let mut client = PrecheckPeerClient::connect(
        connect(socket),
        actor.supervisor,
        PrecheckRequest {
            query: 7001,
            proposal: proposal.clone(),
        },
        proposal.expected_policy_epoch,
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() {
            return response;
        }
        assert!(!report.status.closed());
        pause(1);
    }
}

fn passed(response: PrecheckWireResponse, request: u64) {
    let Ok(Knowledge::Known { value, basis }) = response.result else {
        panic!("missing original recovered disclosure");
    };
    assert_eq!(value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!(
        (basis.query, basis.disclosure, basis.policy_epoch),
        (7001, 80 + request, 3)
    );
    assert_eq!(basis.observed_at, ElapsedTick(1001));
}

fn human(
    profile: PeerProfile,
    request: u64,
    endpoint: PathBuf,
    reject: bool,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = profile
            .connect_client_waiting(request, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        let mut seen = false;
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(!seen);
                    seen = true;
                    assert!(!endpoint.exists(), "disclosure survived into human review");
                    assert_eq!(client.packet().unwrap().action().spec().policy_epoch, 3);
                    client
                        .respond(if reject {
                            ReviewDecision::Reject
                        } else {
                            ReviewDecision::Approve
                        })
                        .unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

#[test]
fn recovered_scheduled_disclosures_publish_with_both_keys_and_respect_later_human_rejection() {
    for checked in [false, true] {
        for reject in [false, true] {
            let root = Root::new();
            let (_, reviewer, before) = seed(&root, checked, 32);
            let config = helper_epoch(&root, 3);
            let actor = scheduled_actor(&root);
            next_graph(&root, 2, 32);
            schedule(&root, &[(8, Some(3)), (9, Some(3))]);
            let third = checked.then(|| {
                second_captures(&root, &config);
                third_capture(&config).to_bytes().unwrap()
            });
            let captures = [root.path("original.bin"), root.path("current.bin")];
            let sockets = [endpoint(&root, 8), endpoint(&root, 9)];
            let checks = [candidate(&config, 2), candidate(&config, 3)];
            let proposals = [proposal(&config, 8, 3, 2), proposal(&config, 9, 3, 3)];
            let polls = [
                request_document(&config, 8, "get"),
                request_document(&config, 9, "get"),
            ];
            let peer_actor = actor.clone();
            let peer_sockets = sockets.clone();
            let store = config.store.clone();
            let bootstrap = config.profile.clone();
            let peer = std::thread::spawn(move || {
                let mut actor_socket = connect(&peer_actor.socket);
                peer_actor.supervisor.verify(&actor_socket).unwrap();
                for index in 0..2 {
                    wait(&peer_sockets[index]);
                    if index == 1 {
                        assert!(!peer_sockets[0].exists());
                        if let Some(bytes) = &third {
                            for path in &captures {
                                replace_capture(path, bytes);
                            }
                        }
                    }
                    let prior = FileOversight::read_publication(&store, &bootstrap).unwrap();
                    passed(
                        query(&peer_actor, &peer_sockets[index], &checks[index]),
                        8 + index as u64,
                    );
                    assert_eq!(
                        query(&peer_actor, &peer_sockets[index], &checks[index]).result,
                        Err(PrecheckWireError::Capacity)
                    );
                    assert_eq!(
                        FileOversight::read_publication(&store, &bootstrap).unwrap(),
                        prior
                    );
                    let reply = finish(&mut actor_socket, &proposals[index], &polls[index]);
                    if index == 1 && reject {
                        assert_eq!(field(knowledge(&reply), "state").as_str(), Some("known"));
                        assert_ne!(field(knowledge(&reply), "value").as_str(), Some("executed"));
                    } else {
                        assert_executed(&reply);
                    }
                    assert!(!peer_sockets[index].exists());
                }
            });
            let first_human = human(reviewer.clone(), 8, sockets[0].clone(), false);
            let second_human = human(reviewer.clone(), 9, sockets[1].clone(), reject);
            let result = command_with_clock(&args(&root, checked, "8,9", false), None, || {
                ElapsedTick(1001)
            });
            let peer_result = peer.join();
            let first_result = first_human.join();
            let second_result = second_human.join();
            assert!(
                result.is_ok(),
                "checked={checked}, reject={reject}: {result:?}"
            );
            peer_result.unwrap();
            first_result.unwrap();
            second_result.unwrap();
            let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
            assert_eq!(state.executions, if reject { 2 } else { 3 });
            assert_eq!(
                (state.control.ledger.charged, state.control.ledger.reserved),
                (if reject { 13 } else { 19 }, 0)
            );
            assert_eq!(
                state.control.ledger.available + state.control.ledger.charged,
                config.profile.delivery.total
            );
            assert_eq!(
                state.control.ledger.stages[&3],
                if reject {
                    ActionState::Cancelled
                } else {
                    ActionState::Confirmed
                }
            );
            assert_eq!(state.control.ledger.epoch, 3);
            assert!(state.stop.is_none());
            let after = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
            assert_eq!(after.history_anchor().revision(), state.revision);
            assert_eq!(
                after.work().checks,
                before.work().checks + if reject { 2 } else { 3 }
            );
            assert_eq!(
                after.work().reserved_read_bytes,
                after.work().checks * TOPOLOGY_REFRESH_READ_BYTES
            );
            assert_eq!(read_checkpoint(&root, "first-checkpoint.bin"), before);
            assert!(!actor.socket.exists());
            assert!(sockets.iter().all(|path| !path.exists()));
        }
    }
}

#[test]
fn recovered_mixed_schedule_observes_null_history_and_discloses_only_new_work() {
    let root = Root::new();
    let (_, reviewer, before) = seed(&root, false, 32);
    let config = helper_epoch(&root, 3);
    let actor = one_connection_actor(&root, 7);
    next_graph(&root, 2, 32);
    schedule(&root, &[(7, None), (8, Some(3))]);
    let historical = proposal(&config, 7, 0, 1);
    let fresh = proposal(&config, 8, 3, 2);
    let historical_poll = request_document(&config, 7, "get");
    let fresh_poll = request_document(&config, 8, "get");
    let selected = endpoint(&root, 8);
    let historical_socket = endpoint(&root, 7);
    let proposed = candidate(&config, 2);
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_actor.socket);
        peer_actor.supervisor.verify(&socket).unwrap();
        assert_executed(&finish(&mut socket, &historical, &historical_poll));
        passed(query(&peer_actor, &selected, &proposed), 8);
        assert!(!historical_socket.exists());
        assert_executed(&finish(&mut socket, &fresh, &fresh_poll));
        assert_executed(&exchange(&mut socket, &historical));
    });
    let reviewer = human(reviewer, 8, endpoint(&root, 8), false);
    let result = command_with_clock(&args(&root, false, "7,8", false), None, || {
        ElapsedTick(1001)
    });
    peer.join().unwrap();
    reviewer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(
        (
            state.executions,
            state.control.ledger.charged,
            state.control.ledger.reserved
        ),
        (2, 13, 0)
    );
    assert_eq!(state.control.ledger.stages.len(), 2);
    let saved = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work().checks, before.work().checks + 2);
}

#[test]
fn recovered_null_historical_rows_need_no_source_capsule_helper_or_new_disclosure() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, true, 32);
    let actor = one_connection_actor(&root, 7);
    schedule(&root, &[(7, None)]);
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
    let old = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let submit = proposal(&config, 7, 0, 1);
    let poll = request_document(&config, 7, "get");
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_actor.socket);
        peer_actor.supervisor.verify(&socket).unwrap();
        assert_executed(&finish(&mut socket, &submit, &poll));
    });
    let result = command_with_clock(
        &args(&root, true, "7", false),
        Some(&root.path("absent-credibility")),
        || ElapsedTick(1001),
    );
    peer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(
        (
            state.executions,
            state.control.ledger.charged,
            state.control.ledger.reserved
        ),
        (1, 7, 0)
    );
    assert_eq!(state.control.ledger.epoch, old.control.ledger.epoch + 1);
    assert!(state.stop.is_none());
    let saved = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work(), before.work());
    assert_eq!(saved.last_operation(), before.last_operation());
    assert!(!saved.available());
    assert!(!reviewer.socket(7).exists());
    assert!(!endpoint(&root, 7).exists());
}

#[test]
fn recovered_non_null_historical_row_refuses_before_rebind_and_retains_the_fence_pair() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, true, 32);
    let actor = one_connection_actor(&root, 7);
    // The other row really is new: it must not hide an invalid historical selection.
    schedule(&root, &[(7, Some(2)), (8, Some(3))]);
    for name in [
        "topology.json",
        "inventory.json",
        "evidence.json",
        "original.bin",
        "current.bin",
    ] {
        fs::remove_file(root.path(name)).unwrap();
    }
    let old = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let error = command_with_clock(&args(&root, true, "7,8", false), None, || ElapsedTick(1001))
        .unwrap_err();
    assert!(
        error.contains(
            "recorded requests are observation-only and cannot acquire a new precheck disclosure"
        ),
        "{error}"
    );
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, old.executions);
    assert_eq!(state.control.ledger.charged, old.control.ledger.charged);
    assert_eq!(state.control.ledger.stages, old.control.ledger.stages);
    assert_eq!(state.control.ledger.epoch, old.control.ledger.epoch + 1);
    assert!(state.stop.is_none());
    let saved = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work(), before.work());
    assert_eq!(saved.last_operation(), before.last_operation());
    assert_eq!(saved.topology_bytes(), before.topology_bytes());
    assert_eq!(saved.inventory_bytes(), before.inventory_bytes());
    for request in [7, 8] {
        assert!(!endpoint(&root, request).exists());
        assert!(!reviewer.socket(request).exists());
    }
    assert!(!actor.socket.exists());
}

#[test]
fn recovered_later_disclosure_does_not_infer_or_wait_for_an_unselected_policy_epoch() {
    let root = Root::new();
    let (_, reviewer, before) = seed(&root, false, 32);
    let config = helper_epoch(&root, 3);
    let actor = scheduled_actor(&root);
    next_graph(&root, 2, 32);
    schedule(&root, &[(8, Some(3)), (9, Some(4))]);
    let selected = endpoint(&root, 8);
    let proposed = candidate(&config, 2);
    let submit = proposal(&config, 8, 3, 2);
    let poll = request_document(&config, 8, "get");
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        passed(query(&peer_actor, &selected, &proposed), 8);
        let mut socket = connect(&peer_actor.socket);
        peer_actor.supervisor.verify(&socket).unwrap();
        assert_executed(&finish(&mut socket, &submit, &poll));
    });
    let reviewer = human(reviewer, 8, endpoint(&root, 8), false);
    let result = command_with_clock(&args(&root, false, "8,9", false), None, || {
        ElapsedTick(1001)
    });
    peer.join().unwrap();
    reviewer.join().unwrap();
    let error = result.unwrap_err();
    assert!(
        error.contains("precheck expected policy epoch does not match the original owner"),
        "{error}"
    );
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(
        (
            state.executions,
            state.control.ledger.charged,
            state.control.ledger.reserved
        ),
        (2, 13, 0)
    );
    assert_eq!(state.control.ledger.stages.len(), 2);
    assert_eq!(state.stop.unwrap().request().operation, 9);
    let saved = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work().checks, before.work().checks + 2);
    assert!(!endpoint(&root, 9).exists());
    assert!(!actor.socket.exists());
}

#[test]
fn recovered_schedule_stop_retires_partial_disclosure_and_never_starts_the_next_row() {
    let root = Root::new();
    let (config, reviewer, before) = seed(&root, true, 32);
    let actor = actor_profile(&root, 8);
    next_graph(&root, 2, 32);
    schedule(&root, &[(8, Some(3)), (9, Some(3))]);
    let selected = endpoint(&root, 8);
    let later = endpoint(&root, 9);
    let evidence = root.path("evidence.json");
    let inventory = root.path("inventory.json");
    let proposed = candidate(&config, 2);
    let submit = proposal(&config, 8, 3, 2);
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        passed(query(&peer_actor, &selected, &proposed), 8);
        let mut partial = connect(&selected);
        partial.write_all(b"{").unwrap();
        let mut actor_socket = connect(&peer_actor.socket);
        actor_socket
            .write_all(&submit.as_bytes()[..submit.len() / 2])
            .unwrap();
        fs::remove_file(evidence).unwrap();
        fs::remove_file(inventory).unwrap();
        let mut output = Vec::new();
        crate::peers::stop::request(&reviewer, 8, &mut output).unwrap();
        assert_eq!(
            field(&parse(&output), "status").as_str(),
            Some("stopped_drained")
        );
        assert!(!later.exists());
    });
    let result = command_with_clock(&args(&root, true, "8,9", false), None, || ElapsedTick(1001));
    peer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(
        (
            state.executions,
            state.control.ledger.charged,
            state.control.ledger.reserved
        ),
        (1, 7, 0)
    );
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert_eq!(state.stop.unwrap().request().operation, 8);
    let saved = retained_pair(&root, "second-checkpoint.bin", "second.anchor");
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work().checks, before.work().checks + 1);
    assert!(!actor.socket.exists());
    assert!(!endpoint(&root, 8).exists());
    assert!(!endpoint(&root, 9).exists());
}

#[test]
fn recovered_schedule_reserves_future_disclosures_and_profiles_before_replay() {
    let root = Root::new();
    let (config, reviewer, _) = seed(&root, true, 32);
    let actor = actor_profile(&root, 8);
    let operator = operator(&root, &config);
    let canonical = config.store.join("delivery.bin");
    let journal = fs::read(&canonical).unwrap();
    let staging = config.store.join("delivery.pending");
    fs::write(&staging, b"must survive preflight refusal").unwrap();
    std::os::unix::fs::symlink(&root.0, root.path("via")).unwrap();
    let destinations = vec![
        root.path("first.anchor"),
        root.path("second.anchor"),
        root.path("first-checkpoint.bin"),
        root.path("second-checkpoint.bin"),
        root.path("initial-topology.json"),
        root.path("initial-inventory.json"),
        root.path("topology.json"),
        root.path("inventory.json"),
        root.path("evidence.json"),
        root.path("original.bin"),
        root.path("current.bin"),
        actor.socket.clone(),
        reviewer.socket(8),
        reviewer.socket(9),
        crate::workflow::control::socket_path(&reviewer, 8),
        crate::workflow::control::socket_path(&reviewer, 9),
        operator.socket.clone(),
        profile_path(&root, 8),
        root.path("precheck-schedule.json"),
        root.path("guarded.json"),
        root.path("continuation.json"),
        root.path("config.json"),
        config.store.join("future-precheck.sock"),
    ];
    for alias in [false, true] {
        for destination in &destinations {
            schedule(&root, &[(8, Some(3)), (9, Some(3))]);
            let selected = if alias {
                root.path("via")
                    .join(destination.strip_prefix(&root.0).unwrap())
            } else {
                destination.clone()
            };
            let text = fs::read_to_string(profile_path(&root, 9)).unwrap().replace(
                &endpoint(&root, 9).display().to_string(),
                &selected.display().to_string(),
            );
            fs::write(profile_path(&root, 9), text).unwrap();
            let result = command_with_clock(&args(&root, true, "8,9", true), None, || {
                panic!("future row alias crossed into clock/replay: {destination:?}, alias={alias}")
            });
            assert!(result.is_err(), "{destination:?}, alias={alias}");
            assert_eq!(fs::read(&canonical).unwrap(), journal);
            assert_eq!(
                fs::read(&staging).unwrap(),
                b"must survive preflight refusal"
            );
            assert!(!root.path("second.anchor").exists());
            assert!(!root.path("second-checkpoint.bin").exists());
            assert!(!actor.socket.exists());
            assert!(!operator.socket.exists());
            assert!(!endpoint(&root, 8).exists());
            assert!(!endpoint(&root, 9).exists());
        }
    }
}

#[test]
fn recovered_schedule_keeps_removed_later_profile_names_out_of_acknowledged_outputs() {
    use crate::recover_stop::guarded::SubmitProfile;
    use crate::workflow::actor_service::precheck::schedule::Schedule;
    use crate::workflow::actor_service::topology::recovery as service;
    use crate::workflow::continuation::guarded::mediated::profile::Selection;

    let root = Root::new();
    let (config, reviewer, before) = seed(&root, false, 32);
    let actor = actor_profile(&root, 8);
    let original = fs::read(config.store.join("delivery.bin")).unwrap();
    for source_output in [false, true] {
        schedule(&root, &[(8, Some(3)), (9, Some(3))]);
        let selected = Schedule::read(&root.path("precheck-schedule.json"), &[8, 9]).unwrap();
        fs::remove_file(profile_path(&root, 9)).unwrap();
        guard_profile(
            &root,
            &before,
            if source_output {
                "second.anchor"
            } else {
                "precheck-9.json"
            },
        );
        continuation_profile(
            &root,
            Some(if source_output {
                "precheck-9.json"
            } else {
                "second-checkpoint.bin"
            }),
        );
        let recovery = SubmitProfile::read(&root.path("guarded.json")).unwrap();
        let topology = Selection::read(&root.path("continuation.json")).unwrap();
        let error = service::serve_sequence(
            Config::read(&root.path("config.json")).unwrap(),
            &actor,
            &reviewer,
            None,
            None,
            &recovery,
            &topology,
            GENERATION,
            None,
            &[8, 9],
            Some(&selected),
            || panic!("a loaded then removed profile became recovery output custody"),
        )
        .unwrap_err();
        assert!(
            error.contains("custody"),
            "source_output={source_output}: {error}"
        );
        assert_eq!(
            fs::read(config.store.join("delivery.bin")).unwrap(),
            original
        );
        assert!(!profile_path(&root, 9).exists());
        assert!(!root.path("second.anchor").exists());
        assert!(!root.path("second-checkpoint.bin").exists());
        assert!(!actor.socket.exists());
        assert!(!endpoint(&root, 8).exists());
        assert!(!endpoint(&root, 9).exists());
    }
}
