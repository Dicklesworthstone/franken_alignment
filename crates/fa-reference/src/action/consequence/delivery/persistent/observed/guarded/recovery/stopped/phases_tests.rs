//! Compare original native transitions, not a second shutdown state machine.
use super::*;
use super::super::{BaseEvent, Event, FileOversightProfile, Machine, Transition, journal, storage};
use crate::action::consequence::delivery::persistent::{JournalIo, RecoveryReserve, MAX_JOURNAL_EVENTS};
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements};
use std::{cell::Cell, fs, panic::{AssertUnwindSafe, catch_unwind}};
#[allow(dead_code)]
#[path = "../reconciled/fixture.rs"]
mod fixture;
use fixture::*;

fn begin(root: &Directory, p: FileOversightProfile, expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor) -> FileGuardedRecovery
{
    FileOversight::begin_open_guarded_anchored(root.store(), p, expected, anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
}
fn ready(run: &mut FileGuardedRecovery, quantum: usize) {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let old = run.progress();
        let next = run.advance(old.replayed_events, quantum).unwrap();
        assert_eq!(next.replayed_events - old.replayed_events,
            quantum.min(old.total_events - old.replayed_events));
        assert_eq!(next.attempted_events, next.replayed_events);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
}
fn stopped(root: &Directory, mode: u64)
    -> (FileRecoveryRequirements, FileHistoryAnchor, StopRequest)
{
    let mut live = Ready::new(root);
    if mode > 0 { live.dispatch(); }
    if mode == 2 { live.publish(); }
    let expected = requirements(&live.host, guards());
    let anchor = live.host.history_anchor().unwrap();
    drop(live);
    let mut run = begin(root, profile(), &expected, &anchor); ready(&mut run, 1);
    let request = run.stop_request(900).unwrap();
    let history = run.finish_admission_stop(request).unwrap();
    (expected, history.history_anchor().clone(), request)
}

#[test]
fn phased_cuts_match_original_machine_bytes_for_all_liabilities_and_quanta() {
    for mode in 0..4 { for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
        let root = Directory::new(); let mut live = Ready::new(&root);
        if mode > 0 { live.dispatch(); }
        if mode == 2 { live.publish(); }
        let expected = requirements(&live.host, guards());
        let anchor = live.host.history_anchor().unwrap();
        let before = live.host.inspect(); let original = root.bytes();
        let request = StopRequest { operation: 900, expected_control_sequence: before.control.sequence,
            expected_authority_epoch: before.control.ledger.epoch };
        let tick = ElapsedTick(if mode == 3 { 2000 } else { 3 });
        let mut events = live.host.events.clone();
        let mut machine = Machine::replay(&profile(), &events).unwrap();
        for event in [BaseEvent::Stop(request), BaseEvent::Fence] {
            let event = Event::Core(event); machine.apply(&event).unwrap(); events.push(event);
        }
        let admission_bytes = journal::encode(&profile(), live.host.store.identity(), &events).unwrap();
        let admission_state = machine.snapshot(events.len());
        let drain = Event::Core(BaseEvent::StopProgress(tick));
        let Transition::StopProgressed(sweep) = machine.apply(&drain).unwrap() else { panic!("native drain"); };
        events.push(drain);
        let final_bytes = journal::encode(&profile(), live.host.store.identity(), &events).unwrap();
        let final_state = machine.snapshot(events.len()); drop(live);
        let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, quantum);
        assert_eq!(root.bytes(), original);
        let admission = run.finish_admission_stop(request).unwrap();
        assert_eq!(admission.snapshot(), &admission_state);
        assert_eq!(admission.appended_events(), 2); assert_eq!(root.bytes(), admission_bytes);
        assert_eq!(admission.progress().charged_units, if mode > 0 { 16 } else { 0 });
        let mut run = begin(&root, profile(), &expected, admission.history_anchor()); ready(&mut run, quantum);
        let result = run.finish_stopped_drain(request, || Ok(tick)).unwrap();
        assert_eq!(result.stopped(), &FileStoppedRecovery::Advanced { snapshot: final_state, sweep });
        assert_eq!(root.bytes(), final_bytes);
        assert_eq!(result.stopped().progress().drained(), mode != 3);
        assert_eq!(result.stopped().progress().charged_units, if mode >= 2 { 16 } else { 0 });
    } }
}

#[test]
fn original_three_event_reserve_and_full_capacity_retries_need_no_recovery_fence() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 40;
    let (mut host, roles) = FileOversight::create_guarded(root.store(), p.clone(), &guards(), None).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let mut live = Ready::from_owner(host, roles.human, 1, 101, ElapsedTick(1));
    live.dispatch(); live.publish();
    while live.host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
        live.host.observe_time(live.host.revision(), ElapsedTick(live.host.revision() + 3)).unwrap();
    }
    let expected = requirements(&live.host, guards()); let anchor = live.host.history_anchor().unwrap(); drop(live);
    let mut run = begin(&root, p.clone(), &expected, &anchor); ready(&mut run, 1);
    let request = run.stop_request(900).unwrap(); let admission = run.finish_admission_stop(request).unwrap();
    assert_eq!(admission.snapshot().revision, 39);
    let mut run = begin(&root, p.clone(), &expected, admission.history_anchor()); ready(&mut run, 1);
    let drain = run.finish_stopped_drain(request, || Ok(ElapsedTick(100))).unwrap();
    assert_eq!(drain.stopped().snapshot().revision, 40);
    let before = root.bytes();
    let mut run = begin(&root, p.clone(), &expected, drain.history_anchor()); ready(&mut run, 1);
    assert_eq!(run.finish_admission_stop(request).unwrap().appended_events(), 0);
    let mut run = begin(&root, p, &expected, drain.history_anchor()); ready(&mut run, 1);
    assert!(matches!(run.finish_stopped_drain(request, || panic!("complete retry")).unwrap().stopped(),
        FileStoppedRecovery::AlreadyDrained { .. }));
    assert_eq!(root.bytes(), before);
}

#[test]
fn full_independent_requirements_are_rechecked_at_both_terminal_entries() {
    for drain in [false, true] { for mismatch in 0..4 {
        let root = Directory::new(); let live = Ready::new(&root);
        let expected = requirements(&live.host, guards()); let anchor = live.host.history_anchor().unwrap(); drop(live);
        let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
        let request = run.stop_request(900).unwrap();
        if drain {
            let history = run.finish_admission_stop(request).unwrap();
            run = begin(&root, profile(), &expected, history.history_anchor()); ready(&mut run, 1);
        }
        fs::write(root.store().join("delivery.pending"), b"retain").unwrap(); let before = root.files();
        match mismatch {
            0 => run.expected.minimum.journal_revision = u64::MAX,
            1 => run.expected.minimum.control_sequence = u64::MAX,
            2 => run.expected.minimum.authority_epoch = u64::MAX,
            _ => run.expected.effective_policy = crate::action::consequence::gate::containment::session::policy::Policy::new(
                2, vec![crate::action::consequence::gate::containment::session::policy::Predicate::PayloadAtMost(1)]).unwrap(),
        }
        if drain { assert!(run.finish_stopped_drain(request, || panic!("mismatch clock")).is_err()); }
        else { assert!(run.finish_admission_stop(request).is_err()); }
        assert_eq!(root.files(), before);
    } }
}

#[test]
fn incomplete_failed_and_interrupted_replays_cannot_enter_either_phase() {
    for state in 0..3 { for drain in [false, true] {
        let root = Directory::new(); let live = Ready::new(&root);
        let expected = requirements(&live.host, guards()); let anchor = live.host.history_anchor().unwrap();
        let before = live.host.inspect(); let request = StopRequest { operation: 900,
            expected_control_sequence: before.control.sequence, expected_authority_epoch: before.control.ledger.epoch };
        drop(live); let before = root.files();
        let mut run = begin(&root, profile(), &expected, &anchor);
        match state {
            1 => { assert!(run.advance_with(0, 1, |_, _| Err(Error::Binding)).is_err()); }
            2 => { assert!(catch_unwind(AssertUnwindSafe(|| run.interrupt_after_original_event())).is_err()); }
            _ => {}
        }
        if drain { assert!(run.finish_stopped_drain(request, || panic!("unverified clock")).is_err()); }
        else { assert!(run.finish_admission_stop(request).is_err()); }
        assert_eq!(root.files(), before);
    } }
    let root = Directory::new(); let live = Ready::new(&root);
    let expected = requirements(&live.host, guards()); let anchor = live.host.history_anchor().unwrap(); drop(live);
    let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
    let request = run.stop_request(900).unwrap(); let before = root.bytes();
    assert_eq!(run.finish_stopped_drain(request, || panic!("active domain")), Err(Error::WrongState.into()));
    assert_eq!(root.bytes(), before);
}

#[test]
fn every_cancelled_cut_preserves_the_same_locked_history_and_staging() {
    for drain in [false, true] {
        let root = Directory::new(); let live = Ready::new(&root);
        let expected = requirements(&live.host, guards()); let mut anchor = live.host.history_anchor().unwrap(); drop(live);
        if drain {
            let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
            let request = run.stop_request(900).unwrap();
            anchor = run.finish_admission_stop(request).unwrap().history_anchor().clone();
        }
        let count = begin(&root, profile(), &expected, &anchor).progress().total_events;
        fs::write(root.store().join("delivery.pending"), b"retain").unwrap(); let before = root.files();
        for cut in 0..=count {
            let mut run = begin(&root, profile(), &expected, &anchor);
            for _ in 0..cut { run.advance(run.progress().replayed_events, 1).unwrap(); }
            assert!(matches!(FileOversight::begin_open_guarded(root.store(), profile(), &expected), Err(JournalError::Busy)));
            run.cancel(); assert_eq!(root.files(), before);
        }
    }
}

#[test]
fn failed_clocks_and_changed_canonical_custody_never_undo_the_admission_stop() {
    for failure in 0..5 {
        let root = Directory::new(); let (expected, anchor, request) = stopped(&root, 1);
        fs::write(root.store().join("delivery.pending"), b"retain").unwrap(); let before = root.files();
        let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
        if failure == 3 { fs::write(root.store().join(storage::CANONICAL), b"changed").unwrap(); }
        let calls = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| run.finish_stopped_drain(request, || {
            calls.set(calls.get() + 1);
            match failure {
                0 => Err(JournalError::Unavailable), 1 => Ok(ElapsedTick(0)),
                2 => panic!("clock unavailable"),
                _ => { fs::write(root.store().join(storage::CANONICAL), b"changed").unwrap(); Ok(ElapsedTick(3)) }
            }
        })));
        if failure == 2 { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        assert_eq!(calls.get(), usize::from(failure != 3));
        if failure < 3 { assert_eq!(root.files(), before); }
        else { assert_eq!(root.bytes(), b"changed"); }
        assert_eq!(fs::read(root.store().join("delivery.pending")).unwrap(), b"retain");
    }
}

#[test]
fn every_storage_barrier_retains_the_real_visibility_and_exact_retry_law() {
    for drain in [false, true] { for barrier in [JournalIo::Stage, JournalIo::Write,
        JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut live = Ready::new(&root); live.dispatch(); live.publish();
        let expected = requirements(&live.host, guards()); let mut anchor = live.host.history_anchor().unwrap(); drop(live);
        let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
        let request = run.stop_request(900).unwrap();
        if drain {
            anchor = run.finish_admission_stop(request).unwrap().history_anchor().clone();
            run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
        }
        let before = root.bytes(); run.store.fail_once(barrier);
        let error = if drain { run.finish_stopped_drain(request, || Ok(ElapsedTick(3))).unwrap_err() }
            else { run.finish_admission_stop(request).unwrap_err() };
        assert!(matches!(error, JournalError::Io(failure) if failure.operation == barrier));
        assert_eq!(root.bytes() != before, barrier == JournalIo::DirectorySync);
        let visible = root.bytes();
        let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
        if drain {
            let result = run.finish_stopped_drain(request, || Ok(ElapsedTick(3))).unwrap();
            assert!(result.stopped().progress().drained()); assert_eq!(result.stopped().progress().charged_units, 16);
        } else { assert_eq!(run.finish_admission_stop(request).unwrap().appended_events(),
            if barrier == JournalIo::DirectorySync { 0 } else { 2 }); }
        if barrier == JournalIo::DirectorySync { assert_eq!(root.bytes(), visible); }
    } }
}

#[test]
fn independent_anchor_rejects_valid_equal_counter_forks_and_invalid_suffixes() {
    let root = Directory::new(); let mut live = Ready::new(&root);
    live.host.observe_time(live.host.revision(), ElapsedTick(3)).unwrap();
    let expected = requirements(&live.host, guards()); let anchor = live.host.history_anchor().unwrap();
    let mut fork = live.host.events.clone();
    *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(4)));
    Machine::replay(&profile(), &fork).unwrap();
    let fork = journal::encode(&profile(), live.host.store.identity(), &fork).unwrap();
    let original = root.bytes(); let mut suffix = live.host.events.clone();
    suffix.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    let invalid = journal::encode(&profile(), live.host.store.identity(), &suffix).unwrap(); drop(live);
    fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
    fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
    assert!(FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).is_err());
    assert_eq!(root.bytes(), fork);
    fs::write(root.store().join(storage::CANONICAL), &invalid).unwrap();
    let mut run = begin(&root, profile(), &expected, &anchor);
    assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err());
    let request = StopRequest { operation: 900, expected_control_sequence: 0, expected_authority_epoch: 0 };
    assert!(run.finish_admission_stop(request).is_err()); assert_eq!(root.bytes(), invalid);
    assert_eq!(fs::read(root.store().join("delivery.pending")).unwrap(), b"retain");
    fs::write(root.store().join(storage::CANONICAL), original).unwrap();
    let mut run = begin(&root, profile(), &expected, &anchor); ready(&mut run, 1);
    let request = run.stop_request(900).unwrap(); assert!(run.finish_admission_stop(request).is_ok());
}
