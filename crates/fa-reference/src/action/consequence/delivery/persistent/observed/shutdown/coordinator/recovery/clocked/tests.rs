//! Original journals, two-key effects and both persistence boundaries. Synthetic
//! clock observations test enforcement, not clock authenticity or hardware loss.
use super::*;
use super::super::test_support::*;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn binding() -> FileShutdownClockBinding { FileShutdownClockBinding { domain: 1, clock_domain: 99 } }
fn coordinator(root: &Directory, plan: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), plan, MAX_JOURNAL_BYTES).unwrap()
}
#[allow(dead_code)]
#[path = "../../../recovery/fixture.rs"]
mod effect_fixture;

#[test]
fn time_acquired_after_replay_controls_retention_and_preserves_unknown_charges() {
    for late in [false, true] {
        let root = effect_fixture::Directory::new(); let control = Directory::new();
        let mut live = effect_fixture::Ready::new(&root, effect_fixture::profile(), true);
        let plan = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 4, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        live.dispatch(); let native = live.host.profile.clone(); let count = live.host.revision(); drop(live);
        let mut c = coordinator(&control, &plan);
        let ready = Cell::new(false); let calls = Cell::new(0);
        let now = Cell::new(ElapsedTick(2));
        let result = c.recover_and_drain_with_clock(0, binding(), 1, |actual| {
            assert_eq!(actual, binding()); assert!(ready.get());
            calls.set(calls.get() + 1);
            assert!(matches!(storage::Store::open(&root.store()), Err(JournalError::Busy)));
            Ok(now.get())
        }, |progress| {
            assert_eq!(calls.get(), 0);
            if progress.status == FileStopRecoveryStatus::Ready {
                assert_eq!(progress.replayed_events as u64, count);
                ready.set(true);
                if late { now.set(ElapsedTick(1001)); }
            }
            true
        }).unwrap().unwrap();
        assert_eq!(calls.get(), 1); assert_eq!(result.executions, 0);
        let stop = result.stop.unwrap();
        assert_eq!(stop.drained(), !late); assert_eq!(stop.charged_units, if late { 16 } else { 0 });
        let disk = FileOversight::read_publication(root.store(), &native).unwrap();
        assert_eq!(disk.control.ledger.elapsed, Some(now.get()));
        assert_eq!(disk.control.ledger.charged, stop.charged_units);
    }
}

#[test]
fn drained_full_capacity_reread_and_retained_resolution_need_no_clock() {
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 5);
    let plan = plan(&host, 2); drop(host); let mut c = coordinator(&control, &plan);
    let first = c.recover_and_drain_with_clock(0, binding(), 1, |_| Ok(ElapsedTick(2)), |_| true).unwrap().unwrap();
    assert_eq!(first.journal_revision, 5); let bytes = root.image();
    let repeated = c.recover_and_drain_with_clock(c.revision(), binding(), 1,
        |_| panic!("completed native retry must not acquire time"), |_| true).unwrap().unwrap();
    assert_eq!(repeated.source, FileShutdownSource::CanonicalImage);
    assert_eq!(repeated.last_drain, first.last_drain); assert_eq!(repeated.stop, first.stop);
    assert_eq!(root.image(), bytes); let anchor = c.history_anchor().unwrap(); drop(c);
    let mut c = FileShutdownCoordinator::open_anchored(control.store(), &plan, MAX_JOURNAL_BYTES, 4, &anchor).unwrap();
    let coordinator_bytes = control.image();
    c.resolve_shutdown_visit(4, 1).unwrap();
    assert!(c.report().all_observed_drained()); assert_eq!(c.report().visits.len(), 2);
    assert_eq!(control.image(), coordinator_bytes); assert_eq!(root.image(), bytes);
}

#[test]
fn binding_and_quantum_admission_precede_intent_and_clock_errors_do_not_mutate_domains() {
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 32);
    let plan = plan(&host, 4); drop(host); let mut c = coordinator(&control, &plan);
    let bytes = control.image(); let native = root.image();
    for (id, quantum, error) in [(FileShutdownClockBinding { clock_domain: 7, ..binding() }, 1, Error::Binding),
        (FileShutdownClockBinding { domain: 9, ..binding() }, 1, Error::Missing),
        (binding(), 0, Error::InvalidInput), (binding(), MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(c.recover_and_drain_with_clock(0, id, quantum,
            |_| panic!("invalid admission"), |_| panic!("invalid admission")), Err(error.into()));
        assert_eq!(control.image(), bytes); assert_eq!(root.image(), native);
    }
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retained staging").unwrap();
    assert_eq!(c.recover_and_drain_with_clock(0, binding(), 1,
        |_| Err(JournalError::Unavailable), |_| true).unwrap(), Err(JournalError::Unavailable));
    assert!(!c.report().unavailable); assert_eq!(root.image(), native); assert!(pending.exists());
    assert_eq!(c.revision(), 2); assert!(!c.report().all_observed_drained());
    c.recover_and_drain_with_clock(2, binding(), 1, |_| Ok(ElapsedTick(2)), |_| true).unwrap().unwrap();
    assert!(c.report().all_observed_drained()); assert!(!pending.exists());
}

#[test]
fn cancellation_at_every_replay_cut_never_calls_time_or_changes_domain_bytes() {
    for cancel_at in 0..=7 {
        let root = Directory::new(); let control = Directory::new(); let mut host = create(&root, 32);
        for tick in 2..=6 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
        assert_eq!(host.revision(), 7); let plan = plan(&host, 2); drop(host);
        let native = root.image(); let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"not a receipt").unwrap(); let mut c = coordinator(&control, &plan);
        assert_eq!(c.recover_and_drain_with_clock(0, binding(), 1,
            |_| panic!("cancelled before native time"), |p| p.replayed_events != cancel_at).unwrap(),
            Err(JournalError::Unavailable));
        assert_eq!(root.image(), native); assert!(pending.exists()); assert!(!c.report().unavailable);
        assert_eq!(c.revision(), 2); assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Refused { .. }));
        c.recover_and_drain_with_clock(2, binding(), 1, |_| Ok(ElapsedTick(7)), |_| true).unwrap().unwrap();
        assert!(c.report().all_observed_drained());
    }
}

#[test]
fn changed_canonical_cut_refuses_before_or_after_time_without_cleanup() {
    for during_clock in [false, true] {
        let root = Directory::new(); let control = Directory::new(); let mut host = create(&root, 32);
        let old = root.image(); host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let plan = plan(&host, 2); drop(host); let mut c = coordinator(&control, &plan);
        let path = root.store().join(storage::CANONICAL); let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"preserve on mismatch").unwrap(); let called = Cell::new(0);
        let result = c.recover_and_drain_with_clock(0, binding(), 1, |_| {
            assert!(during_clock); called.set(called.get() + 1);
            std::fs::write(&path, &old).unwrap(); Ok(ElapsedTick(3))
        }, |p| {
            if p.status == FileStopRecoveryStatus::Ready && !during_clock { std::fs::write(&path, &old).unwrap(); }
            true
        }).unwrap();
        assert_eq!(result, Err(Error::Binding.into())); assert_eq!(called.get(), usize::from(during_clock));
        assert_eq!(root.image(), old); assert!(pending.exists()); assert!(!c.report().all_observed_drained());
    }
}

#[test]
fn intent_encoding_versions_and_external_anchors_preserve_original_visit_semantics() {
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 32);
    let plan = plan(&host, 4); drop(host); let mut c = coordinator(&control, &plan);
    assert_eq!(&control.image()[..8], b"FASHCOO\x01");
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(&control.image()[..8], b"FASHCOO\x02"); let prior = c.history_anchor().unwrap();
    c.recover_and_drain_with_clock(2, binding(), 1, |_| panic!("already drained"), |_| true).unwrap().unwrap();
    let bytes = control.image(); assert_eq!(&bytes[..8], b"FASHCOO\x03");
    assert_eq!(c.report().visits[0].kind, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) });
    assert_eq!(c.report().visits[1].kind, ShutdownVisitKind::RecoverStoppedAtCompletion);
    let next = c.history_anchor_after(&prior).unwrap(); drop(c);
    let mut downgraded = bytes.clone(); downgraded[7] = 2;
    std::fs::write(control.store().join(storage::CANONICAL), downgraded).unwrap();
    assert!(FileShutdownCoordinator::open(control.store(), &plan, MAX_JOURNAL_BYTES, 0).is_err());
    std::fs::write(control.store().join(storage::CANONICAL), &bytes).unwrap();
    let mut c = FileShutdownCoordinator::open_anchored(control.store(), &plan, MAX_JOURNAL_BYTES, 4, &next).unwrap();
    c.resolve_shutdown_visit(4, 1).unwrap(); assert_eq!(control.image(), bytes);
}

#[test]
fn domain_and_coordinator_barriers_remain_separate_and_visible_completion_is_not_repeated() {
    for native in [false, true] { for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new(); let host = create(&root, 5);
        let plan = plan(&host, 4); drop(host); let original = root.image(); let mut c = coordinator(&control, &plan);
        let pair = c.begin(0, 1, ShutdownVisitKind::RecoverStoppedAtCompletion).unwrap();
        let store = storage::Store::open(&root.store()).unwrap();
        if native { store.fail_once(barrier); }
        let result = c.recover_clocked_locked(pair, store, 1, |_| Ok(ElapsedTick(2)), |_| true);
        if !native { c.store.fail_once(barrier); }
        let result = c.finish(pair.0, pair.1, result);
        if native {
            assert!(matches!(result, Ok(Err(JournalError::Io(ref f))) if f.operation == barrier));
            assert!(!c.report().unavailable);
        } else {
            assert!(matches!(result, Err(JournalError::Io(ref f)) if f.operation == barrier));
            assert!(c.report().unavailable);
        }
        let completed = !native || barrier == JournalIo::DirectorySync;
        assert_eq!(root.image() != original, completed); let visible = root.image(); drop(c);
        let mut c = reopen(&control, &plan, 1);
        c.recover_and_drain_with_clock(c.revision(), binding(), 1, |_| {
            assert!(!completed, "visible native completion needs no second time acquisition");
            Ok(ElapsedTick(3))
        }, |_| true).unwrap().unwrap();
        if completed { assert_eq!(root.image(), visible); }
        assert!(c.report().all_observed_drained());
    }}
}

#[test]
fn clock_unwind_leaves_timestamp_free_intent_without_automatic_native_retry() {
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 32);
    let plan = plan(&host, 4); drop(host); let native = root.image(); let mut c = coordinator(&control, &plan);
    assert!(catch_unwind(AssertUnwindSafe(|| c.recover_and_drain_with_clock(0, binding(), 1,
        |_| panic!("trusted clock unavailable"), |_| true))).is_err());
    assert!(c.report().unavailable); assert_eq!(root.image(), native);
    assert_eq!(c.report().visits[0].kind, ShutdownVisitKind::RecoverStoppedAtCompletion);
    assert_eq!(c.report().visits[0].result, ShutdownVisitResult::Entered); drop(c);
    let mut c = reopen(&control, &plan, 1);
    assert_eq!(c.resolve_shutdown_visit(1, 0), Err(Error::Incomplete.into()));
    assert_eq!(root.image(), native); assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
    c.recover_and_drain_with_clock(1, binding(), 1, |_| Ok(ElapsedTick(2)), |_| true).unwrap().unwrap();
    assert!(c.report().all_observed_drained());
}
