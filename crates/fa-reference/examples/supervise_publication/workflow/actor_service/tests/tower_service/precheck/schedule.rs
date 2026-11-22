//! Per-request disclosures stay finite across the original ordinary Tower owner.
//! Helpers here are synthetic fixtures; these tests do not qualify deployment.
use super::*;
use super::super::series as fixture;
use crate::workflow::actor_service::{precheck::schedule::Schedule, series as service};

fn schedule_document(rows: &[(u64, Option<&Path>)]) -> String {
    let rows = rows.iter().map(|(request, path)| {
        let path = path.map_or_else(|| "null".to_owned(),
            |path| format!("\"{}\"", path.display()));
        format!(r#"{{"request":{request},"precheck_profile":{path}}}"#)
    }).collect::<Vec<_>>().join(",");
    format!(r#"{{"schema":"fa.actor-precheck-schedule/1","entries":[{rows}]}}"#)
}

fn selected(root: &Directory, version: u8) -> (Schedule, [PathBuf; 2], [PathBuf; 2]) {
    let sockets = std::array::from_fn(|index|
        root.0.join(format!("actors/precheck-{}.sock", index + 7)));
    let profiles = std::array::from_fn(|index|
        root.0.join(format!("precheck-{}.json", index + 7)));
    for index in 0..2 {
        fs::write(&profiles[index],
            settings_document(&sockets[index], version, 1)
                .replace("\"disclosure_id\":81", &format!("\"disclosure_id\":{}", 81 + index)))
            .unwrap();
    }
    let path = root.0.join("precheck-schedule.json");
    fs::write(&path, schedule_document(&[
        (7, Some(&profiles[0])), (8, Some(&profiles[1])),
    ])).unwrap();
    (Schedule::read(&path, &[7, 8]).unwrap(), sockets, profiles)
}

fn check_pass(response: PrecheckWireResponse, disclosure: u64) {
    assert_eq!(response.query, Some(9001));
    let Ok(Knowledge::Known { value, basis }) = response.result else {
        panic!("expected the explicitly selected original precheck");
    };
    assert_eq!(value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!((basis.query, basis.disclosure, basis.policy_epoch), (9001, disclosure, 0));
    assert_eq!(basis.observed_at, ElapsedTick(1000));
}

fn human_after_retirement(
    reviewer: PeerProfile,
    request: u64,
    precheck: PathBuf,
    decision: ReviewDecision,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = reviewer.connect_client_waiting(request, Duration::from_secs(10)).unwrap();
        let started = Instant::now();
        let mut decision = Some(decision);
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(!precheck.exists(), "disclosure survived into human review");
                    client.respond(decision.take().expect("one original human decision")).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

#[test]
fn tower_scheduled_prechecks_publish_twice_without_renewing_queries_or_actor_connections() {
    for version in 1..=3 {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (mut actor, reviewer) = profiles(&root, &config);
        actor.connections = 1;
        actor.candidates = 1;
        let (schedule, sockets, selected_profiles) = selected(&root, version);
        // Every profile was admitted once. A later handoff must use retained
        // operator selection, not reinterpret bytes changed after admission.
        fs::write(&selected_profiles[1], b"changed after schedule admission").unwrap();
        let first = fixture::document(&config, 7, 1, "6669727374");
        let second = fixture::document(&config, 8, 2, "7365636f6e64");
        let polls = [fixture::poll(&config, 7), fixture::poll(&config, 8)];
        let checks = [
            super::shared_tower::check_document(&config)
                .replace("6e6574776f726b", "6669727374"),
            super::shared_tower::check_document(&config)
                .replace(
                    &format!("\"expected_version\":\"{}\"",
                        config.profile.delivery.target.expected_version),
                    "\"expected_version\":\"2\"",
                )
                .replace("6e6574776f726b", "7365636f6e64"),
        ];
        let mut candidates = [candidate(&config), candidate(&config)];
        candidates[0].payload = b"first".to_vec();
        candidates[1].payload = b"second".to_vec();
        candidates[1].target.expected_version = 2;
        let profile = actor.clone();
        let endpoints = sockets.clone();
        let store = config.store.clone();
        let peer = std::thread::spawn(move || {
            let mut actor_socket = connect(&profile);
            for (index, (proposal, document)) in candidates.into_iter().zip([first, second]).enumerate() {
                wait(&endpoints[index]);
                if index != 0 {
                    assert!(!endpoints[0].exists());
                    assert!(UnixStream::connect(&endpoints[0]).is_err());
                }
                let before = store_files(&store);
                // Alternate which protocol spends the one original query.
                // The other protocol must observe that SAME exhausted grant.
                if index == 0 {
                    super::shared_tower::passed(&raw_exchange(&mut actor_socket, &checks[index]));
                } else {
                    check_pass(query(&profile, &endpoints[index], proposal.clone()), 82);
                }
                assert_eq!(store_files(&store), before, "query or idle actor poll mutated the owner");
                let exhausted = query(&profile, &endpoints[index], proposal);
                assert_eq!(exhausted.result, Err(PrecheckWireError::Capacity));
                let exhausted = raw_exchange(&mut actor_socket, &checks[index]);
                let result = field(&exhausted, "result");
                assert_eq!(field(result, "kind").as_str(), Some("precheck"));
                assert_eq!(field(field(result, "value"), "reason").as_str(), Some("capacity"));
                assert_eq!(store_files(&store), before);
                assert!(fixture::executed(&fixture::finish(&mut actor_socket, &document, &polls[index])));
                assert!(!endpoints[index].exists());
            }
        });
        let first_human = human_after_retirement(
            reviewer.clone(), 7, sockets[0].clone(), ReviewDecision::Approve);
        let second_human = human_after_retirement(
            reviewer.clone(), 8, sockets[1].clone(), ReviewDecision::Approve);
        service::serve_tower_prechecked(
            config, &actor, &reviewer, None, &[7, 8], GENERATION, None, None,
            &schedule, || ElapsedTick(1000),
        ).unwrap();
        peer.join().unwrap();
        first_human.join().unwrap();
        second_human.join().unwrap();
        let config = configured(&root);
        let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!((disk.executions, disk.control.ledger.charged, disk.control.ledger.reserved),
            (2, 14, 0));
        assert_eq!(disk.payload, b"second");
        assert!(disk.stop.is_none());
        assert!(!actor.socket.exists());
        assert!(sockets.iter().all(|path| !path.exists()));
    }
}

#[test]
fn later_scheduled_precheck_does_not_supply_fresh_source_or_independent_human_permission() {
    for change_source in [true, false] {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let (schedule, sockets, _) = selected(&root, 1);
        let first = fixture::document(&config, 7, 1, "6669727374");
        let second = fixture::document(&config, 8, 2, "7365636f6e64");
        let first_poll = fixture::poll(&config, 7);
        let second_poll = fixture::poll(&config, 8);
        let mut later = candidate(&config);
        later.target.expected_version = 2;
        later.payload = b"second".to_vec();
        let denied = EvidenceSnapshot::new(
            EvidenceIdentity { source: 51, generation: 2, scope: config.profile.delivery.scope },
            Snapshot { semantic_epoch: 2, complete: true,
                values: BTreeMap::from([(7, b"blocked".to_vec())]) },
            BTreeMap::from([("alpha".to_owned(), Vec::new()), ("beta".to_owned(), Vec::new())]),
        ).unwrap().encode();
        let source = root.0.join("evidence.json");
        let profile = actor.clone();
        let endpoints = sockets.clone();
        let peer = std::thread::spawn(move || {
            let mut socket = connect(&profile);
            // Querying is optional even for an explicitly selected lifetime.
            wait(&endpoints[0]);
            assert!(fixture::executed(&fixture::finish(&mut socket, &first, &first_poll)));
            wait(&endpoints[1]);
            check_pass(query(&profile, &endpoints[1], later), 82);
            if change_source { fs::write(&source, denied).unwrap(); }
            let reply = fixture::finish(&mut socket, &second, &second_poll);
            assert_eq!(field(knowledge(&reply), "state").as_str(), Some("known"));
            if change_source {
                assert_eq!(field(knowledge(&reply), "value").as_str(), Some("denied"));
            } else {
                assert!(!fixture::executed(&reply));
            }
        });
        let first_human = human_after_retirement(
            reviewer.clone(), 7, sockets[0].clone(), ReviewDecision::Approve);
        let second_human = (!change_source).then(|| human_after_retirement(
            reviewer.clone(), 8, sockets[1].clone(), ReviewDecision::Reject));
        service::serve_tower_prechecked(
            config, &actor, &reviewer, None, &[7, 8], GENERATION, None, None,
            &schedule, || ElapsedTick(1000),
        ).unwrap();
        peer.join().unwrap();
        first_human.join().unwrap();
        if let Some(human) = second_human { human.join().unwrap(); }
        let config = configured(&root);
        let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!((disk.executions, disk.control.ledger.charged, disk.control.ledger.reserved),
            (1, 7, 0));
        assert_eq!(disk.payload, b"first");
        if !change_source {
            let (host, _) = FileOversight::open(&config.store, config.profile).unwrap();
            assert!(matches!(host.request_status(8).unwrap().disposition,
                FileRequestDisposition::Admitted { stage: ActionState::Cancelled, .. }));
        }
        assert!(!reviewer.socket(8).exists());
        assert!(sockets.iter().all(|path| !path.exists()));
    }
}

#[test]
fn all_null_schedule_reaches_original_stop_without_source_or_disclosure_bootstrap() {
    let root = Directory::new();
    let config = configured(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let path = root.0.join("precheck-schedule.json");
    fs::write(&path, schedule_document(&[(7, None), (8, None)])).unwrap();
    let schedule = Schedule::read(&path, &[7, 8]).unwrap();
    let control = reviewer.clone();
    let peer = std::thread::spawn(move || fixture::stop_request(&control, 7));
    service::serve_tower_prechecked(
        config, &actor, &reviewer, None, &[7, 8], GENERATION, None, None,
        &schedule, || ElapsedTick(1000),
    ).unwrap();
    peer.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert!(disk.stop.is_some());
    assert_eq!((disk.executions, disk.control.ledger.charged, disk.control.ledger.reserved), (0, 0, 0));
    assert!(disk.control.ledger.stages.is_empty());
    assert!(!root.0.join("evidence.json").exists());
    assert!(!actor.socket.exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 8).exists());
}

#[test]
fn scheduled_precheck_stop_retires_current_and_never_bootstraps_later_selection() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let (schedule, sockets, _) = selected(&root, 1);
    let control = reviewer.clone();
    let endpoints = sockets.clone();
    let source = root.0.join("evidence.json");
    let peer = std::thread::spawn(move || {
        wait(&endpoints[0]);
        let mut partial = UnixStream::connect(&endpoints[0]).unwrap();
        partial.write_all(b"{").unwrap();
        fs::remove_file(&source).unwrap();
        fixture::stop_request(&control, 7);
        assert!(!endpoints[1].exists());
    });
    service::serve_tower_prechecked(
        config, &actor, &reviewer, None, &[7, 8], GENERATION, None, None,
        &schedule, || ElapsedTick(1000),
    ).unwrap();
    peer.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert!(disk.stop.is_some());
    assert_eq!(disk.executions, 0);
    assert!(disk.control.ledger.stages.is_empty());
    assert!(sockets.iter().all(|path| !path.exists()));
    assert!(!actor.socket.exists());
}

#[test]
fn precheck_schedule_is_closed_bounded_and_matches_original_request_keys() {
    let root = Directory::new();
    let path = root.0.join("precheck-schedule.json");
    let valid = schedule_document(&[(7, None), (8, None)]);
    let excessive = schedule_document(&(1..=65).map(|key| (key, None)).collect::<Vec<_>>());
    for invalid in [
        valid.replace("/1", "/2"),
        valid.replace("\"request\":8", "\"request\":7"),
        valid.replace("\"request\":8", "\"request\":9"),
        valid.replace("\"request\":7", "\"request\":0"),
        valid.replace("\"request\":7", "\"request\":7.0"),
        valid.replace("\"precheck_profile\":null", "\"unknown\":null"),
        valid.replace("\"precheck_profile\":null", "\"precheck_profile\":true"),
        valid.replace("\"precheck_profile\":null", "\"precheck_profile\":\"relative.json\""),
        valid.replace("\"precheck_profile\":null", "\"precheck_profile\":\"/tmp/../profile.json\""),
        valid.replace("\"request\":7", "\"extra\":null,\"request\":7"),
        valid.replace("\"entries\":", "\"extra\":null,\"entries\":"),
        schedule_document(&[(8, None), (7, None)]),
        schedule_document(&[(7, None)]),
        schedule_document(&[]),
        excessive,
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(Schedule::read(&path, &[7, 8]).is_err());
        assert!(!root.0.join("store").exists());
    }
    fs::write(&path, valid).unwrap();
    assert!(Schedule::read(&path, &[7, 8]).is_ok());
    let requests = (1..=64).collect::<Vec<_>>();
    fs::write(&path, schedule_document(&requests.iter().map(|key| (*key, None)).collect::<Vec<_>>())).unwrap();
    assert!(Schedule::read(&path, &requests).is_ok());
    let excessive_keys = (1..=65).collect::<Vec<_>>();
    fs::write(&path, schedule_document(
        &excessive_keys.iter().map(|key| (*key, None)).collect::<Vec<_>>(),
    )).unwrap();
    assert!(Schedule::read(&path, &excessive_keys).is_err(),
        "the row ceiling applies even when every key matches");
}

#[test]
fn every_later_precheck_profile_and_total_allowance_is_admitted_up_front() {
    let root = Directory::new();
    let (_, sockets, paths) = selected(&root, 1);
    let path = root.0.join("precheck-schedule.json");
    let later = settings_document(&sockets[1], 1, 1).replace("\"disclosure_id\":81", "\"disclosure_id\":82");
    for invalid in [
        later.replace("\"query_limit\":1", "\"query_limit\":128"),
        later.replace("\"exchange_limit\":8", "\"exchange_limit\":128"),
        later.replace("\"candidate_limit\":8", "\"candidate_limit\":1024"),
        later.replace("\"candidate_limit\":8", "\"candidate_limit\":1024")
            .replace("\"connection_limit\":4", "\"connection_limit\":1024"),
        later.replace("\"disclosure_id\":82", "\"disclosure_id\":81"),
        later.replace("\"runtime_ms\":15000", "\"runtime_ms\":0"),
        later.replace("\"policy_disclosure\":\"exact_predicates\"", "\"policy_disclosure\":\"full\""),
        "{".to_owned(),
    ] {
        fs::write(&paths[1], invalid).unwrap();
        assert!(Schedule::read(&path, &[7, 8]).is_err());
        assert!(!root.0.join("store").exists());
        assert!(sockets.iter().all(|socket| !socket.exists()));
    }
    for (index, profile) in paths.iter().enumerate() {
        fs::write(profile, settings_document(&sockets[index], 1, 64)
            .replace("\"disclosure_id\":81", &format!("\"disclosure_id\":{}", 81 + index))
            .replace("\"exchange_limit\":8", "\"exchange_limit\":64")
            .replace("\"candidate_limit\":8", "\"candidate_limit\":512")
            .replace("\"connection_limit\":4", "\"connection_limit\":512")).unwrap();
    }
    assert!(Schedule::read(&path, &[7, 8]).is_ok(), "exact aggregate boundaries must remain usable");
}

#[test]
fn future_endpoint_and_topology_aliases_refuse_before_clock_source_or_owner_creation() {
    use crate::workflow::continuation::creation::topology::Selection;
    use std::os::unix::fs::symlink;
    for dimension in 0..11 {
        let root = Directory::new();
        let mut config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let (_, sockets, paths) = selected(&root, 1);
        let mut topology = None;
        let collision = match dimension {
            0 => actor.socket.clone(),
            1 => reviewer.socket(8),
            2 => crate::workflow::control::socket_path(&reviewer, 8),
            3 => sockets[0].clone(),
            4 => root.0.join("store"),
            5 => root.0.join("evidence.json"),
            6 => {
                symlink(root.0.join("reviewers"), root.0.join("alias")).unwrap();
                root.0.join("alias").join(reviewer.socket(8).file_name().unwrap())
            }
            9 => {
                config.source = FileEvidenceSource::new(
                    reviewer.socket(8), 51, config.profile.delivery.scope, 1048576,
                ).unwrap();
                sockets[1].clone()
            }
            _ => {
                let graph = if dimension == 10 { reviewer.socket(8) }
                    else { root.0.join("actors/graph.json") };
                let inventory = root.0.join("inventory.json");
                let output = root.0.join("actors/checkpoint.json");
                topology = Some(Selection::decode(format!(
                    r#"{{"schema_version":"fa.mediated-create/2","topology_path":"{}","inventory_path":"{}","retain_checkpoint":"{}"}}"#,
                    graph.display(), inventory.display(), output.display(),
                ).as_bytes()).unwrap());
                match dimension {
                    7 => graph,
                    8 => output,
                    _ => sockets[1].clone(),
                }
            }
        };
        fs::write(&paths[1], settings_document(&collision, 1, 1)
            .replace("\"disclosure_id\":81", "\"disclosure_id\":82")).unwrap();
        let schedule = Schedule::read(&root.0.join("precheck-schedule.json"), &[7, 8]).unwrap();
        let store = config.store.clone();
        assert!(service::serve_tower_prechecked(
            config, &actor, &reviewer, None, &[7, 8], GENERATION, None, topology,
            &schedule, || panic!("later selection crossed the preflight clock boundary"),
        ).is_err());
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert!(!sockets[0].exists());
    }
}

#[test]
fn unsupported_schedule_cli_combinations_refuse_before_opening_any_profile() {
    for (mode, extras) in [
        ("serve-create", vec!["--precheck-schedule", "/missing/schedule"]),
        ("serve-open", vec!["--requests", "7,8", "--precheck-schedule", "/missing/schedule"]),
        ("serve-create", vec!["--requests", "7,8", "--precheck-schedule", "/missing/schedule",
            "--precheck-profile", "/missing/precheck"]),
        ("serve-create", vec!["--requests", "7,8", "--precheck-schedule", "/missing/schedule",
            "--governor-profile", "/missing/governor"]),
        ("serve-create", vec!["--requests", "7,8", "--precheck-schedule", "/missing/schedule",
            "--guarded", "/missing/recovery"]),
        ("serve-create", vec!["--requests", "7,8", "--precheck-schedule", "/missing/schedule",
            "--precheck-schedule", "/missing/duplicate"]),
    ] {
        let mut args = vec![mode, "/missing/config", "/missing/actor", "/missing/reviewer",
            "--tower-generation", "11"].into_iter().map(str::to_owned).collect::<Vec<_>>();
        args.extend(extras.into_iter().map(str::to_owned));
        let error = crate::workflow::actor_service::tower::command_with_clock(
            &args, None, || panic!("unsupported selection reached the clock"),
        ).unwrap_err();
        assert!(error.contains("serve-create|serve-open"), "{error}");
    }
}

#[test]
fn richer_checked_profile_refuses_before_schedule_bootstrap() {
    let root = Directory::new();
    let config = configured(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let (schedule, sockets, _) = selected(&root, 1);
    let publication = PublicationProfile::decode(format!(
        r#"{{"schema":"fa.supervised-whole-input/1","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
        root.0.display(), scope(&config),
    ).as_bytes()).unwrap();
    let store = config.store.clone();
    assert!(service::serve_tower_prechecked(
        config, &actor, &reviewer, Some(&publication), &[7, 8], GENERATION, None, None,
        &schedule, || panic!("unsupported checked profile reached source/time bootstrap"),
    ).is_err());
    assert!(!store.exists());
    assert!(!actor.socket.exists());
    assert!(sockets.iter().all(|socket| !socket.exists()));
}
