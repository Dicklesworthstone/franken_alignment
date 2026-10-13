//! Original two-key publication histories, not a substitute recovery reducer.
use super::*;
#[path = "../fixture.rs"]
mod fixture;
use fixture::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event};
use std::{cell::Cell, fs};

fn exact_anchor(root: &Directory, history: &FileStoppedHistory) {
    let revision = history.stopped().snapshot().revision;
    assert_eq!(history.history_anchor().revision(), revision);
    assert_eq!(history.history_anchor(), &FileHistoryAnchor::from_acknowledged(
        usize::try_from(revision).unwrap(), root.bytes()));
    assert_eq!(&FileOversight::read_publication(root.store(), &profile()).unwrap(),
        history.stopped().snapshot());
}

// Produce a valid same-path, same-counter fork using the ORIGINAL canonical
// encoder and reducer. Only the final trusted time observation changes. Both
// histories are valid; counter-only stop selection cannot distinguish them.
fn fork_time(root: &Directory, bytes: &[u8]) -> Vec<u8> {
    let mut events = journal::decode(&profile(), &root.store(), bytes).unwrap();
    let original = Machine::replay(&profile(), &events).unwrap().snapshot(events.len());
    assert!(matches!(events.last(), Some(Event::Core(BaseEvent::Time(ElapsedTick(2))))));
    *events.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    let fork = journal::encode(&profile(), &root.store(), &events).unwrap();
    let state = Machine::replay(&profile(), &events).unwrap().snapshot(events.len());
    assert_ne!(fork, bytes); assert_eq!(state.revision, original.revision);
    assert_eq!(state.control.sequence, original.control.sequence);
    assert_eq!(state.control.ledger.epoch, original.control.ledger.epoch);
    fork
}

#[test]
fn anchored_stop_preserves_original_reserved_sent_and_executed_dispositions() {
    for mode in 0..3 {
        let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
        let prefix = ready.host.history_anchor().unwrap();
        if mode > 0 { ready.dispatch(); }
        if mode == 2 { ready.host.publish(ready.host.revision(), 1).unwrap(); }
        let before = ready.host.inspect(); let stop = request(&ready.host); drop(ready);
        let clocks = Cell::new(0);
        let history = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
            &prefix, || { clocks.set(clocks.get() + 1); ElapsedTick(2) }).unwrap();
        assert_eq!(clocks.get(), 1);
        assert!(matches!(history.stopped(), FileStoppedRecovery::Advanced { .. }));
        assert!(history.stopped().progress().drained());
        assert_eq!(history.stopped().progress().receipt.request(), stop);
        let after = history.stopped().snapshot(); assert_eq!(after.revision, before.revision + 3);
        assert_eq!(after.executions, u64::from(mode == 2));
        assert_eq!(after.control.ledger.charged, if mode == 2 { 16 } else { 0 });
        assert_eq!(after.control.ledger.reserved, 0); exact_anchor(&root, &history);
        let bytes = root.bytes();
        let again = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
            history.history_anchor(), || panic!("a drained exact retry never needs time")).unwrap();
        assert!(matches!(again.stopped(), FileStoppedRecovery::AlreadyDrained { .. }));
        assert_eq!(again.stopped().snapshot(), after);
        assert_eq!(again.history_anchor(), history.history_anchor()); assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn a_valid_same_counter_fork_is_refused_before_clock_or_staging_cleanup() {
    let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
    ready.host.observe_time(ready.host.revision(), ElapsedTick(2)).unwrap();
    let anchor = ready.host.history_anchor().unwrap(); let stop = request(&ready.host);
    let genuine = root.bytes(); drop(ready); let fork = fork_time(&root, &genuine);
    let pending = root.store().join("delivery.pending"); fs::write(&pending, b"retain this staging").unwrap();
    fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
    assert!(FileOversight::read_publication(root.store(), &profile()).is_ok());
    assert_eq!(FileOversight::recover_stopped_anchored(root.store(), profile(), stop, &anchor,
        || panic!("an equal-counter fork cannot reach the clock")), Err(Error::Binding.into()));
    assert_eq!(root.bytes(), fork); assert_eq!(fs::read(&pending).unwrap(), b"retain this staging");
    fs::write(root.store().join(storage::CANONICAL), &genuine).unwrap();
    let history = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        &anchor, || ElapsedTick(4)).unwrap();
    assert!(history.stopped().progress().drained()); exact_anchor(&root, &history); assert!(!pending.exists());
}

#[test]
fn a_retained_stopped_successor_rejects_rollback_even_with_the_original_stop_counters() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let prefix = ready.host.history_anchor().unwrap(); let stop = request(&ready.host);
    let before = root.bytes(); drop(ready);
    let history = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        &prefix, || ElapsedTick(2)).unwrap();
    let completed = root.bytes(); let pending = root.store().join("delivery.pending");
    fs::write(&pending, &completed).unwrap();
    fs::write(root.store().join(storage::CANONICAL), &before).unwrap();
    assert_eq!(FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        history.history_anchor(), || panic!("rollback cannot reacquire time")), Err(Error::Stale.into()));
    assert_eq!(root.bytes(), before); assert_eq!(fs::read(&pending).unwrap(), completed);
    fs::write(root.store().join(storage::CANONICAL), &completed).unwrap();
    let retry = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        history.history_anchor(), || panic!("genuine drained history needs no clock")).unwrap();
    assert_eq!(retry.history_anchor(), history.history_anchor()); assert_eq!(root.bytes(), completed);
    assert!(!pending.exists()); exact_anchor(&root, &retry);
}

#[test]
fn full_capacity_retry_confirms_the_same_anchor_without_a_replacement() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 15;
    let ready = Ready::new(&root, p.clone(), true);
    assert_eq!(ready.host.revision(), 12);
    assert_eq!(ready.host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let prefix = ready.host.history_anchor().unwrap(); let stop = request(&ready.host); drop(ready);
    let history = FileOversight::recover_stopped_anchored(root.store(), p.clone(), stop,
        &prefix, || ElapsedTick(2)).unwrap();
    assert_eq!(history.stopped().snapshot().revision, 15); let completed = root.bytes();
    for _ in 0..3 {
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(JournalIo::Stage);
        let retry = recover_anchored_locked(store, p.clone(), stop, history.history_anchor(),
            || panic!("full capacity drained retry never reads time")).unwrap();
        assert!(matches!(retry.stopped(), FileStoppedRecovery::AlreadyDrained { .. }));
        assert_eq!(retry.history_anchor(), history.history_anchor()); assert_eq!(root.bytes(), completed);
        assert_eq!(retry.stopped().snapshot(), history.stopped().snapshot());
    }
}

#[test]
fn every_storage_barrier_yields_no_candidate_anchor_and_retry_uses_the_visible_cut_only() {
    for executed in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
            ready.dispatch();
            if executed { ready.host.publish(ready.host.revision(), 1).unwrap(); }
            let before = ready.host.inspect(); let stop = request(&ready.host);
            let anchor = ready.host.history_anchor().unwrap(); drop(ready);
            let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
            let error = recover_anchored_locked(store, profile(), stop, &anchor, || ElapsedTick(2)).unwrap_err();
            let JournalError::Io(failure) = error else { panic!("selected native storage barrier"); };
            assert_eq!(failure.operation, barrier);
            let visible = barrier == JournalIo::DirectorySync;
            let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
            assert_eq!(disk.revision, before.revision + if visible { 3 } else { 0 });
            let clocks = Cell::new(0);
            let retry = FileOversight::recover_stopped_anchored(root.store(), profile(), stop, &anchor, || {
                assert!(!visible, "acknowledged visible stopping cannot repeat");
                clocks.set(clocks.get() + 1); ElapsedTick(3)
            }).unwrap();
            assert_eq!(clocks.get(), u32::from(!visible));
            assert_eq!(matches!(retry.stopped(), FileStoppedRecovery::AlreadyDrained { .. }), visible);
            assert!(retry.stopped().progress().drained()); exact_anchor(&root, &retry);
            assert_eq!(retry.stopped().snapshot().executions, u64::from(executed));
            assert_eq!(retry.stopped().progress().charged_units, if executed { 16 } else { 0 });
            let completed = root.bytes();
            let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(JournalIo::Stage);
            let repeated = recover_anchored_locked(store, profile(), stop, retry.history_anchor(),
                || panic!("neither ambiguous-write nor anchor retry renews time")).unwrap();
            assert_eq!(repeated.history_anchor(), retry.history_anchor()); assert_eq!(root.bytes(), completed);
        }
    }
}

#[test]
fn invalid_time_unwind_and_clock_time_canonical_change_leave_staging_and_liabilities_unchanged() {
    let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
    ready.dispatch(); ready.host.observe_time(ready.host.revision(), ElapsedTick(2)).unwrap();
    let anchor = ready.host.history_anchor().unwrap(); let stop = request(&ready.host);
    let before = root.bytes(); drop(ready); let fork = fork_time(&root, &before);
    let pending = root.store().join("delivery.pending"); fs::write(&pending, b"do not clean yet").unwrap();
    assert_eq!(FileOversight::recover_stopped_anchored(root.store(), profile(), stop, &anchor,
        || ElapsedTick(0)), Err(Error::Stale.into()));
    assert_eq!(root.bytes(), before); assert_eq!(fs::read(&pending).unwrap(), b"do not clean yet");
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        FileOversight::recover_stopped_anchored(root.store(), profile(), stop, &anchor,
            || panic!("trusted clock unavailable"))
    }));
    assert!(failure.is_err()); assert_eq!(root.bytes(), before);
    assert_eq!(fs::read(&pending).unwrap(), b"do not clean yet");
    let failure = FileOversight::recover_stopped_anchored(root.store(), profile(), stop, &anchor, || {
        fs::write(root.store().join(storage::CANONICAL), &fork).unwrap(); ElapsedTick(4)
    });
    assert_eq!(failure, Err(Error::Binding.into())); assert_eq!(root.bytes(), fork);
    assert_eq!(fs::read(&pending).unwrap(), b"do not clean yet");
    let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
    assert!(disk.stop.is_none()); assert_eq!(disk.control.ledger.charged, 16); assert_eq!(disk.executions, 0);
    fs::write(root.store().join(storage::CANONICAL), &before).unwrap();
    let history = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        &anchor, || ElapsedTick(4)).unwrap();
    assert!(history.stopped().progress().drained()); exact_anchor(&root, &history); assert!(!pending.exists());
}

#[test]
fn expired_unknown_effects_remain_charged_in_every_successor_anchor() {
    let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
    ready.dispatch(); let stop = request(&ready.host);
    let prefix = ready.host.history_anchor().unwrap(); drop(ready);
    let history = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        &prefix, || ElapsedTick(1001)).unwrap();
    let FileStoppedRecovery::Advanced { sweep, .. } = history.stopped() else { panic!("advancing stop"); };
    assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::RetentionExpired));
    assert!(!sweep.progress.drained()); assert_eq!(sweep.progress.unresolved, vec![1]);
    assert_eq!(sweep.progress.irrecoverable, vec![1]); assert_eq!(sweep.progress.charged_units, 16);
    exact_anchor(&root, &history);
    let clocks = Cell::new(0);
    let retry = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        history.history_anchor(), || { clocks.set(clocks.get() + 1); ElapsedTick(1002) }).unwrap();
    assert_eq!(clocks.get(), 1); assert!(!retry.stopped().progress().drained());
    assert_eq!(retry.stopped().progress().charged_units, 16); assert_eq!(retry.stopped().snapshot().executions, 0);
    assert_eq!(retry.history_anchor().revision(), history.history_anchor().revision() + 3);
    exact_anchor(&root, &retry);
}

#[test]
fn exact_stop_profile_and_exclusive_owner_are_required_before_any_anchor_can_escape() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let stop = request(&ready.host); let anchor = ready.host.history_anchor().unwrap();
    assert!(matches!(FileOversight::recover_stopped_anchored(root.store(), profile(), stop, &anchor,
        || panic!("busy owner cannot yield custody")), Err(JournalError::Busy)));
    drop(ready); let before = root.bytes(); let pending = root.store().join("delivery.pending");
    fs::write(&pending, b"retain on refusal").unwrap();
    let mut wrong = stop; wrong.operation = 0;
    assert_eq!(FileOversight::recover_stopped_anchored(root.store(), profile(), wrong, &anchor,
        || panic!("zero operation")), Err(Error::InvalidInput.into()));
    wrong = stop; wrong.expected_control_sequence += 1;
    assert_eq!(FileOversight::recover_stopped_anchored(root.store(), profile(), wrong, &anchor,
        || panic!("stale predecessor")), Err(Error::Stale.into()));
    let mut foreign = profile(); foreign.delivery.total += 1;
    assert!(FileOversight::recover_stopped_anchored(root.store(), foreign, stop, &anchor,
        || panic!("foreign profile")).is_err());
    assert_eq!(root.bytes(), before); assert_eq!(fs::read(&pending).unwrap(), b"retain on refusal");
    let history = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        &anchor, || ElapsedTick(2)).unwrap();
    let completed = root.bytes(); fs::write(&pending, b"retain after completion").unwrap();
    assert_eq!(FileOversight::recover_stopped_anchored(root.store(), profile(), wrong,
        history.history_anchor(), || panic!("conflicting retained request")), Err(Error::Binding.into()));
    wrong = stop; wrong.operation += 1;
    assert_eq!(FileOversight::recover_stopped_anchored(root.store(), profile(), wrong,
        history.history_anchor(), || panic!("different retained operation")), Err(Error::Duplicate.into()));
    assert_eq!(root.bytes(), completed); assert_eq!(fs::read(&pending).unwrap(), b"retain after completion");
    let retry = FileOversight::recover_stopped_anchored(root.store(), profile(), stop,
        history.history_anchor(), || panic!("unchanged completion" )).unwrap();
    exact_anchor(&root, &retry); assert!(!pending.exists());
}
