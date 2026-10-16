//! Original pending reset, numerical audit and durable publication boundaries.
use super::*;
use super::read_fixture::*;
use super::super::{FileLearnedStepPreparationStatus as Status, JournalIo};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::tensor::kv::{model::MAX_MODEL_KV_VALUES,
    decoder::monitoring::restart::KvRestartBudget};
use crate::action::consequence::delivery::{EndpointOutcome,
    persistent::{Reconciliation, RecoveryReserve, observed::containment::FileResetRequest}};
use crate::action::consequence::gate::ReviewBinding;

fn disk(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn capture(host: &mut FileOversight) -> FileLearnedCheckpoint {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_checkpoint(host.revision(), 7, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}
fn begin(host: &mut FileOversight, saved: &FileLearnedCheckpoint, cache_values: usize) {
    let state = host.inspect();
    let control = FileResetRequest { operation: 900, expected_control_sequence: state.control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: state.control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![state.target] };
    let budget = KvRestartBudget { cache_values,
        audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() };
    host.begin_learned_reset(host.revision(), saved, control, budget).unwrap();
}
fn pending(host: &mut FileOversight, cache_values: usize) {
    step(host).unwrap(); let saved = capture(host); step(host).unwrap();
    begin(host, &saved, cache_values);
}
fn prepare(host: &FileOversight) -> FileLearnedResetPreparation {
    host.prepare_learned_reset_completion(host.revision(), 900).unwrap()
}
fn drain(run: &mut FileLearnedResetPreparation, host: &FileOversight, quantum: usize) {
    while run.progress().status == Status::Replaying {
        let old = run.progress();
        let next = run.advance(host, old.replayed_events, quantum).unwrap();
        assert!(next.replayed_events - old.replayed_events <= quantum);
    }
    assert_eq!(run.progress().status, Status::Ready);
}

#[test]
fn cooperative_reset_matches_cold_and_cached_synchronous_completion_bytes() {
    for quantum in [1, 3, 32] {
        let config = config(false, 3);
        let roots = [Directory::new(), Directory::new(), Directory::new()];
        let (mut cold, _) = owner(&roots[0], &config);
        let (mut warm, _) = owner(&roots[1], &config);
        let (mut host, _) = owner(&roots[2], &config);
        for h in [&mut cold, &mut warm, &mut host] {
            pending(h, MAX_MODEL_KV_VALUES);
            for _ in 0..35 { h.observe_time(h.revision(), ElapsedTick(2)).unwrap(); }
        }
        cold.clear_learned_replay_cache();
        assert!(warm.learned_replay.is_some());
        let expected = cold.complete_learned_reset(cold.revision(), 900).unwrap().unwrap();
        assert!(cold.learned_replay.is_none());
        assert_eq!(warm.complete_learned_reset(warm.revision(), 900).unwrap().unwrap().control, expected.control);
        assert!(warm.learned_replay.is_some());
        let before = disk(&host); let intent = host.pending_learned_reset().unwrap();
        let usage = host.learned_recovery_usage().unwrap();
        let cached = host.learned_replay.as_ref().map(|carry| carry.verified_events());
        let mut run = prepare(&host);
        while run.progress().status == Status::Replaying {
            let done = run.progress().replayed_events;
            run.advance(&host, done, quantum).unwrap();
            assert!(run.progress().replayed_events - done <= quantum);
            assert_eq!(disk(&host), before); assert_eq!(host.pending_learned_reset().unwrap(), intent);
            assert_eq!(host.learned_recovery_usage().unwrap(), usage);
            assert_eq!(host.learned_replay.as_ref().map(|carry| carry.verified_events()), cached);
        }
        let revision = host.revision(); let result = run.finish(&mut host).unwrap().unwrap();
        assert_eq!(result.control, expected.control); assert!(result.control.restored);
        assert_eq!(host.revision(), revision + 1);
        let events = journal::decode(&cold.profile, cold.store.identity(), &disk(&cold)).unwrap();
        for h in [&warm, &host] {
            assert_eq!(disk(h), journal::encode(&h.profile, h.store.identity(), &events).unwrap());
            assert_eq!(h.learned_generation_inspection().unwrap().numerical,
                cold.learned_generation_inspection().unwrap().numerical);
            assert_eq!(h.learned_recovery_usage().unwrap(), cold.learned_recovery_usage().unwrap());
        }
        let before = disk(&host); let usage = host.learned_recovery_usage().unwrap();
        let cache = host.learned_replay.as_ref().unwrap().verified_events();
        assert_eq!(host.complete_learned_reset(0, 900).unwrap().unwrap().control, result.control);
        assert_eq!(host.learned_replay.as_ref().unwrap().verified_events(), cache);
        assert_eq!(disk(&host), before); assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        let expected_next = step(&mut cold).unwrap(); let actual_next = step(&mut host).unwrap();
        assert_eq!(actual_next.sample(), expected_next.sample());
        assert_eq!(actual_next.accepted().unwrap().logits, expected_next.accepted().unwrap().logits);
    }
}

#[test]
fn reset_preparation_catches_up_time_but_cannot_adopt_an_interrupted_intent() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    pending(&mut host, MAX_MODEL_KV_VALUES); let intent = host.pending_learned_reset().unwrap();
    let mut run = prepare(&host); run.advance(&host, 0, 1).unwrap(); let old = run.progress();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); let bytes = disk(&host);
    assert_eq!(run.advance(&host, old.replayed_events, 1).err(), Some(Error::Stale.into()));
    assert_eq!(run.progress(), old);
    run.catch_up(&host, host.revision(), old.replayed_events, 1).unwrap();
    assert_eq!(run.progress().replayed_events, old.replayed_events + 1);
    drain(&mut run, &host, 2); drop(run);
    assert_eq!(disk(&host), bytes); assert_eq!(host.pending_learned_reset().unwrap(), intent);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    let mut run = prepare(&host); drain(&mut run, &host, 1);
    host.fence(host.revision()).unwrap(); let before = run.progress(); let bytes = disk(&host);
    assert!(run.catch_up(&host, host.revision(), before.replayed_events, 1).is_err());
    assert_eq!(run.progress(), before); assert!(run.finish(&mut host).is_err());
    assert_eq!(disk(&host), bytes); assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
    assert!(host.complete_learned_reset(host.revision(), 900).is_err());
}

#[test]
fn reset_preparation_rejects_foreign_stale_invalid_and_incomplete_work_without_writes() {
    let root = Directory::new(); let foreign_root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); let (mut foreign, _) = owner(&foreign_root, &config);
    pending(&mut host, MAX_MODEL_KV_VALUES); pending(&mut foreign, MAX_MODEL_KV_VALUES);
    let mut run = prepare(&host); let initial = run.progress(); let bytes = disk(&host);
    assert_eq!(run.advance(&foreign, 0, 1).err(), Some(Error::Binding.into()));
    assert_eq!(run.advance(&host, 1, 1).err(), Some(Error::Stale.into()));
    assert_eq!(run.advance(&host, 0, 0).err(), Some(Error::InvalidInput.into()));
    assert_eq!(run.advance(&host, 0, usize::MAX).err(), Some(Error::Limit.into()));
    assert_eq!(run.progress(), initial);
    assert_eq!(run.finish(&mut host).err(), Some(Error::Incomplete.into()));
    assert_eq!(disk(&host), bytes);
    let mut run = prepare(&host); drain(&mut run, &host, 1); drop(host);
    let (reopened, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(run.advance(&reopened, run.progress().replayed_events, 1).err(), Some(Error::Binding.into()));
    assert!(reopened.pending_learned_reset().unwrap().unwrap().interrupted);
    assert_eq!(reopened.learned_recovery_usage().unwrap().restart_attempts, 0);
}

#[test]
fn cooperative_reset_storage_barriers_keep_the_original_old_or_completed_result() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        pending(&mut host, MAX_MODEL_KV_VALUES); let mut run = prepare(&host); drain(&mut run, &host, 2);
        host.store.fail_once(barrier);
        let JournalError::Io(failure) = run.finish(&mut host).unwrap_err() else { panic!("missing storage barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.learned_reset_result(900).err(), Some(JournalError::Unavailable)); drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let complete = barrier == JournalIo::DirectorySync;
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, usize::from(complete));
        assert_eq!(host.actor_snapshot().unwrap().incident_count, u64::from(complete));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, if complete { 1 } else { 2 });
        let bytes = disk(&host);
        if complete { assert!(host.complete_learned_reset(0, 900).unwrap().unwrap().control.restored); }
        else {
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
            assert!(host.complete_learned_reset(host.revision(), 900).is_err());
        }
        assert_eq!(disk(&host), bytes); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn prepared_reset_cannot_use_terminal_reserve_after_ordinary_capacity_is_consumed() {
    let root = Directory::new(); let config = config(false, 1); let mut bootstrap = profile();
    bootstrap.delivery.limits.events = 64;
    let (mut host, _) = FileOversight::create(root.store(), bootstrap).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), config).unwrap(); pending(&mut host, MAX_MODEL_KV_VALUES);
    let mut run = prepare(&host); drain(&mut run, &host, 1);
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    }
    let bytes = disk(&host); let before = run.progress();
    assert_eq!(run.catch_up(&host, host.revision(), before.replayed_events, 1).err(), Some(Error::Limit.into()));
    assert_eq!(run.progress(), before);
    assert_eq!(host.prepare_learned_reset_completion(host.revision(), 900).err(), Some(Error::Limit.into()));
    assert_eq!(host.complete_learned_reset(host.revision(), 900).err(), Some(Error::Limit.into()));
    assert_eq!(disk(&host), bytes); assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    assert!(host.revision() < 64); host.fence(host.revision()).unwrap();
    assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
}

#[test]
fn cooperative_reset_preserves_sent_liability_revokes_old_keys_and_replays_original_samples() {
    for published in [false, true] {
        let root = Directory::new(); let config = config(false, 3); let (mut host, reviewer) = owner(&root, &config);
        step(&mut host).unwrap(); let saved = capture(&mut host); let expected_next = step(&mut host).unwrap();
        let (action, input, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
        if published { host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap(); }
        let before = host.learned_generation_inspection().unwrap().numerical;
        begin(&mut host, &saved, MAX_MODEL_KV_VALUES); let mut run = prepare(&host); drain(&mut run, &host, 1);
        let receipt = run.finish(&mut host).unwrap().unwrap();
        assert!(receipt.control.restored); assert_eq!(receipt.control.incident_count, 1);
        assert_eq!(receipt.control.refunded_units, 0); assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.cumulative_work, before.cumulative_work);
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
        let actual = step(&mut host).unwrap(); assert_eq!(actual.sample(), expected_next.sample());
        assert_eq!(actual.accepted().unwrap().logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected_next.accepted().unwrap().logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        let reconciled = host.reconcile(host.revision(), 1).unwrap();
        if published { assert!(matches!(reconciled, Reconciliation::Resolved(EndpointOutcome::Executed { .. }))); }
        else { assert_eq!(reconciled, Reconciliation::AwaitingResolution); }
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, u64::from(published));
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
}

#[test]
fn cooperative_reset_retains_native_refusal_as_an_acknowledged_nonrestore() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    pending(&mut host, 0); let revision = host.revision(); let mut run = prepare(&host); drain(&mut run, &host, 1);
    assert_eq!(run.finish(&mut host).unwrap().err(), Some(Error::Limit));
    assert_eq!(host.revision(), revision + 1); assert!(host.pending_learned_reset().unwrap().is_none());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 2);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 0);
    let bytes = disk(&host); let usage = host.learned_recovery_usage().unwrap();
    assert_eq!(host.complete_learned_reset(0, 900).unwrap().err(), Some(Error::Limit));
    assert_eq!(host.learned_recovery_usage().unwrap(), usage); assert_eq!(disk(&host), bytes);
}

#[test]
fn synchronous_reset_rejects_foreign_cache_without_hiding_failure_by_cold_replay() {
    let root = Directory::new(); let foreign_root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); let (mut foreign, _) = owner(&foreign_root, &config);
    pending(&mut host, MAX_MODEL_KV_VALUES); pending(&mut foreign, MAX_MODEL_KV_VALUES);
    let bytes = disk(&host); let intent = host.pending_learned_reset().unwrap();
    host.learned_replay = foreign.learned_replay.take(); assert!(host.learned_replay.is_some());
    assert_eq!(host.complete_learned_reset(host.revision(), 900).err(), Some(Error::Binding.into()));
    assert_eq!(disk(&host), bytes); assert_eq!(host.pending_learned_reset().unwrap(), intent);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    // A separate, explicitly retried call can reconstruct; the failed call did not.
    assert!(host.complete_learned_reset(host.revision(), 900).unwrap().unwrap().control.restored);
}
