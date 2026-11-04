//! Native token boundaries, expiry and the real authenticated stop transport.
use super::*;

#[test]
fn unavailable_manifest_and_insufficient_work_never_qualify_and_leave_a_usable_fresh_check() {
    for missing in [false, true] {
        let root = Root::new(); let mut case = Case::new(&root, Some(true), false, false);
        let before = case.numerical(); let cost = case.cost();
        if missing { fs::remove_file(root.0.join("observed.manifest")).unwrap(); }
        else { case.loaded.identity.as_mut().unwrap().budget.scalar_products = 1; }
        assert!(case.measure().is_err());
        assert!(case.identity.as_ref().unwrap().history().is_empty(), "failed setup executed no token");
        let host = case.driver.supervisor().host().unwrap();
        assert!(host.identity_installation(901).is_err()
            || host.identity_installation(901).unwrap().is_none());
        if !missing { assert_eq!(host.identity_report(901).unwrap().outcome, IdentityOutcome::Unavailable); }
        assert_eq!(host.inspect().executions, 0); drop(host);
        assert_eq!(case.numerical().numerical, before.numerical); assert_eq!(case.cost(), cost);
        fs::write(root.0.join("observed.manifest"), identity_wire::encode_manifest(&manifest())).unwrap();
        let selection = case.loaded.identity.as_mut().unwrap();
        selection.check = 902; selection.measurement_sequence = 2;
        selection.budget.scalar_products = MAX_DECODER_PRODUCTS;
        assert!(case.measure().unwrap(), "setup refusal returned the unique usable observer");
        assert_eq!(case.identity.as_ref().unwrap().history()[0].work().completed_tokens, 5);
        case.publish().close();
    }
}

#[test]
fn workflow_or_native_expiry_after_one_token_retains_work_and_never_installs_a_partial_match() {
    for expiry in [None, Some(11_000), Some(61_000)] {
        let root = Root::new(); let mut case = Case::new(&root, Some(true), false, false);
        let before = case.numerical(); let cost = case.cost();
        let deadline = deadline(); let mut control = Control::new(&case.config, 1, None).unwrap();
        let mut calls = 0;
        let result = check::prepare(&mut case.driver, &mut case.config, &case.loaded,
            &mut case.identity, (&mut control, &case.reviewer, &deadline), &mut || {
                calls += 1;
                // Preflight/source clocks precede the first pre/post-token
                // pair. The work assertions prove expiry followed real work.
                ElapsedTick(if calls == 7 { expiry.unwrap_or(1000) } else { 1000 })
            });
        let history = case.identity.as_ref().unwrap().history();
        assert_eq!(history.len(), 1);
        if expiry.is_some() {
            assert!(result.is_err());
            assert_eq!(history[0].status(), ComputedIdentityStatus::Cancelled);
            assert_eq!(history[0].work().entered_tokens, 1);
            assert_eq!(history[0].work().completed_tokens, 1);
            assert_eq!(history[0].work().measured_anchors, 0);
            assert!(history[0].work().completed_scalar_products > 0);
            assert!(history[0].result().is_none());
            let host = case.driver.supervisor().host().unwrap();
            assert_eq!(host.identity_report(901).unwrap().outcome, IdentityOutcome::Unavailable);
            assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
            assert!(host.identity_installation(901).unwrap().is_none()); drop(host);
            let selection = case.loaded.identity.as_mut().unwrap();
            selection.check = 902; selection.measurement_sequence = 2;
            assert!(case.measure().unwrap());
            let history = case.identity.as_ref().unwrap().history();
            assert_eq!(history.len(), 2);
            assert_eq!(history.iter().map(|run| run.work().completed_tokens).sum::<u64>(), 6);
        } else {
            assert!(result.unwrap());
            assert_eq!(history[0].status(), ComputedIdentityStatus::Committed);
            assert_eq!(history[0].work().completed_tokens, 5);
        }
        assert_eq!(case.numerical().numerical, before.numerical);
        assert_eq!(case.numerical().pending, before.pending); assert_eq!(case.cost(), cost);
        assert_eq!(case.driver.supervisor().host().unwrap().inspect().executions, 0);
        case.publish().close();
    }
}

#[test]
#[cfg(target_os = "linux")]
fn authenticated_stop_after_one_identity_token_cancels_preparation_before_native_stop_mutates_the_owner() {
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket, control::StopClientProgress,
    };
    use std::os::unix::net::UnixStream;
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let mut case = Case::new(&root, mode, false, false);
        let peers = fixture::peers(&root, &case.config);
        let path = crate::workflow::control::socket_path(&peers, 1);
        let expected = peers.expected;
        let mut control = Control::new(&case.config, 1, Some(&peers)).unwrap();
        let (socket, _other) = UnixStream::pair().unwrap();
        let credentials = PeerCredentials::observe(&socket).unwrap();
        let policy = PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap();
        let before = case.numerical(); let cost = case.cost(); let deadline = deadline();
        let mut client = None; let mut calls = 0;
        let result = check::prepare(&mut case.driver, &mut case.config, &case.loaded,
            &mut case.identity, (&mut control, &case.reviewer, &deadline), &mut || {
                calls += 1;
                if calls == 7 {
                    // Connect synchronously at the first completed token, then
                    // let the independent client drive the actual native wire.
                    let socket = UnixStream::connect(&path).unwrap();
                    client = Some(std::thread::spawn(move || {
                        let socket = VerifiedReviewerSocket::verify(socket, policy).unwrap();
                        let mut client = socket.into_stop_client(expected, 1).unwrap();
                        let start = Instant::now();
                        loop {
                            assert!(start.elapsed() < Duration::from_secs(10));
                            match client.step().unwrap() {
                                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                                StopClientProgress::Complete => return client.receipt().unwrap(),
                                _ => std::thread::sleep(Duration::from_millis(1)),
                            }
                        }
                    }));
                }
                ElapsedTick(1000)
            });
        assert!(!result.unwrap());
        let receipt = client.unwrap().join().unwrap();
        assert!(receipt.acknowledged()); assert!(receipt.drained());
        let history = case.identity.as_ref().unwrap().history();
        assert_eq!(history.len(), 1); assert_eq!(history[0].status(), ComputedIdentityStatus::Cancelled);
        assert_eq!(history[0].work().completed_tokens, 1); assert!(history[0].result().is_none());
        let host = case.driver.supervisor().host().unwrap();
        assert_eq!(host.identity_report(901).unwrap().outcome, IdentityOutcome::Unavailable);
        assert!(host.identity_installation(901).unwrap().is_none());
        assert!(host.inspect().stop.is_some()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0); drop(host);
        assert_eq!(case.numerical().numerical, before.numerical);
        assert_eq!(case.numerical().pending, before.pending); assert_eq!(case.cost(), cost);
        case.close();
    }
}
