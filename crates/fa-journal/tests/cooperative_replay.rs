use fa_journal::{Error, Event, Head, Identity, Journal, Limits};
use fa_journal::replay::{Replay, ReplayBudget, ReplayError, ReplayReport, ReplayStatus, ReplayWork};
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
        Event::Dispatch { id: 1, effect: effect(30) },
        Event::MarkUnknown { id: 1 },
        Event::Reserve { id: 2, effect: effect(20) },
        Event::AbortBeforeDispatch { id: 2 },
        Event::Reserve { id: 3, effect: effect(10) },
        Event::Dispatch { id: 3, effect: effect(10) },
        Event::Reconcile { id: 3, occurred: true },
        Event::Reserve { id: 4, effect: effect(5) },
        Event::Dispatch { id: 4, effect: effect(5) },
        Event::Reconcile { id: 4, occurred: false },
        Event::Reserve { id: 5, effect: effect(7) },
        Event::ResetToCheckpoint { keep: BTreeSet::from([1, 3]) },
        Event::RevokeEpoch,
    ] {
        bytes.extend(journal.append(&event).unwrap());
    }
    (journal, bytes, genesis)
}

fn schedules() -> [ReplayBudget; 4] {
    [
        ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 1 },
        ReplayBudget { read_calls: 2, read_bytes: 7, checked_records: 1 },
        ReplayBudget { read_calls: 64, read_bytes: 65_536, checked_records: 3 },
        ReplayBudget::UNLIMITED,
    ]
}

fn poll<R: Read>(run: &mut Replay<R>, budget: ReplayBudget) -> Result<ReplayReport, ReplayError> {
    let before = run.progress().total;
    let report = run.advance(budget)?;
    assert!(report.spent.read_calls <= budget.read_calls);
    assert!(report.spent.read_bytes <= budget.read_bytes);
    assert!(report.spent.checked_records <= budget.checked_records);
    assert_eq!(report.total.read_calls, before.read_calls + report.spent.read_calls);
    assert_eq!(report.total.read_bytes, before.read_bytes + report.spent.read_bytes);
    assert_eq!(report.total.checked_records, before.checked_records + report.spent.checked_records);
    Ok(report)
}

fn finish<R: Read>(mut run: Replay<R>, budget: ReplayBudget) -> Result<Journal, ReplayError> {
    for _ in 0..20_000 {
        let report = poll(&mut run, budget)?;
        if report.status == ReplayStatus::Ready {
            let repeated = poll(&mut run, ReplayBudget::default())?;
            assert_eq!(repeated.spent, ReplayWork::default());
            assert_eq!(repeated.total, report.total);
            return run.finish();
        }
        assert_ne!(report.spent, ReplayWork::default());
    }
    panic!("bounded fixture did not finish");
}

fn class(error: Error) -> String {
    match error {
        Error::Io(error) => format!("io:{:?}", error.kind()),
        Error::Model(error) => format!("model:{error:?}"),
        other => format!("{other:?}"),
    }
}

fn compare(bytes: &[u8], context: Identity, minimum: Head, limits: Limits, budget: ReplayBudget) {
    let eager = Journal::replay(bytes, context, minimum, limits);
    let cooperative = Journal::begin_replay(bytes, context, minimum, limits)
        .map_err(ReplayError::Journal).and_then(|run| finish(run, budget));
    match (eager, cooperative) {
        (Ok(left), Ok(right)) => {
            assert_eq!(left.head(), right.head());
            assert_eq!(left.encoded_len(), right.encoded_len());
            assert_eq!(left.rights(), right.rights());
            assert_eq!(left.identity(), right.identity());
        }
        (Err(left), Err(ReplayError::Journal(right))) => assert_eq!(class(left), class(right)),
        pair => panic!("eager/cooperative mismatch: {pair:?}"),
    }
}

#[test]
fn every_transition_and_unknown_suffix_match_original_replay_under_all_schedules() {
    let (expected, bytes, genesis) = fixture();
    for minimum in [genesis, expected.head()] {
        for budget in schedules() {
            compare(&bytes, identity(), minimum, Limits::default(), budget);
            let run = Journal::begin_replay(bytes.as_slice(), identity(), minimum, Limits::default()).unwrap();
            let recovered = finish(run, budget).unwrap();
            assert_eq!(recovered.rights().state(1), Ok(State::Unknown));
            assert_eq!(recovered.rights().state(2), Ok(State::Aborted));
            assert_eq!(recovered.rights().state(3), Ok(State::Committed));
            assert_eq!(recovered.rights().state(4), Ok(State::Aborted));
            assert_eq!(recovered.rights().state(5), Ok(State::Aborted));
            assert_eq!(recovered.rights(), expected.rights());
            assert!(recovered.rights().conserved());
        }
    }
}

#[test]
fn reaching_an_anchor_or_a_complete_record_does_not_substitute_for_eof() {
    let mut expected = Journal::new(identity(), Limits::default()).unwrap();
    let mut bytes = expected.header();
    bytes.extend(expected.append(&Event::RevokeEpoch).unwrap());
    for finish_early in [false, true] {
        let mut run = Journal::begin_replay(bytes.as_slice(), identity(), expected.head(), Limits::default()).unwrap();
        assert_eq!(poll(&mut run, ReplayBudget::default()).unwrap().total, ReplayWork::default());
        let buffered = poll(&mut run, ReplayBudget {
            checked_records: 0, ..ReplayBudget::UNLIMITED
        }).unwrap();
        assert_eq!(buffered.total.read_bytes, bytes.len() as u64);
        assert_eq!(buffered.total.checked_records, 0);
        assert_eq!(buffered.status, ReplayStatus::Reading);
        let checked = poll(&mut run, ReplayBudget {
            checked_records: 1, ..ReplayBudget::default()
        }).unwrap();
        assert_eq!(checked.total.read_bytes, buffered.total.read_bytes);
        assert_eq!(checked.total.checked_records, 1);
        assert_eq!(checked.status, ReplayStatus::Reading);
        if finish_early {
            assert!(matches!(run.finish(), Err(ReplayError::NotReady)));
        } else {
            let done = poll(&mut run, ReplayBudget {
                read_calls: 1, read_bytes: 1, checked_records: 0,
            }).unwrap();
            assert_eq!(done.status, ReplayStatus::Ready);
            assert_eq!(done.spent.read_bytes, 0);
            assert_eq!(done.spent.read_calls, 1);
            assert_eq!(run.finish().unwrap().rights(), expected.rights());
        }
    }
}

#[test]
fn all_truncations_and_single_byte_damage_match_the_original_error_boundary() {
    let (journal, bytes, _) = fixture();
    for end in 0..bytes.len() {
        compare(&bytes[..end], identity(), journal.head(), Limits::default(), schedules()[1]);
    }
    for offset in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        compare(&changed, identity(), journal.head(), Limits::default(), schedules()[1]);
    }
}

#[test]
fn context_anchor_and_lifetime_limits_cannot_be_reset_by_polling() {
    let (journal, bytes, genesis) = fixture();
    for budget in schedules() {
        for context in [
            Identity { run: [9; 32], ..identity() },
            Identity { policy: [9; 32], ..identity() },
            Identity { total_units: 101, ..identity() },
        ] {
            compare(&bytes, context, genesis, Limits::default(), budget);
        }
        for minimum in [
            Head { checksum: genesis.checksum ^ 1, ..genesis },
            Head { checksum: journal.head().checksum ^ 1, ..journal.head() },
            Head { sequence: journal.head().sequence + 1, checksum: 0 },
        ] {
            compare(&bytes, identity(), minimum, Limits::default(), budget);
        }
        for limits in [
            Limits { max_records: journal.head().sequence, max_total_bytes: bytes.len() as u64, ..Limits::default() },
            Limits { max_records: journal.head().sequence - 1, ..Limits::default() },
            Limits { max_total_bytes: bytes.len() as u64 - 1, ..Limits::default() },
            Limits { max_frame_bytes: 13, ..Limits::default() },
        ] {
            compare(&bytes, identity(), genesis, limits, budget);
        }
    }
}

struct Fragmented<'a> {
    bytes: &'a [u8],
    interrupt: bool,
}

impl Read for Fragmented<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.interrupt = !self.interrupt;
        if self.interrupt { return Err(io::ErrorKind::Interrupted.into()); }
        let length = out.len().min(3).min(self.bytes.len());
        out[..length].copy_from_slice(&self.bytes[..length]);
        self.bytes = &self.bytes[length..];
        Ok(length)
    }
}

#[test]
fn fragmented_reads_never_restart_or_double_charge_a_returned_byte() {
    let (expected, bytes, minimum) = fixture();
    let mut run = Journal::begin_replay(Fragmented { bytes: &bytes, interrupt: false },
        identity(), minimum, Limits::default()).unwrap();
    for _ in 0..20_000 {
        let report = poll(&mut run, schedules()[0]).unwrap();
        if report.status == ReplayStatus::Ready { break; }
    }
    assert_eq!(run.progress().total.read_bytes, bytes.len() as u64);
    assert_eq!(run.progress().total.checked_records, expected.head().sequence);
    assert_eq!(run.finish().unwrap().rights(), expected.rights());
}

struct Interrupted;
impl Read for Interrupted {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::Interrupted.into())
    }
}

#[test]
fn a_permanently_interrupted_reader_yields_and_cancellation_retains_work() {
    let minimum = Journal::new(identity(), Limits::default()).unwrap().head();
    let mut run = Journal::begin_replay(Interrupted, identity(), minimum, Limits::default()).unwrap();
    for total in [7, 14, 21] {
        let report = poll(&mut run, ReplayBudget { read_calls: 7, ..ReplayBudget::UNLIMITED }).unwrap();
        assert_eq!(report.status, ReplayStatus::Reading);
        assert_eq!(report.total.read_calls, total);
        assert_eq!(report.total.read_bytes, 0);
        assert_eq!(report.total.checked_records, 0);
    }
    assert_eq!(run.cancel().read_calls, 21);
}

#[test]
fn hostile_length_refuses_before_payload_reads_or_record_checks_and_stays_failed() {
    let original = Journal::new(identity(), Limits::default()).unwrap();
    let mut bytes = original.header();
    bytes.extend(u32::MAX.to_le_bytes());
    bytes.extend([1; 32]);
    let mut run = Journal::begin_replay(bytes.as_slice(), identity(), original.head(), Limits::default()).unwrap();
    assert!(matches!(run.advance(ReplayBudget::UNLIMITED), Err(ReplayError::Journal(Error::Limit))));
    let failed = run.progress();
    assert_eq!(failed.status, ReplayStatus::Failed);
    assert_eq!(failed.total.read_bytes, 88);
    assert_eq!(failed.total.checked_records, 0);
    assert!(matches!(run.advance(ReplayBudget::UNLIMITED), Err(ReplayError::Failed)));
    assert_eq!(run.progress(), failed);
    assert!(matches!(run.finish(), Err(ReplayError::Failed)));
}

struct FailingReader { panic: bool }
impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        assert!(!self.panic, "injected reader unwind");
        Err(io::ErrorKind::WouldBlock.into())
    }
}

#[test]
fn read_errors_and_unwinds_never_leave_a_resumable_private_prefix() {
    let minimum = Journal::new(identity(), Limits::default()).unwrap().head();
    for panic in [false, true] {
        let mut run = Journal::begin_replay(FailingReader { panic }, identity(), minimum, Limits::default()).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.advance(ReplayBudget::UNLIMITED)));
        if panic { assert!(result.is_err()); }
        else {
            assert!(matches!(result.unwrap(), Err(ReplayError::Journal(Error::Io(error)))
                if error.kind() == io::ErrorKind::WouldBlock));
        }
        assert_eq!(run.progress().status, ReplayStatus::Failed);
        assert_eq!(run.progress().total.read_calls, 1);
        assert!(matches!(run.advance(ReplayBudget::UNLIMITED), Err(ReplayError::Failed)));
        assert!(matches!(run.finish(), Err(ReplayError::Failed)));
    }
}
