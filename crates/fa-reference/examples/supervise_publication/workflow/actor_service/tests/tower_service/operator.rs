//! Independently authenticated supervisor reads during the real live workflow.
//! Helper verdict fixtures establish control behavior, not model safety evidence.

use super::*;
use crate::workflow::actor_service::operator::Settings;
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use std::sync::mpsc;

fn settings_document(
    root: &Directory,
    config: &Config,
    inspection: usize,
    explanation: usize,
) -> String {
    let id = credentials();
    let (replay, disclosure) = if explanation == 0 {
        (0, "none")
    } else {
        (1024, "policy_and_selected_witnesses")
    };
    format!(
        r#"{{"schema":"fa.tower-operator-service/1","scope":{},"socket":"{}/actors/operator.sock","supervisor":{{"uid":{},"gid":{},"pid":{}}},"operator":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"connection_limit":4,"exchange_limit":64,"runtime_ms":15000,"poll_ms":1,"inspection_queries":{inspection},"explanation_queries":{explanation},"explanation_replay_events":{replay},"explanation_disclosure":"{disclosure}"}}"#,
        scope(config),
        root.0.display(),
        id.uid(),
        id.gid(),
        id.pid(),
        id.uid(),
        id.gid(),
        id.pid()
    )
}

fn settings(root: &Directory, config: &Config) -> Settings {
    Settings::decode(settings_document(root, config, 8, 8).as_bytes()).unwrap()
}

fn situation_document(config: &Config) -> String {
    let scope = config.profile.delivery.scope;
    format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"operator-situation","address":"fa://{}/run/{}@{GENERATION}","verb":"situation","arguments":null,"idempotency_key":null}}"#,
        scope.tenant, scope.run
    )
}

fn explain_document(config: &Config) -> String {
    get_document(config)
        .replace("\"verb\":\"get\"", "\"verb\":\"explain\"")
        .replace("tower-read", "operator-explain")
}

fn connect_operator(settings: &Settings) -> UnixStream {
    wait(&settings.socket);
    let socket = UnixStream::connect(&settings.socket).unwrap();
    settings.supervisor.verify(&socket).unwrap();
    socket
}

fn native_operator<'a>(reply: &'a Json, kind: &str) -> &'a Json {
    let result = field(reply, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    field(result, "value")
}

fn known_operator<'a>(reply: &'a Json, kind: &str) -> &'a Json {
    let observed = field(native_operator(reply, kind), "knowledge");
    assert_eq!(field(observed, "state").as_str(), Some("known"));
    assert_eq!(
        field(field(observed, "basis"), "origin").as_str(),
        Some("acknowledged_owner")
    );
    observed
}

fn situation_summary(reply: &Json) -> &Json {
    field(
        field(known_operator(reply, "situation"), "value"),
        "summary",
    )
}

fn reason(reply: &Json, expected: &str) {
    assert_eq!(
        field(field(reply, "envelope"), "reason_code").as_str(),
        Some(expected)
    );
}

fn store_files(store: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(store)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn serve_operator(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    settings: &Settings,
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
        Some(settings),
        || ElapsedTick(at),
    )
}

fn prepared_operator_cli(
    root: &Directory,
    config: &Config,
    inspection: usize,
    explanation: usize,
) -> Vec<String> {
    let profile = root.0.join("operator-profile.json");
    let situation = root.0.join("operator-situation.json");
    let explanation = root.0.join("operator-explanation.json");
    fs::write(
        &profile,
        settings_document(root, config, inspection, explanation),
    )
    .unwrap();
    fs::write(&situation, situation_document(config)).unwrap();
    fs::write(&explanation, explain_document(config)).unwrap();
    vec![
        "tower-operator-exchange".to_owned(),
        profile.to_str().unwrap().to_owned(),
        situation.to_str().unwrap().to_owned(),
        explanation.to_str().unwrap().to_owned(),
    ]
}

fn wait_executed(socket: &mut UnixStream, poll: &str) -> Json {
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let reply = raw_exchange(socket, poll);
        let observed = knowledge(&reply);
        if field(observed, "state").as_str() == Some("known") {
            assert_eq!(field(observed, "value").as_str(), Some("executed"));
            return reply;
        }
        assert!(matches!(
            field(observed, "state").as_str(),
            Some("pending" | "unknown")
        ));
        pause(1);
    }
}

fn published_once(root: &Directory) {
    let config = configured(root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.payload, b"network");
    assert_eq!(
        (disk.control.ledger.charged, disk.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(disk.control.ledger.stages.len(), 1);
    assert!(disk.stop.is_none());
}

#[test]
fn operator_reads_original_admission_during_human_review_and_after_publication_without_actor_disclosure()
 {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.reply_ms = 1000;
    let settings = Settings::decode(settings_document(&root, &config, 3, 2).as_bytes()).unwrap();
    let operator_args = prepared_operator_cli(&root, &config, 3, 2);
    let situation = situation_document(&config);
    let explanation = explain_document(&config);
    let submitted = proposal_document(&config, 7);
    let poll = get_document(&config);
    let store = config.store.clone();
    let source = root.0.join("evidence.json");
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let (waiting_send, waiting_receive) = mpsc::channel();
    let (approve_send, approve_receive) = mpsc::channel();
    let human_profile = reviewer.clone();
    let human = std::thread::spawn(move || {
        wait(&human_profile.socket(7));
        let mut client = human_profile.connect_client(7).unwrap();
        let started = Instant::now();
        let mut offered = false;
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(!offered);
                    offered = true;
                    waiting_send.send(()).unwrap();
                    approve_receive
                        .recv_timeout(Duration::from_secs(10))
                        .unwrap();
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    });
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_profile);
        let initial = store_files(&store);
        let empty = raw_exchange(&mut operator, &situation);
        assert_eq!(
            field(situation_summary(&empty), "attempts").as_str(),
            Some("0")
        );
        assert_eq!(
            store_files(&store),
            initial,
            "operator read changed the original journal"
        );
        let mut actor = connect(&profile);
        reason(&raw_exchange(&mut actor, &situation), "DeniedByPolicy");
        reason(&raw_exchange(&mut actor, &explanation), "DeniedByPolicy");
        assert_eq!(
            store_files(&store),
            initial,
            "actor refusal entered a disclosure or source path"
        );
        let admitted = raw_exchange(&mut actor, &submitted);
        assert_eq!(
            field(knowledge(&admitted), "state").as_str(),
            Some("pending")
        );
        waiting_receive
            .recv_timeout(Duration::from_secs(10))
            .unwrap();

        let held = raw_exchange(&mut operator, &situation);
        assert_eq!(
            field(situation_summary(&held), "attempts").as_str(),
            Some("1")
        );
        assert_eq!(
            field(situation_summary(&held), "pending_attempts").as_str(),
            Some("1")
        );
        assert_eq!(
            field(situation_summary(&held), "reserved_units").as_str(),
            Some("0")
        );
        assert_eq!(
            field(situation_summary(&held), "executions").as_str(),
            Some("0")
        );
        let explained = raw_exchange(&mut operator, &explanation);
        let native = native_operator(&explained, "explanation");
        assert_eq!(
            field(native, "scope").as_str(),
            Some("historical_exact_policy_admission")
        );
        assert_eq!(field(native, "effect_authority").as_bool(), Some(false));
        let observation = known_operator(&explained, "explanation");
        assert_eq!(
            field(field(observation, "basis"), "request").as_str(),
            Some("7")
        );
        let value = field(observation, "value");
        assert_eq!(field(value, "initial_state").as_str(), Some("reviewing"));
        assert_eq!(field(value, "ending_state").as_str(), Some("reviewing"));
        assert_eq!(field(value, "result").as_str(), Some("satisfied"));
        assert!(
            !field(field(value, "predicate_tree"), "nodes")
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            field(value, "witness_values_disclosed").as_bool(),
            Some(false)
        );
        approve_send.send(()).unwrap();
        drop(operator);
        wait_executed(&mut actor, &poll);

        // Original read grants survive review and are still usable by the
        // independent public-profile client during terminal reply grace.
        fs::remove_file(&source).unwrap();
        let settled = store_files(&store);
        let mut output = Vec::new();
        crate::command_with_output(operator_args, &mut output).unwrap();
        let replies: Vec<_> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(parse)
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(
            field(situation_summary(&replies[0]), "executions").as_str(),
            Some("1")
        );
        assert_eq!(
            field(situation_summary(&replies[0]), "charged_units").as_str(),
            Some("7")
        );
        assert_eq!(
            field(
                field(known_operator(&replies[1], "explanation"), "value"),
                "ending_state"
            )
            .as_str(),
            Some("confirmed")
        );
        // Neither entering human review nor completing publication renewed the
        // original two independent read grants during terminal reply grace.
        let mut exhausted = connect_operator(&operator_profile);
        reason(&raw_exchange(&mut exhausted, &situation), "BudgetExhausted");
        reason(
            &raw_exchange(&mut exhausted, &explanation),
            "BudgetExhausted",
        );
        assert_eq!(store_files(&store), settled);
        assert!(!source.exists());
    });
    serve_operator(config, &actor, &reviewer, &settings, false, 1000).unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    published_once(&root);
    assert!(!actor.socket.exists());
    assert!(!settings.socket.exists());
    assert!(!reviewer.socket(7).exists());
}

#[test]
fn operator_cannot_propose_or_acquire_missing_source_but_the_same_actor_proposal_publishes() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let source = root.0.join("evidence.json");
    let source_bytes = fs::read(&source).unwrap();
    fs::remove_file(&source).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root, &config);
    let document = proposal_document(&config, 7);
    let situation = situation_document(&config);
    let store = config.store.clone();
    let review_socket = reviewer.socket(7);
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_profile);
        let before = store_files(&store);
        reason(&raw_exchange(&mut operator, &document), "PurposeMismatch");
        let empty = raw_exchange(&mut operator, &situation);
        assert_eq!(
            field(situation_summary(&empty), "retained_requests").as_str(),
            Some("0")
        );
        assert_eq!(
            field(situation_summary(&empty), "attempts").as_str(),
            Some("0")
        );
        assert_eq!(store_files(&store), before);
        assert!(!source.exists());
        assert!(!review_socket.exists());
        fs::write(&source, source_bytes).unwrap();
        raw_exchange(&mut connect(&profile), &document)
    });
    let human = human(reviewer.clone());
    serve_operator(config, &actor, &reviewer, &settings, false, 1000).unwrap();
    let admitted = peer.join().unwrap();
    assert_eq!(
        field(knowledge(&admitted), "state").as_str(),
        Some("pending")
    );
    human.join().unwrap();
    published_once(&root);
}

#[test]
fn recovered_operator_observations_need_no_source_helpers_or_new_effect() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.reply_ms = 1000;
    let document = proposal_document(&config, 7);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || raw_exchange(&mut connect(&profile), &document));
    let human = human(reviewer.clone());
    serve_tower(config, &actor, &reviewer, false, 1000).unwrap();
    peer.join().unwrap();
    human.join().unwrap();

    let source = root.0.join("evidence.json");
    fs::remove_file(&source).unwrap();
    let mut config = configured(&root);
    config.programs.clear();
    let settings = settings(&root, &config);
    let actor_args = prepared_cli(&root, &config);
    let operator_args = prepared_operator_cli(&root, &config, 8, 8);
    let socket = settings.socket.clone();
    let store = config.store.clone();
    let review_socket = reviewer.socket(7);
    let peer = std::thread::spawn(move || {
        wait(&socket);
        let mut actor_output = Vec::new();
        crate::command_with_output(actor_args, &mut actor_output).unwrap();
        for line in actor_output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let reply = parse(line);
            assert_eq!(field(knowledge(&reply), "state").as_str(), Some("known"));
            assert_eq!(field(knowledge(&reply), "value").as_str(), Some("executed"));
        }
        // Opening/resuming has already performed its original recovery work;
        // subsequent operator reads must leave that acknowledged image alone.
        let before = store_files(&store);
        let mut output = Vec::new();
        crate::command_with_output(operator_args, &mut output).unwrap();
        let replies: Vec<_> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(parse)
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(
            field(situation_summary(&replies[0]), "executions").as_str(),
            Some("1")
        );
        let explained = field(known_operator(&replies[1], "explanation"), "value");
        assert_eq!(
            field(explained, "initial_state").as_str(),
            Some("reviewing")
        );
        assert_eq!(field(explained, "ending_state").as_str(), Some("confirmed"));
        assert_eq!(store_files(&store), before);
        assert!(!source.exists());
        assert!(!review_socket.exists());
    });
    serve_operator(config, &actor, &reviewer, &settings, true, 200000).unwrap();
    peer.join().unwrap();
    published_once(&root);
    assert!(!actor.socket.exists());
    assert!(!settings.socket.exists());
}

#[test]
fn independent_stop_drains_partial_operator_and_actor_frames_before_any_source_admission() {
    let root = Directory::new();
    let config = configured(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root, &config);
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let stop_profile = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_profile);
        operator.write_all(b"{\"schema_version\":").unwrap();
        let mut actor = connect(&profile);
        actor.set_nonblocking(false).unwrap();
        actor.write_all(b"{\"schema_version\":").unwrap();
        wait(&crate::workflow::control::socket_path(&stop_profile, 7));
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_profile, 7, &mut output).unwrap();
        assert_eq!(
            field(&parse(&output), "status").as_str(),
            Some("stopped_drained")
        );
    });
    serve_operator(config, &actor, &reviewer, &settings, false, 1000).unwrap();
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
    assert!(!settings.socket.exists());
    assert!(!reviewer.socket(7).exists());
}

#[test]
fn malformed_queries_refuse_and_truncated_transport_retires_only_the_operator_sidecar() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root, &config);
    let situation = situation_document(&config);
    let document = proposal_document(&config, 7);
    let store = config.store.clone();
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_profile);
        let before = store_files(&store);
        let malformed = raw_exchange(&mut operator, "{\"operator\":true}");
        reason(&malformed, "IntegrityFailure");
        assert_eq!(
            field(
                situation_summary(&raw_exchange(&mut operator, &situation)),
                "attempts"
            )
            .as_str(),
            Some("0")
        );
        assert_eq!(store_files(&store), before);
        operator.write_all(b"{\"schema_version\":").unwrap();
        operator.shutdown(std::net::Shutdown::Write).unwrap();
        let started = Instant::now();
        while operator_profile.socket.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "operator sidecar did not retire"
            );
            pause(1);
        }
        assert!(UnixStream::connect(&operator_profile.socket).is_err());
        assert_eq!(store_files(&store), before);
        raw_exchange(&mut connect(&profile), &document)
    });
    let human = human(reviewer.clone());
    serve_operator(config, &actor, &reviewer, &settings, false, 1000).unwrap();
    let admitted = peer.join().unwrap();
    assert_eq!(
        field(knowledge(&admitted), "state").as_str(),
        Some("pending")
    );
    human.join().unwrap();
    published_once(&root);
}

#[test]
fn independent_read_allowances_and_explicit_explanation_selection_survive_reconnects() {
    for explanation_queries in [0, 1] {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let settings =
            Settings::decode(settings_document(&root, &config, 2, explanation_queries).as_bytes())
                .unwrap();
        let situation = situation_document(&config);
        let explanation = explain_document(&config);
        let document = proposal_document(&config, 7);
        let store = config.store.clone();
        let profile = actor.clone();
        let operator_profile = settings.clone();
        let peer = std::thread::spawn(move || {
            let mut first = connect_operator(&operator_profile);
            let before = store_files(&store);
            known_operator(&raw_exchange(&mut first, &situation), "situation");
            let missing = raw_exchange(&mut first, &explanation);
            if explanation_queries == 0 {
                reason(&missing, "CapabilityUnsupported");
            } else {
                let observed = field(native_operator(&missing, "explanation"), "knowledge");
                assert_eq!(field(observed, "state").as_str(), Some("unknown"));
            }
            drop(first);
            let mut second = connect_operator(&operator_profile);
            // The explanation query spent neither inspection unit. Repeating
            // the same request labels still spends the original read grant.
            known_operator(&raw_exchange(&mut second, &situation), "situation");
            let explained = raw_exchange(&mut second, &explanation);
            reason(
                &explained,
                if explanation_queries == 0 {
                    "CapabilityUnsupported"
                } else {
                    "BudgetExhausted"
                },
            );
            drop(second);
            let mut third = connect_operator(&operator_profile);
            let exhausted = raw_exchange(&mut third, &situation);
            reason(&exhausted, "BudgetExhausted");
            assert_eq!(
                field(
                    field(native_operator(&exhausted, "situation"), "error"),
                    "code"
                )
                .as_str(),
                Some("capacity")
            );
            assert_eq!(store_files(&store), before);
            raw_exchange(&mut connect(&profile), &document)
        });
        let human = human(reviewer.clone());
        serve_operator(config, &actor, &reviewer, &settings, false, 1000).unwrap();
        let admitted = peer.join().unwrap();
        assert_eq!(
            field(knowledge(&admitted), "state").as_str(),
            Some("pending")
        );
        human.join().unwrap();
        published_once(&root);
    }
}

#[test]
fn operator_profile_and_socket_preflight_precede_time_source_and_store_creation() {
    for dimension in 0..7 {
        let root = Directory::new();
        let config = configured(&root);
        let store = config.store.clone();
        let (actor, reviewer) = profiles(&root, &config);
        let mut settings = settings(&root, &config);
        let mut generation = Some(GENERATION);
        match dimension {
            0 => settings.socket = actor.socket.clone(),
            1 => settings.socket = reviewer.socket(7),
            2 => settings.socket = crate::workflow::control::socket_path(&reviewer, 7),
            3 => fs::write(&settings.socket, b"preexisting path must survive").unwrap(),
            4 => settings.scope.run += 1,
            5 => generation = None,
            _ => settings.explanation_disclosure = "none".to_owned(),
        }
        let result = serve_selected_ingress(
            config,
            &actor,
            &reviewer,
            None,
            false,
            None,
            None,
            None,
            generation,
            Some(&settings),
            || panic!("operator preflight entered the clock or source boundary"),
        );
        assert!(result.is_err());
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert!(!root.0.join("evidence.json").exists());
        if dimension == 3 {
            assert_eq!(
                fs::read(&settings.socket).unwrap(),
                b"preexisting path must survive"
            );
        }
    }
}

#[test]
fn rejected_operator_credentials_close_unread_without_stopping_actor_publication() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let mut settings = settings(&root, &config);
    let id = credentials();
    settings.operator =
        PeerPolicy::new(id.uid(), id.gid(), Some(id.pid().checked_add(1).unwrap())).unwrap();
    let document = proposal_document(&config, 7);
    let situation = situation_document(&config);
    let store = config.store.clone();
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let peer = std::thread::spawn(move || {
        let mut rejected = connect_operator(&operator_profile);
        rejected
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        rejected
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let before = store_files(&store);
        // A complete disclosure request is queued, but the server authenticates
        // the socket before reading it. No canonical response may be returned.
        let sent = rejected.write_all(format!("{situation}\n").as_bytes());
        if let Err(error) = sent {
            assert!(matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
            ));
        }
        let mut byte = [0_u8; 1];
        match rejected.read(&mut byte) {
            Ok(count) => assert_eq!(count, 0, "unauthorized operator received protocol data"),
            Err(error) => assert!(matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
            )),
        }
        assert_eq!(store_files(&store), before);
        assert!(
            operator_profile.socket.exists(),
            "one rejected candidate retired the service"
        );
        raw_exchange(&mut connect(&profile), &document)
    });
    let human = human(reviewer.clone());
    serve_operator(config, &actor, &reviewer, &settings, false, 1000).unwrap();
    let admitted = peer.join().unwrap();
    assert_eq!(
        field(knowledge(&admitted), "state").as_str(),
        Some("pending")
    );
    human.join().unwrap();
    published_once(&root);
}
