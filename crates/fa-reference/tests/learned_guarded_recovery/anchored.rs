//! Original complete-history replay with independently retained prefix bytes.
use super::*;

#[test]
fn an_anchored_read_accepts_successors_without_fencing_or_cleaning_the_live_writer() {
    let root = Directory::new(); let c = config(0); let (mut host, _) = create(&root, &c);
    let anchor = host.history_anchor().unwrap(); let anchor_revision = anchor.revision();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    let expected = requirements(&host, guards()); let actual = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap(); let bytes = root.bytes();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retained staging").unwrap();
    for _ in 0..2 {
        let view = FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &c, &anchor).unwrap();
        assert_eq!(view, actual); assert_eq!(root.bytes(), bytes);
        assert_eq!(std::fs::read(&pending).unwrap(), b"retained staging");
        assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
        assert_eq!(anchor.revision(), anchor_revision);
    }
    // The read did not take or release the writer's exclusive ownership.
    assert_eq!(FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &c, &anchor).err(), Some(JournalError::Busy));
    drop(host);
    let (host, roles) = FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &c, &anchor).unwrap();
    assert_eq!(host.revision(), actual.revision + 1);
    assert!(!pending.exists()); assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical.numerical);
    assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
}

#[test]
fn equal_counter_valid_history_forks_require_the_independent_anchor_not_just_floors() {
    let root = Directory::new(); let c = config(0); let (host, _) = create(&root, &c);
    let bootstrap = root.bytes(); let initial = requirements(&host, guards()); drop(host);
    // Produce BOTH forks through the real original APIs, at the SAME path and
    // with the same recipe and guards. Only the observed elapsed time differs.
    let (mut left, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &initial, &c).unwrap();
    resume(&mut left, 1); step(&mut left).unwrap();
    let anchor = left.history_anchor().unwrap(); let expected = requirements(&left, guards());
    let left_view = left.inspect(); let left_bytes = root.bytes(); drop(left);
    std::fs::write(root.store().join("delivery.bin"), bootstrap).unwrap();
    let (mut right, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &initial, &c).unwrap();
    resume(&mut right, 2); step(&mut right).unwrap();
    let right_view = right.inspect(); let right_bytes = root.bytes();
    let right_anchor = right.history_anchor().unwrap(); drop(right);
    assert_eq!(right_view.revision, left_view.revision);
    assert_eq!(right_view.control.sequence, left_view.control.sequence);
    assert_eq!(right_view.control.ledger.epoch, left_view.control.ledger.epoch);
    assert_ne!(right_view.control.ledger.elapsed, left_view.control.ledger.elapsed);
    assert_ne!(right_bytes, left_bytes);
    // Independent original replay proves the alternate cut is not a malformed
    // fixture. Its revision, sequence and epoch also satisfy the retained floors.
    assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &c).unwrap(), right_view);
    assert_eq!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &c, &right_anchor).unwrap(), right_view);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"not a receipt").unwrap();
    assert_eq!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &c, &anchor).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &c, &anchor).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(root.bytes(), right_bytes); assert!(pending.exists());
    std::fs::write(root.store().join("delivery.bin"), left_bytes).unwrap();
    let (host, _) = FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &c, &anchor).unwrap();
    assert_eq!(host.revision(), left_view.revision + 1); assert!(!pending.exists());
}

#[test]
fn truncated_history_is_rejected_even_when_explicit_counter_floors_are_older() {
    let root = Directory::new(); let c = config(0); let (mut host, _) = create(&root, &c);
    let old = root.bytes(); let older = requirements(&host, guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    let anchor = host.history_anchor().unwrap(); let current = root.bytes(); let actual = host.inspect(); drop(host);
    std::fs::write(root.store().join("delivery.bin"), &old).unwrap();
    assert!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &c).is_ok());
    assert_eq!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &older, &c, &anchor).err(),
        Some(JournalError::Contract(Error::Stale)));
    assert_eq!(FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &older, &c, &anchor).err(),
        Some(JournalError::Contract(Error::Stale)));
    assert_eq!(root.bytes(), old);
    std::fs::write(root.store().join("delivery.bin"), current).unwrap();
    assert_eq!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &older, &c, &anchor).unwrap(), actual);
}

#[test]
fn anchored_history_does_not_replace_exact_recipe_guard_or_floor_requirements() {
    let root = Directory::new(); let c = config(0); let (host, _) = create(&root, &c);
    let anchor = host.history_anchor().unwrap(); let expected = requirements(&host, guards()); let bytes = root.bytes(); drop(host);
    let mut changed = source(0); changed.telemetry.source_check_values -= 1;
    let wrong = FileLearnedConfig::new(numerical::model(), changed, LearnedDecoderBindingLimits::default()).unwrap();
    assert_ne!(wrong, c);
    assert_eq!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &wrong, &anchor).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &wrong, &anchor).err(),
        Some(JournalError::Contract(Error::Binding)));
    for field in 0..4 {
        let mut changed = expected.clone();
        match field {
            0 => changed.guards.campaigns = None,
            1 => changed.minimum.journal_revision += 1,
            2 => changed.minimum.control_sequence += 1,
            3 => changed.minimum.authority_epoch += 1,
            _ => unreachable!(),
        }
        assert!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &changed, &c, &anchor).is_err());
        assert!(FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &changed, &c, &anchor).is_err());
        assert_eq!(root.bytes(), bytes);
    }
    assert!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &c, &anchor).is_ok());
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn corruption_in_an_unanchored_suffix_is_not_ignored_by_read_or_recovery() {
    let root = Directory::new(); let c = config(0); let (mut host, _) = create(&root, &c);
    let anchor = host.history_anchor().unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    let expected = requirements(&host, guards()); let good = root.bytes(); let actual = host.inspect(); drop(host);
    let mut bad = good.clone(); *bad.last_mut().unwrap() ^= 1;
    std::fs::write(root.store().join("delivery.bin"), &bad).unwrap();
    assert!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &c, &anchor).is_err());
    assert!(FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &c, &anchor).is_err());
    assert_eq!(root.bytes(), bad);
    std::fs::write(root.store().join("delivery.bin"), good).unwrap();
    assert_eq!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &c, &anchor).unwrap(), actual);
}

#[test]
fn anchored_pending_intent_cannot_be_skipped_and_advancing_the_anchor_is_explicit() {
    let root = Directory::new(); let c = config(0); let (mut host, _) = create(&root, &c);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let anchor = host.history_anchor().unwrap(); let old_revision = anchor.revision();
    let expected = requirements(&host, guards()); drop(host);
    let (mut host, _) = FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &c, &anchor).unwrap();
    resume(&mut host, 2);
    assert_eq!(host.learned_generation_inspection().unwrap().pending,
        Some(LearnedStepIntent { actor_revision: n.actor_revision, position: n.position }));
    assert!(host.complete_learned_step(host.revision(), n.actor_revision, n.position + 1).is_err());
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    assert!(host.learned_generation_inspection().unwrap().pending.is_none());
    let newer = host.history_anchor_after(&anchor).unwrap();
    assert_eq!(newer.revision(), host.revision()); assert_eq!(anchor.revision(), old_revision);
    assert!(newer.revision() > old_revision);
    assert_eq!(host.history_anchor_after(&newer).unwrap(), newer);
}

#[test]
fn anchored_publication_inspection_does_not_repeat_an_executed_effect() {
    let root = Directory::new(); let c = config(0); let (mut host, roles) = create(&root, &c);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    fresh_identity(&mut host, &roles, 1, 1);
    let (action, input, key, request) = prepared(&mut host, 1, 101);
    let revision = host.revision(); let human = roles.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &key, &human, &action, &input, snapshot()).unwrap();
    host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    let anchor = host.history_anchor().unwrap(); let expected = requirements(&host, guards());
    let actual = host.inspect(); let bytes = root.bytes();
    assert_eq!(actual.executions, 1); assert_eq!(actual.control.ledger.charged, 16);
    for _ in 0..2 {
        assert_eq!(FileOversight::read_guarded_anchored_learned_publication(root.store(), &profile(), &expected, &c, &anchor).unwrap(), actual);
        assert_eq!(root.bytes(), bytes); assert_eq!(host.inspect(), actual);
    }
    drop(host);
    let (mut host, _) = FileOversight::open_guarded_anchored_with_learned_generation(root.store(), profile(), &expected, &c, &anchor).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(host.dispatch(host.revision(), &key, &human, &action, &input, snapshot()).is_err());
}

#[test]
fn foreign_anchor_fails_without_changing_an_otherwise_valid_source() {
    let a = Directory::new(); let b = Directory::new(); let c = config(0);
    let (left, _) = create(&a, &c); let foreign = left.history_anchor().unwrap();
    let (right, _) = create(&b, &c); let local = right.history_anchor().unwrap();
    let expected = requirements(&right, guards()); let bytes = b.bytes(); drop(right);
    assert_eq!(foreign.revision(), local.revision());
    assert!(FileOversight::read_guarded_anchored_learned_publication(b.store(), &profile(), &expected, &c, &foreign).is_err());
    assert!(FileOversight::open_guarded_anchored_with_learned_generation(b.store(), profile(), &expected, &c, &foreign).is_err());
    assert_eq!(b.bytes(), bytes);
    assert!(FileOversight::read_guarded_anchored_learned_publication(b.store(), &profile(), &expected, &c, &local).is_ok());
}
