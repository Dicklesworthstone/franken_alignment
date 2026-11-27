//! Real learned generation, native helper/human publication, original topology
//! source custody and independent late labels share one accounted owner.
//! Synthetic weights and graphs establish composition, not detector quality.
use super::super::super::topology as custody;
use super::super::{continuation as native, mediated_topology as topology_fixture};
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot,
    observed::{
        driver::topology::checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
        guarded::evaluation::andon::FileAndonRecoveryRequirements,
        mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
        reviewer::wire::ReviewPacket,
    },
};
use std::io::Cursor;

#[path = "mediated/history.rs"]
mod historical_tests;

const PEER_TEST: &str =
    "workflow::actor_service::learned::tests::evaluation_tests::mediated_tests::evaluator_process";
const PEER_ROOT: &str = "FA_MEDIATED_ACCOUNTED_ROOT";
const PEER_REQUEST: &str = "FA_MEDIATED_ACCOUNTED_REQUEST";
const PEER_ROUND: &str = "FA_MEDIATED_ACCOUNTED_ROUND";
const PEER_MODE: &str = "FA_MEDIATED_ACCOUNTED_MODE";

struct Independent(Option<Child>);

impl Independent {
    fn start(root: &Root, request: u64, round: u64, mode: &str) -> Self {
        Self(Some(
            ProcessCommand::new(std::env::current_exe().unwrap())
                .args(["--exact", PEER_TEST, "--nocapture"])
                .env(PEER_ROOT, &root.0)
                .env(PEER_REQUEST, request.to_string())
                .env(PEER_ROUND, round.to_string())
                .env(PEER_MODE, mode)
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

    fn finish(&mut self) {
        let started = Instant::now();
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(started.elapsed() < Duration::from_secs(30));
            crate::workflow::pause(1);
        }
        let output = self.0.take().unwrap().wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    }
}

impl Drop for Independent {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

#[test]
fn evaluator_process() {
    let Ok(root) = std::env::var(PEER_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    let request = std::env::var(PEER_REQUEST).unwrap();
    let round = std::env::var(PEER_ROUND).unwrap().parse::<u64>().unwrap();
    assert!(
        round <= u8::MAX.into(),
        "the explicit fixture evidence ID is one byte repeated"
    );
    wait_for(&root.join(format!("actor-{request}.finished")));
    let profile = root.join("evaluator.json");
    let selected = evaluator::Settings::read(&profile).unwrap();
    assert_eq!(selected.evaluator.pid, Some(std::process::id()));
    assert_ne!(selected.supervisor.pid, Some(std::process::id()));
    let mode = std::env::var(PEER_MODE).unwrap();
    if mode == "partial" {
        let mut socket = UnixStream::connect(selected.socket_path()).unwrap();
        selected
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
            b"authenticated incomplete assessment",
        )
        .unwrap();
        let mut response = [0; 1];
        match socket.read(&mut response) {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            other => panic!("Stop retained incomplete evaluator input: {other:?}"),
        }
        fs::write(
            root.join("partial.closed"),
            b"original Stop retired evaluator",
        )
        .unwrap();
    } else {
        if mode == "delete" {
            for name in [
                "evidence.json",
                "model.json",
                "weights.safetensors",
                "native-roster.json",
            ] {
                fs::remove_file(root.join(name)).unwrap();
            }
        }
        let mut case = Vec::new();
        evaluator::client::command(
            &[
                "evaluator-case".into(),
                profile.display().to_string(),
                round.to_string(),
            ],
            &mut case,
        )
        .unwrap();
        let original = strict_json::parse(&case, KNOWLEDGE_JSON_LIMITS).unwrap();
        assert_eq!(
            field(&original, &["knowledge", "state"]).as_str(),
            Some("known")
        );
        assert_eq!(number(&original, &["knowledge", "value", "round"]), round);
        let evidence = field(&original, &["knowledge", "value", "case_evidence_root_hex"])
            .as_str()
            .unwrap();
        let assessment = format!(
            r#"{{"schema_version":"fa.native-assessment/1","round":{round},"attempt":{},
            "policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{evidence}",
            "expected_evaluation_revision":{},"origin":{},"evidence_id_hex":"{}","truth":"benign"}}"#,
            number(&original, &["knowledge", "value", "attempt"]),
            number(&original, &["knowledge", "value", "policy_generation"]),
            number(&original, &["knowledge", "value", "congress_generation"]),
            number(&original, &["knowledge", "basis", "evaluation_revision"]),
            9000 + round,
            format!("{round:02x}").repeat(32),
        );
        let path = root.join(format!("assessment-{round}.json"));
        fs::write(&path, assessment).unwrap();
        let mut receipt = Vec::new();
        evaluator::client::command(
            &[
                "evaluator-assess".into(),
                profile.display().to_string(),
                path.display().to_string(),
            ],
            &mut receipt,
        )
        .unwrap();
        let applied = strict_json::parse(&receipt, KNOWLEDGE_JSON_LIMITS).unwrap();
        assert_eq!(field(&applied, &["status"]).as_str(), Some("applied"));
        for name in ["control_sequence", "authority_epoch"] {
            assert_eq!(
                field(&applied, &["before", name]),
                field(&applied, &["after", name])
            );
        }
        assert_eq!(
            number(&applied, &["after", "evaluation_revision"]),
            number(&applied, &["before", "evaluation_revision"]) + 1
        );
        assert_eq!(
            number(&applied, &["after", "journal_revision"]),
            number(&applied, &["before", "journal_revision"]) + 2
        );
        fs::write(root.join(format!("case-{round}.json")), case).unwrap();
        fs::write(root.join(format!("receipt-{round}.json")), receipt).unwrap();
    }
    wait_for(&root.join(format!("service-{request}.finished")));
    assert!(!selected.socket_path().exists());
}

struct Archives {
    history: PathBuf,
    source: PathBuf,
}

impl Archives {
    fn named(root: &Root, tag: &str) -> Self {
        Self {
            history: root.0.join(format!("{tag}.anchor")),
            source: root.0.join(format!("{tag}.source")),
        }
    }

    fn first(root: &Root) -> Self {
        Self {
            history: root.0.join("history-1.bin"),
            source: root.0.join("source-1.bin"),
        }
    }

    fn read(&self) -> TopologyCheckpoint {
        let saved = TopologyCheckpoint::read_trusted(
            &mut Cursor::new(fs::read(&self.source).unwrap()),
            MAX_TOPOLOGY_CHECKPOINT_BYTES,
        )
        .unwrap();
        assert_eq!(
            saved.history_anchor(),
            &archive::load(&self.history).unwrap()
        );
        saved
    }
}

fn next_source(root: &Root, generation: u64) {
    let text = fs::read_to_string(root.0.join("initial.json"))
        .unwrap()
        .replacen(
            "\"generation\":1",
            &format!("\"generation\":{generation}"),
            1,
        )
        .replacen(
            "\"inventory_generation\":1",
            &format!("\"inventory_generation\":{generation}"),
            1,
        )
        .replacen(
            "\"complete_inventory_generation\":1",
            &format!("\"complete_inventory_generation\":{generation}"),
            1,
        );
    fs::write(root.0.join(format!("live-{generation}.json")), text).unwrap();
    fs::copy(
        root.0.join("initial-inventory.json"),
        root.0.join(format!("inventory-{generation}.json")),
    )
    .unwrap();
}

fn topology_profile(
    root: &Root,
    input: &Archives,
    output: Option<&Archives>,
    generation: u64,
    tag: &str,
) -> PathBuf {
    let saved = input.read();
    let path = root.0.join(format!("topology-{tag}.json"));
    let retained = output.map_or_else(String::new, |output| {
        format!(r#","retain_checkpoint":"{}""#, output.source.display())
    });
    fs::write(
        &path,
        format!(
            r#"{{"schema_version":"fa.mediated-submit/2",
        "initial":{{"topology_path":"{}","inventory_path":"{}"}},
        "next":{{"topology_path":"{}","inventory_path":"{}"}},
        "operation":{},"source_checkpoint":"{}"{retained}}}"#,
            root.0.join("initial.json").display(),
            root.0.join("initial-inventory.json").display(),
            root.0.join(format!("live-{generation}.json")).display(),
            root.0
                .join(format!("inventory-{generation}.json"))
                .display(),
            saved.last_operation() + 1,
            input.source.display(),
        ),
    )
    .unwrap();
    path
}

fn accounting_for(root: &Root, minimum: u64, tag: &str) -> PathBuf {
    let path = root.0.join(format!("accounting-{tag}.json"));
    let content = fs::read_to_string(root.0.join("andon.json"))
        .unwrap()
        .replace(
            "\"minimum_evaluation_revision\": 0",
            &format!("\"minimum_evaluation_revision\": {minimum}"),
        );
    fs::write(&path, content).unwrap();
    path
}

struct Invocation<'a> {
    recipe: &'a Path,
    loaded: &'a Loaded,
    accounting: &'a Path,
    topology: &'a Path,
    input: Option<&'a Archives>,
    output: &'a Archives,
    mode: &'a [&'a str],
    round: u64,
    decision: ReviewDecision,
    evaluator_mode: &'a str,
}

fn arguments(
    root: &Root,
    config: &Config,
    actor: &Profile,
    peer: &Path,
    call: &Invocation<'_>,
) -> Vec<String> {
    let mut args = topology_fixture::command_inputs(root, config, call.recipe, actor);
    args.extend([
        "--andon".into(),
        call.accounting.display().to_string(),
        "--evaluator-profile".into(),
        peer.display().to_string(),
        "--topology-profile".into(),
        call.topology.display().to_string(),
        "--retain-history-anchor".into(),
        call.output.history.display().to_string(),
    ]);
    if let Some(input) = call.input {
        args.extend([
            "--history-anchor".into(),
            input.history.display().to_string(),
        ]);
    }
    args.extend(call.mode.iter().map(|value| (*value).to_owned()));
    args
}

struct Completed {
    snapshot: FileDeliverySnapshot,
    packet: ReviewPacket,
    evaluation_revision: u64,
}

fn run_command(root: &Root, config: &Config, call: Invocation<'_>) -> Completed {
    let request = call.loaded.request;
    let actor = peers::actor_profile(root, config, request);
    let reviewer = fixture::peers(root, config);
    let mut child = Independent::start(root, request, call.round, call.evaluator_mode);
    let peer = peer_file(root, &actor, child.pid());
    let args = arguments(root, config, &actor, &peer, &call);
    let due = plus(crate::workflow::clock(), 55_000).unwrap();
    let mut submitted = None;
    let mut approved = None;
    let stop = (call.evaluator_mode == "partial").then(|| {
        let ready = root.0.join("partial.ready");
        let reviewer = reviewer.clone();
        std::thread::spawn(move || {
            wait_for(&ready);
            let mut client = reviewer.connect_stop_client(request).unwrap();
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
    let result = command_with_clock(&args, None, || {
        if submitted.is_none() && actor.socket.exists() {
            let release = if call.mode.first() == Some(&"--finish-after-message") {
                LearnedTextRelease::Finish
            } else {
                LearnedTextRelease::Message
            };
            let peer = peers::actor(actor.clone(), release, due, Behavior::Observe);
            let marker = root.0.join(format!("actor-{request}.finished"));
            submitted = Some(std::thread::spawn(move || {
                let response = peer.join().unwrap();
                fs::write(marker, b"original actor terminal receipt").unwrap();
                response
            }));
        }
        if approved.is_none() && reviewer.socket(request).exists() {
            approved = Some(peers::human(reviewer.clone(), request, call.decision));
        }
        crate::workflow::clock()
    });
    fs::write(
        root.0.join(format!("service-{request}.finished")),
        b"original service retired",
    )
    .unwrap();
    child.finish();
    assert_eq!(
        result.is_ok(),
        call.decision == ReviewDecision::Approve,
        "{result:?}"
    );
    let response = submitted
        .expect("authenticated source-only actor")
        .join()
        .unwrap();
    let packet = approved
        .expect("independent native human review")
        .join()
        .unwrap();
    assert!(matches!(response.result, Ok(Knowledge::Known { value, .. })
        if value == if call.decision == ReviewDecision::Approve {
            ActorOutcome::Executed
        } else { ActorOutcome::CancelledBeforeDispatch }));
    assert_eq!(packet.views().len(), 2);
    let snapshot = FileOversight::read_publication_with_learned_generation(
        &config.store,
        &config.profile,
        &call.loaded.generation,
    )
    .unwrap();
    let saved = call.output.read();
    assert_eq!(saved.minimum().journal_revision, snapshot.revision);
    assert_eq!(saved.minimum().control_sequence, snapshot.control.sequence);
    assert_eq!(
        saved.minimum().authority_epoch,
        snapshot.control.ledger.epoch
    );
    let evaluation_revision = if let Some(stop) = stop {
        let receipt: StopControlReceipt = stop.join().unwrap();
        assert!(receipt.acknowledged() && receipt.drained());
        assert!(snapshot.stop.is_some());
        assert!(!saved.available());
        assert!(root.0.join("partial.closed").exists());
        assert!(!root.0.join(format!("receipt-{}.json", call.round)).exists());
        1
    } else {
        let receipt = read_json(&root.0.join(format!("receipt-{}.json", call.round)));
        assert!(
            saved.history_anchor().revision() >= number(&receipt, &["after", "journal_revision"])
        );
        assert!(saved.available());
        number(&receipt, &["after", "evaluation_revision"])
    };
    assert_eq!(
        saved.work().reserved_read_bytes,
        saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
    );
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(request).exists());
    assert!(
        !evaluator::Settings::read(&peer)
            .unwrap()
            .socket_path()
            .exists()
    );
    assert!(!crate::workflow::control::socket_path(&reviewer, request).exists());
    Completed {
        snapshot,
        packet,
        evaluation_revision,
    }
}

struct Original {
    config: Config,
    loaded: Loaded,
    pair: Archives,
    completed: Completed,
}

fn original(root: &Root, joint: bool, partial: bool) -> Original {
    let (config, recipe, loaded) = topology_fixture::selected(root, None, false);
    let pair = Archives::first(root);
    let accounting = if joint {
        joint_accounting_file(root)
    } else {
        accounting_file(root)
    };
    let completed = run_command(
        root,
        &config,
        Invocation {
            recipe: &recipe,
            loaded: &loaded,
            accounting: &accounting,
            topology: &root.0.join("create-topology.json"),
            input: None,
            output: &pair,
            mode: &[],
            round: 1,
            decision: ReviewDecision::Approve,
            evaluator_mode: if partial { "partial" } else { "assess" },
        },
    );
    assert_eq!(completed.snapshot.executions, 1);
    assert_eq!(completed.snapshot.payload, b"A");
    assert_eq!(
        completed.snapshot.control.ledger.charged,
        completed.packet.action().spec().units
    );
    assert!(pair.read().work().checks >= 2);
    Original {
        config,
        loaded,
        pair,
        completed,
    }
}


fn verified_history(
    root: &Root, config: &Config, loaded: &mut Loaded, input: &Archives, minimum: u64,
) -> fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::learned::FileMediatedAndonLearnedRecovery{
    use fa_reference::action::consequence::delivery::persistent::{
        RecoveryReserve, observed::decoder::learned::FileLearnedRecoveryStatus,
    };
    loaded.recovery_anchor = Some(archive::load(&input.history).unwrap());
    loaded.anchor_output = None;
    loaded.topology = None;
    let path = topology_profile(root, input, None, 99, "historical");
    custody::admit(config, loaded, Some(&path), custody::Mode::Historical).unwrap();
    let settings = andon_settings::Settings::read(&root.0.join("andon.json")).unwrap();
    let mut expected = FileAndonRecoveryRequirements::new(
        recovery::requirements(config, loaded),
        settings.config.clone(),
        settings.windows.clone(),
        minimum,
    )
    .unwrap();
    if let Some(joint) = settings.joint {
        expected = expected.require_joint(joint).unwrap();
    }
    let history = custody::history(loaded).unwrap().unwrap();
    let mut cursor =
        FileOversight::begin_open_mediated_andon_guarded_anchored_with_learned_generation(
            &config.store,
            config.profile.clone(),
            &expected,
            &history.requirement,
            &settings.protocol,
            &loaded.generation,
            loaded.recovery_anchor.as_ref().unwrap(),
        )
        .unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal())
        .unwrap();
    while cursor.progress().status == FileLearnedRecoveryStatus::Replaying {
        cursor
            .advance(cursor.progress().replayed_events, 1)
            .unwrap();
    }
    assert_eq!(cursor.progress().status, FileLearnedRecoveryStatus::Ready);
    cursor
}

#[test]
fn accounted_topology_restarts_preserve_source_work_numerics_and_fresh_finish_approval() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let original = original(&root, true, false);
        let initial = original.pair.read();
        let old_history = fs::read(&original.pair.history).unwrap();
        let old_source = fs::read(&original.pair.source).unwrap();
        evidence(&root, &original.config, 2, true, false);
        next_source(&root, 2);
        let (message_path, message) = native::next(&root, &original.config, 2);
        let second = Archives::named(&root, "message-2");
        let source = topology_profile(&root, &original.pair, Some(&second), 2, "message");
        let accounting = accounting_for(&root, original.completed.evaluation_revision, "message");
        let delivered = run_command(
            &root,
            &original.config,
            Invocation {
                recipe: &message_path,
                loaded: &message,
                accounting: &accounting,
                topology: &source,
                input: Some(&original.pair),
                output: &second,
                mode: &["--after-message", "1"],
                round: 23,
                decision: ReviewDecision::Approve,
                evaluator_mode: "assess",
            },
        );
        let before_finish = second.read();
        assert!(before_finish.work().checks >= initial.work().checks + 2);
        assert!(before_finish.last_operation() > initial.last_operation());
        assert_eq!(before_finish.expected().graph().spec().generation, 2);
        assert_eq!(delivered.snapshot.executions, 2);
        assert_eq!(delivered.snapshot.payload, b"AA");
        assert!(
            delivered.packet.action().spec().policy_epoch
                > original.completed.packet.action().spec().policy_epoch
        );
        assert_eq!(
            delivered.packet.action().spec().policy_epoch,
            delivered.snapshot.control.ledger.epoch
        );
        assert_eq!(
            delivered.snapshot.control.ledger.charged,
            original.completed.snapshot.control.ledger.charged
                + delivered.packet.action().spec().units
        );
        let frame = ReleaseFrame::decode(&delivered.packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), &["A"]);
        assert_eq!(frame.message(), Some("A"));
        let message_history = fs::read(&second.history).unwrap();
        let message_source = fs::read(&second.source).unwrap();

        evidence(&root, &original.config, 3, true, false);
        next_source(&root, 3);
        let (finish_path, mut finish) = native::next(&root, &original.config, 3);
        let final_pair = Archives::named(&root, "finish-3");
        let source = topology_profile(&root, &second, Some(&final_pair), 3, "finish");
        let accounting = accounting_for(&root, delivered.evaluation_revision, "finish");
        let completed = run_command(
            &root,
            &original.config,
            Invocation {
                recipe: &finish_path,
                loaded: &finish,
                accounting: &accounting,
                topology: &source,
                input: Some(&second),
                output: &final_pair,
                mode: &["--finish-after-message", "2"],
                round: 33,
                decision,
                evaluator_mode: "delete",
            },
        );
        let saved = final_pair.read();
        assert!(saved.work().checks >= before_finish.work().checks + 1);
        assert!(saved.last_operation() > before_finish.last_operation());
        assert_eq!(saved.expected().graph().spec().generation, 3);
        assert!(
            completed.packet.action().spec().policy_epoch
                > delivered.packet.action().spec().policy_epoch
        );
        assert_eq!(
            completed.packet.action().spec().policy_epoch,
            completed.snapshot.control.ledger.epoch
        );
        assert_ne!(
            completed.packet.binding().session,
            delivered.packet.binding().session
        );
        let frame = ReleaseFrame::decode(&completed.packet.action().spec().payload).unwrap();
        assert!(frame.is_finish());
        assert_eq!(frame.prior_messages(), &["A", "A"]);
        assert_eq!(
            completed.snapshot.executions,
            2 + u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(completed.snapshot.payload, b"AA");
        assert_eq!(
            completed.snapshot.control.ledger.charged,
            delivered.snapshot.control.ledger.charged
                + if decision == ReviewDecision::Approve {
                    completed.packet.action().spec().units
                } else {
                    0
                }
        );
        assert_eq!(completed.snapshot.control.ledger.reserved, 0);
        assert!(completed.snapshot.stop.is_none());
        for name in [
            "evidence.json",
            "model.json",
            "weights.safetensors",
            "native-roster.json",
        ] {
            assert!(
                !root.0.join(name).exists(),
                "late evaluation reopened {name}"
            );
        }
        assert_eq!(fs::read(&original.pair.history).unwrap(), old_history);
        assert_eq!(fs::read(&original.pair.source).unwrap(), old_source);
        assert_eq!(fs::read(&second.history).unwrap(), message_history);
        assert_eq!(fs::read(&second.source).unwrap(), message_source);

        // One final explicit guarded inspection restart reconstructs the same
        // original history; it cannot refill numerical/source work or old keys.
        let cursor = verified_history(
            &root,
            &original.config,
            &mut finish,
            &final_pair,
            completed.evaluation_revision,
        );
        let (host, roles) = cursor.finish().unwrap();
        assert!(roles.evaluator.is_some());
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
        assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
        let stream = host.stream_snapshot().unwrap();
        assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
        assert_eq!(
            stream.confirmed.finished(),
            decision == ReviewDecision::Approve
        );
        assert_eq!(stream.confirmed, stream.published);
        assert!(stream.pending.is_none());
        assert_eq!(
            host.inspect().control.ledger.charged,
            completed.snapshot.control.ledger.charged
        );
        assert!(host.inspect().control.ledger.epoch > saved.minimum().authority_epoch);
        assert_eq!(host.credibility_report().unwrap().benign_origins, 3);
        assert_eq!(
            host.credibility_report().unwrap().revision,
            completed.evaluation_revision
        );
        assert!(
            host.andon_ledger()
                .unwrap()
                .windows()
                .values()
                .all(|window| window.false_stops == 0
                    && window.total_cost == 0
                    && window.incident.is_none())
        );
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(
            host.joint_credibility_policy().unwrap(),
            andon_settings::Settings::read(&root.0.join("andon.json"))
                .unwrap()
                .joint
        );
    }
}

#[test]
fn independently_changed_accounting_joint_topology_source_and_floors_refuse_before_fence() {
    let root = Root::new();
    let original = original(&root, true, false);
    next_source(&root, 2);
    let (message_path, message) = native::next(&root, &original.config, 2);
    let second = Archives::named(&root, "message-2");
    let source = topology_profile(&root, &original.pair, Some(&second), 2, "message");
    let accounting = accounting_for(&root, original.completed.evaluation_revision, "message");
    let delivered = run_command(
        &root,
        &original.config,
        Invocation {
            recipe: &message_path,
            loaded: &message,
            accounting: &accounting,
            topology: &source,
            input: Some(&original.pair),
            output: &second,
            mode: &["--after-message", "1"],
            round: 23,
            decision: ReviewDecision::Approve,
            evaluator_mode: "assess",
        },
    );
    next_source(&root, 3);
    let (finish_path, finish) = native::next(&root, &original.config, 3);
    let canonical = fs::read(original.config.store.join("delivery.bin")).unwrap();
    let staging = original.config.store.join("delivery.pending");
    fs::write(&staging, b"unacknowledged original staged evidence").unwrap();
    let initial_topology = fs::read_to_string(root.0.join("initial.json")).unwrap();
    let original_recipe = fs::read_to_string(&finish_path).unwrap();
    let history_bytes = fs::read(&second.history).unwrap();
    let source_bytes = fs::read(&second.source).unwrap();
    for fault in ["accounting", "joint", "topology", "source", "floor"] {
        let output = Archives::named(&root, &format!("refused-{fault}"));
        let source = topology_profile(&root, &second, Some(&output), 3, fault);
        let accounting = accounting_for(&root, delivered.evaluation_revision, fault);
        let actor = peers::actor_profile(&root, &original.config, 3);
        let peer = peer_file(
            &root,
            &actor,
            std::process::id().checked_add(100_000).unwrap(),
        );
        match fault {
            "accounting" => {
                let before = fs::read_to_string(&accounting).unwrap();
                let after = before.replace("\"cost_per_stop\": 7", "\"cost_per_stop\": 8");
                assert_ne!(before, after);
                fs::write(&accounting, after).unwrap();
            }
            "joint" => {
                let before = fs::read_to_string(&accounting).unwrap();
                let after = before.replace("\"id\": 71", "\"id\": 72");
                assert_ne!(before, after);
                fs::write(&accounting, after).unwrap();
            }
            "topology" => {
                fs::write(
                    root.0.join("initial.json"),
                    initial_topology.replacen("\"generation\":1", "\"generation\":2", 1),
                )
                .unwrap();
            }
            "source" => {
                let before = fs::read_to_string(&source).unwrap();
                let after = before.replace(
                    second.source.to_str().unwrap(),
                    original.pair.source.to_str().unwrap(),
                );
                assert_ne!(before, after);
                fs::write(&source, after).unwrap();
            }
            "floor" => {
                let changed = original_recipe.replacen(
                    "\"journal_revision\":0",
                    &format!("\"journal_revision\":{}", delivered.snapshot.revision + 1),
                    1,
                );
                assert_ne!(changed, original_recipe);
                fs::write(&finish_path, changed).unwrap();
            }
            _ => unreachable!(),
        }
        let args = arguments(
            &root,
            &original.config,
            &actor,
            &peer,
            &Invocation {
                recipe: &finish_path,
                loaded: &finish,
                accounting: &accounting,
                topology: &source,
                input: Some(&second),
                output: &output,
                mode: &["--finish-after-message", "2"],
                round: 33,
                decision: ReviewDecision::Approve,
                evaluator_mode: "assess",
            },
        );
        let error = command_with_clock(&args, None, crate::workflow::clock).unwrap_err();
        assert!(!error.is_empty());
        assert_eq!(
            fs::read(original.config.store.join("delivery.bin")).unwrap(),
            canonical,
            "{fault}"
        );
        assert_eq!(
            fs::read(&staging).unwrap(),
            b"unacknowledged original staged evidence"
        );
        assert_eq!(fs::read(&second.history).unwrap(), history_bytes);
        assert_eq!(fs::read(&second.source).unwrap(), source_bytes);
        assert!(!output.history.exists() && !output.source.exists());
        assert!(!actor.socket.exists());
        assert!(
            !evaluator::Settings::read(&peer)
                .unwrap()
                .socket_path()
                .exists()
        );
        fs::write(root.0.join("initial.json"), &initial_topology).unwrap();
        fs::write(&finish_path, &original_recipe).unwrap();
    }
}

#[test]
fn missing_next_source_retains_the_acknowledged_accounted_fence_and_can_resume_from_that_pair() {
    let root = Root::new();
    // The marginal mediated constructor shares exactly the same source law.
    let original = original(&root, false, false);
    let before = &original.completed.snapshot;
    let old = original.pair.read();
    let original_history = fs::read(&original.pair.history).unwrap();
    let original_source = fs::read(&original.pair.source).unwrap();
    let (message_path, message) = native::next(&root, &original.config, 2);
    let failed = Archives::named(&root, "failed-startup");
    let source = topology_profile(&root, &original.pair, Some(&failed), 2, "missing-next");
    let accounting = accounting_for(
        &root,
        original.completed.evaluation_revision,
        "missing-next",
    );
    let actor = peers::actor_profile(&root, &original.config, 2);
    let peer = peer_file(
        &root,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let args = arguments(
        &root,
        &original.config,
        &actor,
        &peer,
        &Invocation {
            recipe: &message_path,
            loaded: &message,
            accounting: &accounting,
            topology: &source,
            input: Some(&original.pair),
            output: &failed,
            mode: &["--after-message", "1"],
            round: 23,
            decision: ReviewDecision::Approve,
            evaluator_mode: "assess",
        },
    );
    let error = command_with_clock(&args, None, crate::workflow::clock).unwrap_err();
    assert!(error.contains("NotFound"), "{error}");
    let after = FileOversight::read_publication_with_learned_generation(
        &original.config.store,
        &original.config.profile,
        &message.generation,
    )
    .unwrap();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 2);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(
        after.control.ledger.reserved,
        before.control.ledger.reserved
    );
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert!(after.stop.is_none());
    let retained = failed.read();
    assert_eq!(retained.history_anchor().revision(), after.revision);
    assert_eq!(
        retained.minimum().authority_epoch,
        after.control.ledger.epoch
    );
    assert_eq!(retained.work(), old.work());
    assert_eq!(retained.last_operation(), old.last_operation());
    assert_eq!(retained.expected(), old.expected());
    assert!(!retained.available());
    assert_eq!(retained.topology_bytes(), old.topology_bytes());
    assert_eq!(retained.inventory_bytes(), old.inventory_bytes());
    assert_eq!(fs::read(&original.pair.history).unwrap(), original_history);
    assert_eq!(fs::read(&original.pair.source).unwrap(), original_source);
    assert!(!actor.socket.exists());

    // The old exact pair cannot describe the acknowledged successor. Refuse
    // before another Fence, even after its missing live source becomes available.
    next_source(&root, 2);
    let stale = Archives::named(&root, "stale-startup");
    let source = topology_profile(&root, &original.pair, Some(&stale), 2, "stale");
    let canonical = fs::read(original.config.store.join("delivery.bin")).unwrap();
    let args = arguments(
        &root,
        &original.config,
        &actor,
        &peer,
        &Invocation {
            recipe: &message_path,
            loaded: &message,
            accounting: &accounting,
            topology: &source,
            input: Some(&original.pair),
            output: &stale,
            mode: &["--after-message", "1"],
            round: 23,
            decision: ReviewDecision::Approve,
            evaluator_mode: "assess",
        },
    );
    assert!(command_with_clock(&args, None, crate::workflow::clock).is_err());
    assert_eq!(
        fs::read(original.config.store.join("delivery.bin")).unwrap(),
        canonical
    );
    assert!(!stale.history.exists() && !stale.source.exists());

    let corrected = Archives::named(&root, "corrected-message");
    let source = topology_profile(&root, &failed, Some(&corrected), 2, "corrected");
    let completed = run_command(
        &root,
        &original.config,
        Invocation {
            recipe: &message_path,
            loaded: &message,
            accounting: &accounting,
            topology: &source,
            input: Some(&failed),
            output: &corrected,
            mode: &["--after-message", "1"],
            round: 23,
            decision: ReviewDecision::Approve,
            evaluator_mode: "assess",
        },
    );
    assert_eq!(completed.snapshot.executions, 2);
    assert_eq!(completed.snapshot.payload, b"AA");
    assert_eq!(
        completed.snapshot.control.ledger.charged,
        before.control.ledger.charged + completed.packet.action().spec().units
    );
    assert!(completed.packet.action().spec().policy_epoch > retained.minimum().authority_epoch);
    let current = corrected.read();
    assert!(current.work().checks >= retained.work().checks + 2);
    assert!(current.last_operation() > retained.last_operation());
    assert_eq!(current.expected().graph().spec().generation, 2);
    assert_eq!(fs::read(&original.pair.history).unwrap(), original_history);
    assert_eq!(fs::read(&original.pair.source).unwrap(), original_source);
}

#[test]
fn original_stop_retires_partial_late_evaluation_and_preserves_the_exact_source_history_pair() {
    let root = Root::new();
    let mut original = original(&root, true, true);
    let before = fs::read(original.config.store.join("delivery.bin")).unwrap();
    let saved = original.pair.read();
    assert!(!saved.available());
    assert!(original.completed.snapshot.stop.is_some());
    assert_eq!(original.completed.snapshot.executions, 1);
    assert_eq!(original.completed.snapshot.payload, b"A");
    assert_eq!(
        original.completed.snapshot.control.ledger.charged,
        original.completed.packet.action().spec().units
    );
    assert_eq!(original.completed.snapshot.control.ledger.reserved, 0);
    assert!(saved.work().checks >= 2);
    assert!(!root.0.join("assessment-1.json").exists());
    let checked = verified_history(
        &root,
        &original.config,
        &mut original.loaded,
        &original.pair,
        1,
    );
    drop(checked);
    assert_eq!(
        fs::read(original.config.store.join("delivery.bin")).unwrap(),
        before
    );
    assert_eq!(original.pair.read(), saved);
}

#[test]
fn evaluator_bind_failure_retains_only_an_acknowledged_creation_cut() {
    let root = Root::new();
    let (config, recipe, loaded) = topology_fixture::selected(&root, None, false);
    let pair = Archives::first(&root);
    let accounting = accounting_file(&root);
    let actor = peers::actor_profile(&root, &config, loaded.request);
    let peer = peer_file(
        &root,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let endpoint = root.0.join("evaluator.sock");
    let create_profile = root.0.join("create-topology.json");
    let call = Invocation {
        recipe: &recipe,
        loaded: &loaded,
        accounting: &accounting,
        topology: &create_profile,
        input: None,
        output: &pair,
        mode: &[],
        round: 1,
        decision: ReviewDecision::Approve,
        evaluator_mode: "assess",
    };
    let args = arguments(&root, &config, &actor, &peer, &call);

    // A conflict present during preflight cannot manufacture an initial cut.
    fs::write(&endpoint, b"other owner").unwrap();
    let error = command_with_clock(&args, None, crate::workflow::clock).unwrap_err();
    assert!(
        error.contains("evaluator socket path already exists"),
        "{error}"
    );
    assert!(!config.store.join("delivery.bin").exists());
    assert!(!pair.history.exists() && !pair.source.exists());
    fs::remove_file(&endpoint).unwrap();

    // Occupy the same path after preflight, during topology document intake.
    // The created owner and its original source already exist at evaluator bind.
    let mut occupied = false;
    let error = command_with_clock(&args, None, || {
        if !occupied && actor.socket.exists() {
            fs::write(&endpoint, b"other owner").unwrap();
            occupied = true;
        }
        crate::workflow::clock()
    })
    .unwrap_err();
    assert!(
        occupied && error.contains("evaluator socket path already exists"),
        "{error}"
    );
    assert_eq!(fs::read(&endpoint).unwrap(), b"other owner");
    assert!(!actor.socket.exists());
    let saved = pair.read();
    let snapshot = FileOversight::read_publication_with_learned_generation(
        &config.store,
        &config.profile,
        &loaded.generation,
    )
    .unwrap();
    assert_eq!(saved.minimum().journal_revision, snapshot.revision);
    assert_eq!(
        saved.minimum().authority_epoch,
        snapshot.control.ledger.epoch
    );
    assert_eq!(snapshot.executions, 0);
    assert_eq!(snapshot.control.ledger.charged, 0);
    assert!(snapshot.stop.is_none());

    // The original cut supports genuine anchored continuation. A fresh actor,
    // independent evaluator and human each participate in its first release.
    fs::remove_file(&endpoint).unwrap();
    next_source(&root, 2);
    let resumed = Archives::named(&root, "first-release");
    let source = topology_profile(&root, &pair, Some(&resumed), 2, "resume-first");
    let delivered = run_command(
        &root,
        &config,
        Invocation {
            topology: &source,
            input: Some(&pair),
            output: &resumed,
            mode: &["--continue-generation"],
            ..call
        },
    );
    assert_eq!(delivered.snapshot.executions, 1);
    assert_eq!(delivered.snapshot.payload, b"A");
    assert!(resumed.read().last_operation() > saved.last_operation());
}

#[test]
fn evaluated_mediated_message_keeps_the_original_prompt_checkpoint_after_independent_review() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let config_path = super::super::functional_creation::config_file(&root, "functional_restart");
        let config = Config::read_learned_generation(&config_path).unwrap();
        let recipe_path = native::write(&root);
        evidence(&root, &config, 1, true, false);
        let topology = topology_fixture::creation_profile(&root, false);
        let mut loaded = recipe::load(&recipe_path, &config, true).unwrap();
        let prompt = super::super::checkpoint_on_generation::selected_file(&root, 71, 2, 0);
        let accounting = joint_accounting_file(&root);
        let pair = Archives::first(&root);
        let outcome = run_command(&root, &config, Invocation {
            recipe: &recipe_path,
            loaded: &loaded,
            accounting: &accounting,
            topology: &topology,
            input: None,
            output: &pair,
            mode: &["--checkpoint-before-generation", prompt.to_str().unwrap()],
            round: 1,
            decision,
            evaluator_mode: "assess",
        });
        assert_eq!(outcome.snapshot.executions, u64::from(decision == ReviewDecision::Approve));
        assert_eq!(outcome.evaluation_revision, 2);
        let cursor = verified_history(&root, &config, &mut loaded, &pair, outcome.evaluation_revision);
        let (host, _) = cursor.finish().unwrap();
        let saved = host.learned_stream_checkpoint(71).unwrap();
        assert_eq!((saved.info().position, saved.info().sampled_draws), (2, 0));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        assert_eq!(host.inspect().control.ledger.charged, outcome.snapshot.control.ledger.charged);
        assert_eq!(host.credibility_report().unwrap().revision, outcome.evaluation_revision);
    }
}

#[test]
fn evaluated_audit_only_owner_refuses_checkpoint_capture_before_time_or_storage() {
    let root = Root::new();
    let (config, recipe, loaded) = topology_fixture::selected(&root, None, false);
    let actor = peers::actor_profile(&root, &config, 1);
    let peer = peer_file(&root, &actor, std::process::id().checked_add(1).unwrap());
    let accounting = accounting_file(&root);
    let output = Archives::first(&root);
    let selected = super::super::checkpoint_on_generation::selected_file(&root, 71, 2, 0);
    let args = arguments(&root, &config, &actor, &peer, &Invocation {
        recipe: &recipe,
        loaded: &loaded,
        accounting: &accounting,
        topology: &root.0.join("create-topology.json"),
        input: None,
        output: &output,
        mode: &["--checkpoint-before-generation", selected.to_str().unwrap()],
        round: 1,
        decision: ReviewDecision::Approve,
        evaluator_mode: "assess",
    });
    let err = command_with_clock(&args, None, || panic!("rejected checkpoint acquired a clock"))
        .unwrap_err();
    assert!(err.contains("functional_restart"), "{err}");
    assert!(!config.store.exists());
    assert!(!output.history.exists() && !output.source.exists());
    assert!(!actor.socket.exists());
}
