//! Actual generated reviews and a separately authenticated evaluator process.
//! Synthetic model/helper choices exercise native custody, not detector quality.
use super::*;
use crate::workflow::actor_service::{andon_settings, evaluator};
use crate::workflow::actor_service::learned::anchors as archive;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        evaluation::andon::FileAndonRecoveryRequirements,
    },
    reviewer::peer::control::{StopClientProgress, StopControlReceipt},
};
use fa_reference::action::consequence::oversight::{
    actor_peer::PeerCredentials,
    helper_client::{ClientPhase, HelperClient},
    helper_processes::HelperProgram,
};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::round::Verdict;
use fa_reference::strict_json::{self, Json};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Stdio},
};

const CHILD: &str = "workflow::actor_service::native::tests::evaluation_tests::evaluator_process";
const HOLD_CHILD: &str = "workflow::actor_service::native::tests::evaluation_tests::hold_helper_process";
const CHILD_ROOT: &str = "FA_NATIVE_LIVE_EVALUATOR_ROOT";
const CHILD_MODE: &str = "FA_NATIVE_LIVE_EVALUATOR_MODE";
const HOLD_ENV: &str = "FA_NATIVE_LIVE_HOLD_HELPER";

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| &value.as_object().unwrap()[*key])
}

fn number(value: &Json, path: &[&str]) -> u64 {
    field(value, path).as_str().unwrap().parse().unwrap()
}

fn read_json(path: &Path) -> Json {
    strict_json::parse(&fs::read(path).unwrap(), KNOWLEDGE_JSON_LIMITS).unwrap()
}

fn wait_for(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(started.elapsed() < Duration::from_secs(30), "missing {path:?}");
        pause(1);
    }
}

struct EvaluatorChild(Child);
impl EvaluatorChild {
    fn start(root: &Root, mode: &str) -> Self {
        Self(ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD, "--nocapture"])
            .env(CHILD_ROOT, &root.0)
            .env(CHILD_MODE, mode)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn().unwrap())
    }

    fn finish(&mut self) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success(), "independent evaluator failed: {status}");
                return;
            }
            assert!(started.elapsed() < Duration::from_secs(30), "evaluator did not finish");
            pause(1);
        }
    }
}

impl Drop for EvaluatorChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn accounting_file(root: &Root) -> PathBuf {
    let path = root.0.join("andon.json");
    let end = crate::workflow::clock().0.checked_add(60_000).unwrap();
    let content = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/native_andon_operator.json"))
        .replace("2000000000000", &end.to_string())
        .replace("\"false_stops_allowed\": 2", "\"false_stops_allowed\": 1");
    fs::write(&path, content).unwrap();
    path
}

fn peer_file(root: &Root, actor: &Profile, evaluator_pid: u32) -> PathBuf {
    let path = root.0.join("evaluator.json");
    let scope = actor.scope;
    let identity = actor.supervisor;
    let content = format!(
        r#"{{"schema_version":"fa.native-evaluator-peer/1",
          "socket":"{}/evaluator.sock",
          "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
          "deployment":101,"evaluator":202,
          "supervisor":{{"uid":{},"gid":{},"pid":{}}},
          "peer":{{"uid":{},"gid":{},"pid":{evaluator_pid}}},
          "candidate_limit":2,"runtime_ms":15000,"poll_ms":1}}"#,
        root.0.display(), scope.tenant, scope.principal, scope.run, scope.branch,
        scope.authority, identity.uid(), identity.gid(), identity.pid().unwrap(),
        identity.uid(), identity.gid(),
    );
    fs::write(&path, content).unwrap();
    path
}

fn args(root: &Root, accounting: &Path, peer: &Path) -> Vec<String> {
    vec![
        "serve-create".into(), "config".into(), "actor".into(), "reviewer".into(),
        "--native-text".into(), root.0.join("recipe.json").display().to_string(),
        "--andon".into(), accounting.display().to_string(),
        "--evaluator-profile".into(), peer.display().to_string(),
        "--retain-history-anchor".into(), root.0.join("retained.anchor").display().to_string(),
    ]
}

fn selected(
    root: &Root,
    config: &Config,
    actor: &Profile,
    evaluator_pid: u32,
) -> (evaluation::Selection, anchors::Selection, PathBuf) {
    let accounting = accounting_file(root);
    let peer = peer_file(root, actor, evaluator_pid);
    let (remaining, options) = evaluation::take_options(&args(root, &accounting, &peer)).unwrap();
    let (_, history) = anchors::take_options(&remaining).unwrap();
    let history = history.admit(config, false).unwrap().unwrap();
    let (_, reviewer) = profiles(root, config);
    let evaluation = options.unwrap().read(
        (config, actor, &reviewer), (None, None, Some(&history)),
    ).unwrap();
    (evaluation, history, accounting)
}

#[test]
fn evaluator_process() {
    let Ok(root) = std::env::var(CHILD_ROOT) else { return; };
    let root = PathBuf::from(root);
    wait_for(&root.join("actor.finished"));
    let profile = root.join("evaluator.json");
    let settings = evaluator::Settings::read(&profile).unwrap();
    assert_eq!(settings.evaluator.pid, Some(std::process::id()));
    assert_ne!(settings.supervisor.pid, Some(std::process::id()));
    // A terminal review already owns its complete source evidence. Neither
    // discovery nor delayed labeling may reopen these now-missing inputs.
    fs::remove_file(root.join("evidence.json")).unwrap();
    if std::env::var(CHILD_MODE).unwrap() == "partial" {
        let mut socket = UnixStream::connect(settings.socket_path()).unwrap();
        settings.supervisor.policy().unwrap().verify(&socket).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        fs::write(root.join("partial.ready"), b"authenticated incomplete frame").unwrap();
        let mut response = [0_u8; 1];
        match socket.read(&mut response) {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            other => panic!("Stop left the incomplete evaluator exchange live: {other:?}"),
        }
        fs::write(root.join("partial.closed"), b"original Stop retired intake").unwrap();
        return;
    }
    let mut observation = Vec::new();
    evaluator::client::command(&[
        "evaluator-case".into(), profile.display().to_string(), "91".into(),
    ], &mut observation).unwrap();
    let case = strict_json::parse(&observation, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(field(&case, &["knowledge", "state"]).as_str(), Some("known"));
    assert_eq!(number(&case, &["knowledge", "basis", "evaluation_revision"]), 1);
    let evidence_root = field(&case, &["knowledge", "value", "case_evidence_root_hex"])
        .as_str().unwrap();
    let assessment = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},
          "policy_generation":{},"congress_generation":{},
          "case_evidence_root_hex":"{evidence_root}","expected_evaluation_revision":{},
          "origin":91,"evidence_id_hex":"{}","truth":"benign"}}"#,
        number(&case, &["knowledge", "value", "round"]),
        number(&case, &["knowledge", "value", "attempt"]),
        number(&case, &["knowledge", "value", "policy_generation"]),
        number(&case, &["knowledge", "value", "congress_generation"]),
        number(&case, &["knowledge", "basis", "evaluation_revision"]),
        "13".repeat(32),
    );
    let assessment_path = root.join("assessment.json");
    fs::write(&assessment_path, assessment).unwrap();
    let mut receipt = Vec::new();
    evaluator::client::command(&[
        "evaluator-assess".into(), profile.display().to_string(),
        assessment_path.display().to_string(),
    ], &mut receipt).unwrap();
    let applied = strict_json::parse(&receipt, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(field(&applied, &["status"]).as_str(), Some("applied"));
    for coordinate in ["control_sequence", "authority_epoch"] {
        assert_eq!(field(&applied, &["before", coordinate]), field(&applied, &["after", coordinate]));
    }
    assert_eq!(number(&applied, &["after", "evaluation_revision"]), 2);
    assert_eq!(number(&applied, &["after", "journal_revision"]),
        number(&applied, &["before", "journal_revision"]) + 2);
    fs::write(root.join("case.json"), observation).unwrap();
    fs::write(root.join("receipt.json"), receipt).unwrap();
    // Two original candidates were selected: one discovery and one assessment.
    // A third reconnect cannot obtain a replacement allowance in the late phase.
    wait_for(&root.join("service.finished"));
    assert!(UnixStream::connect(settings.socket_path()).is_err());
}

#[test]
fn hold_helper_process() {
    if std::env::var_os(HOLD_ENV).is_none() { return; }
    let profile = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"
    ))).unwrap().profile;
    let expected = profile.committee.members()["alpha"].profile_at(0);
    let mut client = HelperClient::from_process_stdin(expected).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(30));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), "alpha");
            assert!(input.actual_input().submitted_bytes()
                .windows(b"review this fixture: alpha".len())
                .any(|bytes| bytes == b"review this fixture: alpha"));
            client.respond(Verdict::Hold, b"native-evaluated-hold").unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        pause(1);
    }
}

fn hold_program(config: &mut Config, root: &Root) {
    config.programs.insert("alpha".into(), HelperProgram::new(
        std::env::current_exe().unwrap(), root.0.clone(),
        vec!["--exact".into(), HOLD_CHILD.into(), "--nocapture".into()],
        BTreeMap::from([(OsString::from(HOLD_ENV), OsString::from("1"))]),
    ).unwrap());
}

fn observed_requirements(
    profile: &fa_reference::action::consequence::delivery::persistent::observed::FileOversightProfile,
    native: &Inputs,
    source: fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy,
    settings: &andon_settings::Settings,
    minimum_evaluation_revision: u64,
) -> FileAndonRecoveryRequirements {
    FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(native.stream), decoder: Some(native.decoder.clone()),
                decoder_stop: None, source: Some(source), identity: None,
                campaigns: None, credential: None,
            },
            effective_policy: profile.delivery.policy.clone(), credential_epoch: None,
            minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
        },
        settings.config.clone(), settings.windows.clone(), minimum_evaluation_revision,
    ).unwrap()
}

fn run_assessed(hold: bool, approve: bool, partial: bool) {
    let root = Root::new();
    let mut config = configured(&root);
    if hold { hold_program(&mut config, &root); }
    let native = inputs(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let mut child = EvaluatorChild::start(&root, if partial { "partial" } else { "assess" });
    let (evaluation, history, accounting_path) = selected(&root, &config, &actor, child.0.id());
    let accounting = andon_settings::Settings::read(&accounting_path).unwrap();
    let expected = observed_requirements(&config.profile, &native, config.source_policy,
        &accounting, if partial { 1 } else { 2 });
    let store = config.store.clone();
    let profile = config.profile.clone();
    let decoder = native.decoder.clone();
    let tokenizer = native.tokenizer.clone();
    let (sender, receiver) = mpsc::channel();
    let client = actor_thread(actor.clone(), receiver);
    let finished = root.0.join("actor.finished");
    let client = std::thread::spawn(move || {
        let response = client.join().unwrap();
        fs::write(finished, b"terminal original actor result").unwrap();
        response
    });
    let human = (!hold).then(|| reviewing(
        reviewer.clone(), store.clone(), profile.clone(),
        if approve { ReviewDecision::Approve } else { ReviewDecision::Reject },
    ));
    let stop = partial.then(|| {
        let ready = root.0.join("partial.ready");
        let reviewer = reviewer.clone();
        std::thread::spawn(move || {
            wait_for(&ready);
            let mut client = reviewer.connect_stop_client(91).unwrap();
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(15));
                match client.step().unwrap() {
                    StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                    StopClientProgress::Complete => return client.receipt().unwrap(),
                    _ => pause(1),
                }
            }
        })
    });
    let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let result = serve_lifecycle_with_evaluation(
        config, (&actor, &reviewer), native,
        (false, None, None, finish::Intent::Existing),
        Services { operator: None, precheck: None, history: Some(&history), evaluation: Some(&evaluation) },
        crate::workflow::clock, &mut output,
    );
    fs::write(root.0.join("service.finished"), b"same original service ended").unwrap();
    assert!(result.is_ok(), "{result:?}");
    child.finish();
    let response = client.join().unwrap();
    if let Some(human) = human {
        let packet = human.join().unwrap();
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        assert_eq!(packet.views().len(), 2);
    } else {
        assert!(!reviewer.socket(91).exists(), "Hold never obtains a human effect key");
    }
    let anchor = archive::load(&root.0.join("retained.anchor")).unwrap();
    let saved = FileOversight::read_andon_guarded(
        &store, &profile, &expected, &accounting.protocol, &anchor,
    ).unwrap();
    let generated = FileOversight::read_decoder_text_progress(
        &store, &profile, &decoder, &tokenizer, 7,
    ).unwrap();
    assert_eq!(generated.text.bytes().unwrap(), b"A");
    assert_eq!(generated.text.generation_revision(), 4);
    assert_eq!(generated.numerical.numerical.position, 4);
    assert_eq!(generated.numerical.numerical.sampled_draws, 2);
    assert_eq!(saved.credibility.journal.revision, anchor.revision());
    assert_eq!(saved.credibility.journal.revision, generated.publication.revision);
    assert_eq!(saved.credibility.report.retained_cases, 1);
    assert!(!root.0.join("evidence.json").exists(), "late assessment must not need the old source file");
    let executed = !hold && approve;
    assert_eq!(saved.credibility.journal.executions, u64::from(executed));
    assert_eq!(saved.credibility.journal.payload, if executed { b"A".to_vec() } else { Vec::new() });
    assert!(matches!(response.result, Ok(Knowledge::Known { value, .. })
        if value == if executed { ActorOutcome::Executed } else { ActorOutcome::CancelledBeforeDispatch }));
    assert_eq!(saved.credibility.journal.control.ledger.reserved, 0);
    let window = &saved.accounting.windows()[&1];
    if partial {
        let receipt: StopControlReceipt = stop.unwrap().join().unwrap();
        assert!(receipt.acknowledged());
        assert!(receipt.drained());
        assert!(saved.credibility.journal.stop.is_some());
        assert_eq!(saved.credibility.report.pending_cases, 1);
        assert_eq!(saved.credibility.report.benign_origins, 0);
        assert_eq!(window.false_stops, 0);
        assert!(window.incident.is_none());
        assert!(root.0.join("partial.closed").exists());
        assert!(!root.0.join("assessment.json").exists());
    } else {
        assert_eq!(saved.credibility.report.revision, 2);
        assert_eq!(saved.credibility.report.pending_cases, 0);
        assert_eq!(saved.credibility.report.benign_origins, 1);
        assert_eq!(window.false_stops, u64::from(hold));
        assert_eq!(window.total_cost, if hold { 7 } else { 0 });
        assert_eq!(window.incident.is_some(), hold);
        if let Some(incident) = &window.incident {
            assert_eq!(incident.opened_by_round, 91);
            assert_eq!(incident.rounds.iter().copied().collect::<Vec<_>>(), vec![91]);
        }
        let receipt = read_json(&root.0.join("receipt.json"));
        // The native actor can receive its terminal observation before the
        // original workflow finishes its cleanup checkpoints. The retained
        // exact history must include the acknowledged label, even if those
        // original checkpoints append a later time observation.
        assert!(anchor.revision() >= number(&receipt, &["after", "journal_revision"]));
        assert_eq!(field(&receipt, &["before", "control_sequence"]), field(&receipt, &["after", "control_sequence"]));
        assert_eq!(field(&receipt, &["before", "authority_epoch"]), field(&receipt, &["after", "authority_epoch"]));
        assert!(saved.credibility.journal.stop.is_none());
    }
    assert!(!actor.socket.exists());
    assert!(!evaluation.peer.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 91).exists());
    assert_eq!(std::str::from_utf8(&output.bytes).unwrap().lines().count(), 1);
}

#[test]
fn generated_allow_still_requires_the_original_independent_human_key() {
    for approve in [false, true] { run_assessed(false, approve, false); }
}

#[test]
fn final_generated_hold_remains_assessable_with_the_original_two_candidate_allowance() {
    run_assessed(true, false, false);
}

#[test]
fn original_stop_preempts_a_partial_evaluator_frame_after_generated_hold() {
    run_assessed(true, false, true);
}

#[test]
fn native_evaluation_options_are_closed_before_any_profile_or_clock_is_read() {
    let root = Root::new();
    let valid = args(&root, Path::new("missing-andon"), Path::new("missing-evaluator"));
    let (remaining, options) = evaluation::take_options(&valid).unwrap();
    assert!(options.is_some());
    assert!(!remaining.iter().any(|arg| matches!(arg.as_str(), "--andon" | "--evaluator-profile")));
    for which in 0..7 {
        let mut bad = valid.clone();
        match which {
            0 => { bad[0] = "serve-open".into(); }
            1 => { bad.drain(6..8); }
            2 => { bad.drain(8..10); }
            3 => { bad.truncate(10); }
            4 => bad.extend(["--andon".into(), "duplicate".into()]),
            5 => bad.extend(["--requests".into(), "91,92".into()]),
            _ => bad.extend(["--operator-profile".into(), "unpaired".into()]),
        }
        assert!(evaluation::take_options(&bad).is_err(), "invalid grammar {which}");
        let mut output = Vec::new();
        let error = command_with_clock(&bad, None,
            || panic!("invalid opted-in grammar read a clock"), &mut output).unwrap_err();
        assert!(error.contains("native evaluation requires"), "{error}");
        assert!(output.is_empty());
        assert!(!root.0.join("store").exists());
    }
}

#[test]
fn evaluator_audience_roles_and_custody_aliases_refuse_before_owner_work() {
    for which in 0..5 {
        let root = Root::new();
        let config = configured(&root);
        let native = inputs(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let (socket, _) = UnixStream::pair().unwrap();
        let pid = PeerCredentials::observe(&socket).unwrap().pid();
        let (mut evaluation, history, _) = selected(&root, &config, &actor, pid.checked_add(1).unwrap());
        evaluation.check(&config, &actor, &reviewer, (None, None, Some(&history)),
            (false, None, None)).unwrap();
        match which {
            0 => evaluation.peer.scope.principal += 1,
            1 => evaluation.peer.evaluator.pid = Some(pid),
            2 => evaluation.peer.socket = actor.socket.clone(),
            3 => evaluation.peer.socket = config.source.path().to_path_buf(),
            _ => evaluation.peer.socket = history.output().unwrap().to_path_buf(),
        }
        let source = fs::read(config.source.path()).unwrap();
        let mut output = Vec::new();
        let store = config.store.clone();
        assert!(serve_lifecycle_with_evaluation(
            config, (&actor, &reviewer), native,
            (false, None, None, finish::Intent::Existing),
            Services { operator: None, precheck: None, history: Some(&history), evaluation: Some(&evaluation) },
            || panic!("invalid evaluator selection read a clock"), &mut output,
        ).is_err());
        assert!(output.is_empty());
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert!(!history.output().unwrap().exists());
        assert_eq!(fs::read(root.0.join("evidence.json")).unwrap(), source);
    }
}

#[path = "evaluation/recovery.rs"]
mod recovery;
