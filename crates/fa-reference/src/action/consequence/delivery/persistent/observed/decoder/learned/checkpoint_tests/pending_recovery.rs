//! Original reset and real canonical replacement, with unchanged quarantine controls.
use super::*;
use super::super::checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent};
use crate::action::ActionState;
use crate::action::consequence::activation::tensor::kv::{model::MAX_MODEL_KV_VALUES,
    decoder::monitoring::restart::KvRestartBudget};
use crate::action::consequence::delivery::{EndpointOutcome, StopRequest};
use crate::action::consequence::delivery::persistent::{Reconciliation,
    observed::containment::FileResetRequest};
use crate::action::consequence::gate::ReviewBinding;
use crate::action::consequence::oversight::learned_source::LearnedAvailability;

fn instruction(host: &FileOversight, checkpoint: u64, operation: u64) -> FileLearnedResetIntent {
    let view = host.inspect();
    FileLearnedResetIntent::for_recovery(checkpoint, FileResetRequest {
        operation, expected_control_sequence: view.control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: view.control.ledger.epoch,
        binding: ReviewBinding { round: operation, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![view.target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() }).unwrap()
}
fn begin(host: &mut FileOversight, checkpoint: &FileLearnedCheckpoint, intent: &FileLearnedResetIntent) {
    host.begin_learned_reset(host.revision(), checkpoint, intent.control().clone(), intent.budget()).unwrap();
    assert_eq!(&host.pending_learned_reset().unwrap().unwrap().intent, intent);
}
fn replay(root: &Directory, config: &FileLearnedConfig) -> FileLearnedRecovery {
    replay_with_profile(root, profile(), config)
}
fn replay_with_profile(root: &Directory, profile: FileOversightProfile, config: &FileLearnedConfig)
    -> FileLearnedRecovery
{
    let mut run = FileOversight::begin_open_with_learned_generation(root.store(), profile, config).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 3).unwrap();
    }
    run
}
fn canonical(root: &Directory) -> Vec<u8> { std::fs::read(root.store().join("delivery.bin")).unwrap() }
fn resume(host: &mut FileOversight, now: u64) {
    host.observe_time(host.revision(), ElapsedTick(now)).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
}

#[test]
fn pending_reset_recovers_as_the_original_completion_and_one_fence_then_continues_exactly() {
    let root = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let next = step(&mut host).unwrap();
    let spent = host.learned_generation_inspection().unwrap().numerical;
    let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
    let cut = host.revision(); let before = canonical(&root);
    // Control: original live completion followed by its original recovery fence.
    let mut control = read_machine(&host, &config);
    control.prepare_learned_reset(900).unwrap();
    control.apply(&Event::Core(super::super::super::super::BaseEvent::Fence)).unwrap();
    let expected = control.snapshot(cut as usize + 2);
    let expected_numerical = control.broker.hosted_learned_generation().unwrap();
    let expected_usage = control.broker.hosted_learned_recovery_usage().unwrap();
    drop(host);
    let run = replay(&root, &config);
    assert_eq!(canonical(&root), before);
    let (mut host, _) = run.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.inspect(), expected);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, expected_numerical);
    assert_eq!(host.learned_recovery_usage().unwrap(), expected_usage);
    assert_eq!(host.revision(), cut + 2); assert!(!host.clock_ready());
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_reset_result(900).unwrap().unwrap().resumed_stream, Some(22));
    assert_eq!(expected_numerical.cumulative_work, spent.cumulative_work);
    assert_eq!(expected_numerical.availability, LearnedAvailability::Empty);
    assert!(host.check_learned_checkpoint(&saved).is_err());
    assert!(step(&mut host).is_err());
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    resume(&mut host, 2);
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    let actual = step(&mut host).unwrap();
    assert_eq!(actual.sample(), next.sample());
    assert_eq!(actual.accepted().unwrap().logits, next.accepted().unwrap().logits);
    assert!(actual.audit().source().descriptor().layers().values().all(|layer| layer.stream == 22));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn recovered_reset_still_requires_fresh_source_congress_and_both_publication_keys() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, old_reviewer) = owner(&root, &config);
    step(&mut host).unwrap(); let saved = capture(&mut host, 1); step(&mut host).unwrap();
    let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent); drop(host);
    let (mut host, reviewer) = replay(&root, &config).finish_pending_reset(&intent).unwrap();
    resume(&mut host, 2); step(&mut host).unwrap();
    let (action, inputs, automatic, request) = prepared(&mut host);
    assert!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).is_err());
    let revision = host.revision();
    assert!(old_reviewer.approve(&mut host, revision, &request).is_err());
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    assert_eq!(host.inspect().executions, 0);
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(3)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().payload, b"visible"); assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).is_err());
}

#[test]
fn independently_selected_reset_must_match_every_field_and_cannot_invent_an_intent() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let expected = instruction(&host, 1, 900);
    assert!(FileLearnedResetIntent::for_recovery(0, expected.control().clone(), expected.budget()).is_err());
    let no_intent = canonical(&root); drop(host);
    assert_eq!(replay(&root, &config).finish_pending_reset(&expected).err(), Some(Error::Missing.into()));
    assert_eq!(canonical(&root), no_intent);
    // Reopen normally, then issue a NEW instruction at the new original epoch.
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    resume(&mut host, 2); let saved = host.learned_checkpoint(saved.id()).unwrap();
    let expected = instruction(&host, 1, 901); begin(&mut host, &saved, &expected);
    let bytes = canonical(&root); drop(host);
    let mut wrong = Vec::new();
    wrong.push(FileLearnedResetIntent::for_recovery(2, expected.control().clone(), expected.budget()).unwrap());
    for field in 0..5 {
        let mut control = expected.control().clone();
        match field {
            0 => control.operation += 1,
            1 => control.expected_authority_epoch += 1,
            2 => control.expected_control_sequence += 1,
            3 => control.expected_actor_revision += 1,
            _ => control.binding.evidence_root[0] ^= 1,
        }
        wrong.push(FileLearnedResetIntent::for_recovery(1, control, expected.budget()).unwrap());
    }
    let mut smaller = expected.budget(); smaller.audit.monitoring.refinements -= 1;
    wrong.push(FileLearnedResetIntent::for_recovery(1, expected.control().clone(), smaller).unwrap());
    for changed in wrong {
        assert_eq!(replay(&root, &config).finish_pending_reset(&changed).err(), Some(Error::Binding.into()));
        assert_eq!(canonical(&root), bytes);
    }
    let (host, _) = replay(&root, &config).finish_pending_reset(&expected).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
}

#[test]
fn original_fenced_stopped_and_default_recovery_barriers_cannot_be_rebased() {
    for mode in 0..3 {
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
        if mode == 0 { host.fence(host.revision()).unwrap(); }
        if mode == 1 {
            let c = host.inspect().control;
            host.request_stop(host.revision(), StopRequest { operation: 700,
                expected_control_sequence: c.sequence, expected_authority_epoch: c.ledger.epoch }).unwrap();
        }
        drop(host);
        if mode == 2 {
            let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted); drop(host);
        }
        let before = canonical(&root);
        assert_eq!(replay(&root, &config).finish_pending_reset(&intent).err(), Some(Error::WrongState.into()));
        assert_eq!(canonical(&root), before);
    }
}

#[test]
fn native_audit_failure_is_acknowledged_without_pretending_the_reset_succeeded() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let intent = instruction(&host, 1, 900); let mut budget = intent.budget();
    budget.audit.monitoring.probe_coordinates = 0;
    let intent = FileLearnedResetIntent::for_recovery(1, intent.control().clone(), budget).unwrap();
    begin(&mut host, &saved, &intent); drop(host);
    let (mut host, _) = replay(&root, &config).finish_pending_reset(&intent).unwrap();
    let error = host.learned_reset_result(900).unwrap().unwrap_err();
    let n = host.learned_generation_inspection().unwrap().numerical;
    let usage = host.learned_recovery_usage().unwrap();
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert_eq!(usage.restart_attempts, 1); assert!(n.host_failure.is_some());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.resume_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
    assert!(step(&mut host).is_err()); drop(host);
    let (host, _) = replay(&root, &config).finish_pending_reset(&intent).unwrap();
    assert_eq!(host.learned_reset_result(900).unwrap().unwrap_err(), error);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn every_storage_barrier_keeps_completion_and_fence_atomic_and_retries_do_not_repeat_reset() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        step(&mut host).unwrap(); let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
        let cut = host.revision(); drop(host);
        let run = replay(&root, &config); run.fail_once(barrier);
        let error = run.finish_pending_reset(&intent).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected original replacement barrier"); };
        assert_eq!(failure.operation, barrier);
        let identity = storage::identity(&root.store()).unwrap();
        let mut events = journal::decode(&profile(), &identity, &canonical(&root)).unwrap();
        bind_history(&mut events, &config).unwrap();
        let machine = Machine::replay(&profile(), &events).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(events.len() as u64, cut + if visible { 2 } else { 0 });
        assert_eq!(machine.learned_reset_result_optional(900).unwrap().is_some(), visible);
        if visible {
            assert!(!machine.clock_ready);
            assert!(matches!(events.last(), Some(Event::Core(super::super::super::super::BaseEvent::Fence))));
        } else { assert!(!machine.pending_learned_reset().unwrap().interrupted); }
        let (host, _) = replay(&root, &config).finish_pending_reset(&intent).unwrap();
        assert_eq!(host.revision(), cut + if visible { 3 } else { 2 });
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert!(host.pending_learned_reset().unwrap().is_none()); assert!(!host.clock_ready());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn pending_reset_recovery_preserves_sent_and_executed_effect_liabilities_and_rejects_old_keys() {
    for executed in [false, true] {
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, reviewer) = owner(&root, &config); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        let (action, input, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
        if executed { host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap(); }
        let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent); drop(host);
        let (mut host, _) = replay(&root, &config).finish_pending_reset(&intent).unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.learned_reset_result(900).unwrap().unwrap().control.refunded_units, 0);
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        let outcome = host.reconcile(host.revision(), 1).unwrap();
        if executed {
            assert!(matches!(outcome, Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
            assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().payload, b"visible");
        } else {
            assert_eq!(outcome, Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            host.seal_unexecuted(host.revision(), 1).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, 0);
        }
        assert_eq!(host.inspect().executions, u64::from(executed));
    }
}

#[test]
fn both_events_must_fit_and_an_incomplete_replay_cannot_complete_a_reset() {
    for limit in [9, 10] {
        let root = Directory::new(); let config = config(false, 1);
        let mut profile = profile(); profile.delivery.limits.events = limit;
        let (mut host, _) = FileOversight::create(root.store(), profile.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.enable_learned_generation(host.revision(), config.clone()).unwrap();
        step(&mut host).unwrap(); let saved = capture(&mut host, 1); step(&mut host).unwrap();
        let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
        assert_eq!(host.revision(), 8); let before = canonical(&root); drop(host);
        let early = FileOversight::begin_open_with_learned_generation(root.store(), profile.clone(), &config).unwrap();
        assert_eq!(early.finish_pending_reset(&intent).err(), Some(Error::Incomplete.into()));
        assert_eq!(canonical(&root), before);
        let result = replay_with_profile(&root, profile, &config).finish_pending_reset(&intent);
        if limit == 9 {
            assert_eq!(result.err(), Some(Error::Limit.into())); assert_eq!(canonical(&root), before);
        } else { assert_eq!(result.unwrap().0.revision(), 10); }
    }
}

#[test]
fn a_changed_canonical_cut_or_corrupt_suffix_is_not_overwritten_by_reset_recovery() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let intent = instruction(&host, 1, 900); begin(&mut host, &saved, &intent);
    let identity = host.store.identity().to_path_buf(); let mut events = host.events.clone(); drop(host);
    let run = replay(&root, &config);
    events.push(Event::Core(super::super::super::super::BaseEvent::Time(ElapsedTick(2))));
    let successor = journal::encode(&profile(), &identity, &events).unwrap();
    std::fs::write(root.store().join("delivery.bin"), &successor).unwrap();
    assert_eq!(run.finish_pending_reset(&intent).err(), Some(Error::Binding.into()));
    assert_eq!(canonical(&root), successor);
    // Independently reopen the genuine successor; unchanged control still binds.
    let (host, _) = replay(&root, &config).finish_pending_reset(&intent).unwrap();
    let mut events = host.events.clone(); let canonical = canonical(&root); drop(host);
    let index = events.len() - 2;
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
        CheckpointEvent::Reset { witness, .. }))) = &mut events[index] else { panic!("reset before fence"); };
    let mut corrupt = witness.to_vec(); let end = corrupt.len() - 1; corrupt[end] ^= 1; *witness = corrupt.into();
    let bytes = journal::encode(&profile(), &identity, &events).unwrap();
    std::fs::write(root.store().join("delivery.bin"), &bytes).unwrap();
    let mut run = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let error = run.advance(0, 4096).unwrap_err(); assert_eq!(error, Error::Binding.into());
    assert_eq!(run.finish_pending_reset(&intent).err(), Some(error));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    assert_ne!(bytes, canonical);
}

mod guarded;
