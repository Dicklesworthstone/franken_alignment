//! Actual locked files: yielded NEW writes versus existing single/group commits.
#![cfg(unix)]
use fa_journal::{Event, Identity, Journal, Limits, RecoveryReserve};
use fa_journal::batch::BatchLimits;
use fa_journal::file::{BatchCommitBudget as Budget, BatchCommitStage as Stage, Error, FileJournal};
use fa_reference::{Effect, State};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-yield-commit-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("cooperative commit cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "alice".into(), resolved_target: "ledger://one".into(), payload: vec![0, 255], units: 20 }
}
fn seed() -> Event { Event::Reserve { id: 1, effect: effect() } }
fn group() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect() }, Event::MarkUnknown { id: 1 }, Event::RevokeEpoch]
}
fn owner(temp: &Temp) -> FileJournal {
    let mut owner = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    owner.commit(&seed()).unwrap(); owner
}
fn oracle() -> Journal {
    let mut state = Journal::new(identity(), Limits::default()).unwrap(); state.append(&seed()).unwrap(); state
}

#[test]
fn successful_schedules_equal_original_group_bytes_rights_and_recovery_heads() {
    let mut expected = oracle(); let mut bytes = expected.header();
    let mut genesis = Journal::new(identity(), Limits::default()).unwrap(); bytes.extend(genesis.append(&seed()).unwrap());
    for event in group() { bytes.extend(expected.append(&event).unwrap()); }
    for budget in [Budget::UNLIMITED, Budget { prepared_records: 1, write_calls: 1, write_bytes: 1 },
        Budget { prepared_records: 2, write_calls: 2, write_bytes: 13 }] {
        let temp = Temp::new(); let original = owner(&temp); let old = original.acknowledged_head();
        let mut run = original.begin_commit_batch(group(), BatchLimits::default()).unwrap();
        while run.progress().stage != Stage::Ready {
            let before = run.progress(); let after = run.advance(budget).unwrap();
            assert!(after.preparation_attempts - before.preparation_attempts <= budget.prepared_records);
            assert!(after.write_calls - before.write_calls <= budget.write_calls);
            assert!(after.written_bytes - before.written_bytes <= budget.write_bytes);
            assert_eq!(run.acknowledged_head(), old);
            assert!(matches!(FileJournal::open(temp.path(), identity(), old, Limits::default()), Err(Error::Busy)));
        }
        let ready = run.progress(); assert_eq!(run.advance(Budget::UNLIMITED).unwrap(), ready);
        let owner = run.finish().unwrap(); assert!(!owner.is_poisoned());
        assert_eq!(owner.acknowledged_head(), expected.head()); assert_eq!(owner.rights().unwrap(), expected.rights());
        assert_eq!(fs::read(temp.path()).unwrap(), bytes); drop(owner);
        let mut owner = FileJournal::open(temp.path(), identity(), expected.head(), Limits::default()).unwrap();
        assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
        owner.commit(&Event::Reconcile { id: 1, occurred: false }).unwrap();
        assert_eq!(owner.rights().unwrap().available(), 100);
    }
}

#[test]
fn all_events_validate_before_any_write_even_when_the_last_event_is_invalid() {
    let temp = Temp::new(); let owner = owner(&temp); let old = owner.acknowledged_head();
    let before = fs::read(temp.path()).unwrap(); let mut events = group(); events.push(seed());
    let mut run = owner.begin_commit_batch(events, BatchLimits::default()).unwrap();
    for prepared in 1..=group().len() {
        let report = run.advance(Budget { prepared_records: 1, ..Budget::UNLIMITED }).unwrap();
        assert_eq!(report.prepared_records, prepared); assert_eq!(report.write_calls, 0);
        assert_eq!(fs::read(temp.path()).unwrap(), before);
    }
    assert!(run.advance(Budget::UNLIMITED).is_err());
    assert_eq!(run.progress().stage, Stage::Failed); assert_eq!(run.progress().preparation_attempts, 4);
    assert_eq!(run.acknowledged_head(), old); assert_eq!(fs::read(temp.path()).unwrap(), before);
    assert!(matches!(run.finish(), Err(Error::Poisoned)));
    assert_eq!(FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap().rights().unwrap(), oracle().rights());
}

#[test]
fn stages_yield_before_writing_and_zero_allowances_preserve_bytes() {
    let temp = Temp::new(); let owner = owner(&temp); let before = fs::read(temp.path()).unwrap();
    let mut run = owner.begin_commit_batch(group(), BatchLimits::default()).unwrap();
    let start = run.progress(); assert_eq!(run.advance(Budget::default()).unwrap(), start);
    let written = run.advance(Budget::UNLIMITED).unwrap();
    assert_eq!(written.stage, Stage::Writing); assert_eq!(written.written_bytes, 0);
    assert_eq!(fs::read(temp.path()).unwrap(), before);
    for budget in [Budget::default(), Budget { write_bytes: 0, ..Budget::UNLIMITED },
        Budget { write_calls: 0, ..Budget::UNLIMITED }] {
        assert_eq!(run.advance(budget).unwrap(), written); assert_eq!(fs::read(temp.path()).unwrap(), before);
    }
    assert!(matches!(run.finish(), Err(Error::CommitIncomplete)));
    assert_eq!(fs::read(temp.path()).unwrap(), before);
}

#[test]
fn cancelling_at_every_written_byte_never_repairs_or_refunds_partial_history() {
    let mut expected = oracle(); let mut bytes = Vec::new(); let mut cuts = BTreeMap::from([(0, expected.clone())]);
    for event in group() { bytes.extend(expected.append(&event).unwrap()); cuts.insert(bytes.len(), expected.clone()); }
    for end in 0..=bytes.len() {
        let temp = Temp::new(); let owner = owner(&temp); let old = owner.acknowledged_head();
        let prefix = fs::read(temp.path()).unwrap();
        let mut run = owner.begin_commit_batch(group(), BatchLimits::default()).unwrap();
        run.advance(Budget::UNLIMITED).unwrap();
        while run.progress().written_bytes < end as u64 {
            run.advance(Budget { prepared_records: 0, write_calls: 1,
                write_bytes: end as u64 - run.progress().written_bytes }).unwrap();
        }
        let progress = run.cancel(); assert_eq!(progress.written_bytes, end as u64);
        let mut expected_bytes = prefix; expected_bytes.extend_from_slice(&bytes[..end]);
        assert_eq!(fs::read(temp.path()).unwrap(), expected_bytes);
        let reopened = FileJournal::open(temp.path(), identity(), old, Limits::default());
        if let Some(expected) = cuts.get(&end) {
            let mut reopened = reopened.unwrap(); assert_eq!(reopened.acknowledged_head(), expected.head());
            assert_eq!(reopened.rights().unwrap(), expected.rights()); assert_eq!(reopened.rights().unwrap().available(), 80);
            if end > 0 { assert!(reopened.commit(&Event::AbortBeforeDispatch { id: 1 }).is_err()); }
        } else { assert!(reopened.is_err(), "torn record at byte {end} was not refused"); }
        assert_eq!(fs::read(temp.path()).unwrap(), expected_bytes);
    }
}

#[test]
fn reserved_recovery_group_can_finish_at_exact_lifetime_byte_and_record_limits() {
    let reserve = RecoveryReserve { records: 3, bytes: 80 };
    let mut sizing = Journal::new_with_recovery_reserve(identity(), Limits::default(), reserve).unwrap();
    sizing.append(&seed()).unwrap(); sizing.append(&Event::Dispatch { id: 1, effect: effect() }).unwrap();
    let limits = Limits { max_records: 5, max_total_bytes: sizing.encoded_len() + reserve.bytes, ..Limits::default() };
    let temp = Temp::new();
    let mut owner = FileJournal::create_with_recovery_reserve(temp.path(), identity(), limits, reserve).unwrap();
    owner.commit(&seed()).unwrap(); owner.commit(&Event::Dispatch { id: 1, effect: effect() }).unwrap();
    let old = owner.acknowledged_head();
    assert_eq!(owner.capacity().unwrap().ordinary.records, 0); assert_eq!(owner.capacity().unwrap().ordinary.bytes, 0);
    let mut run = owner.begin_commit_batch(vec![Event::MarkUnknown { id: 1 }, Event::RevokeEpoch,
        Event::Reconcile { id: 1, occurred: false }], BatchLimits::default()).unwrap();
    while run.progress().stage != Stage::Ready {
        run.advance(Budget { prepared_records: 1, write_calls: 1, write_bytes: 7 }).unwrap();
    }
    let owner = run.finish().unwrap(); assert_eq!(owner.rights().unwrap().available(), 100);
    assert_eq!(owner.capacity().unwrap().recovery.records, 0); assert_eq!(owner.capacity().unwrap().recovery.bytes, 0);
    let head = owner.acknowledged_head(); drop(owner);
    let owner = FileJournal::open_with_recovery_reserve(temp.path(), identity(), old, limits, reserve).unwrap();
    assert_eq!(owner.acknowledged_head(), head); assert_eq!(owner.capacity().unwrap().recovery.bytes, 0);
}

#[test]
fn invalid_admission_and_incomplete_drop_release_the_original_lock_without_writing() {
    let temp = Temp::new(); let owner = owner(&temp); let old = owner.acknowledged_head();
    let before = fs::read(temp.path()).unwrap();
    assert!(matches!(owner.begin_commit_batch(Vec::new(), BatchLimits::default()), Err(Error::Journal(_))));
    let owner = FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap();
    let mut run = owner.begin_commit_batch(group(), BatchLimits::default()).unwrap();
    run.advance(Budget { prepared_records: 1, ..Budget::default() }).unwrap();
    assert!(matches!(run.finish(), Err(Error::CommitIncomplete)));
    let owner = FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap();
    assert_eq!(owner.acknowledged_head(), old); assert_eq!(fs::read(temp.path()).unwrap(), before);
}

#[test]
fn cancellation_of_complete_writes_is_not_a_commit_acknowledgment_or_rollback() {
    let temp = Temp::new(); let owner = owner(&temp); let old = owner.acknowledged_head();
    let mut run = owner.begin_commit_batch(group(), BatchLimits::default()).unwrap();
    while run.progress().stage != Stage::Ready { run.advance(Budget::UNLIMITED).unwrap(); }
    assert_eq!(run.acknowledged_head(), old); let bytes = fs::read(temp.path()).unwrap();
    assert_eq!(run.cancel().stage, Stage::Ready);
    let mut owner = FileJournal::open(temp.path(), identity(), old, Limits::default()).unwrap();
    assert_eq!(owner.acknowledged_head().sequence, old.sequence + 3);
    assert_eq!(owner.rights().unwrap().state(1), Ok(State::Unknown));
    assert!(owner.commit_batch(&group(), BatchLimits::default()).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
}
