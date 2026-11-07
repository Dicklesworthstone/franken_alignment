//! Common Tower bytes drive the actual existing helper/human/publication path.
//! Synthetic helper verdicts are fixture inputs, not model or deployment evidence.

use super::*;
use fa_reference::strict_json::{self, Json, Limits};
use std::io::{Read, Write};

const GENERATION: u64 = 11;

fn proposal_document(config: &Config, request: u64) -> String {
    let scope = config.profile.delivery.scope;
    let target = config.profile.delivery.target;
    // Independent wire spelling, not encode_request reproducing its own parser.
    format!(
        concat!(
            "{{\"schema_version\":\"fa.tower_request/1\",\"operation_id\":\"tower-submit\",",
            "\"address\":\"fa://{}/run/{}@{}\",\"verb\":\"propose\",",
            "\"arguments\":{{\"request\":\"{}\",\"proposal\":{{\"target\":{{",
            "\"adapter\":\"{}\",\"object\":\"{}\",\"contract_version\":\"{}\",",
            "\"expected_version\":\"{}\",\"generation\":\"{}\"}},",
            "\"payload_hex\":\"6e6574776f726b\",\"units\":\"7\",\"deadline\":\"100000\",",
            "\"expected_policy_epoch\":\"0\"}}}},\"idempotency_key\":\"{}\"}}"
        ),
        scope.tenant,
        scope.run,
        GENERATION,
        request,
        target.adapter,
        target.object,
        target.contract_version,
        target.expected_version,
        target.generation,
        request
    )
}

fn get_document(config: &Config) -> String {
    let scope = config.profile.delivery.scope;
    format!(
        concat!(
            "{{\"schema_version\":\"fa.tower_request/1\",\"operation_id\":\"tower-read\",",
            "\"address\":\"fa://{}/attempt/request-{}-{}-{}-{}-7@{}\",",
            "\"verb\":\"get\",\"arguments\":null,\"idempotency_key\":null}}"
        ),
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority, GENERATION
    )
}

fn prepared_cli(root: &Directory, config: &Config) -> Vec<String> {
    let profile = root.0.join("tower-actor.json");
    let proposed = root.0.join("tower-propose.json");
    let get = root.0.join("tower-get.json");
    fs::write(&profile, actor_json(root, config)).unwrap();
    // Pretty-printed documents must be canonicalized to exactly one input line.
    fs::write(
        &proposed,
        proposal_document(config, 7).replace(",\"", ",\n\""),
    )
    .unwrap();
    fs::write(&get, get_document(config)).unwrap();
    vec![
        "tower-exchange".to_owned(),
        profile.to_str().unwrap().to_owned(),
        proposed.to_str().unwrap().to_owned(),
        get.to_str().unwrap().to_owned(),
    ]
}

fn parse(bytes: &[u8]) -> Json {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: 16 * 1024,
            max_depth: 16,
            max_items: 2048,
            max_string_bytes: 8192,
        },
    )
    .unwrap()
}

fn field<'a>(json: &'a Json, key: &str) -> &'a Json {
    json.get(key).unwrap()
}
fn native(json: &Json) -> &Json {
    let result = field(json, "result");
    assert_eq!(field(result, "kind").as_str(), Some("publication-request"));
    field(result, "value")
}
fn knowledge(json: &Json) -> &Json {
    field(native(json), "knowledge")
}

fn serve_tower(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    open: bool,
    at: u64,
) -> Result<(), String> {
    serve_selected_ingress(
        config,
        actor,
        reviewer,
        None,
        open,
        None,
        None,
        None,
        Some(GENERATION),
        || ElapsedTick(at),
    )
}

#[test]
fn tower_cli_submits_and_reads_through_both_original_keys_and_one_effect() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let args = prepared_cli(&root, &config);
    let socket = actor.socket.clone();
    let peer = std::thread::spawn(move || {
        wait(&socket);
        let mut output = Vec::new();
        crate::command_with_output(args, &mut output).unwrap();
        output
    });
    let human = human(reviewer.clone());
    serve_tower(config, &actor, &reviewer, false, 1000).unwrap();
    human.join().unwrap();
    let output = peer.join().unwrap();
    let lines: Vec<_> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        2,
        "one complete reply per explicit input document"
    );
    let submitted = parse(lines[0]);
    assert_eq!(
        field(knowledge(&submitted), "state").as_str(),
        Some("pending")
    );
    assert_eq!(field(native(&submitted), "request").as_str(), Some("7"));
    let observed = parse(lines[1]);
    assert_eq!(field(native(&observed), "request").as_str(), Some("7"));
    assert_eq!(
        field(field(&observed, "envelope"), "operation_id").as_str(),
        Some("tower-read")
    );
    // CLI success means complete exchanges; the native owner independently
    // establishes actual completion below. Pending was not promoted in output.
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.payload, b"network");
    assert_eq!(
        (disk.control.ledger.charged, disk.control.ledger.reserved),
        (7, 0)
    );
    assert!(disk.stop.is_none());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
}

#[test]
fn tower_reopened_original_request_is_observation_only_without_source_or_review() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let native_path = root.0.join("original-submit.json");
    fs::write(&native_path, encode_command(&proposal(&config)).unwrap()).unwrap();
    let peer = actor_submit(actor.clone(), native_path);
    let human = human(reviewer.clone());
    serve(config, &actor, &reviewer, None, false, || ElapsedTick(1000)).unwrap();
    peer.join().unwrap();
    human.join().unwrap();

    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = configured(&root);
    config.programs.clear();
    let args = prepared_cli(&root, &config);
    let socket = actor.socket.clone();
    let peer = std::thread::spawn(move || {
        wait(&socket);
        let mut output = Vec::new();
        crate::command_with_output(args, &mut output).unwrap();
        output
    });
    serve_tower(config, &actor, &reviewer, true, 200000).unwrap();
    let output = peer.join().unwrap();
    for line in output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let reply = parse(line);
        assert_eq!(field(knowledge(&reply), "state").as_str(), Some("known"));
        assert_eq!(field(knowledge(&reply), "value").as_str(), Some("executed"));
    }
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.control.ledger.charged, 7);
    assert!(!root.0.join("evidence.json").exists());
    assert!(!reviewer.socket(7).exists());
}

fn raw_exchange(socket: &mut UnixStream, document: &str) -> Json {
    // A test peer, not another production transport: finite blocking socket
    // deadlines keep actual frame interactions easy to inspect independently.
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket.write_all(document.as_bytes()).unwrap();
    socket.write_all(b"\n").unwrap();
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        assert!(response.len() < 16 * 1024);
        socket.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            return parse(&response);
        }
        response.push(byte[0]);
    }
}

#[test]
fn unselected_tower_proposal_cannot_acquire_source_before_the_selected_effect() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let source_path = root.0.join("evidence.json");
    let source_bytes = fs::read(&source_path).unwrap();
    fs::remove_file(&source_path).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let foreign = proposal_document(&config, 8);
    let selected = proposal_document(&config, 7);
    let malformed = selected.replacen("\"arguments\":{", "\"arguments\":{\"operator\":true,", 1);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile);
        let refused = raw_exchange(&mut socket, &malformed);
        assert_eq!(
            field(field(&refused, "envelope"), "operation_id").as_str(),
            Some("<invalid-request>")
        );
        let refused = raw_exchange(&mut socket, &foreign);
        assert_eq!(
            field(field(&refused, "envelope"), "reason_code").as_str(),
            Some("DeniedByPolicy")
        );
        assert!(!source_path.exists());
        // The paired allowed request now supplies actual readable source data.
        fs::write(&source_path, source_bytes).unwrap();
        let response = raw_exchange(&mut socket, &selected);
        assert_eq!(
            field(knowledge(&response), "state").as_str(),
            Some("pending")
        );
    });
    let human = human(reviewer.clone());
    serve_tower(config, &actor, &reviewer, false, 1000).unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.control.ledger.charged, 7);
    assert_eq!(disk.control.ledger.stages.len(), 1);
    assert!(disk.stop.is_none());
}

#[test]
fn independent_stop_remains_live_while_tower_actor_holds_an_incomplete_frame() {
    let root = Directory::new();
    let config = configured(&root);
    // There is deliberately no source file and no approving human. Neither is
    // needed for the original independent stop authority to drain this owner.
    let (actor, reviewer) = profiles(&root, &config);
    let peer_profile = actor.clone();
    let stop_profile = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_profile);
        socket.set_nonblocking(false).unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        let stop = crate::workflow::control::socket_path(&stop_profile, 7);
        wait(&stop);
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_profile, 7, &mut output).unwrap();
        let receipt = strict_json::parse(&output, Limits::default()).unwrap();
        assert_eq!(field(&receipt, "status").as_str(), Some("stopped_drained"));
    });
    serve_tower(config, &actor, &reviewer, false, 1000).unwrap();
    peer.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert!(disk.stop.is_some());
    assert!(disk.control.ledger.stages.is_empty());
    assert_eq!(disk.executions, 0);
    assert_eq!(
        (disk.control.ledger.charged, disk.control.ledger.reserved),
        (0, 0)
    );
    assert!(!root.0.join("evidence.json").exists());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
}

#[path = "tower_service/series.rs"]
mod series;
