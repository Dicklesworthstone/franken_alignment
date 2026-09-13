//! Logical recovery headroom with original authority transitions and real files.
#![cfg(unix)]
#[path = "support/file_delivery.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, StopRequest};
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::Error;

fn stop(host: &FileDelivery) -> StopRequest {
    let state = host.inspect().control;
    StopRequest { operation: 1, expected_control_sequence: state.sequence,
        expected_authority_epoch: state.ledger.epoch }
}
fn fill(host: &mut FileDelivery) {
    loop {
        let before = host.inspect();
        match host.observe_time(host.revision(), ElapsedTick(1)) {
            Ok(()) => {}
            Err(JournalError::Contract(Error::Limit)) => {
                assert_eq!(host.inspect(), before);
                assert!(host.storage_failure().is_none());
                break;
            }
            Err(error) => panic!("unexpected fill failure: {error}"),
        }
    }
}

#[test]
fn exhausted_work_budget_still_reopens_stops_and_settles_original_mixed_effects() {
    let root = Directory::new();
    let mut p = profile(); p.limits.events = 16;
    let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    dispatched(&mut host, 1, b"executed");
    host.publish(host.revision(), 1).unwrap();
    dispatched(&mut host, 2, b"not delivered");
    fill(&mut host);
    assert_eq!(host.revision(), 13);
    let space = host.journal_capacity().unwrap();
    assert_eq!(space.ordinary_remaining().events, 0);
    assert_eq!(space.remaining().events, 3);
    assert!(space.terminal_space_remaining());
    assert_eq!(host.inspect().control.ledger.charged, 32);
    drop(host);
    let mut host = FileDelivery::open(root.store(), p.clone()).unwrap();
    assert_eq!(host.revision(), 14);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Unknown);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    host.request_stop(host.revision(), stop(&host)).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 32);
    let result = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert!(result.progress.drained());
    assert_eq!(result.outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert_eq!(result.outcomes[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed })));
    assert_eq!(host.revision(), 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"executed");
    assert_eq!(host.journal_capacity().unwrap().remaining().events, 0);
    let before = host.inspect();
    assert_eq!(host.fence(host.revision()), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before);
    // Logical reserve is finite, not an infinite reopening promise. Pure reads
    // still reconstruct the final canonical cut without minting another owner.
    drop(host);
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
}

#[test]
fn encoded_byte_headroom_binds_independently_of_the_number_of_events() {
    let root = Directory::new();
    let mut p = profile(); p.limits.events = 64; p.limits.bytes = 8192;
    let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
    let initial = host.journal_capacity().unwrap().used().bytes;
    // Reserve marker: tag + u32 count + u64 bytes + u32 record frame = 17.
    // Leave exactly two ordinary 13-byte clock events after the marker.
    let reserve = RecoveryReserve { events: 3, bytes: p.limits.bytes - initial - 17 - 26 };
    host.enable_recovery_reserve(0, reserve).unwrap();
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().bytes, 26);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let space = host.journal_capacity().unwrap();
    assert_eq!(space.ordinary_remaining().bytes, 0);
    assert!(space.ordinary_remaining().events > 0);
    let before = host.inspect();
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(1)), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before);
    drop(host);
    let mut host = FileDelivery::open(root.store(), p).unwrap();
    host.request_stop(host.revision(), stop(&host)).unwrap();
    assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(reserve));
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().bytes, 0);
}

#[test]
fn malformed_duplicate_and_late_installations_change_neither_authority_nor_disk() {
    let root = Directory::new();
    let mut host = create(&root);
    let before = host.inspect();
    for reserve in [RecoveryReserve { events: 2, bytes: 50 }, RecoveryReserve { events: 3, bytes: 49 },
        RecoveryReserve { events: MAX_JOURNAL_EVENTS, bytes: 50 }, RecoveryReserve { events: 3, bytes: MAX_JOURNAL_BYTES }] {
        assert!(host.enable_recovery_reserve(host.revision(), reserve).is_err());
        assert_eq!(host.inspect(), before);
    }
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let configured = host.inspect();
    assert_eq!(host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.inspect(), configured);
    let root = Directory::new();
    let mut host = create(&root);
    let mut refused = spec(&host, b"wrong target"); refused.target.as_mut().unwrap().object += 1;
    host.submit_request(host.revision(), 10, refused, snapshot()).unwrap();
    assert!(host.inspect().control.ledger.stages.is_empty());
    let before = host.inspect();
    assert_eq!(host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.journal_capacity().unwrap().reserve(), None);
}

#[test]
fn reserve_does_not_convert_expired_outcome_retention_into_refundable_nonexecution() {
    let root = Directory::new();
    let mut p = profile(); p.limits.events = 12;
    let mut host = FileDelivery::create(root.store(), p).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    dispatched(&mut host, 1, b"unknown");
    fill(&mut host);
    host.request_stop(host.revision(), stop(&host)).unwrap();
    let result = host.progress_stop(host.revision(), ElapsedTick(2000)).unwrap();
    assert_eq!(result.outcomes[&1], Ok(Reconciliation::RetentionExpired));
    assert!(!result.progress.drained());
    assert_eq!(result.progress.unresolved, vec![1]);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn unconfigured_legacy_profile_keeps_its_original_full_capacity() {
    let root = Directory::new();
    let mut p = profile(); p.limits.events = 5;
    let mut host = FileDelivery::create(root.store(), p).unwrap();
    fill(&mut host);
    assert_eq!(host.revision(), 5);
    let space = host.journal_capacity().unwrap();
    assert_eq!(space.reserve(), None);
    assert_eq!(space.remaining(), space.ordinary_remaining());
    assert_eq!(space.remaining().events, 0);
}
