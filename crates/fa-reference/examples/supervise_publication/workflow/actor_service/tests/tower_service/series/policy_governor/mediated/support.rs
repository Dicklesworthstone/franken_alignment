//! Original guarded owners, independently retained source custody and actual
//! actor/human/helper traffic. No fabricated request receipts or governor keys.
use super::*;
use crate::workflow::actor_service::OwnerStart;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, mediated::FileMediatedBootstrap,
};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::round::Verdict;

const HELPER_MEMBER: &str = "FA_GOVERNED_MEDIATED_HELPER_MEMBER";
const HELPER_EPOCH: &str = "FA_GOVERNED_MEDIATED_HELPER_EPOCH_FILE";
const HELPER_TEST: &str = "workflow::actor_service::tests::tower_service::series::policy_governor::mediated::support::independently_provisioned_helper_process";
const CHECKS: u64 = 8;

pub(super) fn mediated_config(root: &Directory) -> Config {
    let mut config = configured(root);
    config.programs = ["alpha", "beta"]
        .into_iter()
        .map(|member| {
            (
                member.to_owned(),
                HelperProgram::new(
                    std::env::current_exe().unwrap(),
                    root.0.clone(),
                    vec!["--exact".into(), HELPER_TEST.into(), "--nocapture".into()],
                    BTreeMap::from([
                        (OsString::from(HELPER_MEMBER), OsString::from(member)),
                        (
                            OsString::from(HELPER_EPOCH),
                            root.0.join("helper-epoch").into_os_string(),
                        ),
                    ]),
                )
                .unwrap(),
            )
        })
        .collect();
    config
}

#[test]
fn independently_provisioned_helper_process() {
    let Ok(member) = std::env::var(HELPER_MEMBER) else {
        return;
    };
    let path = std::env::var_os(HELPER_EPOCH).unwrap();
    // The fixture operator provisions the expected epoch before admission. The
    // worker never derives this independent expectation from request bytes.
    let epoch = fs::read_to_string(path).unwrap().parse::<u64>().unwrap();
    let config = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    )))
    .unwrap();
    let expected = config.profile.committee.members()[&member].profile_at(epoch);
    let mut client = HelperClient::from_process_stdin(expected).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let own = format!("review this fixture: {member}");
            assert!(input.actual_input().submitted_bytes().windows(own.len()).any(|bytes| bytes == own.as_bytes()));
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        pause(1);
    }
}

fn graph(generation: u64) -> String {
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology.json"))
        .replacen("\"generation\": 1", &format!("\"generation\": {generation}"), 1)
        .replace("\"inventory_generation\": 1", &format!("\"inventory_generation\": {generation}"))
        .replace("\"complete_inventory_generation\": 1", &format!("\"complete_inventory_generation\": {generation}"))
        .replace("\"checks\": 4096", &format!("\"checks\": {CHECKS}"))
        .replace("\"read_bytes\": 16777216", &format!("\"read_bytes\": {}", CHECKS * TOPOLOGY_REFRESH_READ_BYTES))
}

fn documents(root: &Directory, generation: u64) {
    fs::write(root.0.join(format!("topology-{generation}.json")), graph(generation)).unwrap();
    fs::write(root.0.join(format!("inventory-{generation}.json")), include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"
    )))
    .unwrap();
}

pub(super) fn creation_selection(root: &Directory) -> CreationSelection {
    documents(root, 1);
    fs::copy(root.0.join("topology-1.json"), root.0.join("initial-topology.json")).unwrap();
    fs::copy(root.0.join("inventory-1.json"), root.0.join("initial-inventory.json")).unwrap();
    CreationSelection::decode(format!(
        r#"{{"schema_version":"fa.mediated-create/2","topology_path":{:?},"inventory_path":{:?},"retain_checkpoint":{:?}}}"#,
        root.0.join("topology-1.json").to_str().unwrap(),
        root.0.join("inventory-1.json").to_str().unwrap(),
        root.0.join("source-1.bin").to_str().unwrap(),
    ).as_bytes()).unwrap()
}

pub(super) fn next_selection(root: &Directory, before: &TopologyCheckpoint) -> RecoverySelection {
    documents(root, 2);
    RecoverySelection::decode(format!(
        concat!(
            "{{\"schema_version\":\"fa.mediated-submit/2\",",
            "\"initial\":{{\"topology_path\":{:?},\"inventory_path\":{:?}}},",
            "\"source_checkpoint\":{:?},\"next\":{{\"topology_path\":{:?},\"inventory_path\":{:?}}},",
            "\"operation\":{},\"retain_checkpoint\":{:?}}}"
        ),
        root.0.join("initial-topology.json").to_str().unwrap(),
        root.0.join("initial-inventory.json").to_str().unwrap(),
        root.0.join("source-1.bin").to_str().unwrap(),
        root.0.join("topology-2.json").to_str().unwrap(),
        root.0.join("inventory-2.json").to_str().unwrap(),
        before.last_operation() + 1,
        root.0.join("source-2.bin").to_str().unwrap(),
    ).as_bytes()).unwrap()
}

pub(super) fn read_checkpoint(root: &Directory, name: &str) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(root.0.join(name)).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

pub(super) fn read_anchor(root: &Directory, name: &str) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(root.0.join(name)).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap()
}

pub(super) fn seed(
    root: &Directory,
    checked: bool,
    two_history: bool,
) -> (Config, SubmitProfile, TopologyCheckpoint) {
    let config = mediated_config(root);
    evidence(root, &config);
    write_complete(&root.0.join("helper-epoch"), b"0");
    let selected = creation_selection(root);
    let publication = checked.then(|| checked::prepare(root, &config).0);
    let second_capture = checked.then(|| checked::capture(&config, 2, 0, b"second"));
    let (mut actor, reviewer) = profiles(root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    actor.reply_ms = 1000;
    let governor = governor::Settings::decode(
        governor_document(root, &config, std::process::id() + 1).as_bytes(),
    )
    .unwrap();
    let guards = FileGuardSet {
        stream: None,
        decoder: None,
        decoder_stop: None,
        source: Some(config.source_policy),
        identity: None,
        campaigns: Some(governor.campaign_requirement()),
        credential: None,
    };
    let mut prepared = selected.prepare(&config, &mut || Ok(())).unwrap();
    let output = prepared.take_output().unwrap();
    let (host, roles) = FileOversight::create_mediated_guarded_reserved(
        &config.store,
        config.profile.clone(),
        &guards,
        None,
        FileMediatedBootstrap {
            topology: prepared.loaded().graph().clone(),
            reserve: RecoveryReserve::terminal(),
            publication: publication.as_ref().map(|profile| profile.limits),
        },
        None,
        None,
    )
    .unwrap();
    let source = prepared.bind(&host, roles.topology_observer).unwrap();
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let directory = root.0.clone();
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_actor);
        assert!(executed(&finish(&mut socket, &first, &first_poll)));
        if two_history {
            wait(&next_control);
            if let Some(capture) = second_capture {
                write_complete(&directory.join("original.bin"), &capture);
                write_complete(&directory.join("current.bin"), &capture);
            }
            assert!(executed(&finish(&mut socket, &second, &second_poll)));
        }
    });
    let first_human = human_for(reviewer.clone(), 7);
    let second_human = two_history.then(|| human_for(reviewer.clone(), 8));
    let socket = BoundSocket::bind(&actor.socket, None).unwrap();
    actor.secure_socket().unwrap();
    let requests: &[u64] = if two_history { &[7, 8] } else { &[7] };
    let result = service::serve_ready_ingress(
        config,
        &actor,
        &reviewer,
        publication.as_ref(),
        Options { requests, open: false, credibility: None },
        Some(GENERATION),
        None,
        socket,
        host,
        roles.oversight.human,
        Some(source),
        OwnerStart::Ordinary,
        Deadline { logical: ElapsedTick(16000), started: Instant::now(), wall: Duration::from_millis(15000) },
        || ElapsedTick(1000),
        |host, source, succeeded| {
            assert!(succeeded);
            output.retain(host, source)?;
            let mut bytes = Vec::new();
            host.history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
            write_complete(&root.0.join("before.anchor"), bytes);
            Ok(())
        },
    );
    peer.join().unwrap();
    first_human.join().unwrap();
    if let Some(human) = second_human { human.join().unwrap(); }
    result.unwrap();
    let config = mediated_config(root);
    let saved = read_checkpoint(root, "source-1.bin");
    assert_eq!(saved.history_anchor(), &read_anchor(root, "before.anchor"));
    assert_eq!(saved.work().checks, if two_history { 3 } else { 2 });
    assert_eq!(saved.last_operation(), saved.work().checks);
    let floor = saved.minimum();
    let recovery = SubmitProfile::decode(format!(
        concat!(
            "{{\"schema_version\":\"fa.guarded-submit/2\",\"minimum\":{{",
            "\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},",
            "\"history_anchor\":{:?},\"retain_history_anchor\":{:?},",
            "\"effective_policy\":{{\"generation\":1,\"nodes\":[",
            "{{\"op\":\"exact_value\",\"key\":7,\"value_hex\":\"6f6b\"}},",
            "{{\"op\":\"payload_at_most\",\"maximum\":512}},",
            "{{\"op\":\"units_at_most\",\"maximum\":512}},",
            "{{\"op\":\"all\",\"children\":[0,1,2]}}]}},",
            "\"campaigns\":{{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":4}}}}"
        ),
        floor.journal_revision, floor.control_sequence, floor.authority_epoch,
        root.0.join("before.anchor").to_str().unwrap(),
        root.0.join("after.anchor").to_str().unwrap(),
    ).as_bytes()).unwrap();
    (config, recovery, saved)
}
