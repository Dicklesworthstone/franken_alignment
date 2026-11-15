//! The same admitted peer keeps its lifetime through a deferred driver handoff.
//! Real native owners and sockets establish control behavior, not deployment
//! latency or an identity stronger than the selected kernel peer policy.
use super::*;

#[test]
fn deferred_stop_keeps_its_original_remaining_session_and_exact_retry() {
    for elapsed in [Duration::ZERO, Duration::from_secs(45)] {
        let root = Directory::new();
        let c = configured(&root, false);
        let mut profile = profile(&root, &c, false);
        profile.runtime_ms = 60_000;
        let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
            .unwrap();
        let (_port, mut driver) = host.into_supervised_driver();
        let mut control = Control::new(&c, 1, Some(&profile)).unwrap();
        let mut peer = client(&profile);
        assert!(control.pending_peer().unwrap());
        // Set the retained admission cut instead of making the test wait for
        // native inference. The final quarter of the original budget remains.
        let admitted = Instant::now().checked_sub(elapsed).unwrap();
        control.service.as_ref().unwrap().borrow_mut().pending_since = Some(admitted);
        let client = std::thread::spawn(move || {
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(10));
                match peer.step().unwrap() {
                    StopClientProgress::NeedsDecision => peer.request_stop().unwrap(),
                    StopClientProgress::Complete => return peer.receipt().unwrap(),
                    _ => pause(1),
                }
            }
        });
        let mut handoff = control.share();
        assert!(
            handoff
                .checkpoint(&mut driver, &reviewer, &deadline(), &mut || ElapsedTick(
                    1000
                ))
                .unwrap()
        );
        let receipt = client.join().unwrap();
        assert!(receipt.acknowledged());
        assert!(receipt.drained());
        let completed = driver.supervisor().host().unwrap().inspect();
        assert!(completed.stop.is_some());
        assert_eq!(completed.executions, 0);
        assert_eq!(completed.control.ledger.charged, 0);
        assert_eq!(
            control.service.as_ref().unwrap().borrow().pending_since,
            Some(admitted)
        );
        assert!(
            control
                .checkpoint(&mut driver, &reviewer, &deadline(), &mut || panic!(
                    "Stop retry observed time"
                ))
                .unwrap()
        );
        assert_eq!(driver.supervisor().host().unwrap().inspect(), completed);
    }
}

#[test]
fn expired_accepted_peer_cannot_obtain_a_fresh_offer_or_session_after_handoff() {
    // Both the ordinary numerical-review and cooperative-recovery admission
    // paths can retain a peer before handing it to the original live driver.
    for recovery_admission in [false, true] {
        let root = Directory::new();
        let c = configured(&root, false);
        let profile = profile(&root, &c, false);
        let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
            .unwrap();
        let (_port, mut driver) = host.into_supervised_driver();
        let mut control = Control::new(&c, 1, Some(&profile)).unwrap();
        let mut peer = UnixStream::connect(socket_path(&profile, 1)).unwrap();
        assert!(if recovery_admission {
            control.recovery_pending_peer().unwrap()
        } else {
            control.pending_peer().unwrap()
        });
        let admitted = Instant::now()
            .checked_sub(Duration::from_millis(profile.runtime_ms + 1))
            .unwrap();
        control.service.as_ref().unwrap().borrow_mut().pending_since = Some(admitted);
        let before = driver.supervisor().host().unwrap().inspect();
        let mut handoff = control.share();
        let failure = handoff
            .checkpoint(&mut driver, &reviewer, &deadline(), &mut || {
                panic!("expired peer observed time")
            })
            .unwrap_err();
        assert_eq!(failure, "authenticated stop session timed out");
        peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        assert_eq!(
            peer.read(&mut [0; 1]).unwrap(),
            0,
            "expired peer received protocol bytes"
        );
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
        assert!(
            !control.native_cleanup_started(),
            "expiry is not an authorized Stop"
        );
        assert_eq!(
            control.service.as_ref().unwrap().borrow().pending_since,
            Some(admitted)
        );
        assert_eq!(
            control.checkpoint(&mut driver, &reviewer, &deadline(), &mut || panic!(
                "expiry retry observed time"
            )),
            Err(failure)
        );
        assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    }
}

#[test]
fn slow_clock_cannot_extend_the_admitted_stop_session_before_its_first_offer() {
    let root = Directory::new();
    let c = configured(&root, false);
    let mut profile = profile(&root, &c, false);
    profile.runtime_ms = 1_000;
    let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    let (_port, mut driver) = host.into_supervised_driver();
    let mut control = Control::new(&c, 1, Some(&profile)).unwrap();
    let mut peer = UnixStream::connect(socket_path(&profile, 1)).unwrap();
    assert!(control.pending_peer().unwrap());
    let before = driver.supervisor().host().unwrap().inspect();
    let mut observed_time = false;
    let failure = control
        .checkpoint(&mut driver, &reviewer, &deadline(), &mut || {
            assert!(!observed_time, "expired exchange sampled another clock");
            observed_time = true;
            std::thread::sleep(Duration::from_millis(profile.runtime_ms + 20));
            ElapsedTick(1000)
        })
        .unwrap_err();
    assert!(observed_time);
    assert_eq!(failure, "authenticated stop session timed out");
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    assert!(!control.native_cleanup_started());
    assert_eq!(
        control
            .share()
            .checkpoint(&mut driver, &reviewer, &deadline(), &mut || panic!(
                "slow clock renewed session"
            )),
        Err(failure)
    );
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
}

#[test]
fn acknowledged_stop_drains_its_original_reply_after_session_expiry() {
    let root = Directory::new();
    let c = configured(&root, false);
    let mut profile = profile(&root, &c, false);
    profile.runtime_ms = 1_000;
    let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    let (_port, mut driver) = host.into_supervised_driver();
    let mut control = Control::new(&c, 1, Some(&profile)).unwrap();
    let controller = stopping(profile.clone(), socket_path(&profile, 1), false);
    let before_revision = driver.supervisor().host().unwrap().revision();
    let mut elapsed_after_ack = false;
    let limit = deadline();
    while !control
        .checkpoint(&mut driver, &reviewer, &limit, &mut || {
            let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
            if state.stop.is_some() && !elapsed_after_ack {
                assert!(state.revision > before_revision);
                assert_eq!(state.executions, 0);
                // Native Stop has already acknowledged before its drain clock.
                // Its existing finite cleanup allowance must still carry the
                // reply; session expiry cannot erase or repeat the transition.
                elapsed_after_ack = true;
                std::thread::sleep(Duration::from_millis(profile.runtime_ms + 20));
            }
            ElapsedTick(1000)
        })
        .unwrap()
    {
        assert!(limit.started.elapsed() < Duration::from_secs(10));
        pause(1);
    }
    assert!(elapsed_after_ack);
    let receipt = controller.join().unwrap().unwrap();
    assert!(receipt.acknowledged());
    assert!(receipt.drained());
    let completed = driver.supervisor().host().unwrap().inspect();
    assert!(completed.stop.is_some());
    assert_eq!(completed.executions, 0);
    assert!(
        control
            .checkpoint(&mut driver, &reviewer, &limit, &mut || panic!(
                "acknowledged Stop retried"
            ))
            .unwrap()
    );
    assert_eq!(driver.supervisor().host().unwrap().inspect(), completed);
}
