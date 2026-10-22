//! Real native gates, canonical images, original replay and Store failures.
use super::*;
use super::super::{BaseEvent, FileOversight, FileOversightProfile, Machine, journal, storage};
use super::super::super::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::{JournalIo, RecoveryReserve, MAX_JOURNAL_EVENTS};
use crate::action::consequence::delivery::persistent::observed::credibility::held_out_joint::{
    HeldOutJointBudget, publication::JointPublicationProfile,
};
use crate::action::consequence::delivery::publication_gate::changes::{
    PublicationChangePolicy, freshness::PublicationFreshnessPolicy,
};
use crate::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};
use std::{cell::Cell, fs};
#[path = "fixture.rs"]
mod fixture;
use fixture::*;

fn selected(level: usize) -> FilePublicationRecoveryRequirements {
    FilePublicationRecoveryRequirements {
        validation: (level != 0).then_some(PublicationLimits {
            bindings: 8, validation: RefinementBudget { steps: 10_000, value_bytes: 1_048_576 },
        }),
        feed: matches!(level, 2 | 3 | 5).then_some(JointPublicationFeed {
            changes: PublicationChangePolicy { source: 41, after: 0,
                lookup: RoutingBudget { steps: 10_000, bytes: 1_048_576 } },
            freshness: PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 100 },
            snapshot_fallback: matches!(level, 3 | 5),
        }),
        joint: (level >= 4).then(|| HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0,
            HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap()),
    }
}
fn create(root: &Directory, selected: FilePublicationRecoveryRequirements) -> FileOversight {
    let (mut host, _) = if let Some(joint) = selected.joint {
        FileOversight::create_with_joint_publication(root.store(), profile(), JointPublicationProfile {
            joint, validation: selected.validation, feed: selected.feed,
        }).unwrap()
    } else if let Some(feed) = selected.feed {
        if feed.snapshot_fallback {
            FileOversight::create_with_publication_snapshot_fallback(root.store(), profile(),
                selected.validation.unwrap(), feed.changes, feed.freshness).unwrap()
        } else {
            FileOversight::create_with_publication_change_freshness(root.store(), profile(),
                selected.validation.unwrap(), feed.changes, feed.freshness).unwrap()
        }
    } else if let Some(limits) = selected.validation {
        FileOversight::create_with_publication_validation(root.store(), profile(), limits).unwrap()
    } else {
        let (host, roles) = FileOversight::create_guarded(root.store(), profile(), &guards(), None).unwrap();
        (host, roles.human)
    };
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host
}
fn ready(run: &mut FileGuardedRecovery, quantum: usize) {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
}

#[test]
fn every_exact_profile_matches_original_replay_and_reconciled_bytes() {
    for level in 0..6 {
        for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
            let root = Directory::new(); let selection = selected(level); let host = create(&root, selection);
            let expected = requirements(&host, guards()); let anchor = host.history_anchor().unwrap();
            let mut events = host.events.clone();
            events.extend([Event::Core(BaseEvent::Fence), Event::Core(BaseEvent::Time(ElapsedTick(2))),
                Event::Core(BaseEvent::Sweep)]);
            let oracle = Machine::replay(&host.profile, &events).unwrap().snapshot(events.len());
            let encoded = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
            let original = root.files(); drop(host);
            let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor)
                .unwrap().require_publication_profile(selection).unwrap()
                .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
            assert_eq!(run.progress().attempted_events, 0); assert_eq!(root.files(), original);
            ready(&mut run, quantum); assert_eq!(root.files(), original);
            let calls = Cell::new(0);
            let result = run.finish_reconciled(|| { calls.set(calls.get() + 1); ElapsedTick(2) }).unwrap();
            assert_eq!(calls.get(), 1); assert_eq!(result.owner.inspect(), oracle);
            assert_eq!(root.bytes(), encoded); assert!(result.outcomes.is_empty());
            assert_eq!(result.owner.publication_validation_profile().unwrap(), selection.validation);
            assert_eq!(result.owner.held_out_joint_policy().unwrap(), selection.joint);
            assert_eq!(result.roles.human.reviewer_id(), profile().human.reviewer_id);
            assert_eq!(result.owner.inspect().executions, 0);
        }
    }
}

#[test]
fn absent_gates_and_every_feed_budget_and_joint_pin_refuse_before_replay() {
    let root = Directory::new(); let selection = selected(5); let host = create(&root, selection);
    let expected = requirements(&host, guards()); let anchor = host.history_anchor().unwrap(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"unacknowledged staging").unwrap();
    let original = root.files(); let mut wrongs = (0..5).map(selected).collect::<Vec<_>>();
    for field in 0..10 {
        let mut wrong = selection;
        match field {
            0 => wrong.validation.as_mut().unwrap().bindings -= 1,
            1 => wrong.validation.as_mut().unwrap().validation.steps -= 1,
            2 => wrong.validation.as_mut().unwrap().validation.value_bytes -= 1,
            3 => wrong.feed.as_mut().unwrap().changes.source += 1,
            4 => wrong.feed.as_mut().unwrap().changes.after += 1,
            5 => wrong.feed.as_mut().unwrap().changes.lookup.steps -= 1,
            6 => wrong.feed.as_mut().unwrap().changes.lookup.bytes -= 1,
            7 => wrong.feed.as_mut().unwrap().freshness.clock_domain += 1,
            8 => wrong.feed.as_mut().unwrap().freshness.max_age_ticks += 1,
            _ => wrong.joint = Some(HeldOutJointPolicy::new(72, 1, 1, 2, 0, 0,
                HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap()),
        }
        wrongs.push(wrong);
    }
    let mut wrong = selection; wrong.feed.as_mut().unwrap().snapshot_fallback = false; wrongs.push(wrong);
    for wrong in wrongs {
        let run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
        assert_eq!(run.require_publication_profile(wrong).err(), Some(Error::Binding.into()));
        assert_eq!(root.files(), original);
    }
    let mut invalid = selection; invalid.validation = None;
    assert_eq!(FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap()
        .require_publication_profile(invalid).err(), Some(Error::InvalidInput.into()));
    assert_eq!(root.files(), original);
    let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor)
        .unwrap().require_publication_profile(selection).unwrap();
    ready(&mut run, 1); assert!(run.finish_reconciled(|| ElapsedTick(2)).is_ok());
    assert!(!root.store().join("delivery.pending").exists());
}

#[test]
fn no_late_selection_can_change_a_frozen_profile_or_retain_its_lock() {
    let root = Directory::new(); let selection = selected(3); let host = create(&root, selection);
    let expected = requirements(&host, guards()); drop(host); let original = root.files();
    for complete in [false, true] {
        let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap();
        if complete { ready(&mut run, 1); } else { run.advance(0, 1).unwrap(); }
        assert_eq!(run.require_publication_profile(selection).err(), Some(Error::WrongState.into()));
        assert_eq!(root.files(), original);
    }
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap()
        .require_publication_profile(selection).unwrap();
    ready(&mut run, 1); let request = run.stop_request(81).unwrap();
    assert!(run.finish_stopped(request, || ElapsedTick(2)).unwrap().progress().drained());
}

#[test]
fn matching_publication_never_waives_original_guards_floors_or_suffix_validation() {
    let root = Directory::new(); let selection = selected(2); let host = create(&root, selection);
    let expected = requirements(&host, guards()); let anchor = host.history_anchor().unwrap();
    let canonical = root.bytes(); let identity = host.store.identity().to_path_buf();
    let mut invalid = host.events.clone(); invalid.push(Event::Core(BaseEvent::Reconcile(999)));
    let malformed = journal::encode(&host.profile, &identity, &invalid).unwrap();
    assert!(Machine::replay(&host.profile, &invalid).is_err()); drop(host);
    let mut wrong = expected.clone(); wrong.minimum.journal_revision += 1;
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &wrong).unwrap()
        .require_publication_profile(selection).unwrap();
    assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS).err(), Some(Error::Stale.into()));
    assert_eq!(run.finish_reconciled(|| panic!("failed floor cannot acquire time")).err(), Some(Error::Stale.into()));
    assert_eq!(root.bytes(), canonical);
    fs::write(root.store().join("delivery.bin"), &malformed).unwrap();
    let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor)
        .unwrap().require_publication_profile(selection).unwrap();
    assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err()); drop(run);
    assert_eq!(root.bytes(), malformed);
    fs::write(root.store().join("delivery.bin"), &canonical).unwrap();
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap()
        .require_publication_profile(selection).unwrap();
    ready(&mut run, 1); assert!(run.finish_reconciled(|| ElapsedTick(2)).is_ok());
}

#[test]
fn cancellation_after_every_original_event_never_spends_a_fence() {
    let root = Directory::new(); let selection = selected(5); let host = create(&root, selection);
    let expected = requirements(&host, guards()); let anchor = host.history_anchor().unwrap();
    let count = host.events.len(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"retain on cancellation").unwrap();
    let original = root.files();
    for cut in 0..=count {
        let mut run = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor)
            .unwrap().require_publication_profile(selection).unwrap();
        for done in 0..cut { run.advance(done, 1).unwrap(); }
        assert_eq!(run.cancel().status, FileGuardedRecoveryStatus::Cancelled);
        assert_eq!(root.files(), original);
    }
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap()
        .require_publication_profile(selection).unwrap();
    ready(&mut run, 1); assert!(run.finish().is_ok());
}

#[test]
fn native_storage_failures_return_no_owner_and_retry_preserves_the_selected_profile() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let selection = selected(5); let host = create(&root, selection);
        let expected = requirements(&host, guards()); let anchor = host.history_anchor().unwrap();
        let before = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let mut run = FileGuardedRecovery::begin_store(store, profile(), &expected, Some(&anchor))
            .unwrap().require_publication_profile(selection).unwrap();
        ready(&mut run, 1);
        let error = run.finish_reconciled(|| ElapsedTick(2)).err().unwrap();
        let JournalError::Io(failure) = error else { panic!("original Store failure"); };
        assert_eq!(failure.operation, barrier);
        let current = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(current.revision, before + if barrier == JournalIo::DirectorySync { 3 } else { 0 });
        let mut retry = FileOversight::begin_open_guarded_anchored(root.store(), profile(), &expected, &anchor)
            .unwrap().require_publication_profile(selection).unwrap();
        ready(&mut retry, 1); let result = retry.finish_reconciled(|| ElapsedTick(3)).unwrap();
        assert_eq!(result.owner.revision(), current.revision + 3);
        assert_eq!(result.owner.held_out_joint_policy().unwrap(), selection.joint);
        assert_eq!(result.owner.inspect().executions, 0); assert!(result.outcomes.is_empty());
    }
}

#[test]
fn an_original_subtree_profile_is_not_reinterpreted_as_prefix_routing() {
    let root = Directory::new(); let selection = selected(2); let feed = selection.feed.unwrap();
    let (mut host, _) = FileOversight::create_with_publication_subtree_routing(root.store(), profile(),
        selection.validation.unwrap(), feed.changes).unwrap();
    host.enable_publication_change_freshness(host.revision(), feed.freshness).unwrap();
    let expected = requirements(&host, guards()); let original = root.bytes(); drop(host);
    assert_eq!(FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap()
        .require_publication_profile(selection).err(), Some(Error::Binding.into()));
    assert_eq!(root.bytes(), original);
    let root = Directory::new(); let host = create(&root, selection);
    let expected = requirements(&host, guards()); drop(host);
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected).unwrap()
        .require_publication_profile(selection).unwrap();
    ready(&mut run, 1); assert!(run.finish().is_ok());
}
