//! Invocation-wide disclosure conservation and independently named source custody.
//! Successful cases still run original inference, native review and human keys.
use super::*;
use crate::workflow::actor_service::learned::tests::mediated_topology as mediated;
use crate::workflow::actor_service::learned::{anchors, topology as custody};
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TOPOLOGY_REFRESH_READ_BYTES;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_PRECHECK_QUERIES, precheck_wire::MAX_PRECHECK_EXCHANGES,
};
use fa_reference::action::consequence::oversight::actor_peer::MAX_PEER_CONNECTIONS;

fn selected_mediated(root: &Root) -> (Config, [PathBuf; 3], Vec<resident::Entry>) {
    let (config, first, initial) = mediated::selected(root, None, false);
    let (second, message) = native::next(root, &config, 2);
    let (last, finish) = native::next(root, &config, 3);
    let entries = [initial, message, finish]
        .into_iter()
        .enumerate()
        .map(|(index, loaded)| {
            let mut actor = peers::actor_profile(root, &config, loaded.request);
            actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
            resident::Entry {
                loaded,
                actor,
                precheck: None,
                release: if index == 2 {
                    LearnedTextRelease::Finish
                } else {
                    LearnedTextRelease::Message
                },
            }
        })
        .collect();
    (config, [first, second, last], entries)
}

fn with_limits(
    socket: &Path,
    queries: usize,
    exchanges: usize,
    candidates: u64,
    connections: u64,
) -> precheck::Settings {
    let original = document(socket, 0, 0);
    let selected = original
        .replace("\"query_limit\":1", &format!("\"query_limit\":{queries}"))
        .replace(
            "\"exchange_limit\":8",
            &format!("\"exchange_limit\":{exchanges}"),
        )
        .replace(
            "\"candidate_limit\":8",
            &format!("\"candidate_limit\":{candidates}"),
        )
        .replace(
            "\"connection_limit\":8",
            &format!("\"connection_limit\":{connections}"),
        );
    precheck::Settings::decode(selected.as_bytes()).unwrap()
}

#[test]
fn resident_disclosure_handoffs_cannot_multiply_work_or_reuse_disclosure_identity() {
    for case in 0..5 {
        let root = Root::new();
        let (config, _, mut entries) = selected(&root, None, None);
        let sockets = select(&root, &mut entries);
        let first = sockets[0].as_deref().unwrap();
        entries[0].precheck = Some(match case {
            0 => with_limits(first, MAX_PRECHECK_QUERIES, 8, 8, 8),
            1 => with_limits(first, 1, MAX_PRECHECK_EXCHANGES, 8, 8),
            2 => with_limits(first, 1, 8, MAX_PEER_CONNECTIONS, 8),
            3 => with_limits(first, 1, 8, MAX_PEER_CONNECTIONS, MAX_PEER_CONNECTIONS),
            _ => settings(first, 0, 0),
        });
        if case == 4 {
            entries[2].precheck = Some(settings(sockets[2].as_deref().unwrap(), 0, 0));
        }
        let reviewer = fixture::peers(&root, &config);
        let source = fs::read(config.source.path()).unwrap();
        let store = config.store.clone();
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            panic!("invalid aggregate disclosure {case} reached the clock")
        });
        let error = result
            .err()
            .expect("individually valid grants exceeded invocation admission");
        assert!(
            error.contains(if case == 4 {
                "distinct disclosure IDs"
            } else {
                "aggregate precheck schedule allowance"
            }),
            "{error}"
        );
        assert!(!store.exists());
        assert_eq!(fs::read(root.0.join("evidence.json")).unwrap(), source);
        assert!(sockets.iter().flatten().all(|socket| !socket.exists()));
        quiescent(&root);
    }
}

#[test]
fn exact_aggregate_disclosure_limits_publish_all_three_original_mediated_releases() {
    let root = Root::new();
    let (config, paths, mut entries) = selected_mediated(&root);
    let sockets = select(&root, &mut entries);
    // The two later rows retain one query and eight exchanges/connections each.
    // Every total is exactly its unchanged native ceiling, not merely below it.
    entries[0].precheck = Some(with_limits(
        sockets[0].as_deref().unwrap(),
        MAX_PRECHECK_QUERIES - 2,
        MAX_PRECHECK_EXCHANGES - 16,
        MAX_PEER_CONNECTIONS - 16,
        MAX_PEER_CONNECTIONS - 16,
    ));
    let reader = Reader::new(&root, &paths[0]);
    let journal = config.store.join("delivery.bin");
    let mut peers = Peers::new(&root, &config, &entries);
    let reviewer = peers.reviewer.clone();
    let result = resident::run(config, entries, &reviewer, false, None, None, || {
        for (index, socket) in sockets.iter().enumerate() {
            let socket = socket.as_ref().unwrap();
            if peers.actors[index].is_none() && socket.exists() {
                for previous in &sockets[..index] {
                    assert!(!previous.as_ref().unwrap().exists());
                }
                let profile = peers.profiles[index].clone();
                let release = peers.release[index];
                let socket = socket.clone();
                let journal = journal.clone();
                peers.actors[index] = Some(thread::spawn(move || {
                    let before = fs::read(&journal).unwrap();
                    passed(query(&profile, &socket, release, index, 9), index);
                    assert_eq!(fs::read(&journal).unwrap(), before);
                    peers::actor(profile, release, DUE[index], peers::Behavior::Observe)
                        .join()
                        .unwrap()
                }));
            }
        }
        peers.poll([None; 3], [Some(ReviewDecision::Approve); 3]);
        ElapsedTick(1000)
    })
    .unwrap();
    assert_eq!(peers.seen_actor, [true; 3]);
    assert_eq!(peers.seen_human, [true; 3]);
    let (replies, packets) = peers.join();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(result.responses.len(), 3);
    let mut sessions = BTreeSet::new();
    let mut charged = 0;
    for (index, (reply, packet)) in replies.into_iter().zip(packets).enumerate() {
        let reply = reply.expect("every selected actor submitted its own source-bound release");
        assert!(confirmed(&reply));
        assert_eq!(reply.result, result.responses[index].result);
        let packet = packet.expect("precheck did not replace independent human approval");
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.action().spec().deadline, DUE[index]);
        charged += packet.action().spec().units;
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; index.min(2)].as_slice());
        assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
    }
    let state = reader.snapshot();
    assert_eq!(state.executions, 3);
    assert_eq!(state.payload, b"AA");
    assert_eq!(state.control.ledger.charged, charged);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.epoch, 0);
    assert!(state.stop.is_none());
    let saved = mediated::checkpoint(&root, 1);
    let history = anchors::load(&root.0.join("history-1.bin")).unwrap();
    assert!(saved.available());
    assert!(saved.work().checks >= 6);
    assert_eq!(
        saved.work().reserved_read_bytes,
        saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES
    );
    assert_eq!(saved.history_anchor(), &history);
    assert_eq!(history.revision(), state.revision);
    assert!(sockets.iter().flatten().all(|socket| !socket.exists()));
    quiescent(&root);
}

fn seed_original_message(root: &Root) -> (Config, PathBuf) {
    let (config, path, loaded) = mediated::selected(root, None, false);
    let reviewer = fixture::peers(root, &config);
    let actor = peers::actor_profile(root, &config, 1);
    let mut actor_peer = None;
    let mut human_peer = None;
    let result = run_with_actor(
        config,
        loaded,
        &reviewer,
        false,
        None,
        None,
        None,
        Some(&actor),
        || {
            if actor_peer.is_none() && actor.socket.exists() {
                actor_peer = Some(peers::actor(
                    actor.clone(),
                    LearnedTextRelease::Message,
                    DUE[0],
                    peers::Behavior::Observe,
                ));
            }
            if human_peer.is_none() && reviewer.socket(1).exists() {
                human_peer = Some(peers::human(reviewer.clone(), 1, ReviewDecision::Approve));
            }
            ElapsedTick(1000)
        },
    )
    .unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    let reply = actor_peer.unwrap().join().unwrap();
    assert!(confirmed(&reply));
    assert_eq!(reply.result, result.response.result);
    human_peer.unwrap().join().unwrap();
    (configured(root), path)
}

#[test]
fn absent_recovery_inputs_cannot_consume_the_original_fence_before_disclosure_alias_refusal() {
    for graph in [false, true] {
        for alias in [false, true] {
            let root = Root::new();
            let (config, path) = seed_original_message(&root);
            let reader = Reader::new(&root, &path);
            let before = reader.snapshot();
            let saved = mediated::checkpoint(&root, 1);
            let (_, message) = native::next(&root, &config, 2);
            let (_, finish) = native::next(&root, &config, 3);
            let mut entries = [message, finish]
                .into_iter()
                .enumerate()
                .map(|(index, loaded)| {
                    let mut actor = peers::actor_profile(&root, &config, loaded.request);
                    actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
                    let socket = root
                        .0
                        .join(format!("peers/precheck-{}.sock", loaded.request));
                    resident::Entry {
                        loaded,
                        actor,
                        precheck: Some(settings(&socket, index + 1, 2)),
                        release: if index == 0 {
                            LearnedTextRelease::Message
                        } else {
                            LearnedTextRelease::Finish
                        },
                    }
                })
                .collect::<Vec<_>>();
            entries[0].loaded.recovery_anchor =
                Some(anchors::load(&root.0.join("history-1.bin")).unwrap());
            entries[0].loaded.anchor_output = Some(root.0.join("history-2.bin"));
            let target = root.0.join("peers/precheck-3.sock");
            let selected_input = if alias {
                let parent = root.0.join("peer-alias");
                std::os::unix::fs::symlink(root.0.join("peers"), &parent).unwrap();
                parent.join("precheck-3.sock")
            } else {
                target.clone()
            };
            let next_graph = if graph {
                selected_input.clone()
            } else {
                root.0.join("absent-graph.json")
            };
            let next_inventory = if graph {
                root.0.join("absent-inventory.json")
            } else {
                selected_input
            };
            let profile = root.0.join("recovery-topology.json");
            fs::write(&profile, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{{"topology_path":{},"inventory_path":{}}},"next":{{"topology_path":{},"inventory_path":{}}},"operation":{},"source_checkpoint":{},"retain_checkpoint":{}}}"#,
                quote(root.0.join("initial.json").to_str().unwrap()),
                quote(root.0.join("initial-inventory.json").to_str().unwrap()),
                quote(next_graph.to_str().unwrap()), quote(next_inventory.to_str().unwrap()),
                saved.last_operation() + 1,
                quote(root.0.join("source-1.bin").to_str().unwrap()),
                quote(root.0.join("source-2.bin").to_str().unwrap()))).unwrap();
            custody::admit(
                &config,
                &mut entries[0].loaded,
                Some(&profile),
                custody::Mode::Live,
            )
            .unwrap();
            let canonical = config.store.join("delivery.bin");
            let staging = config.store.join("delivery.pending");
            fs::write(&staging, b"independent unacknowledged staged evidence").unwrap();
            let bytes = fs::read(&canonical).unwrap();
            let source = fs::read(root.0.join("source-1.bin")).unwrap();
            let history = fs::read(root.0.join("history-1.bin")).unwrap();
            let reviewer = fixture::peers(&root, &config);
            let result = resident::run(config, entries, &reviewer, true, Some(1), None, || {
                panic!("aliased next source reached time, the recovery Fence or a listener")
            });
            let error = result
                .err()
                .expect("future source/disclosure custody must be disjoint");
            assert!(error.contains("precheck socket conflicts"), "{error}");
            assert_eq!(fs::read(&canonical).unwrap(), bytes);
            assert_eq!(
                fs::read(&staging).unwrap(),
                b"independent unacknowledged staged evidence"
            );
            assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), source);
            assert_eq!(fs::read(root.0.join("history-1.bin")).unwrap(), history);
            assert_eq!(reader.snapshot(), before);
            assert_eq!(mediated::checkpoint(&root, 1), saved);
            assert!(!root.0.join("source-2.bin").exists());
            assert!(!root.0.join("history-2.bin").exists());
            assert!(!target.exists());
            quiescent(&root);
        }
    }
}

#[test]
fn absent_creation_topology_inputs_cannot_alias_a_future_disclosure_socket() {
    for graph in [false, true] {
        for alias in [false, true] {
            let root = Root::new();
            let (config, _, mut entries) = selected_mediated(&root);
            let sockets = select(&root, &mut entries);
            let target = sockets[2].as_ref().unwrap();
            let selected_input = if alias {
                let parent = root.0.join("peer-alias");
                std::os::unix::fs::symlink(root.0.join("peers"), &parent).unwrap();
                parent.join(target.file_name().unwrap())
            } else {
                target.clone()
            };
            let profile = root.0.join("create-topology.json");
            let original = fs::read_to_string(&profile).unwrap();
            let source_name = if graph {
                "live-1.json"
            } else {
                "inventory-1.json"
            };
            let changed = original.replace(
                root.0.join(source_name).to_str().unwrap(),
                selected_input.to_str().unwrap(),
            );
            assert_ne!(changed, original);
            fs::write(&profile, changed).unwrap();
            entries[0].loaded.topology = None;
            custody::admit(
                &config,
                &mut entries[0].loaded,
                Some(&profile),
                custody::Mode::Create,
            )
            .unwrap();
            let reviewer = fixture::peers(&root, &config);
            let store = config.store.clone();
            let result = resident::run(config, entries, &reviewer, false, None, None, || {
                panic!("future source/disclosure alias reached time or source intake")
            });
            let error = result
                .err()
                .expect("selected topology custody must be reserved first");
            assert!(error.contains("precheck socket conflicts"), "{error}");
            assert!(!store.exists());
            assert!(!root.0.join("history-1.bin").exists());
            assert!(!root.0.join("source-1.bin").exists());
            assert!(sockets.iter().flatten().all(|socket| !socket.exists()));
            quiescent(&root);
        }
    }
}
