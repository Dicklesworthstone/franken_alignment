//! Actual canonical/domain files. Synthetic journals are enforcement fixtures.
use super::*;
use super::super::test_support::*;
use crate::action::consequence::delivery::persistent::observed::shutdown::campaign::observe;
use crate::action::consequence::delivery::persistent::observed::machine::Transition;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn pending(root: &Directory, control: &Directory, visits: usize)
    -> (FileShutdownPlan, FileShutdownCoordinator, FileOversight)
{
    let mut host = create(root, 32);
    let p = plan(&host, visits);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    enter(&mut c); drop(c); // Real durable intent whose reply never completed.
    host.stop_and_drain(host.revision(), request(&host, 900), ElapsedTick(2)).unwrap();
    let c = reopen(control, &p, 1);
    (p, c, host)
}
fn complete(mut read: FileShutdownVisitResolution<'_>, quantum: usize) -> FileShutdownObservation {
    while read.progress().status == FileStopRecoveryStatus::Replaying {
        let before = read.progress();
        let after = read.advance(before.replayed_events, quantum).unwrap();
        assert!(after.replayed_events - before.replayed_events <= quantum);
        assert_eq!(after.replayed_events, after.attempted_events);
    }
    read.finish().unwrap()
}
fn eager(c: &FileShutdownCoordinator, bytes: &[u8]) -> FileShutdownObservation {
    let d = &c.campaign.plan.domains[0];
    let events = journal::decode(&d.profile, &d.directory, bytes).unwrap();
    let mut machine = Machine::new(&d.profile).unwrap();
    let mut last = None;
    for (i, event) in events.iter().enumerate() {
        if let Transition::StopProgressed(sweep) = machine.apply(event).unwrap() {
            last = Some(FileShutdownDrain { journal_revision: (i + 1) as u64, sweep });
        }
    }
    observe(d, &machine, &events, FileShutdownSource::CanonicalImage, last).unwrap()
}

#[test]
fn passive_quanta_match_original_machine_and_resolve_at_the_durable_visit_ceiling() {
    for quantum in [1, 2, 7, MAX_JOURNAL_EVENTS] {
        let root = Directory::new(); let control = Directory::new();
        let (p, mut c, host) = pending(&root, &control, 1);
        let native = root.image(); let expected = eager(&c, &native);
        let read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
        assert_eq!(read.progress().replayed_events, 0);
        // Original domain custody remains live; resolution never asks for it.
        assert!(matches!(FileOversight::open(root.store(), host.profile.clone()), Err(JournalError::Busy)));
        assert_eq!(complete(read, quantum), expected);
        assert_eq!(c.revision(), 2); assert_eq!(c.report().visits.len(), 1);
        assert!(c.report().all_observed_drained()); assert_eq!(root.image(), native);
        let acknowledged = control.image(); drop(c);
        let mut c = reopen(&control, &p, 2);
        let read = c.begin_resolve_shutdown_visit(2, 0).unwrap();
        assert_eq!(complete(read, quantum), expected);
        assert_eq!(control.image(), acknowledged); assert_eq!(root.image(), native);
        assert_eq!(c.revision(), 2); assert_eq!(c.report().visits.len(), 1);
    }
}

#[test]
fn cancellation_at_every_cut_preserves_intents_files_and_the_spent_attempt() {
    for recorded in [false, true] {
        for cut in 0..=4 {
            let root = Directory::new(); let control = Directory::new();
            let (_p, mut c, _host) = pending(&root, &control, 16);
            if recorded { c.resolve_shutdown_visit(1, 0).unwrap(); }
            let before = c.report(); let native = root.image(); let durable = control.image();
            std::fs::write(root.store().join("delivery.pending"), b"domain staged evidence").unwrap();
            std::fs::write(control.store().join("delivery.pending"), b"coordinator staged evidence").unwrap();
            let mut read = c.begin_resolve_shutdown_visit(c.revision(), 0).unwrap();
            assert_eq!(read.progress().total_events, 4);
            for _ in 0..cut { read.advance(read.progress().replayed_events, 1).unwrap(); }
            assert!(read.coordinator.report().unavailable);
            read.cancel().unwrap();
            assert!(!c.report().unavailable); assert!(!c.report().all_observed_drained());
            assert_eq!(c.report().revision, before.revision); assert_eq!(c.report().visits, before.visits);
            assert_eq!(c.report().current_session.attempts.len(), before.current_session.attempts.len() + 1);
            assert_eq!(root.image(), native); assert_eq!(control.image(), durable);
            assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"domain staged evidence");
            assert_eq!(std::fs::read(control.store().join("delivery.pending")).unwrap(), b"coordinator staged evidence");
        }
    }
}

#[test]
fn drop_and_original_event_unwind_cannot_be_cleared_by_finish_or_cancel() {
    for mode in 0..3 {
        let root = Directory::new(); let control = Directory::new();
        let (p, mut c, _host) = pending(&root, &control, 8);
        let native = root.image(); let durable = control.image();
        let mut read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
        if mode == 0 {
            read.advance(0, MAX_JOURNAL_EVENTS).unwrap(); drop(read);
        } else {
            assert!(catch_unwind(AssertUnwindSafe(|| read.read.interrupt_after_original_event())).is_err());
            let stopped = read.progress();
            assert_eq!(stopped.status, FileStopRecoveryStatus::Interrupted);
            assert_eq!(stopped.attempted_events, 1); assert_eq!(stopped.replayed_events, 0);
            assert_eq!(read.advance(0, 1), Err(JournalError::Unavailable));
            assert_eq!(read.progress(), stopped);
            if mode == 1 { assert_eq!(read.finish(), Err(JournalError::Unavailable)); }
            else { assert_eq!(read.cancel(), Err(JournalError::Unavailable)); }
        }
        assert!(c.report().unavailable); assert_eq!(root.image(), native); assert_eq!(control.image(), durable);
        assert!(matches!(c.begin_resolve_shutdown_visit(1, 0), Err(JournalError::Unavailable)));
        drop(c); let mut c = reopen(&control, &p, 1);
        assert!(complete(c.begin_resolve_shutdown_visit(1, 0).unwrap(), 1).stop.unwrap().drained());
        assert_eq!(root.image(), native);
    }
}

#[test]
fn poll_preconditions_do_no_work_and_a_bad_suffix_never_exports_partial_success() {
    let root = Directory::new(); let control = Directory::new();
    let (_p, mut c, host) = pending(&root, &control, 8);
    let native = root.image(); let durable = control.image();
    let mut events = journal::decode(&host.profile, &root.store(), &native).unwrap();
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    let bad = journal::encode(&host.profile, &root.store(), &events).unwrap();
    std::fs::write(root.store().join(storage::CANONICAL), &bad).unwrap();
    let mut read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
    let zero = read.progress();
    for (expected, quantum, error) in [(1, 1, Error::Stale), (0, 0, Error::InvalidInput),
        (0, MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(read.advance(expected, quantum), Err(error.into())); assert_eq!(read.progress(), zero);
    }
    assert_eq!(read.advance(0, MAX_JOURNAL_EVENTS), Err(Error::Stale.into()));
    let failed = read.progress(); assert_eq!(failed.status, FileStopRecoveryStatus::Failed(Error::Stale));
    assert_eq!(failed.replayed_events, 4); assert_eq!(failed.attempted_events, 5);
    assert_eq!(read.advance(4, 1), Err(Error::Stale.into())); assert_eq!(read.progress(), failed);
    assert_eq!(read.finish(), Err(Error::Stale.into()));
    assert!(!c.report().unavailable); assert!(!c.report().all_observed_drained());
    assert_eq!(control.image(), durable); assert_eq!(root.image(), bad);
    assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
    std::fs::write(root.store().join(storage::CANONICAL), &native).unwrap();
    assert!(complete(c.begin_resolve_shutdown_visit(1, 0).unwrap(), 1).stop.unwrap().drained());
}

#[test]
fn a_concurrent_writer_does_not_change_the_frozen_cut_or_create_domain_cleanup() {
    let root = Directory::new(); let control = Directory::new();
    let (_p, mut c, mut host) = pending(&root, &control, 8);
    let expected = eager(&c, &root.image());
    let read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
    host.progress_stop(host.revision(), ElapsedTick(3)).unwrap();
    let successor = root.image();
    std::fs::write(root.store().join("delivery.pending"), b"do not clean").unwrap();
    assert_eq!(complete(read, 1), expected);
    assert_eq!(root.image(), successor);
    assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"do not clean");
    let durable = control.image();
    assert_eq!(c.resolve_shutdown_visit(2, 0), Err(Error::Stale.into()));
    assert!(!c.report().all_observed_drained()); assert_eq!(control.image(), durable);
}

#[test]
fn failed_completion_never_repeats_domain_settlement_and_visible_ack_is_resolvable() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new();
        let (p, mut c, _host) = pending(&root, &control, 1);
        let native = root.image(); c.store.fail_once(barrier);
        let mut read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
        read.advance(0, MAX_JOURNAL_EVENTS).unwrap();
        assert!(matches!(read.finish(), Err(JournalError::Io(f)) if f.operation == barrier));
        assert!(c.report().unavailable); assert!(!c.report().all_observed_drained());
        assert_eq!(root.image(), native); drop(c);
        let mut c = reopen(&control, &p, 1);
        assert_eq!(c.revision(), if barrier == JournalIo::DirectorySync { 2 } else { 1 });
        let revision = c.revision();
        assert!(complete(c.begin_resolve_shutdown_visit(revision, 0).unwrap(), 2).stop.unwrap().drained());
        assert_eq!(c.revision(), 2); assert_eq!(c.report().visits.len(), 1); assert_eq!(root.image(), native);
    }
}

#[test]
fn unstopped_or_wrong_stop_evidence_never_falls_through_into_a_domain_command() {
    for operation in [0, 901, 900] {
        let root = Directory::new(); let control = Directory::new(); let mut host = create(&root, 32);
        let p = plan(&host, 4);
        let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        enter(&mut c); drop(c);
        if operation != 0 { host.request_stop(host.revision(), request(&host, operation)).unwrap(); }
        let mut c = reopen(&control, &p, 1); let native = root.image(); let durable = control.image();
        let mut read = c.begin_resolve_shutdown_visit(1, 0).unwrap();
        read.advance(0, MAX_JOURNAL_EVENTS).unwrap();
        let result = read.finish();
        if operation == 900 {
            assert!(!result.unwrap().stop.unwrap().drained());
            assert!(c.report().current_session.all_observed_stopped()); assert!(!c.report().all_observed_drained());
        } else {
            assert_eq!(result, Err(if operation == 0 { Error::Incomplete } else { Error::Binding }.into()));
            assert_eq!(control.image(), durable);
        }
        assert_eq!(root.image(), native);
    }
}

#[test]
fn foreign_prefix_and_head_space_refuse_before_replay_without_a_new_durable_visit() {
    for enough in [false, true] {
        let root = Directory::new(); let control = Directory::new(); let mut host = create(&root, 32);
        let domain = host.shutdown_domain(1).unwrap(); let original = root.image();
        let p = FileShutdownPlan::new(900, vec![domain], 8,
            if enough { MAX_SHUTDOWN_HEAD_BYTES } else { original.len() }).unwrap();
        let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        enter(&mut c); drop(c);
        host.stop_and_drain(host.revision(), request(&host, 900), ElapsedTick(2)).unwrap();
        let mut c = reopen(&control, &p, 1); let durable = control.image(); let native = root.image();
        if !enough {
            assert!(matches!(c.begin_resolve_shutdown_visit(1, 0), Err(JournalError::Contract(Error::Limit))));
        } else {
            let mut changed = journal::decode(&host.profile, &root.store(), &native).unwrap();
            changed[1] = Event::Core(BaseEvent::Time(ElapsedTick(2)));
            Machine::replay(&host.profile, &changed).unwrap();
            let fork = journal::encode(&host.profile, &root.store(), &changed).unwrap();
            std::fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
            assert!(matches!(c.begin_resolve_shutdown_visit(1, 0), Err(JournalError::Contract(Error::Binding))));
            std::fs::write(root.store().join(storage::CANONICAL), &native).unwrap();
            assert!(complete(c.begin_resolve_shutdown_visit(1, 0).unwrap(), 1).stop.unwrap().drained());
        }
        assert!(!c.report().unavailable); assert_eq!(root.image(), native);
        assert_eq!(c.report().visits.len(), 1);
        if !enough { assert_eq!(control.image(), durable); }
    }
}
