//! The final ordinary review stays independently assessable on its ORIGINAL
//! owner. Actual helper processes, actor traffic and evaluator commands exercise
//! the service; fixture verdicts do not establish empirical detector quality.
use super::*;
use crate::workflow::actor_service::evaluator;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::round::Verdict;
use std::{
    io,
    process::{Child, Command as Process, Stdio},
};

const PEER_ROOT: &str = "FA_ORDINARY_FINAL_EVALUATOR_ROOT";
const PEER_MODE: &str = "FA_ORDINARY_FINAL_EVALUATOR_MODE";
const PEER_ROUND: &str = "FA_ORDINARY_FINAL_EVALUATOR_ROUND";
const PEER_TEST: &str =
    "workflow::actor_service::tests::tower_service::andon::final_evaluation::evaluator_process";
const HOLD_ENV: &str = "FA_ORDINARY_FINAL_HOLD_HELPER";
const HOLD_TEST: &str =
    "workflow::actor_service::tests::tower_service::andon::final_evaluation::hold_helper_process";

fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| field(value, key))
}

fn number(value: &Json, path: &[&str]) -> u64 {
    at(value, path).as_str().unwrap().parse().unwrap()
}

fn wait_for(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "missing {path:?}"
        );
        pause(1);
    }
}

struct EvaluatorProcess(Option<Child>);

impl EvaluatorProcess {
    fn start(root: &Directory, mode: &str, round: u64) -> Self {
        Self(Some(
            Process::new(std::env::current_exe().unwrap())
                .args(["--exact", PEER_TEST, "--nocapture", "--test-threads=1"])
                .env(PEER_ROOT, &root.0)
                .env(PEER_MODE, mode)
                .env(PEER_ROUND, round.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }

    fn pid(&self) -> u32 {
        self.0.as_ref().unwrap().id()
    }

    fn finish(mut self) {
        let started = Instant::now();
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(started.elapsed() < Duration::from_secs(30));
            pause(1);
        }
        let output = self.0.take().unwrap().wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "the separately selected evaluator test must actually execute"
        );
    }
}

impl Drop for EvaluatorProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

fn evaluator_profile(
    root: &Directory,
    actor: &Profile,
    pid: u32,
    candidates: u32,
) -> evaluator::Settings {
    let scope = actor.scope;
    let supervisor = actor.supervisor;
    let document = format!(
        r#"{{"schema_version":"fa.native-evaluator-peer/1","socket":"{}/evaluator.sock",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "deployment":101,"evaluator":202,
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "peer":{{"uid":{},"gid":{},"pid":{pid}}},
        "candidate_limit":{candidates},"runtime_ms":15000,"poll_ms":1}}"#,
        root.0.display(),
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        supervisor.uid(),
        supervisor.gid(),
        supervisor.pid().unwrap(),
        supervisor.uid(),
        supervisor.gid(),
    );
    fs::write(root.0.join("evaluator.json"), &document).unwrap();
    evaluator::Settings::decode(document.as_bytes()).unwrap()
}

fn accounting() -> Settings {
    let mut settings = settings();
    for window in &mut settings.windows {
        window.false_stops_allowed = 1;
    }
    settings.config =
        FileAndonConfig::new(settings.config.identity(), settings.windows.clone()).unwrap();
    settings
}

fn held(config: &mut Config, root: &Directory) {
    config.programs.insert(
        "alpha".into(),
        HelperProgram::new(
            std::env::current_exe().unwrap(),
            root.0.clone(),
            vec!["--exact".into(), HOLD_TEST.into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from(HOLD_ENV), OsString::from("1"))]),
        )
        .unwrap(),
    );
}

#[test]
fn hold_helper_process() {
    if std::env::var_os(HOLD_ENV).is_none() {
        return;
    }
    let profile = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    )))
    .unwrap()
    .profile;
    let mut client =
        HelperClient::from_process_stdin(profile.committee.members()["alpha"].profile_at(0))
            .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(20));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), "alpha");
            assert_eq!(input.round(), 7, "no automatic successor review");
            assert!(
                input
                    .actual_input()
                    .submitted_bytes()
                    .windows(b"review this fixture: alpha".len())
                    .any(|bytes| bytes == b"review this fixture: alpha")
            );
            client
                .respond(Verdict::Hold, b"ordinary-final-hold")
                .unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        pause(1);
    }
}

/// A distinct kernel process, holding no host, ticket or evaluator object. It
/// waits for the actual final actor result before opening its first candidate.
#[test]
fn evaluator_process() {
    let Some(root) = std::env::var_os(PEER_ROOT).map(PathBuf::from) else {
        return;
    };
    wait_for(&root.join("actor.finished"));
    let mode = std::env::var(PEER_MODE).unwrap();
    let round: u64 = std::env::var(PEER_ROUND).unwrap().parse().unwrap();
    let profile = root.join("evaluator.json");
    let settings = evaluator::Settings::read(&profile).unwrap();
    assert_eq!(settings.evaluator.pid, Some(std::process::id()));
    assert_ne!(settings.supervisor.pid, Some(std::process::id()));
    assert!(
        !root.join("evidence.json").exists(),
        "final evaluation is source-free"
    );
    if mode == "partial" {
        let mut socket = UnixStream::connect(settings.socket_path()).unwrap();
        settings
            .supervisor
            .policy()
            .unwrap()
            .verify(&socket)
            .unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        fs::write(root.join("partial.ready"), b"original incomplete peer").unwrap();
        let mut byte = [0_u8; 1];
        match socket.read(&mut byte) {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            other => panic!("terminal boundary left the evaluator live: {other:?}"),
        }
        fs::write(root.join("partial.closed"), b"no case or label").unwrap();
        return;
    }
    let mut observation = Vec::new();
    evaluator::client::command(
        &[
            "evaluator-case".into(),
            profile.display().to_string(),
            round.to_string(),
        ],
        &mut observation,
    )
    .unwrap();
    let case = parse(&observation);
    assert_eq!(at(&case, &["knowledge", "state"]).as_str(), Some("known"));
    assert_eq!(at(&case, &["effect_authority"]).as_bool(), Some(false));
    assert_eq!(number(&case, &["knowledge", "value", "round"]), round);
    fs::write(root.join("case.json"), &observation).unwrap();
    let value = at(&case, &["knowledge", "value"]);
    let assessment = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{round},"attempt":{},
        "policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{}",
        "expected_evaluation_revision":{},"origin":{round},"evidence_id_hex":"{}","truth":"benign"}}"#,
        number(value, &["attempt"]),
        number(value, &["policy_generation"]),
        number(value, &["congress_generation"]),
        at(value, &["case_evidence_root_hex"]).as_str().unwrap(),
        number(&case, &["knowledge", "basis", "evaluation_revision"]),
        "13".repeat(32),
    );
    let assessment_path = root.join("assessment.json");
    fs::write(&assessment_path, assessment).unwrap();
    let mut receipt = Vec::new();
    let result = evaluator::client::command(
        &[
            "evaluator-assess".into(),
            profile.display().to_string(),
            assessment_path.display().to_string(),
        ],
        &mut receipt,
    );
    if mode == "exhausted" {
        assert!(
            result.is_err(),
            "one original candidate cannot cover two exchanges"
        );
        assert!(
            receipt.is_empty(),
            "no second native assessment was acknowledged"
        );
    } else {
        result.unwrap();
        let response = parse(&receipt);
        assert_eq!(at(&response, &["status"]).as_str(), Some("applied"));
        for coordinate in ["control_sequence", "authority_epoch"] {
            assert_eq!(
                at(&response, &["before", coordinate]),
                at(&response, &["after", coordinate])
            );
        }
        assert_eq!(
            number(&response, &["after", "evaluation_revision"]),
            number(&response, &["before", "evaluation_revision"]) + 1
        );
        assert_eq!(
            number(&response, &["after", "journal_revision"]),
            number(&response, &["before", "journal_revision"]) + 2
        );
        fs::write(root.join("receipt.json"), &receipt).unwrap();
    }
    fs::write(
        root.join("evaluation.finished"),
        b"original candidates spent",
    )
    .unwrap();
    wait_for(&root.join("service.finished"));
    assert!(UnixStream::connect(settings.socket_path()).is_err());
}

fn reviewer_for(profile: PeerProfile, decision: ReviewDecision) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut client = profile
            .connect_client_waiting(7, Duration::from_secs(20))
            .unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(20));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(decision).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}

fn actor_after_final(
    root: &Directory,
    config: &Config,
    actor: &Profile,
    broken_retry: bool,
) -> std::thread::JoinHandle<Json> {
    let root = root.0.clone();
    let actor = actor.clone();
    let expiry = crate::workflow::clock().0 + 15_000;
    let document = super::super::series::document(config, 7, 1, "6669727374").replace(
        "\"deadline\":\"100000\"",
        &format!("\"deadline\":\"{expiry}\""),
    );
    let future = super::super::series::document(config, 8, 2, "7365636f6e64").replace(
        "\"deadline\":\"100000\"",
        &format!("\"deadline\":\"{expiry}\""),
    );
    let poll = super::super::series::poll(config, 7);
    std::thread::spawn(move || {
        let mut socket = connect(&actor);
        let result = super::super::series::finish(&mut socket, &document, &poll);
        fs::remove_file(root.join("evidence.json")).unwrap();
        let refused = raw_exchange(&mut socket, &future);
        assert_eq!(field(native(&refused), "reason").as_str(), Some("withheld"));
        assert!(!root.join("evidence.json").exists());
        if broken_retry {
            drop(socket);
            // The original one-candidate session cannot attach this second
            // transport. Wait for its actual refusal before waking evaluation.
            let mut retry = UnixStream::connect(&actor.socket).unwrap();
            retry
                .set_read_timeout(Some(Duration::from_secs(20)))
                .unwrap();
            let mut byte = [0_u8; 1];
            match retry.read(&mut byte) {
                Ok(0) => {}
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
                other => panic!("spent actor candidate was not retired: {other:?}"),
            }
        }
        fs::write(root.join("actor.finished"), b"final native result observed").unwrap();
        result
    })
}

#[derive(Clone, Copy)]
enum Boundary {
    None,
    Deadline,
    Stop,
    Fence,
}

fn fence_request(
    config: &Config,
    operator: &crate::workflow::actor_service::operator::Settings,
) -> std::thread::JoinHandle<()> {
    // Final work is quiescent: this independently read predecessor does not
    // consult the removed evidence source or invoke the native writable owner.
    let before = floor(config);
    let scope = config.profile.delivery.scope;
    let request = format!(
        r#"{{"schema_version":"fa.tower_request/1","operation_id":"final-evaluation-fence",
        "address":"fa://{}/run/{}@{GENERATION}","verb":"fence","arguments":{{
        "operation":"901","expected":{{"scope":{{"tenant":"{}","principal":"{}",
        "run":"{}","branch":"{}","authority":"{}","purpose":"effect"}},
        "journal_revision":"{}","control_sequence":"{}","authority_epoch":"{}"}}}},
        "idempotency_key":"901"}}"#,
        scope.tenant,
        scope.run,
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        before.journal_revision,
        before.control_sequence,
        before.authority_epoch,
    )
    .replace('\n', "");
    let path = operator.socket.clone();
    let supervisor = operator.supervisor;
    std::thread::spawn(move || {
        wait_for(&path);
        let mut socket = UnixStream::connect(&path).unwrap();
        supervisor.verify(&socket).unwrap();
        let reply = raw_exchange(&mut socket, &request);
        let known = super::super::operator::known_operator(&reply, "fence");
        assert_eq!(
            at(known, &["value", "progress", "reserved_units"]).as_str(),
            Some("0")
        );
        assert_eq!(
            at(&reply, &["result", "value", "drain_attempted"]).as_bool(),
            Some(false)
        );
        assert_eq!(
            at(&reply, &["result", "value", "effect_authority"]).as_bool(),
            Some(false)
        );
    })
}

fn run_final(
    hold: bool,
    decision: ReviewDecision,
    candidates: u32,
    boundary: Boundary,
    broken_actor: bool,
) {
    let root = Directory::new();
    let mut config = configured(&root);
    evidence(&root, &config);
    if hold {
        held(&mut config, &root);
    }
    let (mut actor, reviewer) = profiles(&root, &config);
    if broken_actor {
        actor.candidates = 1;
        actor.connections = 1;
    }
    let mode = match boundary {
        Boundary::None if candidates == 1 => "exhausted",
        Boundary::None => "label",
        Boundary::Deadline | Boundary::Stop | Boundary::Fence => "partial",
    };
    let operator = matches!(boundary, Boundary::Fence).then(|| {
        use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_FENCE_REPLAY_EVENTS;
        crate::workflow::actor_service::operator::Settings::decode(
            super::super::operator::settings_document(&root, &config, 0, 0)
                .replacen('{', &format!(
                    "{{\"fence\":{{\"max_replay_events\":{MAX_FENCE_REPLAY_EVENTS},\"authority\":\"stop_original_admission\"}},"
                ), 1)
                .as_bytes(),
        ).unwrap()
    });
    let fence_config = configured(&root);
    let peer = EvaluatorProcess::start(&root, mode, 7);
    let evaluator = evaluator_profile(&root, &actor, peer.pid(), candidates);
    let settings = accounting();
    let output = root.0.join("final.anchor");
    let client = actor_after_final(&root, &config, &actor, broken_actor);
    let human = (!hold).then(|| reviewer_for(reviewer.clone(), decision));
    let partial = root.0.join("partial.ready");
    let mut stop_peer = None;
    let expiry = crate::workflow::clock().0 + 100_000;
    let result = service::serve(
        config,
        &actor,
        &reviewer,
        None,
        service::Selection {
            settings: &settings,
            mode: service::Mode::Create(&output),
            requests: &[7],
            generation: GENERATION,
            operator: operator.as_ref(),
            evaluator: Some(&evaluator),
            prechecks: None,
            inputs: &[],
        },
        || {
            if partial.exists() {
                match boundary {
                    Boundary::Deadline => return ElapsedTick(expiry),
                    Boundary::Stop if stop_peer.is_none() => {
                        let reviewer = reviewer.clone();
                        stop_peer = Some(std::thread::spawn(move || {
                            super::super::series::stop_request(&reviewer, 7);
                        }));
                    }
                    Boundary::Fence if stop_peer.is_none() => {
                        stop_peer = Some(fence_request(&fence_config, operator.as_ref().unwrap()));
                    }
                    _ => {}
                }
            }
            crate::workflow::clock()
        },
    );
    if broken_actor {
        assert!(
            result
                .unwrap_err()
                .contains("actor candidate quota exhausted")
        );
    } else {
        result.unwrap();
    }
    fs::write(root.0.join("service.finished"), b"original owner retired").unwrap();
    let actor_result = client.join().unwrap();
    if let Some(human) = human {
        human.join().unwrap();
    }
    if let Some(stop) = stop_peer {
        stop.join().unwrap();
    }
    peer.finish();
    let executed = !hold && decision == ReviewDecision::Approve;
    assert_eq!(super::super::series::executed(&actor_result), executed);
    let config = configured(&root);
    let saved = inspect(&config, &settings, &output);
    assert_eq!(saved.credibility.report.retained_cases, 1);
    assert_eq!(saved.credibility.journal.executions, u64::from(executed));
    assert_eq!(
        saved.credibility.journal.control.ledger.charged,
        if executed { 7 } else { 0 }
    );
    assert_eq!(saved.credibility.journal.control.ledger.reserved, 0);
    let labeled = matches!(boundary, Boundary::None) && candidates == 2;
    assert_eq!(
        saved.credibility.report.pending_cases,
        usize::from(!labeled)
    );
    assert_eq!(saved.credibility.report.benign_origins, u64::from(labeled));
    assert_eq!(
        saved.credibility.report.revision,
        if labeled { 2 } else { 1 }
    );
    let false_stop = labeled && hold;
    let window = &saved.accounting.windows()[&1];
    assert_eq!(window.false_stops, u64::from(false_stop));
    assert_eq!(window.total_cost, if false_stop { 7 } else { 0 });
    assert_eq!(
        saved.credibility.report.lifetime_false_stops,
        u64::from(false_stop)
    );
    if false_stop {
        let incident = window.incident.as_ref().unwrap();
        assert_eq!(incident.opened_by_round, 7);
        assert_eq!(incident.rounds.iter().copied().collect::<Vec<_>>(), [7]);
    } else {
        assert!(window.incident.is_none());
    }
    if labeled {
        let receipt = parse(&fs::read(root.0.join("receipt.json")).unwrap());
        assert!(
            saved.credibility.journal.revision >= number(&receipt, &["after", "journal_revision"])
        );
    } else {
        assert!(!root.0.join("receipt.json").exists());
    }
    assert_eq!(
        saved.credibility.journal.stop.is_some(),
        matches!(boundary, Boundary::Stop | Boundary::Fence)
    );
    if matches!(
        boundary,
        Boundary::Deadline | Boundary::Stop | Boundary::Fence
    ) {
        assert!(root.0.join("partial.closed").exists());
        assert!(!root.0.join("case.json").exists());
    }
    assert!(!config.source.path().exists());
    for socket in [
        actor.socket,
        reviewer.socket(7),
        crate::workflow::control::socket_path(&reviewer, 7),
        evaluator.socket_path().to_path_buf(),
    ] {
        assert!(!socket.exists(), "retired endpoint {socket:?}");
    }
    if let Some(operator) = operator {
        assert!(!operator.socket.exists());
    }
}

#[test]
fn final_allow_human_rejection_and_helper_hold_are_independently_assessable_without_new_work() {
    for (hold, decision) in [
        (false, ReviewDecision::Approve),
        (false, ReviewDecision::Reject),
        (true, ReviewDecision::Approve),
    ] {
        run_final(hold, decision, 2, Boundary::None, false);
    }
}

#[test]
fn final_case_discovery_cannot_replenish_an_exhausted_original_evaluator_candidate_budget() {
    run_final(true, ReviewDecision::Approve, 1, Boundary::None, false);
}

#[test]
fn final_incomplete_evaluation_yields_to_original_stop_without_labeling_or_publication() {
    run_final(true, ReviewDecision::Approve, 2, Boundary::Stop, false);
}

#[test]
fn final_incomplete_evaluation_yields_to_operator_fence_before_label_or_time_observation() {
    run_final(true, ReviewDecision::Approve, 2, Boundary::Fence, false);
}

#[test]
fn final_incomplete_evaluation_cannot_renew_the_original_outer_logical_deadline() {
    run_final(true, ReviewDecision::Approve, 2, Boundary::Deadline, false);
}

#[test]
fn final_actor_transport_failure_preserves_the_original_independent_labeling_opportunity() {
    run_final(true, ReviewDecision::Approve, 2, Boundary::None, true);
}

#[test]
fn recovered_final_case_keeps_original_accounting_and_needs_no_source_or_helper_restart() {
    let root = Directory::new();
    let original = run_pair(&root);
    let mut config = configured(&root);
    config.programs.clear();
    fs::remove_file(config.source.path()).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings();
    let before = inspect(&config, &settings, &original);
    let output = root.0.join("recovered-evaluated.anchor");
    let recovered = recovery(&config, &original, Some(&output));
    let peer = EvaluatorProcess::start(&root, "label", 8);
    let evaluator = evaluator_profile(&root, &actor, peer.pid(), 2);
    let first = super::super::series::document(&config, 7, 1, "6669727374");
    let second = super::super::series::document(&config, 8, 2, "7365636f6e64");
    let actor_peer = actor.clone();
    let finished = root.0.join("actor.finished");
    let client = std::thread::spawn(move || {
        let mut socket = connect(&actor_peer);
        assert!(super::super::series::executed(&raw_exchange(
            &mut socket,
            &first
        )));
        assert!(super::super::series::executed(&raw_exchange(
            &mut socket,
            &second
        )));
        fs::write(finished, b"both original receipts observed").unwrap();
    });
    service::serve(
        config,
        &actor,
        &reviewer,
        None,
        service::Selection {
            settings: &settings,
            mode: service::Mode::Existing(&recovered),
            requests: &[7, 8],
            generation: GENERATION,
            operator: None,
            evaluator: Some(&evaluator),
            prechecks: None,
            inputs: &[],
        },
        crate::workflow::clock,
    )
    .unwrap();
    fs::write(root.0.join("service.finished"), b"retained successor cut").unwrap();
    client.join().unwrap();
    peer.finish();
    let config = configured(&root);
    let after = inspect(&config, &settings, &output);
    assert_eq!(after.credibility.report.retained_cases, 2);
    assert_eq!(after.credibility.report.pending_cases, 1);
    assert_eq!(after.credibility.report.benign_origins, 1);
    assert_eq!(
        after.credibility.journal.executions,
        before.credibility.journal.executions
    );
    assert_eq!(
        after.credibility.journal.control.ledger.charged,
        before.credibility.journal.control.ledger.charged
    );
    assert!(
        after.credibility.journal.control.ledger.epoch
            > before.credibility.journal.control.ledger.epoch
    );
    assert_eq!(after.accounting.windows()[&1].total_cost, 0);
    assert!(!config.source.path().exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
    assert!(!evaluator.socket_path().exists());
}
