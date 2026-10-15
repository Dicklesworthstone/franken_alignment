use super::*;
use crate::Event;
use crate::replay::ReplayStatus;
use fa_reference::{Effect, State};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture { root: PathBuf }
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!("fa-capture-barriers-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        Self { root }
    }
    fn path(&self) -> PathBuf { self.root.join("journal") }
    fn create(&self) -> FileJournal {
        let mut owner = FileJournal::create(self.path(), identity(), Limits::default()).unwrap();
        owner.commit(&Event::Reserve { id: 1, effect: effect() }).unwrap();
        owner
    }
    fn ready(&self, minimum: Head) -> FileRecovery {
        let mut run = FileJournal::begin_open(self.path(), identity(), minimum, Limits::default())
            .unwrap().with_history_capture().unwrap();
        assert_eq!(run.advance(ReplayBudget::UNLIMITED).unwrap().status, ReplayStatus::Ready);
        run
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) { eprintln!("barrier cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 10 } }
fn effect() -> Effect {
    Effect { principal: "actor".into(), resolved_target: "sink".into(), payload: b"message".to_vec(), units: 4 }
}

#[test]
fn capture_recovery_sync_failures_and_unwinds_never_publish_the_private_archive() {
    for completed_barriers in 0..=2 {
        for unwind in [false, true] {
            let fixture = Fixture::new();
            let owner = fixture.create();
            let head = owner.acknowledged_head();
            drop(owner);
            let bytes = std::fs::read(fixture.path()).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fixture.ready(head).finish_checked(true, |file, directory| {
                    assert!(matches!(FileJournal::begin_open(fixture.path(), identity(), head,
                        Limits::default()), Err(Error::Busy)));
                    if completed_barriers >= 1 { file.sync_all()?; }
                    if completed_barriers >= 2 { directory.sync_all()?; }
                    if unwind { panic!("injected capture synchronization unwind"); }
                    Err(io::Error::other("injected capture synchronization failure"))
                })
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(result, Ok(Err(Error::Io(_)))));
            }
            assert_eq!(std::fs::read(fixture.path()).unwrap(), bytes);
            let (owner, archive) = fixture.ready(head).finish_with_history_anchor().unwrap();
            assert_eq!(archive.as_bytes(), bytes);
            assert_eq!(owner.acknowledged_head(), head);
        }
    }
}

#[test]
fn acknowledged_capture_needs_no_new_sync_but_incomplete_capture_has_no_completion() {
    let fixture = Fixture::new();
    let owner = fixture.create();
    let head = owner.acknowledged_head();
    let cursor = owner.begin_history_anchor().unwrap();
    assert!(matches!(cursor.finish_checked(true, |_, _| panic!("incomplete must not sync")),
        Err(Error::RecoveryIncomplete)));
    let owner = FileJournal::open(fixture.path(), identity(), head, Limits::default()).unwrap();
    let mut cursor = owner.begin_history_anchor().unwrap();
    cursor.advance(ReplayBudget::UNLIMITED).unwrap();
    let (mut owner, archive) = cursor.finish_checked(true, |_, _| panic!("healthy capture must not resync")).unwrap();
    assert_eq!(archive.unwrap().as_bytes(), std::fs::read(fixture.path()).unwrap());
    owner.commit(&Event::Dispatch { id: 1, effect: effect() }).unwrap();
    assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Dispatched);
}

#[test]
fn extent_change_during_successful_sync_is_detected_before_returning_owner_or_anchor() {
    for retain_anchor in [false, true] {
        let fixture = Fixture::new();
        let owner = fixture.create();
        let head = owner.acknowledged_head();
        drop(owner);
        let result = fixture.ready(head).finish_checked(retain_anchor, |file, directory| {
            file.sync_all()?;
            directory.sync_all()?;
            file.set_len(file.metadata()?.len() + 1)?;
            Ok(())
        });
        assert!(matches!(result, Err(Error::Io(_))));
        assert!(FileJournal::open(fixture.path(), identity(), head, Limits::default()).is_err());
    }
}

#[test]
fn uncertain_original_commit_cannot_be_captured_until_original_recovery_acknowledges_it() {
    let fixture = Fixture::new();
    let mut owner = fixture.create();
    let minimum = owner.acknowledged_head();
    assert!(owner.commit_with(&Event::Dispatch { id: 1, effect: effect() }, |file, bytes| {
        file.write_all(bytes)?;
        Err(io::Error::other("complete record written, acknowledgment lost"))
    }).is_err());
    assert!(owner.is_poisoned());
    assert_eq!(owner.acknowledged_head(), minimum);
    assert!(matches!(owner.begin_history_anchor(), Err(Error::Poisoned)));
    let (owner, archive) = fixture.ready(minimum).finish_with_history_anchor().unwrap();
    assert_eq!(owner.acknowledged_head().sequence, minimum.sequence + 1);
    assert_eq!(archive.head(), owner.acknowledged_head());
    assert_eq!(archive.as_bytes(), std::fs::read(fixture.path()).unwrap());
    assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Dispatched);
    assert_eq!(owner.rights().unwrap().available(), 6);
    assert!(owner.rights().unwrap().conserved());
}
