//! Actual scheduled Tower ingress, original child helpers, independent humans
//! and publication effects. Synthetic helper verdicts are fixture inputs only.
use super::*;
use crate::workflow::actor_service::series::{self as service, Options};

pub(super) fn document(config: &Config, request: u64, version: u64, payload_hex: &str) -> String {
    proposal_document(config, request)
        .replace(
            &format!(
                "\"expected_version\":\"{}\"",
                config.profile.delivery.target.expected_version
            ),
            &format!("\"expected_version\":\"{version}\""),
        )
        .replace("6e6574776f726b", payload_hex)
}

pub(super) fn poll(config: &Config, request: u64) -> String {
    get_document(config).replace("-7@", &format!("-{request}@"))
}

pub(super) fn finish(socket: &mut UnixStream, proposal: &str, poll: &str) -> Json {
    let mut reply = raw_exchange(socket, proposal);
    let started = Instant::now();
    loop {
        let observed = knowledge(&reply);
        match field(observed, "state").as_str() {
            Some("pending") => {}
            Some("unknown")
                if observed.get("reason").and_then(Json::as_str) == Some("outcome_unknown") => {}
            _ => return reply,
        }
        assert!(started.elapsed() < Duration::from_secs(10));
        pause(2);
        reply = raw_exchange(socket, poll);
    }
}

pub(super) fn executed(reply: &Json) -> bool {
    let observed = knowledge(reply);
    field(observed, "state").as_str() == Some("known")
        && field(observed, "value").as_str() == Some("executed")
}

pub(super) fn human_for(profile: PeerProfile, request: u64) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = profile
            .connect_client_waiting(request, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

pub(super) fn stop_request(profile: &PeerProfile, request: u64) {
    wait(&crate::workflow::control::socket_path(profile, request));
    let mut output = Vec::new();
    crate::peers::stop::request(profile, request, &mut output).unwrap();
    let receipt = strict_json::parse(&output, Limits::default()).unwrap();
    assert_eq!(field(&receipt, "status").as_str(), Some("stopped_drained"));
}

pub(super) fn run_pair(root: &Directory) {
    let config = configured(root);
    evidence(root, &config);
    let source = root.0.join("evidence.json");
    let source_bytes = fs::read(&source).unwrap();
    fs::remove_file(&source).unwrap();
    let (mut actor, reviewer) = profiles(root, &config);
    // A replacement session or connection would fail this exact allowance.
    actor.connections = 1;
    actor.candidates = 1;
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let conflict = document(&config, 7, 1, "6368616e676564");
    let future = document(&config, 9, 2, "667574757265");
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let profile = actor.clone();
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile);
        let refused = raw_exchange(&mut socket, &future);
        assert_eq!(field(native(&refused), "reason").as_str(), Some("withheld"));
        assert!(!source.exists());
        // The near-identical current key uses an actually available source.
        fs::write(&source, source_bytes).unwrap();
        assert!(executed(&finish(&mut socket, &first, &first_poll)));
        // The next Stop endpoint is installed only after the previous request's
        // helper children were confirmed reaped by the original schedule.
        wait(&next_control);
        assert!(executed(&finish(&mut socket, &second, &second_poll)));
        fs::remove_file(&source).unwrap();
        assert!(executed(&raw_exchange(&mut socket, &first)));
        let changed = raw_exchange(&mut socket, &conflict);
        assert_eq!(
            field(native(&changed), "reason").as_str(),
            Some("idempotency_conflict")
        );
        assert!(executed(&raw_exchange(&mut socket, &first_poll)));
    });
    let first_human = human_for(reviewer.clone(), 7);
    let second_human = human_for(reviewer.clone(), 8);
    service::serve_tower(
        config,
        &actor,
        &reviewer,
        None,
        Options {
            requests: &[7, 8],
            open: false,
            credibility: None,
        },
        GENERATION,
        None,
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    first_human.join().unwrap();
    second_human.join().unwrap();
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn tower_schedule_publishes_twice_under_one_owner_and_keeps_exact_prior_retries() {
    let root = Directory::new();
    run_pair(&root);
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 2);
    assert_eq!(disk.payload, b"second");
    assert_eq!(
        disk.target.expected_version,
        config.profile.delivery.target.expected_version + 2
    );
    assert_eq!(disk.control.ledger.charged, 14);
    assert_eq!(
        disk.control.ledger.available,
        config.profile.delivery.total - 14
    );
    assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(disk.control.ledger.stages.len(), 2);
    assert_eq!(disk.control.ledger.epoch, 0);
    assert!(disk.stop.is_none());
}

#[test]
fn tower_cli_recovers_a_complete_schedule_without_source_helpers_or_new_effects() {
    let root = Directory::new();
    run_pair(&root);
    let mut config = configured(&root);
    config.programs.clear();
    let (actor, reviewer) = profiles(&root, &config);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let missing = root.0.join("must-not-read-activation.capsule");
    let profile = root.0.join("tower-series-actor.json");
    fs::write(&profile, actor_json(&root, &config)).unwrap();
    let mut args = vec![
        "tower-exchange".to_owned(),
        profile.to_str().unwrap().to_owned(),
    ];
    for (index, content) in [
        document(&config, 7, 1, "6669727374"),
        document(&config, 8, 2, "7365636f6e64"),
        poll(&config, 7),
        poll(&config, 8),
    ]
    .into_iter()
    .enumerate()
    {
        let path = root.0.join(format!("tower-series-{index}.json"));
        fs::write(&path, content).unwrap();
        args.push(path.to_str().unwrap().to_owned());
    }
    let socket = actor.socket.clone();
    let peer = std::thread::spawn(move || {
        wait(&socket);
        let mut output = Vec::new();
        crate::command_with_output(args, &mut output).unwrap();
        output
    });
    service::serve_tower(
        config,
        &actor,
        &reviewer,
        None,
        Options {
            requests: &[7, 8],
            open: true,
            credibility: Some(&missing),
        },
        GENERATION,
        None,
        || ElapsedTick(200000),
    )
    .unwrap();
    let output = peer.join().unwrap();
    let responses: Vec<_> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(parse)
        .collect();
    assert_eq!(responses.len(), 4);
    assert!(responses.iter().all(executed));
    let config = configured(&root);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert!(!root.0.join("evidence.json").exists());
    assert!(!missing.exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn tower_second_source_failure_preserves_first_effect_and_does_not_reuse_capture() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let first_poll = poll(&config, 7);
    let profile = actor.clone();
    let control = reviewer.clone();
    let source = root.0.join("evidence.json");
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile);
        assert!(executed(&finish(&mut socket, &first, &first_poll)));
        wait(&crate::workflow::control::socket_path(&control, 8));
        fs::remove_file(source).unwrap();
        let refused = raw_exchange(&mut socket, &second);
        assert_eq!(
            field(native(&refused), "reason").as_str(),
            Some("unavailable")
        );
        assert!(executed(&raw_exchange(&mut socket, &first)));
        stop_request(&control, 8);
    });
    let human = human_for(reviewer.clone(), 7);
    service::serve_tower(
        config,
        &actor,
        &reviewer,
        None,
        Options {
            requests: &[7, 8],
            open: false,
            credibility: None,
        },
        GENERATION,
        None,
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.payload, b"first");
    assert_eq!(disk.control.ledger.charged, 7);
    assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(disk.control.ledger.stages.len(), 1);
    assert_eq!(disk.stop.unwrap().request().operation, 8);
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn independent_stop_drains_while_the_second_tower_frame_is_incomplete() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let first = document(&config, 7, 1, "6669727374");
    let first_poll = poll(&config, 7);
    let profile = actor.clone();
    let control = reviewer.clone();
    let source = root.0.join("evidence.json");
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile);
        assert!(executed(&finish(&mut socket, &first, &first_poll)));
        wait(&crate::workflow::control::socket_path(&control, 8));
        fs::remove_file(source).unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        stop_request(&control, 8);
    });
    let human = human_for(reviewer.clone(), 7);
    service::serve_tower(
        config,
        &actor,
        &reviewer,
        None,
        Options {
            requests: &[7, 8],
            open: false,
            credibility: None,
        },
        GENERATION,
        None,
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.control.ledger.charged, 7);
    assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(disk.control.ledger.stages.len(), 1);
    assert_eq!(disk.stop.unwrap().request().operation, 8);
    assert!(disk.control.suspended);
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn tower_schedule_cannot_replenish_cumulative_effect_rights_at_handoff() {
    for total in [13, 14] {
        let funded = total == 14;
        let root = Directory::new();
        let mut config = configured(&root);
        config.profile.delivery.total = total;
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let first = document(&config, 7, 1, "6669727374");
        let second = document(&config, 8, 2, "7365636f6e64");
        let first_poll = poll(&config, 7);
        let second_poll = poll(&config, 8);
        let profile = actor.clone();
        let next_control = crate::workflow::control::socket_path(&reviewer, 8);
        let peer = std::thread::spawn(move || {
            let mut socket = connect(&profile);
            assert!(executed(&finish(&mut socket, &first, &first_poll)));
            wait(&next_control);
            let reply = finish(&mut socket, &second, &second_poll);
            assert_eq!(field(knowledge(&reply), "state").as_str(), Some("known"));
            assert_eq!(
                field(knowledge(&reply), "value").as_str(),
                Some(if funded { "executed" } else { "not_admitted" })
            );
        });
        let first_human = human_for(reviewer.clone(), 7);
        // Ordinary intake refuses an unfunded proposal before any new helper
        // round or human endpoint. The equal-budget case still needs both keys.
        let second_human = funded.then(|| human_for(reviewer.clone(), 8));
        let profile = config.profile.clone();
        let store = config.store.clone();
        service::serve_tower(
            config,
            &actor,
            &reviewer,
            None,
            Options {
                requests: &[7, 8],
                open: false,
                credibility: None,
            },
            GENERATION,
            None,
            || ElapsedTick(1000),
        )
        .unwrap();
        peer.join().unwrap();
        first_human.join().unwrap();
        if let Some(human) = second_human {
            human.join().unwrap();
        }
        let disk = FileOversight::read_publication(&store, &profile).unwrap();
        let spent = if funded { 14 } else { 7 };
        assert_eq!(disk.executions, if funded { 2 } else { 1 });
        assert_eq!(
            disk.payload.as_slice(),
            if funded {
                b"second".as_slice()
            } else {
                b"first".as_slice()
            }
        );
        assert_eq!(disk.control.ledger.charged, spent);
        assert_eq!(disk.control.ledger.available, total - spent);
        assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.stages.len(), if funded { 2 } else { 1 });
        assert!(disk.stop.is_none());
        assert!(!reviewer.socket(8).exists());
        let (host, _) = FileOversight::open(&store, profile).unwrap();
        assert_eq!(host.retained_requests(), 2);
        if !funded {
            assert!(matches!(
                host.request_status(8).unwrap().disposition,
                FileRequestDisposition::NotAdmitted(Error::Limit)
            ));
        }
    }
}

#[test]
fn tower_schedule_checks_all_keys_and_socket_bindings_before_source_or_store() {
    let root = Directory::new();
    for requests in [
        vec![],
        vec![8],
        vec![7, 0],
        vec![7, 7],
        std::iter::once(7).chain(100..164).collect(),
    ] {
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        assert!(
            service::serve_tower(
                config,
                &actor,
                &reviewer,
                None,
                Options {
                    requests: &requests,
                    open: false,
                    credibility: None
                },
                GENERATION,
                None,
                || panic!("invalid schedule observed time"),
            )
            .is_err()
        );
        assert!(!root.0.join("store").exists());
        assert!(!actor.socket.exists());
    }
    for zero_generation in [false, true] {
        let config = configured(&root);
        let (mut actor, reviewer) = profiles(&root, &config);
        if !zero_generation {
            actor.socket = crate::workflow::control::socket_path(&reviewer, 8);
        }
        assert!(
            service::serve_tower(
                config,
                &actor,
                &reviewer,
                None,
                Options {
                    requests: &[7, 8],
                    open: false,
                    credibility: None
                },
                if zero_generation { 0 } else { GENERATION },
                None,
                || panic!("invalid registration or future socket observed time"),
            )
            .is_err()
        );
        assert!(!root.0.join("store").exists());
        assert!(!actor.socket.exists());
    }
}

#[path = "series/abandonment.rs"]
mod abandonment;
#[path = "series/narrowing.rs"]
mod narrowing;
#[path = "series/policy_governor.rs"]
mod policy_governor;
