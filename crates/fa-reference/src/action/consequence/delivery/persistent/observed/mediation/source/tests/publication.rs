//! Explicit final source acquisition followed by the unchanged native endpoint.
use super::*;
use crate::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use std::cell::Cell;

fn dispatched(root: &Directory, document: &str) -> (FileOversight, FileTopologySource, Ready) {
    let (mut host, human, mut source) = fixture(root, document);
    certified(&refresh(&mut source, &mut host, 1));
    let ready = prepare(&mut host, &human, 1);
    dispatch(&mut host, &ready);
    (host, source, ready)
}
fn request<'a>(host: &FileOversight, current: Option<&'a CommitteeInput>, operation: u64)
    -> TopologyPublication<'a>
{
    TopologyPublication { revision: host.revision(), operation, attempt: 1, current, snapshot: snapshot() }
}

#[test]
fn original_publication_waits_for_fresh_topology_and_a_post_capture_clock() {
    let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
    let before = root.bytes(); let old_cut = host.revision(); let calls = Cell::new(0);
    let input = request(&host, Some(&ready.inputs), 2);
    let report = host.publish_checked_from_topology_source(&mut source, input, || {
        calls.set(calls.get() + 1);
        // The original native check must already be committed before this clock.
        assert_ne!(root.bytes(), before);
        let checked = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(checked.revision, old_cut + 1); assert_eq!(checked.executions, 0);
        ElapsedTick(2)
    }).unwrap();
    assert_eq!(calls.get(), 1);
    let check = report.topology.as_ref().unwrap().as_ref().unwrap(); certified(check);
    assert_eq!(check.journal_revision, old_cut + 1);
    assert_eq!(check.work.checks, 2);
    let publication = report.publication.unwrap().unwrap();
    assert_eq!(publication.basis, PublicationBasis::Revalidated);
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.revision(), old_cut + 2); assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(publication.outcome));
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn missing_final_topology_is_sealed_by_original_endpoint_and_never_rerolled() {
    let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
    fs::remove_file(root.0.join("inventory.json")).unwrap();
    let input = request(&host, Some(&ready.inputs), 2);
    let report = host.publish_checked_from_topology_source(&mut source, input, || ElapsedTick(2)).unwrap();
    let check = report.topology.unwrap().unwrap();
    assert!(matches!(check.refusal, Some(TopologySourceRefusal::Input(_))));
    assert!(check.withdrawal.is_some());
    let publication = report.publication.unwrap().unwrap();
    assert!(matches!(publication.basis, PublicationBasis::Rejected(_)));
    assert!(matches!(publication.outcome, EndpointOutcome::NotExecuted { .. }));
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
    fs::write(root.0.join("inventory.json"), INVENTORY).unwrap();
    let work = source.work(); let input = request(&host, Some(&ready.inputs), 2);
    let retry = host.publish_checked_from_topology_source(&mut source, input, || ElapsedTick(3)).unwrap();
    assert!(retry.topology.is_none()); assert_eq!(source.work(), work);
    let previous = retry.publication.unwrap().unwrap();
    assert_eq!(previous.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(previous.outcome, publication.outcome); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(previous.outcome));
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn valid_topology_does_not_replace_missing_whole_input_or_policy_evidence() {
    for missing_input in [false, true] {
        let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
        let current = if missing_input { None } else { Some(&ready.inputs) };
        let mut input = request(&host, current, 2);
        if !missing_input { input.snapshot.complete = false; }
        let report = host.publish_checked_from_topology_source(&mut source, input, || ElapsedTick(2)).unwrap();
        certified(report.topology.as_ref().unwrap().as_ref().unwrap());
        let publication = report.publication.unwrap().unwrap();
        assert!(matches!(publication.basis, PublicationBasis::Rejected(_)));
        assert!(matches!(publication.outcome, EndpointOutcome::NotExecuted { .. }));
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(publication.outcome));
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn acknowledged_execution_needs_no_new_source_allowance_files_or_source_operation() {
    let root = Directory::new();
    let doc = document(1, 2, 2 * TOPOLOGY_REFRESH_READ_BYTES, 0,
        crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS);
    let (mut host, mut source, ready) = dispatched(&root, &doc);
    let input = request(&host, Some(&ready.inputs), 2);
    let report = host.publish_checked_from_topology_source(&mut source, input, || ElapsedTick(2)).unwrap();
    assert_eq!(report.publication.unwrap().unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(source.work().checks, source.limits().checks);
    for name in ["topology.json", "inventory.json"] { fs::remove_file(root.0.join(name)).unwrap(); }
    let work = source.work(); let input = request(&host, None, 0);
    let report = host.publish_checked_from_topology_source(&mut source, input, || ElapsedTick(3)).unwrap();
    assert!(report.topology.is_none()); assert_eq!(source.work(), work);
    let publication = report.publication.unwrap().unwrap();
    assert_eq!(publication.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(publication.outcome));
}

#[test]
fn original_deadline_wins_before_capture_and_is_rechecked_after_capture() {
    for previously_expired in [false, true] {
        let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
        if previously_expired {
            host.observe_time(host.revision(), ElapsedTick(100)).unwrap();
            fs::remove_file(root.0.join("topology.json")).unwrap();
        }
        let work = source.work(); let calls = Cell::new(0);
        let input = request(&host, Some(&ready.inputs), if previously_expired { 0 } else { 2 });
        let report = host.publish_checked_from_topology_source(&mut source, input, || {
            calls.set(calls.get() + 1); ElapsedTick(100)
        }).unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(report.topology.is_none(), previously_expired);
        assert_eq!(source.work().checks, work.checks + u64::from(!previously_expired));
        let publication = report.publication.unwrap().unwrap();
        assert_eq!(publication.basis, PublicationBasis::DeadlineElapsed);
        assert!(matches!(publication.outcome, EndpointOutcome::NotExecuted { .. }));
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
    }
}

#[test]
fn wrong_owner_predecessor_attempt_or_operation_cannot_trigger_source_or_clock_work() {
    let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
    let other = Directory::new(); let (_other_host, _, mut foreign) = fixture(&other, &text(1));
    let before = root.bytes(); let work = source.work();
    let input = request(&host, Some(&ready.inputs), 2);
    assert!(host.publish_checked_from_topology_source(&mut foreign, input,
        || panic!("foreign source reached clock")).is_err());
    for missing in [false, true] {
        let mut input = request(&host, None, 2);
        if missing { input.attempt = 99; } else { input.revision -= 1; }
        assert!(host.publish_checked_from_topology_source(&mut source, input,
            || panic!("bad predecessor/attempt reached clock")).is_err());
    }
    let input = request(&host, Some(&ready.inputs), 1);
    let stale = host.publish_checked_from_topology_source(&mut source, input,
        || panic!("old source operation reached clock")).unwrap();
    assert_eq!(stale.topology, Some(Err(JournalError::Contract(Error::Stale))));
    assert!(stale.publication.is_none()); assert_eq!(source.work(), work); assert_eq!(root.bytes(), before);
    let input = request(&host, Some(&ready.inputs), 2);
    let corrected = host.publish_checked_from_topology_source(&mut source, input, || ElapsedTick(2)).unwrap();
    assert_eq!(corrected.publication.unwrap().unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn withdrawal_failure_never_acquires_publication_time_or_attempts_the_sink() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
        fs::remove_file(root.0.join("inventory.json")).unwrap(); host.store.fail_once(barrier);
        let input = request(&host, Some(&ready.inputs), 2);
        let report = host.publish_checked_from_topology_source(&mut source, input,
            || panic!("failed withdrawal reached publication time")).unwrap();
        let JournalError::Io(failure) = report.topology.unwrap().unwrap_err() else { panic!("Store error"); };
        assert_eq!(failure.operation, barrier); assert!(report.publication.is_none());
        assert!(matches!(source.last_refusal(), Some(TopologySourceRefusal::Input(_))));
        assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().executions, 0);
    }
}

#[test]
fn later_publication_failure_retains_acknowledged_source_check_and_original_visibility() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
        let old_cut = host.revision(); let input = request(&host, Some(&ready.inputs), 2);
        let report = host.publish_checked_from_topology_source_with(&mut source, input, |host| {
            host.store.fail_once(barrier); ElapsedTick(2)
        }).unwrap();
        let check = report.topology.unwrap().unwrap(); certified(&check);
        assert_eq!(check.journal_revision, old_cut + 1);
        let JournalError::Io(failure) = report.publication.unwrap().unwrap_err() else { panic!("publication Store error"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.revision(), old_cut + 1); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.executions, u64::from(barrier == JournalIo::DirectorySync));
        assert_eq!(source.work().checks, 2); assert!(source.last_refusal().is_none());
    }
}

#[test]
fn a_post_withdrawal_clock_unwind_cannot_revive_the_old_cut_or_send_an_effect() {
    let root = Directory::new(); let (mut host, mut source, ready) = dispatched(&root, &text(1));
    fs::remove_file(root.0.join("topology.json")).unwrap();
    let old_cut = host.revision(); let input = request(&host, Some(&ready.inputs), 2);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        host.publish_checked_from_topology_source(&mut source, input, || panic!("clock unavailable"))
    })).is_err());
    assert_eq!(host.revision(), old_cut + 1); assert!(!host.mediation_snapshot().unwrap().available);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(source.work().checks, 2); assert!(!canonical(&host).available);
    let input = request(&host, Some(&ready.inputs), 3);
    let report = host.publish_checked_from_topology_source(&mut source, input, || ElapsedTick(2)).unwrap();
    assert_eq!(report.topology.unwrap().unwrap().refusal, Some(TopologySourceRefusal::CoverageUnavailable));
    assert!(matches!(report.publication.unwrap().unwrap().outcome, EndpointOutcome::NotExecuted { .. }));
    assert_eq!(source.work().checks, 2); assert_eq!(host.inspect().executions, 0);
}
