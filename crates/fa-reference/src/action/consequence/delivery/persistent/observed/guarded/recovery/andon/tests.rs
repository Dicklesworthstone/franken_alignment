use super::*;
use super::super::super::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements};
use super::super::super::super::{BaseEvent, Event, journal};
use super::super::super::super::credibility::{CredibilityEvent, andon::tests as f};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use crate::round::Verdict;
use crate::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None,
        source: None, identity: None, campaigns: None, credential: None }
}
fn seeded(root: &f::Directory, profile: FileOversightProfile) -> FileOversight {
    let (mut host, roles) = FileOversight::create_andon_guarded(root.store(), profile,
        &guards(), None, f::protocol(), &f::config()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    f::case(&mut host, 1, Verdict::Hold);
    f::case(&mut host, 2, Verdict::Hold);
    f::assess(&mut host, &roles.evaluator, 2, GroundTruth::Benign).unwrap();
    f::assess(&mut host, &roles.evaluator, 1, GroundTruth::Censored).unwrap();
    host
}
fn requirements(host: &FileOversight, profile: &FileOversightProfile) -> FileAndonRecoveryRequirements {
    let c = host.inspect().control;
    FileAndonRecoveryRequirements::new(FileRecoveryRequirements {
        guards: guards(), effective_policy: profile.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: c.sequence,
            authority_epoch: c.ledger.epoch },
    }, f::config(), host.andon_ledger().unwrap().windows().values().map(|w| w.budget).collect(),
        host.andon_source().unwrap().revision()).unwrap()
}
fn ready(run: &mut FileAndonRecovery) {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
}

#[test]
fn one_event_steps_match_original_whole_history_and_fence_without_early_publication() {
    let root = f::Directory::new(); let p = f::profile(); let host = seeded(&root, p.clone());
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap();
    let before = root.bytes(); let total = host.revision() as usize;
    let report = host.credibility_report().unwrap(); let accounting = host.andon_ledger().unwrap().clone();
    let receipts: Vec<_> = host.andon_source().unwrap().receipts().iter().map(|r| (**r).clone()).collect();
    let mut oracle = Machine::replay(&p, &host.events).unwrap();
    oracle.apply(&Event::Core(BaseEvent::Fence)).unwrap();
    let oracle_cut = oracle.snapshot(total + 1); drop(host);
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &expected,
        &f::protocol(), &anchor).unwrap();
    assert_eq!(run.progress().attempted_events, 0);
    assert!(matches!(storage::Store::open(&root.store()), Err(JournalError::Busy)));
    for event in 0..total {
        let step = run.advance(event, 1).unwrap();
        assert_eq!((step.replayed_events, step.attempted_events), (event + 1, event + 1));
        assert_eq!(root.bytes(), before);
    }
    let done = run.progress(); assert_eq!(done.status, FileGuardedRecoveryStatus::Ready);
    assert_eq!(run.advance(total, 1).unwrap(), done);
    let (mut restored, roles) = run.finish().unwrap();
    assert_eq!(restored.inspect(), oracle_cut);
    assert_eq!(restored.credibility_report().unwrap(), report);
    assert_eq!(restored.andon_ledger().unwrap(), &accounting);
    assert_eq!(restored.andon_source().unwrap().receipts().iter().map(|r| (**r).clone()).collect::<Vec<_>>(), receipts);
    assert!(!restored.clock_ready());
    assert_eq!(f::assess(&mut restored, &roles.evaluator, 1, GroundTruth::Benign), Err(JournalError::Contract(Error::Incomplete)));
    restored.observe_time(restored.revision(), ElapsedTick(40)).unwrap();
    assert!(f::assess(&mut restored, &roles.evaluator, 1, GroundTruth::Benign).unwrap());
    assert_eq!(restored.andon_receipt(101).unwrap().confirmation().occurred_at, 1);
    assert_eq!(restored.andon_receipt(101).unwrap().confirmation().confirmed_at, 40);
    assert_eq!(restored.andon_ledger().unwrap().windows()[&1].incident.as_ref().unwrap().opened_by_round, 101);
}

#[test]
fn cancellation_at_every_boundary_including_ready_preserves_canonical_and_staging() {
    let root = f::Directory::new(); let p = f::profile(); let host = seeded(&root, p.clone());
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap();
    let total = host.revision() as usize; let original = root.bytes(); drop(host);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"unacknowledged-staging").unwrap();
    for boundary in 0..=total {
        let mut run = FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &expected,
            &f::protocol(), &anchor).unwrap();
        for event in 0..boundary { run.advance(event, 1).unwrap(); }
        let progress = run.cancel();
        assert_eq!(progress.status, FileGuardedRecoveryStatus::Cancelled);
        assert_eq!(progress.replayed_events, boundary);
        assert_eq!(root.bytes(), original);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged-staging");
        drop(storage::Store::open(&root.store()).unwrap());
    }
    let (restored, _) = FileOversight::open_andon_guarded(root.store(), p, &expected, &f::protocol(), &anchor).unwrap();
    assert_eq!(restored.revision(), total as u64 + 1);
    assert!(!pending.exists());
}

#[test]
fn invalid_quantum_stale_predecessor_and_incomplete_finish_do_no_extra_work() {
    let root = f::Directory::new(); let p = f::profile(); let host = seeded(&root, p.clone());
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap(); let before = root.bytes(); drop(host);
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p, &expected, &f::protocol(), &anchor).unwrap();
    let initial = run.progress();
    for (predecessor, quantum, error) in [(1, 1, Error::Stale), (0, 0, Error::InvalidInput),
        (0, MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(run.advance(predecessor, quantum), Err(JournalError::Contract(error)));
        assert_eq!(run.progress(), initial);
    }
    let one = run.advance(0, 1).unwrap();
    assert_eq!(run.advance(0, 1), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(run.progress(), one);
    assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(root.bytes(), before);
}

#[test]
fn failed_suffix_and_final_floor_never_publish_a_valid_prefix_as_ready() {
    let root = f::Directory::new(); let p = f::profile(); let host = seeded(&root, p.clone());
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap();
    let original = root.bytes(); let total = host.revision() as usize; let mut events = host.events.clone();
    events.push(Event::Credibility(CredibilityEvent::Assess(999,
        Assessment { origin: 999, evidence_id: [31; 32], truth: GroundTruth::Violation })));
    let invalid = journal::encode(&p, host.store.identity(), &events).unwrap(); drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &invalid).unwrap();
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &expected, &f::protocol(), &anchor).unwrap();
    assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS), Err(JournalError::Contract(Error::Missing)));
    assert_eq!((run.progress().replayed_events, run.progress().attempted_events), (total, total + 1));
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Failed(Error::Missing));
    assert_eq!(run.advance(total, 1), Err(JournalError::Contract(Error::Missing)));
    assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(root.bytes(), invalid);
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    let higher = FileAndonRecoveryRequirements::new(expected.base().clone(), expected.config().clone(),
        expected.windows().to_vec(), expected.minimum_evaluation_revision() + 1).unwrap();
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &higher, &f::protocol(), &anchor).unwrap();
    assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(run.progress().replayed_events, total);
    assert_eq!(run.cancel().status, FileGuardedRecoveryStatus::Failed(Error::Stale));
    assert_eq!(root.bytes(), original);
    assert!(FileOversight::open_andon_guarded(root.store(), p, &expected, &f::protocol(), &anchor).is_ok());
}

#[test]
fn caught_original_event_unwind_latches_private_state_and_leaves_storage_unchanged() {
    let root = f::Directory::new(); let p = f::profile(); let host = seeded(&root, p.clone());
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap(); let before = root.bytes(); drop(host);
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p, &expected, &f::protocol(), &anchor).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| run.inner.interrupt_after_original_event())).is_err());
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Interrupted);
    assert_eq!((run.progress().replayed_events, run.progress().attempted_events), (0, 1));
    assert_eq!(run.advance(0, 1), Err(JournalError::Unavailable));
    assert!(matches!(run.finish(), Err(JournalError::Unavailable)));
    assert_eq!(root.bytes(), before);
}

#[test]
fn canonical_change_after_ready_refuses_before_cleaning_staging() {
    let root = f::Directory::new(); let p = f::profile(); let host = seeded(&root, p.clone());
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap();
    let original = root.bytes(); let mut events = host.events.clone(); let last = events.len() - 1;
    events.swap(last, last - 1);
    Machine::replay(&p, &events).unwrap();
    let alternate = journal::encode(&p, host.store.identity(), &events).unwrap(); drop(host);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retain-on-refusal").unwrap();
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &expected, &f::protocol(), &anchor).unwrap();
    ready(&mut run);
    std::fs::write(root.store().join(storage::CANONICAL), &alternate).unwrap();
    assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(root.bytes(), alternate);
    assert_eq!(std::fs::read(&pending).unwrap(), b"retain-on-refusal");
    std::fs::write(root.store().join(storage::CANONICAL), original).unwrap();
    assert!(FileOversight::open_andon_guarded(root.store(), p, &expected, &f::protocol(), &anchor).is_ok());
}

#[test]
fn exact_fence_capacity_and_each_store_barrier_keep_the_original_acknowledgment_boundary() {
    let count = {
        let root = f::Directory::new(); seeded(&root, f::profile()).revision() as usize
    };
    for extra in [0, 1] {
        let root = f::Directory::new(); let mut p = f::profile(); p.delivery.limits.events = count + extra;
        let host = seeded(&root, p.clone()); let expected = requirements(&host, &p);
        let anchor = host.history_anchor().unwrap(); let original = root.bytes(); drop(host);
        let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"staged").unwrap();
        let mut run = FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &expected, &f::protocol(), &anchor).unwrap();
        ready(&mut run);
        if extra == 0 {
            assert!(matches!(run.finish(), Err(JournalError::Contract(Error::Limit))));
            assert_eq!(root.bytes(), original); assert_eq!(std::fs::read(pending).unwrap(), b"staged");
        } else {
            let (host, _) = run.finish().unwrap(); assert_eq!(host.revision(), (count + 1) as u64);
            assert_eq!(host.journal_capacity().unwrap().remaining().events, 0); assert!(!pending.exists());
        }
    }
    for stage in f::BARRIERS {
        let root = f::Directory::new(); let p = f::profile(); let host = seeded(&root, p.clone());
        let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap();
        let report = host.credibility_report().unwrap(); let accounting = host.andon_ledger().unwrap().clone();
        let before = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(stage);
        let mut run = FileAndonRecovery::begin_store(store, p.clone(), &expected, &f::protocol(), &anchor).unwrap();
        ready(&mut run);
        let Err(JournalError::Io(failure)) = run.finish() else { panic!("expected original Store failure"); };
        assert_eq!(failure.operation, stage);
        let observed = FileOversight::read_andon_guarded(root.store(), &p, &expected, &f::protocol(), &anchor).unwrap();
        assert_eq!(observed.credibility.report, report); assert_eq!(observed.accounting, accounting);
        assert_eq!(observed.credibility.journal.revision, before + u64::from(stage == JournalIo::DirectorySync));
    }
}

#[test]
fn reserve_pin_rejects_absence_drift_and_late_selection_without_refilling_capacity() {
    let root = f::Directory::new(); let p = f::profile();
    let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
    let reserve = RecoveryReserve::terminal();
    host.enable_recovery_reserve(host.revision(), reserve).unwrap();
    let evaluator = host.enable_credibility(host.revision(), f::protocol()).unwrap();
    host.enable_andon(host.revision(), &f::config()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    f::case(&mut host, 1, Verdict::Hold); f::assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap();
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap();
    let before = root.bytes(); let capacity = host.journal_capacity().unwrap(); drop(host);
    let begin = || FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &expected, &f::protocol(), &anchor).unwrap();
    let wrong = RecoveryReserve { events: reserve.events + 1, ..reserve };
    assert!(matches!(begin().require_recovery_reserve(wrong), Err(JournalError::Contract(Error::Binding))));
    let mut late = begin(); late.advance(0, 1).unwrap();
    assert!(matches!(late.require_recovery_reserve(reserve), Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(root.bytes(), before);
    let mut run = begin().require_recovery_reserve(reserve).unwrap(); ready(&mut run);
    let (restored, _) = run.finish().unwrap(); let after = restored.journal_capacity().unwrap();
    assert_eq!(after.reserve(), Some(reserve));
    assert_eq!(after.remaining().events + 1, capacity.remaining().events);
    let other = f::Directory::new(); let host = seeded(&other, p.clone());
    let expected = requirements(&host, &p); let anchor = host.history_anchor().unwrap(); let before = other.bytes(); drop(host);
    let run = FileOversight::begin_open_andon_guarded(other.store(), p, &expected, &f::protocol(), &anchor).unwrap();
    assert!(matches!(run.require_recovery_reserve(reserve), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(other.bytes(), before);
}
