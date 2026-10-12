//! Actual locked files and the original encoder/reducer; no simulated ledger.
use super::*;
use fa_reference::{Effect, State};
use crate::{Limits, HEADER_LEN};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-exact-batch-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path()).unwrap() }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("batch recovery cleanup: {error}"); }
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
fn original(limits: Limits) -> Journal {
    let mut journal = Journal::new(identity(), limits).unwrap();
    for event in seed() { journal.append(&event).unwrap(); }
    journal
}
fn owner(temp: &Temp, limits: Limits) -> FileJournal {
    let mut file = FileJournal::create(temp.path(), identity(), limits).unwrap();
    file.commit_batch(&seed(), BatchLimits::default()).unwrap(); file
}
fn open(temp: &Temp, anchor: BatchAnchor) -> FileJournal {
    FileJournal::open(temp.path(), anchor.identity, anchor.head, Limits::default()).unwrap()
}
fn fault() -> io::Error { io::Error::other("injected lost batch acknowledgment") }

// Persist a precise complete prefix but acknowledge no part of the caller's group.
fn interrupt(file: &mut FileJournal, count: usize) {
    let mut prefix = original(Limits::default());
    let mut prefix_bytes = Vec::new();
    for event in group().iter().take(count) { prefix_bytes.extend(prefix.append(event).unwrap()); }
    let result = file.commit_batch_with(&group(), BatchLimits::default(), |handle, bytes| {
        assert!(bytes.starts_with(&prefix_bytes));
        handle.write_all(&prefix_bytes)?; handle.sync_all()?; Err(fault())
    });
    assert!(matches!(result, Err(Error::Io(_))));
}

#[test]
fn every_surviving_record_boundary_is_inspected_without_an_append_or_refund() {
    let mut final_state = original(Limits::default());
    let records = final_state.append_batch(&group(), BatchLimits::default()).unwrap();
    for count in 0..=group().len() {
        let temp = Temp::new(); let mut file = owner(&temp, Limits::default());
        let anchor = file.batch_anchor().unwrap(); interrupt(&mut file, count);
        assert_eq!(file.acknowledged_head(), anchor.head);
        assert!(matches!(file.batch_anchor(), Err(Error::Poisoned)));
        assert!(matches!(file.inspect_batch(anchor, &group(), BatchLimits::default()), Err(Error::Poisoned)));
        assert!(matches!(FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()), Err(Error::Busy)));
        drop(file);
        let mut file = open(&temp, anchor); let bytes = temp.bytes();
        let head = file.acknowledged_head(); let rights = file.rights().unwrap().clone();
        let status = file.inspect_batch(anchor, &group(), BatchLimits::default()).unwrap();
        assert_eq!(status.anchor(), anchor); assert_eq!(status.observed_head(), head);
        assert_eq!(status.persisted_records(), count);
        assert_eq!(status.remaining_records(), group().len() - count);
        assert_eq!(status.remaining_bytes(), records.len() - (bytes.len() - anchor.encoded_len as usize));
        assert_eq!(status.is_complete(), count == group().len());
        assert_eq!(file.inspect_batch(anchor, &group(), BatchLimits::default()).unwrap(), status);
        assert_eq!(temp.bytes(), bytes); assert_eq!(file.rights().unwrap(), &rights);
        assert_eq!(file.rights().unwrap().available(), 60); assert!(!file.is_poisoned());
        if count >= 3 { assert_eq!(file.rights().unwrap().state(1), Ok(State::Unknown)); }
    }
}

#[test]
fn genesis_anchor_can_recover_a_first_batch_without_inventing_prior_records() {
    let temp = Temp::new();
    let mut file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let anchor = file.batch_anchor().unwrap();
    file.commit(&seed()[0]).unwrap(); drop(file);
    let mut file = open(&temp, anchor); let bytes = temp.bytes();
    let status = file.inspect_batch(anchor, &seed(), BatchLimits::default()).unwrap();
    assert_eq!(status.persisted_records(), 1); assert_eq!(status.remaining_records(), 1);
    assert_eq!(file.rights().unwrap().available(), 80); assert_eq!(temp.bytes(), bytes);
}

#[test]
fn different_stored_order_refuses_even_when_the_current_accounting_matches() {
    let temp = Temp::new(); let mut file = owner(&temp, Limits::default());
    let anchor = file.batch_anchor().unwrap();
    file.commit_batch(&group()[..2], BatchLimits::default()).unwrap();
    let mut swapped = group(); swapped.swap(0, 1);
    let mut oracle = original(Limits::default());
    oracle.append_batch(&swapped[..2], BatchLimits::default()).unwrap();
    assert_eq!(oracle.rights(), file.rights().unwrap());
    let bytes = temp.bytes(); let head = file.acknowledged_head();
    assert!(matches!(file.inspect_batch(anchor, &swapped, BatchLimits::default()), Err(Error::Journal(crate::Error::Binding))));
    assert!(file.is_poisoned()); assert_eq!(file.acknowledged_head(), head);
    assert_eq!(temp.bytes(), bytes); drop(file);
    assert_eq!(open(&temp, anchor).inspect_batch(anchor, &group(), BatchLimits::default()).unwrap().persisted_records(), 2);
}

#[test]
fn no_offset_or_anchor_substitution_can_expose_a_private_replayed_prefix() {
    for mutation in 0..4 {
        let temp = Temp::new(); let mut file = owner(&temp, Limits::default());
        let anchor = file.batch_anchor().unwrap(); file.commit_batch(&group(), BatchLimits::default()).unwrap();
        let head = file.acknowledged_head(); let bytes = temp.bytes(); let mut wrong = anchor;
        match mutation {
            0 => wrong.head.checksum ^= 1,
            1 => wrong.encoded_len += 1, // inside the next record's length
            2 => wrong.encoded_len = file.journal.encoded_len(), // clean but later cut
            _ => wrong.encoded_len = HEADER_LEN as u64, // valid header, missing anchor
        }
        assert!(file.inspect_batch(wrong, &group(), BatchLimits::default()).is_err());
        assert!(file.is_poisoned()); assert!(matches!(file.rights(), Err(Error::Poisoned)));
        assert_eq!(file.acknowledged_head(), head); assert_eq!(temp.bytes(), bytes);
    }
}

#[test]
fn a_valid_prefix_does_not_waive_late_semantic_or_whole_group_limits() {
    for mutation in 0..3 {
        let temp = Temp::new(); let mut file = owner(&temp, Limits::default());
        let anchor = file.batch_anchor().unwrap(); file.commit(&group()[0]).unwrap();
        let mut events = group(); let mut limits = BatchLimits::default();
        match mutation {
            0 => events.push(Event::Reserve { id: 1, effect: effect() }),
            1 => limits.max_records = events.len() - 1,
            _ => limits.max_encoded_bytes = 21,
        }
        let bytes = temp.bytes(); let head = file.acknowledged_head();
        assert!(file.inspect_batch(anchor, &events, limits).is_err());
        assert_eq!(temp.bytes(), bytes); assert_eq!(file.acknowledged_head(), head);
        drop(file);
        assert_eq!(open(&temp, anchor).inspect_batch(anchor, &group(), BatchLimits::default()).unwrap().persisted_records(), 1);
    }
}

#[test]
fn changed_extent_or_same_length_corruption_never_returns_stale_success() {
    for grow in [false, true] {
        let temp = Temp::new(); let mut file = owner(&temp, Limits::default());
        let anchor = file.batch_anchor().unwrap(); file.commit_batch(&group(), BatchLimits::default()).unwrap();
        let head = file.acknowledged_head(); let mut bytes = temp.bytes();
        if grow { bytes.push(0); } else { *bytes.last_mut().unwrap() ^= 1; }
        // Deliberate noncooperating corruption, not a supported concurrent writer.
        fs::write(temp.path(), &bytes).unwrap();
        assert!(file.inspect_batch(anchor, &group(), BatchLimits::default()).is_err());
        assert!(file.is_poisoned()); assert_eq!(file.acknowledged_head(), head);
        assert_eq!(temp.bytes(), bytes);
    }
}

#[test]
fn different_identity_and_unrelated_later_records_are_not_a_batch_retry() {
    let temp = Temp::new(); let mut file = owner(&temp, Limits::default());
    let anchor = file.batch_anchor().unwrap(); let mut wrong = anchor; wrong.identity.run[0] ^= 1;
    let bytes = temp.bytes();
    assert!(file.inspect_batch(wrong, &group(), BatchLimits::default()).is_err());
    assert!(!file.is_poisoned()); assert_eq!(temp.bytes(), bytes);
    file.commit_batch(&group(), BatchLimits::default()).unwrap();
    file.commit(&Event::Reconcile { id: 1, occurred: true }).unwrap();
    let bytes = temp.bytes();
    assert!(file.inspect_batch(anchor, &group(), BatchLimits::default()).is_err());
    assert!(!file.is_poisoned()); assert_eq!(temp.bytes(), bytes);
}


fn reserved_profile() -> (Limits, crate::RecoveryReserve, Vec<Event>, Vec<Event>) {
    let seed = vec![Event::Reserve { id: 1, effect: effect() },
        Event::Dispatch { id: 1, effect: effect() }, Event::MarkUnknown { id: 1 }];
    let tail = vec![Event::RevokeEpoch, Event::Reconcile { id: 1, occurred: false }];
    let reserve = crate::RecoveryReserve { records: 2, bytes: 51 };
    let mut limits = Limits { max_records: 5, max_total_bytes: 10_000, ..Limits::default() };
    let mut probe = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    probe.append_batch(&seed, BatchLimits::default()).unwrap();
    probe.append_batch(&tail, BatchLimits::default()).unwrap();
    limits.max_total_bytes = probe.encoded_len();
    (limits, reserve, seed, tail)
}

#[test]
fn reserved_profile_inspection_preserves_exact_tail_capacity_and_original_settlement() {
    let (limits, reserve, seed, tail) = reserved_profile();
    for count in 0..=tail.len() {
        let temp = Temp::new();
        let mut file = FileJournal::create_with_recovery_reserve(temp.path(), identity(), limits, reserve).unwrap();
        file.commit_batch(&seed, BatchLimits::default()).unwrap();
        assert_eq!(file.capacity().unwrap().ordinary.records, 0);
        assert_eq!(file.capacity().unwrap().ordinary.bytes, 0);
        let anchor = file.batch_anchor().unwrap();
        if count != 0 { file.commit_batch(&tail[..count], BatchLimits::default()).unwrap(); }
        drop(file);
        let mut file = FileJournal::open_with_recovery_reserve(temp.path(), identity(), anchor.head, limits, reserve).unwrap();
        let before = temp.bytes(); let capacity = file.capacity().unwrap();
        let status = file.inspect_batch(anchor, &tail, BatchLimits::default()).unwrap();
        assert_eq!(status.persisted_records(), count); assert_eq!(status.is_complete(), count == tail.len());
        assert_eq!(status.remaining_bytes() as u64, capacity.recovery.bytes);
        assert_eq!(status.remaining_records() as u64, capacity.recovery.records);
        assert_eq!(file.capacity().unwrap(), capacity); assert_eq!(temp.bytes(), before);
        assert_eq!(file.rights().unwrap().available(), if count == 2 { 100 } else { 80 });
    }
}
