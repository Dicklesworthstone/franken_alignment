//! Original replay, source inference and real journal storage across host quanta.
use super::*;
use super::super::{FileLearnedRecovery, FileLearnedRecoveryStatus};

fn replay_all(recovery: &mut FileLearnedRecovery, quantum: usize) {
    while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = recovery.progress();
        let after = recovery.advance(before.replayed_events, quantum).unwrap();
        assert!(after.replayed_events > before.replayed_events);
        assert!(after.replayed_events - before.replayed_events <= quantum);
        assert_eq!(after.total_events, before.total_events);
    }
    assert_eq!(recovery.progress().status, FileLearnedRecoveryStatus::Ready);
}

#[test]
fn every_numerical_cut_recovers_in_single_events_then_continues_original_sampling() {
    let model = model();
    let source = source(&model, false, 3);
    let config = FileLearnedConfig::new(model.clone(), source.clone(), LearnedDecoderBindingLimits::default()).unwrap();
    for cut in 0..=4 {
        let mut original = model.monitored_generation_with_telemetry(source.stream,
            source.evaluation_origin, source.spec.clone(), source.policy.clone(),
            source.budget, source.telemetry).unwrap();
        let root = Directory::new();
        let (mut host, _) = owner(&root, &config);
        for _ in 0..cut {
            original.advance(original.position()).unwrap();
            step(&mut host).unwrap();
        }
        let before = host.learned_generation_inspection().unwrap();
        let epoch = host.inspect().control.ledger.epoch;
        let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        drop(host);
        let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let start = recovery.progress();
        assert_eq!(start.replayed_events, 0);
        assert_eq!(start.total_events as u64, before.journal_revision);
        assert_eq!(recovery.advance(1, 1).err(), Some(JournalError::Contract(Error::Stale)));
        assert_eq!(recovery.advance(0, 0).err(), Some(JournalError::Contract(Error::InvalidInput)));
        assert_eq!(recovery.advance(0, usize::MAX).err(), Some(JournalError::Contract(Error::Limit)));
        assert_eq!(recovery.progress(), start);
        replay_all(&mut recovery, 1);
        let ready = recovery.progress();
        assert_eq!(recovery.advance(ready.replayed_events, 1).unwrap(), ready);
        assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
        let (mut host, _) = recovery.finish().unwrap();
        let recovered = host.learned_generation_inspection().unwrap();
        assert_eq!(recovered.numerical, before.numerical);
        assert_eq!(recovered.journal_revision, before.journal_revision + 1);
        assert_eq!(host.inspect().control.ledger.epoch, epoch + 1);
        assert!(recovered.paused && !host.clock_ready());
        assert_eq!(host.inspect().executions, 0);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_learned_generation(host.revision(), recovered.numerical.actor_revision,
            recovered.numerical.position).unwrap();
        while original.status().is_active() {
            let expected = original.advance(original.position()).unwrap();
            let actual = step(&mut host).unwrap();
            assert_eq!(actual.status(), expected.status());
            assert_eq!(actual.sample(), expected.sample());
        }
        let final_state = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(final_state.work, original.work());
        assert_eq!(final_state.telemetry, original.telemetry_work());
        assert_eq!(host.machine.broker.retained_actor_state().tokens(), original.accepted_tokens());
        assert_eq!(host.machine.broker.retained_actor_state().sampler(), original.sampler_state().encode());
    }
}

#[test]
fn dropping_or_early_finishing_recovery_releases_only_the_lock_and_preserves_staging() {
    let root = Directory::new();
    let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    step(&mut host).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging evidence").unwrap();
    let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).err(), Some(JournalError::Busy));
    recovery.advance(0, 4).unwrap();
    drop(recovery);
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
    assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging evidence");
    let recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(recovery.finish().err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging evidence");
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(!pending.exists());
}

#[test]
fn a_late_original_witness_failure_latches_the_first_error_without_cleaning_evidence() {
    let root = Directory::new();
    let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    step(&mut host).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = host.events.clone();
    let failed_at = events.iter().rposition(|event| matches!(event,
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { .. })))).unwrap();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = &mut events[failed_at]
        else { unreachable!("selected original step"); };
    let mut changed = witness.to_vec();
    changed[0] ^= 1;
    *witness = changed.into();
    let corrupt = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    host.store.replace(&corrupt).unwrap();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"preserve failed replay evidence").unwrap();
    let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(recovery.advance(0, events.len()).err(), Some(JournalError::Contract(Error::Binding)));
    let failed = recovery.progress();
    assert_eq!(failed.replayed_events, failed_at);
    assert_eq!(failed.status, FileLearnedRecoveryStatus::Failed(Error::Binding));
    assert_eq!(recovery.advance(failed_at, 1).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(recovery.advance(0, 0).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(recovery.progress(), failed);
    assert_eq!(recovery.finish().err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), corrupt);
    assert_eq!(std::fs::read(&pending).unwrap(), b"preserve failed replay evidence");
    // Same original history with its real witness succeeds and keeps its work.
    std::fs::write(root.store().join("delivery.bin"), canonical).unwrap();
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
}

#[test]
fn changed_canonical_image_refuses_before_cleanup_or_a_recovery_fence() {
    let root = Directory::new();
    let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    let before = host.inspect();
    let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    drop(host);
    let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    replay_all(&mut recovery, 2);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"preserve changed cut").unwrap();
    let mut changed = canonical.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    std::fs::write(root.store().join("delivery.bin"), &changed).unwrap();
    assert_eq!(recovery.finish().err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), changed);
    assert_eq!(std::fs::read(&pending).unwrap(), b"preserve changed cut");
    std::fs::write(root.store().join("delivery.bin"), canonical).unwrap();
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.revision(), before.revision + 1);
    assert_eq!(host.inspect().control.ledger.epoch, before.control.ledger.epoch + 1);
}

#[test]
fn recovered_pending_step_and_unknown_dispatch_keep_work_and_do_not_reissue_old_keys() {
    let root = Directory::new();
    let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap();
    let (action, inputs, automatic, request) = prepared(&mut host);
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    drop(host);
    let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    replay_all(&mut recovery, 2);
    let (mut host, _) = recovery.finish().unwrap();
    let recovered = host.learned_generation_inspection().unwrap();
    assert!(recovered.paused);
    assert_eq!(recovered.numerical, n);
    assert_eq!(recovered.pending, Some(LearnedStepIntent { actor_revision: n.actor_revision, position: n.position }));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(host.propose(host.revision(), 2, action_spec(&host), snapshot()).err(), Some(JournalError::Contract(Error::Incomplete)));
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, n.position + 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 0);
    assert!(matches!(host.seal_unexecuted(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { .. })));
    assert_eq!(host.inspect().control.ledger.charged, 0);
}

#[test]
fn every_fence_storage_failure_returns_no_owner_and_reopens_the_actual_visible_cut() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new();
        let config = config(false, 3);
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap();
        step(&mut host).unwrap();
        let before = host.learned_generation_inspection().unwrap();
        let epoch = host.inspect().control.ledger.epoch;
        drop(host);
        let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
        replay_all(&mut recovery, 1);
        recovery.fail_once(barrier);
        let JournalError::Io(failure) = recovery.finish().unwrap_err()
            else { panic!("expected original storage failure"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let fences = if barrier == JournalIo::DirectorySync { 2 } else { 1 };
        assert_eq!(host.revision(), before.journal_revision + fences);
        assert_eq!(host.inspect().control.ledger.epoch, epoch + fences);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before.numerical);
        assert!(host.learned_generation_inspection().unwrap().paused && !host.clock_ready());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn fence_capacity_refusal_preserves_staging_and_the_one_more_event_twin_recovers() {
    for capacity in [3, 4] {
        let root = Directory::new();
        let config = config(false, 1);
        let mut profile = profile();
        profile.delivery.limits.events = capacity;
        let (mut host, _) = FileOversight::create(root.store(), profile.clone()).unwrap();
        host.enable_learned_generation(host.revision(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let canonical = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        drop(host);
        let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"capacity evidence").unwrap();
        let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile, &config).unwrap();
        replay_all(&mut recovery, 1);
        if capacity == 3 {
            assert_eq!(recovery.finish().err(), Some(JournalError::Contract(Error::Limit)));
            assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
            assert_eq!(std::fs::read(&pending).unwrap(), b"capacity evidence");
        } else {
            let (host, _) = recovery.finish().unwrap();
            assert_eq!(host.revision(), 4);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 0);
            assert!(!pending.exists());
        }
    }
}
