//! Delayed labels exercise the original ordinary owner after its publication
//! service has exited. Independent kernel peers do not possess a native role.
//! Fixture helper verdicts establish custody, never empirical detector quality.
use super::*;
use crate::workflow::actor_service::evaluator;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::StopClientProgress;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::round::Verdict;
use std::{
    io,
    process::{Child, Command as Process, Stdio},
};

const CHILD_ROOT: &str = "FA_ORDINARY_HISTORY_EVALUATOR_ROOT";
const CHILD_MODE: &str = "FA_ORDINARY_HISTORY_EVALUATOR_MODE";
const CHILD_ROUND: &str = "FA_ORDINARY_HISTORY_EVALUATOR_ROUND";
const CHILD_TEST: &str =
    "workflow::actor_service::tests::tower_service::andon::history::historical_evaluator_process";
const HOLD_ENV: &str = "FA_ORDINARY_HISTORY_HOLD_HELPER";
const HOLD_TEST: &str =
    "workflow::actor_service::tests::tower_service::andon::history::historical_hold_helper_process";

fn at<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| field(value, key))
}

fn number(value: &Json, path: &[&str]) -> u64 {
    at(value, path).as_str().unwrap().parse().unwrap()
}

struct EvaluatorProcess(Option<Child>);

impl EvaluatorProcess {
    fn start(root: &Directory, mode: &str, round: u64) -> Self {
        Self(Some(
            Process::new(std::env::current_exe().unwrap())
                .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
                .env(CHILD_ROOT, &root.0)
                .env(CHILD_MODE, mode)
                .env(CHILD_ROUND, round.to_string())
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
            "the independently selected evaluator test must actually execute"
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
        r#"{{"schema_version":"fa.native-evaluator-peer/1","socket":"{}/history-evaluator.sock",
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
    fs::write(root.0.join("history-evaluator.json"), &document).unwrap();
    evaluator::Settings::decode(document.as_bytes()).unwrap()
}

#[test]
fn historical_evaluator_process() {
    let Some(root) = std::env::var_os(CHILD_ROOT).map(PathBuf::from) else {
        return;
    };
    let mode = std::env::var(CHILD_MODE).unwrap();
    let round = std::env::var(CHILD_ROUND).unwrap().parse::<u64>().unwrap();
    let profile = root.join("history-evaluator.json");
    wait(&root.join("history.ready"));
    let settings = evaluator::Settings::read(&profile).unwrap();
    assert_eq!(settings.evaluator.pid, Some(std::process::id()));
    assert_ne!(settings.supervisor.pid, Some(std::process::id()));
    assert!(!root.join("actors/actor.sock").exists());
    assert!(!root.join("reviewers/review-7.sock").exists());
    assert!(!root.join("evidence.json").exists());
    if mode == "partial" {
        let mut socket = UnixStream::connect(settings.socket_path()).unwrap();
        settings.supervisor.policy().unwrap().verify(&socket).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        fs::write(root.join("partial.ready"), b"incomplete original candidate").unwrap();
        let mut byte = [0_u8; 1];
        match socket.read(&mut byte) {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            other => panic!("retired historical evaluator kept the candidate alive: {other:?}"),
        }
        fs::write(root.join("partial.closed"), b"no original case or assessment").unwrap();
    } else {
        let mut observation = Vec::new();
        evaluator::client::command(
            &["evaluator-case".into(), profile.display().to_string(), round.to_string()],
            &mut observation,
        )
        .unwrap();
        let case = parse(&observation);
        assert_eq!(at(&case, &["knowledge", "state"]).as_str(), Some("known"));
        assert_eq!(at(&case, &["effect_authority"]).as_bool(), Some(false));
        assert_eq!(number(&case, &["knowledge", "value", "round"]), round);
        fs::write(root.join("history.case.json"), &observation).unwrap();
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
            "29".repeat(32),
        );
        let assessment_path = root.join("history.assessment.json");
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
            assert!(result.is_err(), "discovery spent the only original candidate");
            assert!(receipt.is_empty());
        } else {
            result.unwrap();
            let applied = parse(&receipt);
            assert_eq!(at(&applied, &["status"]).as_str(), Some("applied"));
            for coordinate in ["control_sequence", "authority_epoch"] {
                assert_eq!(at(&applied, &["before", coordinate]), at(&applied, &["after", coordinate]));
            }
            assert_eq!(
                number(&applied, &["after", "evaluation_revision"]),
                number(&applied, &["before", "evaluation_revision"]) + 1
            );
            assert_eq!(
                number(&applied, &["after", "journal_revision"]),
                number(&applied, &["before", "journal_revision"]) + 2
            );
            fs::write(root.join("history.receipt.json"), &receipt).unwrap();
        }
    }
    fs::write(root.join("evaluation.finished"), b"original candidate work ended").unwrap();
    wait(&root.join("history.finished"));
    assert!(UnixStream::connect(settings.socket_path()).is_err());
}

#[test]
fn historical_hold_helper_process() {
    if std::env::var_os(HOLD_ENV).is_none() {
        return;
    }
    let config = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"
    )))
    .unwrap();
    let mut client = HelperClient::from_process_stdin(
        config.profile.committee.members()["alpha"].profile_at(0),
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(20));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), "alpha");
            assert_eq!(input.round(), 7);
            client.respond(Verdict::Hold, b"ordinary-history-hold").unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        pause(1);
    }
}

fn seed_hold(root: &Directory) -> PathBuf {
    let mut config = configured(root);
    evidence(root, &config);
    config.programs.insert(
        "alpha".into(),
        HelperProgram::new(
            std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), HOLD_TEST.into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from(HOLD_ENV), OsString::from("1"))]),
        )
        .unwrap(),
    );
    let (actor, reviewer) = profiles(root, &config);
    let document = super::super::series::document(&config, 7, 1, "6669727374");
    let poll = super::super::series::poll(&config, 7);
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_actor);
        let reply = super::super::series::finish(&mut socket, &document, &poll);
        assert!(!super::super::series::executed(&reply));
        reply
    });
    let settings = settings();
    let output = root.0.join("held.anchor");
    service::serve(
        config, &actor, &reviewer, None,
        service::Selection {
            settings: &settings, mode: service::Mode::Create(&output), requests: &[7],
            generation: GENERATION, operator: None, evaluator: None, prechecks: None, inputs: &[],
            governor: None,
        },
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    let saved = inspect(&configured(root), &settings, &output);
    assert_eq!(saved.credibility.report.retained_cases, 1);
    assert_eq!(saved.credibility.report.pending_cases, 1);
    assert_eq!(saved.credibility.journal.executions, 0);
    assert!(saved.credibility.journal.control.ledger.stages.values().all(|stage| *stage != ActionState::Confirmed));
    output
}

fn run_history_label(root: &Directory, original: &Path, round: u64, requests: &[u64], candidates: u32) {
    let mut config = configured(root);
    config.programs.clear();
    if config.source.path().exists() {
        fs::remove_file(config.source.path()).unwrap();
    }
    let (actor, reviewer) = profiles(root, &config);
    let settings = settings();
    let before = inspect(&config, &settings, original);
    let original_bytes = fs::read(original).unwrap();
    let output = root.0.join("evaluated.anchor");
    let recovered = recovery(&config, original, Some(&output));
    let child = EvaluatorProcess::start(root, if candidates == 1 { "exhausted" } else { "label" }, round);
    let evaluator = evaluator_profile(root, &actor, child.pid(), candidates);
    let ready = root.0.join("history.ready");
    let result = service::serve_history(
        config, &actor, &reviewer, None,
        service::Selection {
            settings: &settings, mode: service::Mode::Existing(&recovered), requests,
            generation: GENERATION, operator: None, evaluator: Some(&evaluator),
            governor: None,
            prechecks: None, inputs: &[],
        },
        || {
            assert!(!actor.socket.exists(), "historical evaluation cannot bind actor intake");
            for &request in requests {
                assert!(!reviewer.socket(request).exists(), "historical evaluation cannot bind review");
            }
            if evaluator.socket_path().exists() && !ready.exists() {
                fs::write(&ready, b"original independent evaluator bound").unwrap();
            }
            crate::workflow::clock()
        },
    );
    assert!(result.is_ok(), "{result:?}");
    fs::write(root.0.join("history.finished"), b"all evaluator custody retired").unwrap();
    child.finish();
    let config = configured(root);
    let after = inspect(&config, &settings, &output);
    assert_eq!(after.credibility.report.retained_cases, before.credibility.report.retained_cases);
    let labeled = usize::from(candidates == 2);
    assert_eq!(after.credibility.report.pending_cases, before.credibility.report.pending_cases - labeled);
    assert_eq!(after.credibility.report.benign_origins, labeled as u64);
    assert_eq!(after.credibility.journal.executions, before.credibility.journal.executions);
    assert_eq!(after.credibility.journal.payload, before.credibility.journal.payload);
    assert_eq!(after.credibility.journal.control.ledger.charged, before.credibility.journal.control.ledger.charged);
    assert_eq!(after.credibility.journal.control.ledger.reserved, before.credibility.journal.control.ledger.reserved);
    assert_eq!(after.credibility.journal.control.ledger.epoch, before.credibility.journal.control.ledger.epoch + 1);
    assert!(after.credibility.journal.stop.is_none());
    let expected_false_stops = u64::from(before.credibility.journal.executions == 0 && labeled == 1);
    assert_eq!(after.accounting.windows()[&1].false_stops, expected_false_stops);
    assert_eq!(after.accounting.windows()[&1].total_cost, u128::from(expected_false_stops) * 7);
    assert_eq!(fs::read(original).unwrap(), original_bytes);
    let retained = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(&output).unwrap()), MAX_HISTORY_ANCHOR_BYTES,
    ).unwrap();
    assert_eq!(retained.revision(), after.credibility.journal.revision);
    if labeled == 1 {
        let receipt = parse(&fs::read(root.0.join("history.receipt.json")).unwrap());
        assert!(retained.revision() >= number(&receipt, &["after", "journal_revision"]));
    } else {
        assert!(!root.0.join("history.receipt.json").exists());
        assert_eq!(after.accounting, before.accounting);
    }
    assert!(!config.source.path().exists());
    assert!(!actor.socket.exists());
    assert!(!evaluator.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, actor.request).exists());
}

#[test]
fn delayed_ordinary_evaluation_labels_old_cases_without_actor_source_or_helper_reentry() {
    for round in [7, 8] {
        let root = Directory::new();
        let original = run_pair(&root);
        run_history_label(&root, &original, round, &[7, 8], 2);
    }
}

#[test]
fn delayed_ordinary_hold_assessment_preserves_the_original_false_stop_cost() {
    let root = Directory::new();
    let original = seed_hold(&root);
    run_history_label(&root, &original, 7, &[7], 2);
}

#[test]
fn historical_case_discovery_does_not_refill_the_evaluator_candidate_allowance() {
    let root = Directory::new();
    let original = run_pair(&root);
    run_history_label(&root, &original, 7, &[7, 8], 1);
}

#[test]
fn historical_request_accounting_anchor_and_successor_refusals_preserve_the_original_cut() {
    let root = Directory::new();
    let original = run_pair(&root);
    let config = configured(&root);
    fs::remove_file(config.source.path()).unwrap();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    let original_bytes = fs::read(&original).unwrap();
    let staging = config.store.join("delivery.pending");
    fs::write(&staging, b"unacknowledged preexisting staging").unwrap();
    for which in 0..5 {
        let mut config = configured(&root);
        config.programs.clear();
        let (actor, reviewer) = profiles(&root, &config);
        let mut settings = settings();
        if which == 1 {
            settings.minimum_evaluation_revision = 100_000;
        }
        if which == 2 {
            settings.protocol.domain += 1;
        }
        let input = if which == 3 {
            let path = root.0.join("corrupt.anchor");
            let mut corrupt = original_bytes.clone();
            corrupt[0] ^= 1;
            fs::write(&path, corrupt).unwrap();
            path
        } else {
            original.clone()
        };
        let output = root.0.join(format!("refused-{which}.anchor"));
        let recovered = recovery(&config, &input, (which != 4).then_some(output.as_path()));
        let evaluator = evaluator_profile(&root, &actor, std::process::id() + 100_000, 2);
        let requests: &[u64] = if which == 0 { &[7, 9] } else { &[7, 8] };
        let result = service::serve_history(
            config, &actor, &reviewer, None,
            service::Selection {
                settings: &settings, mode: service::Mode::Existing(&recovered), requests,
                generation: GENERATION, operator: None, evaluator: Some(&evaluator),
                governor: None,
                prechecks: None, inputs: &[],
            },
            || {
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(7).exists());
                assert!(!evaluator.socket_path().exists());
                ElapsedTick(1001)
            },
        );
        assert!(result.is_err(), "historical admission {which} acquired an owner");
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
        assert_eq!(fs::read(&original).unwrap(), original_bytes);
        assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged preexisting staging");
        assert!(!output.exists());
        assert!(!actor.socket.exists());
        assert!(!evaluator.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
    }
}

fn stop_peer(reviewer: &PeerProfile) -> std::thread::JoinHandle<()> {
    let mut client = reviewer.connect_stop_client(7).unwrap();
    std::thread::spawn(move || {
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(20));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => {
                    let receipt = client.receipt().unwrap();
                    assert!(receipt.acknowledged() && receipt.drained());
                    return;
                }
                _ => pause(1),
            }
        }
    })
}

#[test]
fn historical_ready_and_final_prefence_stop_win_without_reconciliation_or_role_issue() {
    for boundary in [0, 1, 2] {
        let root = Directory::new();
        let original = run_pair(&root);
        let mut config = configured(&root);
        config.programs.clear();
        fs::remove_file(config.source.path()).unwrap();
        let (actor, reviewer) = profiles(&root, &config);
        let settings = settings();
        let before = inspect(&config, &settings, &original);
        let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
        let staging = config.store.join("delivery.pending");
        fs::write(&staging, b"original unacknowledged staging").unwrap();
        let output = root.0.join("stopped.anchor");
        let recovered = recovery(&config, &original, Some(&output));
        let evaluator = evaluator_profile(&root, &actor, std::process::id() + 100_000, 2);
        // Two setup observations, then one per original replay boundary. Ready
        // precedes selection; one further check closes the pre-Fence gap.
        let selected_call = before.credibility.journal.revision + if boundary == 1 { 4 } else { 3 };
        let requests: &[u64] = if boundary == 1 { &[7, 8] } else { &[7, 9] };
        let mut calls = 0;
        let mut stop = None;
        let mut reached = false;
        let result = service::serve_history(
            config, &actor, &reviewer, None,
            service::Selection {
                settings: &settings, mode: service::Mode::Existing(&recovered), requests,
                generation: GENERATION, operator: None, evaluator: Some(&evaluator),
                governor: None,
                prechecks: None, inputs: &[],
            },
            || {
                calls += 1;
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(7).exists());
                assert!(!evaluator.socket_path().exists());
                if calls == selected_call {
                    reached = true;
                    if boundary == 2 {
                        return ElapsedTick(u64::MAX);
                    }
                    stop = Some(stop_peer(&reviewer));
                }
                ElapsedTick(1001)
            },
        );
        assert!(reached, "the selected original recovery boundary must execute");
        let error = result.unwrap_err();
        if boundary == 2 {
            assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
            assert_eq!(fs::read(&staging).unwrap(), b"original unacknowledged staging");
            assert!(!output.exists());
        } else {
            assert!(error.contains("native admission Stop acknowledged"), "{error}");
            stop.unwrap().join().unwrap();
            let after = inspect(&configured(&root), &settings, &output);
            assert_eq!(after.credibility.journal.revision, before.credibility.journal.revision + 2);
            assert_eq!(after.credibility.journal.control.ledger.elapsed, before.credibility.journal.control.ledger.elapsed);
            assert_eq!(after.credibility.journal.executions, before.credibility.journal.executions);
            assert_eq!(after.credibility.journal.control.ledger.charged, before.credibility.journal.control.ledger.charged);
            assert_eq!(after.credibility.report, before.credibility.report);
            assert_eq!(after.accounting, before.accounting);
            assert!(after.credibility.journal.stop.is_some());
        }
        assert!(!actor.socket.exists());
        assert!(!evaluator.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
    }
}

#[test]
fn historical_incomplete_evaluator_yields_to_authenticated_stop_and_outer_deadline() {
    for stopping in [true, false] {
        let root = Directory::new();
        let original = run_pair(&root);
        let mut config = configured(&root);
        config.programs.clear();
        fs::remove_file(config.source.path()).unwrap();
        let (actor, reviewer) = profiles(&root, &config);
        let settings = settings();
        let before = inspect(&config, &settings, &original);
        let output = root.0.join("partial.anchor");
        let recovered = recovery(&config, &original, Some(&output));
        let child = EvaluatorProcess::start(&root, "partial", 7);
        let evaluator = evaluator_profile(&root, &actor, child.pid(), 2);
        let mut stop = None;
        let ready = root.0.join("history.ready");
        let result = service::serve_history(
            config, &actor, &reviewer, None,
            service::Selection {
                settings: &settings, mode: service::Mode::Existing(&recovered), requests: &[7, 8],
                generation: GENERATION, operator: None, evaluator: Some(&evaluator),
                governor: None,
                prechecks: None, inputs: &[],
            },
            || {
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(7).exists());
                if evaluator.socket_path().exists() && !ready.exists() {
                    fs::write(&ready, b"only evaluator bound").unwrap();
                }
                if root.0.join("partial.ready").exists() {
                    if !stopping {
                        return ElapsedTick(u64::MAX);
                    }
                    if stop.is_none() {
                        stop = Some(stop_peer(&reviewer));
                    }
                }
                crate::workflow::clock()
            },
        );
        assert!(result.is_ok(), "{result:?}");
        fs::write(root.0.join("history.finished"), b"retired before final anchor").unwrap();
        child.finish();
        if let Some(stop) = stop {
            stop.join().unwrap();
        }
        let after = inspect(&configured(&root), &settings, &output);
        assert_eq!(after.credibility.report, before.credibility.report);
        assert_eq!(after.accounting, before.accounting);
        assert_eq!(after.credibility.journal.executions, before.credibility.journal.executions);
        assert_eq!(after.credibility.journal.control.ledger.charged, before.credibility.journal.control.ledger.charged);
        assert_eq!(after.credibility.journal.stop.is_some(), stopping);
        assert!(root.0.join("partial.closed").exists());
        assert!(!root.0.join("history.case.json").exists());
        assert!(!root.0.join("history.receipt.json").exists());
        assert!(!evaluator.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
    }
}

#[test]
fn historical_output_cannot_alias_an_original_source_or_reserved_endpoint() {
    let root = Directory::new();
    let original = run_pair(&root);
    let config = configured(&root);
    fs::remove_file(config.source.path()).unwrap();
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    for which in 0..3 {
        let mut config = configured(&root);
        config.programs.clear();
        let (actor, reviewer) = profiles(&root, &config);
        let evaluator = evaluator_profile(&root, &actor, std::process::id() + 100_000, 2);
        let output = match which {
            0 => config.source.path().to_path_buf(),
            1 => crate::workflow::control::socket_path(&reviewer, 8),
            _ => evaluator.socket_path().to_path_buf(),
        };
        let recovered = recovery(&config, &original, Some(&output));
        let result = service::serve_history(
            config, &actor, &reviewer, None,
            service::Selection {
                settings: &settings(), mode: service::Mode::Existing(&recovered), requests: &[7, 8],
                generation: GENERATION, operator: None, evaluator: Some(&evaluator),
                governor: None,
                prechecks: None, inputs: &[],
            },
            || panic!("aliased history custody acquired time or replay ownership"),
        );
        assert!(result.is_err(), "custody alias {which} was accepted");
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
        assert!(!output.exists());
        assert!(!actor.socket.exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
    }
}

#[test]
fn historical_evaluator_startup_failure_retains_its_fence_for_a_corrected_reopen() {
    let root = Directory::new();
    let original = run_pair(&root);
    let mut config = configured(&root);
    config.programs.clear();
    fs::remove_file(config.source.path()).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings();
    let before = inspect(&config, &settings, &original);
    let original_bytes = fs::read(&original).unwrap();
    let output = root.0.join("startup-failed.anchor");
    let recovered = recovery(&config, &original, Some(&output));
    let evaluator = evaluator_profile(&root, &actor, std::process::id() + 100_000, 2);
    let mut calls = 0;
    let mut raced = false;
    let result = service::serve_history(
        config, &actor, &reviewer, None,
        service::Selection {
            settings: &settings, mode: service::Mode::Existing(&recovered), requests: &[7, 8],
            generation: GENERATION, operator: None, evaluator: Some(&evaluator),
            governor: None,
            prechecks: None, inputs: &[],
        },
        || {
            calls += 1;
            assert!(!actor.socket.exists());
            assert!(!reviewer.socket(7).exists());
            // Ready, final pre-Fence, then the first healthy successor check.
            // Occupation after preflight cannot discard the acknowledged Fence
            // or permit replacing another file with an evaluator listener.
            if calls == before.credibility.journal.revision + 5 {
                fs::write(evaluator.socket_path(), b"occupied after recovery Fence").unwrap();
                raced = true;
            }
            ElapsedTick(1001)
        },
    );
    assert!(raced);
    let error = result.unwrap_err();
    assert!(error.contains("acknowledged"), "{error}");
    assert_eq!(fs::read(evaluator.socket_path()).unwrap(), b"occupied after recovery Fence");
    assert_eq!(fs::read(&original).unwrap(), original_bytes);
    let after = inspect(&configured(&root), &settings, &output);
    assert_eq!(after.credibility.journal.revision, before.credibility.journal.revision + 1);
    assert_eq!(after.credibility.journal.executions, before.credibility.journal.executions);
    assert_eq!(after.credibility.journal.control.ledger.elapsed, before.credibility.journal.control.ledger.elapsed);
    assert_eq!(after.credibility.journal.control.ledger.charged, before.credibility.journal.control.ledger.charged);
    assert_eq!(after.credibility.report, before.credibility.report);
    assert_eq!(after.accounting, before.accounting);
    assert!(!actor.socket.exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
    fs::remove_file(evaluator.socket_path()).unwrap();
    run_history_label(&root, &output, 7, &[7, 8], 2);
}

#[test]
fn historical_mode_owns_its_complete_grammar_before_profiles_or_time() {
    let base: Vec<String> = [
        "serve-open", "missing-config", "missing-actor", "missing-reviewer",
        "--tower-generation", "11", "--requests", "7,8", "--andon", "missing-andon",
        "--guarded", "missing-recovery", "--evaluator-profile", "missing-evaluator",
        "--evaluate-only",
    ].into_iter().map(str::to_owned).collect();
    for command in ["serve-create", "serve-create-checked"] {
        let mut args = base.clone();
        args[0] = command.into();
        if command.ends_with("checked") {
            args.insert(4, "missing-publication".into());
        }
        let error = crate::workflow::actor_service::tower::command_with_clock(
            &args, None, || panic!("historical creation acquired a clock"),
        ).unwrap_err();
        assert!(!error.contains("No such file"), "{error}");
    }
    for removed in ["--tower-generation", "--requests", "--andon", "--guarded", "--evaluator-profile"] {
        let mut args = base.clone();
        let index = args.iter().position(|value| value == removed).unwrap();
        args.drain(index..index + 2);
        let error = crate::workflow::actor_service::tower::command_with_clock(
            &args, None, || panic!("incomplete historical mode acquired a clock"),
        ).unwrap_err();
        assert!(!error.contains("No such file"), "{error}");
    }
    for extra in [
        vec!["--evaluate-only"], vec!["true"],
        vec!["--operator-profile", "missing-operator"],
        vec!["--precheck-profile", "missing-precheck"],
        vec!["--precheck-schedule", "missing-schedule"],
        vec!["--topology-profile", "missing-topology"],
        vec!["--governor-profile", "missing-governor"],
        vec!["--retain-history-anchor", "missing-output"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        let error = crate::workflow::actor_service::tower::command_with_clock(
            &args, None, || panic!("unsupported historical mode acquired a clock"),
        ).unwrap_err();
        assert!(!error.contains("No such file"), "{error}");
    }
    let error = crate::workflow::actor_service::tower::command_with_clock(
        &base, Some(Path::new("missing-activation")), || panic!("activation read a clock"),
    ).unwrap_err();
    assert!(error.contains("credibility activation"), "{error}");
    // The near-identical supported grammars reach the first actual profile read.
    // This does not claim that unavailable profile bytes were admitted.
    for checked in [false, true] {
        let mut args = base.clone();
        if checked {
            args[0] = "serve-open-checked".into();
            args.insert(4, "missing-publication".into());
        }
        let error = crate::workflow::actor_service::tower::command_with_clock(
            &args, None, || panic!("missing config acquired a clock"),
        ).unwrap_err();
        assert!(error.contains("No such file"), "{error}");
    }
}


fn history_profile(joint: bool, generation: u64) -> Settings {
    if !joint {
        return settings();
    }
    let document = std::str::from_utf8(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/fixtures/native_andon_operator.json"
    )))
    .unwrap()
    .replace("fa.native-andon-operator/1", "fa.native-andon-operator/2");
    let joint = format!(
        r#"{{"joint":{{"id":91,"generation":{generation},"maximum_escape_rate":{{"numerator":1,"denominator":2}},"maximum_benign_stop_rate":{{"numerator":1,"denominator":2}},"budget":{{"cases":8,"member_outcomes":32}}}},"#
    );
    Settings::decode(document.replacen('{', &joint, 1).as_bytes()).unwrap()
}

fn history_publication(root: &Directory, bindings: u64) -> PublicationProfile {
    PublicationProfile::decode(format!(
        r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":{:?},"current":{:?},"limits":{{"bindings":{bindings},"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}}]}}"#,
        root.0.join("original.bin"), root.0.join("current.bin"),
    ).as_bytes()).unwrap()
}

fn historical_capture(config: &Config) -> Vec<u8> {
    use fa_reference::ReadWitness;
    use fa_reference::action::consequence::delivery::persistent::observed::publication::{
        capture::{FileCaptureIdentity, FilePublicationCapture},
        witnesses::{FilePublicationInputs, FileWitnessInput},
    };
    use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
    use fa_reference::product_frontier::{
        FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
    };
    use fa_reference::witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry,
    };

    let mut target = config.profile.delivery.target;
    target.expected_version = 1;
    let action = FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: config.profile.delivery.scope,
        target: Some(target),
        payload: b"first".to_vec(),
        required_witnesses: vec![ReadWitness::Exact { key: 7, value: Some(b"ok".to_vec()) }],
        policy_epoch: 0,
        deadline: ElapsedTick(100000),
        units: 7,
    }).unwrap();
    let key = ProjectionKey {
        source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1,
    };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(
        1, 1, 20,
        AdapterDomainInput::new(
            DomainProjection::new(40, 1, key), DomainClosure::Closed(close),
        ),
        vec![SnapshotEntry::new(0, 1, b"original".to_vec()).unwrap()],
        &frontiers,
    ).unwrap();
    FilePublicationCapture::new(
        1, FileCaptureIdentity { source: 91, generation: 1 }, &action,
        FilePublicationInputs::new(Some(input), None),
    ).unwrap().to_bytes().unwrap()
}

fn inspect_profiled(
    config: &Config,
    settings: &Settings,
    publication: Option<&PublicationProfile>,
    path: &Path,
) -> FileNativeAndonSnapshot {
    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()), MAX_HISTORY_ANCHOR_BYTES,
    ).unwrap();
    let expected = settings.requirements(config, floor(config)).unwrap();
    let expected = match publication {
        Some(publication) => expected.require_publication(publication.simple_limits().unwrap()).unwrap(),
        None => expected,
    };
    FileOversight::read_andon_guarded(
        &config.store, &config.profile, &expected, &settings.protocol, &anchor,
    ).unwrap()
}

fn seed_profiled(
    root: &Directory,
    settings: &Settings,
    publication: Option<&PublicationProfile>,
) -> PathBuf {
    let config = configured(root);
    evidence(root, &config);
    if publication.is_some() {
        let capture = historical_capture(&config);
        fs::write(root.0.join("original.bin"), &capture).unwrap();
        fs::write(root.0.join("current.bin"), &capture).unwrap();
    }
    let (actor, reviewer) = profiles(root, &config);
    let proposal = super::super::series::document(&config, 7, 1, "6669727374");
    let poll = super::super::series::poll(&config, 7);
    let peer_actor = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&peer_actor);
        assert!(super::super::series::executed(
            &super::super::series::finish(&mut socket, &proposal, &poll)
        ));
    });
    let human = super::super::series::human_for(reviewer.clone(), 7);
    let output = root.0.join("profiled.anchor");
    service::serve(
        config, &actor, &reviewer, publication,
        service::Selection {
            settings, mode: service::Mode::Create(&output), requests: &[7],
            generation: GENERATION, operator: None, evaluator: None, prechecks: None, inputs: &[],
            governor: None,
        },
        || ElapsedTick(1000),
    ).unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    let before = inspect_profiled(&configured(root), settings, publication, &output);
    assert_eq!(before.credibility.journal.executions, 1);
    assert_eq!(before.credibility.journal.control.ledger.charged, 7);
    assert_eq!(before.credibility.report.retained_cases, 1);
    assert_eq!(before.credibility.report.pending_cases, 1);
    output
}

#[test]
fn historical_checked_and_joint_profiles_keep_the_original_guards_and_label_custody() {
    // The ordinary marginal case is exercised above. These are the other three
    // complete native profiles admitted by the ordinary historical service.
    for (checked, joint) in [(true, false), (false, true), (true, true)] {
        let root = Directory::new();
        let settings = history_profile(joint, 1);
        let publication = checked.then(|| history_publication(&root, 8));
        let original = seed_profiled(&root, &settings, publication.as_ref());
        let config = configured(&root);
        let before = inspect_profiled(&config, &settings, publication.as_ref(), &original);
        let original_bytes = fs::read(&original).unwrap();
        let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
        fs::remove_file(config.source.path()).unwrap();
        if checked {
            fs::remove_file(root.0.join("original.bin")).unwrap();
            fs::remove_file(root.0.join("current.bin")).unwrap();
        }
        let (actor, reviewer) = profiles(&root, &config);
        let refused = root.0.join("refused-profile.anchor");
        let recovered = recovery(&config, &original, Some(&refused));
        let fake = evaluator_profile(&root, &actor, std::process::id() + 100_000, 2);
        // Change one exact checked, joint or current-window guard at a time.
        // Every original source and capture is absent; refuse before the Fence.
        let selected_guards = usize::from(checked) + usize::from(joint);
        for changed in 0..=selected_guards {
            let changed_inventory = changed == selected_guards;
            let changed_publication = checked && changed == 0;
            let mut wrong_settings = history_profile(
                joint, if changed_publication || changed_inventory { 1 } else { 2 },
            );
            if changed_inventory {
                // An independently expected later window cannot be silently
                // dropped merely because the immutable initial inventory fits.
                let mut missing_window = wrong_settings.windows[0];
                missing_window.window = 2;
                missing_window.start = missing_window.end;
                missing_window.end += 1;
                wrong_settings.windows.push(missing_window);
            }
            let wrong_publication = checked.then(|| {
                history_publication(&root, if changed_publication { 9 } else { 8 })
            });
            let mut config = configured(&root);
            config.programs.clear();
            assert!(wrong_settings.requirements(&config, floor(&config)).is_ok());
            let result = service::serve_history(
                config, &actor, &reviewer, wrong_publication.as_ref(),
                service::Selection {
                    settings: &wrong_settings, mode: service::Mode::Existing(&recovered), requests: &[7],
                    generation: GENERATION, operator: None, evaluator: Some(&fake),
                    governor: None,
                    prechecks: None, inputs: &[],
                },
                || {
                    assert!(!actor.socket.exists());
                    assert!(!reviewer.socket(7).exists());
                    assert!(!fake.socket_path().exists());
                    ElapsedTick(1001)
                },
            );
            assert!(result.is_err(), "changed original profile was admitted");
            assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
            assert_eq!(fs::read(&original).unwrap(), original_bytes);
            assert!(!refused.exists());
            assert!(!fake.socket_path().exists());
            assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
        }

        let mut config = configured(&root);
        config.programs.clear();
        let output = root.0.join("profiled-evaluated.anchor");
        let recovered = recovery(&config, &original, Some(&output));
        let child = EvaluatorProcess::start(&root, "label", 7);
        let evaluator = evaluator_profile(&root, &actor, child.pid(), 2);
        let ready = root.0.join("history.ready");
        service::serve_history(
            config, &actor, &reviewer, publication.as_ref(),
            service::Selection {
                settings: &settings, mode: service::Mode::Existing(&recovered), requests: &[7],
                generation: GENERATION, operator: None, evaluator: Some(&evaluator),
                governor: None,
                prechecks: None, inputs: &[],
            },
            || {
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(7).exists());
                assert!(!root.0.join("evidence.json").exists());
                assert!(!root.0.join("original.bin").exists());
                assert!(!root.0.join("current.bin").exists());
                if evaluator.socket_path().exists() && !ready.exists() {
                    fs::write(&ready, b"original checked/joint evaluator bound").unwrap();
                }
                crate::workflow::clock()
            },
        ).unwrap();
        fs::write(root.0.join("history.finished"), b"profiled evaluator custody retired").unwrap();
        child.finish();
        let after = inspect_profiled(&configured(&root), &settings, publication.as_ref(), &output);
        assert_eq!(after.credibility.report.retained_cases, 1);
        assert_eq!(after.credibility.report.pending_cases, 0);
        assert_eq!(after.credibility.report.benign_origins, 1);
        assert_eq!(after.credibility.journal.executions, before.credibility.journal.executions);
        assert_eq!(after.credibility.journal.payload, before.credibility.journal.payload);
        assert_eq!(after.credibility.journal.control.ledger.charged, before.credibility.journal.control.ledger.charged);
        assert_eq!(after.credibility.journal.control.ledger.reserved, before.credibility.journal.control.ledger.reserved);
        assert_eq!(after.credibility.journal.control.ledger.epoch, before.credibility.journal.control.ledger.epoch + 1);
        assert_eq!(after.accounting, before.accounting);
        assert_eq!(fs::read(&original).unwrap(), original_bytes);
        assert!(after.credibility.journal.stop.is_none());
        let receipt = parse(&fs::read(root.0.join("history.receipt.json")).unwrap());
        let retained = FileHistoryAnchor::read_trusted(
            &mut Cursor::new(fs::read(&output).unwrap()), MAX_HISTORY_ANCHOR_BYTES,
        ).unwrap();
        assert_eq!(retained.revision(), after.credibility.journal.revision);
        assert!(retained.revision() >= number(&receipt, &["after", "journal_revision"]));
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(7).exists());
        assert!(!evaluator.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
    }
}

#[test]
fn historical_stopped_owner_refuses_evaluation_and_retains_only_its_original_fence() {
    let root = Directory::new();
    let original = run_pair(&root);
    let mut config = configured(&root);
    config.programs.clear();
    fs::remove_file(config.source.path()).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings();
    let before = inspect(&config, &settings, &original);
    let stopped_anchor = root.0.join("original-stopped.anchor");
    let recovered = recovery(&config, &original, Some(&stopped_anchor));
    let evaluator = evaluator_profile(&root, &actor, std::process::id() + 100_000, 2);
    let mut calls = 0;
    let mut stop = None;
    let error = service::serve_history(
        config, &actor, &reviewer, None,
        service::Selection {
            settings: &settings, mode: service::Mode::Existing(&recovered), requests: &[7, 8],
            generation: GENERATION, operator: None, evaluator: Some(&evaluator),
            governor: None,
            prechecks: None, inputs: &[],
        },
        || {
            calls += 1;
            if calls == before.credibility.journal.revision + 3 {
                stop = Some(stop_peer(&reviewer));
            }
            ElapsedTick(1001)
        },
    ).unwrap_err();
    assert!(error.contains("native admission Stop acknowledged"), "{error}");
    stop.expect("original Ready Stop must execute").join().unwrap();
    let config = configured(&root);
    let stopped = inspect(&config, &settings, &stopped_anchor);
    assert!(stopped.credibility.journal.stop.is_some());
    assert_eq!(stopped.credibility.journal.revision, before.credibility.journal.revision + 2);
    let stopped_bytes = fs::read(&stopped_anchor).unwrap();
    let output = root.0.join("stopped-successor.anchor");
    let recovered = recovery(&config, &stopped_anchor, Some(&output));
    let mut config = configured(&root);
    config.programs.clear();
    let error = service::serve_history(
        config, &actor, &reviewer, None,
        service::Selection {
            settings: &settings, mode: service::Mode::Existing(&recovered), requests: &[7, 8],
            generation: GENERATION, operator: None, evaluator: Some(&evaluator),
            governor: None,
            prechecks: None, inputs: &[],
        },
        || {
            assert!(!actor.socket.exists());
            assert!(!reviewer.socket(7).exists());
            assert!(!reviewer.socket(8).exists());
            assert!(!evaluator.socket_path().exists());
            ElapsedTick(1002)
        },
    ).unwrap_err();
    assert!(error.contains("stopped ordinary owner"), "{error}");
    assert!(error.contains("acknowledged"), "{error}");
    let after = inspect(&configured(&root), &settings, &output);
    assert_eq!(after.credibility.journal.revision, stopped.credibility.journal.revision + 1);
    assert_eq!(after.credibility.journal.control.ledger.epoch, stopped.credibility.journal.control.ledger.epoch + 1);
    assert_eq!(after.credibility.journal.control.ledger.elapsed, stopped.credibility.journal.control.ledger.elapsed);
    assert_eq!(after.credibility.journal.stop, stopped.credibility.journal.stop);
    assert_eq!(after.credibility.journal.executions, stopped.credibility.journal.executions);
    assert_eq!(after.credibility.journal.payload, stopped.credibility.journal.payload);
    assert_eq!(after.credibility.journal.control.ledger.charged, stopped.credibility.journal.control.ledger.charged);
    assert_eq!(after.credibility.journal.control.ledger.reserved, stopped.credibility.journal.control.ledger.reserved);
    assert_eq!(after.credibility.report, stopped.credibility.report);
    assert_eq!(after.accounting, stopped.accounting);
    assert_eq!(fs::read(&stopped_anchor).unwrap(), stopped_bytes);
    let retained = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(&output).unwrap()), MAX_HISTORY_ANCHOR_BYTES,
    ).unwrap();
    assert_eq!(retained.revision(), after.credibility.journal.revision);
    assert!(!root.0.join("history.case.json").exists());
    assert!(!root.0.join("history.receipt.json").exists());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
    assert!(!evaluator.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
}
