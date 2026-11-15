//! The public scheduled CLI retains both original checked and topology gates.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{PrecheckRequest, client::PrecheckPeerClient},
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorProposal, Knowledge},
    actor_transport::DriveBudget,
};

fn query(profile: &Profile, socket: &Path, proposal: ActorProposal, disclosure: u64) {
    let mut client = PrecheckPeerClient::connect(
        connect_waiting(socket), profile.supervisor,
        PrecheckRequest { query: 9001, proposal }, 0,
    ).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10), "scheduled precheck reply timed out");
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() {
            let Knowledge::Known { value, basis } = response.result.unwrap() else {
                panic!("missing original scheduled policy observation");
            };
            assert_eq!(value, FilePrecheckOutcome::PrecheckPassed);
            assert_eq!((basis.query, basis.disclosure, basis.policy_epoch), (9001, disclosure, 0));
            return;
        }
        assert!(!report.status.closed(), "precheck closed before its original answer");
        pause(1);
    }
}

#[test]
fn scheduled_precheck_cli_preserves_two_checked_publications_and_original_topology_work() {
    let root = Root::new();
    let config = configuration(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let operator = operator_profile(&root, &config, 2, 0);
    let archive = root.path("sequence-source.bin");
    selected_checks(&root, &archive, 3);
    publication(&root);
    write_pair(&root, &action_for(&root, &config, 1, 1, b"first"));
    let second_action = action_for(&root, &config, 2, 2, b"second");
    let sockets: [PathBuf; 2] = std::array::from_fn(|index|
        root.path(&format!("actors/precheck-{}.sock", index + 7)));
    let mut entries = Vec::new();
    for (index, socket) in sockets.iter().enumerate() {
        let profile = root.path(&format!("precheck-{}.json", index + 7));
        fs::write(&profile, format!(
            r#"{{"schema":"fa.actor-precheck-service/2","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":0,"disclosure_id":{},"query_limit":1,"exchange_limit":4,"candidate_limit":4,"connection_limit":2,"runtime_ms":15000,"cache_entries":4,"cache_bytes":262144}}"#,
            socket.display(), 81 + index,
        )).unwrap();
        entries.push(format!(
            r#"{{"request":{},"precheck_profile":"{}"}}"#, index + 7, profile.display(),
        ));
    }
    let schedule = root.path("precheck-schedule.json");
    fs::write(&schedule, format!(
        r#"{{"schema":"fa.actor-precheck-schedule/1","entries":[{}]}}"#, entries.join(","),
    )).unwrap();
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let situation = situation_document(&config);
    let first_candidate = ActorProposal {
        target: config.profile.delivery.target,
        payload: b"first".to_vec(),
        units: 7,
        deadline: ElapsedTick(100000),
        expected_policy_epoch: 0,
    };
    let mut second_candidate = first_candidate.clone();
    second_candidate.target.expected_version = 2;
    second_candidate.payload = b"second".to_vec();
    let actor_peer = actor.clone();
    let operator_peer = operator.clone();
    let endpoints = sockets.clone();
    let root_path = root.0.clone();
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_peer);
        let mut actor = connect_actor(&actor_peer);
        counts(&exchange(&mut operator, &situation), &[("attempts", "0")]);
        query(&actor_peer, &endpoints[0], first_candidate, 81);
        assert_executed(&finish(&mut actor, &first, &first_poll));
        assert!(!endpoints[0].exists());
        query(&actor_peer, &endpoints[1], second_candidate, 82);
        let fixture = capture(&second_action, 1, &[0, 2, 4]);
        let original = FilePublicationCapture::new(
            2, fixture.identity(), &second_action, fixture.inputs().clone(),
        ).unwrap().to_bytes().unwrap();
        fs::write(root_path.join("original.bin"), &original).unwrap();
        fs::write(root_path.join("current.bin"), original).unwrap();
        assert_executed(&finish(&mut actor, &second, &second_poll));
        assert!(!endpoints[1].exists());
        counts(&exchange(&mut operator, &situation),
            &[("attempts", "2"), ("executions", "2"), ("charged_units", "14")]);
    });
    let first_human = human_for(reviewer.clone(), 7);
    let second_human = human_for(reviewer.clone(), 8);
    let args = vec![
        "serve-create-checked".to_owned(),
        root.path("config.json").display().to_string(),
        root.path("actor.json").display().to_string(),
        root.path("reviewer.json").display().to_string(),
        root.path("witness.json").display().to_string(),
        "--precheck-schedule".to_owned(), schedule.display().to_string(),
        "--topology-profile".to_owned(), root.path("topology-profile.json").display().to_string(),
        "--tower-generation".to_owned(), GENERATION.to_string(),
        "--operator-profile".to_owned(), root.path("operator.json").display().to_string(),
        "--requests".to_owned(), "7,8".to_owned(),
    ];
    let result = tower_command(&args, None, || ElapsedTick(1000));
    peer.join().unwrap();
    first_human.join().unwrap();
    second_human.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!((state.executions, state.control.ledger.charged, state.control.ledger.reserved), (2, 14, 0));
    assert_eq!(state.payload, b"second");
    assert_eq!(state.target.expected_version, 3);
    assert!(state.stop.is_none());
    let saved = TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(archive).unwrap()), MAX_TOPOLOGY_CHECKPOINT_BYTES,
    ).unwrap();
    assert_eq!(saved.history_anchor().revision(), state.revision);
    assert_eq!(saved.work().checks, 3);
    assert_eq!(saved.work().reserved_read_bytes, 3 * TOPOLOGY_REFRESH_READ_BYTES);
    assert!(!actor.socket.exists());
    assert!(!operator.socket.exists());
    assert!(sockets.iter().all(|path| !path.exists()));
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
}
