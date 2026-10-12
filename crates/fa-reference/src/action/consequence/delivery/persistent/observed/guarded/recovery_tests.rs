//! Original guarded machine, actual files and original replacement fault seam.
use super::*;
use super::super::recovery::{FileGuardedRecovery, FileGuardedRecoveryStatus as Status};
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS, RecoveryReserve};

fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    std::fs::read_dir(root.store()).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file()).map(|path| {
            let bytes = std::fs::read(&path).unwrap(); (path, bytes)
        }).collect()
}
fn ready(run: &mut FileGuardedRecovery, quantum: usize) {
    for _ in 0..=run.progress().total_events {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
        if after.status == Status::Ready { return; }
        assert_eq!(after.status, Status::Replaying);
        assert!(after.replayed_events > before.replayed_events);
    }
    panic!("bounded original history did not complete");
}
fn no_extra_guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements { guards, effective_policy: host.profile.delivery.policy.clone(),
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch } }
}

#[test]
fn both_consumers_and_poll_schedules_match_the_original_complete_fence() {
    let root = Directory::new(); let (host, expected) = setup(&root);
    let original = files(&root); let anchor = host.history_anchor().unwrap();
    // Independent full-history consumer of the ORIGINAL machine/encoder, not
    // another loop over the new cursor or a regenerated expected snapshot.
    let mut oracle_events = host.events.clone();
    oracle_events.push(Event::Core(BaseEvent::Fence));
    let oracle = Machine::replay(&host.profile, &oracle_events).unwrap().snapshot(oracle_events.len());
    let encoded = journal::encode(&host.profile, host.store.identity(), &oracle_events).unwrap();
    let count = host.events.len(); drop(host);
    for anchored in [false, true] {
        for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
            std::fs::write(root.store().join(storage::CANONICAL),
                &original[&root.store().join(storage::CANONICAL)]).unwrap();
            let mut run = if anchored {
                FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor)
            } else { FileOversight::begin_open_guarded(root.store(), profile(), &expected) }.unwrap();
            assert_eq!(run.progress().attempted_events, 0);
            ready(&mut run, quantum);
            assert_eq!(run.progress().replayed_events, count);
            assert_eq!(files(&root), original);
            let progress = run.progress();
            assert_eq!(run.advance(count, quantum).unwrap(), progress);
            assert_eq!(FileOversight::open_guarded(root.store(), profile(), &expected).err(), Some(JournalError::Busy));
            let (mut recovered, roles) = run.finish().unwrap();
            assert_eq!(recovered.inspect(), oracle);
            assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), encoded);
            assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
            assert!(!recovered.clock_ready());
            recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
            assert_eq!(recovered.identity_status().unwrap(), IdentityStatus::Missing);
        }
        std::fs::write(root.store().join(storage::CANONICAL),
            &original[&root.store().join(storage::CANONICAL)]).unwrap();
        let (recovered, _) = if anchored {
            FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor)
        } else { FileOversight::open_guarded(root.store(), profile(), &expected) }.unwrap();
        assert_eq!(recovered.inspect(), oracle);
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), encoded);
    }
}

#[test]
fn poll_admission_and_premature_finish_never_change_files_or_retain_a_lock() {
    let root = Directory::new(); let (host, expected) = setup(&root); drop(host);
    let before = files(&root);
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap();
    let progress = run.progress();
    for (predecessor, quantum, error) in [(1, 1, Error::Stale), (0, 0, Error::InvalidInput),
        (0, MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(run.advance(predecessor, quantum).err(), Some(error.into()));
        assert_eq!(run.progress(), progress); assert_eq!(files(&root), before);
    }
    assert_eq!(run.finish().err(), Some(Error::Incomplete.into()));
    assert_eq!(files(&root), before);
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap();
    ready(&mut run, 1); assert!(run.finish().is_ok());
}

#[test]
fn cancellation_at_every_event_cut_preserves_real_unacknowledged_staging() {
    let root = Directory::new(); let (mut host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap(); let count = host.events.len();
    host.store.fail_once(JournalIo::FileSync);
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err()); drop(host);
    let before = files(&root);
    for cut in 0..=count {
        let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
        for position in 0..cut { run.advance(position, 1).unwrap(); }
        assert_eq!(run.progress().replayed_events, cut);
        assert_eq!(files(&root), before);
        assert_eq!(FileOversight::begin_open_guarded(root.store(), profile(), &expected).err(), Some(JournalError::Busy));
        if cut % 2 == 0 {
            let cancelled = run.cancel();
            assert_eq!(cancelled.status, Status::Cancelled);
            assert_eq!(cancelled.replayed_events, cut);
            assert_eq!(cancelled.attempted_events, cut);
        } else { drop(run); }
        assert_eq!(files(&root), before);
    }
    let (recovered, _) = FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
    // Only the canonical original cut plus its fence, never the staged Time(2).
    assert_eq!(recovered.revision(), count as u64 + 1);
    assert!(!recovered.clock_ready());
    assert_eq!(files(&root).len() + 1, before.len());
}

#[test]
fn independent_guard_policy_and_floor_failures_are_terminal() {
    let root = Directory::new(); let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap(); drop(host); let before = files(&root);
    for case in 0..6 {
        let mut wrong = expected.clone();
        let error = match case {
            0 => { wrong.guards.identity = None; Error::Binding }
            1 => { wrong.effective_policy = Policy::new(1, vec![Predicate::PayloadAtMost(127)]).unwrap(); Error::Binding }
            2 => { wrong.minimum.journal_revision = u64::MAX; Error::Stale }
            3 => { wrong.minimum.control_sequence = u64::MAX; Error::Stale }
            4 => { wrong.minimum.authority_epoch = u64::MAX; Error::Stale }
            _ => { wrong.credential_epoch = Some(FileCredentialEpoch { generation: 1, revoked: false }); Error::InvalidInput }
        };
        let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &wrong, &anchor).unwrap();
        // Changing the caller's copy after begin cannot replace frozen requirements.
        wrong = expected.clone(); assert_eq!(wrong, expected);
        assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS).err(), Some(error.into()));
        assert_eq!(run.progress().status, Status::Failed(error));
        assert_eq!(run.advance(0, 1).err(), Some(error.into()));
        assert_eq!(run.finish().err(), Some(error.into())); assert_eq!(files(&root), before);
    }
    let (recovered, roles) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
    assert!(roles.identity_observer.is_some()); assert!(!recovered.clock_ready());
}

#[test]
fn matching_anchor_never_hides_a_semantically_invalid_suffix() {
    let root = Directory::new(); let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap(); let count = host.events.len();
    let mut events = host.events.clone();
    events.push(Event::Identity(super::super::super::identity::IdentityEvent::Apply(999, 0, 0)));
    let failure = Machine::replay(&host.profile, &events).err().unwrap();
    let bytes = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    drop(host); std::fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    let before = files(&root);
    let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
    for position in 0..count { run.advance(position, 1).unwrap(); }
    assert_eq!(run.advance(count, 1).err(), Some(failure.into()));
    let progress = run.progress();
    assert_eq!(progress.replayed_events, count); assert_eq!(progress.attempted_events, count + 1);
    assert_eq!(progress.status, Status::Failed(failure));
    assert_eq!(run.advance(count, MAX_JOURNAL_EVENTS).err(), Some(failure.into()));
    assert_eq!(run.cancel(), progress); assert_eq!(files(&root), before);
}

#[test]
fn valid_equal_counter_fork_refuses_before_replay_and_cut_replacement_refuses_before_cleanup() {
    let root = Directory::new(); let (mut host, expected) = setup(&root);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let anchor = host.history_anchor().unwrap(); let original = files(&root);
    let mut fork = host.events.clone();
    *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    let fork_machine = Machine::replay(&host.profile, &fork).unwrap();
    expected.check(&host.profile, &fork_machine, &fork).unwrap();
    let bytes = journal::encode(&host.profile, host.store.identity(), &fork).unwrap();
    assert_eq!(bytes.len(), original[&root.store().join(storage::CANONICAL)].len());
    drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    let before = files(&root);
    assert_eq!(FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).err(), Some(Error::Binding.into()));
    assert_eq!(files(&root), before);
    std::fs::write(root.store().join(storage::CANONICAL), &original[&root.store().join(storage::CANONICAL)]).unwrap();
    let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
    run.advance(0, 1).unwrap();
    // Deliberately noncooperating file mutation. The lock is advisory; final
    // byte comparison must not treat an equal-size replacement as the read cut.
    std::fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    ready(&mut run, 1); assert_eq!(run.finish().err(), Some(Error::Binding.into()));
    assert_eq!(files(&root), before);
    std::fs::write(root.store().join(storage::CANONICAL), &original[&root.store().join(storage::CANONICAL)]).unwrap();
    assert!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).is_ok());
}

#[test]
fn every_original_replacement_barrier_still_returns_no_owner_or_roles() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (host, expected) = setup(&root);
        let anchor = host.history_anchor().unwrap(); let revision = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let mut run = FileGuardedRecovery::begin_store(store, profile(), &expected, Some(&anchor)).unwrap();
        let before = files(&root); ready(&mut run, 1); assert_eq!(files(&root), before);
        let JournalError::Io(failure) = run.finish().unwrap_err() else { panic!("expected original storage failure"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.revision, revision + u64::from(barrier == JournalIo::DirectorySync));
        let (recovered, roles) = FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
        assert_eq!(recovered.revision(), disk.revision + 1);
        assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
        assert!(!recovered.clock_ready());
    }
}

#[test]
fn caught_unwinds_after_original_transitions_can_never_resume_the_private_candidate() {
    let root = Directory::new(); let (host, expected) = setup(&root); let count = host.events.len(); drop(host);
    let before = files(&root);
    for cut in 0..count {
        let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap();
        for position in 0..cut { run.advance(position, 1).unwrap(); }
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.interrupt_after_original_event())).is_err());
        assert_eq!(run.progress().status, Status::Interrupted);
        assert_eq!(run.progress().attempted_events, cut + 1);
        assert_eq!(run.progress().replayed_events, cut);
        assert_eq!(run.advance(cut, 1).err(), Some(JournalError::Unavailable));
        assert_eq!(run.finish().err(), Some(JournalError::Unavailable));
        assert_eq!(files(&root), before);
    }
    assert!(FileOversight::open_guarded(root.store(), profile(), &expected).is_ok());
}

#[test]
fn exact_original_reserve_survives_ordinary_exhaustion_and_cannot_be_selected_late() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 7;
    let reserve = RecoveryReserve::terminal();
    let (mut host, _) = FileOversight::create_guarded(root.store(), p.clone(), &no_extra_guards(), None).unwrap();
    host.enable_recovery_reserve(host.revision(), reserve).unwrap();
    let mut tick = 1;
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); tick += 1;
    }
    let expected = requirements(&host, no_extra_guards()); let anchor = host.history_anchor().unwrap();
    let capacity = host.journal_capacity().unwrap(); drop(host); let before = files(&root);
    let wrong = RecoveryReserve { events: reserve.events + 1, ..reserve };
    assert_eq!(FileOversight::begin_open_guarded(root.store(), p.clone(), &expected).unwrap()
        .require_recovery_reserve(wrong).err(), Some(Error::Binding.into()));
    let mut run = FileOversight::begin_open_guarded(root.store(), p.clone(), &expected).unwrap();
    run.advance(0, 1).unwrap();
    assert_eq!(run.require_recovery_reserve(reserve).err(), Some(Error::WrongState.into()));
    assert_eq!(files(&root), before);
    let mut run = FileOversight::begin_open_guarded_anchored(root.store(), p, &expected, &anchor).unwrap()
        .require_recovery_reserve(reserve).unwrap();
    ready(&mut run, 1); let (mut recovered, _) = run.finish().unwrap();
    let after = recovered.journal_capacity().unwrap();
    assert_eq!(after.reserve(), Some(reserve)); assert_eq!(after.ordinary_remaining().events, 0);
    assert_eq!(after.remaining().events + 1, capacity.remaining().events);
    assert_eq!(recovered.inspect().control.ledger.available, 100);
    assert!(!recovered.clock_ready());
    assert_eq!(recovered.observe_time(recovered.revision(), ElapsedTick(tick)).err(), Some(Error::Limit.into()));
    let other = Directory::new(); let (host, expected) = setup(&other); drop(host);
    assert_eq!(FileOversight::begin_open_guarded(other.store(), profile(), &expected).unwrap()
        .require_recovery_reserve(reserve).err(), Some(Error::Binding.into()));
}

#[test]
fn full_unreserved_history_refuses_before_cleanup_without_widening_limits() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 2;
    let (mut host, _) = FileOversight::create_guarded(root.store(), p.clone(), &no_extra_guards(), None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert_eq!(host.journal_capacity().unwrap().remaining().events, 0);
    let expected = requirements(&host, no_extra_guards()); drop(host);
    // Retained staged bytes are not permission to repair a full canonical cut.
    std::fs::write(root.store().join("delivery.pending"), b"unacknowledged staging evidence").unwrap();
    let before = files(&root);
    let mut run = FileOversight::begin_open_guarded(root.store(), p.clone(), &expected).unwrap();
    ready(&mut run, 1); assert_eq!(run.finish().err(), Some(Error::Limit.into()));
    assert_eq!(files(&root), before);
    assert_eq!(FileOversight::open_guarded(root.store(), p, &expected).err(), Some(Error::Limit.into()));
    assert_eq!(files(&root), before);
}

#[path = "recovery_effect_tests.rs"] mod effects;
