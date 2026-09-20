use super::*;
#[path = "fixture.rs"]
mod fixture;
use fixture::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use std::cell::Cell;

#[test]
fn source_free_recovery_distinguishes_reserved_sent_and_executed_effects() {
    for mode in 0..3 {
        let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
        if mode > 0 { ready.dispatch(); }
        if mode == 2 { ready.host.publish(ready.host.revision(), 1).unwrap(); }
        let before = ready.host.inspect(); let stop = request(&ready.host); drop(ready);
        let called = Cell::new(0);
        let result = FileOversight::recover_stopped(root.store(), profile(), stop, || {
            called.set(called.get() + 1); ElapsedTick(2)
        }).unwrap();
        assert_eq!(called.get(), 1); assert!(matches!(result, FileStoppedRecovery::Advanced { .. }));
        assert!(result.progress().drained()); assert_eq!(result.progress().receipt.request(), stop);
        let after = result.snapshot();
        assert_eq!(after.revision, before.revision + 3);
        assert_eq!(after.executions, u64::from(mode == 2));
        assert_eq!(after.control.ledger.charged, if mode == 2 { 16 } else { 0 });
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(&FileOversight::read_publication(root.store(), &profile()).unwrap(), after);
    }
}

#[test]
fn completed_retry_at_full_capacity_does_not_call_clock_or_append_events() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 15;
    let ready = Ready::new(&root, p.clone(), true);
    assert_eq!(ready.host.revision(), 12);
    assert_eq!(ready.host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let stop = request(&ready.host); drop(ready);
    let first = FileOversight::recover_stopped(root.store(), p.clone(), stop, || ElapsedTick(2)).unwrap();
    assert_eq!(first.snapshot().revision, 15);
    let bytes = root.bytes();
    for _ in 0..3 {
        let repeated = FileOversight::recover_stopped(root.store(), p.clone(), stop, || panic!("completed retry must not read time")).unwrap();
        assert!(matches!(repeated, FileStoppedRecovery::AlreadyDrained { .. }));
        assert_eq!(repeated.snapshot(), first.snapshot()); assert_eq!(repeated.progress(), first.progress());
        assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn stopped_but_unfenced_state_still_runs_the_original_terminal_transaction() {
    let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
    ready.dispatch(); let stop = request(&ready.host);
    ready.host.request_stop(ready.host.revision(), stop).unwrap();
    assert!(!ready.host.stop_progress().unwrap().drained());
    let before = ready.host.inspect(); drop(ready);
    let recovered = FileOversight::recover_stopped(root.store(), profile(), stop, || ElapsedTick(2)).unwrap();
    assert!(matches!(recovered, FileStoppedRecovery::Advanced { .. }));
    assert_eq!(recovered.snapshot().revision, before.revision + 3);
    assert!(recovered.progress().drained()); assert_eq!(recovered.snapshot().executions, 0);
}

#[test]
fn expired_unknown_effects_are_never_refunded_or_reported_drained() {
    let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
    ready.dispatch(); let stop = request(&ready.host); drop(ready);
    let result = FileOversight::recover_stopped(root.store(), profile(), stop, || ElapsedTick(1001)).unwrap();
    let FileStoppedRecovery::Advanced { snapshot, sweep } = result else { panic!("first recovery"); };
    assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::RetentionExpired));
    assert!(!sweep.progress.drained()); assert_eq!(sweep.progress.unresolved, vec![1]);
    assert_eq!(sweep.progress.irrecoverable, vec![1]); assert_eq!(snapshot.control.ledger.charged, 16);
    let again = FileOversight::recover_stopped(root.store(), profile(), stop, || ElapsedTick(1002)).unwrap();
    assert!(!again.progress().drained()); assert_eq!(again.progress().charged_units, 16);
    assert_eq!(again.snapshot().executions, 0);
}

#[test]
fn exact_request_and_profile_are_required_even_for_a_completed_stop() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let stop = request(&ready.host);
    assert!(matches!(FileOversight::recover_stopped(root.store(), profile(), stop, || panic!("busy")), Err(JournalError::Busy)));
    drop(ready);
    let before = root.bytes(); let mut wrong = stop; wrong.expected_control_sequence += 1;
    assert_eq!(FileOversight::recover_stopped(root.store(), profile(), wrong, || panic!("stale")), Err(Error::Stale.into()));
    assert_eq!(root.bytes(), before);
    FileOversight::recover_stopped(root.store(), profile(), stop, || ElapsedTick(2)).unwrap();
    let stopped = root.bytes();
    assert_eq!(FileOversight::recover_stopped(root.store(), profile(), wrong, || panic!("binding")), Err(Error::Binding.into()));
    wrong = stop; wrong.operation += 1;
    assert_eq!(FileOversight::recover_stopped(root.store(), profile(), wrong, || panic!("operation")), Err(Error::Duplicate.into()));
    let mut foreign = profile(); foreign.delivery.total += 1;
    assert!(FileOversight::recover_stopped(root.store(), foreign, stop, || panic!("profile")).is_err());
    assert_eq!(root.bytes(), stopped);
}

#[test]
fn storage_failures_expose_no_candidate_and_retry_recognizes_visible_completion() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut ready = Ready::new(&root, profile(), true);
        ready.dispatch(); let before = ready.host.inspect(); let stop = request(&ready.host); drop(ready);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let error = recover_locked(store, profile(), stop, || ElapsedTick(2)).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("selected barrier"); };
        assert_eq!(failure.operation, barrier);
        let visible = barrier == JournalIo::DirectorySync;
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.revision, before.revision + if visible { 3 } else { 0 });
        let recovered = FileOversight::recover_stopped(root.store(), profile(), stop, || {
            assert!(!visible, "a completed replacement must not be applied again"); ElapsedTick(3)
        }).unwrap();
        assert_eq!(matches!(recovered, FileStoppedRecovery::AlreadyDrained { .. }), visible);
        assert!(recovered.progress().drained()); assert_eq!(recovered.snapshot().executions, 0);
        assert_eq!(recovered.snapshot().control.ledger.available, 100);
    }
}

#[test]
fn pending_bytes_never_replace_canonical_state_during_completed_retry() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let stale_live = root.bytes(); let stop = request(&ready.host); drop(ready);
    let first = FileOversight::recover_stopped(root.store(), profile(), stop, || ElapsedTick(2)).unwrap();
    let canonical = root.bytes(); let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, stale_live).unwrap();
    let mut wrong = stop; wrong.expected_authority_epoch += 1;
    assert!(FileOversight::recover_stopped(root.store(), profile(), wrong, || panic!("wrong stop")).is_err());
    assert!(pending.exists(), "rejected identity must precede cleanup");
    let again = FileOversight::recover_stopped(root.store(), profile(), stop, || panic!("no clock")).unwrap();
    assert!(!pending.exists()); assert_eq!(root.bytes(), canonical); assert_eq!(again.snapshot(), first.snapshot());
}

#[test]
fn failed_clock_or_insufficient_recovery_space_preserves_canonical_state() {
    let root = Directory::new(); let ready = Ready::new(&root, profile(), true);
    let stop = request(&ready.host); drop(ready); let before = root.bytes();
    assert_eq!(FileOversight::recover_stopped(root.store(), profile(), stop, || ElapsedTick(0)), Err(Error::Stale.into()));
    assert_eq!(root.bytes(), before);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        FileOversight::recover_stopped(root.store(), profile(), stop, || panic!("unavailable trusted clock"))
    }));
    assert!(result.is_err()); assert_eq!(root.bytes(), before);
    let recovered = FileOversight::recover_stopped(root.store(), profile(), stop, || ElapsedTick(2)).unwrap();
    assert!(recovered.progress().drained());
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 13;
    let ready = Ready::new(&root, p.clone(), false); let stop = request(&ready.host); drop(ready);
    let before = root.bytes();
    assert_eq!(FileOversight::recover_stopped(root.store(), p, stop, || ElapsedTick(2)), Err(Error::Limit.into()));
    assert_eq!(root.bytes(), before);
}
