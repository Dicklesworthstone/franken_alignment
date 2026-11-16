//! Exact operator retries survive a terminal checkpoint before actor intake.
//! Read-only observations queued at the same boundaries must still allow the
//! original numerical, actor, helper and human publications.
use super::*;

fn receive(socket: &mut UnixStream, limit: usize) -> Json {
    let mut response = Vec::new();
    loop {
        let mut byte = [0; 1];
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        assert!(response.len() <= limit + 1);
        if byte[0] == b'\n' {
            break;
        }
    }
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}

/// Queue BOTH original requests synchronously before the supplied workflow
/// clock returns. The production service admits only one frame per checkpoint,
/// so retiring it at the first terminal checkpoint loses the second response.
fn queued_requests(
    settings: &Settings,
    actor: &Profile,
    before: &FileDeliverySnapshot,
    store: PathBuf,
    profile: FileOversightProfile,
    fenced: bool,
) -> JoinHandle<()> {
    let mut command = if fenced {
        request(actor, "early-resident-fence", Verb::Fence)
    } else {
        request(actor, "early-resident-observation", Verb::Situation)
    };
    if fenced {
        command.idempotency_key = Some("1201".into());
        command.arguments = ReadArguments::Fence {
            operation: 1201,
            expected: FileFenceBasis {
                scope: actor.scope,
                journal_revision: before.revision,
                control_sequence: before.control.sequence,
                authority_epoch: before.control.ledger.epoch,
            },
        };
    }
    let mut encoded = encode_request(&command).unwrap().into_bytes();
    encoded.push(b'\n');
    let mut socket = connect(settings);
    socket.write_all(&encoded).unwrap();
    socket.write_all(&encoded).unwrap();
    let response_limit = settings.response_bytes;
    thread::spawn(move || {
        let first = receive(&mut socket, response_limit);
        if fenced {
            // Establish a real successful native control before attributing a
            // missing retry to the reply lifetime, rather than a stale basis.
            assert_eq!(field(&first, &["result", "kind"]).as_str(), Some("fence"));
            assert_eq!(
                field(&first, &["result", "value", "knowledge", "state"]).as_str(),
                Some("known")
            );
        }
        if !fenced {
            known(&first);
        }
        let second = receive(&mut socket, response_limit);
        if !fenced {
            known(&second);
        }
        if fenced {
            assert_eq!(second, first);
            // The second service turn passes the original Stop/drain barrier.
            // Compare a THIRD exact retry only after that real cleanup settles.
            let settled = FileOversight::read_publication(&store, &profile).unwrap();
            let journal = store.join("delivery.bin");
            let bytes = std::fs::read(&journal).unwrap();
            assert_eq!(
                exchange(&mut socket, &command, response_limit),
                first
            );
            assert_eq!(
                FileOversight::read_publication(&store, &profile).unwrap(),
                settled
            );
            assert_eq!(
                std::fs::read(&journal).unwrap(),
                bytes,
                "an exact Fence retry must not append another Stop or repeat cleanup"
            );
        }
    })
}

#[test]
fn resident_operator_retries_survive_pre_intake_and_handoff_fence_without_reissuing_authority() {
    for handoff in [false, true] {
        for fenced in [false, true] {
            let root = Root::new();
            let (config, mut entries, reviewer) = selected(&root, true);
            for entry in &mut entries {
                entry.actor.reply_ms = 500;
            }
            let reader = Reader::new(&config, &entries[0]);
            let chosen = selection(&root, &entries[0].actor, 2, fenced);
            let mut output = Output::new(&entries);
            let profiles = output.profiles.clone();
            let actor_profile = entries[0].actor.clone();
            let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
            let mut observer = None;
            let mut injected_at = None;
            let result = run_with_operator(
                config,
                entries,
                &reviewer,
                None,
                (None, Some(&chosen)),
                || {
                    humans.poll();
                    if observer.is_none() && chosen.settings.socket.exists() {
                        let before = reader.publication();
                        let boundary = if handoff {
                            before.executions == 1
                                && !profiles[0].socket.exists()
                                && !profiles[1].socket.exists()
                        } else {
                            before.executions == 0
                        };
                        if boundary {
                            assert_eq!(before.control.ledger.epoch, 0);
                            assert!(before.stop.is_none());
                            injected_at = Some(before.executions);
                            observer = Some(queued_requests(
                                &chosen.settings,
                                &actor_profile,
                                &before,
                                reader.store.clone(),
                                reader.profile.clone(),
                                fenced,
                            ));
                        }
                    }
                    ElapsedTick(1000)
                },
                &mut output,
            );
            assert_eq!(injected_at, Some(if handoff { 1 } else { 0 }));
            observer
                .expect("the exact original pre-intake boundary must be reached")
                .join()
                .unwrap();
            let packets = humans.join();
            let documents = output.documents.len();
            let replies = output.join();
            if !fenced {
                assert!(result.is_ok(), "{result:?}");
                assert_eq!(documents, 3);
                complete(&reader, &replies, &packets);
            } else {
                assert!(result.is_err(), "a fenced incomplete sequence cannot succeed");
                let completed = if handoff { 1 } else { 0 };
                assert_eq!(documents, completed);
                assert_eq!(replies.len(), completed);
                assert!(replies.iter().all(executed));
                assert!(packets[completed..].iter().all(Option::is_none));
                let image = reader.stream();
                assert_eq!(image.publication.executions, completed as u64);
                assert_eq!(image.publication.payload, vec![b'A'; completed]);
                assert_eq!(image.publication.control.ledger.epoch, 1);
                assert_eq!(image.publication.control.ledger.reserved, 0);
                assert_eq!(
                    image.publication.control.ledger.charged,
                    packets
                        .iter()
                        .flatten()
                        .map(|packet| packet.action().spec().units)
                        .sum::<u64>()
                );
                assert_eq!(
                    image.publication.stop.unwrap().request().operation,
                    1201
                );
                assert_eq!(image.confirmed, image.published);
                assert!(!image.confirmed.finished());
                assert!(image.pending.is_none());
                if handoff {
                    let progress = reader.progress(7);
                    assert_eq!(progress.numerical.numerical.position, 4);
                    assert_eq!(progress.numerical.numerical.sampled_draws, 2);
                } else {
                    assert!(image.publication.control.ledger.stages.is_empty());
                    reader.no_generation(7);
                }
                reader.no_generation(8);
                reader.no_generation(9);
            }
            assert!(!chosen.settings.socket.exists());
            closed(&profiles, &reviewer);
        }
    }
}
