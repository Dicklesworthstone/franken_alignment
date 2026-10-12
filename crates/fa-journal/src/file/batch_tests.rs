//! Inject faults at the ORIGINAL locked writer's single persistence boundary.
use super::*;
use fa_reference::{Effect, State};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-group-fault-{}-{}-{}",
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
fn seed() -> Vec<Event> {
    vec![Event::Reserve { id: 1, effect: effect() }, Event::Reserve { id: 2, effect: effect() }]
}
fn group() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect() }, Event::Dispatch { id: 2, effect: effect() },
        Event::MarkUnknown { id: 1 }, Event::RevokeEpoch]
}
fn owner(temp: &Temp) -> FileJournal {
    let mut file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    for event in seed() { file.commit(&event).unwrap(); }
    file
}
fn original() -> Journal {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    for event in seed() { journal.append(&event).unwrap(); }
    journal
}
fn fault() -> io::Error { io::Error::other("injected group persistence failure") }

#[test]
fn every_byte_cut_poisoning_recovers_only_complete_original_records_without_refunds() {
    let mut expected = original();
    let mut wire = Vec::new();
    let mut complete = std::collections::BTreeMap::from([(0, expected.clone())]);
    for event in group() {
        wire.extend(expected.append(&event).unwrap());
        complete.insert(wire.len(), expected.clone());
    }
    for end in 0..=wire.len() {
        let temp = Temp::new(); let mut file = owner(&temp);
        let anchor = file.acknowledged_head(); let before = fs::read(temp.path()).unwrap();
        let result = file.commit_batch_with(&group(), BatchLimits::default(), |handle, bytes| {
            assert_eq!(bytes, wire);
            handle.write_all(&bytes[..end])?;
            handle.sync_all()?; // Make this exact injected prefix observable on reopen.
            Err(fault())
        });
        assert!(result.is_err(), "cut {end}");
        assert!(file.is_poisoned());
        assert_eq!(file.acknowledged_head(), anchor);
        assert!(matches!(file.rights(), Err(Error::Poisoned)));
        assert!(matches!(file.commit(&Event::RevokeEpoch), Err(Error::Poisoned)));
        assert!(matches!(file.commit_batch(&group(), BatchLimits::default()), Err(Error::Poisoned)));
        assert!(matches!(FileJournal::open(temp.path(), identity(), anchor, Limits::default()), Err(Error::Busy)));
        drop(file);
        let mut expected_bytes = before; expected_bytes.extend_from_slice(&wire[..end]);
        assert_eq!(fs::read(temp.path()).unwrap(), expected_bytes);
        let reopened = FileJournal::open(temp.path(), identity(), anchor, Limits::default());
        if let Some(expected) = complete.get(&end) {
            let mut reopened = reopened.unwrap();
            assert_eq!(reopened.acknowledged_head(), expected.head());
            assert_eq!(reopened.rights().unwrap(), expected.rights());
            assert_eq!(reopened.rights().unwrap().available(), 60);
            if matches!(expected.rights().state(1), Ok(State::Dispatched | State::Unknown)) {
                assert!(reopened.commit(&Event::AbortBeforeDispatch { id: 1 }).is_err());
                assert_eq!(reopened.rights().unwrap().available(), 60);
            }
        } else { assert!(reopened.is_err(), "partial record cut {end} escaped"); }
        assert_eq!(fs::read(temp.path()).unwrap(), expected_bytes);
    }
}

#[test]
fn validation_and_budget_refusals_never_reach_the_persistence_boundary() {
    let temp = Temp::new(); let mut file = owner(&temp);
    let anchor = file.acknowledged_head(); let rights = file.rights().unwrap().clone();
    let bytes = fs::read(temp.path()).unwrap();
    let good = group();
    let mut bad = group(); bad.push(Event::Reserve { id: 1, effect: effect() });
    for (events, limit) in [(bad.as_slice(), BatchLimits::default()),
        (&[][..], BatchLimits::default()), (good.as_slice(), BatchLimits { max_records: 1, ..BatchLimits::default() })] {
        assert!(file.commit_batch_with(events, limit, |_, _| panic!("invalid group reached I/O")).is_err());
        assert!(!file.is_poisoned());
        assert_eq!(file.acknowledged_head(), anchor);
        assert_eq!(file.rights().unwrap(), &rights);
        assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    }
    file.commit_batch(&group(), BatchLimits::default()).unwrap();
}

#[test]
fn one_combined_persist_call_keeps_the_writer_lock_until_final_acknowledgment() {
    let temp = Temp::new(); let mut file = owner(&temp);
    let anchor = file.acknowledged_head(); let mut calls = 0;
    let mut oracle = original(); let mut expected = Vec::new();
    for event in group() { expected.extend(oracle.append(&event).unwrap()); }
    let head = file.commit_batch_with(&group(), BatchLimits::default(), |handle, bytes| {
        calls += 1;
        assert_eq!(bytes, expected);
        assert!(matches!(FileJournal::open(temp.path(), identity(), anchor, Limits::default()), Err(Error::Busy)));
        handle.write_all(bytes)?;
        handle.sync_all()?;
        assert!(matches!(FileJournal::open(temp.path(), identity(), anchor, Limits::default()), Err(Error::Busy)));
        Ok(())
    }).unwrap();
    assert_eq!(calls, 1);
    assert_eq!(head, oracle.head());
    assert_eq!(file.rights().unwrap(), oracle.rights());
}

#[test]
fn failure_or_unwind_after_a_full_write_never_exposes_the_old_live_ledger() {
    for panic_after_write in [false, true] {
        let temp = Temp::new(); let mut file = owner(&temp); let anchor = file.acknowledged_head();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            file.commit_batch_with(&group(), BatchLimits::default(), |handle, bytes| {
                handle.write_all(bytes)?;
                if panic_after_write { panic!("unwind before sync acknowledgement"); }
                Err(fault()) // Simulate a failed sync after the entire buffered group write.
            })
        }));
        if panic_after_write { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        assert!(file.is_poisoned()); assert_eq!(file.acknowledged_head(), anchor);
        assert!(matches!(file.rights(), Err(Error::Poisoned)));
        drop(file);
        let mut reopened = FileJournal::open(temp.path(), identity(), anchor, Limits::default()).unwrap();
        assert_eq!(reopened.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(reopened.rights().unwrap().state(2), Ok(State::Dispatched));
        assert_eq!(reopened.rights().unwrap().available(), 60);
        assert!(reopened.commit_batch(&group(), BatchLimits::default()).is_err());
        assert!(!reopened.is_poisoned());
        reopened.commit_batch(&[Event::MarkUnknown { id: 2 }, Event::Reconcile { id: 1, occurred: true }],
            BatchLimits::default()).unwrap();
        assert_eq!(reopened.rights().unwrap().available(), 60);
    }
}

#[test]
fn changed_extent_refuses_before_writing_any_group_record() {
    let temp = Temp::new(); let mut file = owner(&temp); let anchor = file.acknowledged_head();
    OpenOptions::new().append(true).open(temp.path()).unwrap().write_all(b"unreviewed tail").unwrap();
    let bytes = fs::read(temp.path()).unwrap();
    assert!(file.commit_batch_with(&group(), BatchLimits::default(), |_, _| panic!("changed extent reached write")).is_err());
    assert!(file.is_poisoned()); assert_eq!(file.acknowledged_head(), anchor);
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
}
