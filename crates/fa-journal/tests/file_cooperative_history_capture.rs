#![cfg(unix)]
#![forbid(unsafe_code)]
use fa_journal::file::{Error, FileJournal, FileRecovery};
use fa_journal::replay::{ReplayBudget, ReplayStatus};
use fa_journal::{Event, Head, Identity, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!("fa-cooperative-anchor-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf { self.0.join("journal") }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("capture cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 10 } }
fn reserve() -> RecoveryReserve { RecoveryReserve { records: 2, bytes: 60 } }
fn effect(payload: &[u8]) -> Effect {
    Effect { principal: "actor".into(), resolved_target: "sink".into(), payload: payload.to_vec(), units: 4 }
}
fn create(temp: &Temp, reserved: bool) -> FileJournal {
    if reserved {
        FileJournal::create_with_recovery_reserve(temp.path(), identity(), Limits::default(), reserve()).unwrap()
    } else {
        FileJournal::create(temp.path(), identity(), Limits::default()).unwrap()
    }
}
fn open(temp: &Temp, reserved: bool, head: Head) -> FileJournal {
    if reserved {
        FileJournal::open_with_recovery_reserve(temp.path(), identity(), head, Limits::default(), reserve()).unwrap()
    } else {
        FileJournal::open(temp.path(), identity(), head, Limits::default()).unwrap()
    }
}
fn unknown(owner: &mut FileJournal) {
    let payload = effect(b"proposal");
    for event in [Event::Reserve { id: 1, effect: payload.clone() },
        Event::Dispatch { id: 1, effect: payload }, Event::MarkUnknown { id: 1 }, Event::RevokeEpoch]
    { owner.commit(&event).unwrap(); }
}
fn ready(cursor: &mut FileRecovery, quantum: u64) {
    while cursor.progress().status == ReplayStatus::Reading {
        let report = cursor.advance(ReplayBudget { read_calls: 2, read_bytes: quantum, checked_records: 1 }).unwrap();
        assert!(report.spent.read_calls <= 2);
        assert!(report.spent.read_bytes <= quantum);
        assert!(report.spent.checked_records <= 1);
    }
}

#[test]
fn original_live_owner_captures_both_profiles_and_continues_writing_without_unlocking() {
    for reserved in [false, true] {
        for quantum in [1, 7, 4096] {
            let temp = Temp::new();
            let mut owner = create(&temp, reserved);
            unknown(&mut owner);
            let expected = owner.history_anchor().unwrap();
            let rights = owner.rights().unwrap().clone();
            let capacity = owner.capacity().unwrap();
            let mut cursor = owner.begin_history_anchor().unwrap();
            assert_eq!(cursor.advance(ReplayBudget::default()).unwrap().total.read_calls, 0);
            assert!(matches!(FileJournal::begin_open(temp.path(), identity(), expected.head(), Limits::default()), Err(Error::Busy)));
            ready(&mut cursor, quantum);
            assert_eq!(cursor.progress().total.read_bytes, expected.encoded_len());
            assert_eq!(cursor.progress().total.checked_records, expected.head().sequence);
            let (mut owner, captured) = cursor.finish_with_history_anchor().unwrap();
            assert_eq!(captured.as_bytes(), expected.as_bytes());
            assert_eq!(owner.rights().unwrap(), &rights);
            assert_eq!(owner.capacity().unwrap(), capacity);
            assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Unknown);
            assert!(matches!(FileJournal::begin_open(temp.path(), identity(), captured.head(), Limits::default()), Err(Error::Busy)));
            owner.commit(&Event::Reconcile { id: 1, occurred: true }).unwrap();
            assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Committed);
            assert_eq!(owner.rights().unwrap().available(), 6);
            assert!(owner.rights().unwrap().conserved());
        }
    }
}

#[test]
fn cancellation_releases_only_the_lock_and_preserves_unknown_charges_on_recovery() {
    for reserved in [false, true] {
        for bytes in [0, 1, 7, 64] {
            let temp = Temp::new();
            let mut owner = create(&temp, reserved);
            unknown(&mut owner);
            let head = owner.acknowledged_head();
            let original = fs::read(temp.path()).unwrap();
            let mut cursor = owner.begin_history_anchor().unwrap();
            let report = cursor.advance(ReplayBudget { read_calls: 3, read_bytes: bytes, checked_records: 1 }).unwrap();
            assert_eq!(cursor.cancel(), report.total);
            assert_eq!(fs::read(temp.path()).unwrap(), original);
            let owner = open(&temp, reserved, head);
            assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Unknown);
            assert_eq!(owner.rights().unwrap().available(), 6);
            assert_eq!(owner.acknowledged_head(), head);
        }
    }
}

#[test]
fn equal_crc_fork_cannot_replace_the_acknowledged_owners_actual_rights() {
    let left = Temp::new();
    let right = Temp::new();
    let mut owner = create(&left, false);
    let mut fork = create(&right, false);
    owner.commit(&Event::Reserve { id: 1, effect: effect(b"plumless") }).unwrap();
    fork.commit(&Event::Reserve { id: 1, effect: effect(b"buckeroo") }).unwrap();
    assert_eq!(owner.acknowledged_head(), fork.acknowledged_head());
    assert_ne!(owner.rights().unwrap(), fork.rights().unwrap());
    let retained = owner.history_anchor().unwrap();
    let bytes = fs::read(right.path()).unwrap();
    fs::write(left.path(), &bytes).unwrap();
    let mut cursor = owner.begin_history_anchor().unwrap();
    ready(&mut cursor, 1);
    assert!(matches!(cursor.finish_with_history_anchor(), Err(Error::Journal(fa_journal::Error::Binding))));
    assert_eq!(fs::read(left.path()).unwrap(), bytes);
    let mut permitted = fork.begin_history_anchor().unwrap();
    ready(&mut permitted, 1);
    assert_eq!(permitted.finish_with_history_anchor().unwrap().1.as_bytes(), bytes);
    // Selecting successor capture must not disable the independently retained
    // exact prefix, even though the legacy minimum head collides.
    let mut anchored = FileJournal::begin_open_anchored(
        left.path(), identity(), retained, Limits::default()).unwrap()
        .with_history_capture().unwrap();
    assert!(matches!(anchored.advance(ReplayBudget::UNLIMITED),
        Err(Error::Journal(fa_journal::Error::Binding))));
    assert!(matches!(anchored.finish_with_history_anchor(), Err(Error::Poisoned)));
}

#[test]
fn changed_extent_before_capture_or_after_ready_never_returns_an_archive() {
    for after_ready in [false, true] {
        let temp = Temp::new();
        let mut owner = create(&temp, false);
        unknown(&mut owner);
        let mut changed = fs::read(temp.path()).unwrap();
        changed.push(0);
        if after_ready {
            let mut cursor = owner.begin_history_anchor().unwrap();
            ready(&mut cursor, 7);
            fs::write(temp.path(), &changed).unwrap();
            assert!(matches!(cursor.finish_with_history_anchor(), Err(Error::Io(_))));
        } else {
            fs::write(temp.path(), &changed).unwrap();
            assert!(matches!(owner.begin_history_anchor(), Err(Error::Io(_))));
        }
        assert_eq!(fs::read(temp.path()).unwrap(), changed);
    }
}

#[test]
fn anchored_successor_recovery_captures_in_one_pass_and_can_reopen_the_successor() {
    for reserved in [false, true] {
        let temp = Temp::new();
        let mut owner = create(&temp, reserved);
        let old = owner.history_anchor().unwrap();
        unknown(&mut owner);
        let bytes = fs::read(temp.path()).unwrap();
        let expected = owner.history_anchor().unwrap();
        drop(owner);
        let mut cursor = if reserved {
            FileJournal::begin_open_anchored_with_recovery_reserve(
                temp.path(), identity(), old, Limits::default(), reserve()).unwrap()
        } else {
            FileJournal::begin_open_anchored(temp.path(), identity(), old, Limits::default()).unwrap()
        }.with_history_capture().unwrap();
        ready(&mut cursor, 7);
        assert_eq!(cursor.progress().total.read_bytes, bytes.len() as u64);
        let (owner, successor) = cursor.finish_with_history_anchor().unwrap();
        assert_eq!(successor.as_bytes(), expected.as_bytes());
        assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Unknown);
        drop(owner);
        let reopened = if reserved {
            FileJournal::open_anchored_with_recovery_reserve(
                temp.path(), identity(), successor, Limits::default(), reserve()).unwrap()
        } else {
            FileJournal::open_anchored(temp.path(), identity(), successor, Limits::default()).unwrap()
        };
        assert_eq!(reopened.acknowledged_head(), expected.head());
        assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    }
}

#[test]
fn late_or_missing_capture_selection_cannot_synthesize_an_archive() {
    let temp = Temp::new();
    let mut owner = create(&temp, false);
    unknown(&mut owner);
    let head = owner.acknowledged_head();
    drop(owner);
    let mut plain = FileJournal::begin_open(temp.path(), identity(), head, Limits::default()).unwrap();
    ready(&mut plain, 7);
    assert!(matches!(plain.finish_with_history_anchor(), Err(Error::Journal(fa_journal::Error::Binding))));
    let mut late = FileJournal::begin_open(temp.path(), identity(), head, Limits::default()).unwrap();
    late.advance(ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 0 }).unwrap();
    assert!(matches!(late.with_history_capture(), Err(Error::Journal(fa_journal::Error::InvalidInput))));
    let mut early = FileJournal::begin_open(temp.path(), identity(), head, Limits::default()).unwrap();
    early.advance(ReplayBudget::default()).unwrap();
    let mut early = early.with_history_capture().unwrap();
    ready(&mut early, 1);
    assert_eq!(early.finish_with_history_anchor().unwrap().1.as_bytes(), fs::read(temp.path()).unwrap());
}

#[test]
fn partial_capture_and_poisoned_owner_cannot_escape_as_healthy_accounting() {
    let temp = Temp::new();
    let mut owner = create(&temp, false);
    unknown(&mut owner);
    let head = owner.acknowledged_head();
    let cursor = owner.begin_history_anchor().unwrap();
    assert!(matches!(cursor.finish_with_history_anchor(), Err(Error::RecoveryIncomplete)));
    let mut owner = open(&temp, false, head);
    let mut damaged = fs::read(temp.path()).unwrap();
    damaged.push(0);
    fs::write(temp.path(), &damaged).unwrap();
    assert!(owner.commit(&Event::RevokeEpoch).is_err());
    assert!(owner.is_poisoned());
    assert!(matches!(owner.begin_history_anchor(), Err(Error::Poisoned)));
}
