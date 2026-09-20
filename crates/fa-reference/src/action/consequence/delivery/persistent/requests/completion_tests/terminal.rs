//! Reuse the existing publication fixtures; exercise the real original reducers.
use super::*;
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::RecoveryReserve;

fn stop_request(host: &FileDelivery, operation: u64) -> StopRequest {
    let control = host.inspect().control;
    StopRequest {
        operation,
        expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch,
    }
}

#[test]
fn atomic_stop_cancels_undispatched_reservations_without_execution() {
    let root = Directory::new();
    let p = profile();
    let (mut host, key, action) = prepared(&root, &p);
    let revision = host.revision();
    let request = stop_request(&host, 900);
    let sweep = host.stop_and_drain(revision, request, ElapsedTick(2)).unwrap();
    assert_eq!(host.revision(), revision + 2);
    assert!(sweep.outcomes.is_empty());
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.receipt.request(), request);
    assert_eq!(sweep.progress.receipt.cancelled(), &[key.attempt()]);
    assert_eq!(sweep.progress.receipt.refunded_units(), 16);
    let after = host.inspect();
    assert_eq!(after.control.ledger.available, 100);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.charged, 0);
    assert_eq!(after.control.ledger.stages[&key.attempt()], ActionState::Cancelled);
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, b"initial");
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), after);
    assert_eq!(host.dispatch(host.revision(), &key, &action, snapshot()),
        Err(JournalError::Contract(Error::Missing)));
    assert_eq!(host.inspect(), after);
}

#[test]
fn atomic_stop_seals_sent_but_unpublished_work_without_resending() {
    let root = Directory::new();
    let p = profile();
    let (mut host, key, action) = prepared(&root, &p);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let request = stop_request(&host, 901);
    let sweep = host.stop_and_drain(host.revision(), request, ElapsedTick(2)).unwrap();
    let outcome = EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed };
    assert_eq!(sweep.outcomes[&key.attempt()], Ok(Reconciliation::Resolved(outcome)));
    assert_eq!(host.request_resolution(700), Ok(Some(outcome)));
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().payload, b"initial");
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), host.inspect());
}

#[test]
fn atomic_stop_accepts_a_lost_ack_but_never_refunds_executed_work() {
    let root = Directory::new();
    let p = profile();
    let (mut host, key, action) = prepared(&root, &p);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    let outcome = EndpointOutcome::Executed { resulting_version: 2 };
    assert_eq!(host.publish(host.revision(), key.attempt()), Ok(outcome));
    let request = stop_request(&host, 902);
    let sweep = host.stop_and_drain(host.revision(), request, ElapsedTick(2)).unwrap();
    assert_eq!(sweep.outcomes[&key.attempt()], Ok(Reconciliation::Resolved(outcome)));
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.request_resolution(700), Ok(Some(outcome)));
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), host.inspect());
    let again = host.stop_and_drain(host.revision(), request, ElapsedTick(3)).unwrap();
    assert!(again.outcomes.is_empty());
    assert_eq!(again.progress.receipt, sweep.progress.receipt);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn terminal_validation_never_partially_acknowledges_a_stop() {
    let root = Directory::new();
    let p = profile();
    let (mut host, _key, _action) = prepared(&root, &p);
    let before = host.inspect();
    let request = stop_request(&host, 903);
    assert_eq!(host.stop_and_drain(host.revision() - 1, request, ElapsedTick(2)),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.stop_and_drain(host.revision(), request, ElapsedTick(0)),
        Err(JournalError::Contract(Error::Stale)));
    let mut stale = request;
    stale.expected_authority_epoch += 1;
    assert_eq!(host.stop_and_drain(host.revision(), stale, ElapsedTick(2)),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before);
    assert!(host.stop_receipt().is_none());
    assert!(host.storage_failure().is_none());
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
}

#[test]
fn terminal_stop_spends_only_recovery_capacity() {
    let root = Directory::new();
    let mut p = profile();
    p.limits.events = 8;
    let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (_key, _action) = authorize_request(&mut host, 700);
    assert_eq!(host.revision(), 5);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(2)),
        Err(JournalError::Contract(Error::Limit)));
    let request = stop_request(&host, 904);
    let sweep = host.stop_and_drain(host.revision(), request, ElapsedTick(2)).unwrap();
    assert!(sweep.progress.drained());
    assert_eq!(host.revision(), 7);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), host.inspect());
}

#[test]
fn terminal_retention_expiry_is_unresolved_and_still_charged() {
    let root = Directory::new();
    let p = profile();
    let (mut host, key, action) = prepared(&root, &p);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    let request = stop_request(&host, 905);
    let sweep = host.stop_and_drain(host.revision(), request, ElapsedTick(2000)).unwrap();
    assert_eq!(sweep.outcomes[&key.attempt()], Ok(Reconciliation::RetentionExpired));
    assert!(!sweep.progress.drained());
    assert!(sweep.progress.unresolved.contains(&key.attempt()));
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.request_resolution(700), Ok(None));
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn terminal_storage_barriers_expose_only_the_old_or_complete_stop() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync]
    {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
        let before = host.inspect();
        let request = stop_request(&host, 906);
        host.store.fail_once(barrier);
        assert!(matches!(host.stop_and_drain(host.revision(), request, ElapsedTick(2)),
            Err(JournalError::Io(_))));
        assert_eq!(host.inspect(), before);
        assert!(!host.clock_ready());
        assert_eq!(host.stop_and_drain(host.revision(), request, ElapsedTick(2)),
            Err(JournalError::Unavailable));
        let disk = FileDelivery::read_publication(root.store(), &p).unwrap();
        if barrier == JournalIo::DirectorySync {
            assert_eq!(disk.revision, before.revision + 2);
            assert!(disk.stop.is_some());
            assert_eq!(disk.control.ledger.charged, 0);
            assert_eq!(disk.control.ledger.available, 100);
            assert_eq!(disk.control.ledger.stages[&key.attempt()], ActionState::ConfirmedNotExecuted);
        } else {
            assert_eq!(disk, before);
        }
        assert_eq!(disk.executions, 0);
    }
}

#[test]
fn terminal_reopen_uses_the_exact_reserve_when_ordinary_recovery_cannot_fit() {
    let root = Directory::new();
    let mut p = profile();
    p.limits.events = 9;
    let mut host = FileDelivery::create(root.store(), p.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (key, action) = authorize_request(&mut host, 700);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    let before = host.inspect();
    assert_eq!(before.revision, 6);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let request = stop_request(&host, 1000);
    drop(host);
    assert!(matches!(FileDelivery::open_reconciled(root.store(), p.clone(), ElapsedTick(2)),
        Err(JournalError::Contract(Error::Limit))));
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
    let (mut recovered, sweep) =
        FileDelivery::open_stopped(root.store(), p.clone(), request, ElapsedTick(2)).unwrap();
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.receipt.request(), request);
    assert_eq!(recovered.revision(), 9);
    assert!(recovered.clock_ready());
    assert_eq!(recovered.journal_capacity().unwrap().remaining().events, 0);
    assert_eq!(recovered.inspect().control.ledger.available, 100);
    assert_eq!(recovered.inspect().control.ledger.charged, 0);
    assert_eq!(recovered.inspect().executions, 0);
    assert_eq!(recovered.request_resolution(700),
        Ok(Some(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed })));
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), recovered.inspect());
    assert_eq!(recovered.dispatch(recovered.revision(), &key, &action, snapshot()),
        Err(JournalError::Contract(Error::Binding)));
}

#[test]
fn terminal_reopen_preserves_executed_charges_and_refuses_new_intake() {
    let root = Directory::new();
    let p = profile();
    let (mut host, key, action) = prepared(&root, &p);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    let outcome = EndpointOutcome::Executed { resulting_version: 2 };
    assert_eq!(host.publish(host.revision(), key.attempt()), Ok(outcome));
    let before = host.inspect();
    let request = stop_request(&host, 1001);
    drop(host);
    let (mut recovered, sweep) =
        FileDelivery::open_stopped(root.store(), p.clone(), request, ElapsedTick(2)).unwrap();
    assert_eq!(sweep.outcomes[&key.attempt()], Ok(Reconciliation::Resolved(outcome)));
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(recovered.revision(), before.revision + 3);
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().control.ledger.available, 84);
    let stopped = recovered.inspect();
    assert_eq!(recovered.submit_request(recovered.revision(), 701, action.spec().clone(), snapshot()),
        Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(recovered.inspect(), stopped);
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), stopped);
    // A subsequent exclusive terminal recovery retains the original stop but
    // installs another dispatcher generation rather than reusing the old one.
    drop(recovered);
    let (again, sweep_again) =
        FileDelivery::open_stopped(root.store(), p, request, ElapsedTick(3)).unwrap();
    assert!(sweep_again.outcomes.is_empty());
    assert_eq!(sweep_again.progress.receipt, sweep.progress.receipt);
    assert!(again.inspect().dispatcher_epoch > stopped.dispatcher_epoch);
    assert_eq!(again.inspect().executions, 1);
    assert_eq!(again.inspect().control.ledger.charged, 16);
}

#[test]
fn terminal_reopen_rejects_stale_time_and_request_without_a_partial_fence() {
    let root = Directory::new();
    let p = profile();
    let (host, _key, _action) = prepared(&root, &p);
    let before = host.inspect();
    let request = stop_request(&host, 1002);
    drop(host);
    assert!(matches!(FileDelivery::open_stopped(root.store(), p.clone(), request, ElapsedTick(0)),
        Err(JournalError::Contract(Error::Stale))));
    let mut stale = request;
    stale.expected_control_sequence += 1;
    assert!(matches!(FileDelivery::open_stopped(root.store(), p.clone(), stale, ElapsedTick(2)),
        Err(JournalError::Contract(Error::Stale))));
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
    let (recovered, sweep) =
        FileDelivery::open_stopped(root.store(), p, request, ElapsedTick(2)).unwrap();
    assert!(sweep.progress.drained());
    assert_eq!(recovered.inspect().control.ledger.available, 100);
    assert_eq!(recovered.inspect().executions, 0);
}

#[test]
fn terminal_reopen_preflights_all_three_records() {
    for limit in [6, 7] {
        let root = Directory::new();
        let mut p = profile();
        p.limits.events = limit;
        let (host, _key, _action) = prepared(&root, &p);
        let before = host.inspect();
        assert_eq!(before.revision, 4);
        let request = stop_request(&host, 1003);
        drop(host);
        let result = FileDelivery::open_stopped(root.store(), p.clone(), request, ElapsedTick(2));
        if limit == 6 {
            assert!(matches!(result, Err(JournalError::Contract(Error::Limit))));
            assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
        } else {
            let (recovered, sweep) = result.unwrap();
            assert_eq!(recovered.revision(), 7);
            assert!(sweep.progress.drained());
            assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), recovered.inspect());
        }
    }
}

#[test]
fn terminal_reopen_retention_expiry_never_invents_a_refund() {
    let root = Directory::new();
    let p = profile();
    let (mut host, key, action) = prepared(&root, &p);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    let request = stop_request(&host, 1004);
    drop(host);
    let (recovered, sweep) =
        FileDelivery::open_stopped(root.store(), p.clone(), request, ElapsedTick(2000)).unwrap();
    assert_eq!(sweep.outcomes[&key.attempt()], Ok(Reconciliation::RetentionExpired));
    assert!(!sweep.progress.drained());
    assert!(sweep.progress.unresolved.contains(&key.attempt()));
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(recovered.inspect().control.ledger.available, 84);
    assert_eq!(recovered.inspect().executions, 0);
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), recovered.inspect());
}

#[test]
fn terminal_reopen_recovers_either_cut_of_an_ambiguous_stop() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync]
    {
        let root = Directory::new();
        let p = profile();
        let (mut host, key, action) = prepared(&root, &p);
        host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
        let request = stop_request(&host, 1005);
        host.store.fail_once(barrier);
        assert!(matches!(host.stop_and_drain(host.revision(), request, ElapsedTick(2)),
            Err(JournalError::Io(_))));
        drop(host);
        let (mut recovered, sweep) =
            FileDelivery::open_stopped(root.store(), p.clone(), request, ElapsedTick(3)).unwrap();
        assert!(sweep.progress.drained());
        assert_eq!(sweep.progress.receipt.request(), request);
        assert_eq!(recovered.request_resolution(700),
            Ok(Some(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed })));
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.inspect().control.ledger.charged, 0);
        assert_eq!(recovered.inspect().executions, 0);
        assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), recovered.inspect());
        assert_eq!(recovered.dispatch(recovered.revision(), &key, &action, snapshot()),
            Err(JournalError::Contract(Error::Binding)));
    }
}

#[test]
fn terminal_reopen_requires_exclusive_ownership_and_the_same_stop_identity() {
    let root = Directory::new();
    let p = profile();
    let (mut host, _key, _action) = prepared(&root, &p);
    let request = stop_request(&host, 1006);
    assert!(matches!(FileDelivery::open_stopped(root.store(), p.clone(), request, ElapsedTick(2)),
        Err(JournalError::Busy)));
    host.stop_and_drain(host.revision(), request, ElapsedTick(2)).unwrap();
    let before = host.inspect();
    drop(host);
    let mut changed = request;
    changed.expected_authority_epoch += 1;
    assert!(matches!(FileDelivery::open_stopped(root.store(), p.clone(), changed, ElapsedTick(3)),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
}
