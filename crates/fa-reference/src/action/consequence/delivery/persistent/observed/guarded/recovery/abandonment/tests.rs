//! Exercise the sealed consumer against native events, exact history and Store
//! barriers. Synthetic reviewer inputs test enforcement, not helper accuracy.
use super::super::{Machine, storage};
use super::*;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileCredentialEpoch, FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
};
use crate::action::consequence::delivery::persistent::observed::{
    FileOversight, FileOversightProfile,
};
use crate::action::consequence::delivery::persistent::{
    JournalIo, MAX_JOURNAL_EVENTS, Reconciliation, RecoveryReserve,
};
use crate::action::{ActionState, ElapsedTick};
use std::{
    fs,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[allow(dead_code)]
#[path = "../reconciled/fixture.rs"]
mod fixture;
use fixture::*;

fn request(host: &FileOversight) -> FileAbandonRequest {
    let state = host.inspect();
    FileAbandonRequest {
        operation: 800,
        attempt: 1,
        expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch,
    }
}

fn unknown(live: &mut Ready, mode: u64) {
    live.dispatch();
    assert_eq!(
        live.host.reconcile(live.host.revision(), 1).unwrap(),
        Reconciliation::AwaitingResolution
    );
    if mode > 0 {
        live.publish();
    }
    if mode > 1 {
        live.host.fence(live.host.revision()).unwrap();
        assert!(!live.host.clock_ready());
    }
    assert_eq!(
        live.host.inspect().control.ledger.stages[&1],
        ActionState::Unknown
    );
}

fn begin(
    root: &Directory,
    p: FileOversightProfile,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> FileGuardedRecovery {
    FileOversight::begin_open_guarded_anchored(root.store(), p, expected, anchor)
        .unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal())
        .unwrap()
}

fn ready(run: &mut FileGuardedRecovery, quantum: usize) {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let old = run.progress();
        let next = run.advance(old.replayed_events, quantum).unwrap();
        assert_eq!(
            next.replayed_events - old.replayed_events,
            quantum.min(old.total_events - old.replayed_events)
        );
        assert_eq!(next.attempted_events, next.replayed_events);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
}

#[test]
fn one_native_disposition_preserves_time_authority_payload_and_charge_for_every_replay_quantum() {
    for mode in 0..3 {
        let root = Directory::new();
        let mut live = Ready::new(&root);
        unknown(&mut live, mode);
        let before = live.host.inspect();
        let intent = request(&live.host);
        let expected = requirements(&live.host, guards());
        let anchor = live.host.history_anchor().unwrap();
        let original = root.bytes();
        let action = live.action.clone();
        let mut events = live.host.events.clone();
        let mut oracle = Machine::replay(&profile(), &events).unwrap();
        let event = Event::Core(BaseEvent::AbandonUnknown(intent));
        let Transition::Abandoned(receipt) = oracle.apply(&event).unwrap() else {
            panic!("native disposition");
        };
        events.push(event);
        let state = oracle.snapshot(events.len());
        let bytes = journal::encode(&profile(), live.host.store.identity(), &events).unwrap();
        let retained = FileHistoryAnchor::from_acknowledged(events.len(), bytes.clone());
        drop(live);
        for anchored in [false, true] {
            for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
                fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
                let mut run = if anchored {
                    begin(&root, profile(), &expected, &anchor)
                } else {
                    FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap()
                };
                ready(&mut run, quantum);
                assert_eq!(root.bytes(), original);
                let history = run.finish_abandoned_with_anchor(intent).unwrap();
                assert_eq!(history.receipt(), &receipt);
                assert_eq!(history.receipt().action(), &action);
                assert_eq!(history.receipt().request(), intent);
                assert_eq!(history.receipt().charged_units(), 16);
                assert_eq!(history.snapshot(), &state);
                assert_eq!(history.history_anchor(), &retained);
                assert_eq!(root.bytes(), bytes);
                assert_eq!(state.revision, before.revision + 1);
                assert_eq!(state.control.sequence, before.control.sequence);
                assert_eq!(state.control.ledger.epoch, before.control.ledger.epoch);
                assert_eq!(state.dispatcher_epoch, before.dispatcher_epoch);
                assert_eq!(state.control.ledger.elapsed, before.control.ledger.elapsed);
                assert_eq!(state.control.ledger.charged, before.control.ledger.charged);
                assert_eq!(
                    state.control.ledger.stages[&1],
                    ActionState::IrrecoverablyUnknown
                );
                assert_eq!(state.payload, before.payload);
                assert_eq!(state.executions, u64::from(mode > 0));
            }
        }
    }
}

#[test]
fn full_original_guards_are_rechecked_before_cleanup_even_after_ready() {
    let root = Directory::new();
    let mut live = Ready::new(&root);
    unknown(&mut live, 0);
    let intent = request(&live.host);
    let expected = requirements(&live.host, guards());
    let anchor = live.host.history_anchor().unwrap();
    drop(live);
    fs::write(
        root.store().join("delivery.pending"),
        b"retain diagnostic staging",
    )
    .unwrap();
    let before = root.files();
    for mismatch in 0..6 {
        let mut run = begin(&root, profile(), &expected, &anchor);
        ready(&mut run, 1);
        match mismatch {
            0 => run.expected.minimum.journal_revision = u64::MAX,
            1 => run.expected.minimum.control_sequence = u64::MAX,
            2 => run.expected.minimum.authority_epoch = u64::MAX,
            3 => run.expected.effective_policy = crate::action::consequence::gate::containment::session::policy::Policy::new(
                2, vec![crate::action::consequence::gate::containment::session::policy::Predicate::PayloadAtMost(1)]).unwrap(),
            4 => run.expected.credential_epoch = Some(FileCredentialEpoch { generation: 1, revoked: false }),
            _ => run.expected.guards.stream = Some(crate::action::consequence::delivery::stream::StreamProfile::new(
                1, 1, 2, 64, 128).unwrap()),
        }
        assert!(run.finish_abandoned_with_anchor(intent).is_err());
        assert_eq!(root.files(), before);
    }
    let mut run = begin(&root, profile(), &expected, &anchor);
    ready(&mut run, 1);
    assert!(run.finish_abandoned_with_anchor(intent).is_ok());
    assert!(!root.store().join("delivery.pending").exists());
}

#[test]
fn invalid_intents_and_original_nonunknown_stages_never_clean_staging() {
    for stage in 0..3 {
        let root = Directory::new();
        let mut live = Ready::new(&root);
        if stage > 0 {
            live.dispatch();
        }
        if stage > 1 {
            live.publish();
            live.host.reconcile(live.host.revision(), 1).unwrap();
        }
        let intent = request(&live.host);
        let expected = requirements(&live.host, guards());
        let anchor = live.host.history_anchor().unwrap();
        drop(live);
        fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
        let before = root.files();
        let mut run = begin(&root, profile(), &expected, &anchor);
        ready(&mut run, 1);
        assert_eq!(
            run.finish_abandoned_with_anchor(intent),
            Err(Error::WrongState.into())
        );
        assert_eq!(root.files(), before);
    }
    let root = Directory::new();
    let mut live = Ready::new(&root);
    unknown(&mut live, 0);
    let intent = request(&live.host);
    let expected = requirements(&live.host, guards());
    let anchor = live.host.history_anchor().unwrap();
    drop(live);
    fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
    let before = root.files();
    for mismatch in 0..5 {
        let mut run = begin(&root, profile(), &expected, &anchor);
        ready(&mut run, 1);
        let mut wrong = intent;
        match mismatch {
            0 => wrong.operation = 0,
            1 => wrong.attempt = 0,
            2 => wrong.attempt = 99,
            3 => wrong.expected_control_sequence += 1,
            _ => wrong.expected_authority_epoch += 1,
        }
        assert!(run.finish_abandoned_with_anchor(wrong).is_err());
        assert_eq!(root.files(), before);
    }
}

#[test]
fn every_partial_drop_and_unverified_finish_retains_canonical_and_staging() {
    let root = Directory::new();
    let mut live = Ready::new(&root);
    unknown(&mut live, 0);
    let intent = request(&live.host);
    let expected = requirements(&live.host, guards());
    let anchor = live.host.history_anchor().unwrap();
    let count = live.host.events.len();
    drop(live);
    fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
    let before = root.files();
    for cut in 0..=count {
        let mut run = begin(&root, profile(), &expected, &anchor);
        for position in 0..cut {
            run.advance(position, 1).unwrap();
        }
        assert!(matches!(
            FileOversight::begin_open_guarded(root.store(), profile(), &expected),
            Err(JournalError::Busy)
        ));
        drop(run);
        assert_eq!(root.files(), before);
        if cut < count {
            let mut run = begin(&root, profile(), &expected, &anchor);
            for position in 0..cut {
                run.advance(position, 1).unwrap();
            }
            assert_eq!(
                run.finish_abandoned_with_anchor(intent),
                Err(Error::Incomplete.into())
            );
            assert_eq!(root.files(), before);
        }
    }
    for interrupted in [false, true] {
        let mut run = begin(&root, profile(), &expected, &anchor);
        if interrupted {
            assert!(
                catch_unwind(AssertUnwindSafe(|| run.interrupt_after_original_event())).is_err()
            );
            assert_eq!(
                run.finish_abandoned_with_anchor(intent),
                Err(JournalError::Unavailable)
            );
        } else {
            assert!(run.advance_with(0, 1, |_, _| Err(Error::Binding)).is_err());
            assert_eq!(
                run.finish_abandoned_with_anchor(intent),
                Err(Error::Binding.into())
            );
        }
        assert_eq!(root.files(), before);
    }
}

#[test]
fn exact_anchor_canonical_custody_and_full_suffix_replay_remain_mandatory() {
    let root = Directory::new();
    let mut live = Ready::new(&root);
    unknown(&mut live, 0);
    live.host
        .observe_time(live.host.revision(), ElapsedTick(3))
        .unwrap();
    let intent = request(&live.host);
    let expected = requirements(&live.host, guards());
    let anchor = live.host.history_anchor().unwrap();
    let original = root.bytes();
    let mut events = live.host.events.clone();
    *events.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(4)));
    let fork_state = Machine::replay(&profile(), &events).unwrap();
    expected.check(&profile(), &fork_state, &events).unwrap();
    let fork = journal::encode(&profile(), live.host.store.identity(), &events).unwrap();
    let mut suffix = live.host.events.clone();
    suffix.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    let invalid = journal::encode(&profile(), live.host.store.identity(), &suffix).unwrap();
    drop(live);
    fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
    fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
    let before = root.files();
    assert_eq!(
        FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor)
            .err(),
        Some(Error::Binding.into())
    );
    assert_eq!(root.files(), before);
    fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    let mut run = begin(&root, profile(), &expected, &anchor);
    ready(&mut run, 1);
    fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
    let before = root.files();
    assert_eq!(
        run.finish_abandoned_with_anchor(intent),
        Err(Error::Binding.into())
    );
    assert_eq!(root.files(), before);
    fs::write(root.store().join(storage::CANONICAL), &invalid).unwrap();
    let before = root.files();
    let mut run = begin(&root, profile(), &expected, &anchor);
    assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err());
    assert!(run.finish_abandoned_with_anchor(intent).is_err());
    assert_eq!(root.files(), before);
}

#[test]
fn abandonment_cannot_spend_terminal_reserve_but_exact_retry_works_at_absolute_capacity() {
    for leave_one in [false, true] {
        let root = Directory::new();
        let mut p = profile();
        p.delivery.limits.events = 40;
        let (mut host, roles) =
            FileOversight::create_guarded(root.store(), p.clone(), &guards(), None).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
            .unwrap();
        let mut live = Ready::from_owner(host, roles.human, 1, 101, ElapsedTick(1));
        unknown(&mut live, 0);
        while live
            .host
            .journal_capacity()
            .unwrap()
            .ordinary_remaining()
            .events
            > usize::from(leave_one)
        {
            live.host
                .observe_time(live.host.revision(), ElapsedTick(live.host.revision() + 3))
                .unwrap();
        }
        let intent = request(&live.host);
        let expected = requirements(&live.host, guards());
        let anchor = live.host.history_anchor().unwrap();
        drop(live);
        let before = root.bytes();
        let mut run = begin(&root, p.clone(), &expected, &anchor);
        ready(&mut run, 1);
        if !leave_one {
            fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
            assert_eq!(
                run.finish_abandoned_with_anchor(intent),
                Err(Error::Limit.into())
            );
            assert_eq!(root.bytes(), before);
            assert_eq!(
                fs::read(root.store().join("delivery.pending")).unwrap(),
                b"retain"
            );
            continue;
        }
        let history = run.finish_abandoned_with_anchor(intent).unwrap();
        assert_eq!(history.snapshot().revision, 37);
        let mut run = begin(&root, p.clone(), &expected, history.history_anchor());
        ready(&mut run, 1);
        let stop = run.stop_request(900).unwrap();
        let stopped = run
            .finish_stopped_with_anchor(stop, || ElapsedTick(100))
            .unwrap();
        assert_eq!(stopped.stopped().snapshot().revision, 40);
        assert!(!stopped.stopped().progress().drained());
        assert_eq!(stopped.stopped().progress().irrecoverable, vec![1]);
        assert_eq!(stopped.stopped().progress().unresolved, vec![1]);
        let before = root.bytes();
        let mut run = begin(&root, p.clone(), &expected, stopped.history_anchor());
        ready(&mut run, 1);
        run.store.fail_once(JournalIo::Stage);
        let retried = run.finish_abandoned_with_anchor(intent).unwrap();
        assert_eq!(retried.receipt(), history.receipt());
        assert_eq!(retried.snapshot(), stopped.stopped().snapshot());
        assert_eq!(retried.history_anchor(), stopped.history_anchor());
        assert_eq!(root.bytes(), before);
        for conflict in 0..3 {
            fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
            let files = root.files();
            let mut run = begin(&root, p.clone(), &expected, stopped.history_anchor());
            ready(&mut run, 1);
            let mut changed = intent;
            match conflict {
                0 => changed.attempt = 2,
                1 => changed.expected_control_sequence += 1,
                _ => changed.expected_authority_epoch += 1,
            }
            assert_eq!(
                run.finish_abandoned_with_anchor(changed),
                Err(Error::Binding.into())
            );
            assert_eq!(root.files(), files);
        }
    }
}

#[test]
fn all_five_storage_barriers_return_no_receipt_and_recovery_obeys_actual_visibility() {
    for barrier in [
        JournalIo::Stage,
        JournalIo::Write,
        JournalIo::FileSync,
        JournalIo::Rename,
        JournalIo::DirectorySync,
    ] {
        let root = Directory::new();
        let mut live = Ready::new(&root);
        unknown(&mut live, 1);
        let intent = request(&live.host);
        let expected = requirements(&live.host, guards());
        let anchor = live.host.history_anchor().unwrap();
        let revision = live.host.revision();
        drop(live);
        let before = root.bytes();
        let mut run = begin(&root, profile(), &expected, &anchor);
        ready(&mut run, 1);
        run.store.fail_once(barrier);
        assert!(matches!(run.finish_abandoned_with_anchor(intent),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        let visible = root.bytes();
        assert_eq!(visible != before, barrier == JournalIo::DirectorySync);
        let mut run = begin(&root, profile(), &expected, &anchor);
        ready(&mut run, 1);
        assert_eq!(
            run.machine.snapshot(run.events.len()).revision,
            revision + u64::from(barrier == JournalIo::DirectorySync)
        );
        let history = run.finish_abandoned_with_anchor(intent).unwrap();
        assert_eq!(history.snapshot().revision, revision + 1);
        assert_eq!(history.snapshot().control.ledger.charged, 16);
        assert_eq!(history.snapshot().executions, 1);
        assert_eq!(history.receipt().request(), intent);
        if barrier == JournalIo::DirectorySync {
            assert_eq!(root.bytes(), visible);
        }
        let before = root.bytes();
        let mut run = begin(&root, profile(), &expected, history.history_anchor());
        ready(&mut run, 1);
        run.store.fail_once(barrier);
        let retry = run.finish_abandoned_with_anchor(intent).unwrap();
        assert_eq!(retry, history);
        assert_eq!(root.bytes(), before);
    }
}

mod mediated;
