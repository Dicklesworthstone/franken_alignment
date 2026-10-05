//! Existing public generation calls consume the SAME retained replay path.
use super::*;

fn prefix(host: &FileOversight) -> Option<usize> {
    host.learned_replay.as_ref().map(FileLearnedReplayContinuation::verified_events)
}
fn warm(host: &mut FileOversight) -> Outcome {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap()
}

#[test]
fn standard_generation_consumes_one_event_tails_and_matches_cold_original_records() {
    let root = Directory::new(); let other = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config); let (mut control, _) = owner(&other, &config);
    assert_eq!(prefix(&host), None);
    let mut count = 0;
    while host.learned_generation_inspection().unwrap().numerical.status.is_active() {
        let expected = cold(&mut control).unwrap();
        let actual = if count == 1 {
            // Inspect the very helper used by begin_learned_step, not a fake
            // cost counter. Its candidate starts at the retained live prefix.
            let n = host.learned_generation_inspection().unwrap().numerical;
            let start = prefix(&host).unwrap();
            let mut task = host.cached_learned_intent(host.revision(), n.actor_revision, n.position).unwrap();
            assert_eq!(task.progress().replayed_events, start);
            assert_eq!(task.progress().total_events - start, 1);
            assert_eq!(prefix(&host), None);
            task.advance(&host, start, 1).unwrap();
            host.learned_replay = Some(task.finish_with_continuation(&mut host).unwrap());
            // The public completion consumes this retained predecessor too.
            host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap()
        } else { warm(&mut host).unwrap() };
        assert_eq!(actual.status(), expected.status()); assert_eq!(actual.sample(), expected.sample());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            control.learned_generation_inspection().unwrap().numerical);
        assert_eq!(prefix(&host), Some(host.revision() as usize - 1));
        assert_eq!(bytes(&host), journal::encode(&host.profile, host.store.identity(), &control.events).unwrap());
        count += 1;
    }
    assert!(count > 2); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn clearing_or_losing_the_cache_changes_neither_numerical_state_nor_pending_intent() {
    let root = Directory::new(); let other = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); let (mut control, _) = owner(&other, &config);
    warm(&mut host).unwrap(); cold(&mut control).unwrap();
    let before = host.learned_generation_inspection().unwrap(); let disk = bytes(&host); let old = prefix(&host);
    assert!(old.is_some());
    let n = &before.numerical;
    assert_eq!(host.begin_learned_step(host.revision() - 1, n.actor_revision, n.position).err(), Some(Error::Stale.into()));
    assert_eq!(prefix(&host), old);
    // A wrong operation cannot become admissible because a cache exists.
    assert_eq!(host.begin_learned_step(host.revision(), n.actor_revision + 1, n.position).err(), Some(Error::Stale.into()));
    assert_eq!(host.learned_generation_inspection().unwrap(), before); assert_eq!(bytes(&host), disk);
    assert_eq!(prefix(&host), None);
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let pending = host.learned_generation_inspection().unwrap(); let disk = bytes(&host);
    assert!(prefix(&host).is_some()); host.clear_learned_replay_cache();
    assert_eq!(prefix(&host), None); assert_eq!(host.learned_generation_inspection().unwrap(), pending);
    assert_eq!(bytes(&host), disk);
    let actual = host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    let expected = cold(&mut control).unwrap(); assert_eq!(actual.sample(), expected.sample());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical,
        control.learned_generation_inspection().unwrap().numerical);
    host.clear_learned_replay_cache(); assert!(host.learned_generation_inspection().unwrap().pending.is_none());
}

#[test]
fn cached_and_explicit_paths_interleave_without_overwriting_a_newer_completion() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    warm(&mut host).unwrap(); let oldest = prefix(&host).unwrap();
    // Explicit cold stages do not extract or refresh the private cache.
    cold(&mut host).unwrap(); assert_eq!(prefix(&host), Some(oldest));
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut stale = host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap();
    intent_ready(&mut stale, &host);
    let mut cached = host.cached_learned_intent(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(cached.progress().replayed_events, oldest);
    assert_eq!(cached.progress().total_events - oldest, 3);
    intent_ready(&mut cached, &host);
    host.learned_replay = Some(cached.finish_with_continuation(&mut host).unwrap());
    let after = host.inspect(); let prefix_after = prefix(&host);
    assert_eq!(stale.finish_with_continuation(&mut host).err(), Some(Error::Stale.into()));
    assert_eq!(host.inspect(), after); assert_eq!(prefix(&host), prefix_after);
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    let original = read_machine(&host, &config);
    assert_eq!(original.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
}

#[test]
fn cached_public_write_failures_do_not_fall_back_or_expose_candidate_results() {
    for begin in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
            warm(&mut host).unwrap(); let n = host.learned_generation_inspection().unwrap().numerical;
            if !begin { host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap(); }
            assert!(prefix(&host).is_some()); let before = host.inspect();
            host.store.fail_once(barrier);
            let error = if begin { host.begin_learned_step(host.revision(), n.actor_revision, n.position) }
                else { host.complete_learned_step(host.revision(), n.actor_revision, n.position).map(|_| ()) };
            assert!(matches!(error, Err(JournalError::Io(failure)) if failure.operation == barrier));
            assert_eq!(prefix(&host), None); assert_eq!(host.inspect(), before);
            assert!(host.storage_failure().is_some()); host.clear_learned_replay_cache();
            assert_eq!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).err(),
                Some(JournalError::Unavailable));
            drop(host);
            let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
            assert_eq!(prefix(&recovered), None, "recovery must never hydrate a cache");
            assert!(recovered.learned_generation_inspection().unwrap().paused);
            assert_eq!(recovered.inspect().executions, 0); assert_eq!(recovered.inspect().control.ledger.available, 100);
        }
    }
}

#[test]
fn acknowledged_numerical_failure_is_not_hidden_by_the_cached_quiet_predecessor() {
    let model = model(); let mut source = source(&model, false, 1);
    let mut original = model.observed_learned_generation(source.clone()).unwrap(); original.advance(0).unwrap();
    source.telemetry.source_check_values = original.telemetry_work().source_check_values;
    let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    let root = Directory::new(); let (mut host, _) = owner(&root, &config);
    warm(&mut host).unwrap(); assert!(prefix(&host).is_some());
    let result = warm(&mut host); assert_eq!(result.err(), Some(Error::Limit));
    assert!(host.storage_failure().is_none());
    let state = host.learned_generation_inspection().unwrap();
    assert!(!state.numerical.status.is_active()); assert!(state.pending.is_none());
    assert!(prefix(&host).is_some());
    assert_eq!(host.begin_learned_step(host.revision(), state.numerical.actor_revision, state.numerical.position).err(),
        Some(Error::WrongState.into()));
    assert_eq!(host.learned_generation_inspection().unwrap(), state);
    drop(host);
    let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(recovered.learned_generation_inspection().unwrap().numerical, state.numerical);
    assert_eq!(prefix(&recovered), None);
}

#[test]
fn original_actor_cancellation_is_replayed_before_cached_numerical_progress() {
    use crate::action::consequence::oversight::actor::{ActorProposal, ActorOutcome, Knowledge};
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    warm(&mut host).unwrap(); let start = prefix(&host).unwrap();
    let target = host.inspect().target; let epoch = host.inspect().control.ledger.epoch;
    let (port, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision(); supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let ticket = port.submit(71, &ActorProposal { target, payload: b"visible".to_vec(),
        expected_policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }).unwrap();
    port.cancel(&ticket).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    {
        let mut host = supervisor.host_mut().unwrap();
        assert_eq!(prefix(&host), Some(start));
        let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
        warm(&mut host).unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
        assert_eq!(prefix(&host), Some(host.revision() as usize - 1));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            read_machine(&host, &config).broker.hosted_learned_generation().unwrap());
    }
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
}

mod policy;
