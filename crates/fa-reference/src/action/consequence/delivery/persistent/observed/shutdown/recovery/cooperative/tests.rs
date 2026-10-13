//! Native two-key journals; fixtures do not establish remote-effect guarantees.
use super::*;
#[path = "../fixture.rs"]
mod fixture;
use fixture::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn complete(run: &mut FileStopRecovery, quantum: usize) {
    while run.progress().status == FileStopRecoveryStatus::Replaying {
        let before = run.progress();
        let progress = run.advance(before.replayed_events, quantum).unwrap();
        assert!(progress.replayed_events - before.replayed_events <= quantum);
        assert_eq!(progress.attempted_events, progress.replayed_events);
    }
}

#[test]
fn every_quantum_matches_original_terminal_transitions_for_each_liability() {
    for mode in 0..4 {
        for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
            let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
            if mode > 0 { ready.dispatch(); }
            if mode == 2 { ready.host.publish(ready.host.revision(), 1).unwrap(); }
            let stop = request(&ready.host); let before = root.bytes();
            let revision = ready.host.revision() as usize;
            let now = ElapsedTick(if mode == 3 { 1001 } else { 2 });
            // Independent original reducer oracle, not the new cursor/finalizer.
            let identity = storage::identity(&root.store()).unwrap();
            let events = journal::decode(&profile(), &identity, &before).unwrap();
            let mut oracle = Machine::replay(&profile(), &events).unwrap();
            for event in [BaseEvent::Stop(stop), BaseEvent::Fence, BaseEvent::StopProgress(now)] {
                oracle.apply(&Event::Core(event)).unwrap();
            }
            let expected = oracle.snapshot(revision + 3);
            drop(ready);
            let pending = root.store().join("delivery.pending");
            std::fs::write(&pending, b"never promote staging").unwrap();
            let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap()
                .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
            assert_eq!(run.stop_request(900), Err(Error::Incomplete.into()));
            while run.progress().status == FileStopRecoveryStatus::Replaying {
                let progress = run.progress();
                let next = run.advance(progress.replayed_events, quantum).unwrap();
                assert!(next.replayed_events - progress.replayed_events <= quantum);
                assert_eq!(root.bytes(), before);
                assert_eq!(std::fs::read(&pending).unwrap(), b"never promote staging");
                assert!(matches!(FileOversight::begin_stopped_recovery(root.store(), profile()), Err(JournalError::Busy)));
            }
            assert_eq!(run.stop_request(900).unwrap(), stop);
            assert_eq!(run.progress().attempted_events, revision);
            let result = run.finish_stopped(stop, || now).unwrap();
            assert_eq!(result.snapshot(), &expected);
            assert_eq!(result.progress().drained(), mode != 3);
            assert_eq!(result.snapshot().executions, u64::from(mode == 2));
            assert_eq!(result.snapshot().control.ledger.charged, if mode >= 2 { 16 } else { 0 });
            if mode == 3 {
                let FileStoppedRecovery::Advanced { sweep, .. } = &result else { panic!("first stop"); };
                assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::RetentionExpired));
            }
            assert!(!pending.exists());
        }
    }
}

#[test]
fn cancellation_at_every_replay_boundary_preserves_canonical_and_staging() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let count = ready.host.revision() as usize; drop(ready);
    let before = root.bytes(); let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"staged").unwrap();
    for boundary in 0..=count {
        let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
        for _ in 0..boundary { run.advance(run.progress().replayed_events, 1).unwrap(); }
        let progress = run.cancel();
        assert_eq!(progress.status, FileStopRecoveryStatus::Cancelled);
        assert_eq!(progress.replayed_events, boundary);
        assert_eq!(root.bytes(), before); assert_eq!(std::fs::read(&pending).unwrap(), b"staged");
        // Cancellation really released the original lock, not merely its status.
        drop(FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap());
    }
}

#[test]
fn stale_or_invalid_polls_and_early_completion_never_execute_an_event() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let stop = request(&ready.host); drop(ready); let before = root.bytes();
    let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
    assert_eq!(run.advance(1, 1), Err(Error::Stale.into()));
    assert_eq!(run.advance(0, 0), Err(Error::InvalidInput.into()));
    assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS + 1), Err(Error::Limit.into()));
    assert_eq!(run.progress().attempted_events, 0);
    assert_eq!(run.finish_stopped(stop, || panic!("incomplete replay")), Err(Error::Incomplete.into()));
    assert_eq!(root.bytes(), before);
    let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
    complete(&mut run, 2); let progress = run.progress();
    assert_eq!(run.advance(progress.replayed_events, 1).unwrap(), progress);
    assert_eq!(run.advance(0, 1), Err(Error::Stale.into()));
    assert_eq!(run.stop_request(0), Err(Error::InvalidInput.into()));
    let wrong = StopRequest { expected_control_sequence: stop.expected_control_sequence + 1, ..stop };
    assert_eq!(run.finish_stopped(wrong, || panic!("wrong predecessor")), Err(Error::Stale.into()));
    assert_eq!(root.bytes(), before);
}

#[test]
fn original_transition_failure_and_unwind_latch_the_partial_machine() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let stop = request(&ready.host); drop(ready); let before = root.bytes();
    let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        run.advance_with(0, 1, |machine, event| {
            machine.apply(event)?;
            panic!("interrupted after actual original event");
        })
    })).is_err());
    assert_eq!(run.progress().status, FileStopRecoveryStatus::Interrupted);
    assert_eq!(run.progress().attempted_events, 1); assert_eq!(run.progress().replayed_events, 0);
    assert_eq!(run.advance(0, 1), Err(JournalError::Unavailable));
    assert_eq!(run.finish_stopped(stop, || panic!("unusable machine")), Err(JournalError::Unavailable));
    assert_eq!(root.bytes(), before);
    // Canonically framed but semantically invalid suffix: original time regresses.
    let identity = storage::identity(&root.store()).unwrap();
    let mut events = journal::decode(&profile(), &identity, &before).unwrap();
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    let invalid = journal::encode(&profile(), &identity, &events).unwrap();
    std::fs::write(root.store().join(storage::CANONICAL), &invalid).unwrap();
    let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
    assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS), Err(Error::Stale.into()));
    assert_eq!(run.progress().status, FileStopRecoveryStatus::Failed(Error::Stale));
    assert_eq!(run.progress().attempted_events, events.len());
    assert_eq!(run.progress().replayed_events, events.len() - 1);
    assert_eq!(run.advance(events.len() - 1, 1), Err(Error::Stale.into()));
    assert_eq!(run.stop_request(900), Err(Error::Stale.into()));
    assert_eq!(run.cancel().status, FileStopRecoveryStatus::Failed(Error::Stale));
    assert_eq!(root.bytes(), invalid);
}

#[test]
fn full_capacity_retry_returns_the_exact_stopped_anchor_without_a_clock() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 15;
    let ready = Ready::new(&root, p.clone(), true); let stop = request(&ready.host);
    let anchor = FileHistoryAnchor::from_acknowledged(ready.host.revision() as usize, root.bytes());
    drop(ready);
    let mut run = FileOversight::begin_stopped_recovery_anchored(root.store(), p.clone(), &anchor).unwrap();
    complete(&mut run, 1);
    let first = run.finish_stopped_with_anchor(stop, || ElapsedTick(2)).unwrap();
    assert_eq!(first.stopped().snapshot().revision, 15); let canonical = root.bytes();
    for quantum in [1, MAX_JOURNAL_EVENTS] {
        let mut run = FileOversight::begin_stopped_recovery_anchored(root.store(), p.clone(), first.history_anchor()).unwrap();
        complete(&mut run, quantum);
        assert_eq!(run.stop_request(901), Err(Error::Duplicate.into()));
        let again = run.finish_stopped_with_anchor(stop, || panic!("already drained")).unwrap();
        assert!(matches!(again.stopped(), FileStoppedRecovery::AlreadyDrained { .. }));
        assert_eq!(again.stopped().snapshot(), first.stopped().snapshot());
        assert_eq!(again.history_anchor(), first.history_anchor());
        assert_eq!(root.bytes(), canonical);
    }
}

#[test]
fn canonical_changes_before_or_during_final_clock_cannot_be_overwritten() {
    for during_clock in [false, true] {
        let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
        let stop = request(&ready.host); drop(ready); let before = root.bytes();
        let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"staged").unwrap();
        let identity = storage::identity(&root.store()).unwrap();
        let mut events = journal::decode(&profile(), &identity, &before).unwrap();
        events.push(Event::Core(BaseEvent::Time(ElapsedTick(2))));
        let changed = journal::encode(&profile(), &identity, &events).unwrap();
        let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
        complete(&mut run, 1);
        if !during_clock { std::fs::write(root.store().join(storage::CANONICAL), &changed).unwrap(); }
        let calls = Cell::new(0);
        assert_eq!(run.finish_stopped(stop, || {
            assert!(during_clock); calls.set(calls.get() + 1);
            std::fs::write(root.store().join(storage::CANONICAL), &changed).unwrap(); ElapsedTick(3)
        }), Err(Error::Binding.into()));
        assert_eq!(calls.get(), usize::from(during_clock));
        assert_eq!(root.bytes(), changed); assert_eq!(std::fs::read(&pending).unwrap(), b"staged");
    }
}

#[test]
fn every_storage_barrier_retains_uncertainty_and_recognizes_visible_completion() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
        ready.dispatch(); let stop = request(&ready.host); let revision = ready.host.revision(); drop(ready);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let mut run = FileStopRecovery::begin_store(store, profile(), None).unwrap(); complete(&mut run, 1);
        let error = run.finish_stopped(stop, || ElapsedTick(2)).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("original I/O failure"); };
        assert_eq!(failure.operation, barrier);
        let visible = barrier == JournalIo::DirectorySync;
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.revision, revision + if visible { 3 } else { 0 });
        let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap(); complete(&mut run, 2);
        let stopped = run.finish_stopped(stop, || { assert!(!visible); ElapsedTick(3) }).unwrap();
        assert_eq!(matches!(stopped, FileStoppedRecovery::AlreadyDrained { .. }), visible);
        assert!(stopped.progress().drained()); assert_eq!(stopped.snapshot().executions, 0);
        assert_eq!(stopped.snapshot().control.ledger.available, 100);
    }
}

#[test]
fn anchors_reserves_and_profile_mismatches_refuse_without_cleanup() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let revision = ready.host.revision() as usize; let original = root.bytes(); drop(ready);
    let identity = storage::identity(&root.store()).unwrap();
    let mut events = journal::decode(&profile(), &identity, &original).unwrap();
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(2))));
    let anchor = FileHistoryAnchor::from_acknowledged(revision + 1, journal::encode(&profile(), &identity, &events).unwrap());
    *events.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    let fork = journal::encode(&profile(), &identity, &events).unwrap();
    std::fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"staged").unwrap();
    assert!(FileOversight::begin_stopped_recovery_anchored(root.store(), profile(), &anchor).is_err());
    let mut foreign = profile(); foreign.delivery.total += 1;
    assert!(FileOversight::begin_stopped_recovery(root.store(), foreign).is_err());
    let wrong = RecoveryReserve { events: 1, bytes: 1 };
    assert!(FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap().require_recovery_reserve(wrong).is_err());
    let mut run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap(); run.advance(0, 1).unwrap();
    assert!(matches!(run.require_recovery_reserve(RecoveryReserve::terminal()), Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(root.bytes(), fork); assert_eq!(std::fs::read(pending).unwrap(), b"staged");
}
