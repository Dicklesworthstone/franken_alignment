//! The original durable driver can finish containment after ordinary saturation.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{CapacityDrain, FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::round::Verdict;
use fa_reference::Error;

fn limited() -> Rig {
    let root = Directory::new();
    let mut p = profile(); p.delivery.limits.events = 32;
    let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (port, driver) = host.into_supervised_driver();
    Rig { root, port, driver, reviewer, clients: Clients::new(), inputs: None }
}
fn request(rig: &Rig) -> StopRequest {
    let control = rig.driver.supervisor().host().unwrap().inspect().control;
    StopRequest { operation: 17, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }
}
fn fill(rig: &mut Rig) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1)).unwrap();
    }
}

#[test]
fn stop_is_acknowledged_before_a_bad_clock_and_retry_spends_no_second_stop_event() {
    let mut rig = limited();
    let ticket = rig.submit(1); rig.reviewed(1);
    let key = rig.human(1001, 31);
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
    fill(&mut rig);
    let request = request(&rig);
    let before = rig.driver.supervisor().host().unwrap().revision();
    let first = rig.driver.stop_with_recovery_reserve(request, || ElapsedTick(0)).unwrap();
    assert_eq!(first.drain, Err(JournalError::Contract(Error::Stale)));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), before + 1);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    let second = rig.driver.stop_with_recovery_reserve(request, || ElapsedTick(2)).unwrap();
    assert_eq!(second.stop, first.stop);
    assert!(matches!(second.drain, Ok(CapacityDrain::Advanced(ref swept)) if swept.progress.drained()));
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), before + 2);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
    let last = rig.driver.stop_with_recovery_reserve(request, || panic!("a drained exact retry needs no clock")).unwrap();
    assert!(matches!(last.drain, Ok(CapacityDrain::AlreadyDrained(ref progress)) if progress.drained()));
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), before + 2);
}

#[test]
fn capacity_recovery_acknowledges_an_actual_publication_without_refunding_it() {
    let mut rig = limited(); let ticket = rig.submit(1); rig.reviewed(1);
    let key = rig.human(1001, 31);
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    fill(&mut rig);
    let request = request(&rig);
    let result = rig.driver.stop_with_recovery_reserve(request, || ElapsedTick(2)).unwrap();
    assert!(matches!(result.drain, Ok(CapacityDrain::Advanced(ref swept)) if swept.progress.drained() && swept.progress.charged_units == 16));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let host = rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"publication");
    assert_eq!(host.inspect().control.ledger.available, 84);
}

#[test]
fn no_reserve_or_stale_stop_preflight_preserves_a_healthy_review_and_positive_publication() {
    for configured in [false, true] {
        let mut rig = if configured { limited() } else { Rig::new() };
        let _ticket = rig.submit(1); rig.start(1, 101);
        let mut request = request(&rig);
        if configured { request.expected_control_sequence += 1; }
        let before = rig.driver.supervisor().host().unwrap().inspect();
        assert_eq!(rig.driver.stop_with_recovery_reserve(request, || panic!("no admitted stop")),
            Err(JournalError::Contract(if configured { Error::Stale } else { Error::Incomplete })));
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
        assert_eq!(rig.driver.phase(), FileDriverPhase::Reviewing { request: 1 });
        assert!(matches!(rig.review_event(Verdict::Allow, false), FileDriverEvent::ReviewApplied { .. }));
        let human = rig.human(1001, 31);
        assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
        assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
        assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
    }
}

#[test]
fn failed_stop_storage_returns_no_receipt_and_recovery_keeps_the_original_request() {
    let mut rig = limited(); let _ticket = rig.submit(1); rig.reviewed(1);
    fill(&mut rig);
    let original = request(&rig);
    let before = rig.driver.supervisor().host().unwrap().inspect();
    std::fs::write(rig.root.store().join("delivery.pending"), b"occupied").unwrap();
    assert!(matches!(rig.driver.stop_with_recovery_reserve(original, || panic!("unacknowledged stop")), Err(JournalError::Io(_))));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert!(rig.driver.supervisor().host().unwrap().journal_capacity().is_err());
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    let release = rig.driver.release(); drop(release);
    let mut p = profile(); p.delivery.limits.events = 32;
    let (host, _) = FileOversight::open(rig.root.store(), p).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.retained_requests(), 1);
    let (_, mut driver) = host.into_supervised_driver();
    let control = driver.supervisor().host().unwrap().inspect().control;
    let fresh = StopRequest { operation: 18, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch };
    let report = driver.stop_with_recovery_reserve(fresh, || ElapsedTick(2)).unwrap();
    assert!(matches!(report.drain, Ok(CapacityDrain::Advanced(ref swept)) if swept.progress.drained()));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}
