//! A terminal helper Hold remains available to the original evaluator after
//! actor work ends. Its label records the cost without restarting the stream.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::andon::FileAndonConfig,
    guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        evaluation::andon::FileAndonRecoveryRequirements,
    },
};
use fa_reference::action::consequence::oversight::{
    helper_client::{ClientPhase, HelperClient},
    helper_processes::HelperProgram,
};
use fa_reference::round::Verdict;
use std::{collections::BTreeMap, ffi::OsString, fs::OpenOptions};

const HELPER: &str = "workflow::actor_service::native::sequence::tests::andon_tests::held_case::held_after_prefix_helper_process";
const HELPER_ENV: &str = "FA_NATIVE_EVALUATED_HELD_PREFIX";

/// Run through the actual helper transport. Only this selected helper changes
/// its fixture verdict; beta, native generation and independent human review
/// keep their original implementations and process custody.
#[test]
fn held_after_prefix_helper_process() {
    if std::env::var_os(HELPER_ENV).is_none() {
        return;
    }
    let profile = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    )))
    .unwrap()
    .profile;
    let mut client =
        HelperClient::from_process_stdin(profile.committee.members()["alpha"].profile_at(0))
            .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(20));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), "alpha");
            assert!(
                input
                    .actual_input()
                    .submitted_bytes()
                    .windows(b"review this fixture: alpha".len())
                    .any(|bytes| bytes == b"review this fixture: alpha")
            );
            let round = input.round();
            let verdict = match round {
                91 => Verdict::Allow,
                92 => Verdict::Hold,
                _ => panic!("a final held case must not launch a successor helper"),
            };
            writeln!(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("evaluated-helper-rounds")
                    .unwrap(),
                "{round}"
            )
            .unwrap();
            client.respond(verdict, b"native-evaluated-prefix").unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        pause(1);
    }
}

struct HeldOutput {
    original: Output,
    held: Option<mpsc::Sender<()>>,
}

impl Write for HeldOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.original.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.original.flush()?;
        if self.original.documents.len() == 2
            && let Some(held) = self.held.take()
        {
            let client = self.original.clients.pop().unwrap();
            self.original.clients.push(thread::spawn(move || {
                let response = client.join().unwrap();
                assert!(matches!(
                    &response.result,
                    Ok(Knowledge::Known {
                        value: ActorOutcome::CancelledBeforeDispatch,
                        ..
                    })
                ));
                held.send(()).unwrap();
                response
            }));
        }
        Ok(())
    }
}

#[test]
fn final_native_helper_hold_can_be_assessed_without_restarting_the_confirmed_prefix() {
    let root = Root::new();
    let (mut config, entries, reviewer) = selected(&root, false);
    config.programs.insert(
        "alpha".into(),
        HelperProgram::new(
            std::env::current_exe().unwrap(),
            root.0.clone(),
            vec!["--exact".into(), HELPER.into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from(HELPER_ENV), OsString::from("1"))]),
        )
        .unwrap(),
    );
    let reader = Reader::new(&config, &entries[0]);
    let mut accounting = settings();
    // Select one original allowed false stop in both inventories. This case
    // must cross the native incident threshold on its first benign held label.
    for window in &mut accounting.windows {
        window.false_stops_allowed = 1;
    }
    accounting.config =
        FileAndonConfig::new(accounting.config.identity(), accounting.windows.clone()).unwrap();
    let expected = FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(entries[0].inputs.stream),
                decoder: Some(entries[0].inputs.decoder.clone()),
                decoder_stop: None,
                source: Some(config.source_policy),
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: config.profile.delivery.policy.clone(),
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: 0,
                control_sequence: 0,
                authority_epoch: 0,
            },
        },
        accounting.config.clone(),
        accounting.windows.clone(),
        3,
    )
    .unwrap();
    let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, 2);
    let evaluator = peer.selected.clone();
    let (history, anchor_path) = history(&root, &config, &entries, &reviewer, None);
    let (held, terminal) = mpsc::channel();
    let mut output = HeldOutput {
        original: Output::new(&entries),
        held: Some(held),
    };
    let profiles = output.original.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [Some(ReviewDecision::Approve), None, None],
    );
    let independent = thread::spawn(move || {
        // A helper Hold cancels the original request before execute_serviced
        // returns. Only the outer post-work/reply pump can deliver that terminal
        // receipt: no evaluator connection is opened during active review.
        terminal.recv_timeout(Duration::from_secs(20)).unwrap();
        let (accepted, receipt) = peer.operation("label", REQUESTS[1]);
        assert!(
            accepted,
            "the final held original case must remain assessable: {}",
            String::from_utf8_lossy(&receipt)
        );
        let receipt = parse(&receipt);
        assert_eq!(field(&receipt, &["status"]).as_str(), Some("applied"));
        for coordinate in ["control_sequence", "authority_epoch"] {
            assert_eq!(
                field(&receipt, &["before", coordinate]),
                field(&receipt, &["after", coordinate])
            );
        }
        let case = parse(&fs::read(peer.root.join("case-1.json")).unwrap());
        assert_eq!(
            field(&case, &["knowledge", "value", "round"]).as_str(),
            Some("92")
        );
        assert_eq!(
            field(&case, &["knowledge", "basis", "evaluation_revision"]).as_str(),
            Some("2")
        );
        peer.finish();
        receipt
    });
    let result = run_with_andon(
        config,
        entries,
        &reviewer,
        None,
        None,
        &history,
        &accounting,
        &evaluator,
        || {
            humans.poll();
            crate::workflow::clock()
        },
        &mut output,
    );
    assert_eq!(
        result.unwrap_err(),
        "native sequence ended after 1 of 3 confirmed Executed releases; no successor admitted",
        "the held release ends the schedule without a transport, deadline or cleanup failure"
    );
    let receipt = independent.join().unwrap();
    assert_eq!(output.original.documents.len(), 2);
    assert_eq!(humans.seen, [true, false, false]);
    let packets = humans.join();
    let replies = output.original.join();
    assert_eq!(replies.len(), 2);
    assert!(executed(&replies[0]));
    assert!(!executed(&replies[1]));
    let first = packets[0].as_ref().unwrap();
    assert_eq!(first.views().len(), 2);
    assert_eq!(
        ReleaseFrame::decode(&first.action().spec().payload)
            .unwrap()
            .message(),
        Some("A")
    );
    assert_eq!(
        fs::read_to_string(root.0.join("evaluated-helper-rounds")).unwrap(),
        "91\n92\n"
    );

    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(&anchor_path).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    let saved = FileOversight::read_andon_guarded(
        &reader.store,
        &reader.profile,
        &expected,
        &accounting.protocol,
        &anchor,
    )
    .unwrap();
    assert_eq!(saved.credibility.report.revision, 3);
    assert_eq!(saved.credibility.report.retained_cases, 2);
    assert_eq!(saved.credibility.report.pending_cases, 1);
    assert_eq!(saved.credibility.report.benign_origins, 1);
    assert_eq!(saved.credibility.report.lifetime_false_stops, 1);
    assert_eq!(saved.receipts.len(), 1);
    let window = &saved.accounting.windows()[&1];
    assert_eq!(window.false_stops, 1);
    assert_eq!(window.total_cost, 7);
    let incident = window.incident.as_ref().unwrap();
    assert_eq!(incident.opened_by_round, REQUESTS[1]);
    assert_eq!(
        incident.rounds.iter().copied().collect::<Vec<_>>(),
        [REQUESTS[1]]
    );
    let after = field(&receipt, &["after", "journal_revision"])
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(
        anchor.revision() >= after,
        "retention must follow the acknowledged assessment"
    );
    assert_eq!(saved.credibility.journal.revision, anchor.revision());

    let stream = reader.stream();
    assert_eq!(stream.publication.revision, anchor.revision());
    assert_eq!(stream.publication.executions, 1);
    assert_eq!(stream.publication.payload, b"A");
    assert_eq!(stream.publication.control.ledger.epoch, 0);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.charged,
        first.action().spec().units
    );
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert!(!stream.confirmed.finished());
    assert_eq!(reader.progress(8).numerical.numerical.position, 8);
    reader.no_generation(9);
    assert!(!evaluator.socket_path().exists());
    assert!(UnixStream::connect(evaluator.socket_path()).is_err());
    closed(&profiles, &reviewer);
}
