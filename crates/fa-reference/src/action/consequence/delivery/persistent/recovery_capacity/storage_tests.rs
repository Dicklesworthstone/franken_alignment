//! Existing storage barriers exercise actual canonical cut visibility.
use super::*;
use crate::action::consequence::delivery::persistent::RecoveryReserve;

fn capped(root: &Directory, executed: bool) -> (FileDelivery, FileDeliveryProfile) {
    let mut p = profile(); p.limits.events = 12;
    let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    dispatch(&mut host, 1, b"visible");
    if executed { host.publish(host.revision(), 1).unwrap(); }
    while host.revision() < 9 { host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); }
    (host, p)
}
fn stop_input(host: &FileDelivery, operation: u64) -> StopRequest {
    let current = host.inspect().control;
    StopRequest { operation, expected_control_sequence: current.sequence, expected_authority_epoch: current.ledger.epoch }
}

#[test]
fn failed_stop_at_work_capacity_preserves_enough_tail_for_exclusive_recovery() {
    for stage in BARRIERS {
        for executed in [false, true] {
            let root = Directory::new(); let (mut host, p) = capped(&root, executed);
            let before = host.inspect();
            let request = stop_input(&host, 1);
            host.store.fail_once(stage);
            let error = host.request_stop(host.revision(), request).unwrap_err();
            assert!(matches!(error, JournalError::Io(ref failure) if failure.operation == stage));
            assert_eq!(host.inspect(), before);
            assert_eq!(host.journal_capacity(), Err(JournalError::Unavailable));
            let disk = FileDelivery::read_publication(root.store(), &p).unwrap();
            assert_eq!(disk.stop.is_some(), stage == JournalIo::DirectorySync);
            assert_eq!(disk.control.ledger.charged, 16);
            drop(host);
            let mut host = FileDelivery::open(root.store(), p).unwrap();
            if host.stop_receipt().is_none() {
                host.request_stop(host.revision(), stop_input(&host, 2)).unwrap();
            }
            let drained = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
            assert!(drained.progress.drained());
            assert_eq!(host.revision(), 12);
            assert_eq!(host.inspect().executions, u64::from(executed));
            assert_eq!(host.inspect().control.ledger.charged, if executed { 16 } else { 0 });
            assert_eq!(host.inspect().control.ledger.available, if executed { 84 } else { 100 });
        }
    }
}

#[test]
fn failed_drain_recovery_uses_real_outcomes_without_repeating_a_completed_sweep() {
    for stage in BARRIERS {
        for executed in [false, true] {
            let root = Directory::new(); let (mut host, p) = capped(&root, executed);
            host.request_stop(host.revision(), stop_input(&host, 1)).unwrap();
            let before = host.inspect();
            host.store.fail_once(stage);
            assert!(matches!(host.progress_stop(host.revision(), ElapsedTick(2)), Err(JournalError::Io(_))));
            assert_eq!(host.inspect(), before);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            drop(host);
            let mut host = FileDelivery::open(root.store(), p).unwrap();
            if !host.stop_progress().unwrap().drained() {
                assert!(host.progress_stop(host.revision(), ElapsedTick(3)).unwrap().progress.drained());
            }
            assert_eq!(host.revision(), 12);
            assert_eq!(host.inspect().executions, u64::from(executed));
            assert_eq!(host.inspect().control.ledger.charged, if executed { 16 } else { 0 });
            assert!(host.stop_progress().unwrap().drained());
        }
    }
}

#[test]
fn importing_a_well_framed_history_cannot_spend_the_reserved_tail_on_a_clock() {
    let mut p = profile(); p.limits.events = 8;
    let path = Path::new("/operator-owned/capacity");
    let mut events = vec![Event::ReserveRecovery(RecoveryReserve::terminal())];
    events.extend((0..4).map(|_| Event::Time(ElapsedTick(1))));
    let mut encoded = codec::encode(&p, path, &events).unwrap();
    // Locate the event count after the ORIGINAL domain/path/bootstrap. Construct
    // a structurally valid sixth event without calling the admission encoder.
    let mut offset = 8;
    for _ in 0..2 {
        let length = u32::from_be_bytes(encoded[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4 + length;
    }
    encoded[offset..offset + 4].copy_from_slice(&6_u32.to_be_bytes());
    encoded.extend_from_slice(&9_u32.to_be_bytes());
    encoded.push(0);
    encoded.extend_from_slice(&1_u64.to_be_bytes());
    assert!(matches!(codec::decode(&p, path, &encoded), Err(Error::Limit)));
    events.push(Event::Fence);
    let valid = codec::encode(&p, path, &events).unwrap();
    assert!(codec::decode(&p, path, &valid).is_ok());
    let recovered = Machine::replay(&p, &events).unwrap();
    assert_eq!(recovered.snapshot(events.len()).control.ledger.available, 100);
}
