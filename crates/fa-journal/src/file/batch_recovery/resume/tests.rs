//! Original files, single-record framing oracle and real sync/lock boundaries.
//! Injected failures are not hardware power-cut or remote-receipt qualification.
use super::*;
use crate::{Identity, Journal, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-resume-batch-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path()).unwrap() }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("batch-resume cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "alice".into(), resolved_target: "ledger://one".into(),
        payload: vec![0, 255], units: 20 }
}
fn seed() -> Vec<Event> {
    vec![Event::Reserve { id: 1, effect: effect() }, Event::Reserve { id: 2, effect: effect() }]
}
fn group() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect() }, Event::Dispatch { id: 2, effect: effect() },
        Event::MarkUnknown { id: 1 }, Event::RevokeEpoch]
}
fn fault() -> io::Error { io::Error::other("injected missing group acknowledgment") }
fn owner(temp: &Temp) -> FileJournal {
    let mut owner = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    owner.commit_batch(&seed(), BatchLimits::default()).unwrap();
    owner
}
fn open(temp: &Temp, anchor: BatchAnchor) -> FileJournal {
    FileJournal::open(temp.path(), anchor.identity, anchor.head, Limits::default()).unwrap()
}

// Independent wire oracle: use the original single-record append throughout.
fn original() -> (Journal, Vec<u8>, Vec<Vec<u8>>) {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let mut wire = journal.header();
    for event in seed() { wire.extend(journal.append(&event).unwrap()); }
    let records: Vec<_> = group().iter().map(|event| journal.append(event).unwrap()).collect();
    for record in &records { wire.extend_from_slice(record); }
    (journal, wire, records)
}
fn interrupted(temp: &Temp, count: usize) -> (FileJournal, BatchAnchor) {
    let mut file = owner(temp);
    let anchor = file.batch_anchor().unwrap();
    let (_, _, records) = original();
    let prefix: Vec<u8> = records[..count].concat();
    let error = file.commit_batch_with(&group(), BatchLimits::default(), |handle, bytes| {
        assert!(bytes.starts_with(&prefix));
        handle.write_all(&prefix)?;
        handle.sync_all()?;
        Err(fault())
    }).unwrap_err();
    assert!(matches!(error, Error::Io(_)));
    assert_eq!(file.acknowledged_head(), anchor.head);
    assert!(matches!(file.rights(), Err(Error::Poisoned)));
    assert!(matches!(file.resume_batch(anchor, &group(), BatchLimits::default()), Err(Error::Poisoned)));
    assert!(matches!(FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()), Err(Error::Busy)));
    drop(file);
    (open(temp, anchor), anchor)
}

#[test]
fn every_complete_prefix_resumes_exactly_once_and_keeps_the_original_writer_lock() {
    let (mut expected, mut wire, _) = original();
    let settled = Event::Reconcile { id: 1, occurred: false };
    let settlement = expected.append(&settled).unwrap();
    for count in 0..=group().len() {
        let temp = Temp::new();
        let (mut file, anchor) = interrupted(&temp, count);
        let before = temp.bytes();
        let mut calls = 0;
        let head = file.resume_batch_with(anchor, &group(), BatchLimits::default(), |handle, bytes| {
            calls += 1;
            assert_eq!(bytes, &wire[before.len()..]);
            assert!(matches!(FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()), Err(Error::Busy)));
            handle.write_all(bytes)?;
            handle.sync_all()
        }).unwrap();
        assert_eq!(calls, usize::from(count < group().len()));
        assert_eq!(temp.bytes(), wire);
        assert_eq!(&temp.bytes()[..before.len()], before.as_slice());
        assert_eq!(file.rights().unwrap().state(1), Ok(State::Unknown));
        assert_eq!(file.rights().unwrap().available(), 60);
        assert!(!file.is_poisoned());
        let completed = file.rights().unwrap().clone();
        assert_eq!(file.resume_batch_with(anchor, &group(), BatchLimits::default(), |_, _|
            panic!("an already acknowledged group must not write or sync again")).unwrap(), head);
        assert_eq!(file.rights().unwrap(), &completed);
        assert_eq!(temp.bytes(), wire);
        // A newly supplied original receipt is separate work, not inferred by
        // resumption. It is the ONLY operation here that refunds nonexecution.
        assert_eq!(file.commit(&settled).unwrap(), expected.head());
        assert_eq!(file.rights().unwrap(), expected.rights());
        let mut settled_wire = wire.clone(); settled_wire.extend_from_slice(&settlement);
        assert_eq!(temp.bytes(), settled_wire);
        let before = temp.bytes();
        assert!(file.resume_batch(anchor, &group(), BatchLimits::default()).is_err());
        assert!(!file.is_poisoned());
        assert_eq!(temp.bytes(), before);
    }
    // Keep the source oracle visibly independent of any resumed result.
    wire.extend_from_slice(&settlement);
    assert_eq!(wire.len() as u64, expected.encoded_len());
}

#[test]
fn genesis_batch_can_complete_from_no_records_or_an_exact_first_record() {
    let mut expected = Journal::new(identity(), Limits::default()).unwrap();
    let mut wire = expected.header();
    for event in seed() { wire.extend(expected.append(&event).unwrap()); }
    for count in 0..=seed().len() {
        let temp = Temp::new();
        let mut file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
        let anchor = file.batch_anchor().unwrap();
        for event in seed().iter().take(count) { file.commit(event).unwrap(); }
        drop(file);
        let mut file = open(&temp, anchor);
        assert_eq!(file.resume_batch(anchor, &seed(), BatchLimits::default()).unwrap(), expected.head());
        assert_eq!(file.rights().unwrap(), expected.rights());
        assert_eq!(temp.bytes(), wire);
    }
}

#[test]
fn whole_group_limits_and_a_late_invalid_event_refuse_before_any_missing_byte_is_written() {
    let (_, _, records) = original();
    let encoded: usize = records.iter().map(Vec::len).sum();
    for mutation in 0..4 {
        let temp = Temp::new();
        let (mut file, anchor) = interrupted(&temp, 3);
        let mut events = group();
        let mut limits = BatchLimits { max_records: events.len(), max_encoded_bytes: encoded };
        match mutation {
            0 => limits.max_records -= 1,
            1 => limits.max_encoded_bytes -= 1, // still MORE than the missing record
            2 => { events.push(Event::AbortBeforeDispatch { id: 1 }); limits = BatchLimits::default(); }
            _ => { events.push(Event::Reserve { id: 3, effect: Effect { units: 101, ..effect() } });
                limits = BatchLimits::default(); }
        }
        let before = temp.bytes(); let head = file.acknowledged_head();
        assert!(file.resume_batch_with(anchor, &events, limits, |_, _|
            panic!("invalid original group must not reach persistence")).is_err());
        assert_eq!(file.acknowledged_head(), head); assert_eq!(temp.bytes(), before);
        assert!(file.is_poisoned()); assert!(matches!(file.rights(), Err(Error::Poisoned)));
        drop(file);
        let mut file = open(&temp, anchor);
        let exact = BatchLimits { max_records: group().len(), max_encoded_bytes: encoded };
        file.resume_batch(anchor, &group(), exact).unwrap();
        assert_eq!(temp.bytes(), original().1);
    }
}

#[test]
fn wrong_order_effect_or_anchor_never_becomes_a_missing_suffix_repair() {
    for mutation in 0..7 {
        let temp = Temp::new();
        let (mut file, anchor) = interrupted(&temp, 2);
        let mut supplied = anchor; let mut events = group();
        match mutation {
            0 => events.swap(0, 1), // same accounting, different original sequence
            1 => { let Event::Dispatch { effect, .. } = &mut events[0] else { unreachable!() };
                effect.payload[0] ^= 1; }
            2 => supplied.head.checksum ^= 1,
            3 => supplied.encoded_len += 1,
            4 => supplied.encoded_len = file.journal.encoded_len(),
            5 => supplied.identity.policy[0] ^= 1,
            _ => events.clear(),
        }
        let before = temp.bytes(); let head = file.acknowledged_head();
        assert!(file.resume_batch_with(supplied, &events, BatchLimits::default(), |_, _|
            panic!("conflicting history cannot reach persistence")).is_err());
        assert_eq!(file.acknowledged_head(), head); assert_eq!(temp.bytes(), before);
        assert_eq!(file.is_poisoned(), mutation < 5);
        drop(file);
        let mut file = open(&temp, anchor);
        file.resume_batch(anchor, &group(), BatchLimits::default()).unwrap();
        assert_eq!(temp.bytes(), original().1);
    }
}

#[test]
fn no_write_sync_failure_and_caught_unwind_never_publish_candidate_rights() {
    for failure in 0..4 {
        let temp = Temp::new();
        let (mut file, anchor) = interrupted(&temp, 1);
        let before = temp.bytes(); let head = file.acknowledged_head();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            file.resume_batch_with(anchor, &group(), BatchLimits::default(), |handle, bytes| {
                if failure != 0 { handle.write_all(bytes)?; }
                match failure {
                    0 | 1 => Err(fault()),
                    2 => { handle.sync_all()?; Err(fault()) }
                    _ => { handle.sync_all()?; panic!("injected post-sync unwind") }
                }
            })
        }));
        if failure == 3 { assert!(outcome.is_err()); }
        else { assert!(matches!(outcome.unwrap(), Err(Error::Io(_)))); }
        assert_eq!(file.acknowledged_head(), head);
        assert!(file.is_poisoned()); assert!(matches!(file.rights(), Err(Error::Poisoned)));
        assert!(matches!(file.capacity(), Err(Error::Poisoned)));
        assert!(matches!(file.resume_batch(anchor, &group(), BatchLimits::default()), Err(Error::Poisoned)));
        if failure == 0 { assert_eq!(temp.bytes(), before); }
        else { assert_eq!(temp.bytes(), original().1); }
        drop(file);
        let mut file = open(&temp, anchor);
        let inspected = file.inspect_batch(anchor, &group(), BatchLimits::default()).unwrap();
        assert_eq!(inspected.persisted_records(), if failure == 0 { 1 } else { group().len() });
        assert_eq!(file.resume_batch(anchor, &group(), BatchLimits::default()).unwrap(), original().0.head());
        assert_eq!(temp.bytes(), original().1);
        assert_eq!(file.rights().unwrap(), original().0.rights());
    }
}

#[test]
fn every_byte_cut_keeps_torn_records_unrecoverable_and_complete_records_resumable() {
    let (_, wire, records) = original();
    let remaining: Vec<u8> = records[1..].concat();
    let mut offset = 0;
    let mut boundaries = vec![0];
    for record in &records[1..] { offset += record.len(); boundaries.push(offset); }
    for cut in 0..=remaining.len() {
        let temp = Temp::new();
        let (mut file, anchor) = interrupted(&temp, 1);
        let head = file.acknowledged_head();
        let result = file.resume_batch_with(anchor, &group(), BatchLimits::default(), |handle, bytes| {
            assert_eq!(bytes, remaining.as_slice());
            handle.write_all(&bytes[..cut])?;
            handle.sync_all()?;
            Err(fault())
        });
        assert!(matches!(result, Err(Error::Io(_))));
        assert!(file.is_poisoned()); assert_eq!(file.acknowledged_head(), head);
        let interrupted = temp.bytes(); drop(file);
        let recovered = FileJournal::open(temp.path(), identity(), anchor.head, Limits::default());
        if boundaries.contains(&cut) {
            let mut recovered = recovered.unwrap();
            assert_eq!(recovered.rights().unwrap().available(), 60);
            recovered.resume_batch(anchor, &group(), BatchLimits::default()).unwrap();
            assert_eq!(temp.bytes(), wire);
        } else {
            assert!(recovered.is_err(), "torn cut {cut} must not be repaired");
            assert_eq!(temp.bytes(), interrupted);
        }
    }
}

#[test]
fn reserved_recovery_can_finish_at_both_lifetime_limits_without_refilling_ordinary_capacity() {
    let seed = vec![Event::Reserve { id: 1, effect: effect() },
        Event::Dispatch { id: 1, effect: effect() }, Event::MarkUnknown { id: 1 }];
    let tail = vec![Event::RevokeEpoch, Event::Reconcile { id: 1, occurred: false }];
    let reserve = RecoveryReserve { records: 2, bytes: 51 };
    let mut limits = Limits { max_records: 5, max_total_bytes: 10_000, ..Limits::default() };
    let mut probe = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    for event in seed.iter().chain(&tail) { probe.append(event).unwrap(); }
    limits.max_total_bytes = probe.encoded_len();
    let mut expected = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    let mut wire = expected.header();
    for event in seed.iter().chain(&tail) { wire.extend(expected.append(event).unwrap()); }
    for count in 0..=tail.len() {
        let temp = Temp::new();
        let mut file = FileJournal::create_with_recovery_reserve(temp.path(), identity(), limits, reserve).unwrap();
        file.commit_batch(&seed, BatchLimits::default()).unwrap();
        assert_eq!(file.capacity().unwrap().ordinary.records, 0);
        assert_eq!(file.capacity().unwrap().ordinary.bytes, 0);
        let anchor = file.batch_anchor().unwrap();
        for event in tail.iter().take(count) { file.commit(event).unwrap(); }
        drop(file);
        let mut file = FileJournal::open_with_recovery_reserve(temp.path(), identity(), anchor.head, limits, reserve).unwrap();
        assert_eq!(file.resume_batch(anchor, &tail, BatchLimits::default()).unwrap(), expected.head());
        assert_eq!(file.rights().unwrap(), expected.rights());
        assert_eq!(file.rights().unwrap().available(), 100);
        assert_eq!(temp.bytes(), wire);
        let capacity = file.capacity().unwrap();
        assert_eq!(capacity.ordinary.records, 0); assert_eq!(capacity.ordinary.bytes, 0);
        assert_eq!(capacity.recovery.records, 0); assert_eq!(capacity.recovery.bytes, 0);
        file.resume_batch_with(anchor, &tail, BatchLimits::default(), |_, _|
            panic!("full-capacity completed retry writes no record")).unwrap();
        assert_eq!(file.capacity().unwrap(), capacity);
        assert!(file.commit(&Event::Reserve { id: 2, effect: effect() }).is_err());
        assert_eq!(temp.bytes(), wire); drop(file);
        assert!(FileJournal::open(temp.path(), identity(), anchor.head, limits).is_err());
        let wrong = RecoveryReserve { records: 1, ..reserve };
        assert!(FileJournal::open_with_recovery_reserve(temp.path(), identity(), anchor.head, limits, wrong).is_err());
    }
}

#[test]
fn changed_extent_or_same_length_corruption_does_not_get_overwritten() {
    for growth in [false, true] {
        let temp = Temp::new();
        let (mut file, anchor) = interrupted(&temp, 1);
        let head = file.acknowledged_head();
        let mut bytes = temp.bytes();
        if growth { bytes.push(0); } else { *bytes.last_mut().unwrap() ^= 1; }
        fs::write(temp.path(), &bytes).unwrap(); // deliberate violation of advisory locking
        assert!(file.resume_batch_with(anchor, &group(), BatchLimits::default(), |_, _|
            panic!("changed canonical data must never reach append")).is_err());
        assert!(file.is_poisoned()); assert_eq!(file.acknowledged_head(), head);
        assert_eq!(temp.bytes(), bytes);
    }
}
