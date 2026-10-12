//! Private fault injection at the actual writer and final synchronization cuts.
use super::*;
use crate::{Identity, Limits};
use fa_reference::{Effect, State};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-write-quantum-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("quantum test cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "alice".into(), resolved_target: "ledger://one".into(), payload: vec![0, 255], units: 20 }
}
fn group() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect() }, Event::MarkUnknown { id: 1 }]
}
fn run(temp: &Temp) -> FileBatchCommit {
    let mut owner = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    owner.commit(&Event::Reserve { id: 1, effect: effect() }).unwrap();
    let mut run = owner.begin_commit_batch(group(), BatchLimits::default()).unwrap();
    run.advance(BatchCommitBudget::UNLIMITED).unwrap(); run
}
fn fault() -> io::Error { io::Error::other("injected original file failure") }

#[test]
fn interrupted_and_short_writes_consume_the_exact_poll_allowances() {
    let temp = Temp::new(); let mut run = run(&temp); let before = fs::read(temp.path()).unwrap();
    let report = run.advance_with(BatchCommitBudget { write_calls: 3, ..BatchCommitBudget::UNLIMITED },
        |_, _| Err(io::ErrorKind::Interrupted.into())).unwrap();
    assert_eq!(report.write_calls, 3); assert_eq!(report.written_bytes, 0);
    assert_eq!(report.stage, BatchCommitStage::Writing); assert_eq!(fs::read(temp.path()).unwrap(), before);
    let mut calls = 0;
    let report = run.advance_with(BatchCommitBudget { write_calls: 5, write_bytes: 3, prepared_records: 0 },
        |file, bytes| { calls += 1; file.write(&bytes[..1]) }).unwrap();
    assert_eq!(calls, 3); assert_eq!(report.write_calls, 6); assert_eq!(report.written_bytes, 3);
    while run.progress().stage != BatchCommitStage::Ready { run.advance(BatchCommitBudget::UNLIMITED).unwrap(); }
    assert_eq!(run.finish().unwrap().rights().unwrap().state(1), Ok(State::Unknown));
}

#[test]
fn failed_or_panicking_write_keeps_the_old_minimum_and_never_exposes_live_accounting() {
    for unwind in [false, true] {
        let temp = Temp::new(); let mut run = run(&temp); let old = run.acknowledged_head();
        let length = run.bytes.len() as u64;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run.advance_with(BatchCommitBudget::UNLIMITED, |file, bytes| {
                file.write_all(bytes)?;
                if unwind { panic!("after physical write, before returned count"); }
                Err(fault())
            })
        }));
        if unwind { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        assert_eq!(run.progress().stage, BatchCommitStage::Failed); assert_eq!(run.progress().write_calls, 1);
        // Returned byte accounting must not invent the amount an errored write persisted.
        assert_eq!(run.progress().written_bytes, 0);
        assert_eq!(fs::metadata(temp.path()).unwrap().len(), run.owner.journal.encoded_len() + length);
        assert_eq!(run.acknowledged_head(), old);
        assert!(matches!(run.owner.rights(), Err(Error::Poisoned)));
        assert!(matches!(run.advance(BatchCommitBudget::UNLIMITED), Err(Error::Poisoned)));
        assert!(matches!(FileJournal::open(temp.path(), identity(), old, Limits::default()), Err(Error::Busy)));
        assert!(matches!(run.finish(), Err(Error::Poisoned)));
        let owner = FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap();
        assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(owner.rights().unwrap().available(), 80);
    }
}

#[test]
fn zero_overreported_or_wouldblock_write_is_terminal_not_implicit_progress() {
    for mode in 0..3 {
        let temp = Temp::new(); let mut run = run(&temp); let before = fs::read(temp.path()).unwrap();
        assert!(run.advance_with(BatchCommitBudget::UNLIMITED, |_, bytes| match mode {
            0 => Ok(0), 1 => Ok(bytes.len() + 1), _ => Err(io::ErrorKind::WouldBlock.into()),
        }).is_err());
        assert_eq!(run.progress().stage, BatchCommitStage::Failed); assert_eq!(run.progress().written_bytes, 0);
        assert_eq!(run.progress().write_calls, 1); assert_eq!(fs::read(temp.path()).unwrap(), before);
        assert!(matches!(run.finish(), Err(Error::Poisoned)));
    }
}

#[test]
fn final_sync_is_once_under_the_same_lock_and_failed_sync_returns_no_candidate() {
    for mode in 0..3 {
        let temp = Temp::new(); let mut run = run(&temp); let old = run.acknowledged_head();
        while run.progress().stage != BatchCommitStage::Ready { run.advance(BatchCommitBudget::UNLIMITED).unwrap(); }
        let bytes = fs::read(temp.path()).unwrap(); let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.finish_with(|file| {
            calls += 1;
            assert!(matches!(FileJournal::open(temp.path(), identity(), old, Limits::default()), Err(Error::Busy)));
            match mode { 0 => file.sync_all(), 1 => Err(fault()), _ => panic!("sync unwind") }
        })));
        assert_eq!(calls, 1);
        match mode {
            0 => { let owner = result.unwrap().unwrap();
                assert!(!owner.is_poisoned()); assert_eq!(owner.acknowledged_head().sequence, old.sequence + 2); }
            1 => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
        let owner = FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap();
        assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    }
}

#[test]
fn noncooperating_extent_changes_refuse_before_next_write_or_final_sync() {
    for at_ready in [false, true] {
        let temp = Temp::new(); let mut run = run(&temp);
        if at_ready { while run.progress().stage != BatchCommitStage::Ready {
            run.advance(BatchCommitBudget::UNLIMITED).unwrap(); } }
        OpenOptions::new().append(true).open(temp.path()).unwrap().write_all(b"unreviewed").unwrap();
        let bytes = fs::read(temp.path()).unwrap();
        if at_ready { assert!(run.finish_with(|_| panic!("changed extent reached sync")).is_err()); }
        else {
            assert!(run.advance_with(BatchCommitBudget::UNLIMITED, |_, _| panic!("changed extent reached write")).is_err());
            assert_eq!(run.progress().stage, BatchCommitStage::Failed);
        }
        assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    }
}

#[test]
fn preparation_failure_and_zero_polls_cannot_reach_the_file_writer_hook() {
    let temp = Temp::new();
    let owner = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let bytes = fs::read(temp.path()).unwrap();
    let mut run = owner.begin_commit_batch(vec![Event::RevokeEpoch, Event::Dispatch { id: 1, effect: effect() }],
        BatchLimits::default()).unwrap();
    run.advance_with(BatchCommitBudget::default(), |_, _| panic!("zero reached write")).unwrap();
    assert!(run.advance_with(BatchCommitBudget::UNLIMITED, |_, _| panic!("invalid reached write")).is_err());
    assert_eq!(run.progress().prepared_records, 1); assert_eq!(run.progress().preparation_attempts, 2);
    assert_eq!(run.progress().write_calls, 0); assert_eq!(fs::read(temp.path()).unwrap(), bytes);
}
