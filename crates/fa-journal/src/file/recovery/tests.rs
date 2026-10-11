use super::*;
use crate::Event;
use crate::replay::ReplayStatus;
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { root: PathBuf, head: Head }
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-recovery-sync-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        let mut writer = FileJournal::create(root.join("journal"), identity(), Limits::default()).unwrap();
        let head = writer.commit(&Event::RevokeEpoch).unwrap();
        Self { root, head }
    }
    fn open(&self) -> FileRecovery {
        FileJournal::begin_open(self.root.join("journal"), identity(), self.head, Limits::default()).unwrap()
    }
    fn ready(&self) -> FileRecovery {
        let mut run = self.open();
        assert_eq!(run.advance(ReplayBudget::UNLIMITED).unwrap().status, ReplayStatus::Ready);
        run
    }
    fn readable_again(&self) {
        let writer = FileJournal::open(self.root.join("journal"), identity(), self.head, Limits::default()).unwrap();
        assert_eq!(writer.acknowledged_head(), self.head);
        assert_eq!(writer.rights().unwrap().epoch(), 1);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) { eprintln!("sync fixture cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }

#[test]
fn failed_file_or_directory_sync_returns_no_writer_and_keeps_the_lock_through_the_barrier() {
    for after_file_sync in [false, true] {
        let fixture = Fixture::new();
        let bytes = std::fs::read(fixture.root.join("journal")).unwrap();
        let result = fixture.ready().finish_with(|file, _directory| {
            assert!(matches!(FileJournal::open(fixture.root.join("journal"), identity(),
                fixture.head, Limits::default()), Err(Error::Busy)));
            if after_file_sync { file.sync_all()?; }
            Err(io::Error::other("injected sync failure"))
        });
        assert!(matches!(result, Err(Error::Io(_))));
        assert_eq!(std::fs::read(fixture.root.join("journal")).unwrap(), bytes);
        fixture.readable_again();
    }
}

#[test]
fn incomplete_recovery_cannot_call_the_final_sync_hook() {
    let fixture = Fixture::new();
    let calls = Cell::new(0);
    let result = fixture.open().finish_with(|_, _| { calls.set(calls.get() + 1); Ok(()) });
    assert!(matches!(result, Err(Error::RecoveryIncomplete)));
    assert_eq!(calls.get(), 0);
    fixture.readable_again();
}

#[test]
fn unwind_during_final_sync_releases_the_lock_without_exporting_accounting() {
    let fixture = Fixture::new();
    let bytes = std::fs::read(fixture.root.join("journal")).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fixture.ready().finish_with(|file, _| {
            file.sync_all()?;
            panic!("injected final-sync unwind");
        })
    }));
    assert!(result.is_err());
    assert_eq!(std::fs::read(fixture.root.join("journal")).unwrap(), bytes);
    fixture.readable_again();
}
