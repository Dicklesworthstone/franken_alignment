#![cfg(unix)]

use fa_journal::file::{Error as FileError, FileJournal};
use fa_journal::replay::{HistoryAnchor, ReplayBudget, ReplayStatus};
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
            "fa-file-history-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn journal(&self) -> PathBuf { self.0.join("rights.bin") }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 10 }
}
fn effect(payload: &[u8]) -> Effect {
    Effect {
        principal: "actor".into(), resolved_target: "publication".into(),
        payload: payload.to_vec(), units: 4,
    }
}
fn reserve(payload: &[u8]) -> Event { Event::Reserve { id: 1, effect: effect(payload) } }
fn history(payload: &[u8]) -> (Journal, Vec<u8>) {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let mut bytes = journal.header();
    bytes.extend(journal.append(&reserve(payload)).unwrap());
    (journal, bytes)
}

#[test]
fn capture_and_successor_recovery_preserve_accounting_and_allow_continued_writes() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits::default();
    let mut writer = FileJournal::create(&path, identity(), limits).unwrap();
    let empty = writer.history_anchor().unwrap();
    assert_eq!(empty.head(), writer.acknowledged_head());
    writer.commit(&reserve(b"proposal")).unwrap();
    writer.commit(&Event::Dispatch { id: 1, effect: effect(b"proposal") }).unwrap();
    let anchor = writer.history_anchor().unwrap();
    let independent = anchor.as_bytes().to_vec();
    assert_eq!(independent, fs::read(&path).unwrap());
    assert!(!writer.is_poisoned());
    writer.commit(&Event::MarkUnknown { id: 1 }).unwrap();
    writer.commit(&Event::RevokeEpoch).unwrap();
    let expected_head = writer.acknowledged_head();
    let expected_rights = writer.rights().unwrap().clone();
    let expected_capacity = writer.capacity().unwrap();
    drop(writer);

    let mut recovered = FileJournal::open_anchored(&path, identity(), anchor, limits).unwrap();
    assert_eq!(recovered.acknowledged_head(), expected_head);
    assert_eq!(recovered.rights().unwrap(), &expected_rights);
    assert_eq!(recovered.capacity().unwrap(), expected_capacity);
    assert_eq!(recovered.rights().unwrap().state(1).unwrap(), State::Unknown);
    assert_eq!(recovered.rights().unwrap().available(), 6);
    let next = recovered.commit(&Event::Reconcile { id: 1, occurred: true }).unwrap();
    assert_eq!(next.sequence, expected_head.sequence + 1);
    let captured = recovered.history_anchor().unwrap();
    let restored = HistoryAnchor::from_bytes(captured.as_bytes().to_vec(), identity(), limits).unwrap();
    assert_eq!(restored.head(), next);
    drop(recovered);
    let complete = FileJournal::open_anchored(&path, identity(), restored, limits).unwrap();
    assert_eq!(complete.rights().unwrap().state(1).unwrap(), State::Committed);
    assert!(complete.rights().unwrap().conserved());
    drop(complete);
    // An older independently retained prefix remains a minimum, not a rewind.
    let from_empty = FileJournal::open_anchored(&path, identity(), empty, limits).unwrap();
    assert_eq!(from_empty.acknowledged_head(), next);
}

#[test]
fn the_original_lock_survives_reading_ready_and_finish_and_cancel_releases_it() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits::default();
    let mut writer = FileJournal::create(&path, identity(), limits).unwrap();
    writer.commit(&reserve(b"x")).unwrap();
    let anchor = writer.history_anchor().unwrap();
    let bytes = fs::read(&path).unwrap();
    drop(writer);
    let mut run = FileJournal::begin_open_anchored(&path, identity(), anchor.clone(), limits).unwrap();
    assert_eq!(run.advance(ReplayBudget::default()).unwrap().total.read_calls, 0);
    assert!(matches!(FileJournal::begin_open(&path, identity(), anchor.head(), limits), Err(FileError::Busy)));
    while run.progress().status == ReplayStatus::Reading {
        let report = run.advance(ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 1 }).unwrap();
        assert!(report.spent.read_calls <= 1 && report.spent.read_bytes <= 1);
        assert!(report.spent.checked_records <= 1);
    }
    assert_eq!(run.progress().total.read_bytes, bytes.len() as u64);
    assert!(matches!(FileJournal::begin_open_anchored(&path, identity(), anchor.clone(), limits), Err(FileError::Busy)));
    let writer = run.finish().unwrap();
    assert!(matches!(FileJournal::begin_open(&path, identity(), anchor.head(), limits), Err(FileError::Busy)));
    drop(writer);

    let incomplete = FileJournal::begin_open_anchored(&path, identity(), anchor.clone(), limits).unwrap();
    assert!(matches!(incomplete.finish(), Err(FileError::RecoveryIncomplete)));
    let mut cancelled = FileJournal::begin_open_anchored(&path, identity(), anchor.clone(), limits).unwrap();
    cancelled.advance(ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 0 }).unwrap();
    assert_eq!(cancelled.cancel().read_bytes, 1);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    FileJournal::open_anchored(&path, identity(), anchor, limits).unwrap();
}

#[test]
fn equal_crc_forks_refuse_a_writer_without_repairing_any_bytes() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits::default();
    let (left, original) = history(b"plumless");
    let (right, fork) = history(b"buckeroo");
    assert_eq!(left.head(), right.head());
    assert_ne!(original, fork);
    let anchor = HistoryAnchor::from_bytes(original.clone(), identity(), limits).unwrap();
    fs::write(&path, &fork).unwrap();
    // The legacy CRC minimum really admits this complete, valid fork.
    drop(FileJournal::open(&path, identity(), anchor.head(), limits).unwrap());
    let mut run = FileJournal::begin_open_anchored(&path, identity(), anchor.clone(), limits).unwrap();
    assert!(matches!(run.advance(ReplayBudget::UNLIMITED), Err(FileError::Journal(Error::Binding))));
    assert_eq!(run.progress().status, ReplayStatus::Failed);
    assert!(run.progress().total.read_bytes > 0);
    assert!(matches!(FileJournal::begin_open(&path, identity(), anchor.head(), limits), Err(FileError::Busy)));
    assert!(matches!(run.finish(), Err(FileError::Poisoned)));
    assert_eq!(fs::read(&path).unwrap(), fork);
    // A genuine copy of the same selected history is accepted.
    fs::write(&path, &original).unwrap();
    assert_eq!(FileJournal::open_anchored(&path, identity(), anchor, limits).unwrap().acknowledged_head(), left.head());
}

#[test]
fn context_refusal_precedes_missing_path_access_and_never_creates_a_file() {
    let scratch = Scratch::new();
    let absent = scratch.0.join("missing-parent").join("rights.bin");
    let limits = Limits::default();
    let (_, bytes) = history(b"x");
    let anchor = HistoryAnchor::from_bytes(bytes, identity(), limits).unwrap();
    let mut foreign = identity();
    foreign.policy[0] = 9;
    assert!(matches!(FileJournal::begin_open_anchored(&absent, foreign, anchor.clone(), limits), Err(FileError::Journal(Error::Binding))));
    let wrong = Limits { max_records: limits.max_records - 1, ..limits };
    assert!(matches!(FileJournal::begin_open_anchored(&absent, identity(), anchor.clone(), wrong), Err(FileError::Journal(Error::Binding))));
    assert!(matches!(FileJournal::open_anchored(&absent, identity(), anchor, limits), Err(FileError::Io(_))));
    assert!(!absent.exists());
}

#[test]
fn rollback_truncation_and_invalid_suffixes_never_publish_partial_accounting() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits::default();
    let (journal, bytes) = history(b"x");
    let anchor = HistoryAnchor::from_bytes(bytes.clone(), identity(), limits).unwrap();
    let mut partial = bytes.clone();
    partial.pop();
    let mut suffix = bytes;
    suffix.push(0);
    for broken in [journal.header(), partial, suffix] {
        fs::write(&path, &broken).unwrap();
        let mut run = FileJournal::begin_open_anchored(&path, identity(), anchor.clone(), limits).unwrap();
        assert!(run.advance(ReplayBudget::UNLIMITED).is_err());
        assert!(matches!(run.finish(), Err(FileError::Poisoned)));
        assert_eq!(fs::read(&path).unwrap(), broken);
    }
}

#[test]
fn capture_compares_accounting_even_when_extent_and_crc_head_match() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits::default();
    let mut writer = FileJournal::create(&path, identity(), limits).unwrap();
    writer.commit(&reserve(b"plumless")).unwrap();
    let previous = writer.history_anchor().unwrap();
    let (other, fork) = history(b"buckeroo");
    assert_eq!(writer.acknowledged_head(), other.head());
    // Deliberately violate cooperating-writer assumptions to inject corruption.
    fs::write(&path, &fork).unwrap();
    assert!(matches!(writer.history_anchor(), Err(FileError::Journal(Error::Binding))));
    assert!(writer.is_poisoned());
    assert!(matches!(writer.rights(), Err(FileError::Poisoned)));
    assert!(matches!(writer.history_anchor(), Err(FileError::Poisoned)));
    assert!(matches!(writer.commit(&Event::RevokeEpoch), Err(FileError::Poisoned)));
    assert_eq!(writer.acknowledged_head(), previous.head());
    assert_eq!(fs::read(&path).unwrap(), fork);
    drop(writer);
    assert!(FileJournal::open_anchored(&path, identity(), previous, limits).is_err());
}

#[test]
fn capture_never_adopts_an_unacknowledged_suffix_but_recovery_replays_it() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits::default();
    let mut writer = FileJournal::create(&path, identity(), limits).unwrap();
    writer.commit(&reserve(b"x")).unwrap();
    writer.commit(&Event::Dispatch { id: 1, effect: effect(b"x") }).unwrap();
    let previous = writer.history_anchor().unwrap();
    let mut journal = Journal::replay(previous.as_bytes(), identity(), previous.head(), limits).unwrap();
    let suffix = journal.append(&Event::RevokeEpoch).unwrap();
    let mut disk = OpenOptions::new().append(true).open(&path).unwrap();
    disk.write_all(&suffix).unwrap();
    disk.sync_all().unwrap();
    drop(disk);
    let bytes = fs::read(&path).unwrap();
    assert!(matches!(writer.history_anchor(), Err(FileError::Io(_))));
    assert!(writer.is_poisoned());
    assert_eq!(writer.acknowledged_head(), previous.head());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(writer);
    let recovered = FileJournal::open_anchored(&path, identity(), previous, limits).unwrap();
    assert_eq!(recovered.acknowledged_head(), journal.head());
    assert_eq!(recovered.rights().unwrap(), journal.rights());
    assert_eq!(recovered.rights().unwrap().available(), 6);
}

#[test]
fn explicit_reserved_profile_keeps_unknown_liabilities_and_remaining_tail_capacity() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits { max_records: 5, max_frame_bytes: 1024, max_total_bytes: 4096 };
    let reserve_profile = RecoveryReserve { records: 2, bytes: 60 };
    let mut writer = FileJournal::create_with_recovery_reserve(&path, identity(), limits, reserve_profile).unwrap();
    writer.commit(&reserve(b"x")).unwrap();
    writer.commit(&Event::Dispatch { id: 1, effect: effect(b"x") }).unwrap();
    let previous = writer.history_anchor().unwrap();
    writer.commit(&Event::MarkUnknown { id: 1 }).unwrap();
    writer.commit(&Event::RevokeEpoch).unwrap();
    let capacity = writer.capacity().unwrap();
    drop(writer);
    assert!(matches!(FileJournal::begin_open_anchored(&path, identity(), previous.clone(), limits), Err(FileError::Journal(Error::Binding))));
    let wrong = RecoveryReserve { records: 1, bytes: 30 };
    assert!(matches!(FileJournal::begin_open_anchored_with_recovery_reserve(&path, identity(), previous.clone(), limits, wrong), Err(FileError::Journal(Error::Binding))));
    let mut recovered = FileJournal::open_anchored_with_recovery_reserve(&path, identity(), previous, limits, reserve_profile).unwrap();
    assert_eq!(recovered.capacity().unwrap(), capacity);
    assert_eq!(recovered.rights().unwrap().state(1).unwrap(), State::Unknown);
    assert_eq!(recovered.rights().unwrap().available(), 6);
    let ordinary = Event::Reserve { id: 2, effect: effect(b"other") };
    assert!(matches!(recovered.commit(&ordinary), Err(FileError::Journal(Error::Limit))));
    assert!(!recovered.is_poisoned());
    recovered.commit(&Event::Reconcile { id: 1, occurred: true }).unwrap();
    assert!(matches!(recovered.commit(&Event::RevokeEpoch), Err(FileError::Journal(Error::Limit))));
    let final_anchor = recovered.history_anchor().unwrap();
    assert_eq!(final_anchor.head(), recovered.acknowledged_head());
}

#[test]
fn changed_extent_after_ready_still_fails_the_original_completion_barrier() {
    let scratch = Scratch::new();
    let path = scratch.journal();
    let limits = Limits::default();
    let (_, bytes) = history(b"x");
    let anchor = HistoryAnchor::from_bytes(bytes.clone(), identity(), limits).unwrap();
    fs::write(&path, &bytes).unwrap();
    let mut run = FileJournal::begin_open_anchored(&path, identity(), anchor.clone(), limits).unwrap();
    assert_eq!(run.advance(ReplayBudget::UNLIMITED).unwrap().status, ReplayStatus::Ready);
    OpenOptions::new().append(true).open(&path).unwrap().write_all(&[0]).unwrap();
    let changed = fs::read(&path).unwrap();
    assert!(matches!(run.finish(), Err(FileError::Io(_))));
    assert_eq!(fs::read(&path).unwrap(), changed);
    fs::write(&path, &bytes).unwrap();
    FileJournal::open_anchored(&path, identity(), anchor, limits).unwrap();
}
