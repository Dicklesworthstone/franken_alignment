//! Real learned generation/native helper review and a separate evaluator process.
//! Fixed weights exercise custody and two-key causality, not detector quality.
#[path = "evaluation/live_credibility.rs"]
mod live_credibility_tests;
#[path = "evaluation/mediated.rs"]
mod mediated_tests;
#[path = "evaluation/recovery.rs"]
mod recovery_tests;
use super::super::anchors as archive;
use super::peer_intake::{self as peers, Behavior};
use super::*;
use crate::workflow::actor_service::{Profile, andon_settings, evaluator};
use fa_reference::action::consequence::delivery::{
    persistent::observed::reviewer::{
        peer::control::{StopClientProgress, StopControlReceipt},
        wire::ReviewDecision,
    },
    stream::ReleaseFrame,
};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Stdio},
};

const CHILD: &str = "workflow::actor_service::learned::tests::evaluation_tests::evaluator_process";
const CHILD_ROOT: &str = "FA_LEARNED_LIVE_EVALUATOR_ROOT";
const CHILD_MODE: &str = "FA_LEARNED_LIVE_EVALUATOR_MODE";

pub(super) fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| &value.as_object().unwrap()[*key])
}

pub(super) fn number(value: &Json, path: &[&str]) -> u64 {
    field(value, path).as_str().unwrap().parse().unwrap()
}

pub(super) fn read_json(path: &Path) -> Json {
    strict_json::parse(&fs::read(path).unwrap(), KNOWLEDGE_JSON_LIMITS).unwrap()
}

pub(super) fn wait_for(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "missing {path:?}"
        );
        crate::workflow::pause(1);
    }
}

struct EvaluatorChild(Child);
impl EvaluatorChild {
    fn start(root: &Root, mode: &str) -> Self {
        Self(
            ProcessCommand::new(std::env::current_exe().unwrap())
                .args(["--exact", CHILD, "--nocapture"])
                .env(CHILD_ROOT, &root.0)
                .env(CHILD_MODE, mode)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        )
    }

    fn finish(&mut self) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success(), "independent evaluator failed: {status}");
                return;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "evaluator did not finish"
            );
            crate::workflow::pause(1);
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

pub(super) fn accounting_file(root: &Root) -> PathBuf {
    let path = root.0.join("andon.json");
    let end = crate::workflow::clock().0.checked_add(60_000).unwrap();
    let content = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/native_andon_operator.json"
    ))
    .replace("2000000000000", &end.to_string())
    // Recovery fixtures retain a separate [100, 200) historical window. Their
    // original reviews occur at tick 1000 or wall time, so keep both windows
    // disjoint without changing a review's original accounting interval.
    .replace("\"start\": 0", "\"start\": 200")
    .replace("\"false_stops_allowed\": 2", "\"false_stops_allowed\": 1");
    fs::write(&path, content).unwrap();
    path
}

pub(super) fn joint_accounting_file(root: &Root) -> PathBuf {
    let path = accounting_file(root);
    let marginal = fs::read_to_string(&path).unwrap();
    let joint = with_joint_policy(&marginal);
    assert_ne!(joint, marginal);
    fs::write(&path, joint).unwrap();
    path
}

pub(super) fn with_joint_policy(marginal: &str) -> String {
    marginal.replace(
        "\"schema_version\": \"fa.native-andon-operator/1\",",
        r#""schema_version": "fa.native-andon-operator/2",
  "joint": {
    "id": 71,
    "generation": 1,
    "maximum_escape_rate": {"numerator": 0, "denominator": 1},
    "maximum_benign_stop_rate": {"numerator": 1, "denominator": 2},
    "budget": {"cases": 512, "member_outcomes": 2048}
  },"#,
    )
}

pub(super) fn peer_file(root: &Root, actor: &Profile, evaluator_pid: u32) -> PathBuf {
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
        root.0.display(),
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        identity.uid(),
        identity.gid(),
        identity.pid().unwrap(),
        identity.uid(),
        identity.gid(),
    );
    fs::write(&path, content).unwrap();
    path
}

fn args(root: &Root, accounting: &Path, peer: &Path) -> Vec<String> {
    vec![
        "create-learned-generated".into(),
        "config".into(),
        root.0.join("recipe.json").display().to_string(),
        "reviewer".into(),
        "--actor-profile".into(),
        "actor".into(),
        "--andon".into(),
        accounting.display().to_string(),
        "--evaluator-profile".into(),
        peer.display().to_string(),
        "--retain-history-anchor".into(),
        root.0.join("retained.anchor").display().to_string(),
    ]
}

fn selected(
    root: &Root,
    config: &Config,
    loaded: &mut Loaded,
    actor: &Profile,
    evaluator_pid: u32,
) -> (evaluation::Selection, PathBuf) {
    selected_profile(root, config, loaded, actor, evaluator_pid, false)
}

fn selected_profile(
    root: &Root,
    config: &Config,
    loaded: &mut Loaded,
    actor: &Profile,
    evaluator_pid: u32,
    joint: bool,
) -> (evaluation::Selection, PathBuf) {
    let accounting = if joint {
        joint_accounting_file(root)
    } else {
        accounting_file(root)
    };
    let peer = peer_file(root, actor, evaluator_pid);
    let (remaining, options) =
        evaluation::take_options(&args(root, &accounting, &peer), None).unwrap();
    let evaluation = options.unwrap().read().unwrap();
    let (_, history) = archive::take_options(&remaining).unwrap();
    history.admit(config, loaded, false).unwrap();
    (evaluation, accounting)
}

#[test]
fn evaluator_process() {
    let Ok(root) = std::env::var(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    wait_for(&root.join("actor.finished"));
    let profile = root.join("evaluator.json");
    let settings = evaluator::Settings::read(&profile).unwrap();
    assert_eq!(settings.evaluator.pid, Some(std::process::id()));
    assert_ne!(settings.supervisor.pid, Some(std::process::id()));
    // Complete original review owns its evidence. Delayed truth cannot reopen
    // the source, generation inputs or native helper roster after this cut.
    for path in [
        "evidence.json",
        "model.json",
        "weights.safetensors",
        "native-roster.json",
    ] {
        fs::remove_file(root.join(path)).unwrap();
    }
    if std::env::var(CHILD_MODE).unwrap() == "partial" {
        let mut socket = UnixStream::connect(settings.socket_path()).unwrap();
        settings
            .supervisor
            .policy()
            .unwrap()
            .verify(&socket)
            .unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        fs::write(
            root.join("partial.ready"),
            b"authenticated incomplete frame",
        )
        .unwrap();
        let mut response = [0_u8; 1];
        match socket.read(&mut response) {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            other => panic!("Stop left an incomplete learned evaluator exchange live: {other:?}"),
        }
        fs::write(root.join("partial.closed"), b"original Stop retired intake").unwrap();
        return;
    }
    let mut observation = Vec::new();
    evaluator::client::command(
        &[
            "evaluator-case".into(),
            profile.display().to_string(),
            "1".into(),
        ],
        &mut observation,
    )
    .unwrap();
    let case = strict_json::parse(&observation, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(
        field(&case, &["knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(
        number(&case, &["knowledge", "basis", "evaluation_revision"]),
        1
    );
    let evidence_root = field(&case, &["knowledge", "value", "case_evidence_root_hex"])
        .as_str()
        .unwrap();
    let assessment = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},
          "policy_generation":{},"congress_generation":{},
          "case_evidence_root_hex":"{evidence_root}","expected_evaluation_revision":{},
          "origin":9001,"evidence_id_hex":"{}","truth":"benign"}}"#,
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
    evaluator::client::command(
        &[
            "evaluator-assess".into(),
            profile.display().to_string(),
            assessment_path.display().to_string(),
        ],
        &mut receipt,
    )
    .unwrap();
    let applied = strict_json::parse(&receipt, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(field(&applied, &["status"]).as_str(), Some("applied"));
    for coordinate in ["control_sequence", "authority_epoch"] {
        assert_eq!(
            field(&applied, &["before", coordinate]),
            field(&applied, &["after", coordinate])
        );
    }
    assert_eq!(number(&applied, &["after", "evaluation_revision"]), 2);
    assert_eq!(
        number(&applied, &["after", "journal_revision"]),
        number(&applied, &["before", "journal_revision"]) + 2
    );
    fs::write(root.join("case.json"), observation).unwrap();
    fs::write(root.join("receipt.json"), receipt).unwrap();
    // The original two candidates are case discovery and one assessment.
    // Finishing the actor cannot replace that lifetime with a fresh allowance.
    wait_for(&root.join("service.finished"));
    assert!(UnixStream::connect(settings.socket_path()).is_err());
}

/// Retain the production RequestCycle instead of reopening an evaluated owner.
/// The same native startup and request executor run normally; only final cleanup
/// waits until this test has inspected the original acknowledged live state.
fn run_retained<F>(
    config: Config,
    loaded: &Loaded,
    reviewer: &PeerProfile,
    actor: &Profile,
    selected: &evaluation::Selection,
    time: F,
) -> Result<RequestCycle, String>
where
    F: FnMut() -> ElapsedTick,
{
    run_retained_with_operator(config, loaded, reviewer, actor, selected, None, time)
}

fn run_retained_with_operator<F>(
    mut config: Config,
    loaded: &Loaded,
    reviewer: &PeerProfile,
    actor: &Profile,
    selected: &evaluation::Selection,
    selected_operator: Option<&operator::Selection>,
    mut time: F,
) -> Result<RequestCycle, String>
where
    F: FnMut() -> ElapsedTick,
{
    reviewer.check_host(&config)?;
    ingress::check(&config, loaded, reviewer, actor)?;
    checked::check_selection(&config, loaded)?;
    if let Some(operator) = selected_operator {
        operator.check(&config, loaded, actor, reviewer, None)?;
    }
    selected.check(
        &config,
        loaded,
        Some(actor),
        reviewer,
        (selected_operator, None),
        false,
        None,
        None,
        None,
    )?;
    let start = time();
    let lifetime = loaded
        .ttl_ms
        .min(config.timing.runtime_ms)
        .min(actor.runtime_ms);
    let mut deadline = Deadline {
        logical: plus(start, lifetime)?,
        started: Instant::now(),
        wall: Duration::from_millis(lifetime),
    };
    deadline.check(start)?;
    let mut credential = credential::prepare(loaded, false, &deadline, &mut time)?;
    let mut control = Control::new(&config, loaded.request, Some(reviewer))?;
    let bound_actor = ingress::Bound::bind(actor)?;
    let ((host, human, observer, identity_observer), evaluator_role) =
        selected.create(&config, loaded)?;
    let mut prediction =
        observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let mut identity = identity_observer.map(identity::Session::new);
    prediction::check(&host, prediction.as_ref())?;
    let continuation = request_preflight(&host, loaded, false, None, None, None)?;
    let (port, supervisor) = host
        .into_learned_text_stream_actor_gateway()
        .map_err(debug)?;
    let mut intake =
        Some(bound_actor.attach(port.clone(), LearnedTextRelease::Message, deadline.logical)?);
    let driver = FileSupervisedDriver::new(supervisor);
    let mut service =
        evaluator::Service::start(&selected.peer, actor, evaluator_role, &driver, &deadline)?;
    let mut operator = selected_operator
        .map(|selected| selected.start(actor, port.clone(), &driver, &deadline))
        .transpose()?;
    let mut wire = ActorWire::new(port);
    let mut cycle = execute_request_with_evaluation(
        driver,
        &mut config,
        loaded,
        RequestServices {
            peers: reviewer,
            reviewer: &human,
            prediction: &mut prediction,
            identity: &mut identity,
            credential: &mut credential,
            intake: &mut intake,
            wire: &mut wire,
            operator: &mut operator,
            finishing: None,
            message_continuation: continuation,
            credibility: None,
            stop_operation: loaded.request,
        },
        &mut control,
        &mut deadline,
        &mut time,
        Some(&mut service),
    );
    assert!(
        service.is_retired(),
        "evaluation role survives into retained history"
    );
    service.retire();
    cycle.failure = archive::retain(loaded, &cycle.driver, cycle.failure);
    Ok(cycle)
}

fn run_assessed(hold: bool, approve: bool, partial: bool) {
    run_assessed_profile(hold, approve, partial, false);
}

fn run_assessed_profile(hold: bool, approve: bool, partial: bool, joint: bool) {
    let root = Root::new();
    let config = configured(&root);
    let path = fixture::write(
        &root,
        if hold {
            b"hold".as_slice()
        } else {
            b"allow".as_slice()
        },
    );
    evidence(&root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    assert_eq!(loaded.rounds, vec![1]);
    let reviewer = fixture::peers(&root, &config);
    let actor = peers::actor_profile(&root, &config, 1);
    let mut child = EvaluatorChild::start(&root, if partial { "partial" } else { "assess" });
    let (evaluation, accounting_path) =
        selected_profile(&root, &config, &mut loaded, &actor, child.0.id(), joint);
    let accounting = andon_settings::Settings::read(&accounting_path).unwrap();
    let cleanup_ms = config.timing.cleanup_ms;
    let poll_ms = config.timing.poll_ms;
    let due = plus(crate::workflow::clock(), 55_000).unwrap();
    let mut submitted = None;
    let mut approved = None;
    let stop = partial.then(|| {
        let ready = root.0.join("partial.ready");
        let reviewer = reviewer.clone();
        std::thread::spawn(move || {
            wait_for(&ready);
            let mut client = reviewer.connect_stop_client(1).unwrap();
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(15));
                match client.step().unwrap() {
                    StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                    StopClientProgress::Complete => return client.receipt().unwrap(),
                    _ => crate::workflow::pause(1),
                }
            }
        })
    });
    let result = run_retained(config, &loaded, &reviewer, &actor, &evaluation, || {
        if submitted.is_none() && actor.socket.exists() {
            let peer = peers::actor(
                actor.clone(),
                LearnedTextRelease::Message,
                due,
                Behavior::Observe,
            );
            let finished = root.0.join("actor.finished");
            submitted = Some(std::thread::spawn(move || {
                let response = peer.join().unwrap();
                fs::write(finished, b"terminal original learned actor outcome").unwrap();
                response
            }));
        }
        if !hold && approved.is_none() && reviewer.socket(1).exists() {
            approved = Some(peers::human(
                reviewer.clone(),
                1,
                if approve {
                    ReviewDecision::Approve
                } else {
                    ReviewDecision::Reject
                },
            ));
        }
        crate::workflow::clock()
    })
    .unwrap();
    fs::write(
        root.0.join("service.finished"),
        b"same original service ended",
    )
    .unwrap();
    child.finish();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    let response = submitted
        .expect("real learned actor was admitted")
        .join()
        .unwrap();
    assert_eq!(response.result, result.response.result);
    if let Some(human) = approved {
        let packet = human.join().unwrap();
        assert_eq!(
            ReleaseFrame::decode(&packet.action().spec().payload)
                .unwrap()
                .message(),
            Some("A")
        );
        assert_eq!(packet.views().len(), 2);
    } else {
        assert!(
            !reviewer.socket(1).exists(),
            "a native Hold cannot obtain a human effect key"
        );
    }
    let anchor = archive::load(&root.0.join("retained.anchor")).unwrap();
    let host = result.driver.supervisor().host().unwrap();
    let saved = host.inspect();
    assert_eq!(saved.revision, anchor.revision());
    let executed = !hold && approve;
    assert_eq!(saved.executions, u64::from(executed));
    assert_eq!(
        saved.payload,
        if executed { b"A".to_vec() } else { Vec::new() }
    );
    assert!(matches!(response.result, Ok(Knowledge::Known { value, .. })
        if value == if executed { ActorOutcome::Executed } else { ActorOutcome::CancelledBeforeDispatch }));
    assert_eq!(saved.control.ledger.reserved, 0);
    for path in [
        "evidence.json",
        "model.json",
        "weights.safetensors",
        "native-roster.json",
    ] {
        assert!(
            !root.0.join(path).exists(),
            "late evaluation reopened original input {path}"
        );
    }

    // Inspect the very same owner after label retirement and exact anchor export.
    // No recovery path, fence, new evaluator or replayed numerical state is used.
    assert_eq!(&host.history_anchor().unwrap(), &anchor);
    assert_eq!(
        host.learned_generation_inspection()
            .unwrap()
            .numerical
            .sampled_draws,
        2
    );
    assert_eq!(host.andon_config().unwrap(), Some(&accounting.config));
    assert_eq!(host.joint_credibility_policy().unwrap(), accounting.joint);
    assert_eq!(accounting.joint.is_some(), joint);
    let report = host.credibility_report().unwrap();
    assert_eq!(report.retained_cases, 1);
    // This fixture has no independently labeled violation origin. Observing a
    // benign case cannot qualify the joint candidate or promote its committee.
    if joint {
        assert!(host.joint_credibility_report(report.revision).is_err());
    }
    let window = &host.andon_ledger().unwrap().windows()[&1];
    if partial {
        let receipt: StopControlReceipt = stop.unwrap().join().unwrap();
        assert!(receipt.acknowledged() && receipt.drained());
        assert!(saved.stop.is_some());
        assert_eq!(report.pending_cases, 1);
        assert_eq!(report.benign_origins, 0);
        assert_eq!(window.false_stops, 0);
        assert_eq!(window.total_cost, 0);
        assert!(window.incident.is_none());
        assert!(root.0.join("partial.closed").exists());
        assert!(!root.0.join("assessment.json").exists());
    } else {
        assert_eq!(report.revision, 2);
        assert_eq!(report.pending_cases, 0);
        assert_eq!(report.benign_origins, 1);
        assert_eq!(window.false_stops, u64::from(hold));
        assert_eq!(window.total_cost, if hold { 7 } else { 0 });
        assert_eq!(window.incident.is_some(), hold);
        if let Some(incident) = &window.incident {
            assert_eq!(incident.opened_by_round, 1);
            assert_eq!(incident.rounds.iter().copied().collect::<Vec<_>>(), vec![1]);
        }
        let receipt = read_json(&root.0.join("receipt.json"));
        assert!(anchor.revision() >= number(&receipt, &["after", "journal_revision"]));
        assert!(saved.stop.is_none());
    }
    assert!(!actor.socket.exists());
    assert!(!evaluation.peer.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 1).exists());
    drop(host);
    assert!(
        result.process.is_none(),
        "this fixture owns native learned helpers"
    );
    assert_eq!(cleanup(result.driver, cleanup_ms, poll_ms), 0);
    drop(result.history);
}

#[test]
fn learned_allow_still_needs_the_original_independent_human_key() {
    for approve in [false, true] {
        run_assessed(false, approve, false);
    }
}

#[test]
fn final_learned_hold_remains_assessable_under_the_original_two_candidate_allowance() {
    run_assessed(true, false, false);
}

#[test]
fn original_stop_preempts_a_partial_evaluator_frame_after_learned_hold() {
    run_assessed(true, false, true);
}

#[test]
fn joint_learned_evaluation_preserves_native_and_independent_human_publication_keys() {
    for approve in [false, true] {
        run_assessed_profile(false, approve, false, true);
    }
}

#[test]
fn joint_learned_hold_keeps_original_false_stop_costs_and_unqualified_joint_evidence() {
    run_assessed_profile(true, false, false, true);
}

#[test]
fn joint_learned_stop_retires_partial_evaluation_without_applying_a_label() {
    run_assessed_profile(true, false, true, true);
}

#[test]
fn actual_outer_learned_service_delivers_a_hold_and_retains_the_late_assessment() {
    let root = Root::new();
    let config = configured(&root);
    let path = fixture::write(&root, b"hold");
    evidence(&root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    let reviewer = fixture::peers(&root, &config);
    let actor = peers::actor_profile(&root, &config, 1);
    let mut child = EvaluatorChild::start(&root, "assess");
    let (evaluation, _) = selected(&root, &config, &mut loaded, &actor, child.0.id());
    let due = plus(crate::workflow::clock(), 55_000).unwrap();
    let mut submitted = None;
    let result = run_with_actor_services_evaluated(
        config,
        loaded,
        &reviewer,
        false,
        None,
        None,
        None,
        Some(&actor),
        None,
        None,
        Some(&evaluation),
        || {
            if submitted.is_none() && actor.socket.exists() {
                let peer = peers::actor(
                    actor.clone(),
                    LearnedTextRelease::Message,
                    due,
                    Behavior::Observe,
                );
                let finished = root.0.join("actor.finished");
                submitted = Some(std::thread::spawn(move || {
                    let response = peer.join().unwrap();
                    fs::write(finished, b"terminal original learned actor outcome").unwrap();
                    response
                }));
            }
            crate::workflow::clock()
        },
    )
    .unwrap();
    fs::write(
        root.0.join("service.finished"),
        b"actual outer service ended",
    )
    .unwrap();
    child.finish();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    let response = submitted
        .expect("actual outer service admitted its actor")
        .join()
        .unwrap();
    assert_eq!(response.result, result.response.result);
    assert!(matches!(
        response.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        })
    ));
    let receipt = read_json(&root.0.join("receipt.json"));
    assert_eq!(field(&receipt, &["status"]).as_str(), Some("applied"));
    assert_eq!(number(&receipt, &["after", "evaluation_revision"]), 2);
    let anchor = archive::load(&root.0.join("retained.anchor")).unwrap();
    assert!(anchor.revision() >= number(&receipt, &["after", "journal_revision"]));
    assert!(
        !reviewer.socket(1).exists(),
        "Hold never offered a human publication key"
    );
    assert!(!actor.socket.exists());
    assert!(!evaluation.peer.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 1).exists());
}

#[test]
fn learned_evaluation_grammar_refuses_unsupported_modes_before_profiles_or_clocks() {
    let root = Root::new();
    let valid = args(
        &root,
        Path::new("missing-andon"),
        Path::new("missing-evaluator"),
    );
    let (remaining, options) = evaluation::take_options(&valid, None).unwrap();
    assert!(options.is_some());
    assert!(
        !remaining
            .iter()
            .any(|arg| matches!(arg.as_str(), "--andon" | "--evaluator-profile"))
    );
    for which in 0..10 {
        let mut bad = valid.clone();
        match which {
            0 => bad[0] = "serve-learned-generated".into(),
            1 => {
                bad.drain(4..6);
            }
            2 => {
                bad.drain(6..8);
            }
            3 => {
                bad.drain(8..10);
            }
            4 => bad.truncate(10),
            5 => bad.extend(["--andon".into(), "duplicate".into()]),
            6 => bad.extend(["--history-anchor".into(), "old-custody".into()]),
            7 => bad.extend(["--after-message".into(), "1".into()]),
            8 => bad.extend(["--operator-profile".into(), "unpaired".into()]),
            _ => bad[3] = "--resume".into(),
        }
        let error = command_with_clock(&bad, None, || {
            panic!("invalid evaluation grammar acquired a clock")
        })
        .unwrap_err();
        assert!(
            error.contains("learned evaluation requires"),
            "{which}: {error}"
        );
        assert!(!root.0.join("store").exists());
    }
    assert!(evaluation::take_options(&valid, Some(Path::new("capsule"))).is_err());
    let legacy = vec![
        "create-learned-generated".into(),
        "config".into(),
        "recipe".into(),
        "reviewer".into(),
    ];
    let (remaining, selected) = evaluation::take_options(&legacy, None).unwrap();
    assert_eq!(remaining, legacy);
    assert!(selected.is_none());
}

#[test]
fn learned_evaluator_role_and_custody_mismatches_refuse_before_any_owner_work() {
    for which in 0..7 {
        let root = Root::new();
        let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        evidence(&root, &config, 1, true, false);
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        let reviewer = fixture::peers(&root, &config);
        let actor = peers::actor_profile(&root, &config, 1);
        let pid = actor.supervisor.pid().unwrap();
        let (mut selected, _) = selected(
            &root,
            &config,
            &mut loaded,
            &actor,
            pid.checked_add(1).unwrap(),
        );
        selected
            .check(
                &config,
                &loaded,
                Some(&actor),
                &reviewer,
                (None, None),
                false,
                None,
                None,
                None,
            )
            .unwrap();
        match which {
            0 => selected.peer.scope.principal += 1,
            1 => selected.peer.evaluator.pid = Some(pid),
            2 => selected.peer.socket = actor.socket.clone(),
            3 => selected.peer.socket = config.source.path().to_path_buf(),
            4 => selected.peer.socket = loaded.anchor_output.clone().unwrap(),
            5 => loaded.anchor_output = Some(actor.socket.clone()),
            _ => loaded.anchor_output = None,
        }
        let source = fs::read(config.source.path()).unwrap();
        let store = config.store.clone();
        assert!(
            run_with_actor_services_evaluated(
                config,
                loaded,
                &reviewer,
                false,
                None,
                None,
                None,
                Some(&actor),
                None,
                None,
                Some(&selected),
                || panic!("refused evaluator selection acquired a clock"),
            )
            .is_err(),
            "invalid selection {which}"
        );
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        assert_eq!(fs::read(root.0.join("evidence.json")).unwrap(), source);
    }
}
