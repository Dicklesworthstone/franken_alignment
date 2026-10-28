//! Causal failures retain the original cut; checked identity is not a Stop request.
use super::*;
use std::net::Shutdown;

#[derive(Clone, Copy, Debug)]
enum BadRequest { Nonce, Operation, Truncated }
fn malformed_client(mut stream: UnixStream, attack: BadRequest) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut offer = [0; 104];
        stream.read_exact(&mut offer).unwrap();
        assert_eq!(&offer[..8], b"FASTOFF1");
        assert_ne!(&offer[72..], &[0; 32]);
        offer[..8].copy_from_slice(b"FASTREQ1");
        match attack {
            BadRequest::Nonce => offer[103] ^= 1,
            BadRequest::Operation => offer[71] ^= 1,
            BadRequest::Truncated => {}
        }
        let count = if matches!(attack, BadRequest::Truncated) { 103 } else { 104 };
        stream.write_all(&offer[..count]).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
        reply
    })
}

#[test]
fn authenticated_nonce_operation_and_partial_frame_failures_never_stop_or_clean_staging() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            let case = prepared_native(mode, mediated, false);
            let original = files(&case.config.store);
            for attack in [BadRequest::Nonce, BadRequest::Operation, BadRequest::Truncated] {
                let peers = native_peers(&case, false, 2);
                let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
                let stream = UnixStream::connect(socket_path(&peers, case.loaded.request)).unwrap();
                let client = malformed_client(stream, attack);
                let result = recovery::open_selected_live_with_topology(&case.config, &case.loaded,
                    &mut control, &deadline(), &mut || ElapsedTick(1000));
                let error = result.err().expect("malformed request cannot create live custody or Stop");
                let expected = if matches!(attack, BadRequest::Truncated) { "UnexpectedEof" } else { "Binding" };
                assert!(error.contains(expected), "{attack:?}: {error}");
                assert!(control.recovery_application().is_none());
                assert_eq!(files(&case.config.store), original);
                let mut retries = 0;
                assert_eq!(recovery::open_selected_live_with_topology(&case.config, &case.loaded,
                    &mut control, &deadline(), &mut || { retries += 1; ElapsedTick(1000) })
                    .err().as_deref(), Some(error.as_str()));
                assert_eq!(retries, 1);
                assert_eq!(files(&case.config.store), original);
                drop(control);
                assert!(client.join().unwrap().is_empty(), "no native receipt exists before a full exact request");
                assert_inputs_retained(&case);
            }
            // Same history, operation, credentials and guards; only the explicit
            // frame changes. The original StopClient now receives a native receipt.
            let (expected, canonical) = native_oracle(&case);
            let exchange = exchange(&case, 0, false, |_, _| ElapsedTick(1000));
            assert_stopped(&case, &exchange, &expected, &canonical);
            assert_eq!(exchange.client.receipt, Some(exchange.application.as_ref().unwrap().receipt));
        }
    }
}

#[test]
fn silent_checked_peer_may_read_the_offer_but_only_an_explicit_request_can_stop() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            let case = prepared_native(mode, mediated, false);
            let original = files(&case.config.store);
            let peers = native_peers(&case, false, 2);
            let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
            let mut client = UnixStream::connect(socket_path(&peers, case.loaded.request)).unwrap();
            let limit = deadline();
            let total = case.before.revision as usize;
            let mut calls = 0;
            let result = recovery::open_selected_live_with_topology(&case.config, &case.loaded,
                &mut control, &limit, &mut || {
                    let cut = calls;
                    calls += 1;
                    // N+2 replay checks, then one Offering step. The very next
                    // original budget check expires while no request exists.
                    if cut == total + 3 { limit.logical } else { ElapsedTick(1000) }
                });
            assert_eq!(result.err().as_deref(), Some("one-shot workflow deadline reached"));
            assert_eq!(calls, total + 4);
            assert!(control.recovery_application().is_none());
            assert_eq!(files(&case.config.store), original);
            drop(control);
            client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
            let mut received = Vec::new();
            client.read_to_end(&mut received).unwrap();
            assert_eq!(received.len(), 104);
            assert_eq!(&received[..8], b"FASTOFF1");
            assert_ne!(&received[72..], &[0; 32]);
            assert_inputs_retained(&case);
            let (expected, canonical) = native_oracle(&case);
            let exchange = exchange(&case, 0, false, |_, _| ElapsedTick(1000));
            assert_stopped(&case, &exchange, &expected, &canonical);
        }
    }
}

#[test]
fn authenticated_peer_does_not_relax_source_floors_or_independent_history_binding() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            let mut case = prepared_native(mode, mediated, false);
            let foreign = prepared_native(mode, mediated, false);
            let foreign_anchor: FileHistoryAnchor = anchors::load(&foreign.anchor).unwrap();
            let original = files(&case.config.store);
            let source = case.config.source_policy;
            let floor = case.loaded.floor;
            let anchor = case.loaded.recovery_anchor.clone();
            for changed in 0..5 {
                match changed {
                    0 => case.config.source_policy.source.generation += 1,
                    1 => case.loaded.floor.journal_revision = u64::MAX,
                    2 => case.loaded.floor.control_sequence = u64::MAX,
                    3 => case.loaded.floor.authority_epoch = u64::MAX,
                    4 => case.loaded.recovery_anchor = Some(foreign_anchor.clone()),
                    _ => unreachable!(),
                }
                let peers = native_peers(&case, false, 2);
                let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
                let socket = UnixStream::connect(socket_path(&peers, case.loaded.request)).unwrap();
                let client = stop_client(socket, &peers, case.loaded.request, false);
                let error = recovery::open_selected_live_with_topology(&case.config, &case.loaded,
                    &mut control, &deadline(), &mut || ElapsedTick(1000)).err().unwrap();
                assert!(!error.starts_with("native admission Stop acknowledged"), "{error}");
                assert!(control.recovery_application().is_none());
                assert_eq!(files(&case.config.store), original);
                drop(control);
                let client = client.join().unwrap();
                assert!(client.binding.is_none(), "guards must be complete before any offer");
                assert!(client.receipt.is_none());
                assert!(client.failure.is_some());
                case.config.source_policy = source;
                case.loaded.floor = floor;
                case.loaded.recovery_anchor = anchor.clone();
                assert_inputs_retained(&case);
            }
            let (expected, canonical) = native_oracle(&case);
            let exchange = exchange(&case, 0, false, |_, _| ElapsedTick(1000));
            assert_stopped(&case, &exchange, &expected, &canonical);
            assert_exported(&case, &expected);
        }
    }
}

#[test]
fn canonical_change_after_ready_never_returns_a_false_native_acknowledgment() {
    for mediated in [false, true] {
        for mode in [None, Some(false), Some(true)] {
            let case = prepared_native(mode, mediated, true);
            let original = files(&case.config.store);
            let (_expected, competing) = native_oracle(&case);
            let total = case.before.revision as usize;
            let mut changed = false;
            let exchange = exchange(&case, 0, false, |cut, _| {
                if cut == total + 2 {
                    // This is a genuine native terminal successor, computed
                    // independently above, introduced after target selection.
                    assert!(!changed);
                    changed = true;
                    fs::write(case.config.store.join("delivery.bin"), &competing).unwrap();
                }
                ElapsedTick(1000)
            });
            assert!(changed);
            assert!(exchange.terminal.starts_with("native recovery Stop completion is unconfirmed"),
                "{}", exchange.terminal);
            let application = exchange.application.as_ref().unwrap();
            assert!(matches!(&application.result, Err(JournalError::Contract(Error::Binding))));
            assert_eq!(application.receipt.status, StopStatus::Unconfirmed);
            assert!(!application.receipt.acknowledged());
            assert_eq!(application.receipt.control_sequence, 0);
            assert_eq!(application.receipt.revocation_floor, 0);
            assert_eq!(application.receipt.dispatcher_epoch, 0);
            assert_eq!(exchange.client.receipt, Some(application.receipt));
            assert!(exchange.client.outcome_unknown);
            let mut injected = original;
            injected.insert("delivery.bin".to_owned(), competing);
            assert_eq!(files(&case.config.store), injected);
            assert!(!case.loaded.anchor_output.as_ref().unwrap().exists());
            if let Some(path) = &case.source_output { assert!(!path.exists()); }
            assert_inputs_retained(&case);
        }
    }
}

#[test]
fn original_candidate_quota_and_session_allowance_are_not_renewed_during_replay() {
    let case = prepared_native(Some(true), true, false);
    let original = files(&case.config.store);
    let peers = native_peers(&case, true, 2);
    let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
    let path = socket_path(&peers, case.loaded.request);
    let mut candidates = [UnixStream::connect(&path).unwrap(), UnixStream::connect(&path).unwrap()];
    let mut calls = 0;
    let error = recovery::open_selected_live_with_topology(&case.config, &case.loaded,
        &mut control, &deadline(), &mut || { calls += 1; ElapsedTick(1000) }).err().unwrap();
    assert_eq!(error, "reviewer candidate quota exhausted; no review decision accepted");
    assert_eq!(calls, 2);
    assert!(control.recovery_application().is_none());
    assert_eq!(files(&case.config.store), original);
    // The exhausted service remains terminal even after both rejected sockets
    // leave its queue. A retry cannot get a fresh quota or a live recovery fence.
    let mut retried = 0;
    assert_eq!(recovery::open_selected_live_with_topology(&case.config, &case.loaded,
        &mut control, &deadline(), &mut || { retried += 1; ElapsedTick(1000) })
        .err().as_deref(), Some(error.as_str()));
    assert_eq!(retried, 1);
    assert!(control.recovery_application().is_none());
    assert_eq!(files(&case.config.store), original);
    drop(control);
    for client in &mut candidates {
        client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
    }
    let mut peers = native_peers(&case, false, 2);
    peers.runtime_ms = 20;
    let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
    let mut client = UnixStream::connect(socket_path(&peers, case.loaded.request)).unwrap();
    let mut calls = 0;
    let error = recovery::open_selected_live_with_topology(&case.config, &case.loaded,
        &mut control, &deadline(), &mut || {
            calls += 1;
            if calls == 2 {
                // Admission happened at the first boundary. Expire that same
                // real session before the first event; Ready cannot renew it.
                thread::sleep(Duration::from_millis(30));
            }
            ElapsedTick(1000)
        }).err().unwrap();
    assert_eq!(error, "authenticated stop session timed out");
    assert_eq!(calls, 2);
    assert!(control.recovery_application().is_none());
    assert_eq!(files(&case.config.store), original);
    drop(control);
    client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
    assert_inputs_retained(&case);
    let (expected, canonical) = native_oracle(&case);
    let exchange = exchange(&case, 0, false, |_, _| ElapsedTick(1000));
    assert_stopped(&case, &exchange, &expected, &canonical);
}
