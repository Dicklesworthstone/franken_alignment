//! Original native liabilities and two independent stores; no synthetic receipts.
use super::*;
use super::super::test_support::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
#[path = "../../../recovery/fixture.rs"]
mod effects;

type Clock = fn(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>;
fn binding() -> FileShutdownClockBinding { FileShutdownClockBinding { domain: 1, clock_domain: 99 } }
fn now(_: FileShutdownClockBinding) -> Result<ElapsedTick, JournalError> { Ok(ElapsedTick(2)) }
fn coordinator(root: &Directory, p: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), p, MAX_JOURNAL_BYTES).unwrap()
}

#[test]
fn admission_and_drain_match_original_events_without_erasing_effect_liabilities() {
    for mode in 0..4 { for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
        let root = effects::Directory::new(); let control = Directory::new();
        let mut live = effects::Ready::new(&root, effects::profile(), true);
        let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        if mode > 0 { live.dispatch(); }
        if mode == 2 { live.host.publish(live.host.revision(), 1).unwrap(); }
        let stop = effects::request(&live.host); let before = live.host.inspect();
        let identity = storage::identity(&root.store()).unwrap();
        let events = journal::decode(&effects::profile(), &identity, &root.bytes()).unwrap();
        let mut oracle = Machine::replay(&effects::profile(), &events).unwrap();
        oracle.apply(&Event::Core(BaseEvent::Stop(stop))).unwrap();
        oracle.apply(&Event::Core(BaseEvent::Fence)).unwrap();
        let expected = oracle.snapshot(events.len() + 2); drop(live);
        let mut c = coordinator(&control, &p);
        let stopped = c.recover_admission_stop(0, binding(), quantum, |_| true).unwrap().unwrap();
        assert_eq!(FileOversight::read_publication(root.store(), &effects::profile()).unwrap(), expected);
        assert_eq!(stopped.journal_revision, before.revision + 2);
        assert_eq!(stopped.stop.as_ref().unwrap().charged_units, if mode > 0 { 16 } else { 0 });
        assert_eq!(stopped.dispatches_since_registration, if mode > 0 { vec![1] } else { vec![] });
        assert!(stopped.last_drain.is_none());
        let needs_drain = !stopped.stop.as_ref().unwrap().drained();
        let mut calls = 0;
        let at = ElapsedTick(if mode == 3 { 1001 } else { 2 });
        let result = c.drain_stopped_with_clock(c.revision(), binding(), quantum,
            |actual| { assert_eq!(actual, binding()); calls += 1; Ok(at) }, |_| true).unwrap().unwrap();
        assert_eq!(calls, usize::from(needs_drain));
        assert_eq!(result.journal_revision, stopped.journal_revision + u64::from(needs_drain));
        if needs_drain { oracle.apply(&Event::Core(BaseEvent::StopProgress(at))).unwrap(); }
        assert_eq!(FileOversight::read_publication(root.store(), &effects::profile()).unwrap(),
            oracle.snapshot(result.journal_revision as usize));
        assert_eq!(result.executions, u64::from(mode == 2));
        assert_eq!(result.stop.as_ref().unwrap().charged_units, if mode >= 2 { 16 } else { 0 });
        assert_eq!(result.stop.as_ref().unwrap().drained(), mode != 3);
        let (mut passive, _, _) = c.campaign.read_canonical(0).unwrap();
        passive.source = result.source; assert_eq!(passive, result);
    }}
}

#[test]
fn two_then_one_events_fit_the_original_reserve_and_exact_retries_are_read_only() {
    let sizing = effects::Directory::new(); let mut live = effects::Ready::new(&sizing, effects::profile(), true);
    live.dispatch(); let count = live.host.revision() as usize; drop(live);
    let root = effects::Directory::new(); let control = Directory::new();
    let mut profile = effects::profile(); profile.delivery.limits.events = count + 3;
    let mut live = effects::Ready::new(&root, profile.clone(), true); live.dispatch();
    assert_eq!(live.host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
    drop(live); let mut c = coordinator(&control, &p);
    let stop = c.recover_admission_stop(0, binding(), 1, |_| true).unwrap().unwrap();
    assert_eq!(stop.journal_revision as usize, count + 2);
    let drained = c.drain_stopped_with_clock(c.revision(), binding(), 1, now, |_| true).unwrap().unwrap();
    assert_eq!(drained.journal_revision as usize, count + 3); assert!(drained.stop.as_ref().unwrap().drained());
    let saved = root.bytes();
    let stop = c.recover_admission_stop(c.revision(), binding(), 1, |_| true).unwrap().unwrap();
    assert_eq!(stop.source, FileShutdownSource::CanonicalImage);
    assert_eq!(stop.last_drain, drained.last_drain);
    c.drain_stopped_with_clock(c.revision(), binding(), 1, |_| panic!("no renewed time"), |_| true).unwrap().unwrap();
    assert_eq!(root.bytes(), saved);
}

#[test]
fn binding_quantum_active_drain_and_head_space_refuse_without_native_changes() {
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 64);
    let p = plan(&host, 8); drop(host); let original = root.image();
    let mut c = coordinator(&control, &p); let control_before = control.image();
    for (id, quantum, error) in [(FileShutdownClockBinding { domain: 2, ..binding() }, 1, Error::Missing),
        (FileShutdownClockBinding { clock_domain: 100, ..binding() }, 1, Error::Binding),
        (binding(), 0, Error::InvalidInput), (binding(), MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(c.recover_admission_stop(0, id, quantum, |_| panic!("bad admission")), Err(error.into()));
        assert_eq!(control.image(), control_before); assert_eq!(root.image(), original);
    }
    assert_eq!(c.drain_stopped_with_clock(0, binding(), 1, |_| panic!("active domain"), |_| true).unwrap(), Err(Error::WrongState.into()));
    assert_eq!(root.image(), original);
    c.recover_admission_stop(c.revision(), binding(), 1, |_| true).unwrap().unwrap();
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 64);
    let before = root.image();
    let p = FileShutdownPlan::new(900, vec![host.shutdown_domain(1).unwrap()], 2, before.len()).unwrap(); drop(host);
    let mut c = coordinator(&control, &p);
    assert_eq!(c.recover_admission_stop(0, binding(), 1, |_| true).unwrap(), Err(Error::Limit.into()));
    assert_eq!(root.image(), before);
}

#[test]
fn every_replay_cancellation_preserves_domain_and_pending_bytes_in_both_phases() {
    for drain in [false, true] {
        let sizing = effects::Directory::new(); let control = Directory::new();
        let mut live = effects::Ready::new(&sizing, effects::profile(), true); live.dispatch();
        let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap(); drop(live);
        let mut c = coordinator(&control, &p);
        if drain { c.recover_admission_stop(0, binding(), 1, |_| true).unwrap().unwrap(); }
        let count = FileOversight::read_publication(sizing.store(), &effects::profile()).unwrap().revision as usize;
        for cut in 0..=count {
            let root = effects::Directory::new(); let control = Directory::new();
            let mut live = effects::Ready::new(&root, effects::profile(), true); live.dispatch();
            let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap(); drop(live);
            let mut c = coordinator(&control, &p);
            if drain { c.recover_admission_stop(0, binding(), 1, |_| true).unwrap().unwrap(); }
            let before = root.bytes(); let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"keep").unwrap();
            let poll = |progress: FileStopRecoveryProgress| progress.replayed_events < cut;
            let result = if drain {
                c.drain_stopped_with_clock(c.revision(), binding(), 1, |_| panic!("cancelled"), poll)
            } else { c.recover_admission_stop(c.revision(), binding(), 1, poll) };
            assert_eq!(result.unwrap(), Err(JournalError::Unavailable));
            assert_eq!(root.bytes(), before); assert_eq!(std::fs::read(&pending).unwrap(), b"keep");
            assert!(!c.report().unavailable); assert!(!c.report().all_observed_drained());
        }
    }
}

#[test]
fn failed_or_panicking_clocks_leave_the_acknowledged_admission_stop_intact() {
    for unwind in [false, true] {
        let root = effects::Directory::new(); let control = Directory::new();
        let mut live = effects::Ready::new(&root, effects::profile(), true); live.dispatch();
        let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap(); drop(live);
        let mut c = coordinator(&control, &p); c.recover_admission_stop(0, binding(), 1, |_| true).unwrap().unwrap();
        let before = root.bytes();
        let result = catch_unwind(AssertUnwindSafe(|| c.drain_stopped_with_clock(c.revision(), binding(), 1,
            |_| { if unwind { panic!("clock interrupted"); } Err(JournalError::Unavailable) }, |_| true)));
        if unwind { assert!(result.is_err()); assert!(c.report().unavailable); }
        else { assert_eq!(result.unwrap().unwrap(), Err(JournalError::Unavailable)); }
        assert_eq!(root.bytes(), before); let revision = c.revision(); drop(c);
        let mut c = reopen(&control, &p, revision);
        if unwind {
            let seen = c.resolve_shutdown_visit(c.revision(), 1).unwrap();
            assert!(!seen.stop.unwrap().drained()); assert_eq!(root.bytes(), before);
        }
        c.drain_stopped_with_clock(c.revision(), binding(), 1, now, |_| true).unwrap().unwrap();
        assert!(c.report().all_observed_drained());
    }
}

#[test]
fn both_storage_acknowledgments_keep_uncertain_completion_separate_in_each_phase() {
    for drain in [false, true] { for native in [false, true] { for barrier in BARRIERS {
        let root = effects::Directory::new(); let control = Directory::new();
        let mut live = effects::Ready::new(&root, effects::profile(), true); live.dispatch();
        let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap(); drop(live);
        let mut c = coordinator(&control, &p);
        if drain { c.recover_admission_stop(0, binding(), 1, |_| true).unwrap().unwrap(); }
        let before = root.bytes();
        let kind = if drain { ShutdownVisitKind::DrainStoppedAtCompletion } else { ShutdownVisitKind::StopAdmission };
        let pair = c.begin(c.revision(), 1, kind).unwrap();
        let store = storage::Store::open(&root.store()).unwrap();
        if native { store.fail_once(barrier); } else { c.store.fail_once(barrier); }
        let result = c.recover_phase_locked(pair, store, 1, if drain { Some(now as Clock) } else { None }, |_| true);
        let result = c.finish(pair.0, pair.1, result);
        if native {
            assert!(matches!(result, Ok(Err(JournalError::Io(ref failure))) if failure.operation == barrier));
            assert_eq!(root.bytes() != before, barrier == JournalIo::DirectorySync);
        } else {
            assert!(matches!(result, Err(JournalError::Io(ref failure)) if failure.operation == barrier));
            assert!(c.report().unavailable); assert_ne!(root.bytes(), before);
            let floor = c.revision(); let saved = root.bytes(); drop(c); let mut c = reopen(&control, &p, floor);
            c.resolve_shutdown_visit(c.revision(), usize::from(drain)).unwrap();
            assert_eq!(root.bytes(), saved);
        }
    }}}
}

#[test]
fn changed_canonical_bytes_before_or_during_drain_time_are_never_overwritten() {
    for during_clock in [false, true] {
        let root = effects::Directory::new(); let control = Directory::new();
        let mut live = effects::Ready::new(&root, effects::profile(), true); live.dispatch();
        let p = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        let old = root.bytes(); drop(live); let mut c = coordinator(&control, &p);
        c.recover_admission_stop(0, binding(), 1, |_| true).unwrap().unwrap(); let stopped = root.bytes();
        let mut clocks = 0;
        let result = c.drain_stopped_with_clock(c.revision(), binding(), 1,
            |_| { clocks += 1; if during_clock { std::fs::write(root.store().join(storage::CANONICAL), &old).unwrap(); } Ok(ElapsedTick(2)) },
            |progress| { if !during_clock && progress.status == FileStopRecoveryStatus::Ready {
                std::fs::write(root.store().join(storage::CANONICAL), &old).unwrap(); } true });
        assert_eq!(result.unwrap(), Err(Error::Binding.into()));
        assert_eq!(clocks, usize::from(during_clock)); assert_eq!(root.bytes(), old);
        std::fs::write(root.store().join(storage::CANONICAL), &stopped).unwrap();
        c.drain_stopped_with_clock(c.revision(), binding(), 1, now, |_| true).unwrap().unwrap();
    }
}

#[test]
fn version_four_intents_are_anchored_and_resume_does_not_repeat_their_commands() {
    let root = Directory::new(); let control = Directory::new(); let host = create(&root, 4);
    let p = plan(&host, 1); drop(host); let mut c = coordinator(&control, &p);
    let initial = c.history_anchor().unwrap();
    c.recover_admission_stop(0, binding(), 1, |_| true).unwrap().unwrap();
    let anchor = c.history_anchor_after(&initial).unwrap(); let canonical = control.image(); let native = root.image();
    assert_eq!(&canonical[..8], b"FASHCOO\x04");
    for version in [1, 2, 3] {
        let mut old_reader = canonical.clone(); old_reader[7] = version;
        assert!(super::super::super::codec::decode(c.store.identity(), &p, &p.encode().unwrap(), MAX_JOURNAL_BYTES, 0, &old_reader).is_err());
    }
    c.visits[0].kind = ShutdownVisitKind::DrainStoppedAtCompletion;
    let fork = c.encoded(c.revision()).unwrap(); drop(c);
    std::fs::write(control.store().join(storage::CANONICAL), &fork).unwrap();
    assert!(FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 2, &anchor).is_err());
    std::fs::write(control.store().join(storage::CANONICAL), &canonical).unwrap();
    let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 2, &anchor).unwrap();
    c.resolve_shutdown_visit(2, 0).unwrap();
    assert_eq!(control.image(), canonical); assert_eq!(root.image(), native);
}
