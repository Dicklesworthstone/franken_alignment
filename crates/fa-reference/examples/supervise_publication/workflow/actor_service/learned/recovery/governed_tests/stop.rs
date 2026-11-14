//! Real authenticated Stop retains the same governed replay and custody output.
use super::*;
use crate::workflow::control::socket_path;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
    PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
    control::{StopClientPhase, StopClientProgress, StopControlReceipt},
};
use std::{
    io::{Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    thread::{self, JoinHandle},
};

fn client(stream: UnixStream, peers: &PeerProfile, operation: u64,
    lose_reply: bool) -> JoinHandle<Option<StopControlReceipt>>
{
    let expected = peers.expected;
    thread::spawn(move || {
        let peer = PeerCredentials::observe(&stream).unwrap();
        let socket = VerifiedReviewerSocket::verify(stream,
            PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()).unwrap();
        let mut client = socket.into_stop_client(expected, operation).unwrap();
        let start = Instant::now();
        loop {
            assert!(start.elapsed() < Duration::from_secs(15), "governed recovery Stop stalled");
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt(),
                _ => {}
            }
            if lose_reply && client.phase() == StopClientPhase::AwaitingReceipt {
                assert!(client.outcome_unknown());
                return None;
            }
            thread::sleep(Duration::from_millis(1));
        }
    })
}

#[test]
fn governed_recovery_stop_wins_at_early_middle_and_ready_cuts_and_retains_acknowledged_history() {
    for position in 0..3 {
        for lose_reply in [false, true] {
            let case = prepared();
            let reads = case.config.source.status().read_attempts;
            let total = case.before.revision as usize;
            let connect_at = [0, total / 2, total + 1][position];
            let peers = fixture::peers(&case.root, &case.config);
            let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
            let path = socket_path(&peers, case.loaded.request);
            let mut worker = None;
            let mut calls = 0;
            let result = recovery::open_governed_live(&case.config, &case.loaded,
                &mut control, &deadline(), &mut || {
                    let at = calls;
                    calls += 1;
                    if at == connect_at {
                        worker = Some(client(UnixStream::connect(&path).unwrap(),
                            &peers, case.loaded.request, lose_reply));
                    }
                    ElapsedTick(1000)
                });
            let error = result.err().expect("Stop must return no live owner or governor");
            assert!(error.starts_with("native admission Stop acknowledged at revision"), "{error}");
            let application = control.recovery_application().unwrap();
            assert!(application.receipt.acknowledged());
            let history = application.result.as_ref().unwrap();
            assert_eq!(history.appended_events(), 2);
            assert_eq!(history.snapshot().revision, case.before.revision + 2,
                "the terminal path must not append the ordinary live-recovery Fence first");
            assert_eq!(history.snapshot().control.ledger.elapsed, case.before.control.ledger.elapsed);
            assert_eq!(history.snapshot().control.ledger.charged, case.before.control.ledger.charged);
            assert_eq!(history.snapshot().executions, case.before.executions);
            let retained = anchors::load(case.loaded.anchor_output.as_ref().unwrap()).unwrap();
            assert_eq!(&retained, history.history_anchor());
            assert!(!case.config.store.join("delivery.pending").exists());
            let received = worker.unwrap().join().unwrap();
            assert_eq!(received, (!lose_reply).then_some(application.receipt));
            let current = FileOversight::read_publication_with_learned_generation(
                &case.config.store, &case.config.profile, &case.loaded.generation).unwrap();
            assert_eq!(current, *history.snapshot());
            assert_original_inputs(&case, reads);
            let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
            let mut retry_calls = 0;
            let retry = recovery::open_governed_live(&case.config, &case.loaded,
                &mut control, &deadline(), &mut || { retry_calls += 1; ElapsedTick(1000) });
            assert!(retry.err().unwrap().starts_with("native admission Stop acknowledged at revision"));
            assert_eq!(retry_calls, 1);
            assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), canonical);
        }
    }
}

#[test]
fn an_authenticated_bad_nonce_cannot_stop_or_fall_through_to_a_fresh_governor() {
    let case = prepared();
    let original = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let staged = fs::read(case.config.store.join("delivery.pending")).unwrap();
    let reads = case.config.source.status().read_attempts;
    let peers = fixture::peers(&case.root, &case.config);
    let mut control = Control::new(&case.config, case.loaded.request, Some(&peers)).unwrap();
    let mut stream = UnixStream::connect(socket_path(&peers, case.loaded.request)).unwrap();
    let worker = thread::spawn(move || {
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut offer = [0; 104];
        stream.read_exact(&mut offer).unwrap();
        assert_eq!(&offer[..8], b"FASTOFF1");
        offer[..8].copy_from_slice(b"FASTREQ1");
        offer[103] ^= 1;
        stream.write_all(&offer).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let result = recovery::open_governed_live(&case.config, &case.loaded,
        &mut control, &deadline(), &mut || ElapsedTick(1000));
    assert!(result.err().unwrap().contains("Binding"));
    assert!(control.recovery_application().is_none());
    assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), original);
    assert_eq!(fs::read(case.config.store.join("delivery.pending")).unwrap(), staged);
    assert!(!case.loaded.anchor_output.as_ref().unwrap().exists());
    drop(control);
    assert!(worker.join().unwrap().is_empty());
    assert_original_inputs(&case, reads);
    // Only the explicit request differs from the permitted Stop controls above;
    // a later independent invocation can still complete original live recovery.
    let ((host, _, _, _), _fresh_governor) = recovery::open_governed_with_checkpoints(
        &case.config, &case.loaded, &mut |_| Ok(())).unwrap();
    assert_eq!(host.revision(), case.before.revision + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, case.numerical);
}
