//! Public file-owner integration: actual canonical bytes and original recovery.
#![cfg(unix)]
use fa_journal::{Event, Identity, Journal, Limits};
use fa_journal::batch::BatchLimits;
use fa_journal::file::{Error, FileJournal};
use fa_journal::replay::{ReplayBudget, ReplayStatus};
use fa_reference::{Effect, State};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-public-group-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("batch cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "alice".into(), resolved_target: "ledger://one".into(), payload: vec![0, 255], units: 20 }
}
fn reservations() -> Vec<Event> {
    vec![Event::Reserve { id: 1, effect: effect() }, Event::Reserve { id: 2, effect: effect() }]
}

#[test]
fn grouped_dispatch_recovery_and_reconciliation_equal_original_single_commits() {
    let group_root = Temp::new(); let single_root = Temp::new();
    let mut grouped = FileJournal::create(group_root.path(), identity(), Limits::default()).unwrap();
    let mut single = FileJournal::create(single_root.path(), identity(), Limits::default()).unwrap();
    let stages = [reservations(),
        vec![Event::Dispatch { id: 1, effect: effect() }, Event::Dispatch { id: 2, effect: effect() }],
        vec![Event::MarkUnknown { id: 1 }, Event::MarkUnknown { id: 2 }, Event::RevokeEpoch],
        vec![Event::ResetToCheckpoint { keep: BTreeSet::new() }],
        vec![Event::Reconcile { id: 1, occurred: true }, Event::Reconcile { id: 2, occurred: false }]];
    for stage in stages {
        let head = grouped.commit_batch(&stage, BatchLimits::default()).unwrap();
        for event in &stage { single.commit(event).unwrap(); }
        assert_eq!(head, single.acknowledged_head());
        assert_eq!(grouped.rights().unwrap(), single.rights().unwrap());
        assert_eq!(fs::read(group_root.path()).unwrap(), fs::read(single_root.path()).unwrap());
        drop(grouped);
        let mut recovery = FileJournal::begin_open(group_root.path(), identity(), head, Limits::default()).unwrap();
        while recovery.progress().status == ReplayStatus::Reading {
            recovery.advance(ReplayBudget { read_calls: 1, read_bytes: 3, checked_records: 1 }).unwrap();
        }
        grouped = recovery.finish().unwrap();
        assert_eq!(grouped.rights().unwrap(), single.rights().unwrap());
    }
    assert_eq!(grouped.rights().unwrap().available(), 80);
    assert_eq!(grouped.rights().unwrap().state(1), Ok(State::Committed));
    assert_eq!(grouped.rights().unwrap().state(2), Ok(State::Aborted));
    assert_eq!(grouped.rights().unwrap().incident_count(), 1);
}

#[test]
fn last_invalid_event_leaves_disk_accounting_and_head_unchanged_then_valid_retry_works() {
    let root = Temp::new(); let mut file = FileJournal::create(root.path(), identity(), Limits::default()).unwrap();
    let before = fs::read(root.path()).unwrap(); let anchor = file.acknowledged_head();
    let mut invalid = reservations(); invalid.push(Event::Reserve { id: 1, effect: effect() });
    assert!(file.commit_batch(&invalid, BatchLimits::default()).is_err());
    assert!(!file.is_poisoned()); assert_eq!(file.acknowledged_head(), anchor);
    assert_eq!(file.rights().unwrap().available(), 100);
    assert_eq!(fs::read(root.path()).unwrap(), before);
    let head = file.commit_batch(&reservations(), BatchLimits::default()).unwrap();
    assert_eq!(head.sequence, 2); assert_eq!(file.rights().unwrap().available(), 60);
}

#[test]
fn exact_group_and_lifetime_budgets_work_and_one_short_writes_nothing() {
    let mut oracle = Journal::new(identity(), Limits::default()).unwrap();
    let bytes: usize = reservations().iter().map(|event| oracle.append(event).unwrap().len()).sum();
    for short_group in [false, true] {
        let root = Temp::new();
        let mut file = FileJournal::create(root.path(), identity(), Limits { max_records: 2,
            max_total_bytes: oracle.encoded_len(), ..Limits::default() }).unwrap();
        let before = fs::read(root.path()).unwrap();
        let budget = BatchLimits { max_records: 2, max_encoded_bytes: bytes - usize::from(short_group) };
        let result = file.commit_batch(&reservations(), budget);
        if short_group {
            assert!(result.is_err()); assert!(!file.is_poisoned());
            assert_eq!(fs::read(root.path()).unwrap(), before);
            file.commit_batch(&reservations(), BatchLimits { max_records: 2, max_encoded_bytes: bytes }).unwrap();
        } else { assert_eq!(result.unwrap(), oracle.head()); }
        assert_eq!(file.rights().unwrap(), oracle.rights());
        let before = fs::read(root.path()).unwrap();
        assert!(file.commit_batch(&[Event::RevokeEpoch], BatchLimits::default()).is_err());
        assert!(!file.is_poisoned()); assert_eq!(fs::read(root.path()).unwrap(), before);
    }
}

#[test]
fn empty_groups_do_not_flush_and_cannot_bypass_single_writer_ownership() {
    let root = Temp::new(); let mut file = FileJournal::create(root.path(), identity(), Limits::default()).unwrap();
    let anchor = file.acknowledged_head(); let before = fs::read(root.path()).unwrap();
    assert!(matches!(file.commit_batch(&[], BatchLimits::default()), Err(Error::Journal(fa_journal::Error::InvalidInput))));
    assert!(!file.is_poisoned()); assert_eq!(file.acknowledged_head(), anchor);
    assert_eq!(fs::read(root.path()).unwrap(), before);
    file.commit_batch(&reservations(), BatchLimits::default()).unwrap();
    assert!(matches!(FileJournal::open(root.path(), identity(), anchor, Limits::default()), Err(Error::Busy)));
    drop(file);
    let file = FileJournal::open(root.path(), identity(), anchor, Limits::default()).unwrap();
    assert_eq!(file.rights().unwrap().available(), 60);
}

#[test]
fn an_acknowledged_final_group_head_rejects_rollback_even_at_an_internal_record_boundary() {
    let root = Temp::new();
    let mut file = FileJournal::create(root.path(), identity(), Limits::default()).unwrap();
    let header = fs::read(root.path()).unwrap().len();
    let head = file.commit_batch(&reservations(), BatchLimits::default()).unwrap();
    drop(file);
    let canonical = fs::read(root.path()).unwrap();
    for end in header..canonical.len() {
        // Deliberately restore an obsolete prefix after dropping the writer.
        fs::write(root.path(), &canonical[..end]).unwrap();
        assert!(FileJournal::open(root.path(), identity(), head, Limits::default()).is_err());
        assert_eq!(fs::read(root.path()).unwrap(), canonical[..end]);
    }
    fs::write(root.path(), &canonical).unwrap();
    let reopened = FileJournal::open(root.path(), identity(), head, Limits::default()).unwrap();
    assert_eq!(reopened.acknowledged_head(), head);
    assert_eq!(reopened.rights().unwrap().available(), 60);
}
