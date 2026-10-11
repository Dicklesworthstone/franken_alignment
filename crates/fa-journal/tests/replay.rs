use fa_journal::{Error, Event, Head, Identity, Journal, Limits};
use fa_reference::{Effect, State};
use std::collections::BTreeSet;
use std::io::{self, Read};

fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 100 }
}

fn effect(units: u64) -> Effect {
    Effect {
        principal: "alice".into(),
        resolved_target: "ledger://account/one".into(),
        payload: vec![0, 255, 10, 13, 0],
        units,
    }
}

fn fixture() -> (Journal, Vec<u8>, Head) {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let genesis = journal.head();
    let mut bytes = journal.header();
    for event in [
        Event::Reserve { id: 1, effect: effect(30) },
        Event::Reserve { id: 2, effect: effect(20) },
        Event::Dispatch { id: 1, effect: effect(30) },
        Event::MarkUnknown { id: 1 },
        Event::ResetToCheckpoint { keep: BTreeSet::new() },
        Event::RevokeEpoch,
    ] {
        bytes.extend(journal.append(&event).unwrap());
    }
    (journal, bytes, genesis)
}

#[test]
fn replay_preserves_unknown_liabilities_and_checkpoint_refunds() {
    let (journal, bytes, _) = fixture();
    let replay = Journal::replay(bytes.as_slice(), identity(), journal.head(), Limits::default()).unwrap();
    assert_eq!(replay.rights(), journal.rights());
    assert_eq!(replay.rights().available(), 70);
    assert_eq!(replay.rights().state(1), Ok(State::Unknown));
    assert_eq!(replay.rights().state(2), Ok(State::Aborted));
    assert_eq!(replay.rights().epoch(), 1);
    assert_eq!(replay.rights().incident_count(), 1);
    assert_eq!(replay.encoded_len(), bytes.len() as u64);
    assert!(replay.rights().conserved());
}

#[test]
fn every_truncation_before_acknowledged_head_is_rejected() {
    let (journal, bytes, _) = fixture();
    for end in 0..bytes.len() {
        assert!(Journal::replay(&bytes[..end], identity(), journal.head(), Limits::default()).is_err(), "prefix {end}");
    }
}

#[test]
fn every_single_bit_flip_is_rejected() {
    let (journal, bytes, _) = fixture();
    for offset in 0..bytes.len() {
        for bit in 0..8 {
            let mut damaged = bytes.clone();
            damaged[offset] ^= 1 << bit;
            assert!(Journal::replay(damaged.as_slice(), identity(), journal.head(), Limits::default()).is_err(), "byte {offset}, bit {bit}");
        }
    }
}

#[test]
fn suffix_after_anchor_is_replayed_not_refunded() {
    let (journal, bytes, genesis) = fixture();
    let replay = Journal::replay(bytes.as_slice(), identity(), genesis, Limits::default()).unwrap();
    assert_eq!(replay.head(), journal.head());
    assert_eq!(replay.rights().state(1), Ok(State::Unknown));
}

#[test]
fn wrong_context_and_wrong_anchor_are_rejected() {
    let (journal, bytes, genesis) = fixture();
    for changed in [
        Identity { run: [3; 32], ..identity() },
        Identity { policy: [3; 32], ..identity() },
        Identity { total_units: 101, ..identity() },
    ] {
        assert!(Journal::replay(bytes.as_slice(), changed, genesis, Limits::default()).is_err());
    }
    for head in [genesis, journal.head()] {
        assert!(matches!(Journal::replay(bytes.as_slice(), identity(), Head { checksum: head.checksum ^ 1, ..head }, Limits::default()), Err(Error::Binding)));
    }
    assert!(matches!(Journal::replay(bytes.as_slice(), identity(), Head { sequence: journal.head().sequence + 1, checksum: 0 }, Limits::default()), Err(Error::BehindAnchor)));
}

#[test]
fn errors_do_not_change_state_or_sequence() {
    let (mut journal, _, _) = fixture();
    let head = journal.head();
    let length = journal.encoded_len();
    let rights = journal.rights().clone();
    for event in [
        Event::Reserve { id: 1, effect: effect(1) },
        Event::Reserve { id: 3, effect: effect(71) },
        Event::Reserve { id: 3, effect: effect(0) },
        Event::Dispatch { id: 1, effect: effect(30) },
        Event::AbortBeforeDispatch { id: 1 },
        Event::Reconcile { id: 2, occurred: false },
        Event::ResetToCheckpoint { keep: BTreeSet::from([999]) },
    ] {
        assert!(journal.append(&event).is_err());
        assert_eq!(journal.head(), head);
        assert_eq!(journal.encoded_len(), length);
        assert_eq!(journal.rights(), &rights);
    }
}

#[test]
fn stale_epoch_and_changed_effect_cannot_dispatch() {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    journal.append(&Event::Reserve { id: 1, effect: effect(10) }).unwrap();
    let mut changed = effect(10);
    changed.payload.push(99);
    assert!(journal.append(&Event::Dispatch { id: 1, effect: changed }).is_err());
    journal.append(&Event::RevokeEpoch).unwrap();
    assert!(journal.append(&Event::Dispatch { id: 1, effect: effect(10) }).is_err());
    assert_eq!(journal.rights().state(1), Ok(State::Reserved));
    journal.append(&Event::AbortBeforeDispatch { id: 1 }).unwrap();
    assert_eq!(journal.rights().available(), 100);
}

#[test]
fn successful_and_negative_reconciliation_round_trip() {
    for occurred in [false, true] {
        let mut journal = Journal::new(identity(), Limits::default()).unwrap();
        let genesis = journal.head();
        let mut bytes = journal.header();
        for event in [
            Event::Reserve { id: 1, effect: effect(40) },
            Event::Dispatch { id: 1, effect: effect(40) },
            Event::Reconcile { id: 1, occurred },
        ] {
            bytes.extend(journal.append(&event).unwrap());
        }
        let replay = Journal::replay(bytes.as_slice(), identity(), genesis, Limits::default()).unwrap();
        assert_eq!(replay.rights(), journal.rights());
        assert_eq!(replay.rights().available(), if occurred { 60 } else { 100 });
        assert_eq!(replay.rights().state(1), Ok(if occurred { State::Committed } else { State::Aborted }));
        assert!(journal.append(&Event::Reconcile { id: 1, occurred }).is_err());
    }
}

#[test]
fn record_and_byte_budgets_apply_before_mutation() {
    let limits = Limits { max_records: 1, ..Limits::default() };
    let mut journal = Journal::new(identity(), limits).unwrap();
    journal.append(&Event::RevokeEpoch).unwrap();
    let head = journal.head();
    assert!(matches!(journal.append(&Event::RevokeEpoch), Err(Error::Limit)));
    assert_eq!(journal.head(), head);
    let mut journal = Journal::new(identity(), Limits { max_total_bytes: 84, ..limits }).unwrap();
    assert!(matches!(journal.append(&Event::RevokeEpoch), Err(Error::Limit)));
    assert_eq!(journal.rights().epoch(), 0);
    let mut journal = Journal::new(identity(), Limits { max_frame_bytes: 13, ..limits }).unwrap();
    assert!(matches!(journal.append(&Event::Reserve { id: 1, effect: effect(1) }), Err(Error::Limit)));
    journal.append(&Event::RevokeEpoch).unwrap();
}

#[test]
fn hostile_length_is_rejected_before_allocation() {
    let journal = Journal::new(identity(), Limits::default()).unwrap();
    let mut bytes = journal.header();
    bytes.extend(u32::MAX.to_le_bytes());
    assert!(matches!(Journal::replay(bytes.as_slice(), identity(), journal.head(), Limits::default()), Err(Error::Limit)));
}

#[test]
fn duplicate_reordered_and_extra_bytes_are_rejected() {
    let mut journal = Journal::new(identity(), Limits::default()).unwrap();
    let genesis = journal.head();
    let a = journal.append(&Event::RevokeEpoch).unwrap();
    let b = journal.append(&Event::RevokeEpoch).unwrap();
    for records in [vec![&a, &a], vec![&b, &a], vec![&b]] {
        let mut bytes = journal.header();
        for record in records {
            bytes.extend(record);
        }
        assert!(Journal::replay(bytes.as_slice(), identity(), genesis, Limits::default()).is_err());
    }
    let mut bytes = journal.header();
    bytes.extend(a);
    bytes.push(0);
    assert!(Journal::replay(bytes.as_slice(), identity(), genesis, Limits::default()).is_err());
}

struct Fragmented<'a> {
    remaining: &'a [u8],
    interrupted: bool,
}

impl Read for Fragmented<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.interrupted = false;
        let count = out.len().min(3).min(self.remaining.len());
        out[..count].copy_from_slice(&self.remaining[..count]);
        self.remaining = &self.remaining[count..];
        Ok(count)
    }
}

#[test]
fn fragmented_and_interrupted_reads_replay_identically() {
    let (journal, bytes, genesis) = fixture();
    let input = Fragmented { remaining: &bytes, interrupted: false };
    let replay = Journal::replay(input, identity(), genesis, Limits::default()).unwrap();
    assert_eq!(replay.rights(), journal.rights());
}
