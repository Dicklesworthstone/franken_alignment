#![cfg(unix)]

use fa_journal::batch::BatchLimits;
use fa_journal::file::{
    BatchCommitBudget, BatchCommitStage, BatchInspectionBudget, BatchInspectionStatus,
    Error as FileError, FileBatchCommit, FileBatchInspection, FileJournal,
};
use fa_journal::replay::{HistoryAnchor, ReplayBudget};
use fa_journal::{Error, Event, Identity, Journal, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fa-exact-batch-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.bin") }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}
fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 10 }
}
fn effect(payload: &[u8]) -> Effect {
    Effect { principal: "actor".into(), resolved_target: "publication".into(),
        payload: payload.to_vec(), units: 4 }
}
fn reserve(payload: &[u8]) -> Event { Event::Reserve { id: 1, effect: effect(payload) } }
fn events() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect(b"proposal") },
        Event::MarkUnknown { id: 1 }, Event::RevokeEpoch,
        Event::Reconcile { id: 1, occurred: true }]
}
fn poll_budget(quantum: u64) -> BatchInspectionBudget {
    BatchInspectionBudget { replay: ReplayBudget {
        read_calls: 1, read_bytes: quantum, checked_records: 1,
    }, prepare_records: 1, compare_calls: 1, compare_bytes: quantum }
}
fn inspect(run: &mut FileBatchInspection, quantum: u64) {
    for _ in 0..100_000 {
        if run.progress().status == BatchInspectionStatus::Ready { return; }
        let before = run.progress().total;
        let after = run.advance(poll_budget(quantum)).unwrap().total;
        assert!(after.replay.read_calls - before.replay.read_calls <= 1);
        assert!(after.replay.read_bytes - before.replay.read_bytes <= quantum);
        assert!(after.replay.checked_records - before.replay.checked_records <= 1);
        assert!(after.prepared_records - before.prepared_records <= 1);
        assert!(after.compare_calls - before.compare_calls <= 1);
        assert!(after.compare_bytes - before.compare_bytes <= quantum);
    }
    panic!("inspection failed to make bounded progress");
}
fn write_missing(mut run: FileBatchCommit, quantum: u64) -> FileJournal {
    assert_eq!(run.progress().preparation_attempts, 0);
    let before = run.progress();
    assert_eq!(run.advance(BatchCommitBudget::default()).unwrap(), before);
    for _ in 0..100_000 {
        if run.progress().stage == BatchCommitStage::Ready { return run.finish().unwrap(); }
        let before = run.progress();
        let after = run.advance(BatchCommitBudget {
            prepared_records: 0, write_calls: 1, write_bytes: quantum,
        }).unwrap();
        assert!(after.write_calls - before.write_calls <= 1);
        assert!(after.written_bytes - before.written_bytes <= quantum);
        assert_eq!(after.preparation_attempts, 0);
    }
    panic!("writer failed to make bounded progress");
}
struct Fixture {
    scratch: Scratch,
    owner: FileJournal,
    history: HistoryAnchor,
    oracle: Journal,
    complete_bytes: Vec<u8>,
    stored_bytes: u64,
}
fn fixture(reserved: bool, stored: usize) -> Fixture {
    let scratch = Scratch::new();
    let limits = if reserved { Limits { max_records: 5, ..Limits::default() } }
        else { Limits::default() };
    let reserve = RecoveryReserve { records: 3, bytes: 80 };
    let mut owner = if reserved {
        FileJournal::create_with_recovery_reserve(scratch.path(), identity(), limits, reserve)
    } else { FileJournal::create(scratch.path(), identity(), limits) }.unwrap();
    owner.commit(&self::reserve(b"proposal")).unwrap();
    let history = owner.history_anchor().unwrap();
    let mut oracle = if reserved {
        Journal::replay_with_recovery_reserve(history.as_bytes(), identity(), history.head(), limits, reserve)
    } else { Journal::replay(history.as_bytes(), identity(), history.head(), limits) }.unwrap();
    let mut complete_bytes = history.as_bytes().to_vec();
    let mut surviving = Vec::new();
    for (index, event) in events().iter().enumerate() {
        let record = oracle.append(event).unwrap();
        if index < stored { surviving.extend_from_slice(&record); }
        complete_bytes.extend(record);
    }
    drop(owner);
    // Actual file images at complete-record boundaries of an interrupted group.
    // This is a persistence-cut fixture, not a claim of a live power-loss test.
    OpenOptions::new().append(true).open(scratch.path()).unwrap()
        .write_all(&surviving).unwrap();
    let owner = if reserved {
        FileJournal::open_anchored_with_recovery_reserve(scratch.path(), identity(), history.clone(), limits, reserve)
    } else { FileJournal::open_anchored(scratch.path(), identity(), history.clone(), limits) }.unwrap();
    Fixture { scratch, owner, history, oracle, complete_bytes, stored_bytes: surviving.len() as u64 }
}

#[test]
fn every_surviving_boundary_resumes_exact_bytes_through_the_original_writer() {
    for reserved in [false, true] {
        for stored in 0..=events().len() {
            for quantum in [1, 7, 4096] {
                let f = fixture(reserved, stored);
                let mut run = f.owner.begin_resume_batch_anchored(
                    f.history.clone(), events(), BatchLimits::default(),
                ).unwrap();
                let head = run.acknowledged_head();
                inspect(&mut run, quantum);
                let work = run.progress().total;
                assert_eq!(work.replay.read_bytes, f.history.encoded_len());
                assert_eq!(work.replay.checked_records, f.history.head().sequence);
                assert_eq!(work.prepared_records, events().len() as u64);
                assert_eq!(work.compare_bytes, f.stored_bytes);
                let writer = run.resume().unwrap();
                assert_eq!(writer.acknowledged_head(), head);
                assert_eq!(writer.progress().total_records, events().len() - stored);
                assert_eq!(writer.progress().written_bytes, 0);
                let owner = write_missing(writer, quantum);
                assert_eq!(owner.acknowledged_head(), f.oracle.head());
                assert_eq!(owner.rights().unwrap(), f.oracle.rights());
                assert_eq!(owner.capacity().unwrap(), f.oracle.capacity());
                assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Committed);
                assert_eq!(owner.rights().unwrap().available(), 6);
                assert_eq!(owner.rights().unwrap().epoch(), 1);
                assert!(owner.rights().unwrap().conserved());
                assert_eq!(fs::read(f.scratch.path()).unwrap(), f.complete_bytes);
                if reserved { assert_eq!(owner.capacity().unwrap().recovery.records, 0); }
            }
        }
    }
}

#[test]
fn checksum_collision_fork_cannot_resume_even_when_the_original_suffix_matches() {
    let limits = Limits::default();
    let mut left = Journal::new(identity(), limits).unwrap();
    let mut right = Journal::new(identity(), limits).unwrap();
    let mut left_bytes = left.header();
    let mut right_bytes = right.header();
    left_bytes.extend(left.append(&reserve(b"plumless")).unwrap());
    right_bytes.extend(right.append(&reserve(b"buckeroo")).unwrap());
    assert_eq!(left.head(), right.head());
    assert_ne!(left.rights(), right.rights());
    let history = HistoryAnchor::from_bytes(left_bytes.clone(), identity(), limits).unwrap();
    let batch = vec![Event::RevokeEpoch];
    let original = fa_journal::file::BatchAnchor {
        identity: identity(), head: left.head(), encoded_len: left.encoded_len(),
    };
    for quantum in [1, 7, 4096] {
        let scratch = Scratch::new();
        fs::write(scratch.path(), &right_bytes).unwrap();
        let owner = FileJournal::open(scratch.path(), identity(), left.head(), limits).unwrap();
        let mut legacy = owner.begin_inspect_batch(original, batch.clone(), BatchLimits::default()).unwrap();
        inspect(&mut legacy, quantum);
        let (owner, status) = legacy.finish().unwrap();
        assert_eq!(status.persisted_records(), 0);
        let mut run = owner.begin_resume_batch_anchored(history.clone(), batch.clone(), BatchLimits::default()).unwrap();
        loop {
            match run.advance(poll_budget(quantum)) {
                Ok(report) => assert_eq!(report.status, BatchInspectionStatus::ReplayPrefix),
                Err(FileError::Journal(Error::Binding)) => break,
                other => panic!("unexpected fork result: {other:?}"),
            }
        }
        assert_eq!(run.progress().status, BatchInspectionStatus::Failed);
        assert!(run.progress().total.replay.read_bytes > left.header().len() as u64);
        assert_eq!(run.progress().total.prepared_records, 0);
        assert_eq!(run.progress().total.compare_calls, 0);
        assert!(matches!(run.resume(), Err(FileError::Poisoned)));
        assert_eq!(fs::read(scratch.path()).unwrap(), right_bytes);
        fs::write(scratch.path(), &left_bytes).unwrap();
        let owner = FileJournal::open_anchored(scratch.path(), identity(), history.clone(), limits).unwrap();
        let mut permitted = owner.begin_resume_batch_anchored(history.clone(), batch.clone(), BatchLimits::default()).unwrap();
        inspect(&mut permitted, quantum);
        let owner = write_missing(permitted.resume().unwrap(), quantum);
        assert_eq!(owner.rights().unwrap().epoch(), 1);
        assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Reserved);
    }
}

#[test]
fn read_only_inspection_preserves_unknown_liabilities_and_cannot_upgrade() {
    let f = fixture(true, 2);
    let bytes = fs::read(f.scratch.path()).unwrap();
    let expected = f.owner.rights().unwrap().clone();
    let capacity = f.owner.capacity().unwrap();
    let mut run = f.owner.begin_inspect_batch_anchored(f.history.clone(), events(), BatchLimits::default()).unwrap();
    inspect(&mut run, 1);
    let (owner, status) = run.finish().unwrap();
    assert_eq!(status.persisted_records(), 2);
    assert_eq!(status.remaining_records(), 2);
    assert!(!status.is_complete());
    assert_eq!(owner.rights().unwrap(), &expected);
    assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Unknown);
    assert_eq!(owner.rights().unwrap().available(), 6);
    assert_eq!(owner.capacity().unwrap(), capacity);
    let mut run = owner.begin_inspect_batch_anchored(f.history, events(), BatchLimits::default()).unwrap();
    inspect(&mut run, 7);
    assert!(matches!(run.resume(), Err(FileError::Journal(Error::InvalidInput))));
    assert_eq!(fs::read(f.scratch.path()).unwrap(), bytes);
}

#[test]
fn cancellation_at_each_inspection_stage_keeps_bytes_and_releases_the_lock() {
    for stage in [BatchInspectionStatus::ReplayPrefix, BatchInspectionStatus::PrepareBatch,
        BatchInspectionStatus::CompareSuffix, BatchInspectionStatus::Ready] {
        let f = fixture(false, 2);
        let bytes = fs::read(f.scratch.path()).unwrap();
        let mut run = f.owner.begin_resume_batch_anchored(f.history.clone(), events(), BatchLimits::default()).unwrap();
        while run.progress().status != stage { run.advance(poll_budget(1)).unwrap(); }
        assert!(matches!(FileJournal::open_anchored(f.scratch.path(), identity(), f.history.clone(), Limits::default()), Err(FileError::Busy)));
        let work = run.progress().total;
        assert_eq!(run.advance(BatchInspectionBudget::default()).unwrap().total, work);
        assert_eq!(run.cancel(), work);
        assert_eq!(fs::read(f.scratch.path()).unwrap(), bytes);
        let owner = FileJournal::open_anchored(f.scratch.path(), identity(), f.history, Limits::default()).unwrap();
        assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Unknown);
        assert_eq!(owner.rights().unwrap().available(), 6);
    }
}

#[test]
fn pre_group_limits_are_bound_before_file_io_and_cannot_be_widened() {
    let f = fixture(false, 0);
    let bytes = fs::read(f.scratch.path()).unwrap();
    let different = Limits { max_records: Limits::default().max_records - 1, ..Limits::default() };
    let history = HistoryAnchor::from_bytes(f.history.as_bytes().to_vec(), identity(), different).unwrap();
    // A later extent check would fail with Io. Binding must win before it.
    OpenOptions::new().write(true).open(f.scratch.path()).unwrap().set_len(0).unwrap();
    assert!(matches!(f.owner.begin_resume_batch_anchored(history, events(), BatchLimits::default()), Err(FileError::Journal(Error::Binding))));
    fs::write(f.scratch.path(), bytes).unwrap();
    let owner = FileJournal::open_anchored(f.scratch.path(), identity(), f.history.clone(), Limits::default()).unwrap();
    let mut run = owner.begin_resume_batch_anchored(f.history, events(), BatchLimits::default()).unwrap();
    inspect(&mut run, 7);
    assert_eq!(write_missing(run.resume().unwrap(), 7).rights().unwrap().state(1).unwrap(), State::Committed);
}

#[test]
fn the_whole_group_must_validate_even_when_its_stored_prefix_matches() {
    let f = fixture(false, 2);
    let bytes = fs::read(f.scratch.path()).unwrap();
    let mut bad = events();
    *bad.last_mut().unwrap() = Event::Reconcile { id: 99, occurred: true };
    let mut run = f.owner.begin_resume_batch_anchored(f.history.clone(), bad, BatchLimits::default()).unwrap();
    loop {
        match run.advance(poll_budget(7)) {
            Ok(report) => assert_ne!(report.status, BatchInspectionStatus::Ready),
            Err(FileError::Journal(Error::Model(fa_reference::Error::Missing))) => break,
            other => panic!("unexpected invalid group result: {other:?}"),
        }
    }
    assert_eq!(run.progress().total.prepared_records, 4);
    assert_eq!(run.progress().total.compare_calls, 0);
    assert!(matches!(run.resume(), Err(FileError::Poisoned)));
    assert_eq!(fs::read(f.scratch.path()).unwrap(), bytes);
    let owner = FileJournal::open_anchored(f.scratch.path(), identity(), f.history.clone(), Limits::default()).unwrap();
    let mut run = owner.begin_resume_batch_anchored(f.history, events(), BatchLimits::default()).unwrap();
    inspect(&mut run, 7);
    assert_eq!(write_missing(run.resume().unwrap(), 7).rights().unwrap(), f.oracle.rights());
}

#[test]
fn complete_retry_writes_nothing_and_retains_lock_through_original_handoff() {
    let f = fixture(false, 4);
    let bytes = fs::read(f.scratch.path()).unwrap();
    let head = f.owner.acknowledged_head();
    let mut run = f.owner.begin_resume_batch_anchored(f.history.clone(), events(), BatchLimits::default()).unwrap();
    inspect(&mut run, 1);
    let writer = run.resume().unwrap();
    assert_eq!(writer.progress().stage, BatchCommitStage::Ready);
    assert_eq!(writer.progress().total_records, 0);
    assert_eq!(writer.progress().write_calls, 0);
    assert!(matches!(FileJournal::open_anchored(f.scratch.path(), identity(), f.history.clone(), Limits::default()), Err(FileError::Busy)));
    let mut owner = writer.finish().unwrap();
    assert_eq!(owner.acknowledged_head(), head);
    assert_eq!(fs::read(f.scratch.path()).unwrap(), bytes);
    owner.commit(&Event::RevokeEpoch).unwrap();
    assert_eq!(owner.rights().unwrap().epoch(), 2);
    assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Committed);
}

#[test]
fn changed_extent_after_ready_and_poisoned_owners_never_resume() {
    let f = fixture(false, 1);
    let mut run = f.owner.begin_resume_batch_anchored(f.history.clone(), events(), BatchLimits::default()).unwrap();
    inspect(&mut run, 7);
    OpenOptions::new().append(true).open(f.scratch.path()).unwrap().write_all(&[0]).unwrap();
    assert!(matches!(run.resume(), Err(FileError::Io(_))));
    let f = fixture(false, 1);
    let mut owner = f.owner;
    OpenOptions::new().append(true).open(f.scratch.path()).unwrap().write_all(&[0]).unwrap();
    assert!(matches!(owner.commit(&Event::MarkUnknown { id: 1 }), Err(FileError::Io(_))));
    assert!(owner.is_poisoned());
    assert!(matches!(owner.begin_resume_batch_anchored(f.history, events(), BatchLimits::default()), Err(FileError::Poisoned)));
}
