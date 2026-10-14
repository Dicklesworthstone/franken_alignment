//! Exercise resumption through the original private write/sync fault seams.
use super::*;
use crate::{Identity, Limits};
use crate::file::{BatchInspectionBudget, BatchInspectionStatus};
use fa_reference::{Effect, State};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-resumption-handoff-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("handoff test cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [8; 32], policy: [9; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "alice".into(), resolved_target: "ledger://handoff".into(),
        payload: vec![1, 2], units: 20 }
}
fn group() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect() }, Event::MarkUnknown { id: 1 }]
}
fn handoff(temp: &Temp, count: usize) -> FileBatchCommit {
    let mut owner = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    owner.commit(&Event::Reserve { id: 1, effect: effect() }).unwrap();
    let anchor = owner.batch_anchor().unwrap();
    for event in &group()[..count] { owner.commit(event).unwrap(); }
    let mut run = owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap();
    for _ in 0..3 { run.advance(BatchInspectionBudget::UNLIMITED).unwrap(); }
    assert_eq!(run.progress().status, BatchInspectionStatus::Ready);
    assert_eq!(run.progress().total.prepared_records, 2);
    run.resume().unwrap()
}
fn fault() -> io::Error { io::Error::other("injected original resumption I/O") }

#[test]
fn complete_handoff_never_calls_write_or_sync_but_new_suffix_syncs_once_under_lock() {
    for count in 0..=2 {
        let temp = Temp::new(); let mut run = handoff(&temp, count);
        let old = run.acknowledged_head();
        assert!(run.preparation.is_none()); assert_eq!(run.progress().preparation_attempts, 0);
        if count == 2 {
            assert!(run.already_acknowledged);
            run.advance_with(BatchCommitBudget::UNLIMITED, |_, _| panic!("complete group wrote again")).unwrap();
        } else {
            assert!(!run.already_acknowledged);
            run.advance(BatchCommitBudget::UNLIMITED).unwrap();
        }
        let before = fs::read(temp.path()).unwrap(); let mut syncs = 0;
        let owner = run.finish_with(|file| {
            syncs += 1;
            assert!(matches!(FileJournal::open(temp.path(), identity(), old, Limits::default()), Err(Error::Busy)));
            file.sync_all()
        }).unwrap();
        assert_eq!(syncs, usize::from(count < 2));
        assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(owner.rights().unwrap().available(), 80);
        assert_eq!(fs::read(temp.path()).unwrap(), before);
    }
}

#[test]
fn resumed_suffix_retains_byte_offset_and_charges_interrupted_original_writes() {
    let temp = Temp::new(); let mut run = handoff(&temp, 1);
    let before = fs::read(temp.path()).unwrap(); let suffix = run.bytes.clone();
    let old = run.acknowledged_head();
    run.advance_with(BatchCommitBudget { write_calls: 3, ..BatchCommitBudget::UNLIMITED },
        |_, _| Err(io::ErrorKind::Interrupted.into())).unwrap();
    assert_eq!(run.progress().write_calls, 3); assert_eq!(run.progress().written_bytes, 0);
    assert_eq!(fs::read(temp.path()).unwrap(), before);
    run.advance_with(BatchCommitBudget { prepared_records: 0, write_calls: 4, write_bytes: 2 },
        |file, bytes| file.write(&bytes[..1])).unwrap();
    assert_eq!(run.progress().write_calls, 5); assert_eq!(run.progress().written_bytes, 2);
    assert_eq!(run.acknowledged_head(), old);
    run.advance(BatchCommitBudget::UNLIMITED).unwrap();
    assert_eq!(run.progress().preparation_attempts, 0);
    let owner = run.finish().unwrap();
    assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
    assert_eq!(fs::read(temp.path()).unwrap(), [before, suffix].concat());
}

#[test]
fn resumption_write_error_or_unwind_keeps_old_head_and_no_usable_accounting() {
    for unwind in [false, true] {
        let temp = Temp::new(); let mut run = handoff(&temp, 1); let old = run.acknowledged_head();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run.advance_with(BatchCommitBudget::UNLIMITED, |file, bytes| {
                file.write_all(bytes)?;
                if unwind { panic!("resumption write returned no count"); }
                Err(fault())
            })
        }));
        if unwind { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        assert_eq!(run.progress().stage, BatchCommitStage::Failed);
        assert_eq!(run.progress().write_calls, 1); assert_eq!(run.progress().written_bytes, 0);
        assert_eq!(run.acknowledged_head(), old);
        assert!(matches!(run.owner.rights(), Err(Error::Poisoned)));
        assert!(matches!(run.finish(), Err(Error::Poisoned)));
        let owner = FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap();
        assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(owner.rights().unwrap().available(), 80);
    }
}

#[test]
fn failed_resumption_sync_exposes_no_owner_and_complete_strict_reopen_does_not_refund() {
    for unwind in [false, true] {
        let temp = Temp::new(); let mut run = handoff(&temp, 1); let old = run.acknowledged_head();
        run.advance(BatchCommitBudget::UNLIMITED).unwrap();
        let before = fs::read(temp.path()).unwrap(); let mut syncs = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.finish_with(|_| {
            syncs += 1;
            if unwind { panic!("resumption sync unwind"); }
            Err(fault())
        })));
        assert_eq!(syncs, 1);
        if unwind { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        let owner = FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap();
        assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(owner.rights().unwrap().available(), 80);
        assert_eq!(fs::read(temp.path()).unwrap(), before);
    }
}
