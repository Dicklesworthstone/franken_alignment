//! Actual restarted accounted generation, Message/Finish and late independent
//! labels. Original native fixtures test authority/custody, not detector quality.
use super::super::continuation as native;
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot,
    observed::{
        guarded::evaluation::andon::FileAndonRecoveryRequirements, reviewer::wire::ReviewPacket,
    },
};
use fa_reference::round::review::andon::AndonBudget;
use std::collections::BTreeMap;

const RECOVERY_CHILD: &str =
    "workflow::actor_service::learned::tests::evaluation_tests::recovery_tests::evaluator_process";
const RECOVERY_ROOT: &str = "FA_ACCOUNTED_LEARNED_RECOVERY_ROOT";
const RECOVERY_REQUEST: &str = "FA_ACCOUNTED_LEARNED_RECOVERY_REQUEST";
const RECOVERY_ROUNDS: &str = "FA_ACCOUNTED_LEARNED_RECOVERY_ROUNDS";
const RECOVERY_DELETE: &str = "FA_ACCOUNTED_LEARNED_RECOVERY_DELETE";

struct RecoveryEvaluator(Child);
impl RecoveryEvaluator {
    fn start(root: &Root, request: u64, rounds: &[u64], delete_inputs: bool) -> Self {
        Self(
            ProcessCommand::new(std::env::current_exe().unwrap())
                .args(["--exact", RECOVERY_CHILD, "--nocapture"])
                .env(RECOVERY_ROOT, &root.0)
                .env(RECOVERY_REQUEST, request.to_string())
                .env(
                    RECOVERY_ROUNDS,
                    rounds
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .env(RECOVERY_DELETE, if delete_inputs { "yes" } else { "no" })
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
                assert!(
                    status.success(),
                    "recovered independent evaluator failed: {status}"
                );
                return;
            }
            assert!(started.elapsed() < Duration::from_secs(30));
            crate::workflow::pause(1);
        }
    }
}
impl Drop for RecoveryEvaluator {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn label(root: &Path, round: u64) {
    let profile = root.join("evaluator.json");
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
    let case = strict_json::parse(&observation, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(
        field(&case, &["knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(number(&case, &["knowledge", "value", "round"]), round);
    let evidence = field(&case, &["knowledge", "value", "case_evidence_root_hex"])
        .as_str()
        .unwrap();
    let assessment = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{round},"attempt":{},
        "policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{evidence}",
        "expected_evaluation_revision":{},"origin":{},"evidence_id_hex":"{}","truth":"benign"}}"#,
        number(&case, &["knowledge", "value", "attempt"]),
        number(&case, &["knowledge", "value", "policy_generation"]),
        number(&case, &["knowledge", "value", "congress_generation"]),
        number(&case, &["knowledge", "basis", "evaluation_revision"]),
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
    for coordinate in ["control_sequence", "authority_epoch"] {
        assert_eq!(
            field(&applied, &["before", coordinate]),
            field(&applied, &["after", coordinate])
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
    fs::write(root.join(format!("case-{round}.json")), observation).unwrap();
    fs::write(root.join(format!("receipt-{round}.json")), receipt).unwrap();
}

#[test]
fn evaluator_process() {
    let Ok(root) = std::env::var(RECOVERY_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    let request = std::env::var(RECOVERY_REQUEST).unwrap();
    wait_for(&root.join(format!("actor-{request}.finished")));
    let settings = evaluator::Settings::read(&root.join("evaluator.json")).unwrap();
    assert_eq!(settings.evaluator.pid, Some(std::process::id()));
    assert_ne!(settings.supervisor.pid, Some(std::process::id()));
    if std::env::var(RECOVERY_DELETE).unwrap() == "yes" {
        for name in [
            "evidence.json",
            "model.json",
            "weights.safetensors",
            "native-roster.json",
        ] {
            fs::remove_file(root.join(name)).unwrap();
        }
    }
    for round in std::env::var(RECOVERY_ROUNDS).unwrap().split(',') {
        label(&root, round.parse().unwrap());
    }
    wait_for(&root.join(format!("service-{request}.finished")));
    assert!(UnixStream::connect(settings.socket_path()).is_err());
}

struct Original {
    config: Config,
    loaded: Loaded,
    accounting: PathBuf,
    anchor: PathBuf,
    before: FileDeliverySnapshot,
    pre_generation: Vec<u8>,
    evaluation_revision: u64,
}

fn prepared(root: &Root, published: bool) -> Original {
    let mut config = configured(root);
    let path = native::write(root);
    evidence(root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    let actor = peers::actor_profile(root, &config, 1);
    let (selection, accounting) = super::selected(
        root,
        &config,
        &mut loaded,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let ((mut host, reviewer, observer, identity), evaluator) =
        selection.create(&config, &loaded).unwrap();
    assert!(observer.is_none() && identity.is_none());
    let revision = host.revision();
    host.register_andon_window(
        revision,
        AndonBudget {
            window: 2,
            start: 100,
            end: 200,
            false_stops_allowed: 2,
            cost_per_stop: 9,
        },
    )
    .unwrap();
    let pre_generation = fs::read(config.store.join("delivery.bin")).unwrap();
    drop(evaluator);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let limit = deadline();
    if published {
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        assert!(
            generate(
                &mut driver,
                &reviewer,
                &mut config,
                &mut control,
                &limit,
                &mut || ElapsedTick(1000)
            )
            .unwrap()
        );
        submit(
            &mut driver,
            &mut wire,
            &mut config,
            &loaded,
            &limit,
            &mut || ElapsedTick(1000),
        )
        .unwrap();
        let (reviewed, history) =
            native::review_current(&mut config, &loaded, &reviewer, driver, &limit);
        driver = reviewed;
        let permit = native::approve(&mut driver, &mut config, &reviewer, 1);
        native::publish(&mut driver, &mut config, &permit);
        assert!(executed(&mut wire, 1));
        drop(history);
    } else {
        // Save one ORIGINAL sampled output, before its stop token or request.
        // Recovery must continue this finite generation instead of resampling A.
        refresh(&mut driver, &mut config, &limit, &mut || ElapsedTick(1000)).unwrap();
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        for _ in 0..8 {
            let previous = host.learned_generation_inspection().unwrap().numerical;
            if previous.sampled_draws == 1 {
                break;
            }
            let revision = host.revision();
            host.advance_learned_generation(revision, previous.actor_revision, previous.position)
                .unwrap()
                .unwrap();
        }
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!((numerical.position, numerical.sampled_draws), (3, 1));
        assert!(numerical.status.is_active());
    }
    let anchor = root.0.join("original.anchor");
    let (before, evaluation_revision) = {
        let host = driver.supervisor().host().unwrap();
        archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
        assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
        (host.inspect(), host.credibility_report().unwrap().revision)
    };
    assert_eq!(evaluation_revision, u64::from(published));
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    Original {
        config,
        loaded,
        accounting,
        anchor,
        before,
        pre_generation,
        evaluation_revision,
    }
}

fn accounting_for(original: &Original, root: &Root, minimum: u64, tag: &str) -> PathBuf {
    let initial = fs::read_to_string(&original.accounting).unwrap();
    let current = initial
        .replace("\"minimum_evaluation_revision\": 0", &format!("\"minimum_evaluation_revision\": {minimum}"))
        .replace("\"current_windows\": [", "\"current_windows\": [\n    {\"window\":2,\"start\":100,\"end\":200,\"false_stops_allowed\":2,\"cost_per_stop\":9},");
    assert_ne!(current, initial);
    let path = root.0.join(format!("recovery-andon-{tag}.json"));
    fs::write(&path, current).unwrap();
    path
}

fn arguments(
    root: &Root,
    original: &Original,
    recipe: &Path,
    actor: &Profile,
    accounting: &Path,
    peer: &Path,
    anchor: &Path,
    output: &Path,
    mode: &[&str],
) -> Vec<String> {
    let mut args =
        super::super::mediated_topology::command_inputs(root, &original.config, recipe, actor);
    args.extend([
        "--andon".into(),
        accounting.display().to_string(),
        "--evaluator-profile".into(),
        peer.display().to_string(),
        "--history-anchor".into(),
        anchor.display().to_string(),
        "--retain-history-anchor".into(),
        output.display().to_string(),
    ]);
    args.extend(mode.iter().map(|value| (*value).to_owned()));
    args
}

fn independent_requirements(
    config: &Config,
    loaded: &Loaded,
    settings: &andon_settings::Settings,
    minimum: u64,
) -> FileAndonRecoveryRequirements {
    FileAndonRecoveryRequirements::new(
        recovery::requirements(config, loaded),
        settings.config.clone(),
        settings.windows.clone(),
        minimum,
    )
    .unwrap()
}

struct Invocation<'a> {
    recipe: &'a Path,
    loaded: &'a Loaded,
    anchor: &'a Path,
    mode: &'a [&'a str],
    minimum: u64,
    labels: &'a [u64],
    decision: ReviewDecision,
    delete_inputs: bool,
}
struct Completed {
    snapshot: FileDeliverySnapshot,
    packet: ReviewPacket,
    anchor: PathBuf,
    accounting: andon_settings::Settings,
    evaluation_revision: u64,
}

fn run_command(root: &Root, original: &Original, call: Invocation<'_>) -> Completed {
    let request = call.loaded.request;
    let actor = peers::actor_profile(root, &original.config, request);
    let reviewer = fixture::peers(root, &original.config);
    let mut child = RecoveryEvaluator::start(root, request, call.labels, call.delete_inputs);
    let peer = peer_file(root, &actor, child.0.id());
    let text = fs::read_to_string(&peer).unwrap();
    fs::write(
        &peer,
        text.replace(
            "\"candidate_limit\":2",
            &format!("\"candidate_limit\":{}", call.labels.len() * 2),
        )
        .replace("\"runtime_ms\":15000", "\"runtime_ms\":45000"),
    )
    .unwrap();
    let accounting = accounting_for(original, root, call.minimum, &request.to_string());
    let output = root.0.join(format!("recovered-{request}.anchor"));
    let args = arguments(
        root,
        original,
        call.recipe,
        &actor,
        &accounting,
        &peer,
        call.anchor,
        &output,
        call.mode,
    );
    let release = if call.mode.first() == Some(&"--finish-after-message") {
        LearnedTextRelease::Finish
    } else {
        LearnedTextRelease::Message
    };
    let due = plus(crate::workflow::clock(), 55_000).unwrap();
    let mut submitted = None;
    let mut approved = None;
    let result = command_with_clock(&args, None, || {
        if submitted.is_none() && actor.socket.exists() {
            let peer = peers::actor(actor.clone(), release, due, Behavior::Observe);
            let marker = root.0.join(format!("actor-{request}.finished"));
            submitted = Some(std::thread::spawn(move || {
                let response = peer.join().unwrap();
                fs::write(marker, b"original recovered request finished").unwrap();
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
    if call.decision == ReviewDecision::Approve {
        assert!(result.is_ok(), "{result:?}");
    } else {
        assert_eq!(
            result.err().as_deref(),
            Some("learned publication is not confirmed; inspect the original response")
        );
    }
    let response = submitted
        .expect("fresh authenticated actor")
        .join()
        .unwrap();
    let packet = approved
        .expect("fresh independent human review")
        .join()
        .unwrap();
    assert!(matches!(response.result, Ok(Knowledge::Known { value, .. })
        if value == if call.decision == ReviewDecision::Approve {
            ActorOutcome::Executed
        } else { ActorOutcome::CancelledBeforeDispatch }));
    assert_eq!(packet.views().len(), 2);
    let snapshot = FileOversight::read_publication_with_learned_generation(
        &original.config.store,
        &original.config.profile,
        &call.loaded.generation,
    )
    .unwrap();
    let anchor = archive::load(&output).unwrap();
    assert_eq!(snapshot.revision, anchor.revision());
    let receipt = read_json(
        &root
            .0
            .join(format!("receipt-{}.json", call.labels.last().unwrap())),
    );
    assert!(anchor.revision() >= number(&receipt, &["after", "journal_revision"]));
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(request).exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, request).exists());
    assert!(
        !evaluator::Settings::read(&peer)
            .unwrap()
            .socket_path()
            .exists()
    );
    Completed {
        snapshot,
        packet,
        anchor: output,
        accounting: andon_settings::Settings::read(&accounting).unwrap(),
        evaluation_revision: number(&receipt, &["after", "evaluation_revision"]),
    }
}

fn reopened(original: &Original, loaded: &Loaded, completed: &Completed) -> FileOversight {
    // A second explicit restart uses ALL independently retained current
    // requirements and the acknowledged anchor; no evaluated-only fallback.
    let expected = independent_requirements(
        &original.config,
        loaded,
        &completed.accounting,
        completed.evaluation_revision,
    );
    let (host, roles) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
        &original.config.store,
        original.config.profile.clone(),
        &expected,
        &completed.accounting.protocol,
        &loaded.generation,
        &archive::load(&completed.anchor).unwrap(),
    )
    .unwrap();
    assert_eq!(host.revision(), completed.snapshot.revision + 1);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert_eq!(
        host.credibility_report().unwrap().revision,
        completed.evaluation_revision
    );
    assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
    assert_eq!(host.inspect().executions, completed.snapshot.executions);
    assert_eq!(host.inspect().payload, completed.snapshot.payload);
    drop(roles);
    host
}

#[test]
fn explicit_continue_generation_retains_the_original_sample_before_fresh_review_and_label() {
    let root = Root::new();
    let original = prepared(&root, false);
    let old_archive = fs::read(&original.anchor).unwrap();
    let completed = run_command(
        &root,
        &original,
        Invocation {
            recipe: &root.0.join("recipe.json"),
            loaded: &original.loaded,
            anchor: &original.anchor,
            mode: &["--continue-generation"],
            minimum: 0,
            labels: &[1],
            decision: ReviewDecision::Approve,
            delete_inputs: true,
        },
    );
    assert_eq!(completed.snapshot.executions, 1);
    assert_eq!(completed.snapshot.payload, b"A");
    assert_eq!(
        completed.snapshot.control.ledger.epoch,
        original.before.control.ledger.epoch + 1
    );
    let host = reopened(&original, &original.loaded, &completed);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (4, 2));
    assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
    assert_eq!(host.credibility_report().unwrap().benign_origins, 1);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
    for name in [
        "evidence.json",
        "model.json",
        "weights.safetensors",
        "native-roster.json",
    ] {
        assert!(
            !root.0.join(name).exists(),
            "late label or recovery reread {name}"
        );
    }
}

fn message_then_finish(decision: ReviewDecision) {
    let root = Root::new();
    let original = prepared(&root, true);
    let old_archive = fs::read(&original.anchor).unwrap();
    evidence(&root, &original.config, 2, true, false);
    let (message_path, message) = native::next(&root, &original.config, 2);
    let delivered = run_command(
        &root,
        &original,
        Invocation {
            recipe: &message_path,
            loaded: &message,
            anchor: &original.anchor,
            mode: &["--after-message", "1"],
            minimum: original.evaluation_revision,
            labels: &[1, 23],
            decision: ReviewDecision::Approve,
            delete_inputs: false,
        },
    );
    let frame = ReleaseFrame::decode(&delivered.packet.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]);
    assert_eq!(frame.message(), Some("A"));
    assert_eq!(delivered.snapshot.executions, 2);
    assert_eq!(delivered.snapshot.payload, b"AA");
    assert_eq!(
        delivered.snapshot.control.ledger.epoch,
        original.before.control.ledger.epoch + 1
    );
    assert_eq!(
        delivered.snapshot.control.ledger.charged,
        original.before.control.ledger.charged + delivered.packet.action().spec().units
    );
    let message_archive = fs::read(&delivered.anchor).unwrap();
    let (finish_path, finish) = native::next(&root, &original.config, 3);
    let completed = run_command(
        &root,
        &original,
        Invocation {
            recipe: &finish_path,
            loaded: &finish,
            anchor: &delivered.anchor,
            mode: &["--finish-after-message", "2"],
            minimum: delivered.evaluation_revision,
            labels: &[33],
            decision,
            delete_inputs: true,
        },
    );
    let frame = ReleaseFrame::decode(&completed.packet.action().spec().payload).unwrap();
    assert!(frame.is_finish());
    assert_eq!(frame.prior_messages(), &["A", "A"]);
    assert_ne!(
        completed.packet.binding().session,
        delivered.packet.binding().session
    );
    assert_eq!(
        completed.snapshot.executions,
        if decision == ReviewDecision::Approve {
            3
        } else {
            2
        }
    );
    assert_eq!(completed.snapshot.payload, b"AA");
    assert_eq!(
        completed.snapshot.control.ledger.epoch,
        original.before.control.ledger.epoch + 2
    );
    assert_eq!(completed.snapshot.control.ledger.reserved, 0);
    assert_eq!(
        completed.snapshot.control.ledger.charged,
        delivered.snapshot.control.ledger.charged
            + if decision == ReviewDecision::Approve {
                completed.packet.action().spec().units
            } else {
                0
            }
    );
    let host = reopened(&original, &finish, &completed);
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
    assert_eq!(stream.pending, None);
    assert_eq!(host.credibility_report().unwrap().benign_origins, 3);
    assert!(
        host.andon_ledger()
            .unwrap()
            .windows()
            .values()
            .all(|window| window.false_stops == 0 && window.total_cost == 0)
    );
    assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
    assert_eq!(fs::read(&delivered.anchor).unwrap(), message_archive);
}

#[test]
fn accounted_message_and_finish_use_new_review_keys_and_preserve_pending_old_case_labels() {
    message_then_finish(ReviewDecision::Approve);
}

#[test]
fn recovered_finish_cannot_reuse_a_message_approval_but_its_case_remains_assessable() {
    message_then_finish(ReviewDecision::Reject);
}

fn files(store: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(store)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn live_selection(
    root: &Root,
    original: &Original,
    loaded: &mut Loaded,
    accounting: &Path,
) -> (evaluation::Selection, PeerProfile) {
    let actor = peers::actor_profile(root, &original.config, loaded.request);
    let peer = peer_file(
        root,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let output = root.0.join("live-recovery.anchor");
    let args = arguments(
        root,
        original,
        &root.0.join("recipe.json"),
        &actor,
        accounting,
        &peer,
        &original.anchor,
        &output,
        &["--continue-generation"],
    );
    let (remaining, selection) = evaluation::take_options(&args, None).unwrap();
    let selection = selection.unwrap().read().unwrap();
    let (_, history) = archive::take_options(&remaining).unwrap();
    history.admit(&original.config, loaded, true).unwrap();
    let reviewer = fixture::peers(root, &original.config);
    selection
        .check(
            &original.config,
            loaded,
            Some(&actor),
            &reviewer,
            (None, None),
            true,
            None,
            None,
            None,
        )
        .unwrap();
    (selection, reviewer)
}

#[test]
fn recovery_requires_the_anchor_complete_window_inventory_and_independent_evaluation_floor() {
    for mismatch in 0..4 {
        let root = Root::new();
        let original = prepared(&root, false);
        let accounting = accounting_for(&original, &root, u64::from(mismatch == 2), "negative");
        if mismatch == 1 {
            let raw = fs::read_to_string(&accounting).unwrap();
            fs::write(&accounting, raw.replace(
                "\n    {\"window\":2,\"start\":100,\"end\":200,\"false_stops_allowed\":2,\"cost_per_stop\":9},", "")).unwrap();
        }
        if mismatch == 3 {
            let raw = fs::read_to_string(&accounting).unwrap();
            fs::write(
                &accounting,
                raw.replace("\"cost_per_stop\": 7", "\"cost_per_stop\": 8"),
            )
            .unwrap();
        }
        let mut loaded = recipe::load(&root.0.join("recipe.json"), &original.config, true).unwrap();
        let (selection, reviewer) = live_selection(&root, &original, &mut loaded, &accounting);
        if mismatch == 0 {
            // A real older canonical image cannot satisfy the independently
            // retained later anchor, even with permissive zero scalar floors.
            fs::write(
                original.config.store.join("delivery.bin"),
                &original.pre_generation,
            )
            .unwrap();
        }
        fs::write(
            original.config.store.join("delivery.pending"),
            b"unacknowledged staging",
        )
        .unwrap();
        let before = files(&original.config.store);
        let reads = original.config.source.status().read_attempts;
        let mut control = Control::new(&original.config, loaded.request, Some(&reviewer)).unwrap();
        let result = selection.open(
            &original.config,
            &loaded,
            &mut control,
            &deadline(),
            &mut || ElapsedTick(1000),
        );
        assert!(result.is_err(), "accepted mismatch {mismatch}");
        assert_eq!(files(&original.config.store), before);
        assert_eq!(original.config.source.status().read_attempts, reads);
        assert!(control.recovery_application().is_none());
        assert!(!selection.peer.socket_path().exists());
        assert!(!loaded.anchor_output.as_ref().unwrap().exists());
    }
}

#[test]
fn accounted_replay_deadlines_at_every_event_and_ready_release_no_roles_or_writes() {
    let root = Root::new();
    let original = prepared(&root, false);
    let accounting = accounting_for(&original, &root, 0, "deadline");
    let mut loaded = recipe::load(&root.0.join("recipe.json"), &original.config, true).unwrap();
    let (selection, reviewer) = live_selection(&root, &original, &mut loaded, &accounting);
    fs::write(
        original.config.store.join("delivery.pending"),
        b"unacknowledged staging",
    )
    .unwrap();
    let before = files(&original.config.store);
    let total = usize::try_from(original.before.revision).unwrap();
    let mut control = Control::new(&original.config, loaded.request, Some(&reviewer)).unwrap();
    for expire_at in 0..=total + 1 {
        let limit = deadline();
        let mut calls = 0;
        let result = selection.open(&original.config, &loaded, &mut control, &limit, &mut || {
            let cut = calls;
            calls += 1;
            if cut == expire_at {
                limit.logical
            } else {
                ElapsedTick(1000)
            }
        });
        assert_eq!(
            result.err().as_deref(),
            Some("one-shot workflow deadline reached")
        );
        assert_eq!(calls, expire_at + 1);
        assert_eq!(files(&original.config.store), before);
        assert!(!selection.peer.socket_path().exists());
        assert!(control.recovery_application().is_none());
    }
    let ((host, _human, observer, _identity), _evaluator) = selection
        .open(
            &original.config,
            &loaded,
            &mut control,
            &deadline(),
            &mut || ElapsedTick(1000),
        )
        .unwrap();
    assert_eq!(host.revision(), original.before.revision + 1);
    assert_eq!(
        host.learned_generation_inspection()
            .unwrap()
            .numerical
            .sampled_draws,
        1
    );
    assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
    assert!(observer.is_none());
    assert!(!original.config.store.join("delivery.pending").exists());
}

#[test]
fn invalid_message_predecessor_retains_the_acknowledged_recovery_fence_without_new_work() {
    let root = Root::new();
    let original = prepared(&root, true);
    let (path, loaded) = native::next(&root, &original.config, 3);
    let actor = peers::actor_profile(&root, &original.config, 3);
    let peer = peer_file(
        &root,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let accounting = accounting_for(
        &original,
        &root,
        original.evaluation_revision,
        "missing-predecessor",
    );
    let output = root.0.join("refused-predecessor.anchor");
    let args = arguments(
        &root,
        &original,
        &path,
        &actor,
        &accounting,
        &peer,
        &original.anchor,
        &output,
        &["--after-message", "2"],
    );
    let old_archive = fs::read(&original.anchor).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    assert!(command_with_clock(&args, None, || ElapsedTick(1000)).is_err());
    let current = FileOversight::read_publication_with_learned_generation(
        &original.config.store,
        &original.config.profile,
        &loaded.generation,
    )
    .unwrap();
    assert_eq!(current.revision, original.before.revision + 1);
    assert_eq!(
        current.control.ledger.epoch,
        original.before.control.ledger.epoch + 1
    );
    assert_eq!(current.executions, original.before.executions);
    assert_eq!(current.payload, original.before.payload);
    assert_eq!(
        current.control.ledger.charged,
        original.before.control.ledger.charged
    );
    assert!(!current.control.ledger.stages.contains_key(&3));
    assert_eq!(archive::load(&output).unwrap().revision(), current.revision);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
    assert!(!actor.socket.exists());
    assert!(
        !evaluator::Settings::read(&peer)
            .unwrap()
            .socket_path()
            .exists()
    );
    assert!(!root.0.join("evidence.json").exists());
}

#[test]
fn authenticated_stop_at_early_middle_and_ready_uses_only_the_original_terminal_suffix() {
    for position in 0..3 {
        let root = Root::new();
        let original = prepared(&root, false);
        let accounting = accounting_for(&original, &root, 0, "stop");
        let mut loaded = recipe::load(&root.0.join("recipe.json"), &original.config, true).unwrap();
        let (selection, reviewer) = live_selection(&root, &original, &mut loaded, &accounting);
        let total = usize::try_from(original.before.revision).unwrap();
        let cut = [0, total / 2, total + 1][position];
        let reads = original.config.source.status().read_attempts;
        let old_archive = fs::read(&original.anchor).unwrap();
        for name in ["evidence.json", "native-roster.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let mut control = Control::new(&original.config, loaded.request, Some(&reviewer)).unwrap();
        let mut calls = 0;
        let mut client = None;
        let result = selection.open(
            &original.config,
            &loaded,
            &mut control,
            &deadline(),
            &mut || {
                let now = calls;
                calls += 1;
                if now == cut {
                    let mut peer = reviewer.connect_stop_client(loaded.request).unwrap();
                    client = Some(std::thread::spawn(move || {
                        let start = Instant::now();
                        loop {
                            assert!(start.elapsed() < Duration::from_secs(15));
                            match peer.step().unwrap() {
                                StopClientProgress::NeedsDecision => peer.request_stop().unwrap(),
                                StopClientProgress::Complete => return peer.receipt().unwrap(),
                                _ => crate::workflow::pause(1),
                            }
                        }
                    }));
                }
                ElapsedTick(1000)
            },
        );
        let message = result
            .err()
            .expect("Stop must issue no live owner or evaluator");
        assert!(
            message.starts_with("native admission Stop acknowledged at revision"),
            "{message}"
        );
        let receipt = client.unwrap().join().unwrap();
        assert!(receipt.acknowledged() && !receipt.drained());
        let applied = control.recovery_application().unwrap();
        assert_eq!(receipt, applied.receipt);
        let history = applied.result.as_ref().unwrap();
        assert_eq!(history.appended_events(), 2);
        assert_eq!(history.snapshot().revision, original.before.revision + 2);
        assert_eq!(history.snapshot().executions, original.before.executions);
        assert_eq!(history.snapshot().payload, original.before.payload);
        assert_eq!(
            history.snapshot().control.ledger.charged,
            original.before.control.ledger.charged
        );
        assert_eq!(
            &archive::load(loaded.anchor_output.as_ref().unwrap()).unwrap(),
            history.history_anchor()
        );
        assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
        assert_eq!(original.config.source.status().read_attempts, reads);
        assert!(!selection.peer.socket_path().exists());
        assert!(!reviewer.socket(loaded.request).exists());
        drop(control);
        assert!(!crate::workflow::control::socket_path(&reviewer, loaded.request).exists());
    }
}

#[test]
fn actual_accounted_continuation_command_accepts_stop_at_ready_before_issuing_live_roles() {
    let root = Root::new();
    let original = prepared(&root, false);
    let actor = peers::actor_profile(&root, &original.config, 1);
    let reviewer = fixture::peers(&root, &original.config);
    let peer = peer_file(
        &root,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let accounting = accounting_for(&original, &root, 0, "command-stop");
    let output = root.0.join("command-stop.anchor");
    let args = arguments(
        &root,
        &original,
        &root.0.join("recipe.json"),
        &actor,
        &accounting,
        &peer,
        &original.anchor,
        &output,
        &["--continue-generation"],
    );
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let total = usize::try_from(original.before.revision).unwrap();
    let mut calls = 0;
    let mut client = None;
    let error = command_with_clock(&args, None, || {
        calls += 1;
        // The first call starts the invocation deadline; the next is pre-open.
        // Ready follows every original event and still precedes normal finish.
        if calls == total + 3 {
            let mut peer = reviewer.connect_stop_client(1).unwrap();
            client = Some(std::thread::spawn(move || {
                let started = Instant::now();
                loop {
                    assert!(started.elapsed() < Duration::from_secs(15));
                    match peer.step().unwrap() {
                        StopClientProgress::NeedsDecision => peer.request_stop().unwrap(),
                        StopClientProgress::Complete => return peer.receipt().unwrap(),
                        _ => crate::workflow::pause(1),
                    }
                }
            }));
        }
        ElapsedTick(1000)
    })
    .unwrap_err();
    assert!(
        error.starts_with("native admission Stop acknowledged at revision"),
        "{error}"
    );
    let receipt = client
        .expect("Ready retained original Stop intake")
        .join()
        .unwrap();
    assert!(receipt.acknowledged() && !receipt.drained());
    let current = FileOversight::read_publication_with_learned_generation(
        &original.config.store,
        &original.config.profile,
        &original.loaded.generation,
    )
    .unwrap();
    assert_eq!(current.revision, original.before.revision + 2);
    assert_eq!(current.executions, 0);
    assert_eq!(current.payload, Vec::<u8>::new());
    assert_eq!(
        current.control.ledger.charged,
        original.before.control.ledger.charged
    );
    assert_eq!(archive::load(&output).unwrap().revision(), current.revision);
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(1).exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 1).exists());
    assert!(
        !evaluator::Settings::read(&peer)
            .unwrap()
            .socket_path()
            .exists()
    );
}

#[test]
fn accounted_recovery_grammar_requires_one_explicit_mode_and_both_independent_archives() {
    let root = Root::new();
    let fresh = super::args(
        &root,
        Path::new("missing-andon"),
        Path::new("missing-evaluator"),
    );
    for mode in [
        vec!["--continue-generation"],
        vec!["--after-message", "1"],
        vec!["--finish-after-message", "2"],
    ] {
        let mut valid = fresh.clone();
        valid.extend(["--history-anchor".into(), "previous.anchor".into()]);
        valid.extend(mode.into_iter().map(str::to_owned));
        let (remaining, selection) = evaluation::take_options(&valid, None).unwrap();
        assert!(selection.is_some());
        assert!(remaining.iter().any(|value| value == "--history-anchor"));
        for mismatch in 0..7 {
            let mut bad = valid.clone();
            match mismatch {
                0 => {
                    let at = bad
                        .iter()
                        .position(|value| value == "--history-anchor")
                        .unwrap();
                    bad.drain(at..at + 2);
                }
                1 => {
                    let at = bad
                        .iter()
                        .position(|value| value == "--retain-history-anchor")
                        .unwrap();
                    bad.drain(at..at + 2);
                }
                2 => bad.push("--continue-generation".into()),
                3 => bad.extend(["--after-message".into(), "0".into()]),
                4 => bad.extend(["--finish-after-message".into(), "01".into()]),
                5 => bad.extend(["--topology-profile".into(), "unsupported".into()]),
                _ => bad[0] = "serve-learned-generated".into(),
            }
            let error = command_with_clock(&bad, None, || {
                panic!("invalid recovery selection acquired time")
            });
            assert!(error.unwrap_err().contains("learned evaluation requires"));
            assert!(!root.0.join("store").exists());
        }
    }
}
