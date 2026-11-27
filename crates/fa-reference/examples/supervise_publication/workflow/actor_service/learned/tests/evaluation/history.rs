//! Restarted historical labels use original learned cases and accounting.
//! Fixture models test custody and publication causality, not detector quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    observed::decoder::learned::FileLearnedInspection,
    requests::FileRequestStatus,
};

const HISTORY_CHILD: &str =
    "workflow::actor_service::learned::tests::evaluation_tests::recovery_tests::history_tests::evaluator_process";
const HISTORY_ROOT: &str = "FA_ACCOUNTED_LEARNED_HISTORY_ROOT";
const HISTORY_PARTIAL: &str = "FA_ACCOUNTED_LEARNED_HISTORY_PARTIAL";

struct Historical {
    original: Original,
    numerical: FileLearnedInspection,
    request: FileRequestStatus,
}

fn prepared_history(root: &Root, hold: bool) -> Historical {
    prepared_history_profile(root, hold, false)
}

fn prepared_history_profile(root: &Root, hold: bool, joint: bool) -> Historical {
    let mut config = configured(root);
    let path = fixture::write(root, if hold { b"hold".as_slice() } else { b"allow".as_slice() });
    evidence(root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    let actor = peers::actor_profile(root, &config, loaded.request);
    let (selected, accounting) = super::super::selected_profile(
        root, &config, &mut loaded, &actor,
        std::process::id().checked_add(100_000).unwrap(), joint,
    );
    let ((mut host, reviewer, observer, identity), evaluator) =
        selected.create(&config, &loaded).unwrap();
    assert!(observer.is_none() && identity.is_none());
    let revision = host.revision();
    host.register_andon_window(revision, AndonBudget {
        window: 2, start: 100, end: 200, false_stops_allowed: 2, cost_per_stop: 9,
    }).unwrap();
    let pre_generation = fs::read(config.store.join("delivery.bin")).unwrap();
    drop(evaluator);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let limit = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &limit,
        &mut || ElapsedTick(1000)).unwrap();
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer,
        &mut control, &limit, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    assert!(!reviewed.stop_requested);
    let history = reviewed.history.unwrap();
    assert_eq!(history.status(), NativeReviewStatus::Finished);
    driver = reviewed.driver;
    if hold {
        assert!(!matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }));
    } else {
        assert!(matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }));
        // A real Allow still has no effect key when the original separate
        // reviewer rejects its exact original request.
        let offer = driver.request_learned_human_approval_from_policy_file(
            &mut config.source, 701, ElapsedTick(50_000), || ElapsedTick(1000),
        ).result.unwrap();
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        reviewer.reject(&mut host, revision, &offer).unwrap();
    }
    cancel_unspent(&mut driver, loaded.request).unwrap();
    let anchor = root.0.join("original.anchor");
    let (before, numerical, request, evaluation_revision) = {
        let host = driver.supervisor().host().unwrap();
        archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
        let report = host.credibility_report().unwrap();
        assert_eq!((report.retained_cases, report.pending_cases), (1, 1));
        (host.inspect(), host.learned_generation_inspection().unwrap(),
            host.request_status(loaded.request).unwrap(), report.revision)
    };
    assert_eq!(before.executions, 0);
    assert!(before.payload.is_empty());
    assert_eq!(evaluation_revision, 1);
    assert!(matches!(request.disposition, FileRequestDisposition::Admitted {
        stage: ActionState::Denied | ActionState::Cancelled, ..
    }));
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(history);
    Historical {
        original: Original {
            config, loaded, accounting, anchor, before, pre_generation, evaluation_revision,
        },
        numerical,
        request,
    }
}

fn historical_child(root: &Root, partial: bool) -> RecoveryEvaluator {
    RecoveryEvaluator(ProcessCommand::new(std::env::current_exe().unwrap())
        .args(["--exact", HISTORY_CHILD, "--nocapture"])
        .env(HISTORY_ROOT, &root.0)
        .env(HISTORY_PARTIAL, if partial { "yes" } else { "no" })
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit())
        .spawn().unwrap())
}

#[test]
fn evaluator_process() {
    let Ok(root) = std::env::var(HISTORY_ROOT) else { return; };
    let root = PathBuf::from(root);
    wait_for(&root.join("evaluation.ready"));
    let settings = evaluator::Settings::read(&root.join("evaluator.json")).unwrap();
    assert_eq!(settings.evaluator.pid, Some(std::process::id()));
    assert_ne!(settings.supervisor.pid, Some(std::process::id()));
    // The historical loader already had to replay the exact original numerical
    // inputs. Nothing after that cut may reopen them or generate another token.
    for path in ["model.json", "weights.safetensors"] {
        fs::remove_file(root.join(path)).unwrap();
    }
    if std::env::var(HISTORY_PARTIAL).unwrap() == "yes" {
        let mut socket = UnixStream::connect(settings.socket_path()).unwrap();
        settings.supervisor.policy().unwrap().verify(&socket).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        socket.write_all(b"{\"schema_version\":").unwrap();
        fs::write(root.join("partial.ready"), b"authenticated partial assessment").unwrap();
        let mut response = [0_u8; 1];
        match socket.read(&mut response) {
            Ok(0) => {}
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
            other => panic!("historical Stop left evaluator intake live: {other:?}"),
        }
        fs::write(root.join("partial.closed"), b"original Stop retired evaluator").unwrap();
    } else {
        label(&root, 1);
        wait_for(&root.join("service-1.finished"));
        assert!(UnixStream::connect(settings.socket_path()).is_err());
    }
}

fn forget_live_inputs(root: &Root) {
    for name in ["evidence.json", "native-roster.json", "helper-weights.safetensors"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
}

#[test]
fn historical_hold_and_human_rejection_accept_late_truth_without_live_inputs_or_effects() {
    historical_profile(false);
}

#[test]
fn joint_historical_evaluation_retains_the_original_policy_and_charges_without_live_work() {
    historical_profile(true);
}

fn historical_profile(joint: bool) {
    for hold in [true, false] {
        let root = Root::new();
        let historical = prepared_history_profile(&root, hold, joint);
        let original = &historical.original;
        let actor = peers::actor_profile(&root, &original.config, 1);
        let reviewer = fixture::peers(&root, &original.config);
        let mut child = historical_child(&root, false);
        let peer = peer_file(&root, &actor, child.0.id());
        let accounting = accounting_for(original, &root, 1, "historical");
        let output = root.0.join("historical.anchor");
        let args = arguments(&root, original, &root.0.join("recipe.json"), &actor,
            &accounting, &peer, &original.anchor, &output, &["--evaluate-only"]);
        let old_archive = fs::read(&original.anchor).unwrap();
        forget_live_inputs(&root);
        let endpoint = evaluator::Settings::read(&peer).unwrap();
        let result = command_with_clock(&args, None, || {
            assert!(!actor.socket.exists(), "historical evaluation admitted an actor");
            assert!(!reviewer.socket(1).exists(), "historical evaluation offered a human key");
            if endpoint.socket_path().exists() {
                assert!(crate::workflow::control::socket_path(&reviewer, 1).exists());
                fs::write(root.0.join("evaluation.ready"), b"original replay finished").unwrap();
            }
            crate::workflow::clock()
        });
        fs::write(root.0.join("service-1.finished"), b"historical evaluator retired").unwrap();
        child.finish();
        assert!(result.is_ok(), "{result:?}");
        let snapshot = FileOversight::read_publication_with_learned_generation(
            &original.config.store, &original.config.profile, &original.loaded.generation,
        ).unwrap();
        assert_eq!(snapshot.revision, original.before.revision + 3);
        assert_eq!(snapshot.executions, original.before.executions);
        assert_eq!(snapshot.payload, original.before.payload);
        assert_eq!(snapshot.target, original.before.target);
        assert_eq!(snapshot.control.ledger.charged, original.before.control.ledger.charged);
        assert_eq!(snapshot.control.ledger.reserved, original.before.control.ledger.reserved);
        assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
        let saved = archive::load(&output).unwrap();
        assert_eq!(saved.revision(), snapshot.revision);
        let receipt = read_json(&root.0.join("receipt-1.json"));
        assert_eq!(number(&receipt, &["after", "evaluation_revision"]), 2);
        assert_eq!(number(&receipt, &["after", "journal_revision"]), saved.revision());
        let settings = andon_settings::Settings::read(&accounting).unwrap();
        let expected = independent_requirements(
            &original.config, &original.loaded, &settings, 2,
        );
        let (host, roles) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
            &original.config.store, original.config.profile.clone(), &expected,
            &settings.protocol, &original.loaded.generation, &saved,
        ).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            historical.numerical.numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.joint_credibility_policy().unwrap(), settings.joint);
        assert_eq!(settings.joint.is_some(), joint);
        assert_eq!(host.request_status(1).unwrap(), historical.request);
        let report = host.credibility_report().unwrap();
        assert_eq!((report.revision, report.pending_cases, report.benign_origins), (2, 0, 1));
        let window = &host.andon_ledger().unwrap().windows()[&1];
        assert_eq!(window.false_stops, u64::from(hold));
        assert_eq!(window.total_cost, if hold { 7 } else { 0 });
        assert_eq!(window.incident.is_some(), hold);
        assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
        assert_eq!(host.inspect().executions, 0);
        assert!(host.inspect().payload.is_empty());
        drop(roles);
        drop(host);
        for name in ["evidence.json", "native-roster.json", "helper-weights.safetensors",
            "model.json", "weights.safetensors"] {
            assert!(!root.0.join(name).exists());
        }
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(1).exists());
        assert!(!endpoint.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 1).exists());
    }
}

#[test]
fn historical_mode_rejects_an_unrecorded_request_before_fence_or_new_roles() {
    let root = Root::new();
    let original = prepared(&root, false);
    let actor = peers::actor_profile(&root, &original.config, 1);
    let reviewer = fixture::peers(&root, &original.config);
    let peer = peer_file(&root, &actor, std::process::id().checked_add(100_000).unwrap());
    let accounting = accounting_for(&original, &root, 0, "unrecorded");
    let output = root.0.join("unrecorded.anchor");
    let args = arguments(&root, &original, &root.0.join("recipe.json"), &actor,
        &accounting, &peer, &original.anchor, &output, &["--evaluate-only"]);
    let before = files(&original.config.store);
    let old_archive = fs::read(&original.anchor).unwrap();
    forget_live_inputs(&root);
    assert!(command_with_clock(&args, None, || ElapsedTick(1000)).is_err());
    assert_eq!(files(&original.config.store), before);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
    assert!(!output.exists());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(1).exists());
    assert!(!evaluator::Settings::read(&peer).unwrap().socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 1).exists());
}

#[test]
fn historical_selector_rechecks_deadline_and_stop_before_its_sole_fence() {
    for stop in [false, true] {
        let root = Root::new();
        let historical = prepared_history(&root, true);
        let original = &historical.original;
        let accounting = accounting_for(original, &root, 1, "after-selection");
        let mut loaded = recipe::load(&root.0.join("recipe.json"), &original.config, false).unwrap();
        let (selection, reviewer) = live_selection(&root, original, &mut loaded, &accounting);
        let before = files(&original.config.store);
        let cut = usize::try_from(original.before.revision).unwrap() + 2;
        let limit = deadline();
        let mut calls = 0;
        let mut client = None;
        let mut control = Control::new(&original.config, 1, Some(&reviewer)).unwrap();
        let result = selection.open_history(&original.config, &loaded, &mut control,
            &limit, &mut || {
                let current = calls;
                calls += 1;
                if current == cut {
                    if !stop { return limit.logical; }
                    let mut peer = reviewer.connect_stop_client(1).unwrap();
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
            }, None);
        let error = result.err().expect("late Stop/deadline cannot issue historical roles");
        if stop {
            assert!(error.starts_with("native admission Stop acknowledged at revision"), "{error}");
            let receipt = client.unwrap().join().unwrap();
            assert!(receipt.acknowledged() && !receipt.drained());
            let applied = control.recovery_application().unwrap();
            let history = applied.result.as_ref().unwrap();
            assert_eq!(history.appended_events(), 2);
            assert_eq!(history.snapshot().revision, original.before.revision + 2);
            assert_eq!(history.snapshot().executions, 0);
            assert_eq!(&archive::load(loaded.anchor_output.as_ref().unwrap()).unwrap(),
                history.history_anchor());
        } else {
            assert_eq!(error, "one-shot workflow deadline reached");
            assert_eq!(calls, cut + 1);
            assert_eq!(files(&original.config.store), before);
            assert!(!loaded.anchor_output.as_ref().unwrap().exists());
        }
        assert!(!selection.peer.socket_path().exists());
        assert!(!reviewer.socket(1).exists());
    }
}

#[test]
fn original_stop_retires_a_partial_historical_assessment_without_applying_truth() {
    let root = Root::new();
    let historical = prepared_history(&root, true);
    let original = &historical.original;
    let actor = peers::actor_profile(&root, &original.config, 1);
    let reviewer = fixture::peers(&root, &original.config);
    let mut child = historical_child(&root, true);
    let peer = peer_file(&root, &actor, child.0.id());
    let accounting = accounting_for(original, &root, 1, "partial");
    let output = root.0.join("stopped-historical.anchor");
    let args = arguments(&root, original, &root.0.join("recipe.json"), &actor,
        &accounting, &peer, &original.anchor, &output, &["--evaluate-only"]);
    forget_live_inputs(&root);
    let stop = {
        let marker = root.0.join("partial.ready");
        let reviewer = reviewer.clone();
        std::thread::spawn(move || {
            wait_for(&marker);
            let mut client = reviewer.connect_stop_client(1).unwrap();
            let start = Instant::now();
            loop {
                assert!(start.elapsed() < Duration::from_secs(15));
                match client.step().unwrap() {
                    StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                    StopClientProgress::Complete => return client.receipt().unwrap(),
                    _ => crate::workflow::pause(1),
                }
            }
        })
    };
    let endpoint = evaluator::Settings::read(&peer).unwrap();
    let result = command_with_clock(&args, None, || {
        assert!(!actor.socket.exists() && !reviewer.socket(1).exists());
        if endpoint.socket_path().exists() {
            fs::write(root.0.join("evaluation.ready"), b"historical evaluator bound").unwrap();
        }
        crate::workflow::clock()
    });
    child.finish();
    assert!(result.is_ok(), "{result:?}");
    let receipt = stop.join().unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    let snapshot = FileOversight::read_publication_with_learned_generation(
        &original.config.store, &original.config.profile, &original.loaded.generation,
    ).unwrap();
    assert!(snapshot.stop.is_some());
    assert_eq!(snapshot.executions, 0);
    assert!(snapshot.payload.is_empty());
    assert_eq!(archive::load(&output).unwrap().revision(), snapshot.revision);
    assert!(root.0.join("partial.closed").exists());
    assert!(!root.0.join("assessment-1.json").exists());
    assert!(!root.0.join("receipt-1.json").exists());
    assert!(!endpoint.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 1).exists());
}

#[test]
fn historical_evaluation_owns_the_complete_closed_grammar_before_profiles_or_time() {
    let root = Root::new();
    let mut valid = super::super::args(&root, Path::new("missing-accounting"),
        Path::new("missing-evaluator"));
    valid.extend(["--evaluate-only".into(), "--history-anchor".into(), "old.anchor".into()]);
    let (_, selected) = evaluation::take_options(&valid, None).unwrap();
    assert!(selected.unwrap().evaluate_only);
    let mut finish = valid.clone();
    finish.extend(["--finish-after-message".into(), "1".into()]);
    assert!(evaluation::take_options(&finish, None).is_ok());
    for flag in ["--actor-profile", "--andon", "--evaluator-profile",
        "--history-anchor", "--retain-history-anchor"] {
        let mut bad = valid.clone();
        let at = bad.iter().position(|arg| arg == flag).unwrap();
        bad.drain(at..at + 2);
        assert!(command_with_clock(&bad, None,
            || panic!("invalid historical grammar acquired time")).is_err());
    }
    for extra in [
        vec!["--evaluate-only"], vec!["--continue-generation"],
        vec!["--after-message", "1"], vec!["--precheck-profile", "extra"],
        vec!["--operator-profile", "extra", "--tower-generation", "1"],
        vec!["--topology-profile", "extra", "--topology-profile", "duplicate"],
        vec!["--finish-after-message", "01"],
        vec!["--checkpoint-before-generation", "extra"],
    ] {
        let mut bad = valid.clone();
        bad.extend(extra.into_iter().map(str::to_owned));
        assert!(command_with_clock(&bad, None,
            || panic!("unsupported historical composition acquired time")).is_err());
    }
    assert!(!root.0.join("store").exists());
}
