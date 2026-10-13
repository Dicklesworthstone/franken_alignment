//! Native two-store evidence and original terminal effects, not simulated stops.
use super::*;
use super::super::test_support::*;
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn coordinator(root: &Directory, p: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), p, MAX_JOURNAL_BYTES).unwrap()
}
fn ready(visit: &mut FileShutdownVisitRecovery<'_>, quantum: usize) {
    while visit.progress().status == FileStopRecoveryStatus::Replaying {
        let old = visit.progress();
        let next = visit.advance(old.replayed_events, quantum).unwrap();
        assert!(next.attempted_events - old.attempted_events <= quantum);
        assert_eq!(next.attempted_events, next.replayed_events);
    }
    assert_eq!(visit.progress().status, FileStopRecoveryStatus::Ready);
    assert_eq!(visit.progress().attempted_events, visit.progress().total_events);
}

#[test]
fn each_quantum_replays_once_and_reports_the_same_original_canonical_observation() {
    for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
        let root = Directory::new(); let control = Directory::new();
        let mut host = create(&root, 64); let p = plan(&host, 8);
        for tick in 2..=19 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
        let before = host.inspect(); let native = host.profile.clone(); drop(host);
        let mut c = coordinator(&control, &p);
        let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(20)).unwrap().unwrap();
        assert_eq!(visit.progress().replayed_events, 0);
        assert_eq!(visit.progress().total_events as u64, before.revision);
        assert_eq!(visit.domain(), 1);
        assert!(matches!(storage::Store::open(&root.store()), Err(JournalError::Busy)));
        ready(&mut visit, quantum);
        let result = visit.finish().unwrap().unwrap();
        assert_eq!(result.journal_revision, before.revision + 3);
        assert!(result.stop.as_ref().unwrap().drained());
        assert_eq!(result.executions, 0);
        let (passive, head, revision) = c.campaign.read_canonical(0).unwrap();
        let mut expected = result.clone(); expected.source = FileShutdownSource::CanonicalImage;
        assert_eq!(passive, expected);
        assert_eq!(revision as u64, result.journal_revision);
        assert_eq!(head.as_ref(), root.image().as_slice());
        assert_eq!(FileOversight::read_publication(root.store(), &native).unwrap().control.ledger.available, 100);
        assert!(c.report().all_observed_drained()); assert_eq!(c.revision(), 2);
    }
}

#[test]
fn cancellation_at_every_event_and_ready_records_no_domain_stop_or_cleanup() {
    for count in 0..=7 {
        let root = Directory::new(); let control = Directory::new();
        let mut host = create(&root, 64);
        for tick in 2..=6 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
        let p = plan(&host, 4); let original = root.image(); drop(host);
        let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"not an outcome").unwrap();
        let mut c = coordinator(&control, &p);
        let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(7)).unwrap().unwrap();
        assert_eq!(visit.progress().total_events, 7);
        for _ in 0..count { visit.advance(visit.progress().replayed_events, 1).unwrap(); }
        visit.cancel().unwrap();
        assert_eq!(root.image(), original); assert_eq!(std::fs::read(&pending).unwrap(), b"not an outcome");
        assert!(!c.report().all_observed_drained()); assert!(!c.report().unavailable);
        assert_eq!(c.revision(), 2); assert_eq!(c.report().visits.len(), 1);
        assert!(matches!(&c.report().visits[0].result, ShutdownVisitResult::Refused { diagnostic }
            if diagnostic == "Unavailable"));
        // An explicit new visit, not the cancelled cursor, can finish normally.
        c.recover_and_drain(c.revision(), 1, ElapsedTick(7)).unwrap().unwrap();
        assert!(!pending.exists()); assert!(c.report().all_observed_drained());
    }
}

#[test]
fn dropping_or_unwinding_keeps_intent_unacknowledged_and_old_success_unavailable() {
    for unwind in [false, true] {
        let root = Directory::new(); let control = Directory::new();
        let host = create(&root, 64); let p = plan(&host, 4); drop(host);
        let original = root.image(); let mut c = coordinator(&control, &p);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
            visit.advance(0, 1).unwrap();
            if unwind { panic!("operator interruption between quanta"); }
            drop(visit);
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(root.image(), original); assert!(c.report().unavailable);
        assert!(!c.report().all_observed_drained());
        assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
        assert_eq!(c.recover_and_drain(c.revision(), 1, ElapsedTick(2)), Err(JournalError::Unavailable));
        drop(c);
        let mut c = reopen(&control, &p, 1);
        assert!(!c.report().all_observed_drained());
        c.recover_and_drain(c.revision(), 1, ElapsedTick(2)).unwrap().unwrap();
        assert!(c.report().all_observed_drained());
    }
}

#[test]
fn rejected_polls_do_no_event_work_and_early_finish_is_an_acknowledged_refusal() {
    let root = Directory::new(); let control = Directory::new();
    let host = create(&root, 64); let p = plan(&host, 4); let original = root.image(); drop(host);
    let mut c = coordinator(&control, &p);
    let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    let before = visit.progress();
    for (expected, quantum, error) in [(1, 1, Error::Stale), (0, 0, Error::InvalidInput),
        (0, MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(visit.advance(expected, quantum), Err(error.into())); assert_eq!(visit.progress(), before);
    }
    assert_eq!(visit.finish().unwrap(), Err(Error::Incomplete.into()));
    assert_eq!(root.image(), original); assert!(!c.report().unavailable);
    let visit = c.begin_recover_and_drain(c.revision(), 1, ElapsedTick(0)).unwrap().unwrap();
    let mut visit = visit; ready(&mut visit, 1);
    assert_eq!(visit.finish().unwrap(), Err(Error::Stale.into()));
    assert_eq!(root.image(), original);
}

#[test]
fn original_head_limits_and_latest_prefix_are_checked_before_replay_or_domain_cleanup() {
    let root = Directory::new(); let control = Directory::new();
    let mut host = create(&root, 64); let p = plan(&host, 8); let old = root.image();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); let original = root.image();
    let mut c = coordinator(&control, &p); c.inspect_canonical(0, 1).unwrap().unwrap(); drop(host);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retained").unwrap();
    std::fs::write(root.store().join(storage::CANONICAL), &old).unwrap();
    let refused = c.begin_recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap().unwrap_err();
    assert_eq!(refused, Error::Stale.into()); assert!(pending.exists()); assert_eq!(root.image(), old);
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    let mut visit = c.begin_recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap().unwrap();
    ready(&mut visit, 1);
    // Same registered prefix no longer suffices if canonical custody changed
    // during replay. An older but valid image must never be overwritten.
    std::fs::write(root.store().join(storage::CANONICAL), &old).unwrap();
    assert_eq!(visit.finish().unwrap(), Err(Error::Binding.into()));
    assert_eq!(root.image(), old); assert!(pending.exists());
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 64);
    let original = root.image();
    let p = FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap()], 4, original.len()).unwrap();
    drop(host); let mut c = coordinator(&control, &p);
    let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap(); ready(&mut visit, 1);
    assert_eq!(visit.finish().unwrap(), Err(Error::Limit.into())); assert_eq!(root.image(), original);
}

#[test]
fn drained_capacity_retry_retains_the_original_sweep_without_new_domain_records() {
    let root = Directory::new(); let control = Directory::new();
    let host = create(&root, 5); let p = plan(&host, 4); drop(host);
    let mut c = coordinator(&control, &p);
    let first = c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(first.journal_revision, 5); let original = root.image();
    let mut visit = c.begin_recover_and_drain(c.revision(), 1, ElapsedTick(0)).unwrap().unwrap();
    ready(&mut visit, 1); let again = visit.finish().unwrap().unwrap();
    assert_eq!(again.source, FileShutdownSource::CanonicalImage);
    assert_eq!(again.last_drain, first.last_drain); assert_eq!(again.stop, first.stop);
    assert_eq!(root.image(), original); assert_eq!(again.journal_revision, 5);
}

#[test]
fn both_stores_preserve_their_distinct_failure_and_acknowledgment_boundaries() {
    for native in [true, false] { for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new();
        let host = create(&root, 5); let p = plan(&host, 8); let original = root.image(); drop(host);
        let mut c = coordinator(&control, &p);
        let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap(); ready(&mut visit, 1);
        if native { visit.recovery.fail_registered_once(barrier); }
        else { visit.coordinator.store.fail_once(barrier); }
        let result = visit.finish();
        if native {
            assert!(matches!(result, Ok(Err(JournalError::Io(ref failure))) if failure.operation == barrier));
            assert!(!c.report().unavailable);
            assert_eq!(root.image() != original, barrier == JournalIo::DirectorySync);
        } else {
            assert!(matches!(result, Err(JournalError::Io(ref failure)) if failure.operation == barrier));
            assert!(c.report().unavailable); assert_ne!(root.image(), original);
        }
        assert!(!c.report().all_observed_drained()); drop(c);
        let mut c = reopen(&control, &p, 1);
        let completed = root.image();
        c.recover_and_drain(c.revision(), 1, ElapsedTick(3)).unwrap().unwrap();
        assert!(c.report().all_observed_drained());
        if !native || barrier == JournalIo::DirectorySync { assert_eq!(root.image(), completed); }
    }}
}

#[allow(dead_code)]
#[path = "../../../recovery/fixture.rs"]
mod effect_fixture;

#[test]
fn original_reserved_executed_and_unknown_liabilities_survive_registered_recovery() {
    for mode in 0..4 {
        let root = effect_fixture::Directory::new(); let control = Directory::new();
        let mut live = effect_fixture::Ready::new(&root, effect_fixture::profile(), true);
        let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        if mode > 0 { live.dispatch(); }
        if mode == 2 { live.host.publish(live.host.revision(), 1).unwrap(); }
        let before = live.host.inspect(); drop(live);
        let mut c = coordinator(&control, &p);
        let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(if mode == 3 { 1001 } else { 2 })).unwrap().unwrap();
        ready(&mut visit, 1); let result = visit.finish().unwrap().unwrap();
        assert_eq!(result.journal_revision, before.revision + 3);
        assert_eq!(result.executions, u64::from(mode == 2));
        assert_eq!(result.dispatches_since_registration, if mode > 0 { vec![1] } else { vec![] });
        let progress = result.stop.unwrap(); assert_eq!(progress.drained(), mode != 3);
        assert_eq!(progress.charged_units, if mode >= 2 { 16 } else { 0 });
        assert_eq!(c.report().all_observed_drained(), mode != 3);
    }
}
