//! Actual historical evaluation keeps the original source work without opening
//! next-source files or reissuing actor, helper or human publication services.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    observed::decoder::learned::FileLearnedInspection, requests::FileRequestStatus,
};

struct Historical {
    config: Config,
    recipe: PathBuf,
    loaded: Loaded,
    pair: Archives,
    snapshot: FileDeliverySnapshot,
    numerical: FileLearnedInspection,
    request: FileRequestStatus,
    evaluation_revision: u64,
}

fn prepared(root: &Root, joint: bool, hold: bool) -> Historical {
    let mut config = configured(root);
    let path = native::write(root);
    if hold {
        native::provision_response(root, b"hold");
    }
    evidence(root, &config, 1, true, false);
    let topology = topology_fixture::creation_profile(root, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    let actor = peers::actor_profile(root, &config, loaded.request);
    let (selected, _) = super::super::selected_profile(
        root,
        &config,
        &mut loaded,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
        joint,
    );
    let pair = Archives::first(root);
    loaded.anchor_output = Some(pair.history.clone());
    custody::admit(&config, &mut loaded, Some(&topology), custody::Mode::Create).unwrap();
    let limit = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let (started, evaluator) = selected
        .start(&config, &loaded, false, &mut control, &limit, &mut || {
            ElapsedTick(1000)
        })
        .unwrap();
    drop(evaluator);
    let custody::Started {
        custody: (host, reviewer, observer, identity),
        source,
        output,
    } = started;
    assert!(observer.is_none() && identity.is_none());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    custody::attach(&mut driver, source).unwrap();
    custody::certify(&mut driver, &limit, &mut || ElapsedTick(1000)).unwrap();
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
    let reviewed = review::run(
        driver,
        &mut config,
        &loaded,
        &reviewer,
        &mut control,
        &limit,
        &mut || ElapsedTick(1000),
    );
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    assert!(!reviewed.stop_requested);
    let history = reviewed.history.unwrap();
    assert_eq!(history.status(), NativeReviewStatus::Finished);
    driver = reviewed.driver;
    if hold {
        assert!(!matches!(
            driver.phase(),
            FileDriverPhase::AwaitingDispatch { .. }
        ));
    } else {
        assert!(matches!(
            driver.phase(),
            FileDriverPhase::AwaitingDispatch { .. }
        ));
        let offer = driver
            .request_learned_human_approval_from_policy_file(
                &mut config.source,
                701,
                ElapsedTick(50_000),
                || ElapsedTick(1000),
            )
            .result
            .unwrap();
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        reviewer.reject(&mut host, revision, &offer).unwrap();
    }
    cancel_unspent(&mut driver, loaded.request).unwrap();
    let (snapshot, numerical, request) = {
        let host = driver.supervisor().host().unwrap();
        let report = host.credibility_report().unwrap();
        assert_eq!(
            (report.revision, report.retained_cases, report.pending_cases),
            (1, 1, 1)
        );
        output
            .as_ref()
            .unwrap()
            .retain(&host, driver.topology())
            .unwrap();
        archive::export_acknowledged(&host.history_anchor().unwrap(), &pair.history).unwrap();
        (
            host.inspect(),
            host.learned_generation_inspection().unwrap(),
            host.request_status(loaded.request).unwrap(),
        )
    };
    assert_eq!(snapshot.executions, 0);
    assert!(snapshot.payload.is_empty());
    assert!(pair.read().work().checks > 0);
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(history);
    Historical {
        config,
        recipe: path,
        loaded,
        pair,
        snapshot,
        numerical,
        request,
        evaluation_revision: 1,
    }
}

fn prepared_finish(root: &Root) -> Historical {
    // Reuse the real authenticated Message path, then retain a genuinely
    // reviewed Finish with its separate human rejection and pending label.
    let original = super::original(root, true, false);
    let mut config = original.config;
    evidence(root, &config, 2, true, false);
    next_source(root, 2);
    let (message_path, message) = native::next(root, &config, 2);
    let second = Archives::named(root, "message-2");
    let source = topology_profile(root, &original.pair, Some(&second), 2, "message");
    let accounting = accounting_for(root, original.completed.evaluation_revision, "message");
    let delivered = run_command(
        root,
        &config,
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
    assert_eq!(delivered.snapshot.executions, 2);
    evidence(root, &config, 3, true, false);
    next_source(root, 3);
    let (path, mut loaded) = native::next(root, &config, 3);
    let pair = Archives::named(root, "pending-finish");
    let source = topology_profile(root, &second, Some(&pair), 3, "pending-finish");
    let accounting = accounting_for(root, delivered.evaluation_revision, "pending-finish");
    let actor = peers::actor_profile(root, &config, loaded.request);
    let peer = peer_file(
        root,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let args = arguments(
        root,
        &config,
        &actor,
        &peer,
        &Invocation {
            recipe: &path,
            loaded: &loaded,
            accounting: &accounting,
            topology: &source,
            input: Some(&second),
            output: &pair,
            mode: &["--finish-after-message", "2"],
            round: 33,
            decision: ReviewDecision::Reject,
            evaluator_mode: "assess",
        },
    );
    let (remaining, options) = evaluation::take_options(&args, None).unwrap();
    let selected = options.unwrap().read().unwrap();
    let (_, anchors) = archive::take_options(&remaining).unwrap();
    anchors.admit(&config, &mut loaded, true).unwrap();
    custody::admit(&config, &mut loaded, Some(&source), custody::Mode::Live).unwrap();
    let limit = Deadline {
        logical: plus(crate::workflow::clock(), 55_000).unwrap(),
        started: Instant::now(),
        wall: Duration::from_secs(55),
    };
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let (started, evaluator) = selected
        .start(
            &config,
            &loaded,
            true,
            &mut control,
            &limit,
            &mut crate::workflow::clock,
        )
        .unwrap();
    drop(evaluator);
    let custody::Started {
        custody: (host, reviewer, observer, identity),
        source,
        output,
    } = started;
    assert!(observer.is_none() && identity.is_none());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    custody::attach(&mut driver, source).unwrap();
    custody::certify(&mut driver, &limit, &mut crate::workflow::clock).unwrap();
    assert!(
        crate::workflow::actor_service::learned::finish::prepare(
            &mut driver,
            &reviewer,
            &mut config,
            &loaded,
            2,
            &mut control,
            &limit,
            &mut crate::workflow::clock,
        )
        .unwrap()
    );
    submit_release(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        LearnedTextRelease::Finish,
        &limit,
        &mut crate::workflow::clock,
    )
    .unwrap();
    let reviewed = review::run(
        driver,
        &mut config,
        &loaded,
        &reviewer,
        &mut control,
        &limit,
        &mut crate::workflow::clock,
    );
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    assert!(!reviewed.stop_requested);
    let history = reviewed.history.unwrap();
    assert_eq!(history.status(), NativeReviewStatus::Finished);
    driver = reviewed.driver;
    assert!(matches!(
        driver.phase(),
        FileDriverPhase::AwaitingDispatch { request: 3 }
    ));
    let due = plus(crate::workflow::clock(), 50_000).unwrap();
    let offer = driver
        .request_learned_human_approval_from_policy_file(
            &mut config.source,
            703,
            due,
            crate::workflow::clock,
        )
        .result
        .unwrap();
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        reviewer.reject(&mut host, revision, &offer).unwrap();
    }
    cancel_unspent(&mut driver, loaded.request).unwrap();
    let (snapshot, numerical, request, evaluation_revision) = {
        let host = driver.supervisor().host().unwrap();
        assert!(host.evaluation_history(33).unwrap().is_empty());
        host.evaluation_ticket(33).unwrap();
        let action = host.request_action(3).unwrap();
        let frame = ReleaseFrame::decode(&action.spec().payload).unwrap();
        assert!(frame.is_finish());
        assert_eq!(frame.prior_messages(), &["A", "A"]);
        output
            .as_ref()
            .unwrap()
            .retain(&host, driver.topology())
            .unwrap();
        archive::export_acknowledged(&host.history_anchor().unwrap(), &pair.history).unwrap();
        (
            host.inspect(),
            host.learned_generation_inspection().unwrap(),
            host.request_status(3).unwrap(),
            host.credibility_report().unwrap().revision,
        )
    };
    assert_eq!(snapshot.executions, 2);
    assert_eq!(snapshot.payload, b"AA");
    assert_eq!(
        snapshot.control.ledger.charged,
        delivered.snapshot.control.ledger.charged
    );
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(history);
    Historical {
        config,
        recipe: path,
        loaded,
        pair,
        snapshot,
        numerical,
        request,
        evaluation_revision,
    }
}

fn command_args(
    root: &Root,
    original: &Historical,
    input: &Archives,
    output: &Archives,
    actor: &Profile,
    peer: &Path,
    minimum: u64,
    tag: &str,
) -> Vec<String> {
    // Generation 99 is deliberately never provisioned. Historical assessment
    // may read the independent initial documents and saved source cut only.
    let topology = topology_profile(root, input, Some(output), 99, tag);
    let accounting = accounting_for(root, minimum, tag);
    arguments(
        root,
        &original.config,
        actor,
        peer,
        &Invocation {
            recipe: &original.recipe,
            loaded: &original.loaded,
            accounting: &accounting,
            topology: &topology,
            input: Some(input),
            output,
            mode: &["--evaluate-only"],
            round: 1,
            decision: ReviewDecision::Reject,
            evaluator_mode: "assess",
        },
    )
}

fn remove_live_inputs(root: &Root) {
    for name in [
        "evidence.json",
        "native-roster.json",
        "helper-weights.safetensors",
        "live-1.json",
        "inventory-1.json",
    ] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
}

fn assert_pair(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
    assert!(!after.available());
}

fn observe_historical(
    root: &Root,
    args: &[String],
    actor: &Profile,
    reviewer: &PeerProfile,
    peer: &Path,
) -> Result<(), String> {
    let endpoint = evaluator::Settings::read(peer).unwrap();
    command_with_clock(args, None, || {
        assert!(
            !actor.socket.exists(),
            "historical assessment opened an actor listener"
        );
        assert!(
            !reviewer.socket(actor.request).exists(),
            "historical assessment offered a human key"
        );
        if endpoint.socket_path().exists() {
            assert!(crate::workflow::control::socket_path(reviewer, actor.request).exists());
            // Reuse the independent fixture's readiness barrier. No actor is
            // started: this marker means the original historical replay ended.
            fs::write(
                root.0.join(format!("actor-{}.finished", actor.request)),
                b"historical evaluator ready",
            )
            .unwrap();
        }
        assert!(!root.0.join("live-99.json").exists());
        assert!(!root.0.join("inventory-99.json").exists());
        crate::workflow::clock()
    })
}

#[test]
fn historical_topology_labels_the_exact_rejected_finish_without_new_source_or_numerical_work() {
    let root = Root::new();
    let mut original = prepared_finish(&root);
    let before = original.pair.read();
    let actor = peers::actor_profile(&root, &original.config, 3);
    let reviewer = fixture::peers(&root, &original.config);
    let mut child = Independent::start(&root, 3, 33, "assess");
    let peer = peer_file(&root, &actor, child.pid());
    let output = Archives::named(&root, "historical-finish");
    let mut args = command_args(
        &root,
        &original,
        &original.pair,
        &output,
        &actor,
        &peer,
        original.evaluation_revision,
        "historical-finish",
    );
    args.extend(["--finish-after-message".into(), "2".into()]);
    remove_live_inputs(&root);
    for name in [
        "live-2.json",
        "inventory-2.json",
        "live-3.json",
        "inventory-3.json",
    ] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    let bytes = fs::read(original.config.store.join("delivery.bin")).unwrap();
    let mut wrong_predecessor = args.clone();
    *wrong_predecessor.last_mut().unwrap() = "1".into();
    assert!(command_with_clock(&wrong_predecessor, None, crate::workflow::clock).is_err());
    assert_eq!(
        fs::read(original.config.store.join("delivery.bin")).unwrap(),
        bytes
    );
    assert!(!output.history.exists() && !output.source.exists());
    let result = observe_historical(&root, &args, &actor, &reviewer, &peer);
    fs::write(
        root.0.join("service-3.finished"),
        b"historical Finish evaluator retired",
    )
    .unwrap();
    child.finish();
    assert!(result.is_ok(), "{result:?}");
    let saved = output.read();
    assert_pair(&before, &saved);
    assert_eq!(
        saved.history_anchor().revision(),
        original.snapshot.revision + 3
    );
    let receipt = read_json(&root.0.join("receipt-33.json"));
    assert_eq!(
        number(&receipt, &["after", "evaluation_revision"]),
        original.evaluation_revision + 1
    );
    let cursor = verified_history(
        &root,
        &original.config,
        &mut original.loaded,
        &output,
        original.evaluation_revision + 1,
    );
    let (host, roles) = cursor.finish().unwrap();
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        original.numerical.numerical
    );
    assert_eq!(host.request_status(3).unwrap(), original.request);
    assert_eq!(host.inspect().executions, original.snapshot.executions);
    assert_eq!(host.inspect().payload, original.snapshot.payload);
    assert_eq!(
        host.inspect().control.ledger.charged,
        original.snapshot.control.ledger.charged
    );
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert!(!stream.confirmed.finished());
    assert_eq!(host.evaluation_history(33).unwrap().len(), 1);
    let reused = roles
        .topology_observer
        .evaluated_topology_checkpoint(&host, &saved)
        .unwrap();
    assert_pair(&saved, &reused);
}

#[test]
fn historical_source_checkpoint_refuses_a_real_numerical_resume_suffix() {
    let root = Root::new();
    let mut original = prepared(&root, true, false);
    let saved = original.pair.read();
    let cursor = verified_history(
        &root,
        &original.config,
        &mut original.loaded,
        &original.pair,
        original.evaluation_revision,
    );
    let (mut host, roles) = cursor.finish().unwrap();
    let observer = roles.topology_observer;
    assert!(
        observer
            .evaluated_topology_checkpoint(&host, &saved)
            .is_ok()
    );
    host.observe_time(host.revision(), ElapsedTick(1001))
        .unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(
        host.revision(),
        numerical.actor_revision,
        numerical.position,
    )
    .unwrap();
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical
    );
    let before = fs::read(original.config.store.join("delivery.bin")).unwrap();
    assert!(
        observer
            .evaluated_topology_checkpoint(&host, &saved)
            .is_err()
    );
    assert_eq!(
        fs::read(original.config.store.join("delivery.bin")).unwrap(),
        before
    );
}

#[test]
fn historical_topology_labels_held_and_rejected_messages_without_live_inputs_or_new_work() {
    for joint in [false, true] {
        for hold in [false, true] {
            let root = Root::new();
            let mut original = prepared(&root, joint, hold);
            let before = original.pair.read();
            let old_history = fs::read(&original.pair.history).unwrap();
            let old_source = fs::read(&original.pair.source).unwrap();
            let actor = peers::actor_profile(&root, &original.config, 1);
            let reviewer = fixture::peers(&root, &original.config);
            let mut child = Independent::start(&root, 1, 1, "assess");
            let peer = peer_file(&root, &actor, child.pid());
            let output = Archives::named(&root, "historical");
            let args = command_args(
                &root,
                &original,
                &original.pair,
                &output,
                &actor,
                &peer,
                1,
                "historical",
            );
            remove_live_inputs(&root);
            let result = observe_historical(&root, &args, &actor, &reviewer, &peer);
            fs::write(
                root.0.join("service-1.finished"),
                b"historical service retired",
            )
            .unwrap();
            child.finish();
            assert!(result.is_ok(), "{result:?}");
            let saved = output.read();
            assert_pair(&before, &saved);
            assert_eq!(
                saved.history_anchor().revision(),
                original.snapshot.revision + 3
            );
            assert_eq!(fs::read(&original.pair.history).unwrap(), old_history);
            assert_eq!(fs::read(&original.pair.source).unwrap(), old_source);
            let receipt = read_json(&root.0.join("receipt-1.json"));
            assert_eq!(number(&receipt, &["after", "evaluation_revision"]), 2);
            assert_eq!(
                number(&receipt, &["after", "journal_revision"]),
                saved.history_anchor().revision()
            );
            // An exported pair is useful recovery input, not a detached report.
            let cursor =
                verified_history(&root, &original.config, &mut original.loaded, &output, 2);
            let (host, roles) = cursor.finish().unwrap();
            assert_eq!(
                host.learned_generation_inspection().unwrap().numerical,
                original.numerical.numerical
            );
            assert_eq!(host.request_status(1).unwrap(), original.request);
            assert_eq!(host.inspect().executions, original.snapshot.executions);
            assert_eq!(host.inspect().payload, original.snapshot.payload);
            assert_eq!(
                host.inspect().control.ledger.charged,
                original.snapshot.control.ledger.charged
            );
            let report = host.credibility_report().unwrap();
            assert_eq!(
                (report.revision, report.pending_cases, report.benign_origins),
                (2, 0, 1)
            );
            assert_eq!(host.joint_credibility_policy().unwrap().is_some(), joint);
            let window = &host.andon_ledger().unwrap().windows()[&1];
            assert_eq!(window.false_stops, u64::from(hold));
            assert_eq!(window.total_cost, if hold { 7 } else { 0 });
            let reused = roles
                .topology_observer
                .evaluated_topology_checkpoint(&host, &saved)
                .unwrap();
            assert_pair(&saved, &reused);
            assert_eq!(
                reused.history_anchor().revision(),
                saved.history_anchor().revision() + 1
            );
            assert!(!actor.socket.exists() && !reviewer.socket(1).exists());
            assert!(
                !evaluator::Settings::read(&peer)
                    .unwrap()
                    .socket_path()
                    .exists()
            );
        }
    }
}

#[test]
fn historical_topology_refuses_changed_contracts_unknown_requests_and_output_aliases_before_fence()
{
    for mode in [
        "accounting",
        "unknown-request",
        "missing-output",
        "actor-output",
    ] {
        let root = Root::new();
        let original = prepared(&root, true, true);
        let actor = peers::actor_profile(&root, &original.config, 1);
        let reviewer = fixture::peers(&root, &original.config);
        let peer = peer_file(
            &root,
            &actor,
            std::process::id().checked_add(100_000).unwrap(),
        );
        let output = Archives::named(&root, "refused");
        let mut args = command_args(
            &root,
            &original,
            &original.pair,
            &output,
            &actor,
            &peer,
            1,
            "refused",
        );
        match mode {
            "accounting" => {
                let path = root.0.join("accounting-refused.json");
                let current = fs::read_to_string(&path).unwrap();
                let changed = current.replace("\"cost_per_stop\": 7", "\"cost_per_stop\": 8");
                assert_ne!(changed, current);
                fs::write(path, changed).unwrap();
            }
            "unknown-request" => {
                let (recipe, _) = native::next(&root, &original.config, 2);
                let actor = peers::actor_profile(&root, &original.config, 2);
                let replacement =
                    topology_fixture::command_inputs(&root, &original.config, &recipe, &actor);
                args[..6].clone_from_slice(&replacement);
            }
            _ => {
                let path = root.0.join("topology-refused.json");
                let current = fs::read_to_string(&path).unwrap();
                let retained = format!(r#","retain_checkpoint":"{}""#, output.source.display());
                let changed = if mode == "missing-output" {
                    current.replace(&retained, "")
                } else {
                    current.replace(
                        output.source.to_str().unwrap(),
                        actor.socket.to_str().unwrap(),
                    )
                };
                assert_ne!(changed, current);
                fs::write(path, changed).unwrap();
            }
        }
        let before = fs::read(original.config.store.join("delivery.bin")).unwrap();
        assert!(
            command_with_clock(&args, None, crate::workflow::clock).is_err(),
            "{mode}"
        );
        assert_eq!(
            fs::read(original.config.store.join("delivery.bin")).unwrap(),
            before,
            "{mode}"
        );
        assert!(
            !output.history.exists() && !output.source.exists(),
            "{mode}"
        );
        assert!(!actor.socket.exists() && !reviewer.socket(1).exists());
        assert!(
            !evaluator::Settings::read(&peer)
                .unwrap()
                .socket_path()
                .exists()
        );
    }
}

#[test]
fn historical_topology_stop_retires_partial_truth_and_retains_the_same_source_work() {
    let root = Root::new();
    let original = prepared(&root, true, true);
    let saved = original.pair.read();
    let actor = peers::actor_profile(&root, &original.config, 1);
    let reviewer = fixture::peers(&root, &original.config);
    let mut child = Independent::start(&root, 1, 1, "partial");
    let peer = peer_file(&root, &actor, child.pid());
    let output = Archives::named(&root, "stopped-history");
    let args = command_args(
        &root,
        &original,
        &original.pair,
        &output,
        &actor,
        &peer,
        1,
        "stopped-history",
    );
    remove_live_inputs(&root);
    let stop = {
        let marker = root.0.join("partial.ready");
        let reviewer = reviewer.clone();
        std::thread::spawn(move || {
            wait_for(&marker);
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
    };
    let result = observe_historical(&root, &args, &actor, &reviewer, &peer);
    fs::write(
        root.0.join("service-1.finished"),
        b"historical Stop completed",
    )
    .unwrap();
    child.finish();
    assert!(result.is_ok(), "{result:?}");
    let receipt = stop.join().unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert_pair(&saved, &output.read());
    let snapshot = FileOversight::read_publication_with_learned_generation(
        &original.config.store,
        &original.config.profile,
        &original.loaded.generation,
    )
    .unwrap();
    assert!(snapshot.stop.is_some());
    assert_eq!(snapshot.executions, 0);
    assert_eq!(
        snapshot.control.ledger.charged,
        original.snapshot.control.ledger.charged
    );
    assert_eq!(output.read().history_anchor().revision(), snapshot.revision);
    assert!(root.0.join("partial.closed").exists());
    assert!(!root.0.join("receipt-1.json").exists());
}

#[test]
fn historical_evaluator_bind_failure_retains_the_fence_pair_for_a_corrected_restart() {
    let root = Root::new();
    let original = prepared(&root, false, true);
    let saved = original.pair.read();
    let actor = peers::actor_profile(&root, &original.config, 1);
    let reviewer = fixture::peers(&root, &original.config);
    let peer = peer_file(
        &root,
        &actor,
        std::process::id().checked_add(100_000).unwrap(),
    );
    let output = Archives::named(&root, "startup-fence");
    let args = command_args(
        &root,
        &original,
        &original.pair,
        &output,
        &actor,
        &peer,
        1,
        "startup-fence",
    );
    remove_live_inputs(&root);
    let endpoint = evaluator::Settings::read(&peer).unwrap();
    let mut occupied = false;
    let error = command_with_clock(&args, None, || {
        if !occupied && crate::workflow::control::socket_path(&reviewer, 1).exists() {
            fs::write(endpoint.socket_path(), b"another owner").unwrap();
            occupied = true;
        }
        assert!(!actor.socket.exists() && !reviewer.socket(1).exists());
        crate::workflow::clock()
    })
    .unwrap_err();
    assert!(
        occupied && error.contains("evaluator socket path already exists"),
        "{error}"
    );
    assert_eq!(fs::read(endpoint.socket_path()).unwrap(), b"another owner");
    let fenced = output.read();
    assert_pair(&saved, &fenced);
    assert_eq!(
        fenced.history_anchor().revision(),
        saved.history_anchor().revision() + 1
    );
    fs::remove_file(endpoint.socket_path()).unwrap();

    // The same recorded Message remains independently labelable after the
    // operator corrects only the listener conflict and selects the new pair.
    let mut child = Independent::start(&root, 1, 1, "assess");
    let peer = peer_file(&root, &actor, child.pid());
    let next = Archives::named(&root, "corrected-history");
    let args = command_args(
        &root,
        &original,
        &output,
        &next,
        &actor,
        &peer,
        1,
        "corrected-history",
    );
    let result = observe_historical(&root, &args, &actor, &reviewer, &peer);
    fs::write(
        root.0.join("service-1.finished"),
        b"corrected historical service retired",
    )
    .unwrap();
    child.finish();
    assert!(result.is_ok(), "{result:?}");
    assert_pair(&saved, &next.read());
    assert_eq!(
        next.read().history_anchor().revision(),
        fenced.history_anchor().revision() + 3
    );
    assert_eq!(
        number(
            &read_json(&root.0.join("receipt-1.json")),
            &["after", "evaluation_revision"]
        ),
        2
    );
}
