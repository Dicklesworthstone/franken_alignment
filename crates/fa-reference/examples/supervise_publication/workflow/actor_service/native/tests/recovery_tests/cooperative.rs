//! Real original journal and authenticated Stop transport at replay boundaries.
//! Synthetic weights do not establish deployment latency or detector quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    StopClientProgress, StopControlReceipt,
};
use std::thread::{self, JoinHandle};

fn limit() -> Deadline {
    Deadline {
        logical: ElapsedTick(10_000),
        started: Instant::now(),
        wall: Duration::from_secs(10),
    }
}

fn replay_length(config: &Config, native: &Inputs) -> usize {
    let selected = bootstrap::selection(config, native, None).unwrap();
    let recovery = FileOversight::begin_open_generated_text_stream_from_source(
        &config.store,
        config.profile.clone(),
        &native.decoder,
        &native.tokenizer,
        selected,
    )
    .unwrap();
    recovery.progress().total_events
}

fn stopping(reviewer: &PeerProfile, operation: u64) -> JoinHandle<StopControlReceipt> {
    // Connect before returning, so the next recovery checkpoint deterministically
    // sees this admitted peer. Only the original challenge authorizes its Stop.
    let mut client = reviewer.connect_stop_client(operation).unwrap();
    thread::spawn(move || {
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
fn native_cooperative_open_preserves_computed_prefix_and_checks_every_event_before_one_fence() {
    let root = Root::new();
    let mut config = configured(&root);
    let native = inputs(&root, &config);
    seed(&root, &mut config, &native, true, 1);
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let canonical = std::fs::read(config.store.join("delivery.bin")).unwrap();
    let before = FileOversight::read_decoder_text_progress(
        &config.store,
        &config.profile,
        &native.decoder,
        &native.tokenizer,
        7,
    )
    .unwrap();
    let total = replay_length(&config, &native);
    let mut checkpoints = 0;
    let (host, _reviewer) =
        bootstrap::open_with_checkpoints(&config, &native, selected, None, &limit(), &mut || {
            checkpoints += 1;
            assert_eq!(
                std::fs::read(config.store.join("delivery.bin")).unwrap(),
                canonical
            );
            ElapsedTick(1001)
        })
        .unwrap();
    assert_eq!(checkpoints, total + 2); // Before loading, every event, final Ready.
    assert_eq!(host.revision(), before.publication.revision + 1);
    assert_eq!(
        host.decoder_inspection().unwrap().numerical,
        before.numerical.numerical
    );
    assert_eq!(
        host.decoder_generation_progress(7)
            .unwrap()
            .generation_revision(),
        1
    );
    assert!(host.decoder_inspection().unwrap().paused);
    assert_eq!(
        host.inspect().control.ledger.charged,
        before.publication.control.ledger.charged
    );
    assert_eq!(host.inspect().executions, 0);
    assert!(host.inspect().stop.is_none());
}

#[test]
fn native_replay_deadline_at_initial_middle_and_ready_cuts_preserves_exact_history() {
    for boundary in [0, 1, 2, usize::MAX] {
        let root = Root::new();
        let mut config = configured(&root);
        let native = inputs(&root, &config);
        seed(&root, &mut config, &native, true, 1);
        let selected = bootstrap::selection(&config, &native, None).unwrap();
        let total = replay_length(&config, &native);
        let cut = if boundary == usize::MAX {
            total + 1
        } else {
            boundary
        };
        let canonical = std::fs::read(config.store.join("delivery.bin")).unwrap();
        let mut calls = 0;
        let result = bootstrap::open_with_checkpoints(
            &config,
            &native,
            selected,
            None,
            &limit(),
            &mut || {
                let current = calls;
                calls += 1;
                ElapsedTick(if current == cut { 20_000 } else { 1001 })
            },
        );
        assert!(result.is_err(), "cut {cut} acquired a live owner");
        assert_eq!(calls, cut + 1);
        assert_eq!(
            std::fs::read(config.store.join("delivery.bin")).unwrap(),
            canonical
        );
        // The failed cursor released its exclusive lock; no recoverable Stop,
        // ordinary Fence or partial live owner was written on timeout.
        assert_eq!(replay_length(&config, &native), total);
    }
}

#[test]
fn native_recovery_authenticated_stop_wins_at_first_middle_and_ready_cuts() {
    for boundary in [0, 1, 2] {
        let root = Root::new();
        let mut config = configured(&root);
        let native = inputs(&root, &config);
        seed(&root, &mut config, &native, true, 1);
        let (_, peers) = profiles(&root, &config);
        let mut control = Control::new(&config, 91, Some(&peers)).unwrap();
        let selected = bootstrap::selection(&config, &native, None).unwrap();
        let total = replay_length(&config, &native);
        let cut = [1, total / 2 + 1, total + 1][boundary];
        let before = FileOversight::read_decoder_text_progress(
            &config.store,
            &config.profile,
            &native.decoder,
            &native.tokenizer,
            7,
        )
        .unwrap();
        let mut client = None;
        let mut calls = 0;
        let error = bootstrap::open_with_checkpoints(
            &config,
            &native,
            selected,
            Some(&mut control),
            &limit(),
            &mut || {
                if calls == cut {
                    client = Some(stopping(&peers, 91));
                }
                calls += 1;
                ElapsedTick(1001)
            },
        )
        .err()
        .expect("authenticated recovery Stop issued live custody");
        assert!(
            error.starts_with("native admission Stop acknowledged at revision"),
            "{error}"
        );
        let receipt = client.unwrap().join().unwrap();
        assert!(receipt.acknowledged());
        let after = FileOversight::read_decoder_text_progress(
            &config.store,
            &config.profile,
            &native.decoder,
            &native.tokenizer,
            7,
        )
        .unwrap();
        assert_eq!(after.publication.revision, before.publication.revision + 2);
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(
            after.text.generation_revision(),
            before.text.generation_revision()
        );
        assert_eq!(after.publication.executions, before.publication.executions);
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert_eq!(
            after.publication.control.ledger.elapsed,
            before.publication.control.ledger.elapsed
        );
        assert_eq!(
            after.publication.stop.as_ref().unwrap().request().operation,
            91
        );
        let acknowledged = std::fs::read(config.store.join("delivery.bin")).unwrap();
        assert!(control.recovery_pending_peer().is_err());
        assert_eq!(
            std::fs::read(config.store.join("delivery.bin")).unwrap(),
            acknowledged
        );
    }
}

#[test]
fn native_recovery_a_connection_without_a_stop_request_never_mutates_or_falls_through() {
    let root = Root::new();
    let mut config = configured(&root);
    let native = inputs(&root, &config);
    seed(&root, &mut config, &native, true, 1);
    let (_, peers) = profiles(&root, &config);
    let mut control = Control::new(&config, 91, Some(&peers)).unwrap();
    let silent = UnixStream::connect(crate::workflow::control::socket_path(&peers, 91)).unwrap();
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let total = replay_length(&config, &native);
    let canonical = std::fs::read(config.store.join("delivery.bin")).unwrap();
    let mut calls = 0;
    let result = bootstrap::open_with_checkpoints(
        &config,
        &native,
        selected,
        Some(&mut control),
        &limit(),
        &mut || {
            calls += 1;
            ElapsedTick(if calls > total + 4 { 20_000 } else { 1001 })
        },
    );
    assert!(result.is_err());
    drop(silent);
    assert_eq!(
        std::fs::read(config.store.join("delivery.bin")).unwrap(),
        canonical
    );
    assert!(control.recovery_pending_peer().is_err());
    assert_eq!(
        std::fs::read(config.store.join("delivery.bin")).unwrap(),
        canonical
    );
}

#[test]
fn native_continuation_stop_is_available_during_replay_before_live_sources_or_helpers() {
    let root = Root::new();
    publish(&root); // Real reviewed and acknowledged original publication.
    let mut config = configured(&root);
    let mut native = inputs(&root, &config);
    native.generation = 8;
    let (mut actor, peers) = profiles(&root, &config);
    actor.request = 92;
    let store = config.store.clone();
    let profile = config.profile.clone();
    let before = FileOversight::read_publication(&store, &profile).unwrap();
    let stop_path = crate::workflow::control::socket_path(&peers, 92);
    config.programs.clear();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut client = None;
    let mut output = Vec::new();
    let error = serve_continued(
        config,
        &actor,
        &peers,
        native,
        (true, None, None, Some(91)),
        || {
            if client.is_none() && stop_path.exists() {
                assert_eq!(
                    FileOversight::read_publication(&store, &profile)
                        .unwrap()
                        .revision,
                    before.revision,
                    "Stop listener was created after recovery fenced"
                );
                client = Some(stopping(&peers, 92));
            }
            ElapsedTick(1001)
        },
        &mut output,
    )
    .unwrap_err();
    assert!(
        error.starts_with("native admission Stop acknowledged at revision"),
        "{error}"
    );
    assert!(client.unwrap().join().unwrap().acknowledged());
    assert!(output.is_empty());
    let after = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(after.revision, before.revision + 2);
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.stop.as_ref().unwrap().request().operation, 92);
    assert!(!actor.socket.exists());
    assert!(!peers.socket(92).exists());
    assert!(!stop_path.exists());
}
