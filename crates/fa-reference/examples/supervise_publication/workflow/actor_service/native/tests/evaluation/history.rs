//! Explicit historical evaluation reaches the original retained cases without
//! renewing source evidence, helper review, actor submission or publication.
//! Real kernel peers and synthetic native weights establish custody only.
use super::recovery::Original;
use super::*;
#[path = "history/governed.rs"]
mod governed;

const HISTORY_CHILD: &str = "workflow::actor_service::native::tests::evaluation_tests::history::historical_evaluator_process";
const HISTORY_ROUND: &str = "FA_NATIVE_HISTORICAL_EVALUATOR_ROUND";
const HISTORY_PENDING_SELECTION: &str = "FA_NATIVE_HISTORICAL_PENDING_SELECTION";

fn peer(root: &Root, mode: &str, round: u64) -> EvaluatorChild {
    EvaluatorChild(
        ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", HISTORY_CHILD, "--nocapture"])
            .env(CHILD_ROOT, &root.0)
            .env(CHILD_MODE, mode)
            .env(HISTORY_ROUND, round.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

fn pending_peer(root: &Root, select_last: bool) -> EvaluatorChild {
    EvaluatorChild(
        ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", HISTORY_CHILD, "--nocapture"])
            .env(CHILD_ROOT, &root.0)
            .env(CHILD_MODE, "history")
            .env_remove(HISTORY_ROUND)
            .env(
                HISTORY_PENDING_SELECTION,
                if select_last { "last" } else { "first" },
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

/// The seed consumes one discovery candidate while leaving its case pending.
/// The recovered peer receives no round ID and uses its two original candidates
/// for pending discovery and a label bound entirely to that returned identity.
#[test]
fn historical_evaluator_process() {
    let Ok(mode) = std::env::var(CHILD_MODE) else {
        return;
    };
    let known_round = if mode == "history" {
        assert!(
            std::env::var(HISTORY_ROUND).is_err(),
            "recovered discovery was given a round ID"
        );
        None
    } else {
        Some(
            std::env::var(HISTORY_ROUND)
                .unwrap()
                .parse::<u64>()
                .unwrap(),
        )
    };
    let root = PathBuf::from(std::env::var(CHILD_ROOT).unwrap());
    wait_for(&root.join(format!("{mode}.ready")));
    let path = root.join("evaluator.json");
    let selected = evaluator::Settings::read(&path).unwrap();
    assert_eq!(selected.evaluator.pid, Some(std::process::id()));
    assert_ne!(selected.supervisor.pid, Some(std::process::id()));
    let mut observation = Vec::new();
    evaluator::client::command(
        &[
            "evaluator-case".into(),
            path.display().to_string(),
            known_round.map_or_else(|| "--pending".into(), |round| round.to_string()),
        ],
        &mut observation,
    )
    .unwrap();
    let case = strict_json::parse(&observation, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(
        field(&case, &["knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(field(&case, &["effect_authority"]).as_bool(), Some(false));
    fs::write(root.join(format!("{mode}.case.json")), &observation).unwrap();
    if let Some(round) = known_round {
        assert_eq!(number(&case, &["knowledge", "value", "round"]), round);
        return;
    }
    assert!(!root.join("evidence.json").exists());
    assert!(!root.join("model.safetensors").exists());
    let inventory = field(&case, &["knowledge", "value"]);
    let cases = field(inventory, &["cases"]).as_array().unwrap();
    assert_eq!(number(inventory, &["total_pending"]), cases.len() as u64);
    assert_eq!(number(inventory, &["unfinished_reviews"]), 0);
    assert_eq!(field(inventory, &["complete"]).as_bool(), Some(true));
    assert!(field(inventory, &["after_round"]).is_null());
    assert!(field(inventory, &["next_after_round"]).is_null());
    assert!(field(&case, &["request", "continuation"]).is_null());
    let value = match std::env::var(HISTORY_PENDING_SELECTION).unwrap().as_str() {
        "first" => cases.first().unwrap(),
        "last" => cases.last().unwrap(),
        other => panic!("unknown pending-history selection {other}"),
    };
    assert_eq!(field(value, &["status"]).as_str(), Some("unassessed"));
    let round = number(value, &["round"]);
    let assessment = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},
        "policy_generation":{},"congress_generation":{},
        "case_evidence_root_hex":"{}","expected_evaluation_revision":{},
        "origin":{round},"evidence_id_hex":"{}","truth":"benign"}}"#,
        number(value, &["round"]),
        number(value, &["attempt"]),
        number(value, &["policy_generation"]),
        number(value, &["congress_generation"]),
        field(value, &["case_evidence_root_hex"]).as_str().unwrap(),
        number(&case, &["knowledge", "basis", "evaluation_revision"]),
        "17".repeat(32),
    );
    let assessment_path = root.join("historical-assessment.json");
    fs::write(&assessment_path, assessment).unwrap();
    let mut receipt = Vec::new();
    evaluator::client::command(
        &[
            "evaluator-assess".into(),
            path.display().to_string(),
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
    assert_eq!(
        number(&applied, &["after", "evaluation_revision"]),
        number(&applied, &["before", "evaluation_revision"]) + 1
    );
    assert_eq!(
        number(&applied, &["after", "journal_revision"]),
        number(&applied, &["before", "journal_revision"]) + 2
    );
    fs::write(root.join("historical-receipt.json"), receipt).unwrap();
    wait_for(&root.join("history.finished"));
    assert!(UnixStream::connect(selected.socket_path()).is_err());
}

fn replace_output(args: &mut [String], path: &Path) {
    let index = args
        .iter()
        .position(|arg| arg == "--retain-history-anchor")
        .unwrap();
    args[index + 1] = path.display().to_string();
}

fn admit(
    config: &Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    args: &[String],
    open: bool,
) -> (evaluation::Selection, anchors::Selection) {
    let (remaining, options) = evaluation::take_options(args).unwrap();
    assert!(!remaining.iter().any(|arg| arg == "--evaluate-only"));
    let (_, history) = anchors::take_options(&remaining).unwrap();
    let history = history.admit(config, open).unwrap().unwrap();
    let evaluation = options
        .unwrap()
        .read((config, actor, reviewer), (None, None, Some(&history)))
        .unwrap();
    evaluation
        .check(
            config,
            actor,
            reviewer,
            (None, None, Some(&history)),
            (open, None, None),
        )
        .unwrap();
    (evaluation, history)
}

fn reviewing_at(
    reviewer: PeerProfile,
    request: u64,
    store: PathBuf,
    profile: fa_reference::action::consequence::delivery::persistent::observed::FileOversightProfile,
    executions: u64,
) -> std::thread::JoinHandle<ReviewPacket> {
    std::thread::spawn(move || {
        wait_for(&reviewer.socket(request));
        let mut client = reviewer
            .connect_client_waiting(request, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(30));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert_eq!(
                        FileOversight::read_publication(&store, &profile)
                            .unwrap()
                            .executions,
                        executions
                    );
                    packet = Some(client.packet().unwrap().clone());
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => pause(1),
            }
        }
    })
}

/// Seed each retained case through actual source admission, monitored generation,
/// helper processes, actor transport and (unless held) independent human review.
/// The optional second invocation is the existing exact Finish path.
fn seed_release(root: &Root, previous: Option<Original>, hold: bool) -> Original {
    let finishing = previous.is_some();
    let request = if finishing { 92 } else { 91 };
    let mode = if finishing {
        "seed-finish"
    } else {
        "seed-message"
    };
    let mut child = peer(root, mode, request);
    let (mut config, native, accounting, mut arguments) = match previous {
        Some(original) => {
            let mut arguments = recovery::recovery_args(root, &original, child.0.id());
            arguments.extend(["--finish-after-message".into(), "91".into()]);
            (
                original.config,
                original.native,
                original.accounting,
                arguments,
            )
        }
        None => {
            let config = configured(root);
            let native = inputs(root, &config);
            let (actor, _) = profiles(root, &config);
            let accounting = accounting_file(root);
            let evaluator_path = peer_file(root, &actor, child.0.id());
            let arguments = args(root, &accounting, &evaluator_path);
            (config, native, accounting, arguments)
        }
    };
    if finishing {
        config.programs = native_programs_at_epoch(root, 1);
    }
    if hold {
        assert!(!finishing);
        hold_program(&mut config, root);
    }
    let evaluator_path = root.0.join("evaluator.json");
    fs::write(
        &evaluator_path,
        fs::read_to_string(&evaluator_path)
            .unwrap()
            .replace("\"candidate_limit\":2", "\"candidate_limit\":1"),
    )
    .unwrap();
    let anchor = root.0.join(if finishing {
        "finish.anchor"
    } else {
        "message.anchor"
    });
    replace_output(&mut arguments, &anchor);
    let (mut actor, reviewer) = profiles(root, &config);
    actor.request = request;
    let (evaluation, history) = admit(&config, &actor, &reviewer, &arguments, finishing);
    let store = config.store.clone();
    let profile = config.profile.clone();
    let (sender, receiver) = mpsc::channel();
    let actor_client = actor_thread(actor.clone(), receiver);
    let marker = root.0.join(format!("{mode}.ready"));
    let actor_client = std::thread::spawn(move || {
        let response = actor_client.join().unwrap();
        fs::write(marker, b"terminal original actor result").unwrap();
        response
    });
    let human = (!hold).then(|| {
        reviewing_at(
            reviewer.clone(),
            request,
            store.clone(),
            profile.clone(),
            u64::from(finishing),
        )
    });
    let mut output = ReferenceOutput {
        bytes: Vec::new(),
        sender: Some(sender),
    };
    let result = serve_lifecycle_with_evaluation(
        config,
        (&actor, &reviewer),
        native,
        (
            finishing,
            None,
            None,
            if finishing {
                finish::Intent::Finish(91)
            } else {
                finish::Intent::Existing
            },
        ),
        Services {
            operator: None,
            precheck: None,
            history: Some(&history),
            evaluation: Some(&evaluation),
        },
        crate::workflow::clock,
        &mut output,
    );
    assert!(result.is_ok(), "original seed release failed: {result:?}");
    child.finish();
    let response = actor_client.join().unwrap();
    assert!(matches!(
        response.result,
        Ok(Knowledge::Known { value, .. })
            if value == if hold { ActorOutcome::CancelledBeforeDispatch } else { ActorOutcome::Executed }
    ));
    if let Some(human) = human {
        let packet = human.join().unwrap();
        assert_eq!(packet.action().spec().policy_epoch, u64::from(finishing));
        assert_eq!(
            ReleaseFrame::decode(&packet.action().spec().payload)
                .unwrap()
                .is_finish(),
            finishing
        );
    }
    let before = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(
        before.executions,
        if hold { 0 } else { 1 + u64::from(finishing) }
    );
    assert_eq!(
        std::str::from_utf8(&output.bytes).unwrap().lines().count(),
        1
    );
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(request).exists());
    assert!(!evaluation.peer.socket_path().exists());
    assert_eq!(archive::load(&anchor).unwrap().revision(), before.revision);
    let mut config = configured(root);
    config.programs.clear();
    let native = recipe::load(
        &root.0.join("recipe.json"),
        config.profile.delivery.scope.tenant,
        config.profile.delivery.limits.bytes,
    )
    .unwrap();
    Original {
        config,
        native,
        accounting,
        anchor,
        before,
    }
}

fn historical_args(root: &Root, original: &Original, pid: u32, finishing: bool) -> Vec<String> {
    let mut arguments = recovery::recovery_args(root, original, pid);
    replace_output(&mut arguments, &root.0.join("historical.anchor"));
    arguments.push("--evaluate-only".into());
    if finishing {
        arguments.extend(["--finish-after-message".into(), "91".into()]);
    }
    arguments
}

#[test]
fn historical_native_message_hold_and_finish_keep_exact_work_and_account_delayed_labels() {
    // The final arm queries an earlier Message after a later Finish; the latest
    // native generation is not authority to erase an older retained case.
    for (hold, completed_finish, select_finish) in [
        (false, false, false),
        (true, false, false),
        (false, true, true),
        (false, true, false),
    ] {
        let root = Root::new();
        let mut original = seed_release(&root, None, hold);
        if completed_finish {
            original = seed_release(&root, Some(original), false);
        }
        let request = if select_finish { 92 } else { 91 };
        let mut child = pending_peer(&root, select_finish);
        let arguments = historical_args(&root, &original, child.0.id(), select_finish);
        let (mut actor, reviewer) = profiles(&root, &original.config);
        actor.request = request;
        let (evaluation, history) = admit(&original.config, &actor, &reviewer, &arguments, true);
        let selected = bootstrap::selection(&original.config, &original.native, None).unwrap();
        let expected = evaluation
            .recovery_requirements(&original.config, &original.native, &selected)
            .unwrap();
        let accounting = andon_settings::Settings::read(&original.accounting).unwrap();
        let store = original.config.store.clone();
        let profile = original.config.profile.clone();
        let decoder = original.native.decoder.clone();
        let tokenizer = original.native.tokenizer.clone();
        let before =
            FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7)
                .unwrap();
        let old_archive = fs::read(&original.anchor).unwrap();
        let old_case = read_json(&root.0.join(if select_finish {
            "seed-finish.case.json"
        } else {
            "seed-message.case.json"
        }));
        fs::remove_file(original.config.source.path()).unwrap();
        fs::remove_file(root.0.join("model.safetensors")).unwrap();
        assert!(original.config.programs.is_empty());
        let evaluator_socket = evaluation.peer.socket_path().to_path_buf();
        let ready = root.0.join("history.ready");
        let result = evaluation.serve_history(
            original.config,
            (&actor, &reviewer),
            original.native,
            None,
            &history,
            select_finish.then_some(91),
            || {
                assert!(
                    !actor.socket.exists(),
                    "historical evaluation bound actor intake"
                );
                assert!(
                    !reviewer.socket(request).exists(),
                    "historical evaluation requested a new human key"
                );
                if evaluator_socket.exists() && !ready.exists() {
                    fs::write(&ready, b"original historical evaluator is resident").unwrap();
                }
                crate::workflow::clock()
            },
        );
        fs::write(
            root.0.join("history.finished"),
            b"finite historical service retired",
        )
        .unwrap();
        assert!(result.is_ok(), "historical service failed: {result:?}");
        child.finish();
        let retained = archive::load(history.output().unwrap()).unwrap();
        let saved = FileOversight::read_andon_guarded(
            &store,
            &profile,
            &expected,
            &accounting.protocol,
            &retained,
        )
        .unwrap();
        let after =
            FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7)
                .unwrap();
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(
            after.text.generation_revision(),
            before.text.generation_revision()
        );
        assert_eq!(after.publication.executions, before.publication.executions);
        assert_eq!(after.publication.payload, before.publication.payload);
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert_eq!(
            after.publication.control.ledger.reserved,
            before.publication.control.ledger.reserved
        );
        assert_eq!(
            after.publication.control.ledger.epoch,
            before.publication.control.ledger.epoch + 1
        );
        assert_eq!(after.publication.revision, before.publication.revision + 3);
        assert_eq!(retained.revision(), after.publication.revision);
        assert!(after.publication.stop.is_none());
        let count = 1 + usize::from(completed_finish);
        assert_eq!(saved.credibility.report.retained_cases, count);
        assert_eq!(saved.credibility.report.pending_cases, count - 1);
        assert_eq!(saved.credibility.report.benign_origins, 1);
        assert_eq!(saved.credibility.report.revision, count as u64 + 1);
        let window = &saved.accounting.windows()[&1];
        assert_eq!(window.false_stops, u64::from(hold));
        assert_eq!(window.total_cost, if hold { 7 } else { 0 });
        assert_eq!(window.incident.is_some(), hold);
        let page = read_json(&root.0.join("history.case.json"));
        assert_eq!(
            field(&page, &["schema_version"]).as_str(),
            Some("fa.live-evaluator-pending/1")
        );
        let cases = field(&page, &["knowledge", "value", "cases"])
            .as_array()
            .unwrap();
        assert_eq!(cases.len(), count);
        assert_eq!(
            cases
                .iter()
                .map(|case| number(case, &["round"]))
                .collect::<Vec<_>>(),
            if completed_finish {
                vec![91, 92]
            } else {
                vec![91]
            }
        );
        let case = if select_finish {
            cases.last().unwrap()
        } else {
            cases.first().unwrap()
        };
        assert_eq!(number(case, &["round"]), request);
        assert_eq!(
            field(case, &["case_evidence_root_hex"]),
            field(&old_case, &["knowledge", "value", "case_evidence_root_hex"])
        );
        assert_eq!(
            field(case, &["policy_generation"]),
            field(&old_case, &["knowledge", "value", "policy_generation"])
        );
        assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(request).exists());
        assert!(!evaluator_socket.exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, request).exists());
        assert!(!root.0.join("evidence.json").exists());
        assert!(!root.0.join("model.safetensors").exists());
    }
}

#[test]
fn historical_native_selection_refuses_pending_wrong_request_recipe_and_finish_before_fence() {
    for which in 0..5 {
        let root = Root::new();
        let mut original = recovery::prepared_selected(&root, which != 0, None);
        let arguments = historical_args(
            &root,
            &original,
            std::process::id().checked_add(100_000).unwrap(),
            which == 4,
        );
        let (mut actor, reviewer) = profiles(&root, &original.config);
        if which == 1 || which == 4 {
            actor.request = 92;
        }
        let (evaluation, history) = admit(&original.config, &actor, &reviewer, &arguments, true);
        match which {
            2 => original.native.generation += 1,
            3 => {
                fs::write(root.0.join("prompt.txt"), b"cd").unwrap();
                original.native = recipe::load(
                    &root.0.join("recipe.json"),
                    original.config.profile.delivery.scope.tenant,
                    original.config.profile.delivery.limits.bytes,
                )
                .unwrap();
            }
            _ => {}
        }
        let canonical = fs::read(original.config.store.join("delivery.bin")).unwrap();
        let staging = original.config.store.join("delivery.pending");
        fs::write(&staging, b"original unacknowledged historical staging").unwrap();
        let old_archive = fs::read(&original.anchor).unwrap();
        fs::remove_file(original.config.source.path()).unwrap();
        original.config.programs.clear();
        let store = original.config.store.clone();
        let result = evaluation.serve_history(
            original.config,
            (&actor, &reviewer),
            original.native,
            None,
            &history,
            (which == 4).then_some(91),
            || {
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(actor.request).exists());
                ElapsedTick(1001)
            },
        );
        assert!(
            result.is_err(),
            "changed historical selection {which} acquired custody"
        );
        assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), canonical);
        assert_eq!(
            fs::read(&staging).unwrap(),
            b"original unacknowledged historical staging"
        );
        assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
        assert!(!history.output().unwrap().exists());
        assert!(!evaluation.peer.socket_path().exists());
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(actor.request).exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, actor.request).exists());
    }
}

#[test]
fn native_historical_evaluation_grammar_requires_explicit_independent_recovery() {
    let root = Root::new();
    let mut valid = args(
        &root,
        Path::new("missing-andon"),
        Path::new("missing-evaluator"),
    );
    valid[0] = "serve-open".into();
    valid.extend([
        "--history-anchor".into(),
        "missing-original-anchor".into(),
        "--evaluation-recovery".into(),
        "missing-requirements".into(),
        "--evaluate-only".into(),
    ]);
    let (remaining, selected) = evaluation::take_options(&valid).unwrap();
    assert!(selected.is_some());
    assert!(!remaining.iter().any(|arg| arg == "--evaluate-only"));
    let mut finishing = valid.clone();
    finishing.extend(["--finish-after-message".into(), "90".into()]);
    assert!(evaluation::take_options(&finishing).is_ok());
    finishing[0] = "serve-open-checked".into();
    finishing.insert(4, "missing-publication-profile".into());
    assert!(evaluation::take_options(&finishing).is_ok());
    for which in 0..10 {
        let mut bad = valid.clone();
        match which {
            0 => bad[0] = "serve-create".into(),
            1..=3 => {
                let option = match which {
                    1 => "--history-anchor",
                    2 => "--evaluation-recovery",
                    _ => "--retain-history-anchor",
                };
                let index = bad.iter().position(|arg| arg == option).unwrap();
                bad.drain(index..index + 2);
            }
            4 => bad.push("--evaluate-only".into()),
            5 => bad.push("true".into()),
            6 => bad.extend(["--after-message".into(), "90".into()]),
            7 => bad.extend(["--precheck-profile".into(), "missing-precheck".into()]),
            8 => bad.extend([
                "--operator-profile".into(),
                "missing-operator".into(),
                "--tower-generation".into(),
                "1".into(),
            ]),
            _ => {
                let index = bad.iter().position(|arg| arg == "--andon").unwrap();
                bad.drain(index..index + 4);
            }
        }
        assert!(
            evaluation::take_options(&bad).is_err(),
            "historical grammar {which}"
        );
        let mut output = Vec::new();
        let error = command_with_clock(
            &bad,
            None,
            || panic!("invalid historical mode read a clock"),
            &mut output,
        )
        .unwrap_err();
        assert!(error.contains("native evaluation requires"), "{error}");
        assert!(output.is_empty());
        assert!(!root.0.join("store").exists());
    }
}

#[test]
fn historical_native_ready_stop_precedes_selection_and_expiry_preserves_original_staging() {
    for expire in [false, true] {
        let root = Root::new();
        // There is still incomplete numerical work and no recorded request.
        // Stop wins at Ready before historical selection can refuse this input.
        let mut original = recovery::prepared_selected(&root, false, None);
        let arguments = historical_args(
            &root,
            &original,
            std::process::id().checked_add(100_000).unwrap(),
            false,
        );
        let (actor, reviewer) = profiles(&root, &original.config);
        let (evaluation, history) = admit(&original.config, &actor, &reviewer, &arguments, true);
        let selected = bootstrap::selection(&original.config, &original.native, None).unwrap();
        let expected = evaluation
            .recovery_requirements(&original.config, &original.native, &selected)
            .unwrap();
        let accounting = andon_settings::Settings::read(&original.accounting).unwrap();
        let store = original.config.store.clone();
        let profile = original.config.profile.clone();
        let decoder = original.native.decoder.clone();
        let tokenizer = original.native.tokenizer.clone();
        let before =
            FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7)
                .unwrap();
        let canonical = fs::read(store.join("delivery.bin")).unwrap();
        let staging = store.join("delivery.pending");
        fs::write(&staging, b"original unacknowledged history staging").unwrap();
        let old_archive = fs::read(&original.anchor).unwrap();
        fs::remove_file(original.config.source.path()).unwrap();
        original.config.programs.clear();
        let total = usize::try_from(original.before.revision).unwrap();
        let mut calls = 0;
        let mut client = None;
        let result = evaluation.serve_history(
            original.config,
            (&actor, &reviewer),
            original.native,
            None,
            &history,
            None,
            || {
                // One service-start observation, then the shared Ready driver's
                // entry observation and one observation at each event boundary.
                let at_ready = calls == total + 2;
                calls += 1;
                if at_ready && !expire {
                    client = Some(recovery::stopping(&reviewer));
                }
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(actor.request).exists());
                assert!(!evaluation.peer.socket_path().exists());
                if at_ready && expire {
                    ElapsedTick(u64::MAX)
                } else {
                    ElapsedTick(1001)
                }
            },
        );
        let error = result.unwrap_err();
        let after =
            FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7)
                .unwrap();
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(
            after.text.generation_revision(),
            before.text.generation_revision()
        );
        assert_eq!(after.publication.executions, 0);
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert_eq!(
            after.publication.control.ledger.elapsed,
            before.publication.control.ledger.elapsed
        );
        assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
        if expire {
            assert!(client.is_none());
            assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), canonical);
            assert_eq!(
                fs::read(&staging).unwrap(),
                b"original unacknowledged history staging"
            );
            assert!(!history.output().unwrap().exists());
        } else {
            assert!(
                error.starts_with("native admission Stop acknowledged at revision"),
                "{error}"
            );
            assert!(
                client
                    .expect("original Ready Stop was selected")
                    .join()
                    .unwrap()
                    .acknowledged()
            );
            assert_eq!(after.publication.revision, before.publication.revision + 2);
            let retained = archive::load(history.output().unwrap()).unwrap();
            assert_eq!(retained.revision(), after.publication.revision);
            let saved = FileOversight::read_andon_guarded(
                &store,
                &profile,
                &expected,
                &accounting.protocol,
                &retained,
            )
            .unwrap();
            assert_eq!(saved.credibility.report.revision, 0);
            assert_eq!(saved.accounting.windows().len(), 2);
            assert_eq!(
                saved
                    .credibility
                    .journal
                    .stop
                    .as_ref()
                    .unwrap()
                    .request()
                    .operation,
                actor.request
            );
        }
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(actor.request).exists());
        assert!(!evaluation.peer.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, actor.request).exists());
    }
}
