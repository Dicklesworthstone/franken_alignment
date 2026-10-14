//! Original native labels, two-key effects and Store barriers; no fake reducer.
use super::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation, MAX_JOURNAL_EVENTS};
use crate::action::consequence::delivery::persistent::observed::{BaseEvent, Event, Machine, journal};
use crate::action::consequence::delivery::persistent::observed::guarded::{FileRecoveryFloor, FileRecoveryRequirements};
use crate::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::FileAndonRecoveryRequirements;
use crate::action::consequence::delivery::persistent::observed::credibility::andon::tests as native;
use crate::action::consequence::oversight::credibility::GroundTruth;
use crate::round::Verdict;
use std::{cell::Cell, fs};
#[path = "../../reconciled/fixture.rs"]
mod fixture;
use fixture::{Directory, Ready, guards, requirements};
use native::{profile, protocol, config, case, assess, BARRIERS};

fn owner(root: &Directory) -> (FileOversight, FileEvaluatedOversightRoles) {
    FileOversight::create_andon_guarded_reserved(root.store(), profile(), &guards(), None,
        protocol(), &config(), RecoveryReserve::terminal()).unwrap()
}
fn expected(host: &FileOversight) -> FileAndonRecoveryRequirements {
    FileAndonRecoveryRequirements::new(requirements(host, guards()), config(),
        config().windows().to_vec(), host.credibility_report().unwrap().revision).unwrap()
}
fn ready(run: &mut FileAndonRecovery, quantum: usize) {
    while run.progress().status == super::super::FileGuardedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum).unwrap();
        assert_eq!(after.attempted_events, after.replayed_events);
        assert!(after.replayed_events > before.replayed_events);
        assert!(after.attempted_events - before.attempted_events <= quantum);
    }
    assert_eq!(run.progress().status, super::super::FileGuardedRecoveryStatus::Ready);
}

#[test]
fn reserve_and_all_native_guards_are_in_the_first_acknowledged_image() {
    let root = Directory::new(); let (host, _) = owner(&root);
    assert_eq!(host.revision(), 4);
    assert!(host.publication_guard_required());
    assert_eq!(host.andon_config().unwrap(), Some(&config()));
    assert_eq!(host.credibility_protocol().unwrap(), Some(&protocol()));
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    let anchor = host.history_anchor().unwrap(); let expected = expected(&host);
    let original = root.bytes(); drop(host);
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), profile(), &expected, &protocol(), &anchor)
        .unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    ready(&mut run, 1); assert_eq!(root.bytes(), original);
    let (host, _, outcomes) = run.finish_reconciled(|| ElapsedTick(1)).unwrap();
    assert_eq!(host.revision(), 7); assert!(outcomes.is_empty());
    assert!(host.clock_ready()); assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    let legacy = Directory::new();
    let (host, _) = FileOversight::create_andon_guarded(legacy.store(), profile(), &guards(), None, protocol(), &config()).unwrap();
    assert_eq!(host.revision(), 3); assert_eq!(host.journal_capacity().unwrap().reserve(), None);
}

#[test]
fn invalid_reserve_refuses_before_creation_and_bootstrap_barriers_return_no_roles() {
    for reserve in [RecoveryReserve { events: 2, bytes: 50 },
        RecoveryReserve { events: 3, bytes: 49 },
        RecoveryReserve { events: usize::MAX, bytes: 50 }] {
        let root = Directory::new();
        assert!(FileOversight::create_andon_guarded_reserved(root.store(), profile(), &guards(), None,
            protocol(), &config(), reserve).is_err());
        assert!(!root.store().exists());
    }
    for stage in BARRIERS {
        let root = Directory::new();
        let prepared = PreparedGuardedBootstrap::prepare_reserved(profile(), &guards(), None, RecoveryReserve::terminal())
            .unwrap().evaluated(protocol()).unwrap().with_andon(&config()).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(stage);
        assert!(prepared.publish(store).is_err());
        if stage == JournalIo::DirectorySync {
            let bytes = root.bytes();
            let identity = storage::identity(&root.store()).unwrap();
            let events = journal::decode(&profile(), &identity, &bytes).unwrap();
            let machine = Machine::replay(&profile(), &events).unwrap();
            assert_eq!(events.len(), 4);
            assert_eq!(machine.broker.andon_ledger().unwrap().windows()[&1].false_stops, 0);
            assert_eq!(machine.broker.credibility_report().unwrap().retained_cases, 0);
        } else { assert!(!root.store().join("delivery.bin").exists()); }
    }
}

#[test]
fn reconciled_native_cut_matches_original_history_for_every_effect_disposition() {
    for mode in 0..4 {
        let root = Directory::new(); let (mut host, roles) = owner(&root);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        case(&mut host, 1, Verdict::Hold);
        assess(&mut host, &roles.evaluator, 1, GroundTruth::Benign).unwrap();
        let mut prepared = Ready::from_owner(host, roles.oversight.human, 2, 102, ElapsedTick(1));
        if mode > 0 { prepared.dispatch(); }
        if mode > 1 { prepared.publish_at(2, ElapsedTick(2)); }
        let now = ElapsedTick(if mode == 3 { 2000 } else { 3 });
        let basis = expected(&prepared.host); let anchor = prepared.host.history_anchor().unwrap();
        let original = root.bytes(); let before = prepared.host.inspect();
        let ledger = prepared.host.andon_ledger().unwrap().clone();
        let receipt = prepared.host.andon_receipt(101).unwrap().clone();
        let mut events = prepared.host.events.clone();
        events.extend([Event::Core(BaseEvent::Fence), Event::Core(BaseEvent::Time(now)), Event::Core(BaseEvent::Sweep)]);
        let mut oracle = Machine::new(&profile()).unwrap(); let mut outcomes = None;
        for event in &events {
            if let crate::action::consequence::delivery::persistent::observed::machine::Transition::Swept(result)
                = oracle.apply(event).unwrap() { outcomes = Some(result); }
        }
        let expected_state = oracle.snapshot(events.len());
        let bytes = journal::encode(&profile(), prepared.host.store.identity(), &events).unwrap();
        let outcomes = outcomes.unwrap(); drop(prepared);
        for quantum in [1, MAX_JOURNAL_EVENTS] {
            fs::write(root.store().join("delivery.bin"), &original).unwrap();
            let mut run = FileOversight::begin_open_andon_guarded(root.store(), profile(), &basis, &protocol(), &anchor).unwrap();
            ready(&mut run, quantum);
            let (host, _, actual) = run.finish_reconciled(|| now).unwrap();
            assert_eq!(host.revision(), before.revision + 3);
            assert_eq!(host.inspect(), expected_state); assert_eq!(actual, outcomes); assert_eq!(root.bytes(), bytes);
            assert_eq!(host.andon_ledger().unwrap(), &ledger); assert_eq!(host.andon_receipt(101).unwrap(), &receipt);
            assert_eq!(host.inspect().executions, u64::from(mode > 1));
        }
    }
}

#[test]
fn fresh_evaluator_can_resolve_pending_case_but_old_brand_cannot() {
    let root = Directory::new(); let (mut host, old) = owner(&root);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); case(&mut host, 1, Verdict::Hold);
    assess(&mut host, &old.evaluator, 1, GroundTruth::Censored).unwrap();
    let basis = expected(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), profile(), &basis, &protocol(), &anchor).unwrap();
    ready(&mut run, 1); let (mut host, fresh, outcomes) = run.finish_reconciled(|| ElapsedTick(3)).unwrap();
    assert!(outcomes.values().all(|result| matches!(result, Ok(Reconciliation::Resolved(_)))));
    let before = root.bytes();
    assert!(assess(&mut host, &old.evaluator, 1, GroundTruth::Benign).is_err()); assert_eq!(root.bytes(), before);
    assert!(assess(&mut host, &fresh.evaluator, 1, GroundTruth::Benign).unwrap());
    assert_eq!(host.andon_receipt(101).unwrap().confirmation().confirmed_at, 3);
    assert_eq!(host.evaluation_history(101).unwrap().len(), 2);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
}

#[test]
fn incomplete_or_changed_canonical_cut_does_not_call_clock_or_clean_staging() {
    let root = Directory::new(); let (host, _) = owner(&root);
    let basis = expected(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"diagnostic staging").unwrap(); let original = root.files();
    let run = FileOversight::begin_open_andon_guarded(root.store(), profile(), &basis, &protocol(), &anchor).unwrap();
    assert!(run.finish_reconciled(|| panic!("incomplete clock")).is_err()); assert_eq!(root.files(), original);
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), profile(), &basis, &protocol(), &anchor).unwrap();
    ready(&mut run, 1); fs::write(root.store().join("delivery.bin"), b"changed canonical").unwrap();
    let changed = root.files();
    assert!(run.finish_reconciled(|| panic!("changed canonical clock")).is_err()); assert_eq!(root.files(), changed);
}

#[test]
fn live_reconciliation_never_spends_the_terminal_only_tail() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 9;
    let (mut host, _) = FileOversight::create_andon_guarded_reserved(root.store(), p.clone(), &guards(), None,
        protocol(), &config(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let basis = expected(&host); let anchor = host.history_anchor().unwrap(); drop(host);
    let original = root.bytes();
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p.clone(), &basis, &protocol(), &anchor).unwrap();
    ready(&mut run, 1); assert!(run.finish_reconciled(|| ElapsedTick(3)).is_err()); assert_eq!(root.bytes(), original);
    // The failed live transaction did not spend its preliminary Fence.
    let mut run = FileOversight::begin_open_andon_guarded(root.store(), p, &basis, &protocol(), &anchor).unwrap();
    ready(&mut run, 1); let (host, _) = run.finish().unwrap(); assert_eq!(host.revision(), 7);
}

#[test]
fn every_completion_barrier_returns_no_roles_and_replay_keeps_native_evidence() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, roles) = owner(&root);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); case(&mut host, 1, Verdict::Hold);
        assess(&mut host, &roles.evaluator, 1, GroundTruth::Benign).unwrap();
        let basis = expected(&host); let anchor = host.history_anchor().unwrap();
        let ledger = host.andon_ledger().unwrap().clone(); let before = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(stage);
        let mut run = FileAndonRecovery::begin_store(store, profile(), &basis, &protocol(), &anchor).unwrap();
        ready(&mut run, 1); let calls = Cell::new(0);
        assert!(run.finish_reconciled(|| { calls.set(calls.get() + 1); ElapsedTick(3) }).is_err());
        assert_eq!(calls.get(), 1);
        let read = FileOversight::read_andon_guarded(root.store(), &profile(), &basis, &protocol(), &anchor).unwrap();
        assert_eq!(read.accounting, ledger); assert_eq!(read.receipts.len(), 1);
        assert_eq!(read.credibility.journal.revision, before + if stage == JournalIo::DirectorySync { 3 } else { 0 });
        assert_eq!(read.credibility.journal.executions, 0);
    }
}
