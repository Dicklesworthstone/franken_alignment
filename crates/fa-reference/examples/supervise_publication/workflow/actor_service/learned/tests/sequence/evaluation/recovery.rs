//! Resume a resident suffix through one original accounted numerical owner.
//! Actual command, native inference, authenticated actors/humans and independent
//! evaluator processes defend composition; these fixtures do not qualify models.
use super::*;
use crate::workflow::actor_service::andon_settings;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::FileAndonRecoveryRequirements;
use fa_reference::round::review::andon::AndonBudget;

struct Original {
    config: Config,
    paths: Vec<PathBuf>,
    entries: Vec<resident::Entry>,
    anchor: PathBuf,
    before: FileDeliverySnapshot,
    pre_generation: Vec<u8>,
    evaluation_revision: u64,
}

fn prepared(root: &Root, published: usize) -> Original {
    assert!(published <= 2);
    let (mut config, paths, mut entries) = selected(root, None, None);
    let selection = select_evaluation(
        root,
        &config,
        &mut entries,
        std::process::id().checked_add(100_000).unwrap(),
        4,
    );
    let ((mut host, reviewer, observer, identity), evaluator) =
        selection.create(&config, &entries[0].loaded).unwrap();
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
    if published != 0 {
        for (index, entry) in entries[..published].iter().enumerate() {
            let loaded = &entry.loaded;
            let mut control = Control::new(&config, loaded.request, None).unwrap();
            if index != 0 {
                let command = driver
                    .supervisor()
                    .host()
                    .unwrap()
                    .prepare_learned_text_stream_continuation(
                        entries[index - 1].loaded.request,
                        loaded.request,
                    )
                    .unwrap();
                assert!(
                    crate::workflow::actor_service::learned::continuation::prepare(
                        &mut driver,
                        &reviewer,
                        &mut config,
                        command,
                        &mut control,
                        &limit,
                        &mut || ElapsedTick(1000),
                    )
                    .unwrap()
                );
            }
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
                loaded,
                &limit,
                &mut || ElapsedTick(1000),
            )
            .unwrap();
            let (reviewed, history) =
                native::review_current(&mut config, loaded, &reviewer, driver, &limit);
            driver = reviewed;
            let permit =
                native::approve(&mut driver, &mut config, &reviewer, loaded.request);
            native::publish(&mut driver, &mut config, &permit);
            assert!(executed(&mut wire, loaded.request));
            drop(history);
            let retiring = Instant::now();
            while !driver.retire_completed_request(loaded.request).unwrap() {
                assert!(retiring.elapsed() < Duration::from_millis(config.timing.cleanup_ms));
                crate::workflow::pause(config.timing.poll_ms.clamp(1, 50));
            }
        }
        let numerical = driver
            .supervisor()
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap()
            .numerical;
        assert_eq!(numerical.position, 2 + 2 * published as u64);
        assert_eq!(numerical.sampled_draws, 2 * published as u64);
    } else {
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
    let anchor = root.0.join("accounted-original.anchor");
    let (before, evaluation_revision) = {
        let host = driver.supervisor().host().unwrap();
        archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
        assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
        (host.inspect(), host.credibility_report().unwrap().revision)
    };
    assert_eq!(evaluation_revision, published as u64);
    drop(wire);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    Original {
        config,
        paths,
        entries,
        anchor,
        before,
        pre_generation,
        evaluation_revision,
    }
}

fn accounting(root: &Root, minimum: u64) -> PathBuf {
    let initial = fs::read_to_string(root.0.join("andon.json")).unwrap();
    let current = initial
        .replace("\"minimum_evaluation_revision\": 0",
            &format!("\"minimum_evaluation_revision\": {minimum}"))
        .replace("\"current_windows\": [",
            "\"current_windows\": [\n    {\"window\":2,\"start\":100,\"end\":200,\"false_stops_allowed\":2,\"cost_per_stop\":9},");
    assert_ne!(current, initial);
    let path = root.0.join("resident-recovery-andon.json");
    fs::write(&path, current).unwrap();
    path
}

fn arguments_for(
    root: &Root,
    original: &Original,
    first: usize,
    peer: &Path,
    accounting: &Path,
    modes: &[&str],
) -> Vec<String> {
    let mut rows = Vec::new();
    for (path, entry) in original.paths[first..]
        .iter()
        .zip(&original.entries[first..])
    {
        let inputs = super::super::super::mediated_topology::command_inputs(
            root,
            &original.config,
            path,
            &entry.actor,
        );
        let actor = root
            .0
            .join(format!("recovered-actor-{}.json", entry.loaded.request));
        fs::rename(&inputs[5], &actor).unwrap();
        rows.push(format!(
            r#"{{"recipe":{},"actor_profile":{},"release":"{}"}}"#,
            quote(path.to_str().unwrap()),
            quote(actor.to_str().unwrap()),
            if entry.release == LearnedTextRelease::Finish {
                "finish"
            } else {
                "message"
            }
        ));
    }
    let sequence = root.0.join("accounted-suffix.json");
    fs::write(
        &sequence,
        format!(
            r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{}]}}"#,
            rows.join(",")
        ),
    )
    .unwrap();
    let mut args = vec![
        "serve-learned-generated".into(),
        root.0.join("command-host.json").display().to_string(),
        sequence.display().to_string(),
        root.0.join("command-reviewer.json").display().to_string(),
    ];
    // Keep the unary selector before later paired options: custody discovery
    // must not shift the history/operator paths when parsing it.
    args.extend(modes.iter().map(|value| (*value).to_owned()));
    args.extend([
        "--andon".into(),
        accounting.display().to_string(),
        "--evaluator-profile".into(),
        peer.display().to_string(),
        "--history-anchor".into(),
        original.anchor.display().to_string(),
        "--retain-history-anchor".into(),
        root.0.join("recovered-final.anchor").display().to_string(),
    ]);
    args
}

fn replace_value(args: &mut [String], name: &str, value: impl Into<String>) {
    let at = args.iter().position(|arg| arg == name).unwrap();
    args[at + 1] = value.into();
}

fn configured_peer(
    root: &Root,
    original: &Original,
    first: usize,
    pid: u32,
    candidates: u32,
) -> PathBuf {
    let path = peer_file(root, &original.entries[first].actor, pid);
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original
            .replace(
                "\"candidate_limit\":2",
                &format!("\"candidate_limit\":{candidates}"),
            )
            .replace("\"runtime_ms\":15000", "\"runtime_ms\":45000"),
    )
    .unwrap();
    path
}

fn run_suffix(first: usize, decision: ReviewDecision, exhausted: bool) {
    let root = Root::new();
    let original = prepared(&root, first);
    evidence(&root, &original.config, 2, true, false);
    let old_archive = fs::read(&original.anchor).unwrap();
    let mode = if exhausted {
        "recovered-exhausted"
    } else {
        "both"
    };
    let mut child = EvaluatorChild::start(&root, mode);
    let peer = configured_peer(
        &root,
        &original,
        first,
        child.0.id(),
        if exhausted { 2 } else { 4 },
    );
    let accounting = accounting(&root, original.evaluation_revision);
    let modes: &[&str] = match first {
        0 => &["--continue-generation"],
        1 => &["--after-message", "1"],
        2 => &["--after-message", "2"],
        _ => unreachable!(),
    };
    let args = arguments_for(&root, &original, first, &peer, &accounting, modes);
    let settings = evaluator::Settings::read(&peer).unwrap();
    let mut peers = Peers::new(&root, &original.config, &original.entries);
    let mut socket_identity = None;
    let outcome = resident::command_with_clock(&args, None, || {
        use std::os::unix::fs::MetadataExt;
        if let Ok(metadata) = fs::symlink_metadata(settings.socket_path()) {
            let actual = (metadata.dev(), metadata.ino());
            if let Some(previous) = socket_identity {
                assert_eq!(previous, actual);
            } else {
                socket_identity = Some(actual);
            }
        }
        poll(
            &mut peers,
            &root,
            [
                Some(ReviewDecision::Approve),
                Some(ReviewDecision::Approve),
                Some(decision),
            ],
        );
        crate::workflow::clock()
    });
    fs::write(
        root.0.join("service.finished"),
        b"original recovered owner retired",
    )
    .unwrap();
    child.finish();
    assert_eq!(
        outcome.is_ok(),
        decision == ReviewDecision::Approve,
        "{outcome:?}"
    );
    assert!(socket_identity.is_some());
    let expected_seen = std::array::from_fn(|index| index >= first);
    assert_eq!(peers.seen_actor, expected_seen);
    assert_eq!(peers.seen_human, expected_seen);
    let (responses, packets) = peers.join();
    let mut charged = original.before.control.ledger.charged;
    let mut sessions = BTreeSet::new();
    for index in 0..3 {
        if index < first {
            assert!(responses[index].is_none() && packets[index].is_none());
            continue;
        }
        let executed = index != 2 || decision == ReviewDecision::Approve;
        assert_eq!(confirmed(responses[index].as_ref().unwrap()), executed);
        let packet = packets[index].as_ref().unwrap();
        assert_eq!(packet.views().len(), 2);
        assert_eq!(
            packet.action().spec().policy_epoch,
            original.before.control.ledger.epoch + 1
        );
        assert!(sessions.insert(packet.binding().session));
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.is_finish(), index == 2);
        assert_eq!(frame.prior_messages(), vec!["A"; index].as_slice());
        if executed {
            charged += packet.action().spec().units;
        }
    }
    let loaded = &original.entries[first].loaded;
    let snapshot = FileOversight::read_publication_with_learned_generation(
        &original.config.store,
        &original.config.profile,
        &loaded.generation,
    )
    .unwrap();
    let messages = 2;
    let finished = decision == ReviewDecision::Approve;
    assert_eq!(snapshot.executions, messages + u64::from(finished));
    assert_eq!(snapshot.payload, vec![b'A'; messages as usize]);
    assert_eq!(snapshot.control.ledger.charged, charged);
    assert_eq!(snapshot.control.ledger.reserved, 0);
    assert_eq!(
        snapshot.control.ledger.available + charged,
        original.config.profile.delivery.total
    );
    assert_eq!(
        snapshot.control.ledger.epoch,
        original.before.control.ledger.epoch + 1
    );
    assert!(snapshot.stop.is_none());
    let anchor = archive::load(&root.0.join("recovered-final.anchor")).unwrap();
    assert_eq!(anchor.revision(), snapshot.revision);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_archive);
    let minimum = if exhausted {
        assert!(root.0.join("budget.exhausted").exists());
        assert!(!root.0.join("receipt-final.json").exists());
        0
    } else {
        let final_label = read_json(&root.0.join("receipt-final.json"));
        let first_label = read_json(&root.0.join("receipt-first.json"));
        assert_eq!(
            number(
                &read_json(&root.0.join("case-first.json")),
                &["knowledge", "value", "round"]
            ),
            1
        );
        assert_eq!(
            number(
                &read_json(&root.0.join("case-final.json")),
                &["knowledge", "value", "round"]
            ),
            33
        );
        assert!(
            number(&first_label, &["before", "evaluation_revision"])
                >= original.evaluation_revision
        );
        assert_eq!(
            number(&final_label, &["after", "evaluation_revision"]),
            number(&first_label, &["after", "evaluation_revision"]) + 1
        );
        assert!(anchor.revision() >= number(&final_label, &["after", "journal_revision"]));
        number(&final_label, &["after", "evaluation_revision"])
    };
    // A separate explicit native reopen independently checks the archived
    // original accounting and complete numerical history. It performs no source
    // or model-file read; the evaluator has already removed those inputs.
    let settings = andon_settings::Settings::read(&accounting).unwrap();
    let expected = FileAndonRecoveryRequirements::new(
        recovery::requirements(&original.config, loaded),
        settings.config.clone(),
        settings.windows.clone(),
        minimum,
    )
    .unwrap();
    let (host, roles) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
        &original.config.store,
        original.config.profile.clone(),
        &expected,
        &settings.protocol,
        &loaded.generation,
        &anchor,
    )
    .unwrap();
    assert_eq!(host.revision(), snapshot.revision + 1);
    assert_eq!(host.inspect().executions, snapshot.executions);
    assert_eq!(host.inspect().control.ledger.charged, charged);
    assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
    assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(
        stream.confirmed.messages().collect::<Vec<_>>(),
        vec!["A"; messages as usize]
    );
    assert_eq!(stream.confirmed.finished(), finished);
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.pending.is_none());
    if !exhausted {
        assert_eq!(host.credibility_report().unwrap().revision, minimum);
        assert_eq!(host.credibility_report().unwrap().benign_origins, 2);
    }
    drop(roles);
    drop(host);
    assert!(
        !evaluator::Settings::read(&peer)
            .unwrap()
            .socket_path()
            .exists()
    );
    quiescent(&root);
}

#[test]
fn accounted_resident_continuation_retains_original_draws_and_pending_case_labels() {
    run_suffix(0, ReviewDecision::Approve, false);
}

#[test]
fn accounted_resident_suffix_retains_original_accounting_through_message_and_finish() {
    run_suffix(1, ReviewDecision::Approve, false);
}

#[test]
fn accounted_singleton_finish_needs_no_new_draw_but_requires_fresh_human_approval() {
    run_suffix(2, ReviewDecision::Approve, false);
    run_suffix(2, ReviewDecision::Reject, false);
}

#[test]
fn accounted_recovered_evaluator_candidates_are_not_renewed_at_the_next_message() {
    run_suffix(1, ReviewDecision::Approve, true);
}

#[test]
fn accounted_resident_recovery_rejects_rollback_missing_windows_and_higher_floor_without_mutation()
{
    for fault in 0..3 {
        let root = Root::new();
        let original = prepared(&root, 1);
        let peer = configured_peer(
            &root,
            &original,
            1,
            std::process::id().checked_add(100_000).unwrap(),
            4,
        );
        let accounting = accounting(&root, original.evaluation_revision + u64::from(fault == 2));
        let mut args = arguments_for(
            &root,
            &original,
            1,
            &peer,
            &accounting,
            &["--after-message", "1"],
        );
        if fault == 0 {
            fs::write(
                original.config.store.join("delivery.bin"),
                &original.pre_generation,
            )
            .unwrap();
        } else if fault == 1 {
            replace_value(
                &mut args,
                "--andon",
                root.0.join("andon.json").display().to_string(),
            );
        }
        let original_bytes = fs::read(original.config.store.join("delivery.bin")).unwrap();
        let old_anchor = fs::read(&original.anchor).unwrap();
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(resident::command_with_clock(&args, None, || ElapsedTick(1000)).is_err());
        assert_eq!(
            fs::read(original.config.store.join("delivery.bin")).unwrap(),
            original_bytes
        );
        assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
        assert!(!root.0.join("recovered-final.anchor").exists());
        assert!(
            !evaluator::Settings::read(&peer)
                .unwrap()
                .socket_path()
                .exists()
        );
        assert!(!original.config.store.join("delivery.pending").exists());
        quiescent(&root);
    }
}

#[test]
fn accounted_resident_replay_stop_wins_before_live_roles_at_preopen_midpoint_and_ready() {
    for boundary in 0..3 {
        let root = Root::new();
        let original = prepared(&root, 0);
        let reviewer = fixture::peers(&root, &original.config);
        let peer = configured_peer(
            &root,
            &original,
            0,
            std::process::id().checked_add(100_000).unwrap(),
            4,
        );
        let accounting = accounting(&root, 0);
        let args = arguments_for(
            &root,
            &original,
            0,
            &peer,
            &accounting,
            &["--continue-generation"],
        );
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let total = usize::try_from(original.before.revision).unwrap();
        let trigger = [2, 2 + total / 2, total + 3][boundary];
        let mut calls = 0;
        let mut client = None;
        let error = resident::command_with_clock(&args, None, || {
            calls += 1;
            if calls == trigger {
                let mut peer = reviewer.connect_stop_client(1).unwrap();
                client = Some(thread::spawn(move || {
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
            .expect("original recovery Stop listener")
            .join()
            .unwrap();
        assert!(receipt.acknowledged() && !receipt.drained());
        let current = FileOversight::read_publication_with_learned_generation(
            &original.config.store,
            &original.config.profile,
            &original.entries[0].loaded.generation,
        )
        .unwrap();
        assert_eq!(current.revision, original.before.revision + 2);
        assert_eq!(current.executions, original.before.executions);
        assert_eq!(current.payload, original.before.payload);
        assert_eq!(
            current.control.ledger.charged,
            original.before.control.ledger.charged
        );
        assert_eq!(
            archive::load(&root.0.join("recovered-final.anchor"))
                .unwrap()
                .revision(),
            current.revision
        );
        assert!(
            !evaluator::Settings::read(&peer)
                .unwrap()
                .socket_path()
                .exists()
        );
        for entry in &original.entries {
            assert!(!entry.actor.socket.exists());
            assert!(!reviewer.socket(entry.loaded.request).exists());
        }
        quiescent(&root);
    }
}

#[test]
fn accounted_resident_recovery_admits_exact_modes_and_preserves_unary_option_boundaries() {
    let root = Root::new();
    let fresh = arguments(
        &root,
        Path::new("not-read-andon"),
        Path::new("not-read-evaluator"),
    );
    for mode in [vec!["--continue-generation"], vec!["--after-message", "7"]] {
        let mut valid = fresh[..4].to_vec();
        valid.extend(mode.iter().map(|value| (*value).to_owned()));
        valid.extend_from_slice(&fresh[4..]);
        valid.extend(["--history-anchor".into(), "not-read.anchor".into()]);
        let (remaining, options) =
            selected_evaluation::take_sequence_options(&valid, None).unwrap();
        assert!(options.is_some());
        assert!(remaining.iter().any(|value| value == mode[0]));
        for fault in 0..8 {
            let mut invalid = valid.clone();
            match fault {
                0 | 1 => {
                    let flag = if fault == 0 {
                        "--history-anchor"
                    } else {
                        "--retain-history-anchor"
                    };
                    let at = invalid.iter().position(|value| value == flag).unwrap();
                    invalid.drain(at..at + 2);
                }
                2 => invalid.push("--continue-generation".into()),
                3 => invalid.extend(["--after-message".into(), "0".into()]),
                4 => invalid.extend(["--after-message".into(), "01".into()]),
                5 => invalid.extend(["--finish-after-message".into(), "7".into()]),
                6 => invalid.extend(["--topology-profile".into(), "unsupported".into()]),
                _ => invalid.extend(["--operator-profile".into(), "missing-generation".into()]),
            }
            assert!(
                resident::command_with_clock(&invalid, None, || panic!(
                    "invalid recovery grammar sampled time"
                ))
                .is_err()
            );
            assert!(!root.0.join("store").exists());
        }
    }
}

#[test]
fn recovered_history_cannot_become_a_future_actor_endpoint_or_retained_output() {
    for fault in 0..3 {
        let root = Root::new();
        let mut original = prepared(&root, 1);
        let peer = configured_peer(
            &root,
            &original,
            1,
            std::process::id().checked_add(100_000).unwrap(),
            4,
        );
        let accounting = accounting(&root, original.evaluation_revision);
        if fault == 0 {
            original.entries[2].actor.socket = original.anchor.clone();
        } else if fault == 1 {
            let alias = root.0.join("source-alias");
            std::os::unix::fs::symlink(&root.0, &alias).unwrap();
            original.entries[2].actor.socket = alias.join("accounted-original.anchor");
        }
        let mut args = arguments_for(
            &root,
            &original,
            1,
            &peer,
            &accounting,
            &["--after-message", "1"],
        );
        if fault == 2 {
            replace_value(
                &mut args,
                "--retain-history-anchor",
                original.anchor.display().to_string(),
            );
        }
        let journal = fs::read(original.config.store.join("delivery.bin")).unwrap();
        let history = fs::read(&original.anchor).unwrap();
        // Complete path-role admission must reject this before any original
        // model bytes are read, even for a later entry's endpoint.
        fs::remove_file(root.0.join("weights.safetensors")).unwrap();
        let error = resident::command_with_clock(&args, None, || {
            panic!("conflicting history custody sampled time")
        })
        .unwrap_err();
        assert!(error.contains("conflicts with"), "{error}");
        assert_eq!(
            fs::read(original.config.store.join("delivery.bin")).unwrap(),
            journal
        );
        assert_eq!(fs::read(&original.anchor).unwrap(), history);
        assert!(!root.0.join("recovered-final.anchor").exists());
        assert!(
            !evaluator::Settings::read(&peer)
                .unwrap()
                .socket_path()
                .exists()
        );
        quiescent(&root);
    }
}
