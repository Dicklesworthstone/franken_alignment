//! Exercise the original uncertain commit and recovery synchronization boundaries.
use super::*;
use crate::file::{BatchCommitBudget, BatchCommitStage, BatchInspectionBudget};
use fa_reference::{Effect, State};
use std::cell::Cell;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fa-direct-batch-fault-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
}
impl Drop for Temp {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 10 } }
fn effect() -> Effect {
    Effect { principal: "actor".into(), resolved_target: "publication".into(), payload: vec![7], units: 4 }
}
fn group() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect() }, Event::MarkUnknown { id: 1 },
        Event::RevokeEpoch, Event::Reconcile { id: 1, occurred: true }]
}
fn setup(stored: usize) -> (Temp, HistoryAnchor, Journal, Vec<u8>) {
    let temp = Temp::new();
    let mut owner = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    owner.commit(&Event::Reserve { id: 1, effect: effect() }).unwrap();
    let history = owner.history_anchor().unwrap();
    let mut oracle = Journal::replay(history.as_bytes(), identity(), history.head(), Limits::default()).unwrap();
    let mut suffix = Vec::new();
    let mut cut = 0;
    for (index, event) in group().iter().enumerate() {
        suffix.extend(oracle.append(event).unwrap());
        if index < stored { cut = suffix.len(); }
    }
    let minimum = owner.acknowledged_head();
    let attempted = owner.commit_batch_with(&group(), BatchLimits::default(), |file, bytes| {
        assert_eq!(bytes, suffix);
        file.write_all(&bytes[..cut])?;
        Err(io::ErrorKind::PermissionDenied.into())
    });
    assert!(matches!(attempted, Err(Error::Io(_))));
    assert!(owner.is_poisoned());
    assert_eq!(owner.acknowledged_head(), minimum);
    assert!(matches!(owner.rights(), Err(Error::Poisoned)));
    let mut complete = history.as_bytes().to_vec();
    complete.extend(suffix);
    drop(owner);
    (temp, history, oracle, complete)
}
fn begin(temp: &Temp, history: &HistoryAnchor) -> FileBatchInspection {
    FileJournal::begin_recover_batch_anchored(temp.path(), identity(), history.clone(),
        Limits::default(), group(), BatchLimits::default()).unwrap()
}
fn ready(run: &mut FileBatchInspection) {
    for _ in 0..8 {
        if run.advance(BatchInspectionBudget::UNLIMITED).unwrap().status == BatchInspectionStatus::Ready { return; }
    }
    panic!("original stages did not finish");
}

#[test]
fn actual_uncertain_batch_commits_recover_without_a_preliminary_reopen() {
    for stored in 0..=4 {
        let (temp, history, oracle, complete) = setup(stored);
        let old_bytes = fs::read(temp.path()).unwrap();
        let mut run = begin(&temp, &history);
        ready(&mut run);
        assert_eq!(run.progress().total.replay.read_bytes, history.encoded_len());
        assert_eq!(run.progress().total.replay.read_bytes + run.progress().total.compare_bytes, old_bytes.len() as u64);
        assert_eq!(run.acknowledged_head(), history.head());
        let mut writer = run.resume().unwrap();
        while writer.progress().stage != BatchCommitStage::Ready {
            writer.advance(BatchCommitBudget { prepared_records: 0, write_calls: 1, write_bytes: 3 }).unwrap();
        }
        let owner = writer.finish().unwrap();
        assert_eq!(owner.acknowledged_head(), oracle.head());
        assert_eq!(owner.rights().unwrap(), oracle.rights());
        assert_eq!(fs::read(temp.path()).unwrap(), complete);
    }
}

#[test]
fn every_recovery_sync_failure_or_unwind_returns_no_owner_even_for_complete_groups() {
    for stored in [0, 2, 4] {
        for read_only in [false, true] {
            for fail_after in 0..=2 {
                for unwind in [false, true] {
                    let (temp, history, _, _) = setup(stored);
                    let bytes = fs::read(temp.path()).unwrap();
                    let mut run = begin(&temp, &history);
                    ready(&mut run);
                    let reached = Cell::new(0);
                    let sync = |file: &std::fs::File, directory: &std::fs::File| -> io::Result<()> {
                        reached.set(1);
                        if fail_after >= 1 { file.sync_all()?; reached.set(2); }
                        if fail_after >= 2 { directory.sync_all()?; reached.set(3); }
                        assert!(matches!(FileJournal::begin_recover_batch_anchored(temp.path(), identity(),
                            history.clone(), Limits::default(), group(), BatchLimits::default()), Err(Error::Busy)));
                        if unwind { panic!("injected recovery synchronization unwind"); }
                        Err(io::ErrorKind::PermissionDenied.into())
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if read_only { run.finish_with_sync(sync).map(|_| ()) }
                        else { run.resume_with_sync(sync).map(|_| ()) }
                    }));
                    if unwind { assert!(result.is_err()); }
                    else { assert!(matches!(result.unwrap(), Err(Error::Io(_)))); }
                    assert_eq!(reached.get(), fail_after + 1);
                    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
                    let mut retry = begin(&temp, &history);
                    ready(&mut retry);
                    let (owner, status) = retry.finish().unwrap();
                    assert_eq!(status.persisted_records(), stored);
                    assert_eq!(owner.rights().unwrap().available(), 6);
                    assert!(owner.rights().unwrap().conserved());
                }
            }
        }
    }
}

#[test]
fn existing_acknowledged_inspection_and_resumption_do_not_gain_new_sync_barriers() {
    for read_only in [false, true] {
        let (temp, history, _, _) = setup(4);
        let owner = FileJournal::open_anchored(temp.path(), identity(), history.clone(), Limits::default()).unwrap();
        let mut run = if read_only {
            owner.begin_inspect_batch_anchored(history, group(), BatchLimits::default())
        } else { owner.begin_resume_batch_anchored(history, group(), BatchLimits::default()) }.unwrap();
        ready(&mut run);
        let owner = if read_only {
            run.finish_with_sync(|_, _| panic!("already acknowledged inspection synchronized")).unwrap().0
        } else {
            run.resume_with_sync(|_, _| panic!("already acknowledged resumption synchronized")).unwrap().finish().unwrap()
        };
        assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Committed);
    }
}

#[test]
fn changed_extent_during_final_sync_cannot_publish_a_stale_recovered_owner() {
    for read_only in [false, true] {
        let (temp, history, _, _) = setup(2);
        let mut run = begin(&temp, &history);
        ready(&mut run);
        let sync = |file: &std::fs::File, directory: &std::fs::File| -> io::Result<()> {
            file.sync_all()?;
            OpenOptions::new().append(true).open(temp.path())?.write_all(&[0])?;
            directory.sync_all()
        };
        if read_only { assert!(matches!(run.finish_with_sync(sync), Err(Error::Io(_)))); }
        else { assert!(matches!(run.resume_with_sync(sync), Err(Error::Io(_)))); }
        let mut retry = begin(&temp, &history);
        assert_eq!(retry.advance(BatchInspectionBudget::UNLIMITED).unwrap().status, BatchInspectionStatus::PrepareBatch);
        assert!(retry.advance(BatchInspectionBudget::UNLIMITED).is_err());
        assert!(matches!(retry.finish(), Err(Error::Poisoned)));
    }
}

#[test]
fn incomplete_or_failed_recovery_never_calls_the_sync_hook() {
    for failed in [false, true] {
        for read_only in [false, true] {
            let (temp, history, _, _) = setup(2);
            let mut events = group();
            if failed { events.push(Event::MarkUnknown { id: 99 }); }
            let mut run = FileJournal::begin_recover_batch_anchored(temp.path(), identity(), history.clone(),
                Limits::default(), events, BatchLimits::default()).unwrap();
            if failed {
                run.advance(BatchInspectionBudget::UNLIMITED).unwrap();
                assert!(run.advance(BatchInspectionBudget::UNLIMITED).is_err());
            }
            let result = if read_only {
                run.finish_with_sync(|_, _| panic!("unverified recovery synchronized")).map(|_| ())
            } else {
                run.resume_with_sync(|_, _| panic!("unverified resumption synchronized")).map(|_| ())
            };
            if failed { assert!(matches!(result, Err(Error::Poisoned))); }
            else { assert!(matches!(result, Err(Error::RecoveryIncomplete))); }
            let mut retry = begin(&temp, &history);
            ready(&mut retry);
            let (owner, status) = retry.finish().unwrap();
            assert_eq!(status.persisted_records(), 2);
            assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Unknown);
            assert_eq!(owner.rights().unwrap().available(), 6);
        }
    }
}

#[test]
fn direct_recovery_retains_original_interruption_and_unwind_latches() {
    for fault in [false, true] {
        let (temp, history, _, _) = setup(2);
        let bytes = fs::read(temp.path()).unwrap();
        let mut run = begin(&temp, &history);
        run.advance(BatchInspectionBudget::UNLIMITED).unwrap();
        run.advance(BatchInspectionBudget::UNLIMITED).unwrap();
        assert_eq!(run.progress().status, BatchInspectionStatus::CompareSuffix);
        let budget = BatchInspectionBudget { compare_calls: 3, compare_bytes: 10,
            ..BatchInspectionBudget::default() };
        let report = run.advance_with(budget, |_, _| Err(io::ErrorKind::Interrupted.into())).unwrap();
        assert_eq!(report.total.compare_calls, 3);
        assert_eq!(report.total.compare_bytes, 0);
        if fault {
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run.advance_with(budget, |_, _| panic!("injected stored-suffix read unwind"))
            })).is_err());
            assert_eq!(run.progress().status, BatchInspectionStatus::Failed);
            assert_eq!(run.progress().total.compare_calls, 4);
            assert!(matches!(run.resume(), Err(Error::Poisoned)));
        } else {
            ready(&mut run);
            assert_eq!(run.finish().unwrap().0.rights().unwrap().state(1).unwrap(), State::Unknown);
        }
        assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    }
}
