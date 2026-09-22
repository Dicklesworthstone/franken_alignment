//! Exercise discovery through a real durable owner, not a parallel queue model.

use super::*;

fn requests(rows: &[FileRequestStatus]) -> Vec<u64> {
    rows.iter().map(|row| row.request).collect()
}

#[test]
fn stable_pages_cover_nonmonotonic_ids_and_maximum_cursor_without_mutation() {
    let root = Directory::new();
    let p = profile();
    let (mut host, _, _) = prepared(&root, &p);
    authorize(&mut host, 9);
    authorize(&mut host, u64::MAX);
    let revision = host.revision();
    let before = host.inspect();
    let first = host.request_status_page(revision, None, 1).unwrap();
    assert_eq!(requests(&first), vec![9]);
    let second = host.request_status_page(revision, Some(first[0].request), 1).unwrap();
    assert_eq!(requests(&second), vec![700]);
    let third = host.request_status_page(revision, Some(second[0].request), 1).unwrap();
    assert_eq!(requests(&third), vec![u64::MAX]);
    assert!(host.request_status_page(revision, Some(u64::MAX), 1).unwrap().is_empty());
    assert_eq!(requests(&host.request_status_page(revision, Some(10), 2).unwrap()), vec![700, u64::MAX]);
    assert_eq!(requests(&host.pending_request_statuses(revision).unwrap()), vec![9, 700, u64::MAX]);
    for row in [first[0], second[0], third[0]] {
        assert_eq!(host.request_status(row.request), Ok(row));
    }
    assert_eq!(host.inspect(), before);
    assert_eq!(FileDelivery::read_publication(root.store(), &p).unwrap(), before);
}

#[test]
fn an_intervening_transition_invalidates_the_entire_page_cut() {
    let root = Directory::new();
    let (mut host, _, _) = prepared(&root, &profile());
    let revision = host.revision();
    assert_eq!(requests(&host.request_status_page(revision, None, 1).unwrap()), vec![700]);
    // A smaller ID inserted between pages must not disappear from a supposedly
    // complete scan whose exclusive cursor was already at 700.
    authorize(&mut host, 9);
    assert_eq!(host.request_status_page(revision, Some(700), 1),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.pending_request_statuses(revision),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(requests(&host.request_status_page(host.revision(), None, 2).unwrap()), vec![9, 700]);
}

#[test]
fn rejected_admissions_are_discoverable_but_never_pending() {
    let root = Directory::new();
    let (mut host, _, action) = prepared(&root, &profile());
    let mut spec = action.spec().clone();
    spec.required_witnesses.clear();
    let refused = host.submit_request(host.revision(), 44, spec, Snapshot::default()).unwrap();
    assert!(matches!(refused.disposition, FileRequestDisposition::NotAdmitted(_)));
    let rows = host.request_status_page(host.revision(), None, 8).unwrap();
    assert_eq!(requests(&rows), vec![44, 700]);
    assert_eq!(rows[0], refused);
    assert_eq!(requests(&host.pending_request_statuses(host.revision()).unwrap()), vec![700]);
}

#[test]
fn restart_discovers_lost_ack_and_unknown_work_without_a_sidecar_id_list() {
    let root = Directory::new();
    let p = profile();
    let (mut host, executed, action) = prepared(&root, &p);
    let (unknown, unknown_action) = authorize(&mut host, 99);
    authorize(&mut host, 9);
    host.dispatch(host.revision(), &executed, &action, snapshot()).unwrap();
    host.dispatch(host.revision(), &unknown, &unknown_action, snapshot()).unwrap();
    host.publish(host.revision(), executed.attempt()).unwrap();
    let old_revision = host.revision();
    drop(host);
    // No in-memory queue or FilePermit is consulted when finding obligations.
    let (mut recovered, _) = FileDelivery::open_reconciled(root.store(), p, ElapsedTick(2)).unwrap();
    assert_eq!(recovered.request_status_page(old_revision, None, 8),
        Err(JournalError::Contract(Error::Stale)));
    let rows = recovered.request_status_page(recovered.revision(), None, 8).unwrap();
    assert_eq!(requests(&rows), vec![9, 99, 700]);
    assert_eq!(stage(rows[0]), ActionState::Cancelled);
    assert!(matches!(stage(rows[1]), ActionState::Dispatching | ActionState::Unknown));
    assert_eq!(stage(rows[2]), ActionState::Confirmed);
    let pending = recovered.pending_request_statuses(recovered.revision()).unwrap();
    assert_eq!(requests(&pending), vec![99]);
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().control.ledger.charged, 32);
    // Drive the new targeted settlement using ONLY the recovered request ID.
    recovered.cancel_and_resolve_request(recovered.revision(), pending[0].request, ElapsedTick(3)).unwrap();
    assert!(recovered.pending_request_statuses(recovered.revision()).unwrap().is_empty());
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
}

#[test]
fn bounds_capacity_and_faults_do_not_turn_discovery_into_a_write() {
    let root = Directory::new();
    let mut p = profile();
    p.limits.events = 4;
    let (host, _, _) = prepared(&root, &p);
    assert_eq!(host.revision(), 4);
    assert_eq!(requests(&host.pending_request_statuses(host.revision()).unwrap()), vec![700]);
    for limit in [0, 129, usize::MAX] {
        assert_eq!(host.request_status_page(host.revision(), None, limit),
            Err(JournalError::Contract(Error::Limit)));
    }
    assert_eq!(host.revision(), 4);
    drop(host);

    let root = Directory::new();
    let (mut host, _, _) = prepared(&root, &profile());
    host.store.fail_once(JournalIo::Stage);
    let revision = host.revision();
    assert_eq!(requests(&host.request_status_page(revision, None, 1).unwrap()), vec![700]);
    assert_eq!(requests(&host.pending_request_statuses(revision).unwrap()), vec![700]);
    // Read-only discovery did not consume the pending storage failure.
    assert!(matches!(host.cancel_and_resolve_request(revision, 700, ElapsedTick(2)),
        Err(JournalError::Io(_))));
    assert_eq!(host.request_status_page(revision, None, 1), Err(JournalError::Unavailable));
    assert_eq!(host.pending_request_statuses(revision), Err(JournalError::Unavailable));
}

#[test]
fn expired_unknown_liabilities_remain_in_the_recovered_inventory() {
    let root = Directory::new();
    let p = profile();
    let (mut host, key, action) = prepared(&root, &p);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    drop(host);
    let (recovered, _) = FileDelivery::open_reconciled(root.store(), p, ElapsedTick(1002)).unwrap();
    let pending = recovered.pending_request_statuses(recovered.revision()).unwrap();
    assert_eq!(requests(&pending), vec![700]);
    assert!(matches!(stage(pending[0]), ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown));
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    assert_eq!(recovered.inspect().executions, 0);
}
