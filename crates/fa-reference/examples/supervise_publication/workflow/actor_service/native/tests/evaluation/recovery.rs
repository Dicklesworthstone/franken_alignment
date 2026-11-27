//! Real interrupted native work, recovered independent assessment and Stop.
//! Synthetic weights and helper choices test custody, not detector performance.
use super::*;
use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;
use fa_reference::round::review::andon::AndonBudget;

fn later_window(accounting: &Path) -> (AndonBudget, String) {
    let settings = andon_settings::Settings::read(accounting).unwrap();
    let start = settings.config.windows().iter().map(|window| window.end).max().unwrap();
    let end = start.checked_add(100).unwrap();
    let budget = AndonBudget { window: 2, start, end, false_stops_allowed: 2, cost_per_stop: 9 };
    let row = format!(r#"{{"window":2,"start":{start},"end":{end},"false_stops_allowed":2,"cost_per_stop":9}},"#);
    (budget, row)
}

pub(super) struct Original {
    pub(super) config: Config,
    pub(super) native: Inputs,
    pub(super) accounting: PathBuf,
    pub(super) anchor: PathBuf,
    pub(super) before: FileDeliverySnapshot,
}

fn prepared(root: &Root) -> Original {
    prepared_selected(root, false, None)
}

pub(super) fn prepared_selected(root: &Root, recorded: bool, publication: Option<&PublicationProfile>) -> Original {
    let mut config = configured(root);
    let native = inputs(root, &config);
    let (actor, _) = profiles(root, &config);
    let (evaluation, _, accounting) = selected(
        root, &config, &actor, std::process::id().checked_add(100_000).unwrap(),
    );
    let selected = bootstrap::selection(&config, &native, publication).unwrap();
    let (mut host, human, evaluator) = evaluation.create(&config, &native, selected).unwrap();
    let (budget, later) = later_window(&accounting);
    host.register_andon_window(host.revision(), budget).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        native.generation, n.actor_revision, n.position, native.request.clone(),
    ).unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    // Save the original prompt plus its first sampled output. Recovery must
    // continue the same finite command, RNG stream and cumulative work spend.
    let steps = if recorded { 4 } else { 3 };
    for revision in 0..steps {
        host.advance_decoder_text(host.revision(), native.generation, revision).unwrap();
    }
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws),
        if recorded { (4, 2) } else { (3, 1) });
    assert_eq!(host.decoder_text_progress(native.generation).unwrap().bytes().unwrap(), b"A");
    let anchor = root.0.join("pending.anchor");
    assert_eq!(host.credibility_report().unwrap().revision, 0);
    drop(evaluator);
    drop(human);
    let before = if recorded {
        let source = FileTextMessageRequest {
            request: 91, generation: native.generation, generation_revision: 4,
            target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch,
            deadline: ElapsedTick(10_000),
        };
        let proposal = FileGeneratedTextActorPort::encode_message(&source).unwrap();
        let (port, supervisor) = host.into_generated_text_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port);
        assert!(wire.exchange(&encode_command(&Command::Submit {
            request: 91, proposal,
        }).unwrap()).result.is_ok());
        let host = supervisor.host().unwrap();
        assert_eq!(host.decoder_text_message_request(91).unwrap(), &source);
        archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
        host.inspect()
    } else {
        archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
        host.inspect()
    };
    config.programs = native_programs_at_epoch(root, 1);
    let current = fs::read_to_string(accounting).unwrap()
        .replace("\"current_windows\": [", &format!("\"current_windows\": [{later}"));
    let accounting = root.0.join("recovery-andon.json");
    fs::write(&accounting, current).unwrap();
    Original { config, native, accounting, anchor, before }
}

pub(super) fn recovery_args(root: &Root, original: &Original, evaluator_pid: u32) -> Vec<String> {
    let (actor, _) = profiles(root, &original.config);
    let peer = peer_file(root, &actor, evaluator_pid);
    let requirements = root.0.join("recovery-requirements.json");
    fs::write(&requirements, format!(
        r#"{{"schema":"fa.native-evaluation-recovery/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}}}"#,
        original.before.revision, original.before.control.sequence,
        original.before.control.ledger.epoch,
    )).unwrap();
    let mut args = super::args(root, &original.accounting, &peer);
    args[0] = "serve-open".into();
    args.extend([
        "--history-anchor".into(), original.anchor.display().to_string(),
        "--evaluation-recovery".into(), requirements.display().to_string(),
    ]);
    args
}

fn admit(original: &Original, args: &[String], root: &Root)
    -> (evaluation::Selection, anchors::Selection)
{
    admit_selected(original, args, root, None)
}

fn admit_selected(original: &Original, args: &[String], root: &Root, publication: Option<&PublicationProfile>)
    -> (evaluation::Selection, anchors::Selection)
{
    let (actor, reviewer) = profiles(root, &original.config);
    let (remaining, options) = evaluation::take_options(args).unwrap();
    let (_, options_history) = anchors::take_options(&remaining).unwrap();
    let history = options_history.admit(&original.config, true).unwrap().unwrap();
    let selected = options.unwrap().read(
        (&original.config, &actor, &reviewer), (None, None, Some(&history)),
    ).unwrap();
    selected.check(
        &original.config, &actor, &reviewer, (None, None, Some(&history)),
        (true, publication, None),
    ).unwrap();
    (selected, history)
}

fn limit() -> Deadline {
    Deadline {
        logical: ElapsedTick(10_000), started: Instant::now(), wall: Duration::from_secs(10),
    }
}

pub(super) fn stopping(reviewer: &PeerProfile) -> std::thread::JoinHandle<StopControlReceipt> {
    let mut client = reviewer.connect_stop_client(91).unwrap();
    std::thread::spawn(move || {
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => pause(1),
            }
        }
    })
}

#[test]
fn accounted_native_pending_recovery_keeps_work_and_independent_human_and_evaluator_custody() {
    for approve in [false, true] {
        let root = Root::new();
        let original = prepared(&root);
        let mut child = EvaluatorChild::start(&root, "assess");
        let args = recovery_args(&root, &original, child.0.id());
        let (evaluation, history) = admit(&original, &args, &root);
        let (actor, reviewer) = profiles(&root, &original.config);
        let selected = bootstrap::selection(&original.config, &original.native, None).unwrap();
        let expected = evaluation.recovery_requirements(
            &original.config, &original.native, &selected,
        ).unwrap();
        let accounting = andon_settings::Settings::read(&original.accounting).unwrap();
        let old_archive = fs::read(&original.anchor).unwrap();
        let store = original.config.store.clone();
        let profile = original.config.profile.clone();
        let decoder = original.native.decoder.clone();
        let tokenizer = original.native.tokenizer.clone();
        let before = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, 7,
        ).unwrap();
        let (sender, receiver) = mpsc::channel();
        let client = actor_thread(actor.clone(), receiver);
        let finished = root.0.join("actor.finished");
        let client = std::thread::spawn(move || {
            let response = client.join().unwrap();
            fs::write(finished, b"terminal recovered actor result").unwrap();
            response
        });
        let human = reviewing(
            reviewer.clone(), store.clone(), profile.clone(),
            if approve { ReviewDecision::Approve } else { ReviewDecision::Reject },
        );
        let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
        let result = serve_lifecycle_with_evaluation(
            original.config, (&actor, &reviewer), original.native,
            (true, None, None, finish::Intent::Existing),
            Services {
                operator: None, precheck: None, history: Some(&history), evaluation: Some(&evaluation),
            },
            crate::workflow::clock, &mut output,
        );
        fs::write(root.0.join("service.finished"), b"original recovered service ended").unwrap();
        assert!(result.is_ok(), "{result:?}");
        child.finish();
        let packet = human.join().unwrap();
        assert_eq!(packet.action().spec().policy_epoch, 1);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        assert_eq!(packet.views().len(), 2);
        let response = client.join().unwrap();
        let retained = archive::load(history.output().unwrap()).unwrap();
        let saved = FileOversight::read_andon_guarded(
            &store, &profile, &expected, &accounting.protocol, &retained,
        ).unwrap();
        let after = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, 7,
        ).unwrap();
        assert_eq!(before.text.generation_revision(), 3);
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(after.text.generation_revision(), 4);
        assert_eq!(after.numerical.numerical.position, 4);
        assert_eq!(after.numerical.numerical.sampled_draws, 2);
        assert_eq!(saved.credibility.report.revision, 2);
        assert_eq!(saved.credibility.report.pending_cases, 0);
        assert_eq!(saved.credibility.report.retained_cases, 1);
        assert_eq!(saved.credibility.report.benign_origins, 1);
        assert_eq!(saved.accounting.windows().len(), 2);
        assert!(saved.accounting.windows().values().all(|window| {
            window.false_stops == 0 && window.total_cost == 0 && window.incident.is_none()
        }));
        assert_eq!(saved.credibility.journal.control.ledger.epoch, 1);
        assert_eq!(saved.credibility.journal.executions, u64::from(approve));
        assert_eq!(saved.credibility.journal.payload, if approve { b"A".to_vec() } else { Vec::new() });
        assert_eq!(saved.credibility.journal.revision, retained.revision());
        assert_eq!(saved.credibility.journal.control.ledger.reserved, 0);
        assert!(matches!(response.result, Ok(Knowledge::Known { value, .. })
            if value == if approve { ActorOutcome::Executed } else { ActorOutcome::CancelledBeforeDispatch }));
        assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
        assert!(!root.0.join("evidence.json").exists(), "late label used retained case evidence");
        assert_eq!(std::str::from_utf8(&output.bytes).unwrap().lines().count(), 1);
        assert!(!actor.socket.exists());
        assert!(!evaluation.peer.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 91).exists());
    }
}

#[test]
fn accounted_native_recovery_refuses_anchor_inventory_protocol_and_each_independent_floor() {
    for which in 0..7 {
        let root = Root::new();
        let original = prepared(&root);
        let args = recovery_args(&root, &original, std::process::id().checked_add(100_000).unwrap());
        let requirements = root.0.join("recovery-requirements.json");
        match which {
            0 => {
                let other_root = Root::new();
                let other = prepared(&other_root);
                fs::write(&original.anchor, fs::read(other.anchor).unwrap()).unwrap();
            }
            1..=3 => {
                let (name, current) = match which {
                    1 => ("journal_revision", original.before.revision),
                    2 => ("control_sequence", original.before.control.sequence),
                    _ => ("authority_epoch", original.before.control.ledger.epoch),
                };
                let document = fs::read_to_string(&requirements).unwrap();
                fs::write(&requirements, document.replace(
                    &format!("\"{name}\":{current}"), &format!("\"{name}\":{}", current + 1),
                )).unwrap();
            }
            4 => {
                let document = fs::read_to_string(&original.accounting).unwrap();
                fs::write(&original.accounting, document.replace(
                    "\"minimum_evaluation_revision\": 0", "\"minimum_evaluation_revision\": 1",
                )).unwrap();
            }
            5 => {
                let document = fs::read_to_string(&original.accounting).unwrap();
                let (_, row) = later_window(&original.accounting);
                assert!(document.contains(&row));
                fs::write(&original.accounting, document.replace(&row, "")).unwrap();
            }
            _ => {
                let document = fs::read_to_string(&original.accounting).unwrap();
                fs::write(&original.accounting, document.replace("\"domain\": 1", "\"domain\": 99")).unwrap();
            }
        }
        let (evaluation, history) = admit(&original, &args, &root);
        let (_, reviewer) = profiles(&root, &original.config);
        let mut control = Control::new(&original.config, 91, Some(&reviewer)).unwrap();
        let selected = bootstrap::selection(&original.config, &original.native, None).unwrap();
        let canonical = fs::read(original.config.store.join("delivery.bin")).unwrap();
        let staging = original.config.store.join("delivery.pending");
        fs::write(&staging, b"unacknowledged numerical staging").unwrap();
        let result = evaluation.open(
            &original.config, &original.native, selected, &history,
            &mut control, &limit(), &mut || ElapsedTick(1001),
        );
        assert!(result.is_err(), "changed contract {which} acquired live roles");
        assert_eq!(fs::read(original.config.store.join("delivery.bin")).unwrap(), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged numerical staging");
        assert!(!history.output().unwrap().exists());
        assert!(!evaluation.peer.socket_path().exists());
    }
}

#[test]
fn accounted_native_stop_at_ready_retains_original_work_and_exact_terminal_history() {
    let root = Root::new();
    let mut original = prepared(&root);
    let args = recovery_args(&root, &original, std::process::id().checked_add(100_000).unwrap());
    let (evaluation, history) = admit(&original, &args, &root);
    let (_, reviewer) = profiles(&root, &original.config);
    let mut control = Control::new(&original.config, 91, Some(&reviewer)).unwrap();
    let selected = bootstrap::selection(&original.config, &original.native, None).unwrap();
    let expected = evaluation.recovery_requirements(&original.config, &original.native, &selected).unwrap();
    let accounting = andon_settings::Settings::read(&original.accounting).unwrap();
    let before = FileOversight::read_decoder_text_progress(
        &original.config.store, &original.config.profile,
        &original.native.decoder, &original.native.tokenizer, 7,
    ).unwrap();
    let old_anchor = fs::read(&original.anchor).unwrap();
    let total = usize::try_from(original.before.revision).unwrap();
    // Recovery must need neither current evidence nor helper invocations before
    // the original authenticated terminal branch wins at the complete Ready cut.
    fs::remove_file(original.config.source.path()).unwrap();
    original.config.programs.clear();
    let mut client = None;
    let mut calls = 0;
    let result = evaluation.open(
        &original.config, &original.native, selected, &history, &mut control, &limit(),
        &mut || {
            if calls == total + 1 { client = Some(stopping(&reviewer)); }
            calls += 1;
            ElapsedTick(1001)
        },
    );
    let error = result.err().expect("Ready Stop issued a live evaluator or human role");
    assert!(error.starts_with("native admission Stop acknowledged at revision"), "{error}");
    assert!(client.unwrap().join().unwrap().acknowledged());
    let retained = archive::load(history.output().unwrap()).unwrap();
    let saved = FileOversight::read_andon_guarded(
        &original.config.store, &original.config.profile, &expected, &accounting.protocol, &retained,
    ).unwrap();
    let after = FileOversight::read_decoder_text_progress(
        &original.config.store, &original.config.profile,
        &original.native.decoder, &original.native.tokenizer, 7,
    ).unwrap();
    assert_eq!(after.publication.revision, before.publication.revision + 2);
    assert_eq!(retained.revision(), after.publication.revision);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.text.generation_revision(), before.text.generation_revision());
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(after.publication.control.ledger.elapsed, before.publication.control.ledger.elapsed);
    assert_eq!(after.publication.executions, 0);
    assert_eq!(saved.credibility.report.revision, 0);
    assert_eq!(saved.accounting.windows().len(), 2);
    assert_eq!(saved.credibility.journal.stop.as_ref().unwrap().request().operation, 91);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
    assert!(!evaluation.peer.socket_path().exists());
    assert!(!reviewer.socket(91).exists());
}

#[test]
fn accounted_native_recovery_requires_fresh_source_before_resuming_and_retains_the_fence() {
    let root = Root::new();
    let original = prepared(&root);
    let args = recovery_args(&root, &original, std::process::id().checked_add(100_000).unwrap());
    let (evaluation, history) = admit(&original, &args, &root);
    let (actor, reviewer) = profiles(&root, &original.config);
    let store = original.config.store.clone();
    let profile = original.config.profile.clone();
    let decoder = original.native.decoder.clone();
    let tokenizer = original.native.tokenizer.clone();
    let before = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
    let old_anchor = fs::read(&original.anchor).unwrap();
    fs::remove_file(original.config.source.path()).unwrap();
    let mut output = Vec::new();
    let result = serve_lifecycle_with_evaluation(
        original.config, (&actor, &reviewer), original.native,
        (true, None, None, finish::Intent::Existing),
        Services { operator: None, precheck: None, history: Some(&history), evaluation: Some(&evaluation) },
        crate::workflow::clock, &mut output,
    );
    assert!(result.is_err(), "missing fresh source resumed generated work");
    let after = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.text.generation_revision(), before.text.generation_revision());
    assert_eq!(after.publication.executions, 0);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert!(after.publication.control.ledger.epoch > before.publication.control.ledger.epoch);
    assert!(after.publication.stop.is_some(), "original failure cleanup retains its Stop");
    assert_eq!(archive::load(history.output().unwrap()).unwrap().revision(), after.publication.revision);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
    assert!(output.is_empty());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(91).exists());
    assert!(!evaluation.peer.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 91).exists());
}

#[test]
fn evaluated_native_open_requires_all_explicit_inputs_before_any_io_or_clock() {
    let root = Root::new();
    let mut valid = super::args(&root, Path::new("missing-andon"), Path::new("missing-evaluator"));
    valid[0] = "serve-open".into();
    valid.extend([
        "--history-anchor".into(), "missing-input".into(),
        "--evaluation-recovery".into(), "missing-requirements".into(),
    ]);
    assert!(evaluation::take_options(&valid).is_ok());
    for which in 0..8 {
        let mut bad = valid.clone();
        match which {
            0 => { bad[0] = "serve-create".into(); }
            1 => { bad.drain(12..14); }
            2 => { bad.truncate(14); }
            3 => { bad.drain(6..10); }
            4 => bad.extend(["--evaluation-recovery".into(), "duplicate".into()]),
            5 => bad.extend(["--after-message".into(), "01".into()]),
            6 => bad.extend([
                "--after-message".into(), "90".into(),
                "--finish-after-message".into(), "90".into(),
            ]),
            _ => bad.extend(["--continue-generation".into()]),
        }
        assert!(evaluation::take_options(&bad).is_err(), "invalid grammar {which}");
        let mut output = Vec::new();
        let error = command_with_clock(
            &bad, None, || panic!("invalid evaluated recovery read a clock"), &mut output,
        ).unwrap_err();
        assert!(error.contains("native evaluation requires"), "{error}");
        assert!(output.is_empty());
    }
}

#[test]
fn evaluated_recorded_request_refuses_receipt_only_fallback_and_retains_acknowledged_fence() {
    let root = Root::new();
    let mut original = prepared_selected(&root, true, None);
    let args = recovery_args(&root, &original, std::process::id().checked_add(100_000).unwrap());
    let (evaluation, history) = admit(&original, &args, &root);
    let (actor, reviewer) = profiles(&root, &original.config);
    let store = original.config.store.clone();
    let profile = original.config.profile.clone();
    let decoder = original.native.decoder.clone();
    let tokenizer = original.native.tokenizer.clone();
    let before = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
    let selected = bootstrap::selection(&original.config, &original.native, None).unwrap();
    let expected = evaluation.recovery_requirements(&original.config, &original.native, &selected).unwrap();
    let accounting = andon_settings::Settings::read(&original.accounting).unwrap();
    let old_anchor = fs::read(&original.anchor).unwrap();
    original.config.programs.clear();
    fs::remove_file(original.config.source.path()).unwrap();
    let mut output = Vec::new();
    let error = serve_lifecycle_with_evaluation(
        original.config, (&actor, &reviewer), original.native,
        (true, None, None, finish::Intent::Existing),
        Services { operator: None, precheck: None, history: Some(&history), evaluation: Some(&evaluation) },
        || ElapsedTick(1001), &mut output,
    ).unwrap_err();
    assert!(error.contains("recorded native requests are observation-only"), "{error}");
    let retained = archive::load(history.output().unwrap()).unwrap();
    let saved = FileOversight::read_andon_guarded(&store, &profile, &expected, &accounting.protocol, &retained).unwrap();
    let after = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
    assert_eq!(after.publication.revision, before.publication.revision + 1);
    assert_eq!(retained.revision(), after.publication.revision);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.text.generation_revision(), before.text.generation_revision());
    assert_eq!(after.publication.executions, 0);
    assert!(after.publication.stop.is_none(), "pre-listener refusal retains only the original Fence");
    assert_eq!(saved.credibility.report.revision, 0);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
    assert!(output.is_empty());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(91).exists());
    assert!(!evaluation.peer.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 91).exists());
}

fn simple_publication(root: &Root) -> PublicationProfile {
    PublicationProfile::decode(format!(
        r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}/original.bin","current":"{}/current.bin","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), root.0.display(),
    ).as_bytes()).unwrap()
}

#[test]
fn evaluated_checked_recovery_requires_the_original_publication_guard_and_limits() {
    for which in 0..3 {
        let root = Root::new();
        let mut publication = simple_publication(&root);
        let original = prepared_selected(&root, false, Some(&publication));
        let mut args = recovery_args(&root, &original, std::process::id().checked_add(100_000).unwrap());
        let publication = if which == 1 { None } else {
            if which == 2 { publication.limits.bindings += 1; }
            args[0] = "serve-open-checked".into();
            args.insert(4, "witness-profile".into());
            Some(publication)
        };
        let (evaluation, history) = admit_selected(&original, &args, &root, publication.as_ref());
        let (_, reviewer) = profiles(&root, &original.config);
        let mut control = Control::new(&original.config, 91, Some(&reviewer)).unwrap();
        let selected = bootstrap::selection(&original.config, &original.native, publication.as_ref()).unwrap();
        let canonical = fs::read(original.config.store.join("delivery.bin")).unwrap();
        let staging = original.config.store.join("delivery.pending");
        fs::write(&staging, b"retain this unacknowledged checked staging").unwrap();
        let mut calls = 0;
        let result = evaluation.open(
            &original.config, &original.native, selected, &history, &mut control, &limit(),
            &mut || {
                calls += 1;
                assert_eq!(fs::read(original.config.store.join("delivery.bin")).unwrap(), canonical);
                ElapsedTick(1001)
            },
        );
        if which == 0 {
            let (host, human, evaluator) = result.unwrap();
            assert_eq!(calls, usize::try_from(original.before.revision).unwrap() + 2);
            assert_eq!(host.revision(), original.before.revision + 1);
            assert_eq!(host.publication_validation_profile().unwrap(), publication.as_ref().map(|p| p.limits));
            assert_eq!(host.decoder_inspection().unwrap().numerical.position, 3);
            assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
            history.retain(&host, Ok(())).unwrap();
            assert_eq!(archive::load(history.output().unwrap()).unwrap().revision(), host.revision());
            assert!(!staging.exists());
            drop(evaluator);
            drop(human);
        } else {
            assert!(result.is_err(), "changed checked guard {which} acquired live roles");
            assert_eq!(fs::read(original.config.store.join("delivery.bin")).unwrap(), canonical);
            assert_eq!(fs::read(&staging).unwrap(), b"retain this unacknowledged checked staging");
            assert!(!history.output().unwrap().exists());
        }
        assert!(!evaluation.peer.socket_path().exists());
        assert!(!reviewer.socket(91).exists());
    }
}

#[test]
fn accounted_feed_recovery_pins_the_exact_change_and_snapshot_contract_before_fencing() {
    for change in ["unchanged", "omit feed", "remove fallback", "freshness", "feed identity"] {
        let root = Root::new();
        let text = checked_tests::profile_json(&root, &configured(&root), true);
        let original_profile = PublicationProfile::decode(text.as_bytes()).unwrap();
        let original = prepared_selected(&root, false, Some(&original_profile));
        let publication = match change {
            "omit feed" => simple_publication(&root),
            "remove fallback" => PublicationProfile::decode(
                checked_tests::profile_json(&root, &original.config, false).as_bytes(),
            ).unwrap(),
            "freshness" => PublicationProfile::decode(
                text.replace("\"max_age_ms\":5000", "\"max_age_ms\":5001").as_bytes(),
            ).unwrap(),
            "feed identity" => PublicationProfile::decode(
                text.replace("\"source\":41", "\"source\":42").as_bytes(),
            ).unwrap(),
            _ => original_profile,
        };
        let mut args = recovery_args(
            &root, &original, std::process::id().checked_add(100_000).unwrap(),
        );
        args[0] = "serve-open-checked".into();
        args.insert(4, "witness-profile".into());
        let (evaluation, history) = admit_selected(&original, &args, &root, Some(&publication));
        let (_, reviewer) = profiles(&root, &original.config);
        let mut control = Control::new(&original.config, 91, Some(&reviewer)).unwrap();
        let selected = bootstrap::selection(&original.config, &original.native, Some(&publication)).unwrap();
        let canonical = original.config.store.join("delivery.bin");
        let before = fs::read(&canonical).unwrap();
        let staging = original.config.store.join("delivery.pending");
        fs::write(&staging, b"keep until verified").unwrap();
        // Recovery checks the selected contract against recorded work. Live
        // witness files become necessary only when a new review asks for them.
        assert!(!root.0.join("producer").exists());
        let result = evaluation.open(
            &original.config, &original.native, selected, &history, &mut control, &limit(),
            &mut || ElapsedTick(1001),
        );
        if change == "unchanged" {
            let (host, _human, _evaluator) = result.unwrap();
            assert!(host.publication_snapshot_fallback_enabled().unwrap());
            assert_eq!(host.revision(), original.before.revision + 1);
            let progress = host.decoder_inspection().unwrap().numerical;
            assert_eq!((progress.position, progress.sampled_draws), (3, 1));
            assert_eq!(host.inspect().executions, 0);
            history.retain(&host, Ok(())).unwrap();
            assert_eq!(archive::load(history.output().unwrap()).unwrap().revision(), host.revision());
        } else {
            assert!(result.is_err(), "{change} acquired fresh roles");
            assert_eq!(fs::read(&canonical).unwrap(), before);
            assert_eq!(fs::read(staging).unwrap(), b"keep until verified");
            assert!(!history.output().unwrap().exists());
        }
    }
}
