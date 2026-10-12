#![cfg(unix)]
use fa_journal::{Event, Head, Identity, Journal, Limits, RecoveryReserve};
use fa_journal::batch::BatchLimits;
use fa_journal::file::{Error, FileJournal, FileRecovery};
use fa_journal::replay::{ReplayBudget, ReplayStatus};
use fa_reference::{Effect, State};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-reserved-journal-{}-{nanos}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("journal") }
    fn bytes(&self) -> Vec<u8> { std::fs::read(self.path()).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("reserve fixture cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn limits() -> Limits { Limits { max_frame_bytes: 1024, max_records: 6, max_total_bytes: 1024 } }
fn reserve() -> RecoveryReserve { RecoveryReserve { records: 4, bytes: 120 } }
fn effect() -> Effect {
    Effect { principal: "actor".into(), resolved_target: "disposable".into(), payload: b"exact".to_vec(), units: 40 }
}
fn ordinary() -> [Event; 2] {
    [Event::Reserve { id: 1, effect: effect() }, Event::Dispatch { id: 1, effect: effect() }]
}
fn tail(occurred: bool) -> [Event; 4] {
    [Event::RevokeEpoch, Event::MarkUnknown { id: 1 }, Event::Reconcile { id: 1, occurred }, Event::RevokeEpoch]
}
fn full(root: &Directory) -> (FileJournal, Head) {
    let mut writer = FileJournal::create_with_recovery_reserve(root.path(), identity(), limits(), reserve()).unwrap();
    let anchor = writer.commit_batch(&ordinary(), BatchLimits::default()).unwrap();
    assert_eq!(writer.capacity().unwrap().ordinary.records, 0); (writer, anchor)
}
fn ready(mut replay: FileRecovery) -> FileRecovery {
    for _ in 0..10_000 {
        let report = replay.advance(ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 1 }).unwrap();
        assert!(report.spent.read_bytes <= 1 && report.spent.checked_records <= 1);
        if report.status == ReplayStatus::Ready { return replay; }
    }
    panic!("file fixture did not finish");
}
fn limit(result: Result<Head, Error>) { assert!(matches!(result, Err(Error::Journal(fa_journal::Error::Limit)))); }

#[test]
fn first_image_pins_the_reserve_and_invalid_configuration_never_creates_a_file() {
    let root = Directory::new();
    assert!(FileJournal::create_with_recovery_reserve(root.path(), identity(), limits(),
        RecoveryReserve { records: 0, bytes: 120 }).is_err());
    assert!(!root.path().exists());
    let writer = FileJournal::create_with_recovery_reserve(root.path(), identity(), limits(), reserve()).unwrap();
    let expected = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
    assert_eq!(root.bytes(), expected.header()); assert_eq!(root.bytes().len(), 120);
    assert_eq!(writer.acknowledged_head(), expected.head());
    assert_eq!(writer.capacity().unwrap(), expected.capacity());
    assert!(FileJournal::create_with_recovery_reserve(root.path(), identity(), limits(), reserve()).is_err());
    assert_eq!(root.bytes(), expected.header());
}

#[test]
fn exhausted_ordinary_space_survives_each_reopen_while_original_obligations_settle() {
    for occurred in [false, true] {
        let root = Directory::new(); let (mut writer, _) = full(&root);
        for event in tail(occurred) {
            let before = root.bytes();
            limit(writer.commit(&Event::Reserve { id: 2, effect: effect() }));
            assert!(!writer.is_poisoned()); assert_eq!(root.bytes(), before);
            let head = writer.commit(&event).unwrap(); let capacity = writer.capacity().unwrap();
            let rights = writer.rights().unwrap().clone(); drop(writer);
            writer = FileJournal::open_with_recovery_reserve(root.path(), identity(), head, limits(), reserve()).unwrap();
            assert_eq!(writer.rights().unwrap(), &rights); assert_eq!(writer.capacity().unwrap(), capacity);
            assert_eq!(writer.capacity().unwrap().ordinary.records, 0);
        }
        assert_eq!(writer.rights().unwrap().state(1), Ok(if occurred { State::Committed } else { State::Aborted }));
        assert_eq!(writer.rights().unwrap().available(), if occurred { 60 } else { 100 });
        assert_eq!(writer.rights().unwrap().epoch(), 2);
        assert_eq!(writer.capacity().unwrap().recovery.records, 0);
        limit(writer.commit(&Event::RevokeEpoch));
    }
}

#[test]
fn an_exact_byte_tail_remains_available_after_cooperative_file_recovery() {
    let mut oracle = Journal::new(identity(), Limits::default()).unwrap();
    let ordinary_bytes = oracle.append_batch(&ordinary(), BatchLimits::default()).unwrap().len() as u64;
    let cap = Limits { max_records: 20, max_total_bytes: 120 + ordinary_bytes + 101, ..limits() };
    let reserve = RecoveryReserve { records: 4, bytes: 101 }; let root = Directory::new();
    let mut writer = FileJournal::create_with_recovery_reserve(root.path(), identity(), cap, reserve).unwrap();
    let anchor = writer.commit_batch(&ordinary(), BatchLimits::default()).unwrap(); drop(writer);
    let mut writer = ready(FileJournal::begin_open_with_recovery_reserve(root.path(), identity(), anchor, cap, reserve).unwrap()).finish().unwrap();
    assert_eq!(writer.capacity().unwrap().ordinary.bytes, 0);
    limit(writer.commit(&Event::Reserve { id: 2, effect: effect() }));
    writer.commit_batch(&tail(false), BatchLimits::default()).unwrap();
    assert_eq!(writer.capacity().unwrap().recovery.bytes, 0);
    assert_eq!(writer.rights().unwrap().available(), 100);
    let head = writer.acknowledged_head(); drop(writer);
    let mut writer = FileJournal::open_with_recovery_reserve(root.path(), identity(), head, cap, reserve).unwrap();
    limit(writer.commit(&Event::RevokeEpoch)); assert_eq!(root.bytes().len() as u64, cap.max_total_bytes);
}

#[test]
fn altered_profile_and_legacy_downgrade_refuse_without_repair_or_a_lock_leak() {
    let root = Directory::new(); let (writer, anchor) = full(&root); drop(writer); let before = root.bytes();
    for (cap, reserve) in [
        (Limits { max_records: 7, ..limits() }, reserve()),
        (Limits { max_total_bytes: 2048, ..limits() }, reserve()),
        (Limits { max_frame_bytes: 2048, ..limits() }, reserve()),
        (limits(), RecoveryReserve { records: 3, ..reserve() }),
        (limits(), RecoveryReserve { bytes: 119, ..reserve() }),
    ] {
        assert!(matches!(FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, cap, reserve),
            Err(Error::Journal(fa_journal::Error::Binding))));
        assert_eq!(root.bytes(), before);
    }
    assert!(matches!(FileJournal::open(root.path(), identity(), anchor, limits()), Err(Error::Journal(fa_journal::Error::Binding))));
    assert_eq!(root.bytes(), before);
    let writer = FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()).unwrap();
    assert_eq!(writer.capacity().unwrap().ordinary.records, 0); assert_eq!(root.bytes(), before);
}

#[test]
fn cancellation_at_header_boundaries_and_ready_keeps_bytes_and_releases_the_original_lock() {
    let root = Directory::new(); let (writer, anchor) = full(&root); drop(writer); let before = root.bytes();
    for bytes in [0, 1, 7, 8, 83, 84, 119, 120] {
        let mut run = FileJournal::begin_open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()).unwrap();
        run.advance(ReplayBudget { read_calls: u64::MAX, read_bytes: bytes, checked_records: 1 }).unwrap();
        assert!(matches!(FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()), Err(Error::Busy)));
        let work = run.progress().total; assert_eq!(run.cancel(), work); assert_eq!(root.bytes(), before);
    }
    let run = ready(FileJournal::begin_open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()).unwrap());
    assert!(matches!(FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()), Err(Error::Busy)));
    run.cancel(); assert_eq!(root.bytes(), before);
    let run = FileJournal::begin_open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()).unwrap();
    assert!(matches!(run.finish(), Err(Error::RecoveryIncomplete)));
    FileJournal::open_with_recovery_reserve(root.path(), identity(), anchor, limits(), reserve()).unwrap();
}

#[test]
fn invalid_group_cannot_spend_the_tail_and_acknowledged_tail_cannot_be_rolled_back() {
    let root = Directory::new(); let (mut writer, _) = full(&root); let before = root.bytes();
    limit(writer.commit_batch(&[Event::RevokeEpoch, Event::Reserve { id: 2, effect: effect() }], BatchLimits::default()));
    assert_eq!(root.bytes(), before); assert_eq!(writer.rights().unwrap().epoch(), 0);
    assert!(!writer.is_poisoned());
    let head = writer.commit_batch(&tail(true), BatchLimits::default()).unwrap(); drop(writer);
    // Deliberately restore an older trusted-directory image; the independent
    // acknowledged final head, not an internal group prefix, is the minimum.
    std::fs::write(root.path(), &before).unwrap();
    assert!(matches!(FileJournal::open_with_recovery_reserve(root.path(), identity(), head, limits(), reserve()),
        Err(Error::Journal(fa_journal::Error::BehindAnchor))));
    assert_eq!(root.bytes(), before);
}

#[test]
fn available_ordinary_capacity_does_not_waive_the_original_revoked_epoch_check() {
    let root = Directory::new(); let cap = Limits { max_records: 20, ..limits() };
    let mut writer = FileJournal::create_with_recovery_reserve(root.path(), identity(), cap, reserve()).unwrap();
    writer.commit(&ordinary()[0]).unwrap(); let head = writer.commit(&Event::RevokeEpoch).unwrap();
    drop(writer);
    let mut writer = FileJournal::open_with_recovery_reserve(root.path(), identity(), head, cap, reserve()).unwrap();
    assert!(writer.capacity().unwrap().ordinary.records > 0); let before = root.bytes();
    assert!(matches!(writer.commit(&ordinary()[1]), Err(Error::Journal(fa_journal::Error::Model(fa_reference::Error::Stale)))));
    assert_eq!(root.bytes(), before); assert!(!writer.is_poisoned());
    writer.commit(&Event::AbortBeforeDispatch { id: 1 }).unwrap();
    writer.commit(&Event::Reserve { id: 2, effect: effect() }).unwrap();
    writer.commit(&Event::Dispatch { id: 2, effect: effect() }).unwrap();
    writer.commit(&Event::Reconcile { id: 2, occurred: true }).unwrap();
    assert_eq!(writer.rights().unwrap().state(1), Ok(State::Aborted));
    assert_eq!(writer.rights().unwrap().state(2), Ok(State::Committed));
    assert_eq!(writer.rights().unwrap().available(), 60); assert!(writer.rights().unwrap().conserved());
}
