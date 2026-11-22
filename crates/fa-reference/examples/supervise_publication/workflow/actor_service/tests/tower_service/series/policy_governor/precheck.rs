//! Selected disclosures and real governance share the original finite owner.
//! Helper verdicts and explicit custodian choices are controlled test inputs.
use super::*;
use crate::workflow::actor_service::precheck::schedule::Schedule;
use crate::workflow::continuation::creation::topology::Selection as TopologySelection;
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::{
    MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckRequest, PrecheckWireError, client::PrecheckPeerClient,
};
use std::io::Cursor;

fn profile_document(socket: &Path, disclosure: u64, epoch: u64) -> String {
    format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":{epoch},"disclosure_id":{disclosure},"query_limit":1,"exchange_limit":4,"candidate_limit":4,"connection_limit":2,"runtime_ms":15000}}"#,
        socket.display(),
    )
}

fn schedule(root: &Directory, later_epoch: u64) -> (Schedule, [PathBuf; 2], [PathBuf; 2]) {
    let sockets =
        std::array::from_fn(|index| root.0.join(format!("actors/precheck-{}.sock", index + 7)));
    let profiles = std::array::from_fn(|index| root.0.join(format!("precheck-{}.json", index + 7)));
    for (index, path) in profiles.iter().enumerate() {
        fs::write(
            path,
            profile_document(
                &sockets[index],
                81 + index as u64,
                if index == 0 { 0 } else { later_epoch },
            ),
        )
        .unwrap();
    }
    let path = root.0.join("precheck-schedule.json");
    fs::write(&path, format!(
        r#"{{"schema":"fa.actor-precheck-schedule/1","entries":[{{"request":7,"precheck_profile":"{}"}},{{"request":8,"precheck_profile":"{}"}}]}}"#,
        profiles[0].display(), profiles[1].display(),
    )).unwrap();
    (Schedule::read(&path, &[7, 8]).unwrap(), sockets, profiles)
}

fn proposal(config: &Config, version: u64, payload: &[u8]) -> ActorProposal {
    let mut target = config.profile.delivery.target;
    target.expected_version = version;
    ActorProposal {
        target,
        payload: payload.to_vec(),
        units: 7,
        deadline: ElapsedTick(100000),
        expected_policy_epoch: 0,
    }
}

fn check_document(config: &Config, candidate: ActorProposal) -> String {
    let scope = config.profile.delivery.scope;
    let mut request = TowerRequest::new(
        "governed-precheck",
        Address::parse(&format!(
            "fa://{}/run/{}@{GENERATION}",
            scope.tenant, scope.run,
        ))
        .unwrap(),
        Verb::Precheck,
    );
    request.arguments = ReadArguments::Precheck {
        query: 9001,
        proposal: candidate,
    };
    encode_request(&request).unwrap()
}

fn precheck_knowledge(reply: &Json) -> &Json {
    let result = field(reply, "result");
    assert_eq!(field(result, "kind").as_str(), Some("precheck"));
    field(field(result, "value"), "knowledge")
}

fn passed(reply: &Json, disclosure: &str) {
    let knowledge = precheck_knowledge(reply);
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    assert_eq!(field(knowledge, "value").as_str(), Some("precheck_passed"));
    assert_eq!(field(knowledge, "review_required").as_bool(), Some(true));
    let basis = field(knowledge, "basis");
    assert_eq!(field(basis, "disclosure").as_str(), Some(disclosure));
    assert_eq!(field(basis, "policy_epoch").as_str(), Some("0"));
}

fn withheld(reply: &Json) {
    let knowledge = precheck_knowledge(reply);
    assert_eq!(field(knowledge, "state").as_str(), Some("withheld"));
    assert_eq!(
        field(knowledge, "authority_required").as_str(),
        Some("exact_policy_disclosure")
    );
}

fn exhausted_native(profile: &Profile, socket: &Path, proposal: ActorProposal) {
    let mut client = PrecheckPeerClient::connect(
        UnixStream::connect(socket).unwrap(),
        profile.supervisor,
        PrecheckRequest {
            query: 9002,
            proposal,
        },
        0,
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let report = client.drive(drive_budget()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(reply) = client.take_response() {
            assert_eq!(reply.result, Err(PrecheckWireError::Capacity));
            return;
        }
        assert!(!report.status.closed());
        pause(1);
    }
}

fn topology(root: &Directory) -> TopologySelection {
    let graph = root.0.join("topology.json");
    let inventory = root.0.join("inventory.json");
    fs::write(
        &graph,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/authority_topology.json"
        )),
    )
    .unwrap();
    fs::write(
        &inventory,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/authority_topology_inventory.json"
        )),
    )
    .unwrap();
    TopologySelection::decode(format!(
        r#"{{"schema_version":"fa.mediated-create/2","topology_path":"{}","inventory_path":"{}","retain_checkpoint":"{}"}}"#,
        graph.display(), inventory.display(), root.0.join("source-checkpoint.bin").display(),
    ).as_bytes()).unwrap()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Publish,
    HumanRejects,
    SourceChanges,
}

#[test]
fn governed_scheduled_prechecks_publish_then_promote_without_renewing_disclosure() {
    governed_pair(false, false, Outcome::Publish);
    governed_pair(true, true, Outcome::Publish);
}

#[test]
fn governed_precheck_and_approval_cannot_replace_fresh_source_or_human_permission() {
    governed_pair(false, false, Outcome::HumanRejects);
    governed_pair(true, false, Outcome::SourceChanges);
}

fn governed_pair(checked: bool, mediated: bool, outcome: Outcome) {
    let root = Directory::new();
    let config = mediated::governed_config(&root);
    evidence(&root, &config);
    let publication = checked.then(|| checked::prepare(&root, &config));
    let second_capture = publication.as_ref().map(|(_, bytes)| bytes.clone());
    let topology = mediated.then(|| topology(&root));
    let (schedule, sockets, _) = schedule(&root, 0);
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    actor.reply_ms = 1000;
    let operator = operator_settings(&root, &config);
    let mut governor_child = GovernorChild::start(&root);
    let governor_text = governor_document(&root, &config, governor_child.0.id());
    let governor = governor::Settings::decode(governor_text.as_bytes()).unwrap();
    write_complete(&root.0.join("governor.json"), governor_text);
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64").replace(
        "\"expected_policy_epoch\":\"0\"",
        "\"expected_policy_epoch\":\"1\"",
    );
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let candidates = [
        proposal(&config, 1, b"first"),
        proposal(&config, 2, b"second"),
    ];
    let checks = candidates
        .clone()
        .map(|candidate| check_document(&config, candidate));
    let blocked = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 2,
            scope: config.profile.delivery.scope,
        },
        Snapshot {
            semantic_epoch: 2,
            complete: true,
            values: BTreeMap::from([(7, b"blocked".to_vec())]),
        },
        BTreeMap::from([
            ("alpha".to_owned(), Vec::new()),
            ("beta".to_owned(), Vec::new()),
        ]),
    )
    .unwrap()
    .encode();
    let selected_scope = config.profile.delivery.scope;
    let profile = config.profile.clone();
    let store = config.store.clone();
    let directory = root.0.clone();
    let actor_profile = actor.clone();
    let operator_profile = operator.clone();
    let endpoints = sockets.clone();
    let first_human = human_for(reviewer.clone(), 7);
    let second_human = (outcome != Outcome::SourceChanges).then(|| {
        let reviewer = reviewer.clone();
        let endpoints = sockets.clone();
        std::thread::spawn(move || {
            let mut client = reviewer
                .connect_client_waiting(8, Duration::from_secs(10))
                .unwrap();
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(10));
                match client.step().unwrap() {
                    ReviewClientProgress::NeedsDecision => {
                        let packet = client.packet().unwrap();
                        assert_eq!(packet.policy_generation(), 2);
                        assert_eq!(packet.action().spec().policy_epoch, 1);
                        assert_eq!(packet.action().spec().payload, b"second");
                        assert_eq!(packet.views().len(), 2);
                        assert!(endpoints.iter().all(|path| !path.exists()));
                        client
                            .respond(if outcome == Outcome::HumanRejects {
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
    });
    let peer = std::thread::spawn(move || {
        let mut actor = connect(&actor_profile);
        wait(&operator_profile.socket);
        let mut operator = UnixStream::connect(&operator_profile.socket).unwrap();
        operator_profile.supervisor.verify(&operator).unwrap();
        wait(&endpoints[0]);
        let before = store_files(&store);
        passed(&raw_exchange(&mut actor, &checks[0]), "81");
        // A second transport cannot replenish the exact same selected grant.
        exhausted_native(&actor_profile, &endpoints[0], candidates[0].clone());
        assert_eq!(store_files(&store), before);
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
        wait(&endpoints[1]);
        assert!(!endpoints[0].exists());
        let before = store_files(&store);
        passed(&raw_exchange(&mut actor, &checks[1]), "82");
        assert_eq!(store_files(&store), before);
        let cut = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(
            (
                cut.executions,
                cut.control.ledger.charged,
                cut.control.ledger.epoch
            ),
            (1, 7, 0)
        );
        let source = directory.join("evidence.json");
        let source_bytes = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        write_complete(
            &directory.join("governor-review.json"),
            review_document(&cut, selected_scope),
        );
        wait(&directory.join("governor-approved.json"));
        let approved = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(approved.revision, cut.revision + 2);
        let before = store_files(&store);
        withheld(&raw_exchange(&mut actor, &checks[1]));
        assert!(!endpoints[1].exists());
        assert!(UnixStream::connect(&endpoints[1]).is_err());
        assert_eq!(
            store_files(&store),
            before,
            "retired disclosure query performed owner work"
        );
        let plan = raw_exchange(&mut operator, &next(selected_scope));
        let value = field(known_operator(&plan, "policy-promotion-next"), "value");
        assert_eq!(field(value, "can_promote").as_bool(), Some(true));
        let command = governor::protocol::json(field(value, "action"));
        let denied = raw_exchange(&mut actor, &command);
        assert_eq!(
            field(field(&denied, "envelope"), "reason_code").as_str(),
            Some("DeniedByPolicy")
        );
        let promoted = raw_exchange(&mut operator, &command);
        let promotion = known_operator(&promoted, "policy-promotion");
        assert_eq!(
            field(field(promotion, "basis"), "authority_epoch").as_str(),
            Some("1")
        );
        let after = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(
            (
                after.executions,
                after.control.ledger.charged,
                after.control.ledger.epoch
            ),
            (1, 7, 1)
        );
        let before = store_files(&store);
        withheld(&raw_exchange(&mut actor, &checks[1]));
        withheld(&raw_exchange(
            &mut actor,
            &checks[1].replace(
                "\"expected_policy_epoch\":\"0\"",
                "\"expected_policy_epoch\":\"1\"",
            ),
        ));
        assert_eq!(
            store_files(&store),
            before,
            "promotion implicitly renewed the disclosure or read the missing source"
        );
        assert!(!source.exists());
        assert!(executed(&raw_exchange(&mut actor, &first)));
        write_complete(
            &source,
            if outcome == Outcome::SourceChanges {
                blocked
            } else {
                source_bytes
            },
        );
        write_complete(&directory.join("helper-epoch"), b"1");
        if let Some(capture) = second_capture {
            write_complete(&directory.join("original.bin"), &capture);
            write_complete(&directory.join("current.bin"), &capture);
        }
        let reply = finish(&mut actor, &second, &second_poll);
        assert_eq!(executed(&reply), outcome == Outcome::Publish);
        if outcome == Outcome::SourceChanges {
            assert_eq!(field(knowledge(&reply), "value").as_str(), Some("denied"));
        }
    });
    let served = service::serve_tower_governed_prechecked(
        config,
        &actor,
        &reviewer,
        publication.as_ref().map(|(profile, _)| profile),
        &[7, 8],
        GENERATION,
        &operator,
        &governor,
        topology,
        &schedule,
        || ElapsedTick(1000),
    );
    let peer_result = peer.join();
    let first_result = first_human.join();
    let second_result = second_human.map(|human| human.join());
    let governor_status = governor_child.finish();
    served.unwrap();
    peer_result.unwrap();
    first_result.unwrap();
    if let Some(result) = second_result {
        result.unwrap();
    }
    assert!(governor_status.success());
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let effects = if outcome == Outcome::Publish { 2 } else { 1 };
    assert_eq!(
        (
            disk.executions,
            disk.control.ledger.charged,
            disk.control.ledger.reserved
        ),
        (effects, 7 * effects, 0)
    );
    assert_eq!(disk.control.ledger.epoch, 1);
    assert_eq!(
        disk.payload,
        if outcome == Outcome::Publish {
            b"second".as_slice()
        } else {
            b"first".as_slice()
        }
    );
    assert!(disk.stop.is_none());
    if mediated {
        let saved = TopologyCheckpoint::read_trusted(
            &mut Cursor::new(fs::read(root.0.join("source-checkpoint.bin")).unwrap()),
            MAX_TOPOLOGY_CHECKPOINT_BYTES,
        )
        .unwrap();
        assert_eq!(saved.history_anchor().revision(), disk.revision);
        assert_eq!(saved.work().checks, 3);
    }
    assert!(sockets.iter().all(|path| !path.exists()));
    for socket in [
        &actor.socket,
        &operator.socket,
        &governor.socket,
        &reviewer.socket(7),
        &reviewer.socket(8),
    ] {
        assert!(!socket.exists());
    }
}

#[test]
fn governed_later_precheck_refuses_a_future_epoch_without_waiting_or_inference() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let operator = operator_settings(&root, &config);
    let governor = governor::Settings::decode(
        governor_document(
            &root,
            &config,
            std::process::id().checked_add(10_000).unwrap(),
        )
        .as_bytes(),
    )
    .unwrap();
    let (schedule, sockets, _) = schedule(&root, 1);
    let first = document(&config, 7, 1, "6669727374");
    let first_poll = poll(&config, 7);
    let actor_peer = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut actor = connect(&actor_peer);
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
    });
    let human = human_for(reviewer.clone(), 7);
    let error = service::serve_tower_governed_prechecked(
        config,
        &actor,
        &reviewer,
        None,
        &[7, 8],
        GENERATION,
        &operator,
        &governor,
        None,
        &schedule,
        || ElapsedTick(1000),
    )
    .unwrap_err();
    peer.join().unwrap();
    human.join().unwrap();
    assert!(error.contains("expected policy epoch"), "{error}");
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(
        (
            disk.executions,
            disk.control.ledger.charged,
            disk.control.ledger.reserved
        ),
        (1, 7, 0)
    );
    assert_eq!(disk.payload, b"first");
    assert_eq!(disk.control.ledger.stages.len(), 1);
    assert!(sockets.iter().all(|path| !path.exists()));
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn governed_schedule_reserves_every_future_disclosure_and_loaded_input_before_clock() {
    for dimension in 0..5 {
        let root = Directory::new();
        let mut config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let operator = operator_settings(&root, &config);
        let (schedule, sockets, profiles) = schedule(&root, 0);
        let selected_path = match dimension {
            0 => sockets[1].clone(),
            1 | 4 => {
                fs::remove_file(&profiles[1]).unwrap();
                profiles[1].clone()
            }
            2 => {
                let path = root.0.join("precheck-schedule.json");
                fs::remove_file(&path).unwrap();
                path
            }
            _ => root.0.join("evidence.json"),
        };
        if dimension == 4 {
            let alias = root.0.join("input-alias");
            std::os::unix::fs::symlink(&root.0, &alias).unwrap();
            config.source = FileEvidenceSource::new(
                alias.join(profiles[1].file_name().unwrap()),
                51,
                config.profile.delivery.scope,
                1048576,
            )
            .unwrap();
        }
        let mut governor = governor::Settings::decode(
            governor_document(
                &root,
                &config,
                std::process::id().checked_add(10_000).unwrap(),
            )
            .as_bytes(),
        )
        .unwrap();
        governor.socket = selected_path.clone();
        let store = config.store.clone();
        let error = service::serve_tower_governed_prechecked(
            config,
            &actor,
            &reviewer,
            None,
            &[7, 8],
            GENERATION,
            &operator,
            &governor,
            None,
            &schedule,
            || panic!("future precheck/governor alias reached the clock"),
        )
        .unwrap_err();
        assert!(error.contains("custody"), "dimension {dimension}: {error}");
        assert!(!store.exists());
        assert!(!selected_path.exists());
        assert!(!actor.socket.exists());
        assert!(!operator.socket.exists());
        assert!(sockets.iter().all(|path| !path.exists()));
        assert!(!root.0.join("evidence.json").exists());
    }
}
