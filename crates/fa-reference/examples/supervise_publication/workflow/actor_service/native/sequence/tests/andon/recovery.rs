//! A recovered native suffix keeps the original numerical and accounting owner.
//! Actual actor, helper, human and independent evaluator processes exercise the
//! composition. Fixture weights and benign labels do not qualify a detector.
use super::super::super::evaluation::Recovery;
use super::*;
use super::super::precheck_tests;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    evaluation::andon::FileAndonRecoveryRequirements,
};

const AFTER: u64 = 92;

struct Seed {
    config: Config,
    reviewer: PeerProfile,
    reader: Reader,
    packets: Vec<ReviewPacket>,
    anchor: PathBuf,
    before: FileDeliverySnapshot,
    earlier: Vec<u8>,
    publication: Option<PublicationProfile>,
}

fn seeded(root: &Root, maximum: usize) -> Seed {
    seeded_with_witnesses(root, maximum, None)
}

fn seeded_with_witnesses(root: &Root, maximum: usize, fallback: Option<bool>) -> Seed {
    let (config, mut entries, reviewer) = selected(root, false);
    entries.truncate(2);
    for entry in &mut entries {
        entry.inputs.stream = StreamProfile::new(9, 1, maximum, 64, 256).unwrap();
    }
    let reader = Reader::new(&config, &entries[0]);
    let publication = fallback.map(|fallback| checked::evaluated_publication(root, &reader, fallback));
    // A separate root prevents the resumed subprocess from mistaking an old
    // serial's completion file for its own operation. One discovery spends this
    // first service's allowance without labeling either historical case.
    let peer_root = Root::new();
    let mut peer = Some(EvaluatorProcess::start(&peer_root, &entries[0].actor, 1));
    let evaluator = peer.as_ref().unwrap().selected.clone();
    let (history, anchor) = history(root, &config, &entries, &reviewer, None);
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut discovery = None;
    let mut earlier = None;
    let result = run_with_andon(
        config,
        entries,
        &reviewer,
        publication.as_ref(),
        None,
        &history,
        &settings(),
        &evaluator,
        || {
            if earlier.is_none()
                && reader.store.join("delivery.bin").exists()
                && reader.publication().executions == 1
            {
                earlier = Some(fs::read(reader.store.join("delivery.bin")).unwrap());
            }
            humans.poll();
            if reviewer.socket(AFTER).exists() && discovery.is_none() {
                let mut peer = peer.take().unwrap();
                discovery = Some(thread::spawn(move || {
                    let (accepted, response) = peer.operation("read", 91);
                    assert!(accepted, "{}", String::from_utf8_lossy(&response));
                    assert_eq!(
                        field(&parse(&response), &["knowledge", "state"]).as_str(),
                        Some("known")
                    );
                    peer.finish();
                }));
            }
            crate::workflow::clock()
        },
        &mut output,
    );
    assert!(result.is_ok(), "actual accounted prefix failed: {result:?}");
    discovery
        .expect("the second original human boundary was reached")
        .join()
        .unwrap();
    assert_eq!(output.documents.len(), 2);
    assert!(output.join().iter().all(executed));
    let packets = humans.join().into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(packets.len(), 2);
    let before = reader.publication();
    assert_eq!(
        (before.executions, before.payload.as_slice()),
        (2, b"AA".as_slice())
    );
    assert_eq!(before.control.ledger.epoch, 0);
    assert_eq!(reader.progress(8).numerical.numerical.position, 8);
    assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
    assert!(!reader.stream().confirmed.finished());
    retained(&anchor, &reader);
    assert!(!evaluator.socket_path().exists());
    closed(&profiles, &reviewer);
    let mut config = fixture::configured(root);
    config.programs = fixture::native_programs_at_epoch(root, 1);
    Seed {
        config,
        reviewer,
        reader,
        packets,
        anchor,
        before,
        earlier: earlier.expect("the first actual publication was observed"),
        publication,
    }
}

fn suffix(root: &Root, seed: &Seed, finish_only: bool) -> Vec<Entry> {
    let (actor, _) = profiles(root, &seed.config);
    let make = |request, generation, release| {
        let mut actor = actor.clone();
        actor.request = request;
        actor.socket = root.0.join(format!("actor-{request}.sock"));
        let mut inputs = recipe::load(
            &root.0.join("recipe.json"),
            seed.config.profile.delivery.scope.tenant,
            seed.config.profile.delivery.limits.bytes,
        )
        .unwrap();
        inputs.generation = generation;
        inputs.stream = seed.reader.stream;
        Entry {
            actor,
            inputs,
            release,
            precheck: None,
        }
    };
    if finish_only {
        vec![make(93, 8, Release::Finish)]
    } else {
        vec![make(93, 9, Release::Message), make(94, 9, Release::Finish)]
    }
}

fn recovery_file(root: &Root, before: &FileDeliverySnapshot) -> PathBuf {
    let path = root.0.join("evaluation-recovery.json");
    fs::write(&path, format!(
        r#"{{"schema":"fa.native-evaluation-recovery/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}}}"#,
        before.revision, before.control.sequence, before.control.ledger.epoch,
    )).unwrap();
    path
}

fn recovered_history(root: &Root, seed: &Seed, entries: &[Entry]) -> anchors::Selection {
    let args = [
        "serve-native-generated".into(),
        "config".into(),
        "sequence".into(),
        "reviewer".into(),
        "--after-message".into(),
        AFTER.to_string(),
        "--history-anchor".into(),
        seed.anchor.display().to_string(),
        "--retain-history-anchor".into(),
        root.0.join("resumed.anchor").display().to_string(),
    ];
    anchors::take_options(&args)
        .unwrap()
        .1
        .admit(
            &seed.config,
            &entries
                .iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            &seed.reviewer,
            None,
            Some(AFTER),
            None,
        )
        .unwrap()
}

fn expected(
    seed: &Seed,
    entries: &[Entry],
    settings: &andon_settings::Settings,
) -> FileAndonRecoveryRequirements {
    FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(entries[0].inputs.stream),
                decoder: Some(entries[0].inputs.decoder.clone()),
                decoder_stop: None,
                source: Some(seed.config.source_policy),
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: seed.config.profile.delivery.policy.clone(),
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: seed.before.revision,
                control_sequence: seed.before.control.sequence,
                authority_epoch: seed.before.control.ledger.epoch,
            },
        },
        settings.config.clone(),
        settings.windows.clone(),
        settings.minimum_evaluation_revision,
    )
    .unwrap()
}

fn archive(path: &Path) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap()
}

fn number(value: &Json, path: &[&str]) -> u64 {
    field(value, path).as_str().unwrap().parse().unwrap()
}

fn recovered_suffix(finish_only: bool, approve: bool, candidates: u32) {
    recovered_suffix_selected(finish_only, approve, candidates, false, None);
}

fn recovered_suffix_selected(
    finish_only: bool,
    approve: bool,
    candidates: u32,
    prechecked: bool,
    fallback: Option<bool>,
) {
    let root = Root::new();
    let seed = seeded_with_witnesses(&root, if finish_only { 2 } else { 4 }, fallback);
    let mut entries = suffix(&root, &seed, finish_only);
    let precheck_sockets = if prechecked {
        precheck_tests::select(&root, &mut entries, 1)
    } else {
        Vec::new()
    };
    let requests = entries
        .iter()
        .map(|entry| entry.actor.request)
        .collect::<Vec<_>>();
    let history = recovered_history(&root, &seed, &entries);
    let requirements_path = recovery_file(&root, &seed.before);
    let requirements = Recovery::read(&requirements_path, &seed.config).unwrap();
    let mut accounting = settings();
    accounting.minimum_evaluation_revision = 2;
    let mut expected = expected(&seed, &entries, &accounting);
    if let Some(publication) = &seed.publication {
        expected = expected.require_publication(publication.limits).unwrap();
    }
    let original_anchor = fs::read(&seed.anchor).unwrap();
    let before = seed.reader.progress(8);
    let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, candidates);
    let evaluator = peer.selected.clone();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(seed.reviewer.clone(), requests.clone(), offered, decisions);
    let store = seed.reader.store.clone();
    let profile = seed.reader.profile.clone();
    let last = *requests.last().unwrap();
    let retired_prechecks = precheck_sockets.clone();
    let independent = thread::spawn(move || {
        let mut inode = None;
        for (index, request) in requests.iter().copied().enumerate() {
            let (offered, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(offered, request);
            assert_eq!(packet.action().spec().policy_epoch, 1);
            if prechecked {
                assert!(
                    retired_prechecks[index].as_ref().is_none_or(|path| !path.exists()),
                    "a release disclosure retained custody at the fresh human boundary"
                );
            }
            let publication = || FileOversight::read_publication(&store, &profile).unwrap();
            assert_eq!(publication().executions, 2 + index as u64);
            if index == 0 {
                inode = Some(fs::metadata(peer.selected.socket_path()).unwrap().ino());
                let (accepted, receipt) = peer.operation("label", 91);
                assert!(
                    accepted,
                    "historical case was lost: {}",
                    String::from_utf8_lossy(&receipt)
                );
                let receipt = parse(&receipt);
                assert_eq!(field(&receipt, &["status"]).as_str(), Some("applied"));
                assert_eq!(
                    number(&receipt, &["after", "evaluation_revision"]),
                    number(&receipt, &["before", "evaluation_revision"]) + 1
                );
                assert_eq!(
                    number(&receipt, &["after", "journal_revision"]),
                    number(&receipt, &["before", "journal_revision"]) + 2
                );
                for coordinate in ["control_sequence", "authority_epoch"] {
                    assert_eq!(
                        field(&receipt, &["before", coordinate]),
                        field(&receipt, &["after", coordinate])
                    );
                }
            }
            if request == last {
                if candidates == 4 {
                    assert_eq!(
                        fs::metadata(peer.selected.socket_path()).unwrap().ino(),
                        inode.unwrap()
                    );
                    let (accepted, receipt) = peer.operation("label", request);
                    assert!(
                        accepted,
                        "final case was lost: {}",
                        String::from_utf8_lossy(&receipt)
                    );
                    assert_eq!(
                        field(&parse(&receipt), &["status"]).as_str(),
                        Some("applied")
                    );
                } else {
                    let started = Instant::now();
                    while peer.selected.socket_path().exists() {
                        assert!(started.elapsed() < Duration::from_secs(20));
                        pause(1);
                    }
                    let (accepted, bytes) = peer.operation("read", request);
                    assert!(
                        !accepted && bytes.is_empty(),
                        "a recovered handoff refilled evaluator candidates"
                    );
                }
            }
            assert_eq!(
                publication().executions,
                2 + index as u64,
                "independent evaluation cannot supply the missing fresh human key"
            );
            release
                .send(Some(if request == last && !approve {
                    ReviewDecision::Reject
                } else {
                    ReviewDecision::Approve
                }))
                .unwrap();
        }
        peer.finish();
    });
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut checked_output = prechecked.then(|| {
        precheck_tests::CheckedOutput::live(&entries, &seed.reader, &seed.reviewer, 1)
    });
    let mut writer: &mut dyn Write = match checked_output.as_mut() {
        Some(checked) => checked,
        None => &mut output,
    };
    let result = run_with_andon_recovery(
        seed.config,
        entries,
        &seed.reviewer,
        seed.publication.as_ref(),
        (AFTER, None),
        &history,
        &accounting,
        &evaluator,
        &requirements,
        crate::workflow::clock,
        &mut writer,
    );
    assert_eq!(result.is_ok(), approve, "{result:?}");
    independent.join().unwrap();
    let packets = human.join().unwrap();
    let replies = match checked_output {
        Some(checked) => checked.join(),
        None => output.join(),
    };
    assert_eq!(replies.len(), if finish_only { 1 } else { 2 });
    let mut sessions = seed
        .packets
        .iter()
        .map(|packet| packet.binding().session)
        .collect::<BTreeSet<_>>();
    let mut charged = seed.before.control.ledger.charged;
    for (index, (packet, response)) in packets.iter().zip(&replies).enumerate() {
        let finish = index + 1 == replies.len();
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.views().len(), 2);
        assert_eq!(packet.action().spec().policy_epoch, 1);
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; 2 + index].as_slice());
        assert_eq!(frame.is_finish(), finish);
        assert_eq!(executed(response), !finish || approve);
        if !finish || approve {
            charged += packet.action().spec().units;
        }
    }
    // Historical accounting and numerical projections need no new source/model
    // bytes after the resident owner and its evaluator have retired.
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    fs::remove_file(root.0.join("model.safetensors")).unwrap();
    let final_anchor = archive(history.output_path().unwrap());
    let saved = FileOversight::read_andon_guarded(
        &seed.reader.store,
        &seed.reader.profile,
        &expected,
        &accounting.protocol,
        &final_anchor,
    )
    .unwrap();
    let count = if finish_only { 3 } else { 4 };
    let labels = if candidates == 4 { 2 } else { 1 };
    assert_eq!(saved.credibility.report.retained_cases, count);
    assert_eq!(saved.credibility.report.pending_cases, count - labels);
    assert_eq!(saved.credibility.report.benign_origins, labels as u64);
    assert_eq!(saved.credibility.report.revision, (count + labels) as u64);
    assert_eq!(saved.accounting.windows().len(), accounting.windows.len());
    assert!(saved.accounting.windows().values().all(|window| {
        window.false_stops == 0 && window.total_cost == 0 && window.incident.is_none()
    }));
    let stream = seed.reader.stream();
    assert_eq!(
        stream.publication.executions,
        2 + u64::from(!finish_only) + u64::from(approve)
    );
    assert_eq!(
        stream.publication.payload,
        if finish_only {
            b"AA".as_slice()
        } else {
            b"AAA".as_slice()
        }
    );
    assert_eq!(stream.publication.control.ledger.charged, charged);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.epoch,
        seed.before.control.ledger.epoch + 1
    );
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.confirmed.finished(), approve);
    assert!(stream.pending.is_none());
    let prior_generation = seed.reader.progress(8);
    assert_eq!(prior_generation.text.command(), before.text.command());
    assert_eq!(
        prior_generation.text.generation_revision(),
        before.text.generation_revision()
    );
    if finish_only {
        assert_eq!(
            prior_generation.numerical.numerical,
            before.numerical.numerical
        );
        assert_eq!(
            stream.confirmed.message_count(),
            stream.confirmed.profile().max_messages()
        );
        seed.reader.no_generation(9);
    } else {
        let progress = seed.reader.progress(9);
        assert_eq!(progress.text.command().position(), 8);
        assert_eq!(progress.numerical.numerical.position, 12);
        assert_eq!(progress.numerical.numerical.sampled_draws, 6);
        seed.reader.no_generation(10);
    }
    assert_eq!(final_anchor.revision(), stream.publication.revision);
    assert_eq!(fs::read(&seed.anchor).unwrap(), original_anchor);
    assert!(!evaluator.socket_path().exists());
    assert!(
        precheck_sockets
            .iter()
            .all(|path| path.as_ref().is_none_or(|path| !path.exists()))
    );
    closed(&profiles, &seed.reviewer);
}

#[test]
fn recovered_native_accounting_retains_old_cases_and_fresh_human_keys_across_message_and_finish() {
    recovered_suffix(false, true, 4);
    recovered_suffix(false, false, 4);
}

#[test]
fn recovered_accounted_singleton_finish_spends_no_new_draws_and_needs_a_new_human_key() {
    recovered_suffix(true, true, 4);
    recovered_suffix(true, false, 4);
}

#[test]
fn recovered_native_evaluator_cannot_replenish_candidates_at_the_finish_handoff() {
    recovered_suffix(false, true, 2);
}

#[test]
fn recovered_accounted_prechecks_preserve_original_cases_finish_and_separate_human_keys() {
    recovered_suffix_selected(false, true, 4, true, None);
    recovered_suffix_selected(true, true, 2, true, None);
    recovered_suffix_selected(false, false, 4, true, None);
}

#[test]
fn feed_checked_accounted_prefix_and_suffix_keep_original_labels_and_fresh_review() {
    // A newly assessed old Message and the last case share the recovered
    // evaluator's four candidates. Rejection still prevents the final effect.
    recovered_suffix_selected(false, true, 4, false, Some(false));
    recovered_suffix_selected(true, false, 4, false, Some(true));
}

#[test]
fn changed_feed_contract_cannot_fence_a_confirmed_resident_prefix() {
    let root = Root::new();
    let seed = seeded_with_witnesses(&root, 4, Some(true));
    let path = root.0.join("producer/delivery.bin");
    let canonical = seed.reader.store.join("delivery.bin");
    let before = fs::read(&canonical).unwrap();
    let staging = seed.reader.store.join("delivery.pending");
    fs::write(&staging, b"preserve staging on mismatch").unwrap();
    let old_anchor = fs::read(&seed.anchor).unwrap();
    let requirements_path = recovery_file(&root, &seed.before);
    let requirements = Recovery::read(&requirements_path, &seed.config).unwrap();
    let mut accounting = settings();
    accounting.minimum_evaluation_revision = 2;
    for changed in [None, Some((false, 120_000)), Some((true, 119_999))] {
        let entries = suffix(&root, &seed, true);
        let evaluator = inactive_peer(&root, &entries[0]);
        let history = recovered_history(&root, &seed, &entries);
        let publication = changed.map(|(fallback, age)| {
            PublicationProfile::decode(checked::witness_document(&path, &seed.reader, fallback, age).as_bytes()).unwrap()
        });
        let mut config = fixture::configured(&root);
        config.programs = fixture::native_programs_at_epoch(&root, 1);
        let mut output = Vec::new();
        let result = run_with_andon_recovery(
            config, entries, &seed.reviewer, publication.as_ref(), (AFTER, None),
            &history, &accounting, &evaluator, &requirements, crate::workflow::clock, &mut output,
        );
        assert!(result.is_err(), "changed witnesses acquired fresh roles");
        assert!(output.is_empty());
        assert_eq!(fs::read(&canonical).unwrap(), before);
        assert_eq!(fs::read(&staging).unwrap(), b"preserve staging on mismatch");
        assert_eq!(fs::read(&seed.anchor).unwrap(), old_anchor);
        assert!(!history.output_path().unwrap().exists());
        assert!(!evaluator.socket_path().exists());
    }
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
fn accounted_suffix_rejects_independent_floors_protocol_windows_and_rollback_before_fencing() {
    for fault in 0..7 {
        let root = Root::new();
        let seed = seeded(&root, 4);
        let entries = suffix(&root, &seed, false);
        let history = recovered_history(&root, &seed, &entries);
        let path = recovery_file(&root, &seed.before);
        let mut accounting = settings();
        accounting.minimum_evaluation_revision = 2;
        let canonical = seed.reader.store.join("delivery.bin");
        match fault {
            0..=2 => {
                let (name, value) = match fault {
                    0 => ("journal_revision", seed.before.revision),
                    1 => ("control_sequence", seed.before.control.sequence),
                    _ => ("authority_epoch", seed.before.control.ledger.epoch),
                };
                let document = fs::read_to_string(&path).unwrap();
                let changed = document.replace(
                    &format!("\"{name}\":{value}"),
                    &format!("\"{name}\":{}", value + 1),
                );
                assert_ne!(document, changed);
                fs::write(&path, changed).unwrap();
            }
            3 => accounting.minimum_evaluation_revision = 3,
            4 => accounting.protocol.stratum += 1,
            5 => accounting
                .windows
                .push(fa_reference::round::review::andon::AndonBudget {
                    window: 2,
                    start: 100,
                    end: 200,
                    false_stops_allowed: 2,
                    cost_per_stop: 9,
                }),
            _ => fs::write(&canonical, &seed.earlier).unwrap(),
        }
        let requirements = Recovery::read(&path, &seed.config).unwrap();
        let evaluator = inactive_peer(&root, &entries[0]);
        let bytes = fs::read(&canonical).unwrap();
        let old_anchor = fs::read(&seed.anchor).unwrap();
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let result = run_with_andon_recovery(
            seed.config,
            entries,
            &seed.reviewer,
            None,
            (AFTER, None),
            &history,
            &accounting,
            &evaluator,
            &requirements,
            crate::workflow::clock,
            &mut output,
        );
        assert!(
            result.is_err(),
            "accepted independent recovery mismatch {fault}"
        );
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        assert_eq!(
            fs::read(&canonical).unwrap(),
            bytes,
            "mismatch {fault} mutated the history"
        );
        assert_eq!(fs::read(&seed.anchor).unwrap(), old_anchor);
        assert!(!history.output_path().unwrap().exists());
        assert!(!evaluator.socket_path().exists());
        closed(&profiles, &seed.reviewer);
    }
}

#[test]
fn accounted_suffix_validates_the_whole_history_before_source_or_evaluator_startup() {
    for fault in 0..3 {
        let root = Root::new();
        let seed = seeded(&root, 4);
        let mut entries = suffix(&root, &seed, false);
        let after = match fault {
            0 => 91,
            1 => {
                entries[1].actor.request = 91;
                entries[1].actor.socket = root.0.join("actor-91.sock");
                AFTER
            }
            _ => {
                entries[1].release = Release::Message;
                entries[1].inputs.generation = 7;
                AFTER
            }
        };
        let history = recovered_history(&root, &seed, &entries);
        let path = recovery_file(&root, &seed.before);
        let requirements = Recovery::read(&path, &seed.config).unwrap();
        let mut accounting = settings();
        accounting.minimum_evaluation_revision = 2;
        let evaluator = inactive_peer(&root, &entries[0]);
        let before = seed.reader.progress(8);
        let old_anchor = fs::read(&seed.anchor).unwrap();
        fs::remove_file(seed.config.source.path()).unwrap();
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let result = run_with_andon_recovery(
            seed.config,
            entries,
            &seed.reviewer,
            None,
            (after, None),
            &history,
            &accounting,
            &evaluator,
            &requirements,
            || {
                assert!(
                    !evaluator.socket_path().exists(),
                    "historical refusal started an evaluator"
                );
                crate::workflow::clock()
            },
            &mut output,
        );
        assert!(result.is_err(), "accepted historical mismatch {fault}");
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        let current = seed.reader.publication();
        assert_eq!(current.revision, seed.before.revision + 1);
        assert_eq!(
            current.control.ledger.epoch,
            seed.before.control.ledger.epoch + 1
        );
        assert_eq!(current.executions, seed.before.executions);
        assert_eq!(current.payload, seed.before.payload);
        assert_eq!(
            current.control.ledger.charged,
            seed.before.control.ledger.charged
        );
        assert_eq!(
            current.control.ledger.elapsed,
            seed.before.control.ledger.elapsed
        );
        assert!(current.stop.is_none());
        assert_eq!(
            seed.reader.progress(8).numerical.numerical,
            before.numerical.numerical
        );
        seed.reader.no_generation(9);
        assert_eq!(
            archive(history.output_path().unwrap()).revision(),
            current.revision
        );
        assert_eq!(fs::read(&seed.anchor).unwrap(), old_anchor);
        closed(&profiles, &seed.reviewer);
    }
}

fn stop_at(reviewer: &PeerProfile, operation: u64) -> JoinHandle<StopControlReceipt> {
    let mut client = reviewer.connect_stop_client(operation).unwrap();
    thread::spawn(move || {
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(20));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => pause(1),
            }
        }
    })
}

#[test]
fn accounted_resident_ready_stop_retains_the_original_terminal_cut_without_live_roles() {
    let root = Root::new();
    let seed = seeded(&root, 4);
    let entries = suffix(&root, &seed, false);
    let history = recovered_history(&root, &seed, &entries);
    let path = recovery_file(&root, &seed.before);
    let requirements = Recovery::read(&path, &seed.config).unwrap();
    let mut accounting = settings();
    accounting.minimum_evaluation_revision = 2;
    let evaluator = inactive_peer(&root, &entries[0]);
    let expected = expected(&seed, &entries, &accounting);
    let selected = bootstrap::selection(&seed.config, &entries[0].inputs, None).unwrap();
    let total = {
        let cursor =
            FileOversight::begin_open_andon_guarded_generated_text_stream_from_source_anchored(
                &seed.config.store,
                seed.config.profile.clone(),
                &expected,
                &entries[0].inputs.decoder,
                &entries[0].inputs.tokenizer,
                selected,
                &accounting.protocol,
                history.input().unwrap(),
            )
            .unwrap();
        cursor.progress().total_events
    };
    let canonical = fs::read(seed.reader.store.join("delivery.bin")).unwrap();
    let old_anchor = fs::read(&seed.anchor).unwrap();
    let before = seed.reader.progress(8);
    fs::remove_file(seed.config.source.path()).unwrap();
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut client = None;
    let mut calls = 0;
    let result = run_with_andon_recovery(
        seed.config,
        entries,
        &seed.reviewer,
        None,
        (AFTER, None),
        &history,
        &accounting,
        &evaluator,
        &requirements,
        || {
            // One invocation sample, one pre-open sample, then one per-event
            // sample and the separate complete Ready boundary precede a Fence.
            if calls == total + 2 {
                assert_eq!(
                    fs::read(seed.reader.store.join("delivery.bin")).unwrap(),
                    canonical
                );
                assert!(!evaluator.socket_path().exists());
                assert!(!seed.reviewer.socket(93).exists());
                assert!(!seed.reviewer.socket(94).exists());
                client = Some(stop_at(&seed.reviewer, 93));
            }
            calls += 1;
            crate::workflow::clock()
        },
        &mut output,
    );
    let error = result.unwrap_err();
    assert!(
        error.starts_with("native admission Stop acknowledged at revision"),
        "{error}"
    );
    let stop = client
        .expect("the accounted original cursor reached Ready")
        .join()
        .unwrap();
    assert!(stop.acknowledged());
    assert_eq!(stop.binding.operation, 93);
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    let current = seed.reader.publication();
    assert_eq!(current.revision, seed.before.revision + 2);
    assert_eq!(current.executions, seed.before.executions);
    assert_eq!(current.payload, seed.before.payload);
    assert_eq!(
        current.control.ledger.charged,
        seed.before.control.ledger.charged
    );
    assert_eq!(
        current.control.ledger.elapsed,
        seed.before.control.ledger.elapsed
    );
    assert_eq!(current.stop.as_ref().unwrap().request().operation, 93);
    assert_eq!(
        seed.reader.progress(8).numerical.numerical,
        before.numerical.numerical
    );
    seed.reader.no_generation(9);
    let retained = archive(history.output_path().unwrap());
    assert_eq!(retained.revision(), current.revision);
    let saved = FileOversight::read_andon_guarded(
        &seed.reader.store,
        &seed.reader.profile,
        &expected,
        &accounting.protocol,
        &retained,
    )
    .unwrap();
    assert_eq!(saved.credibility.report.revision, 2);
    assert_eq!(saved.credibility.report.pending_cases, 2);
    assert_eq!(fs::read(&seed.anchor).unwrap(), old_anchor);
    assert!(!evaluator.socket_path().exists());
    closed(&profiles, &seed.reviewer);
}

#[test]
fn native_recovery_requirements_cannot_become_a_future_actor_or_evaluator_endpoint() {
    for alias_parent in [false, true] {
        let root = Root::new();
        let seed = seeded(&root, 4);
        let mut entries = suffix(&root, &seed, false);
        let path = recovery_file(&root, &seed.before);
        let endpoint = if alias_parent {
            let alias = root.0.join("alias");
            std::os::unix::fs::symlink(&root.0, &alias).unwrap();
            alias.join("evaluation-recovery.json")
        } else {
            path.clone()
        };
        let mut evaluator = inactive_peer(&root, &entries[0]);
        if alias_parent {
            evaluator.socket = endpoint;
        } else {
            entries[1].actor.socket = endpoint;
        }
        let history = recovered_history(&root, &seed, &entries);
        let requirements = Recovery::read(&path, &seed.config).unwrap();
        let mut accounting = settings();
        accounting.minimum_evaluation_revision = 2;
        let canonical = fs::read(seed.reader.store.join("delivery.bin")).unwrap();
        let original_requirements = fs::read(&path).unwrap();
        let original_anchor = fs::read(&seed.anchor).unwrap();
        let mut output = Vec::new();
        let error = run_with_andon_recovery(
            seed.config,
            entries,
            &seed.reviewer,
            None,
            (AFTER, None),
            &history,
            &accounting,
            &evaluator,
            &requirements,
            || panic!("future endpoint consumed independently selected recovery requirements"),
            &mut output,
        )
        .unwrap_err();
        assert!(error.contains("recovery requirements overlap"), "{error}");
        assert!(output.is_empty());
        assert_eq!(
            fs::read(seed.reader.store.join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(fs::read(&path).unwrap(), original_requirements);
        assert_eq!(fs::read(&seed.anchor).unwrap(), original_anchor);
        assert!(!history.output_path().unwrap().exists());
        assert!(fs::symlink_metadata(&path).unwrap().is_file());
    }
}

#[test]
fn accounted_native_suffix_grammar_requires_every_independent_recovery_selection() {
    let root = Root::new();
    let base = [
        "serve-native-generated",
        "missing-config",
        "missing-sequence",
        "missing-reviewer",
        "--andon",
        "missing-andon",
        "--evaluator-profile",
        "missing-evaluator",
        "--retain-history-anchor",
        "new-output",
        "--after-message",
        "92",
        "--history-anchor",
        "old-input",
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
            || panic!("missing configuration sampled time"),
            &mut Vec::new()
        )
        .unwrap_err(),
        missing
    );
    for selected in [
        "--andon",
        "--evaluator-profile",
        "--retain-history-anchor",
        "--after-message",
        "--history-anchor",
        "--evaluation-recovery",
    ] {
        let mut invalid = base.clone();
        let index = invalid.iter().position(|value| value == selected).unwrap();
        invalid.drain(index..index + 2);
        assert!(
            andon::take_options(&invalid, None).is_err(),
            "omitted {selected}"
        );
    }
    for value in ["0", "01", "-1", "18446744073709551616"] {
        let mut invalid = base.clone();
        invalid[11] = value.into();
        assert!(
            andon::take_options(&invalid, None).is_err(),
            "accepted predecessor {value}"
        );
    }
    for flag in [
        "--after-message",
        "--evaluation-recovery",
        "--governor-profile",
        "--precheck-profile",
        "--precheck-schedule",
        "--topology-profile",
        "--continue-generation",
    ] {
        let mut invalid = base.clone();
        invalid.extend([flag.into(), "extra".into()]);
        assert!(
            andon::take_options(&invalid, None).is_err(),
            "accepted {flag}"
        );
    }
    assert!(andon::take_options(&base, Some(Path::new("activation"))).is_err());
    assert!(!root.0.join("store").exists());
}
