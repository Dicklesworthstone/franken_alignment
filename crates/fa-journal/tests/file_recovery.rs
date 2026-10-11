#![cfg(unix)]
use fa_journal::{Event, Head, Identity, Journal, Limits};
use fa_journal::file::{Error, FileJournal, FileRecovery};
use fa_journal::replay::{ReplayBudget, ReplayStatus, ReplayWork};
use fa_reference::{Effect, State};
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-cooperative-recovery-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn file(&self) -> PathBuf { self.0.join("rights.bin") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("recovery test cleanup: {error}"); }
    }
}

fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 100 }
}
fn effect(units: u64) -> Effect {
    Effect { principal: "alice".into(), resolved_target: "account/one".into(), payload: vec![0, 255, 1], units }
}
fn fixture(path: &Path) -> (Journal, Head) {
    let mut expected = Journal::new(identity(), Limits::default()).unwrap();
    let genesis = expected.head();
    let mut writer = FileJournal::create(path, identity(), Limits::default()).unwrap();
    for event in [
        Event::Reserve { id: 1, effect: effect(30) },
        Event::Reserve { id: 2, effect: effect(20) },
        Event::Dispatch { id: 1, effect: effect(30) },
        Event::MarkUnknown { id: 1 },
        Event::ResetToCheckpoint { keep: BTreeSet::new() },
        Event::RevokeEpoch,
    ] {
        expected.append(&event).unwrap();
        assert_eq!(writer.commit(&event).unwrap(), expected.head());
    }
    (expected, genesis)
}
fn busy(path: &Path, head: Head) {
    assert!(matches!(FileJournal::open(path, identity(), head, Limits::default()), Err(Error::Busy)));
}
fn complete(mut run: FileRecovery, budget: ReplayBudget) -> (FileJournal, ReplayWork) {
    for _ in 0..10_000 {
        let before = run.progress().total;
        let report = run.advance(budget).unwrap();
        assert!(report.spent.read_calls <= budget.read_calls);
        assert!(report.spent.read_bytes <= budget.read_bytes);
        assert!(report.spent.checked_records <= budget.checked_records);
        assert_eq!(report.total.read_calls, before.read_calls + report.spent.read_calls);
        assert_eq!(report.total.read_bytes, before.read_bytes + report.spent.read_bytes);
        assert_eq!(report.total.checked_records, before.checked_records + report.spent.checked_records);
        if report.status == ReplayStatus::Ready {
            assert_eq!(run.advance(ReplayBudget::default()).unwrap().spent, ReplayWork::default());
            return (run.finish().unwrap(), report.total);
        }
        assert_ne!(report.spent, ReplayWork::default());
    }
    panic!("small file recovery did not complete");
}

#[test]
fn bounded_file_recovery_keeps_the_lock_and_preserves_unknown_suffix_before_appending() {
    for budget in [
        ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 1 },
        ReplayBudget { read_calls: 3, read_bytes: 31, checked_records: 1 },
        ReplayBudget::UNLIMITED,
    ] {
        let root = Directory::new();
        let (mut expected, minimum) = fixture(&root.file());
        let bytes = std::fs::read(root.file()).unwrap();
        let run = FileJournal::begin_open(root.file(), identity(), minimum, Limits::default()).unwrap();
        assert_eq!(run.progress().total, ReplayWork::default());
        busy(&root.file(), minimum);
        let (mut writer, work) = complete(run, budget);
        assert_eq!(work.read_bytes, bytes.len() as u64);
        assert_eq!(work.checked_records, expected.head().sequence);
        assert_eq!(writer.acknowledged_head(), expected.head());
        assert_eq!(writer.rights().unwrap(), expected.rights());
        assert_eq!(writer.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(writer.rights().unwrap().available(), 70);
        assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
        busy(&root.file(), minimum);
        let event = Event::Reconcile { id: 1, occurred: false };
        expected.append(&event).unwrap();
        assert_eq!(writer.commit(&event).unwrap(), expected.head());
        let bytes = std::fs::read(root.file()).unwrap();
        let control = Journal::replay(bytes.as_slice(), identity(), minimum, Limits::default()).unwrap();
        assert_eq!(control.rights(), writer.rights().unwrap());
        assert_eq!(control.rights(), expected.rights());
    }
}

#[test]
fn cancelling_partial_headers_records_and_ready_recovery_never_changes_journal_bytes() {
    let root = Directory::new();
    let (expected, minimum) = fixture(&root.file());
    let bytes = std::fs::read(root.file()).unwrap();
    for cut in [0, 1, 83, 84, 87, 88, bytes.len() / 2, bytes.len()] {
        let mut run = FileJournal::begin_open(root.file(), identity(), minimum, Limits::default()).unwrap();
        run.advance(ReplayBudget { read_bytes: cut as u64, ..ReplayBudget::UNLIMITED }).unwrap();
        busy(&root.file(), minimum);
        let work = run.cancel();
        assert_eq!(work.read_bytes, cut as u64);
        assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
        let writer = FileJournal::open(root.file(), identity(), minimum, Limits::default()).unwrap();
        assert_eq!(writer.rights().unwrap(), expected.rights());
    }
    let mut run = FileJournal::begin_open(root.file(), identity(), minimum, Limits::default()).unwrap();
    assert_eq!(run.advance(ReplayBudget::UNLIMITED).unwrap().status, ReplayStatus::Ready);
    busy(&root.file(), minimum);
    drop(run);
    assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
    assert!(FileJournal::open(root.file(), identity(), minimum, Limits::default()).is_ok());
}

#[test]
fn incomplete_finish_releases_the_lock_without_publishing_accounting_or_repairing() {
    let root = Directory::new();
    let (expected, minimum) = fixture(&root.file());
    let bytes = std::fs::read(root.file()).unwrap();
    let mut run = FileJournal::begin_open(root.file(), identity(), minimum, Limits::default()).unwrap();
    run.advance(ReplayBudget { read_bytes: bytes.len() as u64 - 1, ..ReplayBudget::UNLIMITED }).unwrap();
    assert!(matches!(run.finish(), Err(Error::RecoveryIncomplete)));
    assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
    let recovered = FileJournal::open(root.file(), identity(), minimum, Limits::default()).unwrap();
    assert_eq!(recovered.rights().unwrap(), expected.rights());
}

#[test]
fn damaged_or_behind_anchor_files_cannot_export_a_checked_prefix() {
    let root = Directory::new();
    let (expected, minimum) = fixture(&root.file());
    let bytes = std::fs::read(root.file()).unwrap();
    for cut in [0, 1, 83, 84, 87, 88, bytes.len() / 2, bytes.len() - 1] {
        let damaged = root.0.join(format!("damaged-{cut}"));
        std::fs::write(&damaged, &bytes[..cut]).unwrap();
        let mut run = FileJournal::begin_open(&damaged, identity(), expected.head(), Limits::default()).unwrap();
        assert!(run.advance(ReplayBudget::UNLIMITED).is_err());
        let failed = run.progress();
        assert_eq!(failed.status, ReplayStatus::Failed);
        busy(&damaged, minimum);
        assert!(matches!(run.advance(ReplayBudget::UNLIMITED), Err(Error::Poisoned)));
        assert_eq!(run.progress(), failed);
        assert!(matches!(run.finish(), Err(Error::Poisoned)));
        assert_eq!(std::fs::read(&damaged).unwrap(), bytes[..cut]);
        assert!(FileJournal::open(&damaged, identity(), expected.head(), Limits::default()).is_err());
    }
    assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
    assert!(FileJournal::open(root.file(), identity(), minimum, Limits::default()).is_ok());
}

#[test]
fn wrong_context_and_forged_anchor_refuse_without_altering_the_original_file() {
    let root = Directory::new();
    let (expected, minimum) = fixture(&root.file());
    let bytes = std::fs::read(root.file()).unwrap();
    for (context, anchor) in [
        (Identity { policy: [9; 32], ..identity() }, minimum),
        (identity(), Head { checksum: minimum.checksum ^ 1, ..minimum }),
        (identity(), Head { checksum: expected.head().checksum ^ 1, ..expected.head() }),
        (identity(), Head { sequence: expected.head().sequence + 1, checksum: 0 }),
    ] {
        let mut run = FileJournal::begin_open(root.file(), context, anchor, Limits::default()).unwrap();
        assert!(run.advance(ReplayBudget::UNLIMITED).is_err());
        assert_eq!(run.progress().status, ReplayStatus::Failed);
        run.cancel();
        assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
        assert!(FileJournal::open(root.file(), identity(), minimum, Limits::default()).is_ok());
    }
}

#[test]
fn lifetime_limits_refuse_even_when_later_polls_offer_unlimited_work() {
    let root = Directory::new();
    let (expected, minimum) = fixture(&root.file());
    let bytes = std::fs::read(root.file()).unwrap();
    let short = Limits { max_total_bytes: bytes.len() as u64 - 1, ..Limits::default() };
    assert!(matches!(FileJournal::begin_open(root.file(), identity(), minimum, short),
        Err(Error::Journal(fa_journal::Error::Limit))));
    let limits = Limits { max_records: expected.head().sequence - 1, ..Limits::default() };
    let mut run = FileJournal::begin_open(root.file(), identity(), minimum, limits).unwrap();
    assert!(matches!(run.advance(ReplayBudget::UNLIMITED), Err(Error::Journal(fa_journal::Error::Limit))));
    assert!(matches!(run.advance(ReplayBudget::UNLIMITED), Err(Error::Poisoned)));
    run.cancel();
    let limits = Limits { max_records: expected.head().sequence, max_total_bytes: bytes.len() as u64, ..limits };
    let writer = FileJournal::open(root.file(), identity(), minimum, limits).unwrap();
    assert_eq!(writer.rights().unwrap(), expected.rights());
    assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
}

#[test]
fn changed_extent_after_clean_eof_prevents_finalization_without_removing_the_tail() {
    let root = Directory::new();
    let (_, minimum) = fixture(&root.file());
    let mut run = FileJournal::begin_open(root.file(), identity(), minimum, Limits::default()).unwrap();
    assert_eq!(run.advance(ReplayBudget::UNLIMITED).unwrap().status, ReplayStatus::Ready);
    // Deliberately violate the trusted-writer assumption inside this disposable
    // fixture. The adapter must still reject the changed extent, not repair it.
    let mut other = std::fs::OpenOptions::new().append(true).open(root.file()).unwrap();
    other.write_all(&[0]).unwrap(); other.sync_all().unwrap(); drop(other);
    let bytes = std::fs::read(root.file()).unwrap();
    assert!(matches!(run.finish(), Err(Error::Io(error)) if error.kind() == io::ErrorKind::InvalidData));
    assert_eq!(std::fs::read(root.file()).unwrap(), bytes);
    assert!(FileJournal::open(root.file(), identity(), minimum, Limits::default()).is_err());
}
