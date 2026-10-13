//! Original two-key journals and reducers, not synthetic successful stop reports.
use super::*;
#[path = "../../fixture.rs"]
mod fixture;
use fixture::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];

fn replay(root: &Directory, p: FileOversightProfile, quantum: usize) -> FileStopRecovery {
    let mut run = FileOversight::begin_stopped_recovery(root.store(), p).unwrap();
    while run.progress().status == FileStopRecoveryStatus::Replaying {
        let old = run.progress();
        let new = run.advance(old.replayed_events, quantum).unwrap();
        assert!(new.replayed_events - old.replayed_events <= quantum);
    }
    run
}
fn pending(root: &Directory) -> std::path::PathBuf {
    let path = root.store().join("delivery.pending");
    std::fs::write(&path, b"not a committed outcome").unwrap(); path
}
fn assert_pending(path: &Path) { assert_eq!(std::fs::read(path).unwrap(), b"not a committed outcome"); }

#[test]
fn separate_stop_and_drain_match_original_events_and_preserve_every_kind_of_charge() {
    for mode in 0..4 { for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
        let root = Directory::new(); let p = profile(); let mut live = Ready::new(&root, p.clone(), true);
        if mode > 0 { live.dispatch(); }
        if mode == 2 { live.host.publish(live.host.revision(), 1).unwrap(); }
        let request = request(&live.host); let before = live.host.inspect();
        let identity = storage::identity(&root.store()).unwrap();
        let mut events = journal::decode(&p, &identity, &root.bytes()).unwrap();
        let mut oracle = Machine::replay(&p, &events).unwrap();
        for event in [Event::Core(BaseEvent::Stop(request)), Event::Core(BaseEvent::Fence)] {
            oracle.apply(&event).unwrap(); events.push(event);
        }
        let expected = oracle.snapshot(events.len()); drop(live);
        let stopped = replay(&root, p.clone(), quantum).finish_admission_stop(request).unwrap();
        assert_eq!(stopped.snapshot(), &expected);
        assert_eq!(stopped.appended_events(), 2);
        assert_eq!(stopped.snapshot().control.ledger.elapsed, before.control.ledger.elapsed);
        assert_eq!(stopped.snapshot().control.ledger.charged, if mode > 0 { 16 } else { 0 });
        assert_eq!(stopped.snapshot().executions, u64::from(mode == 2));
        assert_eq!(root.bytes(), journal::encode(&p, &identity, &events).unwrap());
        stopped.history_anchor().check(&p, &identity, &events).unwrap();
        let stopped_bytes = root.bytes();
        let retry = replay(&root, p.clone(), quantum).finish_admission_stop(request).unwrap();
        assert_eq!(retry.appended_events(), 0); assert_eq!(retry.snapshot(), stopped.snapshot());
        assert_eq!(root.bytes(), stopped_bytes);
        let needs_drain = !oracle.broker.stop_progress().unwrap().drained();
        let now = ElapsedTick(if mode == 3 { 1001 } else { 2 });
        if needs_drain {
            let event = Event::Core(BaseEvent::StopProgress(now));
            oracle.apply(&event).unwrap(); events.push(event);
        }
        let calls = Cell::new(0);
        let drained = replay(&root, p.clone(), quantum).finish_stopped_drain(request, || {
            calls.set(calls.get() + 1); Ok(now)
        }).unwrap();
        assert_eq!(calls.get(), usize::from(needs_drain));
        assert_eq!(drained.stopped().snapshot(), &oracle.snapshot(events.len()));
        assert_eq!(root.bytes(), journal::encode(&p, &identity, &events).unwrap());
        assert_eq!(drained.stopped().snapshot().control.ledger.charged, if mode >= 2 { 16 } else { 0 });
        assert_eq!(drained.stopped().progress().drained(), mode != 3);
        if mode == 3 {
            let FileStoppedRecovery::Advanced { sweep, .. } = drained.stopped() else { panic!("required drain"); };
            assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::RetentionExpired));
        }
    }}
}

#[test]
fn a_three_event_reserve_supports_stop_then_one_event_drain_and_full_capacity_retries() {
    let sizing = Directory::new(); let mut live = Ready::new(&sizing, profile(), true); live.dispatch();
    let count = live.host.revision() as usize; drop(live);
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = count + 3;
    let mut live = Ready::new(&root, p.clone(), true); live.dispatch();
    assert_eq!(live.host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let request = request(&live.host); drop(live);
    let stopped = replay(&root, p.clone(), 1).finish_admission_stop(request).unwrap();
    assert_eq!(stopped.snapshot().revision as usize, count + 2);
    assert_eq!(stopped.snapshot().control.ledger.charged, 16);
    let completed = replay(&root, p.clone(), 1).finish_stopped_drain(request, || Ok(ElapsedTick(2))).unwrap();
    assert_eq!(completed.stopped().snapshot().revision as usize, count + 3);
    assert!(completed.stopped().progress().drained());
    let bytes = root.bytes();
    replay(&root, p.clone(), 1).finish_admission_stop(request).unwrap();
    let again = replay(&root, p, 1).finish_stopped_drain(request, || panic!("full exact retry needs no clock")).unwrap();
    assert!(matches!(again.stopped(), FileStoppedRecovery::AlreadyDrained { .. }));
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn incomplete_active_foreign_and_stale_requests_refuse_without_clock_or_cleanup() {
    let root = Directory::new(); let live = Ready::new(&root, profile(), true);
    let request = request(&live.host); drop(live); let original = root.bytes(); let staged = pending(&root);
    let run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
    assert_eq!(run.finish_admission_stop(request), Err(Error::Incomplete.into()));
    let run = FileOversight::begin_stopped_recovery(root.store(), profile()).unwrap();
    assert_eq!(run.finish_stopped_drain(request, || panic!("incomplete")), Err(Error::Incomplete.into()));
    assert_eq!(replay(&root, profile(), 1).finish_stopped_drain(request, || panic!("active domain")), Err(Error::WrongState.into()));
    let stale = StopRequest { expected_control_sequence: request.expected_control_sequence + 1, ..request };
    assert_eq!(replay(&root, profile(), 1).finish_admission_stop(stale), Err(Error::Stale.into()));
    assert_eq!(root.bytes(), original); assert_pending(&staged);
    replay(&root, profile(), 1).finish_admission_stop(request).unwrap();
    let stopped = root.bytes(); let staged = pending(&root);
    assert_eq!(replay(&root, profile(), 1).finish_admission_stop(stale), Err(Error::Binding.into()));
    let foreign = StopRequest { operation: 901, ..request };
    assert_eq!(replay(&root, profile(), 1).finish_stopped_drain(foreign, || panic!("foreign stop")), Err(Error::Duplicate.into()));
    assert_eq!(root.bytes(), stopped); assert_pending(&staged);
}

#[test]
fn failed_or_unwound_drain_clocks_keep_the_acknowledged_stop_and_original_liability() {
    for mode in 0..3 {
        let root = Directory::new(); let mut live = Ready::new(&root, profile(), true); live.dispatch();
        let request = request(&live.host); drop(live);
        replay(&root, profile(), 1).finish_admission_stop(request).unwrap();
        let stopped = root.bytes(); let staged = pending(&root);
        let run = replay(&root, profile(), 1);
        let outcome = catch_unwind(AssertUnwindSafe(|| run.finish_stopped_drain(request, || match mode {
            0 => Err(JournalError::Unavailable), 1 => panic!("clock failed"), _ => Ok(ElapsedTick(0)),
        })));
        match mode {
            0 => assert_eq!(outcome.unwrap(), Err(JournalError::Unavailable)),
            1 => assert!(outcome.is_err()),
            _ => assert_eq!(outcome.unwrap(), Err(Error::Stale.into())),
        }
        assert_eq!(root.bytes(), stopped); assert_pending(&staged);
        let state = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert!(state.stop.is_some()); assert_eq!(state.control.ledger.charged, 16);
        assert!(replay(&root, profile(), 1).finish_stopped_drain(request, || Ok(ElapsedTick(2)))
            .unwrap().stopped().progress().drained());
    }
}

#[test]
fn canonical_identity_is_checked_before_and_after_the_drain_clock() {
    for after_clock in [false, true] {
        let root = Directory::new(); let mut live = Ready::new(&root, profile(), true); live.dispatch();
        let request = request(&live.host); let original = root.bytes(); drop(live);
        replay(&root, profile(), 1).finish_admission_stop(request).unwrap(); let stopped = root.bytes();
        let run = replay(&root, profile(), 1); let staged = pending(&root); let calls = Cell::new(0);
        if !after_clock { std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap(); }
        let outcome = run.finish_stopped_drain(request, || {
            calls.set(calls.get() + 1);
            std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap(); Ok(ElapsedTick(2))
        });
        assert_eq!(outcome, Err(Error::Binding.into()));
        assert_eq!(calls.get(), usize::from(after_clock));
        assert_eq!(root.bytes(), original); assert_pending(&staged);
        std::fs::write(root.store().join(storage::CANONICAL), stopped).unwrap();
        replay(&root, profile(), 1).finish_stopped_drain(request, || Ok(ElapsedTick(2))).unwrap();
    }
}

#[test]
fn every_storage_barrier_returns_no_speculative_stop_or_drain_acknowledgment() {
    for drain in [false, true] { for barrier in BARRIERS {
        let root = Directory::new(); let mut live = Ready::new(&root, profile(), true); live.dispatch();
        let request = request(&live.host); drop(live);
        if drain { replay(&root, profile(), 1).finish_admission_stop(request).unwrap(); }
        let original = root.bytes(); let run = replay(&root, profile(), 1); run.store.fail_once(barrier);
        let result = if drain { run.finish_stopped_drain(request, || Ok(ElapsedTick(2))).map(|_| ()) }
            else { run.finish_admission_stop(request).map(|_| ()) };
        assert!(matches!(result, Err(JournalError::Io(ref failure)) if failure.operation == barrier));
        assert_eq!(root.bytes() != original, barrier == JournalIo::DirectorySync);
        // Explicit retry inspects the actual canonical cut; no uncertain result is imported.
        let stopped = replay(&root, profile(), 1).finish_admission_stop(request).unwrap();
        assert_eq!(stopped.appended_events(), if !drain && barrier != JournalIo::DirectorySync { 2 } else { 0 });
        let drained = replay(&root, profile(), 1).finish_stopped_drain(request, || Ok(ElapsedTick(2))).unwrap();
        assert!(drained.stopped().progress().drained());
        assert_eq!(drained.stopped().snapshot().executions, 0);
    }}
}

#[test]
fn each_cancellation_cut_and_independent_anchor_refuse_to_resurrect_an_active_image() {
    let root = Directory::new(); let mut live = Ready::new(&root, profile(), true); live.dispatch();
    let request = request(&live.host); let original = root.bytes(); drop(live);
    let stopped = replay(&root, profile(), 1).finish_admission_stop(request).unwrap();
    let bytes = root.bytes(); let staged = pending(&root);
    let count = stopped.snapshot().revision as usize;
    for cut in 0..=count {
        let mut run = FileOversight::begin_stopped_recovery_anchored(root.store(), profile(), stopped.history_anchor()).unwrap();
        for _ in 0..cut { run.advance(run.progress().replayed_events, 1).unwrap(); }
        assert_eq!(run.cancel().status, FileStopRecoveryStatus::Cancelled);
        assert_eq!(root.bytes(), bytes); assert_pending(&staged);
    }
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    assert!(matches!(FileOversight::begin_stopped_recovery_anchored(root.store(), profile(), stopped.history_anchor()),
        Err(JournalError::Contract(Error::Stale))));
    assert_pending(&staged); assert_eq!(root.bytes(), original);
    std::fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    replay(&root, profile(), 1).finish_stopped_drain(request, || Ok(ElapsedTick(2))).unwrap();
}

#[test]
fn insufficient_stop_or_drain_space_never_spends_a_partial_terminal_tail() {
    let sizing = Directory::new(); let mut live = Ready::new(&sizing, profile(), false); live.dispatch();
    let count = live.host.revision() as usize; drop(live);
    for available in [1, 2] {
        let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = count + available;
        let mut live = Ready::new(&root, p.clone(), false); live.dispatch();
        let request = request(&live.host); drop(live); let original = root.bytes(); let staged = pending(&root);
        let result = replay(&root, p.clone(), 1).finish_admission_stop(request);
        if available == 1 {
            assert_eq!(result, Err(Error::Limit.into())); assert_eq!(root.bytes(), original); assert_pending(&staged);
        } else {
            let stopped = result.unwrap(); assert_eq!(stopped.snapshot().control.ledger.charged, 16);
            let bytes = root.bytes(); let staged = pending(&root);
            assert_eq!(replay(&root, p.clone(), 1).finish_stopped_drain(request, || panic!("no event space")), Err(Error::Limit.into()));
            assert_eq!(root.bytes(), bytes); assert_pending(&staged);
            assert_eq!(replay(&root, p, 1).finish_admission_stop(request).unwrap().appended_events(), 0);
        }
    }
}
