//! Exact-history recovery supplements, rather than upgrades, the CRC contract.
use fa_journal::replay::{HistoryAnchor, Replay, ReplayBudget, ReplayError, ReplayStatus};
use fa_journal::{Error, Event, Identity, Journal, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::io::{self, Read};

fn identity() -> Identity {
    Identity { run: [1; 32], policy: [2; 32], total_units: 10 }
}

fn effect(payload: &[u8]) -> Effect {
    Effect {
        principal: "actor".into(),
        resolved_target: "publication".into(),
        payload: payload.to_vec(),
        units: 4,
    }
}

fn append(journal: &mut Journal, bytes: &mut Vec<u8>, event: Event) {
    bytes.extend(journal.append(&event).unwrap());
}

fn complete<R: Read>(mut replay: Replay<R>, quantum: u64) -> Journal {
    while replay.progress().status == ReplayStatus::Reading {
        let report = replay.advance(ReplayBudget {
            read_calls: 2,
            read_bytes: quantum,
            checked_records: 1,
        }).unwrap();
        assert!(report.spent.read_calls <= 2);
        assert!(report.spent.read_bytes <= quantum);
        assert!(report.spent.checked_records <= 1);
    }
    replay.finish().unwrap()
}

#[test]
fn exact_prefix_and_valid_successors_preserve_original_accounting_at_each_quantum() {
    let limits = Limits::default();
    let mut journal = Journal::new(identity(), limits).unwrap();
    let initial = journal.head();
    let mut bytes = journal.header();
    let payload = effect(b"proposal");
    append(&mut journal, &mut bytes, Event::Reserve { id: 7, effect: payload.clone() });
    append(&mut journal, &mut bytes, Event::Dispatch { id: 7, effect: payload });
    let anchor = HistoryAnchor::from_bytes(bytes.clone(), identity(), limits).unwrap();
    assert_eq!(anchor.head(), journal.head());
    assert_eq!(anchor.as_bytes(), bytes);
    for quantum in [1, 7, 4096, u64::MAX] {
        let replay = Journal::begin_replay_anchored(bytes.as_slice(), identity(), anchor.clone(), limits).unwrap();
        let exact = complete(replay, quantum);
        assert_eq!(exact.rights().state(7).unwrap(), State::Dispatched);
        assert_eq!(exact.rights().available(), 6);
    }
    append(&mut journal, &mut bytes, Event::MarkUnknown { id: 7 });
    append(&mut journal, &mut bytes, Event::RevokeEpoch);
    append(&mut journal, &mut bytes, Event::Reconcile { id: 7, occurred: true });
    let oracle = Journal::replay(bytes.as_slice(), identity(), initial, limits).unwrap();
    for quantum in [1, 7, 4096, u64::MAX] {
        let replay = Journal::begin_replay_anchored(bytes.as_slice(), identity(), anchor.clone(), limits).unwrap();
        let recovered = complete(replay, quantum);
        assert_eq!(recovered.head(), oracle.head());
        assert_eq!(recovered.encoded_len(), oracle.encoded_len());
        assert_eq!(recovered.rights(), oracle.rights());
        assert_eq!(recovered.rights().state(7).unwrap(), State::Committed);
        assert!(recovered.rights().conserved());
    }
}

#[test]
fn a_valid_crc_collision_fork_passes_legacy_head_but_not_exact_history() {
    // Equal-length IEEE CRC-32 collision; actual public codec confirms it below.
    let limits = Limits::default();
    let mut left = Journal::new(identity(), limits).unwrap();
    let mut right = Journal::new(identity(), limits).unwrap();
    let mut left_bytes = left.header();
    let mut right_bytes = right.header();
    append(&mut left, &mut left_bytes, Event::Reserve { id: 1, effect: effect(b"plumless") });
    append(&mut right, &mut right_bytes, Event::Reserve { id: 1, effect: effect(b"buckeroo") });
    assert_eq!(left.head(), right.head());
    assert_ne!(left_bytes, right_bytes);
    Journal::replay(right_bytes.as_slice(), identity(), left.head(), limits).unwrap();
    let anchor = HistoryAnchor::from_bytes(left_bytes.clone(), identity(), limits).unwrap();
    for quantum in [1, 7, 4096] {
        let mut replay = Journal::begin_replay_anchored(right_bytes.as_slice(), identity(), anchor.clone(), limits).unwrap();
        loop {
            match replay.advance(ReplayBudget { read_calls: 1, read_bytes: quantum, checked_records: 1 }) {
                Ok(report) => assert_eq!(report.status, ReplayStatus::Reading),
                Err(ReplayError::Journal(Error::Binding)) => break,
                other => panic!("unexpected fork result: {other:?}"),
            }
        }
        assert_eq!(replay.progress().status, ReplayStatus::Failed);
        assert!(replay.progress().total.read_bytes > left.header().len() as u64);
        assert!(matches!(replay.finish(), Err(ReplayError::Failed)));
    }
    let permitted = Journal::begin_replay_anchored(left_bytes.as_slice(), identity(), anchor, limits).unwrap();
    assert_eq!(complete(permitted, 1).head(), left.head());
}

#[test]
fn rollback_partial_prefix_and_invalid_suffix_never_export_rights() {
    let limits = Limits::default();
    let mut journal = Journal::new(identity(), limits).unwrap();
    let mut bytes = journal.header();
    append(&mut journal, &mut bytes, Event::Reserve { id: 1, effect: effect(b"x") });
    let anchor = HistoryAnchor::from_bytes(bytes.clone(), identity(), limits).unwrap();
    let header = journal.header();
    let mut partial = bytes.clone();
    partial.pop();
    let mut invalid_suffix = bytes.clone();
    invalid_suffix.push(0);
    for bad in [header, partial, invalid_suffix] {
        let mut replay = Journal::begin_replay_anchored(bad.as_slice(), identity(), anchor.clone(), limits).unwrap();
        assert!(replay.advance(ReplayBudget::UNLIMITED).is_err());
        assert!(matches!(replay.finish(), Err(ReplayError::Failed)));
    }
}

struct NoReads;
impl Read for NoReads {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("preflight or zero-budget recovery must not read")
    }
}

#[test]
fn context_and_limits_refuse_before_reads_and_zero_budget_remains_incomplete() {
    let limits = Limits::default();
    let journal = Journal::new(identity(), limits).unwrap();
    let anchor = HistoryAnchor::from_bytes(journal.header(), identity(), limits).unwrap();
    let mut foreign = identity();
    foreign.run[0] = 9;
    assert!(matches!(Journal::begin_replay_anchored(NoReads, foreign, anchor.clone(), limits), Err(Error::Binding)));
    let changed = Limits { max_records: limits.max_records - 1, ..limits };
    assert!(matches!(Journal::begin_replay_anchored(NoReads, identity(), anchor.clone(), changed), Err(Error::Binding)));
    let mut replay = Journal::begin_replay_anchored(NoReads, identity(), anchor, limits).unwrap();
    assert_eq!(replay.advance(ReplayBudget::default()).unwrap().total.read_calls, 0);
    assert!(matches!(replay.finish(), Err(ReplayError::NotReady)));
}

#[test]
fn anchor_import_uses_original_complete_decoder_and_limits() {
    let limits = Limits::default();
    let mut journal = Journal::new(identity(), limits).unwrap();
    let mut bytes = journal.header();
    append(&mut journal, &mut bytes, Event::RevokeEpoch);
    for end in 0..bytes.len() {
        if end != journal.header().len() {
            assert!(HistoryAnchor::from_bytes(bytes[..end].to_vec(), identity(), limits).is_err());
        }
    }
    let short = Limits { max_total_bytes: journal.header().len() as u64, ..limits };
    assert!(matches!(HistoryAnchor::from_bytes(bytes.clone(), identity(), short), Err(Error::Limit)));
    let mut damaged = bytes.clone();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(HistoryAnchor::from_bytes(damaged, identity(), limits).is_err());
    HistoryAnchor::from_bytes(bytes, identity(), limits).unwrap();
}

#[test]
fn reserved_profile_is_explicit_and_preserves_unknown_charges_and_tail_capacity() {
    let limits = Limits { max_records: 5, max_frame_bytes: 1024, max_total_bytes: 4096 };
    let reserve = RecoveryReserve { records: 2, bytes: 60 };
    let mut journal = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    let mut bytes = journal.header();
    let payload = effect(b"x");
    append(&mut journal, &mut bytes, Event::Reserve { id: 1, effect: payload.clone() });
    append(&mut journal, &mut bytes, Event::Dispatch { id: 1, effect: payload });
    let anchor = HistoryAnchor::from_bytes_with_recovery_reserve(bytes.clone(), identity(), limits, reserve).unwrap();
    assert!(HistoryAnchor::from_bytes(bytes.clone(), identity(), limits).is_err());
    assert!(Journal::begin_replay_anchored(NoReads, identity(), anchor.clone(), limits).is_err());
    let wrong = RecoveryReserve { records: 1, bytes: 30 };
    assert!(Journal::begin_replay_anchored_with_recovery_reserve(NoReads, identity(), anchor.clone(), limits, wrong).is_err());
    append(&mut journal, &mut bytes, Event::MarkUnknown { id: 1 });
    append(&mut journal, &mut bytes, Event::RevokeEpoch);
    let replay = Journal::begin_replay_anchored_with_recovery_reserve(bytes.as_slice(), identity(), anchor, limits, reserve).unwrap();
    let recovered = complete(replay, 1);
    assert_eq!(recovered.rights().state(1).unwrap(), State::Unknown);
    assert_eq!(recovered.rights().available(), 6);
    assert_eq!(recovered.capacity(), journal.capacity());
    assert_eq!(recovered.recovery_reserve(), Some(reserve));
}

#[test]
fn interrupted_reads_spend_calls_and_cancellation_returns_no_partial_ledger() {
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    let limits = Limits::default();
    let journal = Journal::new(identity(), limits).unwrap();
    let anchor = HistoryAnchor::from_bytes(journal.header(), identity(), limits).unwrap();
    let mut replay = Journal::begin_replay_anchored(Interrupted, identity(), anchor, limits).unwrap();
    let report = replay.advance(ReplayBudget { read_calls: 3, read_bytes: 100, checked_records: 1 }).unwrap();
    assert_eq!(report.total.read_calls, 3);
    assert_eq!(report.total.read_bytes, 0);
    assert_eq!(replay.cancel(), report.total);
}
