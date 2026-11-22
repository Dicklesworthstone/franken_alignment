//! Independently anchored ordinary recovery needs neither a topology service
//! nor policy governance. Actual helper/human traffic still gates each effect.
use super::*;
use crate::workflow::actor_service::{OwnerStart, precheck::schedule::Schedule};
use crate::workflow::actor_service::tests::tower_service::operator as observer_fixture;

fn configured_epoch(root: &Directory, epoch: u64) -> Config {
    let mut config = configured(root);
    config.programs = ["alpha", "beta"].into_iter().map(|member| {
        (member.to_owned(), HelperProgram::new(
            std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
            BTreeMap::from([
                (OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(member)),
                (OsString::from("FA_EXAMPLE_HELPER_POLICY_EPOCH"), OsString::from(epoch.to_string())),
            ]),
        ).unwrap())
    }).collect();
    config
}

fn archive(root: &Directory, owner: &FileOversight) {
    let mut bytes = Vec::new();
    owner.history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    write_complete(&root.0.join("before.anchor"), bytes);
}

fn selected(root: &Directory, config: &Config) -> SubmitProfile {
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let bytes = format!(
        r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":{:?},"retain_history_anchor":{:?}}}"#,
        before.revision, before.control.sequence, before.control.ledger.epoch,
        root.0.join("before.anchor"), root.0.join("after.anchor"),
    );
    write_complete(&root.0.join("guarded-submit.json"), &bytes);
    SubmitProfile::decode(bytes.as_bytes()).unwrap()
}

fn fresh(root: &Directory, config: &Config, publication: Option<&PublicationProfile>) -> SubmitProfile {
    // These are the original ordinary constructor and exact installed guards.
    // No fresh governor is created and no unsupported guard is left implicit.
    let (owner, _human) = crate::workflow::actor_service::prepare_host(config, publication, false).unwrap();
    archive(root, &owner);
    drop(owner);
    selected(root, config)
}

fn schedule(root: &Directory, disclosure: bool) -> Schedule {
    let profile = root.0.join("precheck-7.json");
    write_complete(&profile, format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":{:?},"expected_policy_epoch":1,"disclosure_id":81,"query_limit":1,"exchange_limit":4,"candidate_limit":4,"connection_limit":2,"runtime_ms":15000}}"#,
        root.0.join("actors/precheck-7.sock"),
    ));
    let path = root.0.join("precheck-schedule.json");
    write_complete(&path, format!(
        r#"{{"schema":"fa.actor-precheck-schedule/1","entries":[{{"request":7,"precheck_profile":{}}}]}}"#,
        if disclosure { format!("{:?}", profile) } else { "null".into() },
    ));
    Schedule::read(&path, &[7]).unwrap()
}

fn retained(root: &Directory, config: &Config) {
    let current = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(root.0.join("after.anchor")).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    ).unwrap();
    assert_eq!(anchor.revision(), current.revision);
    assert_eq!(anchor.retained_bytes(), fs::read(config.store.join("delivery.bin")).unwrap().len());
    assert_eq!(fs::metadata(root.0.join("after.anchor")).unwrap().permissions().mode() & 0o077, 0);
}

fn one_publication(checked_mode: bool, operator_selected: bool, human_allows: bool, change_source: bool) {
    let root = Directory::new();
    let config = configured_epoch(&root, 1);
    evidence(&root, &config);
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    let publication = checked_mode.then(||
        PublicationProfile::decode(checked::profile_document(&root).as_bytes()).unwrap());
    let recovery = fresh(&root, &config, publication.as_ref());
    let original_anchor = fs::read(root.0.join("before.anchor")).unwrap();
    let selected_schedule = schedule(&root, true);
    if checked_mode {
        let capture = checked::capture(&config, 1, 1, b"network");
        write_complete(&root.0.join("original.bin"), &capture);
        write_complete(&root.0.join("current.bin"), capture);
    }
    let operator = operator_selected.then(|| operator::Settings::decode(
        observer_fixture::settings_document(&root, &config, 4, 0).as_bytes()).unwrap());
    let check = precheck::document(&config, 1, b"network", 1);
    let candidate = precheck::proposal(&config, 1, b"network", 1);
    let submit = document(&config, 7, 1, "6e6574776f726b")
        .replace("\"expected_policy_epoch\":\"0\"", "\"expected_policy_epoch\":\"1\"");
    let poll = poll(&config, 7);
    let endpoint = root.0.join("actors/precheck-7.sock");
    let source = root.0.join("evidence.json");
    let invalid_source = EvidenceSnapshot::new(
        EvidenceIdentity { source: 51, generation: 2, scope: config.profile.delivery.scope },
        Snapshot { semantic_epoch: 2, complete: true, values: BTreeMap::from([(7, b"no".to_vec())]) },
        BTreeMap::from([
            ("alpha".into(), b"review this fixture: alpha".to_vec()),
            ("beta".into(), b"review this fixture: beta".to_vec()),
        ]),
    ).unwrap().encode();
    let human = (!change_source).then(|| {
        let reviewer = reviewer.clone();
        let endpoint = endpoint.clone();
        std::thread::spawn(move || {
            let mut client = reviewer.connect_client_waiting(7, Duration::from_secs(10)).unwrap();
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(10));
                match client.step().unwrap() {
                    ReviewClientProgress::NeedsDecision => {
                        let packet = client.packet().unwrap();
                        assert_eq!(packet.policy_generation(), 1);
                        assert_eq!(packet.action().spec().policy_epoch, 1);
                        assert_eq!(packet.action().spec().payload, b"network");
                        assert_eq!(packet.views().len(), 2);
                        assert!(!endpoint.exists(), "precheck survived into fresh review");
                        client.respond(if human_allows { ReviewDecision::Approve } else { ReviewDecision::Reject }).unwrap();
                    }
                    ReviewClientProgress::Complete => return,
                    _ => pause(1),
                }
            }
        })
    });
    let peer_profile = actor.clone();
    let operator_profile = operator.clone();
    let situation = observer_fixture::situation_document(&config);
    let peer = std::thread::spawn(move || {
        let mut actor = connect(&peer_profile);
        if let Some(operator) = operator_profile {
            wait(&operator.socket);
            let mut observer = UnixStream::connect(&operator.socket).unwrap();
            operator.supervisor.verify(&observer).unwrap();
            let reply = raw_exchange(&mut observer, &situation);
            observer_fixture::known_operator(&reply, "situation");
        }
        wait(&endpoint);
        precheck::passed(&raw_exchange(&mut actor, &check), "81", "1");
        precheck::exhausted(&peer_profile, &endpoint, candidate);
        if change_source { write_complete(&source, invalid_source); }
        let reply = finish(&mut actor, &submit, &poll);
        assert_eq!(executed(&reply), human_allows && !change_source);
        if !human_allows && !change_source {
            assert_eq!(field(knowledge(&reply), "value").as_str(), Some("cancelled_before_dispatch"));
        }
        if change_source {
            assert_eq!(field(knowledge(&reply), "value").as_str(), Some("denied"));
        }
    });
    let result = policy_recovery::serve_plain(
        config, &actor, &reviewer, publication.as_ref(), None, &recovery,
        &[7], GENERATION, operator.as_ref(), Some(&selected_schedule), || ElapsedTick(2000),
    );
    let peer_result = peer.join();
    let human_result = human.map(|thread| thread.join());
    result.unwrap();
    peer_result.unwrap();
    if let Some(result) = human_result { result.unwrap(); }
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let published = human_allows && !change_source;
    assert_eq!(disk.executions, u64::from(published));
    assert_eq!(disk.control.ledger.charged, if published { 7 } else { 0 });
    assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(disk.control.ledger.available, config.profile.delivery.total - disk.control.ledger.charged);
    assert_eq!(disk.payload, if published { &b"network"[..] } else { &b"initial"[..] });
    assert_eq!(fs::read(root.0.join("before.anchor")).unwrap(), original_anchor);
    retained(&root, &config);
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!root.0.join("actors/precheck-7.sock").exists());
}

#[test]
fn plain_guarded_singleton_resumes_plain_and_checked_publication_without_a_governor() {
    one_publication(false, false, true, false);
    one_publication(true, true, true, false);
}

#[test]
fn recovered_disclosure_cannot_replace_fresh_source_or_independent_human_approval() {
    one_publication(false, false, false, false);
    one_publication(true, false, false, false);
    one_publication(false, false, true, true);
}

fn published(root: &Directory) -> SubmitProfile {
    let config = configured(root);
    evidence(root, &config);
    let (actor, reviewer) = profiles(root, &config);
    let request = document(&config, 7, 1, "6e6574776f726b");
    let query = poll(&config, 7);
    let peer_profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_profile);
        assert!(executed(&finish(&mut socket, &request, &query)));
    });
    let human = human_for(reviewer.clone(), 7);
    let (owner, role) = crate::workflow::actor_service::prepare_host(&config, None, false).unwrap();
    let (port, driver) = owner.into_supervised_driver();
    let socket = BoundSocket::bind(&actor.socket, None).unwrap();
    actor.secure_socket().unwrap();
    let deadline = Deadline { logical: ElapsedTick(16000), started: Instant::now(), wall: Duration::from_millis(15000) };
    let result = service::serve_ready_driver_with_governor(
        config, &actor, &reviewer, None,
        Options { requests: &[7], open: false, credibility: None },
        Some(GENERATION), None, socket, port, driver, role, None, OwnerStart::Ordinary,
        None, None, None, deadline, || ElapsedTick(1000),
        |owner, _, _| { archive(root, owner); Ok(()) },
    );
    let peer_result = peer.join();
    let human_result = human.join();
    result.unwrap();
    peer_result.unwrap();
    human_result.unwrap();
    selected(root, &configured(root))
}

#[test]
fn plain_guarded_historical_singleton_needs_no_source_helpers_or_new_disclosure() {
    let root = Directory::new();
    let recovery = published(&root);
    let mut config = configured(&root);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    config.programs.clear();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let schedule = schedule(&root, false);
    let (actor, reviewer) = profiles(&root, &config);
    let request = document(&config, 7, 1, "6e6574776f726b");
    let query = poll(&config, 7);
    let peer_profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_profile);
        assert!(executed(&raw_exchange(&mut socket, &request)));
        assert!(executed(&raw_exchange(&mut socket, &query)));
    });
    let missing_capsule = root.0.join("must-not-read-activation.capsule");
    let result = policy_recovery::serve_plain(
        config, &actor, &reviewer, None, Some(&missing_capsule), &recovery, &[7],
        GENERATION, None, Some(&schedule), || ElapsedTick(2000),
    );
    let peer_result = peer.join();
    result.unwrap();
    peer_result.unwrap();
    let config = configured(&root);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 1);
    assert!(!root.0.join("evidence.json").exists());
    assert!(!root.0.join("actors/precheck-7.sock").exists());
    retained(&root, &config);
}

#[test]
fn historical_disclosure_refusal_retains_the_original_fence_and_confirmed_effect() {
    let root = Directory::new();
    let recovery = published(&root);
    let mut config = configured(&root);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    config.programs.clear();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let selected_schedule = schedule(&root, true);
    let error = policy_recovery::serve_plain(
        config, &actor, &reviewer, None, None, &recovery, &[7], GENERATION,
        None, Some(&selected_schedule), || ElapsedTick(2000),
    ).unwrap_err();
    assert!(error.contains("recorded requests are observation-only"), "{error}");
    let config = configured(&root);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 1);
    assert_eq!((after.executions, after.control.ledger.charged), (1, 7));
    assert_eq!(after.payload, before.payload);
    assert!(!root.0.join("evidence.json").exists());
    assert!(!root.0.join("actors/precheck-7.sock").exists());
    retained(&root, &config);
}

#[test]
fn plain_guarded_recovery_rejects_wrong_policy_source_guard_witness_and_anchor_before_mutation() {
    for mismatch in 0..4 {
        let root = Directory::new();
        let mut config = configured(&root);
        let publication = (mismatch == 2).then(||
            PublicationProfile::decode(checked::profile_document(&root).as_bytes()).unwrap());
        let recovery = fresh(&root, &config, publication.as_ref());
        let (actor, reviewer) = profiles(&root, &config);
        fs::write(config.store.join("delivery.pending"), b"unacknowledged staging").unwrap();
        let original = store_files(&config.store);
        match mismatch {
            0 => config.profile.delivery.policy = Policy::new(2, vec![Predicate::PayloadAtMost(512)]).unwrap(),
            1 => config.source_policy.source.generation += 1,
            2 => {}, // Omitting the original checked guard cannot recover a plain owner.
            3 => fs::write(root.0.join("before.anchor"), b"foreign incomplete anchor").unwrap(),
            _ => unreachable!(),
        }
        let store = config.store.clone();
        assert!(policy_recovery::serve_plain(
            config, &actor, &reviewer, None, None, &recovery, &[7], GENERATION,
            None, None, || ElapsedTick(2000),
        ).is_err(), "mismatch {mismatch}");
        assert_eq!(store_files(&store), original, "mismatch {mismatch} changed original history/staging");
        assert!(!root.0.join("after.anchor").exists());
        assert!(!root.0.join("evidence.json").exists());
        assert!(!actor.socket.exists());
    }
}

#[test]
fn plain_guarded_entry_rejects_unpaired_promotion_or_missing_schedule_before_clock() {
    for mismatch in 0..3 {
        let root = Directory::new();
        let config = configured(&root);
        let recovery = fresh(&root, &config, None);
        let (actor, reviewer) = profiles(&root, &config);
        let promotion = operator_settings(&root, &config);
        let original = store_files(&config.store);
        let store = config.store.clone();
        let error = policy_recovery::serve_plain(
            config, &actor, &reviewer, None, None, &recovery,
            if mismatch == 1 { &[] } else { &[7] },
            if mismatch == 2 { 0 } else { GENERATION },
            (mismatch == 0).then_some(&promotion), None,
            || panic!("incomplete independent recovery selection reached the clock"),
        ).unwrap_err();
        assert!(error.contains(if mismatch == 0 { "independent governor" } else { "explicit request schedule" }), "{error}");
        assert_eq!(store_files(&store), original);
        assert!(!root.0.join("after.anchor").exists());
        assert!(!actor.socket.exists());
        assert!(!promotion.socket.exists());
    }
}

#[test]
fn ordinary_guarded_recovery_keeps_native_stop_live_with_an_incomplete_actor_frame() {
    let root = Directory::new();
    let config = configured(&root);
    let recovery = fresh(&root, &config, None);
    let (actor, reviewer) = profiles(&root, &config);
    let peer_profile = actor.clone();
    let stop_profile = reviewer.clone();
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_profile);
        let started = Instant::now();
        while FileOversight::read_publication(&store, &bootstrap).unwrap().control.ledger.epoch == 0 {
            assert!(started.elapsed() < Duration::from_secs(10));
            pause(1);
        }
        socket.set_nonblocking(false).unwrap();
        socket.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        stop_request(&stop_profile, 7);
    });
    let result = policy_recovery::serve_plain(
        config, &actor, &reviewer, None, None, &recovery, &[7], GENERATION,
        None, None, || ElapsedTick(2000),
    );
    let peer_result = peer.join();
    if let Err(error) = result { assert!(error.contains("native Stop acknowledged"), "{error}"); }
    peer_result.unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert!(disk.stop.is_some());
    assert_eq!(disk.executions, 0);
    assert_eq!((disk.control.ledger.charged, disk.control.ledger.reserved), (0, 0));
    assert!(!root.0.join("evidence.json").exists());
    retained(&root, &config);
}
