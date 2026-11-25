//! Exact actor disclosure composes with original independent accounting.
//! Actual sockets, native generation, helper processes and human decisions
//! exercise the boundary; fixture labels do not establish detector quality.
use super::*;
use super::super::precheck_tests::{self, CheckedOutput};
use crate::workflow::actor_service::precheck::Settings;

fn selected_check(index: usize, epoch: u64, socket: &Path) -> exact_precheck::Selection {
    exact_precheck::Selection::new(
        Settings::decode(
            format!(
                r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":{epoch},"disclosure_id":{},"query_limit":1,"exchange_limit":8,"candidate_limit":8,"connection_limit":8,"runtime_ms":15000}}"#,
                socket.display(), 81 + index,
            )
            .as_bytes(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn evaluated_disclosures(omit_middle: bool, approve: bool) {
    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, true);
    let mut sockets = precheck_tests::select(&root, &mut entries, 0);
    if omit_middle {
        entries[1].precheck = None;
        sockets[1] = None;
    }
    let reader = Reader::new(&config, &entries[0]);
    let count = if approve { 3 } else { 2 };
    let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, 2 * count as u32);
    let evaluator = peer.selected.clone();
    let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
    let profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
    let requests = REQUESTS[..count].to_vec();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(reviewer.clone(), requests.clone(), offered, decisions);
    let store = reader.store.clone();
    let profile = reader.profile.clone();
    let observed_sockets = sockets.clone();
    let independent = thread::spawn(move || {
        let mut original_inode = None;
        for (index, request_id) in requests.into_iter().enumerate() {
            let (actual, packet) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
            assert_eq!(actual, request_id);
            assert_eq!(packet.action().spec().policy_epoch, 0);
            assert!(observed_sockets[index].as_ref().is_none_or(|path| !path.exists()));
            let inode = fs::metadata(peer.selected.socket_path()).unwrap().ino();
            assert_eq!(*original_inode.get_or_insert(inode), inode);
            let publication = || FileOversight::read_publication(&store, &profile).unwrap();
            assert_eq!(publication().executions, index as u64);
            let (accepted, response) = peer.operation("label", request_id);
            assert!(accepted, "{}", String::from_utf8_lossy(&response));
            assert_eq!(field(&parse(&response), &["status"]).as_str(), Some("applied"));
            assert_eq!(
                publication().executions,
                index as u64,
                "neither an exact precheck nor independent evaluation supplies the human key"
            );
            release.send(Some(if approve || index == 0 {
                ReviewDecision::Approve
            } else {
                ReviewDecision::Reject
            })).unwrap();
        }
        peer.finish();
    });
    let mut output = CheckedOutput::live(&entries, &reader, &reviewer, 0);
    let result = run_with_andon(
        config, entries, &reviewer, None, None, &history, &settings(), &evaluator,
        crate::workflow::clock, &mut output,
    );
    assert_eq!(result.is_ok(), approve, "{result:?}");
    assert_eq!(output.documents.len(), count);
    independent.join().unwrap();
    let packets = human.join().unwrap();
    let replies = output.join();
    assert_eq!(packets.len(), count);
    assert_eq!(replies.len(), count);
    assert!(executed(&replies[0]));
    let mut charged = 0;
    let mut sessions = BTreeSet::new();
    for (index, (packet, reply)) in packets.iter().zip(&replies).enumerate() {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(executed(reply), approve || index == 0);
        if approve || index == 0 {
            charged += packet.action().spec().units;
        }
    }
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, if approve { 3 } else { 1 });
    assert_eq!(stream.publication.payload, if approve { b"AA".as_slice() } else { b"A".as_slice() });
    assert_eq!(stream.publication.control.ledger.charged, charged);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.confirmed.finished(), approve);
    assert_eq!(reader.progress(8).numerical.numerical.position, 8);
    assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
    reader.no_generation(9);
    retained(&anchor, &reader);
    assert!(!evaluator.socket_path().exists());
    assert!(sockets.iter().all(|path| path.as_ref().is_none_or(|path| !path.exists())));
    closed(&profiles, &reviewer);
}

#[test]
fn accounted_native_prechecks_preserve_independent_evaluation_across_messages_and_finish() {
    evaluated_disclosures(false, true);
    evaluated_disclosures(true, true);
}

#[test]
fn accounted_native_prechecks_and_labels_cannot_override_a_later_human_rejection() {
    evaluated_disclosures(false, false);
}

#[test]
fn accounted_future_precheck_aliases_refuse_before_clock_or_original_owner_creation() {
    for case in ["equal", "ancestor", "descendant", "anchor"] {
        let root = Root::new();
        let (config, mut entries, reviewer) = selected(&root, true);
        precheck_tests::select(&root, &mut entries, 0);
        let actor = &entries[0].actor;
        let mut evaluator = evaluator::Settings::decode(
            peer_profile(&root, actor, actor.supervisor.pid().unwrap() + 1, 8).as_bytes(),
        ).unwrap();
        // Admit an independently valid archive selection before exercising the
        // direct caller's separately required full-schedule custody preflight.
        let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
        let socket = match case {
            "equal" => evaluator.socket_path().to_path_buf(),
            "ancestor" => {
                evaluator.socket = root.0.join("future/check.sock");
                root.0.join("future")
            }
            "descendant" => evaluator.socket_path().join("future.sock"),
            "anchor" => anchor.clone(),
            _ => unreachable!(),
        };
        entries[2].precheck = Some(selected_check(2, 0, &socket));
        if case != "anchor" {
            let accounting = settings();
            let selected = andon::Selection {
                settings: &accounting,
                evaluator: &evaluator,
                recovery: None,
                paths: None,
            };
            let error = selected.endpoints_with_prechecks(
                &config,
                &entries.iter().map(|entry| &entry.actor).collect::<Vec<_>>(),
                &reviewer,
                None,
                entries.iter().filter_map(|entry| entry.precheck.as_ref()),
            ).unwrap_err();
            assert!(error.contains("precheck endpoints must be disjoint"), "{case}: {error}");
        }
        let store = config.store.clone();
        let profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
        let mut clocks = 0;
        let mut output = Vec::new();
        let result = run_with_andon(
            config, entries, &reviewer, None, None, &history, &settings(), &evaluator,
            || { clocks += 1; crate::workflow::clock() }, &mut output,
        );
        assert!(result.is_err(), "accepted {case}");
        assert_eq!(clocks, 0, "{case}");
        assert!(output.is_empty());
        assert!(!store.exists() && !anchor.exists());
        assert!(!socket.exists() && !evaluator.socket_path().exists());
        closed(&profiles, &reviewer);
    }
}

#[test]
fn accounted_precheck_epoch_mismatch_keeps_the_confirmed_prefix_and_its_final_history() {
    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, true);
    let sockets = precheck_tests::select(&root, &mut entries, 0);
    entries[1].precheck = Some(selected_check(1, 1, sockets[1].as_ref().unwrap()));
    let reader = Reader::new(&config, &entries[0]);
    let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, 2);
    let evaluator = peer.selected.clone();
    let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
    let profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
    let (offered, offers) = mpsc::channel();
    let (release, decisions) = mpsc::channel();
    let human = held_reviews(reviewer.clone(), vec![91], offered, decisions);
    let independent = thread::spawn(move || {
        assert_eq!(offers.recv_timeout(Duration::from_secs(20)).unwrap().0, 91);
        let (accepted, response) = peer.operation("label", 91);
        assert!(accepted, "{}", String::from_utf8_lossy(&response));
        release.send(Some(ReviewDecision::Approve)).unwrap();
        peer.finish();
    });
    let mut output = CheckedOutput::live(&entries, &reader, &reviewer, 0);
    let error = run_with_andon(
        config, entries, &reviewer, None, None, &history, &settings(), &evaluator,
        crate::workflow::clock, &mut output,
    ).unwrap_err();
    assert!(error.contains("precheck expected policy epoch does not match the original owner"), "{error}");
    independent.join().unwrap();
    let packets = human.join().unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(output.documents.len(), 1);
    let replies = output.join();
    assert_eq!(replies.len(), 1);
    assert!(executed(&replies[0]));
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, 1);
    assert_eq!(stream.publication.payload, b"A");
    assert_eq!(stream.publication.control.ledger.charged, packets[0].action().spec().units);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert!(!stream.confirmed.finished());
    assert_eq!(reader.progress(7).numerical.numerical.position, 4);
    reader.no_generation(8);
    reader.no_generation(9);
    retained(&anchor, &reader);
    assert!(!evaluator.socket_path().exists());
    assert!(sockets.iter().all(|path| path.as_ref().is_none_or(|path| !path.exists())));
    closed(&profiles, &reviewer);
}

struct PartialDisclosure {
    original: Output,
    socket: PathBuf,
    reviewer: PeerProfile,
    stop: Option<JoinHandle<StopControlReceipt>>,
}

impl Write for PartialDisclosure {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.original.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.original.documents.len() != 1 {
            return self.original.flush();
        }
        self.original.documents.push(std::mem::take(&mut self.original.pending));
        let socket = self.socket.clone();
        let reviewer = self.reviewer.clone();
        self.stop = Some(thread::spawn(move || {
            let mut unfinished = UnixStream::connect(socket).unwrap();
            unfinished.write_all(b"{").unwrap();
            let receipt = stopping(reviewer).join().unwrap();
            drop(unfinished);
            receipt
        }));
        Ok(())
    }
}

#[test]
fn partial_accounted_precheck_keeps_original_stop_live_and_retires_the_evaluator() {
    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, true);
    let sockets = precheck_tests::select(&root, &mut entries, 0);
    entries[0].precheck = None;
    let reader = Reader::new(&config, &entries[0]);
    let peer = EvaluatorProcess::start(&root, &entries[0].actor, 8);
    let evaluator = peer.selected.clone();
    let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
    let profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve), None, None]);
    let mut output = PartialDisclosure {
        original: Output::new(&entries),
        socket: sockets[1].clone().unwrap(),
        reviewer: reviewer.clone(),
        stop: None,
    };
    let result = run_with_andon(
        config, entries, &reviewer, None, None, &history, &settings(), &evaluator,
        || { humans.poll(); ElapsedTick(1000) }, &mut output,
    );
    assert!(result.is_err());
    let stop = output.stop.take().expect("the second disclosure was reached").join().unwrap();
    assert!(stop.acknowledged() && stop.drained());
    assert_eq!(stop.binding.operation, REQUESTS[0]);
    assert_eq!(output.original.documents.len(), 2);
    assert_eq!(humans.seen, [true, false, false]);
    let packets = humans.join();
    let replies = output.original.join();
    assert_eq!(replies.len(), 1);
    assert!(executed(&replies[0]));
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, 1);
    assert_eq!(stream.publication.payload, b"A");
    assert_eq!(stream.publication.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert!(stream.publication.stop.is_some());
    assert!(!stream.confirmed.finished());
    assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
    reader.no_generation(9);
    retained(&anchor, &reader);
    assert!(!evaluator.socket_path().exists());
    assert!(sockets.iter().all(|path| path.as_ref().is_none_or(|path| !path.exists())));
    peer.finish();
    closed(&profiles, &reviewer);
}
