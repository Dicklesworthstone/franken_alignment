//! Real original replay, original endpoint outcomes and native Store barriers.
use super::*;
use super::super::{FileGuardedRecoveryStatus as Status, storage};
use crate::action::consequence::delivery::persistent::{RecoveryReserve, Reconciliation, MAX_JOURNAL_EVENTS};
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements};
use std::{cell::Cell, fs};
#[path = "fixture.rs"]
mod fixture;
use fixture::*;

fn ready(run: &mut FileGuardedRecovery, quantum: usize) {
    for _ in 0..=run.progress().total_events {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
        if after.status == Status::Ready { return; }
        assert!(after.replayed_events > before.replayed_events);
    }
    panic!("bounded original replay did not complete");
}
fn begin(root: &Directory, expected: &FileRecoveryRequirements) -> FileGuardedRecovery {
    FileOversight::begin_open_guarded(root.store(), profile(), expected).unwrap()
}

#[test]
fn every_disposition_and_quantum_matches_full_original_replay_and_encoded_history() {
    for mode in 0..3 {
        let root = Directory::new(); let mut prepared = Ready::new(&root);
        if mode > 0 { prepared.dispatch(); }
        if mode == 2 { prepared.publish(); }
        let before = prepared.host.inspect(); let expected = requirements(&prepared.host, guards());
        let anchor = prepared.host.history_anchor().unwrap(); let original = root.bytes();
        // Independent batch consumer of the original reducer and canonical
        // encoder, not a second loop over the new completion implementation.
        let mut events = prepared.host.events.clone();
        events.extend([Event::Core(BaseEvent::Fence), Event::Core(BaseEvent::Time(ElapsedTick(3))),
            Event::Core(BaseEvent::Sweep)]);
        let mut oracle = Machine::new(&profile()).unwrap(); let mut outcomes = None;
        for event in &events {
            if let Transition::Swept(result) = oracle.apply(event).unwrap() { outcomes = Some(result); }
        }
        let expected_state = oracle.snapshot(events.len());
        let expected_bytes = journal::encode(&profile(), prepared.host.store.identity(), &events).unwrap();
        let expected_outcomes = outcomes.unwrap(); drop(prepared);
        for anchored in [false, true] {
            for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
                fs::write(root.store().join("delivery.bin"), &original).unwrap();
                let mut run = if anchored {
                    FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap()
                } else { begin(&root, &expected) };
                ready(&mut run, quantum); assert_eq!(root.bytes(), original);
                let calls = Cell::new(0);
                let recovered = run.finish_reconciled(|| { calls.set(calls.get() + 1); ElapsedTick(3) }).unwrap();
                assert_eq!(calls.get(), 1); assert_eq!(recovered.owner.inspect(), expected_state);
                assert_eq!(recovered.outcomes, expected_outcomes); assert_eq!(root.bytes(), expected_bytes);
                assert_eq!(recovered.owner.revision(), before.revision + 3);
                assert_eq!(recovered.owner.inspect().executions, u64::from(mode == 2));
                assert_eq!(recovered.owner.inspect().control.ledger.charged, if mode == 2 { 16 } else { 0 });
                assert_eq!(recovered.owner.inspect().control.ledger.reserved, 0);
                assert!(recovered.owner.clock_ready());
                assert!(recovered.roles.identity_observer.is_none());
                assert!(recovered.roles.policy_governor.is_none());
            }
        }
    }
}

#[test]
fn old_approval_handles_cannot_dispatch_through_the_recovered_owner() {
    let root = Directory::new(); let prepared = Ready::new(&root);
    let expected = requirements(&prepared.host, guards());
    let Ready { host, automatic, human, action, inputs } = prepared; drop(host);
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    let mut recovered = run.finish_reconciled(|| ElapsedTick(3)).unwrap();
    let bytes = root.bytes(); let revision = recovered.owner.revision();
    assert!(recovered.owner.dispatch(revision, &automatic, &human, &action, &inputs, snapshot()).is_err());
    assert_eq!(root.bytes(), bytes); assert_eq!(recovered.owner.inspect().executions, 0);
    // A legitimate recovery still returns separate reviewer custody, not just
    // the owner with a permanently lost role. Original role provisioning owns it.
    let mut fresh = Ready::from_owner(recovered.owner, recovered.roles.human, 2, 102, ElapsedTick(3));
    fresh.dispatch(); fresh.publish_at(2, ElapsedTick(4));
    assert_eq!(fresh.host.inspect().executions, 1);
    assert_eq!(fresh.host.inspect().payload, b"visible");
}

#[test]
fn every_incomplete_or_interrupted_cut_returns_no_roles_and_preserves_staging() {
    let root = Directory::new(); let prepared = Ready::new(&root);
    let expected = requirements(&prepared.host, guards()); let count = prepared.host.events.len(); drop(prepared);
    fs::write(root.store().join("delivery.pending"), b"unacknowledged staging").unwrap();
    let original = root.files();
    for cut in 0..count {
        let mut run = begin(&root, &expected);
        for position in 0..cut { run.advance(position, 1).unwrap(); }
        assert_eq!(run.finish_reconciled(|| panic!("unfinished replay cannot read time")).err(), Some(Error::Incomplete.into()));
        assert_eq!(root.files(), original);
        let mut run = begin(&root, &expected);
        for position in 0..cut { run.advance(position, 1).unwrap(); }
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run.interrupt_after_original_event();
        })).is_err());
        assert_eq!(run.progress().status, Status::Interrupted);
        assert_eq!(run.finish_reconciled(|| panic!("interrupted replay cannot read time")).err(), Some(JournalError::Unavailable));
        assert_eq!(root.files(), original);
    }
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    assert_eq!(run.cancel().status, Status::Cancelled); assert_eq!(root.files(), original);
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    assert!(run.finish_reconciled(|| ElapsedTick(3)).is_ok());
    assert!(!root.store().join("delivery.pending").exists());
}

#[test]
fn independently_declared_guards_policy_and_floors_remain_mandatory() {
    let root = Directory::new(); let prepared = Ready::new(&root);
    let expected = requirements(&prepared.host, guards()); drop(prepared); let original = root.files();
    for field in 0..4 {
        let mut wrong = expected.clone();
        if field == 0 { wrong.minimum.journal_revision = u64::MAX; }
        if field == 1 { wrong.minimum.control_sequence = u64::MAX; }
        if field == 2 { wrong.minimum.authority_epoch = u64::MAX; }
        if field == 3 {
            wrong.effective_policy = crate::action::consequence::gate::containment::session::policy::Policy::new(
                1, vec![crate::action::consequence::gate::containment::session::policy::Predicate::PayloadAtMost(1)]).unwrap();
        }
        let error = if field == 3 { Error::Binding } else { Error::Stale };
        let mut run = begin(&root, &wrong);
        assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS).err(), Some(error.into()));
        assert_eq!(run.finish_reconciled(|| panic!("invalid requirements cannot reach clock")).err(), Some(error.into()));
        assert_eq!(root.files(), original);
    }
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    assert!(run.finish_reconciled(|| ElapsedTick(3)).is_ok());
}

#[test]
fn stale_or_unavailable_clock_changes_neither_canonical_nor_staged_history() {
    let root = Directory::new(); let prepared = Ready::new(&root);
    let expected = requirements(&prepared.host, guards()); drop(prepared);
    fs::write(root.store().join("delivery.pending"), b"keep staging").unwrap(); let original = root.files();
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    assert_eq!(run.finish_reconciled(|| ElapsedTick(0)).err(), Some(Error::Stale.into()));
    assert_eq!(root.files(), original);
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run.finish_reconciled(|| panic!("trusted clock unavailable"))
    })).is_err());
    assert_eq!(root.files(), original);
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    assert!(run.finish_reconciled(|| ElapsedTick(3)).is_ok());
}

#[test]
fn valid_equal_counter_canonical_changes_are_never_overwritten() {
    let root = Directory::new(); let mut prepared = Ready::new(&root);
    prepared.host.observe_time(prepared.host.revision(), ElapsedTick(2)).unwrap();
    let expected = requirements(&prepared.host, guards()); let anchor = prepared.host.history_anchor().unwrap();
    let original = root.bytes(); let mut events = prepared.host.events.clone();
    *events.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    let fork_state = Machine::replay(&profile(), &events).unwrap();
    expected.check(&profile(), &fork_state, &events).unwrap();
    let fork = journal::encode(&profile(), prepared.host.store.identity(), &events).unwrap();
    assert_ne!(fork, original); assert_eq!(fork.len(), original.len()); drop(prepared);
    let pending = root.store().join("delivery.pending"); fs::write(&pending, b"unacknowledged").unwrap();
    let canonical = root.store().join("delivery.bin"); fs::write(&canonical, &fork).unwrap();
    assert_eq!(FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).err(), Some(Error::Binding.into()));
    for during_clock in [false, true] {
        fs::write(&canonical, &original).unwrap();
        let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
        ready(&mut run, 1);
        if !during_clock { fs::write(&canonical, &fork).unwrap(); }
        let calls = Cell::new(0);
        assert_eq!(run.finish_reconciled(|| {
            assert!(during_clock); calls.set(calls.get() + 1);
            fs::write(&canonical, &fork).unwrap(); ElapsedTick(4)
        }).err(), Some(Error::Binding.into()));
        assert_eq!(calls.get(), u32::from(during_clock)); assert_eq!(root.bytes(), fork);
        assert_eq!(fs::read(&pending).unwrap(), b"unacknowledged");
    }
    fs::write(&canonical, &original).unwrap();
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    assert!(run.finish_reconciled(|| ElapsedTick(4)).is_ok());
}

#[test]
fn live_reconciliation_cannot_consume_the_terminal_only_reserve() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 8;
    let (mut host, _) = FileOversight::create_guarded(root.store(), p.clone(), &guards(), None).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let mut tick = 1;
    while host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
        host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); tick += 1;
    }
    let expected = requirements(&host, guards()); let before = host.inspect(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"reserve test staging").unwrap(); let original = root.files();
    let mut run = FileOversight::begin_open_guarded(root.store(), p.clone(), &expected).unwrap();
    ready(&mut run, 1);
    assert_eq!(run.finish_reconciled(|| ElapsedTick(tick)).err(), Some(Error::Limit.into()));
    assert_eq!(root.files(), original);
    // The denied live-recovery transaction spent no preliminary fence. The
    // exact same remaining three events still support the original terminal path.
    let mut run = FileOversight::begin_open_guarded(root.store(), p, &expected).unwrap();
    ready(&mut run, 1); let stop = run.stop_request(900).unwrap();
    let stopped = run.finish_stopped(stop, || ElapsedTick(tick)).unwrap();
    assert!(stopped.progress().drained()); assert_eq!(stopped.snapshot().revision, before.revision + 3);
}

#[test]
fn every_native_storage_barrier_returns_no_owner_and_visible_recovery_never_resends() {
    for executed in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let mut prepared = Ready::new(&root); prepared.dispatch();
            if executed { prepared.publish(); }
            let expected = requirements(&prepared.host, guards()); let before = prepared.host.inspect();
            let anchor = prepared.host.history_anchor().unwrap(); drop(prepared);
            let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
            let mut run = FileGuardedRecovery::begin_store(store, profile(), &expected, Some(&anchor)).unwrap();
            ready(&mut run, 1);
            let error = run.finish_reconciled(|| ElapsedTick(3)).err().unwrap();
            let JournalError::Io(failure) = error else { panic!("original selected storage barrier"); };
            assert_eq!(failure.operation, barrier);
            let visible = barrier == JournalIo::DirectorySync;
            let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
            assert_eq!(disk.revision, before.revision + if visible { 3 } else { 0 });
            let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
            ready(&mut run, 1); let recovered = run.finish_reconciled(|| ElapsedTick(4)).unwrap();
            assert_eq!(recovered.owner.revision(), disk.revision + 3);
            assert_eq!(recovered.owner.inspect().executions, u64::from(executed));
            assert_eq!(recovered.owner.inspect().control.ledger.charged, if executed { 16 } else { 0 });
            assert_eq!(recovered.owner.inspect().control.ledger.reserved, 0);
            assert!(!root.store().join("delivery.pending").exists());
        }
    }
}

#[test]
fn expired_unknowns_remain_explicit_and_charged_in_the_returned_owner_and_outcomes() {
    let root = Directory::new(); let mut prepared = Ready::new(&root); prepared.dispatch();
    let expected = requirements(&prepared.host, guards()); drop(prepared);
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    let recovered = run.finish_reconciled(|| ElapsedTick(1001)).unwrap();
    assert_eq!(recovered.outcomes.get(&1), Some(&Ok(Reconciliation::RetentionExpired)));
    assert_eq!(recovered.owner.inspect().executions, 0);
    assert_eq!(recovered.owner.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), recovered.owner.inspect());
}

#[test]
fn optional_identity_and_governance_roles_survive_but_saved_identity_eligibility_does_not() {
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::activation::identity::{IdentityAnchor, ModelManifest, ModelPassport};
    use crate::action::consequence::delivery::persistent::observed::guarded::{FileIdentityRequirement, FileCampaignRequirement};
    use crate::action::consequence::oversight::identity::{IdentityPolicy, IdentityStatus};
    use crate::action::consequence::policy_campaign::ReplayLimits;
    let root = Directory::new(); let mut g = guards();
    let manifest = ModelManifest { tenant: 1, model: 9, model_generation: 1, host_generation: 1,
        tokenizer_generation: 1, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] };
    let anchor = IdentityAnchor::new(10, CaptureProfile { tenant: 1, model: 9, model_generation: 1,
        tap: 4, layout_generation: 1 }, 5, vec![7], &[[-1.0, 1.0]]).unwrap();
    g.identity = Some(FileIdentityRequirement {
        passport: ModelPassport::new(51, 1, manifest, vec![anchor]).unwrap(),
        policy: IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 8 },
    });
    g.campaigns = Some(FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 }, max_campaigns: 8,
    });
    let (mut host, roles) = FileOversight::create_guarded(root.store(), profile(), &g, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let identity = g.identity.as_ref().unwrap();
    let observer = roles.identity_observer.as_ref().unwrap();
    let check = host.begin_identity_check(host.revision(), 1, 0, 0).unwrap().unwrap();
    let revision = host.revision();
    observer.observe_manifest(&mut host, revision, &check, identity.passport.manifest().clone(),
        ElapsedTick(1)).unwrap().measurement.unwrap();
    let anchor = &identity.passport.anchors()[&10];
    let frame = SourceFrame::capture(FrameIdentity { profile: anchor.profile(), stream: anchor.stream(),
        sequence: 1, position: 0 }, &[0.0]).unwrap();
    let revision = host.revision();
    observer.observe_anchor(&mut host, revision, &check, 10, &frame,
        ElapsedTick(1)).unwrap().measurement.unwrap();
    host.apply_identity_check(host.revision(), &check, 0, 0).unwrap();
    assert_ne!(host.identity_status().unwrap(), IdentityStatus::Missing);
    let expected = requirements(&host, g); drop(host); let original = root.files();
    let mut wrong = expected.clone(); wrong.guards.identity = None;
    let mut run = begin(&root, &wrong);
    assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS).err(), Some(Error::Binding.into()));
    assert_eq!(run.finish_reconciled(|| panic!("missing independent guard")).err(), Some(Error::Binding.into()));
    assert_eq!(root.files(), original);
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    let recovered = run.finish_reconciled(|| ElapsedTick(2)).unwrap();
    assert!(recovered.roles.identity_observer.is_some());
    assert!(recovered.roles.policy_governor.is_some());
    assert_eq!(recovered.owner.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(recovered.owner.clock_ready()); assert!(recovered.outcomes.is_empty());
}
