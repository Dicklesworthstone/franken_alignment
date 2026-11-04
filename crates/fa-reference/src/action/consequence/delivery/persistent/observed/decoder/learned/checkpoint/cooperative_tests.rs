//! Original numerical checkpoints, original synchronous oracle and real Store.
use super::*;
use super::read_fixture::*;
use super::super::{FileLearnedStepPreparationStatus as Status, JournalIo};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::RecoveryReserve;

fn prepare(host: &FileOversight, id: u64) -> FileLearnedCheckpointPreparation {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.prepare_learned_checkpoint(host.revision(), id, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}
fn drain(run: &mut FileLearnedCheckpointPreparation, host: &FileOversight, quantum: usize) {
    while run.progress().status == Status::Replaying {
        let before = run.progress();
        let after = run.advance(host, before.replayed_events, quantum).unwrap();
        assert!(after.replayed_events - before.replayed_events <= quantum);
        assert_eq!(after.journal_revision, host.revision());
    }
    assert_eq!(run.progress().status, Status::Ready);
}
fn disk(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }

#[test]
fn cooperative_capture_matches_original_synchronous_witness_and_recovered_pair() {
    for quantum in [1, 3, 32] {
        let root = Directory::new(); let sync_root = Directory::new(); let config = config(false, 3);
        let (mut host, _) = owner(&root, &config); let (mut sync, _) = owner(&sync_root, &config);
        step(&mut host).unwrap(); step(&mut sync).unwrap();
        for _ in 0..35 {
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            sync.observe_time(sync.revision(), ElapsedTick(2)).unwrap();
        }
        let before = host.learned_generation_inspection().unwrap();
        let control = host.inspect().control;
        let expected = sync.capture_learned_checkpoint(sync.revision(), 7,
            before.numerical.actor_revision, control.ledger.epoch).unwrap();
        let events = journal::decode(&sync.profile, sync.store.identity(), &disk(&sync)).unwrap();
        let expected_bytes = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
        let original = disk(&host);
        let cached = host.learned_replay.as_ref().map(|carry| carry.verified_events());
        let mut run = prepare(&host, 7);
        assert_eq!(run.progress().replayed_events, 0);
        while run.progress().status == Status::Replaying {
            let done = run.progress().replayed_events;
            run.advance(&host, done, quantum).unwrap();
            assert!(run.progress().replayed_events - done <= quantum);
            assert_eq!(disk(&host), original);
            assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 0);
            assert_eq!(host.learned_replay.as_ref().map(|carry| carry.verified_events()), cached);
        }
        let saved = run.finish(&mut host).unwrap();
        assert_eq!(saved.info(), expected.info());
        assert_eq!(disk(&host), expected_bytes);
        assert_eq!(host.revision(), before.journal_revision + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before.numerical);
        assert_eq!(host.inspect().control, sync.inspect().control);
        assert_eq!(host.inspect().control.ledger, control.ledger);
        let usage = host.learned_recovery_usage().unwrap();
        assert_eq!(usage.checkpoints, 1); assert!(usage.checkpoint_bytes > 0);
        step(&mut host).unwrap();
        let advanced = host.learned_generation_inspection().unwrap().numerical;
        let again = host.capture_learned_checkpoint(0, 7, saved.info().actor_revision,
            saved.info().authority_epoch).unwrap();
        assert_eq!(again.info(), saved.info());
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_checkpoint(7).unwrap().info(), saved.info());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, advanced);
        assert!(host.check_learned_checkpoint(&saved).is_err());
    }
}

#[test]
fn checkpoint_cursor_preserves_work_across_explicit_time_catch_up_and_refuses_new_inference() {
    let root = Directory::new(); let foreign_root = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config); let (mut foreign, _) = owner(&foreign_root, &config);
    step(&mut host).unwrap(); step(&mut foreign).unwrap();
    let original = disk(&host); let mut run = prepare(&host, 1); let initial = run.progress();
    assert_eq!(run.advance(&foreign, 0, 1).err(), Some(Error::Binding.into()));
    assert_eq!(run.advance(&host, 1, 1).err(), Some(Error::Stale.into()));
    assert_eq!(run.advance(&host, 0, 0).err(), Some(Error::InvalidInput.into()));
    assert_eq!(run.progress(), initial); assert_eq!(disk(&host), original);
    run.advance(&host, 0, 1).unwrap(); let before = run.progress();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(run.advance(&host, before.replayed_events, 1).err(), Some(Error::Stale.into()));
    assert_eq!(run.progress(), before);
    run.catch_up(&host, host.revision(), before.replayed_events, 1).unwrap();
    assert_eq!(run.progress().replayed_events, before.replayed_events + 1);
    drain(&mut run, &host, 2);
    let n = host.learned_generation_inspection().unwrap().numerical;
    run.finish(&mut host).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    let mut stale = prepare(&host, 2); let before = stale.progress();
    step(&mut host).unwrap();
    assert_eq!(stale.catch_up(&host, host.revision(), 0, 32).err(), Some(Error::Stale.into()));
    assert_eq!(stale.progress(), before);
    let bytes = disk(&host); drop(stale); assert_eq!(disk(&host), bytes);
    let mut fresh = prepare(&host, 2); drain(&mut fresh, &host, 3);
    fresh.finish(&mut host).unwrap();
    assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 2);
}

#[test]
fn incomplete_pending_fenced_and_abandoned_checkpoint_tasks_cannot_capture() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let bytes = disk(&host);
    assert_eq!(prepare(&host, 1).finish(&mut host).err(), Some(Error::Incomplete.into()));
    assert_eq!(disk(&host), bytes);
    let mut run = prepare(&host, 1);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let before = run.progress(); let bytes = disk(&host);
    assert!(run.catch_up(&host, host.revision(), 0, 32).is_err());
    assert_eq!(run.progress(), before); assert_eq!(disk(&host), bytes);
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    drop(run);
    let mut run = prepare(&host, 1); drain(&mut run, &host, 1);
    host.fence(host.revision()).unwrap(); let bytes = disk(&host);
    assert!(run.catch_up(&host, host.revision(), run.progress().replayed_events, 1).is_err());
    drop(run); assert_eq!(disk(&host), bytes);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    let mut run = prepare(&host, 1); drain(&mut run, &host, 1);
    drop(host); // A preparation must not keep the original writer lock alive.
    let (reopened, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(run.advance(&reopened, run.progress().replayed_events, 1).err(), Some(Error::Binding.into()));
    assert_eq!(reopened.learned_recovery_usage().unwrap().checkpoints, 0);
}

#[test]
fn checkpoint_capture_never_spends_the_terminal_only_recovery_reserve() {
    let root = Directory::new(); let config = config(false, 1); let mut bootstrap = profile();
    bootstrap.delivery.limits.events = 64;
    let (mut host, _) = FileOversight::create(root.store(), bootstrap).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_learned_generation(host.revision(), config).unwrap(); step(&mut host).unwrap();
    let mut run = prepare(&host, 1); drain(&mut run, &host, 1);
    while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    }
    let bytes = disk(&host); let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(host.prepare_learned_checkpoint(host.revision(), 2, n.actor_revision,
        host.inspect().control.ledger.epoch).err(), Some(Error::Limit.into()));
    assert_eq!(run.catch_up(&host, host.revision(), run.progress().replayed_events, 1).err(), Some(Error::Limit.into()));
    assert_eq!(disk(&host), bytes); assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 0);
    assert!(host.revision() < 64); host.fence(host.revision()).unwrap();
}

#[test]
fn cooperative_checkpoint_storage_failures_return_no_handle_or_repeat_capture() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let numerical = host.learned_generation_inspection().unwrap().numerical;
        let mut run = prepare(&host, 1); drain(&mut run, &host, 2);
        host.store.fail_once(barrier);
        let error = run.finish(&mut host).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("missing original storage failure"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.learned_checkpoint(1).err(), Some(JournalError::Unavailable));
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, usize::from(barrier == JournalIo::DirectorySync));
        assert_eq!(host.learned_checkpoint(1).is_ok(), barrier == JournalIo::DirectorySync);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn cooperatively_captured_pair_restores_original_numerics_without_rewinding_effect_liability() {
    use crate::action::consequence::activation::tensor::kv::{model::MAX_MODEL_KV_VALUES,
        decoder::monitoring::restart::KvRestartBudget};
    use crate::action::consequence::delivery::{EndpointOutcome,
        persistent::{Reconciliation, observed::containment::FileResetRequest}};
    use crate::action::consequence::gate::ReviewBinding;
    for published in [false, true] {
        let root = Directory::new(); let config = config(false, 3);
        let (mut host, reviewer) = owner(&root, &config);
        step(&mut host).unwrap();
        let mut run = prepare(&host, 7); drain(&mut run, &host, 1);
        let saved = run.finish(&mut host).unwrap();
        let captured = host.learned_generation_inspection().unwrap().numerical;
        let expected_next = step(&mut host).unwrap();
        let (action, input, automatic, request) = prepared(&mut host);
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
        if published {
            host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
        }
        let before = host.learned_generation_inspection().unwrap().numerical;
        let state = host.inspect();
        let intent = FileResetRequest { operation: 900, expected_control_sequence: state.control.sequence,
            expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
            expected_authority_epoch: state.control.ledger.epoch,
            binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
            retained_targets: vec![state.target] };
        let budget = KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
            audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() };
        let receipt = host.reset_learned_checkpoint(host.revision(), &saved, intent.clone(), budget)
            .unwrap().unwrap();
        assert!(receipt.control.restored); assert_eq!(receipt.control.incident_count, 1);
        assert_eq!(receipt.control.refunded_units, 0);
        let restored = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(restored.position, captured.position); assert_eq!(restored.work, captured.work);
        assert_eq!(restored.cumulative_work, before.cumulative_work);
        assert!(host.inspect().control.ledger.epoch > state.control.ledger.epoch);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, u64::from(published));
        let bytes = disk(&host);
        let repeated = host.reset_learned_checkpoint(0, &saved, intent, budget).unwrap().unwrap();
        assert_eq!(repeated.control, receipt.control); assert_eq!(disk(&host), bytes);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
        let actual_next = step(&mut host).unwrap();
        assert_eq!(actual_next.sample(), expected_next.sample());
        assert_eq!(actual_next.accepted().unwrap().logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected_next.accepted().unwrap().logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        let outcome = host.reconcile(host.revision(), 1).unwrap();
        if published {
            assert!(matches!(outcome, Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
        } else { assert_eq!(outcome, Reconciliation::AwaitingResolution); }
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, u64::from(published));
    }
}
