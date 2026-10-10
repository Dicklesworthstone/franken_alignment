//! First-image reserve admission and recovery against the original locked cut.
use super::*;
use fa_reference::action::consequence::delivery::{StopRequest, persistent::{
    RecoveryReserve, observed::guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements},
}};

fn requirements(host: &FileOversight, profile: &FileOversightProfile) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(stream()), decoder: None, decoder_stop: None,
            source: None, identity: None, campaigns: None, credential: None },
        effective_policy: profile.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch },
    }
}

#[test]
fn reserve_is_in_first_image_and_legacy_late_installation_still_refuses() {
    let pinned = recipe();
    let root = Directory::new();
    let (host, _) = FileOversight::create_with_learned_text_stream_with_reserve(
        root.store(), profile(), pinned.clone(), RecoveryReserve::terminal()).unwrap();
    assert_eq!(host.revision(), 3);
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    assert!(host.learned_text_stream_required() && host.publication_guard_required());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 0);
    assert_eq!(numerical.work.admitted_tokens, 0);
    assert_eq!(numerical.sampled_draws, 0);
    assert!(!host.clock_ready());
    let before = root.files();
    let snapshot = FileOversight::read_stream_publication_with_learned_generation(
        root.store(), &profile(), &pinned).unwrap();
    assert!(snapshot.confirmed.visible().is_empty() && snapshot.published.visible().is_empty());
    assert_eq!(root.files(), before);

    let legacy = Directory::new();
    let (mut host, _) = FileOversight::create_with_learned_text_stream(
        legacy.store(), profile(), pinned).unwrap();
    assert_eq!(host.revision(), 2);
    assert_eq!(host.journal_capacity().unwrap().reserve(), None);
    let before = legacy.files();
    assert_eq!(host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).err(),
        Some(Error::WrongState.into()));
    assert_eq!(legacy.files(), before);
    assert!(host.storage_failure().is_none());
}

#[test]
fn malformed_or_unfunded_reserve_never_publishes_a_canonical_image() {
    for reserve in [RecoveryReserve { events: 2, bytes: 50 }, RecoveryReserve { events: 3, bytes: 49 }] {
        let root = Directory::new();
        assert_eq!(FileOversight::create_with_learned_text_stream_with_reserve(
            root.store(), profile(), recipe(), reserve).err(), Some(Error::InvalidInput.into()));
        assert!(!root.store().join("delivery.bin").exists());
    }
    let root = Directory::new();
    let mut small = profile();
    small.delivery.limits.events = 5; // Three bootstrap records plus three reserved slots cannot fit.
    assert_eq!(FileOversight::create_with_learned_text_stream_with_reserve(
        root.store(), small, recipe(), RecoveryReserve::terminal()).err(), Some(Error::Limit.into()));
    assert!(!root.store().join("delivery.bin").exists());
}

#[test]
fn reserve_mismatch_and_late_expectations_preserve_canonical_and_staged_bytes() {
    let pinned = recipe();
    for reserve in [None, Some(RecoveryReserve { events: 4, bytes: 60 })] {
        let root = Directory::new();
        let (host, _) = match reserve {
            None => FileOversight::create_with_learned_text_stream(root.store(), profile(), pinned.clone()),
            Some(value) => FileOversight::create_with_learned_text_stream_with_reserve(
                root.store(), profile(), pinned.clone(), value),
        }.unwrap();
        let expected = requirements(&host, &profile());
        drop(host);
        std::fs::write(root.store().join("delivery.pending"), b"unconfirmed evidence").unwrap();
        let before = root.files();
        let run = FileOversight::begin_open_guarded_with_learned_generation(
            root.store(), profile(), &expected, &pinned).unwrap();
        assert_eq!(run.progress().replayed_events, 0);
        assert_eq!(run.require_recovery_reserve(RecoveryReserve::terminal()).err(), Some(Error::Binding.into()));
        assert_eq!(root.files(), before);
    }

    let root = Directory::new();
    let (host, _) = FileOversight::create_with_learned_text_stream_with_reserve(
        root.store(), profile(), pinned.clone(), RecoveryReserve::terminal()).unwrap();
    let expected = requirements(&host, &profile());
    drop(host);
    std::fs::write(root.store().join("delivery.pending"), b"unconfirmed evidence").unwrap();
    let before = root.files();
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &pinned).unwrap();
    run.advance(0, 1).unwrap();
    assert_eq!(run.require_recovery_reserve(RecoveryReserve::terminal()).err(), Some(Error::WrongState.into()));
    assert_eq!(root.files(), before);

    // A matching reserve never substitutes for the independently retained floor.
    let mut changed = expected.clone();
    changed.minimum.journal_revision += 1;
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &changed, &pinned).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    let total = run.progress().total_events;
    assert!(run.advance(0, total).is_err());
    assert!(run.finish().is_err());
    assert_eq!(root.files(), before);

    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), profile(), &expected, &pinned).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    let total = run.progress().total_events;
    run.advance(0, total).unwrap();
    let (host, _) = run.finish().unwrap();
    assert_eq!(host.revision(), 4);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!root.store().join("delivery.pending").exists());
}

#[test]
fn reserved_tail_fences_stops_and_drains_after_ordinary_learned_work_is_full() {
    let root = Directory::new();
    let pinned = recipe();
    let mut limited = profile();
    limited.delivery.limits.events = 9;
    let (mut host, _) = FileOversight::create_with_learned_text_stream_with_reserve(
        root.store(), limited.clone(), pinned.clone(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host); // The original prompt intent and numerical outcome are both retained.
    assert_eq!(host.revision(), 6);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    assert!(host.journal_capacity().unwrap().terminal_space_remaining());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert!(numerical.work.admitted_tokens > 0);
    let before = root.files();
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(2)).err(), Some(Error::Limit.into()));
    assert_eq!(root.files(), before);
    assert!(host.storage_failure().is_none());
    let expected = requirements(&host, &limited);
    drop(host);
    let mut run = FileOversight::begin_open_guarded_with_learned_generation(
        root.store(), limited, &expected, &pinned).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    let total = run.progress().total_events;
    run.advance(0, total).unwrap();
    let (mut host, _) = run.finish().unwrap();
    assert_eq!(host.revision(), 7);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    let control = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 91,
        expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }).unwrap();
    host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.revision(), 9);
    assert_eq!(host.journal_capacity().unwrap().remaining().events, 0);
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    assert_eq!(host.inspect().executions, 0);
    assert!(host.inspect().stop.is_some());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
}
