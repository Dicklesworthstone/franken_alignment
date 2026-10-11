use super::*;
use fa_reference::{Effect, State};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        // create_dir is exclusive: collisions fail rather than reuse a directory.
        let path = std::env::temp_dir().join(format!(
            "fa-journal-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.join("rights.wal")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.path());
        let _ = fs::remove_dir(&self.0);
    }
}

fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 100 }
}

fn reservation() -> Event {
    Event::Reserve {
        id: 1,
        effect: Effect {
            principal: "alice".into(),
            resolved_target: "ledger://one".into(),
            payload: b"hello".to_vec(),
            units: 40,
        },
    }
}

fn injected() -> io::Error {
    io::Error::other("injected persistence failure")
}

#[test]
fn committed_state_survives_close_and_reopen() {
    let temp = Temp::new();
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let head = journal.commit(&reservation()).unwrap();
    assert_eq!(journal.rights().unwrap().available(), 60);
    drop(journal);
    let mut reopened = FileJournal::open(temp.path(), identity(), head, Limits::default()).unwrap();
    assert_eq!(reopened.acknowledged_head(), head);
    assert_eq!(reopened.rights().unwrap().state(1), Ok(State::Reserved));
    reopened.commit(&Event::AbortBeforeDispatch { id: 1 }).unwrap();
    assert_eq!(reopened.rights().unwrap().available(), 100);
}

#[test]
fn exclusive_lock_blocks_a_second_writer_until_drop() {
    let temp = Temp::new();
    let journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let head = journal.acknowledged_head();
    assert!(matches!(FileJournal::open(temp.path(), identity(), head, Limits::default()), Err(Error::Busy)));
    drop(journal);
    FileJournal::open(temp.path(), identity(), head, Limits::default()).unwrap();
}

#[test]
fn create_never_replaces_an_existing_file() {
    let temp = Temp::new();
    let journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let before = fs::read(temp.path()).unwrap();
    drop(journal);
    assert!(FileJournal::create(temp.path(), identity(), Limits::default()).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), before);
}

#[test]
fn invalid_transition_writes_nothing_and_does_not_poison() {
    let temp = Temp::new();
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let before = fs::read(temp.path()).unwrap();
    assert!(journal.commit_with(&Event::MarkUnknown { id: 99 }, |_, _| panic!("invalid event reached storage")).is_err());
    assert!(!journal.is_poisoned());
    assert_eq!(fs::read(temp.path()).unwrap(), before);
    journal.commit(&reservation()).unwrap();
}

#[test]
fn partial_write_poisoning_prevents_refunds_retries_and_silent_repair() {
    let temp = Temp::new();
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let head = journal.acknowledged_head();
    assert!(journal.commit_with(&reservation(), |file, bytes| {
        file.write_all(&bytes[..bytes.len() / 2])?;
        Err(injected())
    }).is_err());
    assert!(journal.is_poisoned());
    assert!(matches!(journal.rights(), Err(Error::Poisoned)));
    assert!(matches!(journal.commit(&reservation()), Err(Error::Poisoned)));
    assert_eq!(journal.acknowledged_head(), head);
    drop(journal);
    let damaged = fs::read(temp.path()).unwrap();
    assert!(FileJournal::open(temp.path(), identity(), head, Limits::default()).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), damaged);
}

#[test]
fn complete_unacknowledged_write_is_recovered_as_a_liability() {
    let temp = Temp::new();
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let head = journal.acknowledged_head();
    assert!(journal.commit_with(&reservation(), |file, bytes| {
        file.write_all(bytes)?;
        Err(injected()) // model a failed sync after the complete write
    }).is_err());
    assert!(matches!(journal.rights(), Err(Error::Poisoned)));
    drop(journal);
    let mut reopened = FileJournal::open(temp.path(), identity(), head, Limits::default()).unwrap();
    assert_eq!(reopened.acknowledged_head().sequence, head.sequence + 1);
    assert_eq!(reopened.rights().unwrap().available(), 60);
    assert!(reopened.commit(&reservation()).is_err()); // not safe to retry blindly
}

#[test]
fn panic_after_write_keeps_the_writer_poisoned() {
    let temp = Temp::new();
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = journal.commit_with(&reservation(), |file, bytes| {
            file.write_all(bytes)?;
            panic!("interrupted between write and acknowledgement");
        });
    }));
    assert!(result.is_err());
    assert!(journal.is_poisoned());
    assert!(matches!(journal.rights(), Err(Error::Poisoned)));
}

#[test]
fn missing_file_and_rollback_fail_without_recreating_or_truncating() {
    let temp = Temp::new();
    let genesis = Journal::new(identity(), Limits::default()).unwrap();
    assert!(FileJournal::open(temp.path(), identity(), genesis.head(), Limits::default()).is_err());
    assert!(!temp.path().exists());
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let head = journal.commit(&reservation()).unwrap();
    drop(journal);
    fs::write(temp.path(), genesis.header()).unwrap();
    assert!(matches!(FileJournal::open(temp.path(), identity(), head, Limits::default()), Err(Error::Journal(crate::Error::BehindAnchor))));
    assert_eq!(fs::read(temp.path()).unwrap(), genesis.header());
}

#[test]
fn extent_change_poisoning_prevents_appending_to_an_unreviewed_tail() {
    let temp = Temp::new();
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    // Simulate a noncooperating writer: advisory locks are not a security wall.
    OpenOptions::new().append(true).open(temp.path()).unwrap().write_all(b"unexpected").unwrap();
    let before = fs::read(temp.path()).unwrap();
    assert!(journal.commit(&reservation()).is_err());
    assert!(journal.is_poisoned());
    assert_eq!(fs::read(temp.path()).unwrap(), before);
}

#[test]
fn uncertain_dispatch_survives_restart_without_refund_or_reexecution() {
    let temp = Temp::new();
    let mut journal = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let reserve = reservation();
    let head = journal.commit(&reserve).unwrap();
    let Event::Reserve { id, effect } = reserve else { unreachable!() };
    let dispatch = Event::Dispatch { id, effect };
    assert!(journal.commit_with(&dispatch, |file, bytes| {
        file.write_all(bytes)?;
        Err(injected())
    }).is_err());
    drop(journal);
    let mut reopened = FileJournal::open(temp.path(), identity(), head, Limits::default()).unwrap();
    assert_eq!(reopened.rights().unwrap().state(1), Ok(State::Dispatched));
    assert_eq!(reopened.rights().unwrap().available(), 60);
    assert!(reopened.commit(&dispatch).is_err());
    assert!(reopened.commit(&Event::AbortBeforeDispatch { id: 1 }).is_err());
    reopened.commit(&Event::ResetToCheckpoint { keep: Default::default() }).unwrap();
    assert_eq!(reopened.rights().unwrap().available(), 60);
    reopened.commit(&Event::MarkUnknown { id: 1 }).unwrap();
    assert_eq!(reopened.rights().unwrap().state(1), Ok(State::Unknown));
}
