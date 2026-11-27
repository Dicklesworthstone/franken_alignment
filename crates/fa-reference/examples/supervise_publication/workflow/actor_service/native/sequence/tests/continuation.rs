//! Ordinary resident recovery of an actual interrupted native text intent.
//! Synthetic weights establish workflow and accounting behavior, not deployment
//! quality. Each resumed Message/Finish still uses real helper and peer paths.
use super::*;
use crate::workflow::actor_service::learned::anchors as archive;
use std::fs;

struct Pending {
    config: Config,
    entries: Vec<Entry>,
    reviewer: PeerProfile,
    reader: Reader,
    anchor: PathBuf,
    before: FileDeliverySnapshot,
    before_intent: Vec<u8>,
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
    let selected = bootstrap::selection(&config, &entries[0].inputs, None).unwrap();
    let (mut host, human) =
        bootstrap::prepare(&config, &entries[0].inputs, selected, false).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000))
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    if earlier_generation {
        // Preserve real work predating the selected interrupted intent.
        begin(&mut host, &entries[0].inputs, 6, 4);
    }
    let before_intent = fs::read(config.store.join("delivery.bin")).unwrap();
    begin(&mut host, &entries[0].inputs, 7, steps);
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert_eq!(numerical.position, steps + if earlier_generation { 4 } else { 0 });
    assert_eq!(numerical.sampled_draws, steps - 2 + if earlier_generation { 2 } else { 0 });
    assert_eq!(host.decoder_text_progress(7).unwrap().bytes().unwrap(), b"A");
    assert!(
        crate::workflow::actor_service::native::recovery::select(
            &host, 91, &entries[0].inputs,
        )
        .unwrap()
        .is_none()
    );
    let anchor = root.0.join("pending.anchor");
    archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
    let before = host.inspect();
    assert_eq!(before.executions, 0);
    assert_eq!(before.control.ledger.charged, 0);
    drop(human);
    drop(host);
    config.programs = fixture::native_programs_at_epoch(root, 1);
    Pending { config, entries, reviewer, reader, anchor, before, before_intent }
}

fn history(root: &Root, pending: &Pending) -> anchors::Selection {
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
    anchors::take_options(&args)
        .unwrap()
        .1
        .admit_mode(
            &pending.config,
            &pending.entries.iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            &pending.reviewer,
            None,
            Mode::ContinueGeneration,
            None,
        )
        .unwrap()
}

fn run_pending(steps: u64, earlier_generation: bool, singleton: bool, approve: bool) {
    let root = Root::new();
    let mut pending = prepared(&root, steps, earlier_generation);
    if singleton {
        pending.entries.truncate(1);
    }
    let history = history(&root, &pending);
    let old_anchor = fs::read(&pending.anchor).unwrap();
    let before = pending.reader.progress(7);
    let mut output = Output::new(&pending.entries);
    let profiles = output.profiles.clone();
    let decisions = if approve {
        [Some(ReviewDecision::Approve); 3]
    } else {
        [Some(ReviewDecision::Reject), None, None]
    };
    let mut humans = Humans::new(&pending.reviewer, &pending.reader, decisions);
    let offset = if earlier_generation { 4 } else { 0 };
    let prior_draws = if earlier_generation { 2 } else { 0 };
    let mut saw_first = false;
    let result = run_with_continuation(
        pending.config,
        pending.entries,
        &pending.reviewer,
        None,
        None,
        &history,
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
                assert_eq!(current.publication.executions, 0);
                saw_first = true;
            }
            humans.poll();
            ElapsedTick(1001)
        },
        &mut output,
    );
    assert_eq!(result.is_ok(), approve, "{result:?}");
    assert!(saw_first);
    let packets = humans.join().into_iter().flatten().collect::<Vec<_>>();
    let replies = output.join();
    let count = if approve && !singleton { 3 } else { 1 };
    assert_eq!(packets.len(), count);
    assert_eq!(replies.len(), count);
    let mut sessions = BTreeSet::new();
    let mut charged = 0;
    for (index, (packet, response)) in packets.iter().zip(&replies).enumerate() {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.views().len(), 2);
        assert_eq!(packet.action().spec().policy_epoch, pending.before.control.ledger.epoch + 1);
        assert_eq!(executed(response), approve);
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; index].as_slice());
        assert_eq!(frame.message(), if index == 2 { None } else { Some("A") });
        assert_eq!(frame.is_finish(), index == 2);
        if approve {
            charged += packet.action().spec().units;
        }
    }
    let current = pending.reader.stream();
    assert_eq!(current.publication.executions, if approve { count as u64 } else { 0 });
    assert_eq!(
        current.publication.payload,
        if !approve { b"".as_slice() } else if singleton { b"A".as_slice() } else { b"AA".as_slice() }
    );
    assert_eq!(current.publication.control.ledger.epoch, pending.before.control.ledger.epoch + 1);
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
        let last = pending.reader.progress(8);
        assert_eq!(last.text.command().position(), offset + 4);
        assert_eq!(last.numerical.numerical.position, offset + 8);
        assert_eq!(last.numerical.numerical.sampled_draws, prior_draws + 4);
        pending.reader.no_generation(9);
    } else {
        assert_eq!(original.numerical.numerical.position, offset + 4);
        assert_eq!(original.numerical.numerical.sampled_draws, prior_draws + 2);
        pending.reader.no_generation(8);
    }
    let retained = archive::load(history.output_path().unwrap()).unwrap();
    assert_eq!(retained.revision(), current.publication.revision);
    assert_eq!(fs::read(&pending.anchor).unwrap(), old_anchor);
    closed(&profiles, &pending.reviewer);
}

#[test]
fn ordinary_resident_continuation_reuses_partial_and_completed_original_numerical_work() {
    run_pending(3, false, false, true);
    run_pending(4, false, false, true);
    run_pending(3, true, false, true);
}

#[test]
fn ordinary_continuation_accepts_a_single_message_and_requires_a_fresh_human_key() {
    run_pending(3, false, true, true);
    run_pending(4, false, false, false);
}

#[test]
fn ordinary_continuation_rejects_changed_intent_and_later_generation_collisions_before_source() {
    for fault in 0..3 {
        let root = Root::new();
        let mut pending = prepared(&root, 3, fault == 2);
        match fault {
            0 => pending.entries[0].inputs.request.prompt = b"ac".to_vec(),
            1 => pending.entries[0].inputs.request.max_new_tokens += 1,
            _ => {
                pending.entries[1].inputs.generation = 6;
                pending.entries[2].inputs.generation = 6;
            }
        }
        let history = history(&root, &pending);
        let before = pending.reader.progress(7);
        let old_anchor = fs::read(&pending.anchor).unwrap();
        fs::remove_file(pending.config.source.path()).unwrap();
        let mut output = Output::new(&pending.entries);
        let profiles = output.profiles.clone();
        let error = run_with_continuation(
            pending.config, pending.entries, &pending.reviewer, None, None,
            &history, || ElapsedTick(1001), &mut output,
        )
        .unwrap_err();
        assert!(
            error.contains(if fault == 2 { "already exists" } else { "original text intent" }),
            "{error}"
        );
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        let current = pending.reader.progress(7);
        assert_eq!(current.publication.revision, pending.before.revision + 1);
        assert_eq!(current.publication.control.ledger.epoch, pending.before.control.ledger.epoch + 1);
        assert_eq!(current.publication.control.ledger.elapsed, pending.before.control.ledger.elapsed);
        assert_eq!(current.text.command(), before.text.command());
        assert_eq!(current.text.generation_revision(), 3);
        assert_eq!(current.numerical.numerical, before.numerical.numerical);
        assert_eq!(current.publication.executions, 0);
        assert_eq!(current.publication.control.ledger.charged, 0);
        assert!(current.publication.stop.is_none());
        pending.reader.no_generation(8);
        assert_eq!(
            archive::load(history.output_path().unwrap()).unwrap().revision(),
            current.publication.revision
        );
        assert_eq!(fs::read(&pending.anchor).unwrap(), old_anchor);
        closed(&profiles, &pending.reviewer);
    }
}

#[test]
fn ordinary_continuation_loses_source_without_resuming_inference_or_publishing() {
    let root = Root::new();
    let pending = prepared(&root, 3, false);
    let history = history(&root, &pending);
    let before = pending.reader.progress(7);
    fs::remove_file(pending.config.source.path()).unwrap();
    let mut output = Output::new(&pending.entries);
    let profiles = output.profiles.clone();
    let error = run_with_continuation(
        pending.config, pending.entries, &pending.reviewer, None, None,
        &history, || ElapsedTick(1001), &mut output,
    )
    .unwrap_err();
    assert!(!error.is_empty());
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    let current = pending.reader.progress(7);
    assert_eq!(current.text.command(), before.text.command());
    assert_eq!(current.text.generation_revision(), 3);
    assert_eq!(current.numerical.numerical, before.numerical.numerical);
    assert_eq!(current.publication.executions, 0);
    assert_eq!(current.publication.control.ledger.charged, 0);
    assert!(current.publication.stop.is_some());
    pending.reader.no_generation(8);
    assert_eq!(
        archive::load(history.output_path().unwrap()).unwrap().revision(),
        current.publication.revision
    );
    closed(&profiles, &pending.reviewer);
}

#[test]
fn ordinary_continuation_stop_wins_before_during_and_after_original_replay() {
    for boundary in 0..3 {
        let root = Root::new();
        let pending = prepared(&root, 3, false);
        let history = history(&root, &pending);
        let before = pending.reader.progress(7);
        let canonical = fs::read(pending.reader.store.join("delivery.bin")).unwrap();
        let old_anchor = fs::read(&pending.anchor).unwrap();
        let total = usize::try_from(pending.before.revision).unwrap();
        let stop_at = match boundary {
            0 => 2,
            1 => total / 2 + 2,
            _ => total + 2,
        };
        fs::remove_file(pending.config.source.path()).unwrap();
        let mut output = Output::new(&pending.entries);
        let profiles = output.profiles.clone();
        let mut controller = None;
        let mut calls = 0;
        let error = run_with_continuation(
            pending.config, pending.entries, &pending.reviewer, None, None,
            &history,
            || {
                if calls == stop_at {
                    assert_eq!(
                        fs::read(pending.reader.store.join("delivery.bin")).unwrap(),
                        canonical
                    );
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
                ElapsedTick(1001)
            },
            &mut output,
        )
        .unwrap_err();
        assert!(error.starts_with("native admission Stop acknowledged at revision"), "{error}");
        let receipt = controller.expect("replay reached the selected boundary").join().unwrap();
        assert!(receipt.acknowledged());
        assert_eq!(receipt.binding.operation, 91);
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        let current = pending.reader.progress(7);
        assert_eq!(current.publication.revision, pending.before.revision + 2);
        assert_eq!(current.text.command(), before.text.command());
        assert_eq!(current.text.generation_revision(), 3);
        assert_eq!(current.numerical.numerical, before.numerical.numerical);
        assert_eq!(current.publication.control.ledger.elapsed, pending.before.control.ledger.elapsed);
        assert_eq!(current.publication.executions, 0);
        assert_eq!(current.publication.control.ledger.charged, 0);
        pending.reader.no_generation(8);
        assert_eq!(
            archive::load(history.output_path().unwrap()).unwrap().revision(),
            current.publication.revision
        );
        assert_eq!(fs::read(&pending.anchor).unwrap(), old_anchor);
        closed(&profiles, &pending.reviewer);
    }
}

#[test]
fn ordinary_continuation_refuses_a_rolled_back_store_before_fencing_or_source_work() {
    let root = Root::new();
    let pending = prepared(&root, 3, false);
    let history = history(&root, &pending);
    let canonical = pending.reader.store.join("delivery.bin");
    fs::write(&canonical, &pending.before_intent).unwrap();
    fs::remove_file(pending.config.source.path()).unwrap();
    let mut output = Output::new(&pending.entries);
    let profiles = output.profiles.clone();
    assert!(run_with_continuation(
        pending.config, pending.entries, &pending.reviewer, None, None,
        &history, || ElapsedTick(1001), &mut output,
    ).is_err());
    assert_eq!(fs::read(canonical).unwrap(), pending.before_intent);
    assert!(!history.output_path().unwrap().exists());
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    closed(&profiles, &pending.reviewer);
}

#[test]
fn ordinary_continuation_reserves_delayed_publication_inputs_before_the_recovery_fence() {
    let root = Root::new();
    let pending = prepared(&root, 3, false);
    let history = history(&root, &pending);
    let output_path = history.output_path().unwrap();
    let publication = PublicationProfile::decode(format!(
        r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}/missing-original","current":"{}","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), output_path.display(),
    ).as_bytes()).unwrap();
    let canonical = fs::read(pending.reader.store.join("delivery.bin")).unwrap();
    let profiles = pending.entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
    let mut output = Vec::new();
    let error = run_with_continuation(
        pending.config, pending.entries, &pending.reviewer, Some(&publication), None,
        &history, || panic!("aliased delayed witness acquired a clock"), &mut output,
    ).unwrap_err();
    assert!(error.contains("history custody overlaps"), "{error}");
    assert!(output.is_empty());
    assert!(!output_path.exists());
    assert_eq!(fs::read(pending.reader.store.join("delivery.bin")).unwrap(), canonical);
    closed(&profiles, &pending.reviewer);
}
