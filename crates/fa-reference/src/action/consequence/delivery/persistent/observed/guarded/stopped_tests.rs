//! Direct terminal completion of actual original guarded journal histories.
use super::*;
use super::super::recovery::{FileGuardedRecovery, FileGuardedRecoveryStatus as Status};
use super::super::anchored::FileHistoryAnchor;
use crate::action::consequence::delivery::{StopRequest, persistent::{RecoveryReserve,
    observed::shutdown::recovery::FileStoppedRecovery}};
use std::{cell::Cell, fs};

fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(root.store()).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file()).map(|path| {
            let bytes = fs::read(&path).unwrap(); (path, bytes)
        }).collect()
}
fn replay(run: &mut FileGuardedRecovery, quantum: usize) {
    for _ in 0..=run.progress().total_events {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
        if after.status == Status::Ready { return; }
        assert_eq!(after.status, Status::Replaying);
        assert!(after.replayed_events > before.replayed_events);
    }
    panic!("bounded original replay did not complete");
}
fn request(host: &FileOversight) -> StopRequest {
    let control = host.inspect().control;
    StopRequest { operation: 900, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }
}
fn open(root: &Directory, expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor)
    -> FileGuardedRecovery
{
    FileOversight::begin_open_guarded_anchored(root.store(), profile(), expected, anchor).unwrap()
}

#[test]
fn direct_guarded_stop_matches_original_batch_semantics_and_retains_identical_drained_history() {
    for anchored in [false, true] {
        for quantum in [1, 3, 1024] {
            let root = Directory::new(); let (host, expected) = setup(&root);
            let stop = request(&host); let anchor = host.history_anchor().unwrap();
            let mut events = host.events.clone();
            events.extend([Event::Core(BaseEvent::Stop(stop)), Event::Core(BaseEvent::Fence),
                Event::Core(BaseEvent::StopProgress(ElapsedTick(2)))]);
            let oracle = Machine::replay(&host.profile, &events).unwrap();
            let snapshot = oracle.snapshot(events.len()); let progress = oracle.broker.stop_progress().unwrap();
            let bytes = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
            let before = files(&root); drop(host);
            let mut run = if anchored { open(&root, &expected, &anchor) }
                else { FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap() };
            replay(&mut run, quantum); assert_eq!(files(&root), before);
            assert_eq!(run.stop_request(900).unwrap(), stop);
            let calls = Cell::new(0);
            let history = run.finish_stopped_with_anchor(stop, || {
                calls.set(calls.get() + 1); ElapsedTick(2)
            }).unwrap();
            assert_eq!(calls.get(), 1); assert_eq!(history.stopped().snapshot(), &snapshot);
            assert_eq!(history.stopped().progress(), &progress);
            assert_eq!(fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
            assert_eq!(history.history_anchor(), &FileHistoryAnchor::from_acknowledged(events.len(), bytes));
            let completed = files(&root);
            let mut retry = open(&root, &expected, history.history_anchor()); replay(&mut retry, 1);
            assert_eq!(retry.stop_request(900).unwrap(), stop);
            let again = retry.finish_stopped_with_anchor(stop, || panic!("drained retry has no clock")).unwrap();
            assert!(matches!(again.stopped(), FileStoppedRecovery::AlreadyDrained { .. }));
            assert_eq!(again.history_anchor(), history.history_anchor());
            assert_eq!(again.stopped().snapshot(), &snapshot); assert_eq!(files(&root), completed);
            let mut retry = open(&root, &expected, history.history_anchor()); replay(&mut retry, 1);
            let legacy = retry.finish_stopped(stop, || panic!("receipt-only retry has no clock")).unwrap();
            assert_eq!(legacy.snapshot(), &snapshot); assert_eq!(files(&root), completed);
        }
    }
}

#[test]
fn every_incomplete_or_cancelled_cut_preserves_staging_and_cannot_finish_stopped() {
    let root = Directory::new(); let (host, expected) = setup(&root);
    let stop = request(&host); let anchor = host.history_anchor().unwrap(); let count = host.events.len(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"preserve incomplete work").unwrap();
    let before = files(&root);
    for cut in 0..=count {
        let mut run = open(&root, &expected, &anchor);
        for position in 0..cut { run.advance(position, 1).unwrap(); }
        if cut < count {
            assert_eq!(run.stop_request(900), Err(Error::Incomplete.into()));
            assert_eq!(run.finish_stopped_with_anchor(stop, || panic!("incomplete replay has no clock")),
                Err(Error::Incomplete.into()));
        } else {
            assert_eq!(run.stop_request(900).unwrap(), stop);
            assert_eq!(run.cancel().status, Status::Cancelled);
        }
        assert_eq!(files(&root), before);
    }
    let mut run = open(&root, &expected, &anchor); replay(&mut run, 1);
    assert!(run.finish_stopped(stop, || ElapsedTick(2)).unwrap().progress().drained());
    assert!(!root.store().join("delivery.pending").exists());
}

#[test]
fn guard_policy_and_all_independent_floors_remain_mandatory_on_terminal_completion() {
    let root = Directory::new(); let (host, expected) = setup(&root);
    let stop = request(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"keep on failed admission").unwrap();
    let before = files(&root);
    for case in 0..7 {
        let mut wrong = expected.clone();
        let error = match case {
            0 => { wrong.guards.identity = None; Error::Binding }
            1 => { wrong.guards.campaigns = None; Error::Binding }
            2 => { wrong.effective_policy = Policy::new(1, vec![Predicate::PayloadAtMost(127)]).unwrap(); Error::Binding }
            3 => { wrong.minimum.journal_revision = u64::MAX; Error::Stale }
            4 => { wrong.minimum.control_sequence = u64::MAX; Error::Stale }
            5 => { wrong.minimum.authority_epoch = u64::MAX; Error::Stale }
            _ => { wrong.credential_epoch = Some(FileCredentialEpoch { generation: 1, revoked: false }); Error::InvalidInput }
        };
        let mut run = open(&root, &wrong, &anchor);
        assert_eq!(run.advance(0, 1024).err(), Some(error.into()));
        assert_eq!(run.stop_request(900), Err(error.into()));
        assert_eq!(run.finish_stopped_with_anchor(stop, || panic!("wrong guard never reads time")), Err(error.into()));
        assert_eq!(files(&root), before);
    }
    let mut run = open(&root, &expected, &anchor); replay(&mut run, 1);
    assert!(run.finish_stopped(stop, || ElapsedTick(2)).unwrap().progress().drained());
}

#[test]
fn interrupted_original_replay_never_returns_a_stop_intent_receipt_or_anchor() {
    let root = Directory::new(); let (host, expected) = setup(&root);
    let stop = request(&host); let anchor = host.history_anchor().unwrap(); let count = host.events.len(); drop(host);
    let before = files(&root);
    for cut in 0..count {
        let mut run = open(&root, &expected, &anchor);
        for position in 0..cut { run.advance(position, 1).unwrap(); }
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.interrupt_after_original_event())).is_err());
        assert_eq!(run.progress().status, Status::Interrupted);
        assert_eq!(run.stop_request(900), Err(JournalError::Unavailable));
        assert_eq!(run.finish_stopped_with_anchor(stop, || panic!("interrupted replay cannot finish")), Err(JournalError::Unavailable));
        assert_eq!(files(&root), before);
    }
    let mut run = open(&root, &expected, &anchor); replay(&mut run, 1);
    assert!(run.finish_stopped(stop, || ElapsedTick(2)).unwrap().progress().drained());
}

#[test]
fn direct_stop_uses_the_reserved_tail_without_an_ordinary_recovery_fence() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 7;
    let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None };
    let (mut host, _) = FileOversight::create_guarded(root.store(), p.clone(), &guards, None).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let mut tick = 1;
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); tick += 1;
    }
    let stop = request(&host); let anchor = host.history_anchor().unwrap();
    let expected = FileRecoveryRequirements { guards, effective_policy: p.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: stop.expected_control_sequence,
            authority_epoch: stop.expected_authority_epoch } };
    assert_eq!(host.revision(), 4); drop(host);
    let mut run = FileOversight::begin_open_guarded_anchored(root.store(), p.clone(), &expected, &anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    replay(&mut run, 1);
    let history = run.finish_stopped_with_anchor(stop, || ElapsedTick(tick)).unwrap();
    assert_eq!(history.stopped().snapshot().revision, 7); assert!(history.stopped().progress().drained());
    let completed = files(&root);
    let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(JournalIo::Stage);
    let mut run = FileGuardedRecovery::begin_store(store, p, &expected, Some(history.history_anchor())).unwrap();
    replay(&mut run, 1);
    let retry = run.finish_stopped_with_anchor(stop, || panic!("full drained retry reads no clock")).unwrap();
    assert_eq!(retry.history_anchor(), history.history_anchor()); assert_eq!(files(&root), completed);
}

#[test]
fn native_storage_failures_return_no_candidate_and_retry_only_the_visible_original_cut() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (host, expected) = setup(&root);
        let stop = request(&host); let anchor = host.history_anchor().unwrap(); let before = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let mut run = FileGuardedRecovery::begin_store(store, profile(), &expected, Some(&anchor)).unwrap();
        replay(&mut run, 1);
        let JournalError::Io(failure) = run.finish_stopped_with_anchor(stop, || ElapsedTick(2)).unwrap_err()
            else { panic!("selected native Store barrier"); };
        assert_eq!(failure.operation, barrier);
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().revision,
            before + if visible { 3 } else { 0 });
        let mut retry = open(&root, &expected, &anchor); replay(&mut retry, 1);
        let history = retry.finish_stopped_with_anchor(stop, || {
            assert!(!visible, "visible completion must not stop again"); ElapsedTick(3)
        }).unwrap();
        assert_eq!(matches!(history.stopped(), FileStoppedRecovery::AlreadyDrained { .. }), visible);
        assert_eq!(history.stopped().snapshot().revision, before + 3);
        assert_eq!(history.history_anchor(), &FileHistoryAnchor::from_acknowledged((before + 3) as usize,
            fs::read(root.store().join(storage::CANONICAL)).unwrap()));
    }
}

#[test]
fn stale_intent_time_and_canonical_replacement_never_clean_or_overwrite_the_original_cut() {
    let root = Directory::new(); let (mut host, expected) = setup(&root);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let stop = request(&host); let anchor = host.history_anchor().unwrap();
    let mut fork = host.events.clone(); *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    let fork_state = Machine::replay(&host.profile, &fork).unwrap();
    expected.check(&host.profile, &fork_state, &fork).unwrap();
    let fork_bytes = journal::encode(&host.profile, host.store.identity(), &fork).unwrap();
    let path = root.store().join(storage::CANONICAL); let canonical = fs::read(&path).unwrap(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"retain on refusal").unwrap();
    let before = files(&root);
    let mut wrong = stop; wrong.expected_control_sequence += 1;
    let mut run = open(&root, &expected, &anchor); replay(&mut run, 1);
    assert_eq!(run.finish_stopped_with_anchor(wrong, || panic!("stale intent cannot read time")), Err(Error::Stale.into()));
    assert_eq!(files(&root), before);
    let mut run = open(&root, &expected, &anchor); replay(&mut run, 1);
    assert_eq!(run.finish_stopped_with_anchor(stop, || ElapsedTick(0)), Err(Error::Stale.into()));
    assert_eq!(files(&root), before);
    for during_clock in [false, true] {
        let mut run = open(&root, &expected, &anchor); replay(&mut run, 1);
        if !during_clock { fs::write(&path, &fork_bytes).unwrap(); }
        let result = run.finish_stopped_with_anchor(stop, || {
            assert!(during_clock, "changed canonical cut must refuse before clock");
            fs::write(&path, &fork_bytes).unwrap(); ElapsedTick(4)
        });
        assert_eq!(result, Err(Error::Binding.into())); assert_eq!(fs::read(&path).unwrap(), fork_bytes);
        assert_eq!(fs::read(root.store().join("delivery.pending")).unwrap(), b"retain on refusal");
        fs::write(&path, &canonical).unwrap();
    }
    let mut run = open(&root, &expected, &anchor); replay(&mut run, 1);
    let history = run.finish_stopped_with_anchor(stop, || ElapsedTick(4)).unwrap();
    let completed = files(&root);
    for (request, error) in [(wrong, Error::Binding), (StopRequest { operation: 901, ..stop }, Error::Duplicate)] {
        let mut run = open(&root, &expected, history.history_anchor()); replay(&mut run, 1);
        assert_eq!(run.finish_stopped_with_anchor(request, || panic!("conflicting retained stop")), Err(error.into()));
        assert_eq!(files(&root), completed);
    }
}
