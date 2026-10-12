use fa_journal::{Error, Event, Head, Identity, Journal, Limits, RecoveryReserve};
use fa_journal::batch::BatchLimits;
use fa_journal::replay::{ReplayBudget, ReplayError, ReplayStatus};
use fa_reference::{Effect, State};

fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn limits() -> Limits { Limits { max_frame_bytes: 1024, max_records: 9, max_total_bytes: 1024 } }
fn reserve() -> RecoveryReserve { RecoveryReserve { records: 5, bytes: 160 } }
fn effect(units: u64) -> Effect {
    Effect { principal: "agent".into(), resolved_target: "sandbox://thing".into(), payload: vec![0, 1, 255], units }
}
fn history() -> Vec<Event> {
    vec![Event::Reserve { id: 1, effect: effect(30) }, Event::Dispatch { id: 1, effect: effect(30) },
        Event::Reserve { id: 2, effect: effect(20) }, Event::Reserve { id: 3, effect: effect(10) },
        Event::RevokeEpoch, Event::MarkUnknown { id: 1 }, Event::AbortBeforeDispatch { id: 2 },
        Event::Reconcile { id: 1, occurred: true }, Event::AbortBeforeDispatch { id: 3 }]
}
fn fixture() -> (Journal, Vec<u8>, Head) {
    let mut journal = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
    let anchor = journal.head(); let mut bytes = journal.header();
    for event in &history() { bytes.extend(journal.append(event).unwrap()); }
    (journal, bytes, anchor)
}
fn cooperative(bytes: &[u8], anchor: Head, cap: Limits, tail: RecoveryReserve,
    budget: ReplayBudget) -> Result<Journal, ReplayError>
{
    let mut replay = Journal::begin_replay_with_recovery_reserve(bytes, identity(), anchor, cap, tail)?;
    for _ in 0..10_000 {
        let report = replay.advance(budget)?;
        assert!(report.spent.read_calls <= budget.read_calls);
        assert!(report.spent.read_bytes <= budget.read_bytes);
        assert!(report.spent.checked_records <= budget.checked_records);
        if report.status == ReplayStatus::Ready { return replay.finish(); }
    }
    panic!("bounded fixture failed to complete");
}
fn same(left: &Journal, right: &Journal) {
    assert_eq!(left.head(), right.head()); assert_eq!(left.encoded_len(), right.encoded_len());
    assert_eq!(left.rights(), right.rights()); assert_eq!(left.capacity(), right.capacity());
    assert_eq!(left.header(), right.header()); assert_eq!(left.recovery_reserve(), right.recovery_reserve());
}

#[test]
fn full_ordinary_record_budget_still_revokes_and_settles_all_original_obligations() {
    let mut journal = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
    for event in &history()[..4] { journal.append(event).unwrap(); }
    assert_eq!(journal.capacity().ordinary.records, 0);
    assert_eq!(journal.capacity().recovery.records, 5);
    let before = journal.clone();
    for event in [Event::Reserve { id: 4, effect: effect(1) },
        Event::Dispatch { id: 2, effect: effect(20) }, Event::ResetToCheckpoint { keep: Default::default() }] {
        assert!(matches!(journal.append(&event), Err(Error::Limit))); same(&journal, &before);
    }
    for event in &history()[4..] { journal.append(event).unwrap(); }
    assert_eq!(journal.rights().state(1), Ok(State::Committed));
    assert_eq!(journal.rights().state(2), Ok(State::Aborted));
    assert_eq!(journal.rights().state(3), Ok(State::Aborted));
    assert_eq!(journal.rights().epoch(), 1); assert_eq!(journal.rights().available(), 70);
    assert!(journal.rights().conserved()); assert_eq!(journal.capacity().recovery.records, 0);
    assert!(matches!(journal.append(&Event::RevokeEpoch), Err(Error::Limit)));
}

#[test]
fn exact_byte_tail_and_one_under_preserve_refunds_without_refilling_journal_space() {
    let mut control = Journal::new(identity(), Limits::default()).unwrap();
    let ordinary: usize = history()[..2].iter().map(|event| control.append(event).unwrap().len()).sum();
    let recovery = [Event::MarkUnknown { id: 1 }, Event::RevokeEpoch,
        Event::Reconcile { id: 1, occurred: false }, Event::RevokeEpoch];
    // Original fixed-size wire records: 29 + 21 + 30 + 21 = 101 bytes.
    for tail_bytes in [100, 101] {
        let cap = Limits { max_total_bytes: 120 + ordinary as u64 + tail_bytes, max_records: 20, ..limits() };
        let tail = RecoveryReserve { records: 4, bytes: tail_bytes };
        let mut journal = Journal::new_with_recovery_reserve(identity(), cap, tail).unwrap();
        for event in &history()[..2] { journal.append(event).unwrap(); }
        assert_eq!(journal.capacity().ordinary.bytes, 0);
        for event in &recovery[..3] { journal.append(event).unwrap(); }
        assert_eq!(journal.rights().available(), 100);
        let before = journal.clone();
        assert!(matches!(journal.append(&Event::Reserve { id: 2, effect: effect(1) }), Err(Error::Limit)));
        same(&before, &journal);
        if tail_bytes == 100 {
            assert!(matches!(journal.append(&recovery[3]), Err(Error::Limit))); same(&before, &journal);
        } else {
            journal.append(&recovery[3]).unwrap(); assert_eq!(journal.capacity().recovery.bytes, 0);
            assert!(matches!(journal.append(&Event::RevokeEpoch), Err(Error::Limit)));
        }
    }
}

#[test]
fn every_group_partition_matches_single_record_bytes_and_both_replay_schedules() {
    let (expected, wire, anchor) = fixture(); let events = history();
    // All 2^8 contiguous partitions of one nine-transition history.
    for mask in 0..1_usize << (events.len() - 1) {
        let mut journal = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
        let mut bytes = journal.header(); let mut start = 0;
        for end in 1..=events.len() {
            if end == events.len() || mask & (1 << (end - 1)) != 0 {
                bytes.extend(journal.append_batch(&events[start..end], BatchLimits::default()).unwrap());
                start = end;
            }
        }
        assert_eq!(bytes, wire); same(&journal, &expected);
    }
    let eager = Journal::replay_with_recovery_reserve(wire.as_slice(), identity(), anchor, limits(), reserve()).unwrap();
    same(&eager, &expected);
    for budget in [ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 1 },
        ReplayBudget { read_calls: 3, read_bytes: 17, checked_records: 2 }, ReplayBudget::UNLIMITED] {
        same(&cooperative(&wire, anchor, limits(), reserve(), budget).unwrap(), &expected);
    }
}

#[test]
fn invalid_mixed_group_cannot_publish_its_valid_recovery_prefix() {
    let mut journal = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
    journal.append_batch(&history()[..4], BatchLimits::default()).unwrap();
    let before = journal.clone();
    let mixed = [Event::MarkUnknown { id: 1 }, Event::Reserve { id: 4, effect: effect(1) }];
    assert!(matches!(journal.append_batch(&mixed, BatchLimits::default()), Err(Error::Limit)));
    same(&journal, &before); assert_eq!(journal.rights().state(1), Ok(State::Dispatched));
    // Recovery eligibility never waives the original state's no-refund rule.
    assert!(matches!(journal.append(&Event::AbortBeforeDispatch { id: 1 }),
        Err(Error::Model(fa_reference::Error::WrongState))));
    assert!(matches!(journal.append(&Event::Reconcile { id: 2, occurred: false }),
        Err(Error::Model(fa_reference::Error::WrongState))));
    same(&journal, &before);
    journal.append_batch(&history()[4..], BatchLimits::default()).unwrap();
    assert_eq!(journal.rights().available(), 70);
}

#[test]
fn header_binds_all_limits_and_reserve_before_reading_any_record() {
    let (_, bytes, anchor) = fixture();
    let variants = [
        (Limits { max_frame_bytes: 1023, ..limits() }, reserve()),
        (Limits { max_records: 10, ..limits() }, reserve()),
        (Limits { max_total_bytes: 2048, ..limits() }, reserve()),
        (limits(), RecoveryReserve { records: 4, ..reserve() }),
        (limits(), RecoveryReserve { bytes: 159, ..reserve() }),
    ];
    for (cap, tail) in variants {
        let mut input = bytes.as_slice();
        assert!(matches!(Journal::replay_with_recovery_reserve(&mut input, identity(), anchor, cap, tail), Err(Error::Binding)));
        assert_eq!(input.len(), bytes.len() - 120);
        let mut replay = Journal::begin_replay_with_recovery_reserve(bytes.as_slice(), identity(), anchor, cap, tail).unwrap();
        assert!(matches!(replay.advance(ReplayBudget::UNLIMITED), Err(ReplayError::Journal(Error::Binding))));
        assert_eq!(replay.progress().total.checked_records, 0);
        assert_eq!(replay.progress().total.read_bytes, 120);
        assert!(matches!(replay.finish(), Err(ReplayError::Failed)));
    }
    assert!(matches!(Journal::replay(bytes.as_slice(), identity(), anchor, limits()), Err(Error::Binding)));
    let mut legacy = Journal::new(identity(), limits()).unwrap(); let mut old = legacy.header();
    old.extend(legacy.append_batch(&history(), BatchLimits::default()).unwrap());
    assert!(Journal::replay_with_recovery_reserve(old.as_slice(), identity(), legacy.head(), limits(), reserve()).is_err());
}

// Independent polynomial long division for malicious-but-well-framed test data.
fn checksum(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 { crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 }; }
    }
    !crc
}

#[test]
fn valid_checksums_cannot_smuggle_ordinary_events_into_the_reserved_tail() {
    for illegal in [Event::Reserve { id: 4, effect: effect(1) },
        Event::Dispatch { id: 2, effect: effect(20) }, Event::ResetToCheckpoint { keep: Default::default() }] {
        let mut journal = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
        let anchor = journal.head(); let mut bytes = journal.header();
        bytes.extend(journal.append_batch(&history()[..4], BatchLimits::default()).unwrap());
        // Original unrestricted encoding proves the event is otherwise lawful.
        let mut legacy = Journal::new(identity(), limits()).unwrap();
        legacy.append_batch(&history()[..4], BatchLimits::default()).unwrap();
        let mut frame = legacy.append(&illegal).unwrap();
        frame[12..16].copy_from_slice(&journal.head().checksum.to_le_bytes());
        let end = frame.len() - 4; let crc = checksum(&frame[..end]);
        frame[end..].copy_from_slice(&crc.to_le_bytes()); bytes.extend(frame);
        assert!(matches!(Journal::replay_with_recovery_reserve(bytes.as_slice(), identity(), anchor, limits(), reserve()), Err(Error::Limit)));
        assert!(matches!(cooperative(&bytes, anchor, limits(), reserve(), ReplayBudget::UNLIMITED),
            Err(ReplayError::Journal(Error::Limit))));
    }
}

#[test]
fn complete_unknown_suffix_is_retained_but_every_truncation_behind_the_anchor_refuses() {
    let (expected, wire, _) = fixture();
    for end in 0..wire.len() {
        assert!(Journal::replay_with_recovery_reserve(&wire[..end], identity(), expected.head(), limits(), reserve()).is_err());
        assert!(cooperative(&wire[..end], expected.head(), limits(), reserve(), ReplayBudget::UNLIMITED).is_err());
    }
    let mut journal = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
    let mut bytes = journal.header(); bytes.extend(journal.append_batch(&history()[..4], BatchLimits::default()).unwrap());
    let anchor = journal.head(); bytes.extend(journal.append_batch(&history()[4..6], BatchLimits::default()).unwrap());
    let mut replay = cooperative(&bytes, anchor, limits(), reserve(), ReplayBudget::UNLIMITED).unwrap();
    assert_eq!(replay.rights().state(1), Ok(State::Unknown)); assert_eq!(replay.rights().available(), 40);
    same(&replay, &journal);
    replay.append_batch(&history()[6..], BatchLimits::default()).unwrap(); same(&replay, &expected);
}

#[test]
fn malformed_reserves_refuse_and_legacy_profile_still_uses_its_last_slot() {
    for tail in [RecoveryReserve { records: 0, bytes: 160 }, RecoveryReserve { records: 10, bytes: 210 },
        RecoveryReserve { records: 5, bytes: 104 }, RecoveryReserve { records: 5, bytes: 905 },
        RecoveryReserve { records: u64::MAX, bytes: u64::MAX }] {
        assert!(matches!(Journal::new_with_recovery_reserve(identity(), limits(), tail), Err(Error::Limit)));
    }
    let mut old = Journal::new(identity(), Limits { max_records: 1, ..limits() }).unwrap();
    assert_eq!(old.recovery_reserve(), None); assert_eq!(old.header().len(), 84);
    assert_eq!(old.capacity().ordinary, old.capacity().recovery);
    old.append(&history()[0]).unwrap();
    assert!(matches!(old.append(&Event::RevokeEpoch), Err(Error::Limit)));
    // Recovery-only profiles are explicit, not a silent empty ordinary admission.
    let cap = Limits { max_records: 1, max_total_bytes: 141, ..limits() };
    let mut only = Journal::new_with_recovery_reserve(identity(), cap, RecoveryReserve { records: 1, bytes: 21 }).unwrap();
    assert_eq!(only.capacity().ordinary.records, 0); assert_eq!(only.capacity().ordinary.bytes, 0);
    only.append(&Event::RevokeEpoch).unwrap(); assert_eq!(only.rights().epoch(), 1);
}

#[test]
fn fixed_header_vector_and_legacy_event_payloads_have_independent_encodings() {
    // Generated independently with Python struct.pack + zlib.crc32, not this codec.
    let hex = include_str!("fixtures/recovery_reserve_v2.hex").trim();
    let vector: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
    let mut journal = Journal::new_with_recovery_reserve(identity(), limits(), reserve()).unwrap();
    assert_eq!(journal.header(), vector); assert_eq!(vector.len(), 120);
    let mut legacy = Journal::new(identity(), limits()).unwrap();
    for event in history() {
        let old = legacy.append(&event).unwrap(); let new = journal.append(&event).unwrap();
        assert_eq!(&old[..12], &new[..12]); // same length and control sequence
        assert_eq!(&old[16..old.len() - 4], &new[16..new.len() - 4]); // unchanged canonical event
    }
}
