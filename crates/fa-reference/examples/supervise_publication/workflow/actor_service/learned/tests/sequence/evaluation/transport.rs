//! A healthy first request may become the last resident request because its
//! actor transport fails, or because the original native review holds it.
//! Neither outcome can discard the independent evaluator's original lifetime.
use super::*;

fn first_actor(root: &Root, profile: &Profile, hold: bool) -> JoinHandle<WireResponse> {
    let profile = profile.clone();
    let root = root.0.clone();
    let peer = peers::actor(
        profile.clone(),
        LearnedTextRelease::Message,
        plus(crate::workflow::clock(), 55_000).unwrap(),
        peers::Behavior::Observe,
    );
    thread::spawn(move || {
        let response = peer.join().unwrap();
        assert_eq!(confirmed(&response), !hold);
        if !hold {
            // The original actor has its actual Executed receipt and its first
            // connection has ended. A new OS connection exceeds its one
            // original candidate; this is an actual transport failure, not an
            // invented workflow error or a changed publication result.
            let mut excess = UnixStream::connect(&profile.socket).unwrap();
            profile.supervisor.verify(&excess).unwrap();
            excess
                .set_read_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            let mut byte = [0_u8; 1];
            match excess.read(&mut byte) {
                Ok(0) => {}
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
                other => panic!("spent resident actor candidate remained live: {other:?}"),
            }
            fs::write(
                root.join("actor.transport-failed"),
                b"original candidate exhausted",
            )
            .unwrap();
            // Do not let a quick evaluator hide the bug by labeling during
            // leftover actor reply grace. Only the subsequent terminal phase
            // can remove this listener while keeping independent intake live.
            let started = Instant::now();
            while profile.socket.exists() {
                assert!(
                    started.elapsed() < Duration::from_secs(30),
                    "failed actor listener was not retired"
                );
                crate::workflow::pause(1);
            }
        }
        fs::write(
            root.join("actor-1.finished"),
            b"original first-request result observed",
        )
        .unwrap();
        response
    })
}

fn run_early(hold: bool) {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    let held = hold.then(Root::new);
    if let Some(held) = &held {
        fixture::write(held, b"hold");
        native::provision_response(held, b"hold");
        let document = fs::read_to_string(&paths[0]).unwrap();
        fs::write(
            &paths[0],
            document.replace(
                root.0.join("native-roster.json").to_str().unwrap(),
                held.0.join("native-roster.json").to_str().unwrap(),
            ),
        )
        .unwrap();
        entries[0].loaded = recipe::load(&paths[0], &config, true).unwrap();
        assert_eq!(entries[0].loaded.generation, entries[1].loaded.generation);
    }
    let reader = Reader::new(&root, &paths[0]);
    entries[0].actor.candidates = 1;
    entries[0].actor.connections = 1;
    entries[0].actor.reply_ms = 1_000;
    let mut child = EvaluatorChild::start(&root, if hold { "early-hold" } else { "transport" });
    let selection = select_evaluation(&root, &config, &mut entries, child.0.id(), 2);
    let mut peers = Peers::new(&root, &config, &entries);
    let reviewer = peers.reviewer.clone();
    let result =
        resident::run_with_evaluation(config, entries, &reviewer, None, &selection, || {
            if peers.profiles[0].socket.exists() {
                peers.seen_actor[0] = true;
                if peers.actors[0].is_none() {
                    peers.actors[0] = Some(first_actor(&root, &peers.profiles[0], hold));
                }
            }
            if reviewer.socket(1).exists() {
                peers.seen_human[0] = true;
                assert!(!hold, "native Hold exposed a human publication offer");
                if peers.humans[0].is_none() {
                    peers.humans[0] =
                        Some(peers::human(reviewer.clone(), 1, ReviewDecision::Approve));
                }
            }
            for profile in &peers.profiles[1..] {
                assert!(
                    !profile.socket.exists(),
                    "terminal first request admitted a successor actor"
                );
                assert!(
                    !reviewer.socket(profile.request).exists(),
                    "terminal first request opened a successor review"
                );
            }
            crate::workflow::clock()
        })
        .unwrap();
    fs::write(
        root.0.join("service.finished"),
        b"original resident service ended",
    )
    .unwrap();
    child.finish();
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(result.responses.len(), 1);
    let failure = result
        .failure
        .as_ref()
        .expect("terminal sequence outcome retained");
    if hold {
        assert!(failure.contains("without an Executed receipt"), "{failure}");
    } else {
        assert!(
            failure.contains("learned actor candidate quota exhausted"),
            "{failure}"
        );
        assert!(root.0.join("actor.transport-failed").exists());
    }
    assert_eq!(peers.seen_actor, [true, false, false]);
    assert_eq!(peers.seen_human, [!hold, false, false]);
    let (responses, packets) = peers.join();
    let actor = responses[0].as_ref().unwrap();
    assert_eq!(actor.result, result.responses[0].result);
    assert_eq!(confirmed(actor), !hold);
    assert!(responses[1..].iter().all(Option::is_none));
    assert!(packets[1..].iter().all(Option::is_none));
    let snapshot = reader.snapshot();
    assert_eq!(snapshot.executions, u64::from(!hold));
    assert_eq!(snapshot.control.ledger.stages.len(), 1);
    assert_eq!(snapshot.control.ledger.reserved, 0);
    assert_eq!(
        snapshot.control.ledger.charged,
        packets[0]
            .as_ref()
            .map_or(0, |packet| packet.action().spec().units)
    );
    assert_eq!(
        snapshot.payload.as_slice(),
        if hold {
            b"".as_slice()
        } else {
            b"A".as_slice()
        }
    );
    // Only transport failure reaches the sequence's existing error cleanup;
    // the label above it still preserved the exact pre-Stop control cut.
    assert_eq!(snapshot.stop.is_some(), !hold);
    let stream = FileOversight::read_stream_publication_with_learned_generation(
        &reader.config.store,
        &reader.config.profile,
        &reader.loaded.generation,
    )
    .unwrap();
    assert_eq!(stream.confirmed.messages().count(), usize::from(!hold));
    assert!(!stream.confirmed.finished());
    let receipt = read_json(&root.0.join("receipt-final.json"));
    let case = read_json(&root.0.join("case-final.json"));
    assert_eq!(number(&case, &["knowledge", "value", "round"]), 1);
    let anchor = archive::load(&root.0.join("resident.anchor")).unwrap();
    assert_eq!(anchor.revision(), snapshot.revision);
    assert!(anchor.revision() >= number(&receipt, &["after", "journal_revision"]));
    assert!(!selection.peer.socket_path().exists());
    assert!(!reader.config.source.path().exists());
    for file in ["model.json", "weights.safetensors", "native-roster.json"] {
        assert!(!root.0.join(file).exists());
    }
    quiescent(&root);
}

#[test]
fn first_executed_receipt_transport_failure_keeps_terminal_independent_evaluation() {
    run_early(false);
}

#[test]
fn first_native_hold_is_assessable_without_admitting_another_message() {
    run_early(true);
}
