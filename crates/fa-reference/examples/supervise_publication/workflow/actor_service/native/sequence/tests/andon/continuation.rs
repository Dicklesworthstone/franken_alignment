//! Resume an original pending or complete-unsubmitted native text command.
//! Numerical draws, registered accounting windows and independent effect keys
//! survive one recovery Fence; no Message predecessor or new intent is invented.
use super::super::super::evaluation::Recovery;
use super::*;
use super::super::precheck_tests;
use crate::workflow::actor_service::learned::anchors as archive;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    evaluation::andon::FileAndonRecoveryRequirements,
};
use fa_reference::round::review::andon::AndonBudget;

struct Pending {
    config: Config,
    entries: Vec<Entry>,
    reviewer: PeerProfile,
    reader: Reader,
    accounting: andon_settings::Settings,
    anchor: PathBuf,
    before: FileDeliverySnapshot,
}

fn begin(host: &mut FileOversight, inputs: &Inputs, generation: u64, steps: u64) {
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        generation,
        numerical.actor_revision,
        numerical.position,
        inputs.request.clone(),
    )
    .unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    for completed in 0..steps {
        host.advance_decoder_text(host.revision(), generation, completed)
            .unwrap();
    }
}

fn prepared(root: &Root, steps: u64, earlier_generation: bool) -> Pending {
    assert!(matches!(steps, 3 | 4));
    let (mut config, entries, reviewer) = selected(root, true);
    let reader = Reader::new(&config, &entries[0]);
    let mut accounting = settings();
    let selected = bootstrap::selection(&config, &entries[0].inputs, None).unwrap();
    let (mut host, human, evaluator) =
        bootstrap::prepare_andon(&config, &entries[0].inputs, selected, &accounting).unwrap();
    let later = AndonBudget {
        window: 2,
        start: 100,
        end: 200,
        false_stops_allowed: 2,
        cost_per_stop: 9,
    };
    host.register_andon_window(host.revision(), later).unwrap();
    accounting.windows.push(later);
    let now = crate::workflow::clock();
    host.refresh_file_source(host.revision(), &mut config.source, now)
        .unwrap();
    host.observe_time(host.revision(), now).unwrap();
    if earlier_generation {
        // A real, distinct historical generation makes a later ID collision
        // pass manifest uniqueness and reach the whole-history preflight.
        begin(&mut host, &entries[0].inputs, 6, 4);
    }
    begin(&mut host, &entries[0].inputs, 7, steps);
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert_eq!(
        numerical.position,
        steps + if earlier_generation { 4 } else { 0 }
    );
    assert_eq!(
        numerical.sampled_draws,
        steps - 2 + if earlier_generation { 2 } else { 0 }
    );
    assert_eq!(
        host.decoder_text_progress(7).unwrap().bytes().unwrap(),
        b"A"
    );
    assert!(
        crate::workflow::actor_service::native::recovery::select(&host, 91, &entries[0].inputs,)
            .unwrap()
            .is_none(),
        "the first selected intent is valid and has no recorded actor request"
    );
    assert_eq!(host.credibility_report().unwrap().revision, 0);
    let anchor = root.0.join("original-pending.anchor");
    archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
    let before = host.inspect();
    assert_eq!(before.executions, 0);
    assert_eq!(before.control.ledger.charged, 0);
    drop(evaluator);
    drop(human);
    drop(host);
    config.programs = fixture::native_programs_at_epoch(root, 1);
    Pending {
        config,
        entries,
        reviewer,
        reader,
        accounting,
        anchor,
        before,
    }
}

fn selections(root: &Root, pending: &Pending) -> (Recovery, anchors::Selection) {
    let requirements = root.0.join("continue-recovery.json");
    fs::write(&requirements, format!(
        r#"{{"schema":"fa.native-evaluation-recovery/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}}}"#,
        pending.before.revision, pending.before.control.sequence, pending.before.control.ledger.epoch,
    )).unwrap();
    let recovery = Recovery::read(&requirements, &pending.config).unwrap();
    // The accounting parser owns and removes the unary continuation selector;
    // anchor admission receives its explicit Mode separately from these pairs.
    let args = vec![
        "serve-native-generated".into(),
        "config".into(),
        "sequence".into(),
        "reviewer".into(),
        "--history-anchor".into(),
        pending.anchor.display().to_string(),
        "--retain-history-anchor".into(),
        root.0.join("continued.anchor").display().to_string(),
    ];
    let history = anchors::take_options(&args)
        .unwrap()
        .1
        .admit_mode(
            &pending.config,
            &pending
                .entries
                .iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            &pending.reviewer,
            None,
            Mode::ContinueGeneration,
            None,
        )
        .unwrap();
    (recovery, history)
}

fn expected(pending: &Pending) -> FileAndonRecoveryRequirements {
    FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(pending.entries[0].inputs.stream),
                decoder: Some(pending.entries[0].inputs.decoder.clone()),
                decoder_stop: None,
                source: Some(pending.config.source_policy),
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: pending.config.profile.delivery.policy.clone(),
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: pending.before.revision,
                control_sequence: pending.before.control.sequence,
                authority_epoch: pending.before.control.ledger.epoch,
            },
        },
        pending.accounting.config.clone(),
        pending.accounting.windows.clone(),
        0,
    )
    .unwrap()
}

fn run_pending(steps: u64, earlier_generation: bool, singleton: bool, approve: bool) {
    run_pending_selected(steps, earlier_generation, singleton, approve, false);
}

fn run_pending_selected(
    steps: u64,
    earlier_generation: bool,
    singleton: bool,
    approve: bool,
    prechecked: bool,
) {
    let root = Root::new();
    let mut pending = prepared(&root, steps, earlier_generation);
    if singleton {
        pending.entries.truncate(1);
    }
    let precheck_sockets = if prechecked {
        precheck_tests::select(&root, &mut pending.entries, 1)
    } else {
        Vec::new()
    };
    let (recovery, history) = selections(&root, &pending);
    let expected = expected(&pending);
    let old_anchor = fs::read(&pending.anchor).unwrap();
    let before = pending.reader.progress(7);
    let mut peer = EvaluatorProcess::start(
        &root,
        &pending.entries[0].actor,
        if !singleton && approve { 4 } else { 2 },
    );
    let evaluator = peer.selected.clone();
    let requests = if approve && !singleton {
        vec![91, 92, 93]
    } else {
        vec![91]
    };
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(
        pending.reviewer.clone(),
        requests.clone(),
        offered,
        decisions,
    );
    let store = pending.reader.store.clone();
    let profile = pending.reader.profile.clone();
    let retired_prechecks = precheck_sockets.clone();
    let independent = thread::spawn(move || {
        let mut inode = None;
        for (index, request) in requests.into_iter().enumerate() {
            let (actual, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(actual, request);
            assert_eq!(packet.action().spec().policy_epoch, 1);
            if prechecked {
                assert!(
                    retired_prechecks[index].as_ref().is_none_or(|path| !path.exists()),
                    "a release disclosure retained custody at the fresh human boundary"
                );
            }
            let current = fs::metadata(peer.selected.socket_path()).unwrap().ino();
            assert_eq!(*inode.get_or_insert(current), current);
            assert_eq!(
                FileOversight::read_publication(&store, &profile)
                    .unwrap()
                    .executions,
                index as u64
            );
            if request != 92 {
                let (accepted, receipt) = peer.operation("label", request);
                assert!(accepted, "{}", String::from_utf8_lossy(&receipt));
                assert_eq!(
                    field(&parse(&receipt), &["status"]).as_str(),
                    Some("applied")
                );
            }
            assert_eq!(
                FileOversight::read_publication(&store, &profile)
                    .unwrap()
                    .executions,
                index as u64,
                "a resumed numerical result and independent label are not the new human key"
            );
            release
                .send(Some(if approve {
                    ReviewDecision::Approve
                } else {
                    ReviewDecision::Reject
                }))
                .unwrap();
        }
        peer.finish();
    });
    let mut output = Output::new(&pending.entries);
    let profiles = output.profiles.clone();
    let mut checked_output = prechecked.then(|| {
        precheck_tests::CheckedOutput::live(&pending.entries, &pending.reader, &pending.reviewer, 1)
    });
    let mut writer: &mut dyn Write = match checked_output.as_mut() {
        Some(checked) => checked,
        None => &mut output,
    };
    let mut saw_first = false;
    let offset = if earlier_generation { 4 } else { 0 };
    let prior_draws = if earlier_generation { 2 } else { 0 };
    let result = run_with_andon_continuation(
        pending.config,
        pending.entries,
        &pending.reviewer,
        None,
        None,
        &history,
        &pending.accounting,
        &evaluator,
        &recovery,
        || {
            if !saw_first && pending.reviewer.socket(91).exists() {
                let current = pending.reader.progress(7);
                assert_eq!(current.text.command(), before.text.command());
                assert_eq!(current.text.generation_revision(), 4);
                assert_eq!(current.numerical.numerical.position, offset + 4);
                assert_eq!(current.numerical.numerical.sampled_draws, prior_draws + 2);
                if steps == 4 {
                    assert_eq!(
                        current.numerical.numerical, before.numerical.numerical,
                        "completed-unsubmitted recovery performed new numerical work"
                    );
                }
                saw_first = true;
            }
            crate::workflow::clock()
        },
        &mut writer,
    );
    assert_eq!(result.is_ok(), approve, "{result:?}");
    assert!(saw_first);
    independent.join().unwrap();
    let packets = human.join().unwrap();
    let replies = match checked_output {
        Some(checked) => checked.join(),
        None => output.join(),
    };
    let completed = if approve && !singleton { 3 } else { 1 };
    assert_eq!(packets.len(), completed);
    assert_eq!(replies.len(), completed);
    let mut sessions = BTreeSet::new();
    let mut charged = 0;
    for (index, (packet, response)) in packets.iter().zip(&replies).enumerate() {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.views().len(), 2);
        assert_eq!(
            packet.action().spec().policy_epoch,
            pending.before.control.ledger.epoch + 1
        );
        assert_eq!(executed(response), approve);
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; index].as_slice());
        assert_eq!(frame.is_finish(), index == 2);
        if approve {
            charged += packet.action().spec().units;
        }
    }
    let current = pending.reader.stream();
    assert_eq!(
        current.publication.executions,
        if approve { completed as u64 } else { 0 }
    );
    assert_eq!(
        current.publication.payload,
        if !approve {
            b"".as_slice()
        } else if singleton {
            b"A".as_slice()
        } else {
            b"AA".as_slice()
        }
    );
    assert_eq!(
        current.publication.control.ledger.epoch,
        pending.before.control.ledger.epoch + 1
    );
    assert_eq!(current.publication.control.ledger.charged, charged);
    assert_eq!(current.publication.control.ledger.reserved, 0);
    assert!(current.publication.stop.is_none());
    assert_eq!(current.confirmed, current.published);
    assert_eq!(current.confirmed.finished(), approve && !singleton);
    assert!(current.pending.is_none());
    let original = pending.reader.progress(7);
    assert_eq!(original.text.command(), before.text.command());
    assert_eq!(original.text.generation_revision(), 4);
    if approve && !singleton {
        let final_generation = pending.reader.progress(8);
        assert_eq!(final_generation.text.command().position(), offset + 4);
        assert_eq!(final_generation.numerical.numerical.position, offset + 8);
        assert_eq!(
            final_generation.numerical.numerical.sampled_draws,
            prior_draws + 4
        );
        pending.reader.no_generation(9);
    } else {
        assert_eq!(original.numerical.numerical.position, offset + 4);
        assert_eq!(original.numerical.numerical.sampled_draws, prior_draws + 2);
        pending.reader.no_generation(8);
    }
    let retained = archive::load(history.output_path().unwrap()).unwrap();
    let saved = FileOversight::read_andon_guarded(
        &pending.reader.store,
        &pending.reader.profile,
        &expected,
        &pending.accounting.protocol,
        &retained,
    )
    .unwrap();
    assert_eq!(saved.credibility.report.retained_cases, completed);
    let labels = if completed == 3 { 2 } else { 1 };
    assert_eq!(saved.credibility.report.benign_origins, labels);
    assert_eq!(
        saved.credibility.report.pending_cases,
        completed - labels as usize
    );
    assert_eq!(saved.credibility.report.revision, completed as u64 + labels);
    assert_eq!(saved.accounting.windows().len(), 2);
    assert_eq!(retained.revision(), current.publication.revision);
    assert_eq!(fs::read(&pending.anchor).unwrap(), old_anchor);
    assert!(!evaluator.socket_path().exists());
    assert!(
        precheck_sockets
            .iter()
            .all(|path| path.as_ref().is_none_or(|path| !path.exists()))
    );
    closed(&profiles, &pending.reviewer);
}

#[test]
fn accounted_resident_continuation_resumes_partial_and_completed_original_intents() {
    run_pending(3, false, false, true);
    run_pending(4, false, false, true);
    run_pending(3, true, false, true);
}

#[test]
fn accounted_continuation_accepts_a_single_message_but_never_invents_its_human_key() {
    run_pending(3, false, true, true);
    run_pending(3, false, false, false);
}

#[test]
fn accounted_continuation_prechecks_retain_partial_and_completed_work_and_fresh_human_approval() {
    run_pending_selected(3, false, false, true, true);
    run_pending_selected(4, false, true, true, true);
    run_pending_selected(3, false, false, false, true);
}

fn inactive_peer(root: &Root, entry: &Entry) -> evaluator::Settings {
    evaluator::Settings::decode(
        peer_profile(
            root,
            &entry.actor,
            std::process::id().checked_add(100_000).unwrap(),
            4,
        )
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn accounted_pending_precheck_epoch_matches_the_recovered_owner_before_source_or_resume() {
    for selected_epoch in [0, 2] {
        let root = Root::new();
        let mut pending = prepared(&root, 3, false);
        let sockets = precheck_tests::select(&root, &mut pending.entries, selected_epoch);
        let (recovery, history) = selections(&root, &pending);
        let expected = expected(&pending);
        let before = pending.reader.progress(7);
        let old_anchor = fs::read(&pending.anchor).unwrap();
        let evaluator = inactive_peer(&root, &pending.entries[0]);
        let mut output = Output::new(&pending.entries);
        let profiles = output.profiles.clone();
        fs::remove_file(pending.config.source.path()).unwrap();
        let error = run_with_andon_continuation(
            pending.config,
            pending.entries,
            &pending.reviewer,
            None,
            None,
            &history,
            &pending.accounting,
            &evaluator,
            &recovery,
            crate::workflow::clock,
            &mut output,
        )
        .unwrap_err();
        assert!(error.contains("policy epoch"), "{error}");
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        let current = pending.reader.publication();
        assert_eq!(current.revision, pending.before.revision + 1);
        assert_eq!(
            current.control.ledger.epoch,
            pending.before.control.ledger.epoch + 1
        );
        assert_eq!(
            current.control.ledger.elapsed,
            pending.before.control.ledger.elapsed
        );
        assert_eq!(current.executions, 0);
        assert_eq!(current.control.ledger.charged, 0);
        assert!(current.stop.is_none());
        let after = pending.reader.progress(7);
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(after.text.generation_revision(), 3);
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        pending.reader.no_generation(8);
        let retained = archive::load(history.output_path().unwrap()).unwrap();
        let saved = FileOversight::read_andon_guarded(
            &pending.reader.store,
            &pending.reader.profile,
            &expected,
            &pending.accounting.protocol,
            &retained,
        )
        .unwrap();
        assert_eq!(saved.credibility.report.revision, 0);
        assert_eq!(saved.credibility.report.retained_cases, 0);
        assert_eq!(saved.accounting.windows().len(), 2);
        assert_eq!(retained.revision(), current.revision);
        assert_eq!(fs::read(&pending.anchor).unwrap(), old_anchor);
        assert!(!evaluator.socket_path().exists());
        assert!(
            sockets
                .iter()
                .all(|path| path.as_ref().is_none_or(|path| !path.exists()))
        );
        closed(&profiles, &pending.reviewer);
    }
}

#[test]
fn accounted_continuation_refuses_changed_intent_and_later_historical_generation_before_work() {
    for fault in 0..3 {
        let root = Root::new();
        let mut pending = prepared(&root, 3, fault == 2);
        if fault == 2 {
            pending.entries.truncate(2);
            pending.entries[1].inputs.generation = 6;
        } else {
            if fault == 0 {
                fs::write(root.0.join("prompt.txt"), b"ac").unwrap();
            } else {
                let recipe = fs::read_to_string(root.0.join("recipe.json")).unwrap();
                assert!(recipe.contains("\"max_new_tokens\":2"));
                fs::write(
                    root.0.join("recipe.json"),
                    recipe.replace("\"max_new_tokens\":2", "\"max_new_tokens\":3"),
                )
                .unwrap();
            }
            pending.entries[0].inputs = recipe::load(
                &root.0.join("recipe.json"),
                pending.config.profile.delivery.scope.tenant,
                pending.config.profile.delivery.limits.bytes,
            )
            .unwrap();
        }
        let (recovery, history) = selections(&root, &pending);
        let before = pending.reader.progress(7);
        let old_anchor = fs::read(&pending.anchor).unwrap();
        let evaluator = inactive_peer(&root, &pending.entries[0]);
        let mut output = Output::new(&pending.entries);
        let profiles = output.profiles.clone();
        fs::remove_file(pending.config.source.path()).unwrap();
        let error = run_with_andon_continuation(
            pending.config,
            pending.entries,
            &pending.reviewer,
            None,
            None,
            &history,
            &pending.accounting,
            &evaluator,
            &recovery,
            || {
                assert!(!evaluator.socket_path().exists());
                crate::workflow::clock()
            },
            &mut output,
        )
        .unwrap_err();
        assert!(
            error.contains(if fault == 2 {
                "already exists"
            } else {
                "original text intent"
            }),
            "{error}"
        );
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        let current = pending.reader.publication();
        assert_eq!(current.revision, pending.before.revision + 1);
        assert_eq!(
            current.control.ledger.epoch,
            pending.before.control.ledger.epoch + 1
        );
        assert_eq!(
            current.control.ledger.elapsed,
            pending.before.control.ledger.elapsed
        );
        assert_eq!(current.executions, 0);
        assert_eq!(current.control.ledger.charged, 0);
        assert!(current.stop.is_none());
        let after = pending.reader.progress(7);
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(
            after.text.generation_revision(),
            before.text.generation_revision()
        );
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        pending.reader.no_generation(8);
        assert_eq!(
            archive::load(history.output_path().unwrap())
                .unwrap()
                .revision(),
            current.revision
        );
        assert_eq!(fs::read(&pending.anchor).unwrap(), old_anchor);
        closed(&profiles, &pending.reviewer);
    }
}

#[test]
fn accounted_pending_ready_stop_preserves_the_sampled_prefix_and_registered_windows() {
    let root = Root::new();
    let pending = prepared(&root, 3, false);
    let (recovery, history) = selections(&root, &pending);
    let expected = expected(&pending);
    let evaluator = inactive_peer(&root, &pending.entries[0]);
    let before = pending.reader.progress(7);
    let canonical = fs::read(pending.reader.store.join("delivery.bin")).unwrap();
    let old_anchor = fs::read(&pending.anchor).unwrap();
    let total = usize::try_from(pending.before.revision).unwrap();
    fs::remove_file(pending.config.source.path()).unwrap();
    let mut output = Output::new(&pending.entries);
    let profiles = output.profiles.clone();
    let mut controller = None;
    let mut calls = 0;
    let error = run_with_andon_continuation(
        pending.config,
        pending.entries,
        &pending.reviewer,
        None,
        None,
        &history,
        &pending.accounting,
        &evaluator,
        &recovery,
        || {
            if calls == total + 2 {
                assert_eq!(
                    fs::read(pending.reader.store.join("delivery.bin")).unwrap(),
                    canonical
                );
                assert!(!evaluator.socket_path().exists());
                assert!(!pending.reviewer.socket(91).exists());
                let mut client = pending.reviewer.connect_stop_client(91).unwrap();
                controller = Some(thread::spawn(move || {
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(20));
                        match client.step().unwrap() {
                            StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                            StopClientProgress::Complete => return client.receipt().unwrap(),
                            _ => pause(1),
                        }
                    }
                }));
            }
            calls += 1;
            crate::workflow::clock()
        },
        &mut output,
    )
    .unwrap_err();
    assert!(
        error.starts_with("native admission Stop acknowledged at revision"),
        "{error}"
    );
    let receipt = controller
        .expect("original pending replay reached Ready")
        .join()
        .unwrap();
    assert!(receipt.acknowledged());
    assert_eq!(receipt.binding.operation, 91);
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    let after = pending.reader.progress(7);
    assert_eq!(after.publication.revision, pending.before.revision + 2);
    assert_eq!(after.text.command(), before.text.command());
    assert_eq!(after.text.generation_revision(), 3);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(
        after.publication.control.ledger.elapsed,
        pending.before.control.ledger.elapsed
    );
    assert_eq!(after.publication.executions, 0);
    assert_eq!(after.publication.control.ledger.charged, 0);
    pending.reader.no_generation(8);
    let retained = archive::load(history.output_path().unwrap()).unwrap();
    let saved = FileOversight::read_andon_guarded(
        &pending.reader.store,
        &pending.reader.profile,
        &expected,
        &pending.accounting.protocol,
        &retained,
    )
    .unwrap();
    assert_eq!(saved.credibility.report.revision, 0);
    assert_eq!(saved.credibility.report.retained_cases, 0);
    assert_eq!(saved.accounting.windows().len(), 2);
    assert_eq!(retained.revision(), after.publication.revision);
    assert_eq!(fs::read(&pending.anchor).unwrap(), old_anchor);
    assert!(!evaluator.socket_path().exists());
    closed(&profiles, &pending.reviewer);
}

#[test]
fn accounted_continuation_grammar_is_explicit_closed_and_preserves_unary_option_boundaries() {
    let base = [
        "serve-native-generated",
        "missing-config",
        "missing-sequence",
        "missing-reviewer",
        "--continue-generation",
        "--andon",
        "missing-andon",
        "--evaluator-profile",
        "missing-evaluator",
        "--history-anchor",
        "original-input",
        "--retain-history-anchor",
        "new-output",
        "--evaluation-recovery",
        "missing-requirements",
    ]
    .map(str::to_owned)
    .to_vec();
    assert!(andon::take_options(&base, None).unwrap().1.is_some());
    let missing = Config::read(Path::new("missing-config")).err().unwrap();
    assert_eq!(
        profile::command_with_clock(
            &base,
            None,
            || panic!("missing config acquired time"),
            &mut Vec::new()
        )
        .unwrap_err(),
        missing
    );
    for flag in [
        "--andon",
        "--evaluator-profile",
        "--history-anchor",
        "--retain-history-anchor",
        "--evaluation-recovery",
    ] {
        let mut invalid = base.clone();
        let index = invalid.iter().position(|value| value == flag).unwrap();
        invalid.drain(index..index + 2);
        assert!(
            andon::take_options(&invalid, None).is_err(),
            "omitted {flag}"
        );
    }
    for extra in [
        vec!["--continue-generation"],
        vec!["--after-message", "92"],
        vec!["--finish-after-message", "92"],
        vec!["--governor-profile", "governor"],
        vec!["--precheck-profile", "precheck"],
        vec!["--topology-profile", "topology"],
    ] {
        let mut invalid = base.clone();
        invalid.extend(extra.into_iter().map(str::to_owned));
        assert!(andon::take_options(&invalid, None).is_err());
    }
    let mut implicit = base;
    implicit.remove(4);
    assert!(andon::take_options(&implicit, None).is_err());
}
