//! Resume a cancelled ORIGINAL cooperative commit using only the public API.
//! No fault callback, imported ledger, external endpoint or alternate codec.
#![cfg(unix)]
#![forbid(unsafe_code)]

use fa_journal::{Event, Identity, Journal, Limits, RecoveryReserve};
use fa_journal::batch::BatchLimits;
use fa_journal::file::{BatchAnchor, BatchCommitBudget, BatchCommitStage,
    BatchInspectionBudget, BatchInspectionStatus, Error, FileJournal};
use fa_journal::replay::ReplayBudget;
use fa_reference::Effect;
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-public-resume-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path()).unwrap() }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("public resume cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [3; 32], policy: [4; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "custodian".into(), resolved_target: "ledger://public".into(),
        payload: vec![1, 0, 255], units: 20 }
}
fn group() -> Vec<Event> {
    vec![Event::Reserve { id: 1, effect: effect() }, Event::Dispatch { id: 1, effect: effect() },
        Event::MarkUnknown { id: 1 }, Event::RevokeEpoch, Event::Reconcile { id: 1, occurred: true }]
}
fn original() -> (Journal, Vec<u8>, Vec<usize>) {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let mut bytes = journal.header(); let start = bytes.len(); let mut ends = vec![0];
    for event in group() { bytes.extend(journal.append(&event).unwrap()); ends.push(bytes.len() - start); }
    (journal, bytes, ends)
}

fn cancel_at(file: FileJournal, events: Vec<Event>, cut: usize, quantum: usize) -> BatchAnchor {
    let anchor = file.batch_anchor().unwrap();
    let mut commit = file.begin_commit_batch(events.clone(), BatchLimits::default()).unwrap();
    for _ in 0..events.len() {
        if commit.progress().stage != BatchCommitStage::Preparing { break; }
        commit.advance(BatchCommitBudget { prepared_records: 1, write_calls: 0, write_bytes: 0 }).unwrap();
    }
    assert_eq!(commit.progress().stage, BatchCommitStage::Writing);
    assert_eq!(commit.progress().written_bytes, 0);
    while commit.progress().written_bytes < cut as u64 {
        let before = commit.progress().written_bytes;
        let bytes = (cut as u64 - before).min(quantum as u64);
        let report = commit.advance(BatchCommitBudget { prepared_records: 0, write_calls: 1,
            write_bytes: bytes }).unwrap();
        assert!(report.written_bytes > before);
        assert!(report.written_bytes - before <= bytes);
        assert_eq!(commit.acknowledged_head(), anchor.head);
    }
    let report = commit.cancel();
    assert_eq!(report.written_bytes, cut as u64);
    anchor
}

fn inspect(file: FileJournal, anchor: BatchAnchor, events: Vec<Event>) -> (FileJournal, usize) {
    let mut run = file.begin_inspect_batch(anchor, events, BatchLimits::default()).unwrap();
    let budget = BatchInspectionBudget { replay: ReplayBudget::UNLIMITED, prepare_records: 1,
        compare_calls: 1, compare_bytes: 1 };
    for _ in 0..100_000 {
        if run.progress().status == BatchInspectionStatus::Ready {
            let (owner, status) = run.finish().unwrap();
            return (owner, status.persisted_records());
        }
        run.advance(budget).unwrap();
    }
    panic!("bounded original inspection did not finish");
}

#[test]
fn cancelled_cooperative_writer_and_inspector_compose_with_exact_resumption() {
    let (expected, wire, ends) = original();
    for quantum in [1, 7, usize::MAX] {
        for (count, &cut) in ends.iter().enumerate() {
            let temp = Temp::new();
            let file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
            let anchor = cancel_at(file, group(), cut, quantum);
            let stored = temp.bytes();
            assert_eq!(stored, wire[..anchor.encoded_len as usize + cut]);
            let file = FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()).unwrap();
            let (mut file, persisted) = inspect(file, anchor, group());
            assert_eq!(persisted, count); assert_eq!(temp.bytes(), stored);
            assert!(matches!(FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()), Err(Error::Busy)));
            assert_eq!(file.resume_batch(anchor, &group(), BatchLimits::default()).unwrap(), expected.head());
            assert_eq!(file.rights().unwrap(), expected.rights());
            assert_eq!(file.rights().unwrap().available(), 80); // executed units remain spent
            assert_eq!(temp.bytes(), wire);
            assert_eq!(file.resume_batch(anchor, &group(), BatchLimits::default()).unwrap(), expected.head());
            assert_eq!(temp.bytes(), wire); drop(file);
            let file = FileJournal::open(temp.path(), identity(), expected.head(), Limits::default()).unwrap();
            assert_eq!(file.rights().unwrap(), expected.rights());
        }
    }
}

#[test]
fn dropping_a_cooperative_writer_inside_a_record_never_enables_tail_repair() {
    let (_, wire, ends) = original();
    for &boundary in &ends[1..] {
        let temp = Temp::new();
        let file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
        let anchor = cancel_at(file, group(), boundary - 1, 3);
        let interrupted = temp.bytes();
        assert_eq!(interrupted, wire[..anchor.encoded_len as usize + boundary - 1]);
        assert!(FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()).is_err());
        assert_eq!(temp.bytes(), interrupted);
    }
}

#[test]
fn a_previous_successful_inspection_is_not_permission_to_ignore_a_new_conflicting_record() {
    let (_, _, ends) = original();
    let temp = Temp::new();
    let file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let anchor = cancel_at(file, group(), ends[2], 1);
    let file = FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()).unwrap();
    let (mut file, count) = inspect(file, anchor, group());
    assert_eq!(count, 2);
    // Legal independent work occupies the next sequence, where this original
    // group requires MarkUnknown. Prior inspection must not act as a cached permit.
    file.commit(&Event::RevokeEpoch).unwrap();
    let acknowledged = file.acknowledged_head(); let before = temp.bytes();
    assert!(file.resume_batch(anchor, &group(), BatchLimits::default()).is_err());
    assert_eq!(file.acknowledged_head(), acknowledged);
    assert!(file.is_poisoned()); assert_eq!(temp.bytes(), before);
}

#[test]
fn cancelled_reserved_tail_finishes_without_reinstalling_or_replenishing_its_profile() {
    let seed = vec![Event::Reserve { id: 1, effect: effect() },
        Event::Dispatch { id: 1, effect: effect() }, Event::MarkUnknown { id: 1 }];
    let tail = vec![Event::RevokeEpoch, Event::Reconcile { id: 1, occurred: false }];
    let reserve = RecoveryReserve { records: 2, bytes: 51 };
    let mut limits = Limits { max_records: 5, max_total_bytes: 10_000, ..Limits::default() };
    let mut probe = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    for event in seed.iter().chain(&tail) { probe.append(event).unwrap(); }
    limits.max_total_bytes = probe.encoded_len();
    let mut expected = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    let mut wire = expected.header();
    for event in &seed { wire.extend(expected.append(event).unwrap()); }
    let first = expected.append(&tail[0]).unwrap(); wire.extend_from_slice(&first);
    wire.extend(expected.append(&tail[1]).unwrap());
    let temp = Temp::new();
    let mut file = FileJournal::create_with_recovery_reserve(temp.path(), identity(), limits, reserve).unwrap();
    file.commit_batch(&seed, BatchLimits::default()).unwrap();
    assert_eq!(file.capacity().unwrap().ordinary.records, 0);
    assert_eq!(file.capacity().unwrap().ordinary.bytes, 0);
    let anchor = cancel_at(file, tail.clone(), first.len(), 1);
    let file = FileJournal::open_with_recovery_reserve(temp.path(), identity(), anchor.head, limits, reserve).unwrap();
    let (mut file, count) = inspect(file, anchor, tail.clone());
    assert_eq!(count, 1); assert_eq!(file.rights().unwrap().available(), 80);
    assert_eq!(file.resume_batch(anchor, &tail, BatchLimits::default()).unwrap(), expected.head());
    assert_eq!(file.rights().unwrap(), expected.rights());
    assert_eq!(temp.bytes(), wire);
    let capacity = file.capacity().unwrap();
    assert_eq!(capacity.ordinary.records, 0); assert_eq!(capacity.ordinary.bytes, 0);
    assert_eq!(capacity.recovery.records, 0); assert_eq!(capacity.recovery.bytes, 0);
    file.resume_batch(anchor, &tail, BatchLimits::default()).unwrap();
    assert_eq!(file.capacity().unwrap(), capacity); assert_eq!(temp.bytes(), wire);
}

#[path = "file_batch_resume/cooperative.rs"]
mod cooperative;
