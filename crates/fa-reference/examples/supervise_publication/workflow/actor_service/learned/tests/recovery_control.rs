//! Live recovery deadlines and real Linux credential admission at original cuts.
//! A retained stop connection aborts replay before any Stop protocol or receipt;
//! these tests do not establish a bound on individual numerical or I/O latency.
#![cfg(target_os = "linux")]
use super::*;
use super::super::prediction;
use crate::workflow::control::socket_path;
use fa_reference::action::consequence::oversight::learned_host::HostedLearnedInspection;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::PeerCredentials;
use std::{collections::BTreeMap, io::{ErrorKind, Read}, os::unix::net::UnixStream, path::Path};

const STOP_REFUSAL: &str = "learned recovery interrupted by authenticated stop peer; no durable Stop applied or acknowledged";

struct Case {
    root: Root,
    config: Config,
    loaded: Loaded,
    numerical: HostedLearnedInspection,
    mode: Option<bool>,
}
fn prepared(mode: Option<bool>) -> Case {
    let root = Root::new();
    let mut config = configured(&root);
    let path = fixture::write(&root, b"allow");
    if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer, _identity) = recovery::create_selected(&config, &loaded).unwrap();
    let mut forecast = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let wire = ActorWire::new(port);
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000), forecast.as_mut()).unwrap());
    let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 4);
    assert_eq!(numerical.sampled_draws, 2);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(wire);
    drop(forecast);
    fs::write(config.store.join("delivery.pending"), b"unacknowledged recovery staging").unwrap();
    Case { root, config, loaded, numerical, mode }
}
fn files(store: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(store).unwrap().map(|entry| {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        (entry.file_name().into_string().unwrap(), fs::read(entry.path()).unwrap())
    }).collect()
}
fn events(case: &Case) -> usize {
    let mut total = None;
    assert_eq!(recovery::open_selected_with_checkpoints(&case.config, &case.loaded, &mut |view| {
        if let Some(progress) = view {
            assert_eq!(progress.replayed_events, 0);
            total = Some(progress.total_events);
            Err("inspect original cut without finishing".into())
        } else { Ok(()) }
    }).err().as_deref(), Some("inspect original cut without finishing"));
    let total = total.unwrap();
    assert!(total > 4);
    total
}
fn peer_profile(case: &Case, wrong_pid: bool) -> PeerProfile {
    let (socket, _client) = UnixStream::pair().unwrap();
    let peer = PeerCredentials::observe(&socket).unwrap();
    let reviewer_pid = if wrong_pid { peer.pid().checked_add(1).unwrap() } else { peer.pid() };
    let scope = case.config.profile.delivery.scope;
    PeerProfile::decode(format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":{},"socket_directory":"{}/peers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        case.config.profile.human.reviewer_id, case.root.0.display(),
        peer.uid(), peer.gid(), peer.pid(), peer.uid(), peer.gid(), reviewer_pid).as_bytes()).unwrap()
}
fn recovered(case: &Case, host: &FileOversight, observer: bool, total: usize) {
    assert_eq!(host.revision(), total as u64 + 1);
    let current = host.learned_generation_inspection().unwrap();
    assert_eq!(current.numerical, case.numerical);
    assert!(current.paused);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
    assert!(host.inspect().stop.is_none());
    assert_eq!(observer, case.mode.is_some());
    assert!(!case.config.store.join("delivery.pending").exists());
    if case.mode.is_some() {
        let prediction = host.action_consistency_snapshot().unwrap();
        assert!(prediction.coverage_lost);
        assert_eq!(prediction.evidence.samples(), 0);
        assert!(prediction.pending_attempt.is_some());
    }
    if case.mode == Some(true) {
        let prediction = host.learned_action_consistency_snapshot().unwrap();
        assert!(prediction.work.probe_coordinates > 0);
        assert!(prediction.retained_source_bytes > 0);
    }
}

#[test]
fn live_deadline_cancels_before_open_every_event_and_ready_without_changing_the_original_cut() {
    for mode in [None, Some(false), Some(true)] {
        let case = prepared(mode);
        let original = files(&case.config.store);
        let total = events(&case);
        let peers = peer_profile(&case, false);
        let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
        // N+2 clock boundaries: before open, before each of N events, and Ready.
        // Every failed prefix remains in the denominator; no store is recreated.
        for expire_at in 0..=total + 1 {
            let limit = deadline();
            let mut calls = 0;
            let result = recovery::open_selected_live(&case.config, &case.loaded,
                &mut control, &limit, &mut || {
                    let cut = calls;
                    calls += 1;
                    if cut == expire_at { limit.logical } else { ElapsedTick(1000) }
                });
            assert_eq!(result.err().as_deref(), Some("one-shot workflow deadline reached"));
            assert_eq!(calls, expire_at + 1);
            assert_eq!(files(&case.config.store), original);
            // The next original guarded admission succeeds: cancellation released
            // the writer lock and did not require cleanup or a Stop receipt.
            assert_eq!(events(&case), total);
        }
        let limit = deadline();
        let mut calls = 0;
        let (host, _human, observer, _identity) = recovery::open_selected_live(&case.config, &case.loaded,
            &mut control, &limit, &mut || { calls += 1; ElapsedTick(1000) }).unwrap();
        assert_eq!(calls, total + 2);
        recovered(&case, &host, observer.is_some(), total);
        assert_ne!(fs::read(case.config.store.join("delivery.bin")).unwrap(), original["delivery.bin"]);
    }
}

#[test]
fn authenticated_stop_presence_cancels_at_every_live_cut_without_protocol_bytes_or_acknowledgment() {
    for mode in [None, Some(false), Some(true)] {
        let case = prepared(mode);
        let original = files(&case.config.store);
        let total = events(&case);
        let peers = peer_profile(&case, false);
        let path = socket_path(&peers, case.loaded.request);
        for connect_at in 0..=total + 1 {
            let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
            let limit = deadline();
            let mut calls = 0;
            let mut client = None;
            let result = recovery::open_selected_live(&case.config, &case.loaded,
                &mut control, &limit, &mut || {
                    let cut = calls;
                    calls += 1;
                    if cut == connect_at {
                        // connect returns after this real candidate is queued;
                        // the following pending_peer check performs kernel admission.
                        client = Some(UnixStream::connect(&path).unwrap());
                    }
                    ElapsedTick(1000)
                });
            assert_eq!(result.err().as_deref(), Some(STOP_REFUSAL));
            assert_eq!(calls, connect_at + 1);
            assert_eq!(files(&case.config.store), original);
            assert_eq!(events(&case), total);
            let mut client = client.unwrap();
            client.set_nonblocking(true).unwrap();
            assert_eq!(client.read(&mut [0; 1]).unwrap_err().kind(), ErrorKind::WouldBlock);
            assert!(control.pending_peer().unwrap());
            // Retained authenticated custody is not discarded to restart replay.
            let mut retry_calls = 0;
            assert_eq!(recovery::open_selected_live(&case.config, &case.loaded, &mut control,
                &limit, &mut || { retry_calls += 1; ElapsedTick(1000) }).err().as_deref(),
                Some(STOP_REFUSAL));
            assert_eq!(retry_calls, 1);
            assert_eq!(files(&case.config.store), original);
            drop(control);
            assert!(!path.exists());
            assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
        }
        // Near-identical permitted counterpart: the same history and admitted
        // peer policy with no connection reaches exactly the original fence.
        let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
        let (host, _human, observer, _identity) = recovery::open_selected_live(&case.config, &case.loaded,
            &mut control, &deadline(), &mut || ElapsedTick(1000)).unwrap();
        recovered(&case, &host, observer.is_some(), total);
    }
}

#[test]
fn rejected_process_credentials_cannot_cancel_live_replay_or_receive_protocol_bytes() {
    for mode in [None, Some(false), Some(true)] {
        let case = prepared(mode);
        let original = files(&case.config.store);
        let total = events(&case);
        // Only the independently selected reviewer PID differs from the admitted
        // case above. Supervisor identity, scope, socket and finite quota are real.
        let peers = peer_profile(&case, true);
        let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
        let mut client = UnixStream::connect(socket_path(&peers, case.loaded.request)).unwrap();
        let mut calls = 0;
        let (host, _human, observer, _identity) = recovery::open_selected_live(&case.config, &case.loaded,
            &mut control, &deadline(), &mut || { calls += 1; ElapsedTick(1000) }).unwrap();
        assert_eq!(calls, total + 2);
        client.set_nonblocking(true).unwrap();
        assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
        assert!(!control.pending_peer().unwrap());
        recovered(&case, &host, observer.is_some(), total);
        assert_ne!(fs::read(case.config.store.join("delivery.bin")).unwrap(), original["delivery.bin"]);
    }
}

#[test]
fn continuation_entry_uses_live_recovery_before_request_checks_or_a_fence() {
    for mode in [None, Some(false), Some(true)] {
        for connect in [false, true] {
            let case = prepared(mode);
            let original = files(&case.config.store);
            let total = events(&case);
            let peers = peer_profile(&case, false);
            let socket = socket_path(&peers, case.loaded.request);
            let store = case.config.store.clone();
            let expiry = ElapsedTick(1000 + case.loaded.ttl_ms.min(case.config.timing.runtime_ms));
            let Case { root, config, loaded, .. } = case;
            let mut calls = 0;
            let mut client = None;
            let result = run(config, loaded, &peers, true, || {
                calls += 1;
                if calls == 1 { return ElapsedTick(1000); }
                // The actual --continue-generation route already established
                // its original deadline and authenticated stop socket. Its next
                // clock read must be the live pre-open recovery checkpoint.
                assert_eq!(calls, 2);
                if connect {
                    client = Some(UnixStream::connect(&socket).unwrap());
                    ElapsedTick(1000)
                } else { expiry }
            });
            assert_eq!(result.err().as_deref(), Some(if connect {
                STOP_REFUSAL
            } else { "one-shot workflow deadline reached" }));
            assert_eq!(calls, 2);
            assert_eq!(files(&store), original);
            assert!(!socket.exists());
            if let Some(mut client) = client {
                client.set_nonblocking(true).unwrap();
                assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
            }
            // The real command returned before constructing any owner/roles;
            // its aborted cursor and stop socket released their original locks.
            let config = configured(&root);
            let loaded = recipe::load(&root.0.join("recipe.json"), &config, false).unwrap();
            assert_eq!(recovery::open_selected_with_checkpoints(&config, &loaded, &mut |view| {
                if let Some(progress) = view {
                    assert_eq!(progress.replayed_events, 0);
                    assert_eq!(progress.total_events, total);
                    Err("continuation replay lock was released".into())
                } else { Ok(()) }
            }).err().as_deref(), Some("continuation replay lock was released"));
            assert_eq!(files(&store), original);
        }
    }
}
