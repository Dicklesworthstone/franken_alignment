//! Capture must describe the original bytes, not merely their CRC or final state.
use fa_journal::replay::{HistoryAnchor, Replay, ReplayBudget, ReplayError, ReplayStatus};
use fa_journal::{Error, Event, Identity, Journal, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::io::{self, Read};

fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 10 }
}

fn history(reserved: bool) -> (Journal, Vec<u8>) {
    let limits = Limits::default();
    let mut journal = if reserved {
        Journal::new_with_recovery_reserve(identity(), limits,
            RecoveryReserve { records: 2, bytes: 60 }).unwrap()
    } else {
        Journal::new(identity(), limits).unwrap()
    };
    let mut bytes = journal.header();
    let effect = Effect { principal: "actor".into(), resolved_target: "sink".into(),
        payload: b"message".to_vec(), units: 4 };
    for event in [Event::Reserve { id: 1, effect: effect.clone() },
        Event::Dispatch { id: 1, effect }, Event::MarkUnknown { id: 1 }, Event::RevokeEpoch]
    {
        bytes.extend(journal.append(&event).unwrap());
    }
    (journal, bytes)
}

fn finish<R: Read>(mut replay: Replay<R>, quantum: u64) -> (Journal, HistoryAnchor) {
    while replay.progress().status == ReplayStatus::Reading {
        let report = replay.advance(ReplayBudget {
            read_calls: 2, read_bytes: quantum, checked_records: 1,
        }).unwrap();
        assert!(report.spent.read_calls <= 2);
        assert!(report.spent.read_bytes <= quantum);
        assert!(report.spent.checked_records <= 1);
    }
    let total = replay.progress().total;
    assert_eq!(replay.advance(ReplayBudget::UNLIMITED).unwrap().total, total);
    let result = replay.finish_with_history_anchor().unwrap();
    assert_eq!(total.read_bytes, result.1.encoded_len());
    assert_eq!(total.checked_records, result.0.head().sequence);
    result
}

#[test]
fn captured_v1_and_v2_match_independent_import_and_conserve_unknown_liabilities() {
    for reserved in [false, true] {
        let (expected, bytes) = history(reserved);
        for quantum in [1, 7, 4096] {
            let (cursor, oracle) = if let Some(reserve) = expected.recovery_reserve() {
                (Journal::begin_replay_with_history_anchor_and_recovery_reserve(
                    bytes.as_slice(), identity(), expected.head(), Limits::default(), reserve).unwrap(),
                 HistoryAnchor::from_bytes_with_recovery_reserve(
                    bytes.clone(), identity(), Limits::default(), reserve).unwrap())
            } else {
                (Journal::begin_replay_with_history_anchor(
                    bytes.as_slice(), identity(), expected.head(), Limits::default()).unwrap(),
                 HistoryAnchor::from_bytes(bytes.clone(), identity(), Limits::default()).unwrap())
            };
            let (checked, anchor) = finish(cursor, quantum);
            assert_eq!(anchor.as_bytes(), oracle.as_bytes());
            assert_eq!(anchor.head(), oracle.head());
            assert_eq!(checked.rights(), expected.rights());
            assert_eq!(checked.rights().state(1).unwrap(), State::Unknown);
            assert_eq!(checked.rights().available(), 6);
            assert_eq!(checked.capacity(), expected.capacity());
            assert!(checked.rights().conserved());
        }
    }
}

#[test]
fn ordinary_replay_cannot_upgrade_to_exact_evidence_after_observing_the_result() {
    let (expected, bytes) = history(false);
    let mut plain = Journal::begin_replay(bytes.as_slice(), identity(), expected.head(), Limits::default()).unwrap();
    plain.advance(ReplayBudget::UNLIMITED).unwrap();
    assert!(matches!(plain.finish_with_history_anchor(), Err(ReplayError::Journal(Error::Binding))));
    let recording = Journal::begin_replay_with_history_anchor(
        bytes.as_slice(), identity(), expected.head(), Limits::default()).unwrap();
    assert_eq!(finish(recording, 1).1.as_bytes(), bytes);
}

#[test]
fn every_truncation_before_minimum_and_invalid_complete_suffix_refuse_an_archive() {
    let (expected, bytes) = history(false);
    for end in 0..bytes.len() {
        let mut cursor = Journal::begin_replay_with_history_anchor(
            &bytes[..end], identity(), expected.head(), Limits::default()).unwrap();
        assert!(cursor.advance(ReplayBudget::UNLIMITED).is_err(), "cut {end}");
        assert!(matches!(cursor.finish_with_history_anchor(), Err(ReplayError::Failed)));
    }
    let mut damaged = bytes.clone();
    *damaged.last_mut().unwrap() ^= 1;
    let mut extra = bytes.clone();
    extra.push(0);
    for invalid in [damaged, extra] {
        let mut cursor = Journal::begin_replay_with_history_anchor(
            invalid.as_slice(), identity(), expected.head(), Limits::default()).unwrap();
        assert!(cursor.advance(ReplayBudget::UNLIMITED).is_err());
        assert!(matches!(cursor.advance(ReplayBudget::UNLIMITED), Err(ReplayError::Failed)));
        assert!(matches!(cursor.finish_with_history_anchor(), Err(ReplayError::Failed)));
    }
    let cursor = Journal::begin_replay_with_history_anchor(
        bytes.as_slice(), identity(), expected.head(), Limits::default()).unwrap();
    assert_eq!(finish(cursor, 7).1.as_bytes(), bytes);
}

#[test]
fn zero_budget_and_cancellation_never_issue_partial_evidence() {
    let (expected, bytes) = history(false);
    let begin = || Journal::begin_replay_with_history_anchor(
        bytes.as_slice(), identity(), expected.head(), Limits::default()).unwrap();
    let mut cursor = begin();
    let report = cursor.advance(ReplayBudget::default()).unwrap();
    assert_eq!(report.total.read_calls, 0);
    assert!(matches!(cursor.finish_with_history_anchor(), Err(ReplayError::NotReady)));
    for count in 0..bytes.len() {
        let mut cursor = begin();
        let report = cursor.advance(ReplayBudget {
            read_calls: u64::MAX, read_bytes: count as u64, checked_records: u64::MAX,
        }).unwrap();
        assert_eq!(report.status, ReplayStatus::Reading);
        assert_eq!(cursor.cancel(), report.total);
    }
}

#[test]
fn exhausted_lifetime_bounds_and_wrong_profiles_still_fail_closed() {
    let (expected, bytes) = history(false);
    let mut cursor = Journal::begin_replay_with_history_anchor(bytes.as_slice(), identity(), expected.head(),
        Limits { max_records: expected.head().sequence - 1, ..Limits::default() }).unwrap();
    assert!(cursor.advance(ReplayBudget::UNLIMITED).is_err());
    assert!(cursor.finish_with_history_anchor().is_err());
    let (v2, reserved) = history(true);
    let mut wrong = Journal::begin_replay_with_history_anchor(
        reserved.as_slice(), identity(), v2.head(), Limits::default()).unwrap();
    assert!(matches!(wrong.advance(ReplayBudget::UNLIMITED), Err(ReplayError::Journal(Error::Binding))));
    assert!(wrong.finish_with_history_anchor().is_err());
}

#[test]
fn interrupted_or_unwinding_reader_cannot_reset_work_or_export_a_prefix() {
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { Err(io::ErrorKind::Interrupted.into()) }
    }
    let initial = Journal::new(identity(), Limits::default()).unwrap().head();
    let mut cursor = Journal::begin_replay_with_history_anchor(Interrupted, identity(), initial, Limits::default()).unwrap();
    let report = cursor.advance(ReplayBudget { read_calls: 3, read_bytes: 100, checked_records: 1 }).unwrap();
    assert_eq!(report.total.read_calls, 3);
    assert_eq!(report.total.read_bytes, 0);
    assert_eq!(cursor.cancel(), report.total);

    struct Panics;
    impl Read for Panics {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("injected reader unwind") }
    }
    let mut cursor = Journal::begin_replay_with_history_anchor(Panics, identity(), initial, Limits::default()).unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = cursor.advance(ReplayBudget::UNLIMITED);
    })).is_err());
    assert_eq!(cursor.progress().status, ReplayStatus::Failed);
    assert_eq!(cursor.progress().total.read_calls, 1);
    assert!(matches!(cursor.finish_with_history_anchor(), Err(ReplayError::Failed)));
}
