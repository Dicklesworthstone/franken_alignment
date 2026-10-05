//! Both cooperative stages retain the original write-ahead and effect laws.
use super::*;
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;
use crate::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};

fn prepare_intent(host: &FileOversight) -> FileLearnedIntentPreparation {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap()
}
fn ready_intent(task: &mut FileLearnedIntentPreparation, host: &FileOversight) {
    while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
        task.advance(host, task.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(task.progress().status, FileLearnedStepPreparationStatus::Ready);
}
fn one(host: &mut FileOversight) -> Result<Result<Rc<GenerationEvent>, Error>, JournalError> {
    let mut admission = prepare_intent(host); ready_intent(&mut admission, host);
    admission.finish(host)?;
    let n = host.learned_generation_inspection()?.numerical;
    let mut completion = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position)?;
    ready(&mut completion, host); completion.finish(host)
}

#[test]
fn cooperative_intent_matches_original_transact_begin_and_preserves_canonical_event_history() {
    let a = Directory::new(); let b = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&a, &config); let (mut reference, _) = owner(&b, &config);
    while host.learned_generation_inspection().unwrap().numerical.status.is_active() {
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
        let mut task = prepare_intent(&host);
        while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
            let previous = task.progress().replayed_events;
            let next = task.advance(&host, previous, 2).unwrap();
            assert_eq!(next.replayed_events, (previous + 2).min(next.total_events));
            assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), before);
        }
        task.finish(&mut host).unwrap();
        let n = before.numerical;
        let intent = LearnedStepIntent { actor_revision: n.actor_revision, position: n.position };
        assert_eq!(host.learned_generation_inspection().unwrap().pending, Some(intent));
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        // Independent original transaction path, not the new intent finalizer.
        reference.transact(reference.revision(), Event::Decoder(DecoderEvent::Learned(LearnedEvent::Begin(intent)))).unwrap();
        assert_eq!(disk(&host), journal::encode(&host.profile, host.store.identity(), &reference.events).unwrap());
        let mut completion = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
        ready(&mut completion, &host); let actual = completion.finish(&mut host).unwrap().unwrap();
        let expected = reference.complete_learned_step(reference.revision(), n.actor_revision, n.position).unwrap().unwrap();
        assert_eq!(actual.sample(), expected.sample()); assert_eq!(actual.status(), expected.status());
        assert_eq!(host.revision(), before.journal_revision + 2);
        assert_eq!(disk(&host), journal::encode(&host.profile, host.store.identity(), &reference.events).unwrap());
        assert_eq!(host.learned_generation_inspection().unwrap(), reference.learned_generation_inspection().unwrap());
    }
}

#[test]
fn abandoning_intent_preparation_creates_no_pending_operation_or_partial_numerical_state() {
    for stage in 0..3 {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
        let mut task = prepare_intent(&host);
        match stage {
            0 => assert_eq!(task.finish(&mut host), Err(JournalError::Contract(Error::Incomplete))),
            1 => { task.advance(&host, 0, 1).unwrap(); drop(task); }
            _ => { ready_intent(&mut task, &host); drop(task); }
        }
        assert_eq!(host.learned_generation_inspection().unwrap(), before); assert_eq!(disk(&host), bytes);
        assert!(host.learned_generation_inspection().unwrap().pending.is_none());
        assert!(host.prepare_learned_step_completion(host.revision(), before.numerical.actor_revision,
            before.numerical.position).is_err());
        one(&mut host).unwrap().unwrap();
        assert_eq!(host.revision(), before.journal_revision + 2);
    }
}

#[test]
fn intent_preparation_requires_two_ordinary_slots_and_rejects_wrong_positions_before_replay() {
    for events in [3, 4] {
        let root = Directory::new(); let config = config(false, 1); let mut limits = profile();
        limits.delivery.limits.events = events;
        let (mut host, _) = FileOversight::create(root.store(), limits).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.enable_learned_generation(host.revision(), config).unwrap();
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host); let n = &before.numerical;
        let task = host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position);
        if events == 3 {
            assert!(matches!(task, Err(JournalError::Contract(Error::Limit))));
            assert_eq!(host.learned_generation_inspection().unwrap(), before); assert_eq!(disk(&host), bytes);
        } else {
            assert!(matches!(host.prepare_learned_step_intent(host.revision(), n.actor_revision + 1, n.position),
                Err(JournalError::Contract(Error::Stale))));
            assert!(matches!(host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position + 1),
                Err(JournalError::Contract(Error::Stale))));
            let mut task = task.unwrap(); ready_intent(&mut task, &host); task.finish(&mut host).unwrap();
            let mut completion = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
            ready(&mut completion, &host); completion.finish(&mut host).unwrap().unwrap();
            assert_eq!(host.revision(), 4);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, n.position + 1);
        }
    }
}

#[test]
fn each_intent_storage_failure_returns_no_admission_and_never_executes_the_new_step() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let mut task = prepare_intent(&host); ready_intent(&mut task, &host);
        let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap().numerical;
        host.store.fail_once(barrier);
        assert!(matches!(task.finish(&mut host), Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some());
        assert_eq!(host.machine.broker.hosted_learned_generation().unwrap(), numerical);
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn actor_poll_and_cancellation_can_interleave_with_either_preparation_stage() {
    for pending in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let spec = action_spec(&host);
        let (port, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let ticket = port.submit(71, &ActorProposal { target: spec.target.unwrap(), payload: spec.payload,
            expected_policy_epoch: spec.policy_epoch, deadline: spec.deadline, units: spec.units }).unwrap();
        let mut admission = Some(prepare_intent(&supervisor.host().unwrap()));
        ready_intent(admission.as_mut().unwrap(), &supervisor.host().unwrap());
        let mut completion = None;
        if pending {
            admission.take().unwrap().finish(&mut supervisor.host_mut().unwrap()).unwrap();
            let host = supervisor.host().unwrap(); let n = host.learned_generation_inspection().unwrap().numerical;
            let mut task = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
            task.advance(&host, 0, 1).unwrap(); completion = Some(task);
        }
        assert_eq!(port.poll(&ticket), Knowledge::Pending { request: 71 });
        let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap().numerical;
        port.cancel(&ticket).unwrap();
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        let mut host = supervisor.host_mut().unwrap(); let bytes = disk(&host);
        if let Some(task) = admission {
            assert_eq!(task.finish(&mut host), Err(JournalError::Contract(Error::Stale)));
        }
        if let Some(task) = completion {
            assert_eq!(task.finish(&mut host).err(), Some(JournalError::Contract(Error::Stale)));
        }
        assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.learned_generation_inspection().unwrap().pending.is_some(), pending);
        if !pending { let mut task = prepare_intent(&host); ready_intent(&mut task, &host); task.finish(&mut host).unwrap(); }
        let mut task = host.prepare_learned_step_completion(host.revision(), numerical.actor_revision, numerical.position).unwrap();
        ready(&mut task, &host); task.finish(&mut host).unwrap().unwrap();
        assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
        drop(host);
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
}

#[test]
fn explicit_two_stage_execution_preserves_original_monitor_alarm_and_quiet_control() {
    for alarm in [false, true] {
        let root = Directory::new(); let config = config(alarm, 1); let (mut host, _) = owner(&root, &config);
        one(&mut host).unwrap().unwrap();
        let accepted = host.learned_generation_inspection().unwrap().numerical;
        let event = one(&mut host).unwrap().unwrap();
        if alarm {
            assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
            assert!(event.accepted().is_none() && event.sample().is_none());
            let after = host.learned_generation_inspection().unwrap();
            assert_eq!(after.numerical.position, accepted.position);
            assert_eq!(after.numerical.sampled_draws, accepted.sampled_draws);
            assert!(host.prepare_learned_step_intent(host.revision(), after.numerical.actor_revision, after.numerical.position).is_err());
        } else {
            assert!(event.sample().is_some());
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, accepted.position + 1);
        }
        assert!(host.learned_generation_inspection().unwrap().pending.is_none());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn acknowledged_numerical_budget_failure_is_not_an_uncommitted_or_refillable_completion() {
    let model = model(); let mut source = source(&model, false, 1);
    let mut control = model.observed_learned_generation(source.clone()).unwrap(); control.advance(0).unwrap();
    source.telemetry.source_check_values = control.telemetry_work().source_check_values;
    let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    let root = Directory::new(); let (mut host, _) = owner(&root, &config);
    one(&mut host).unwrap().unwrap(); let revision = host.revision();
    assert_eq!(one(&mut host).unwrap().err(), Some(Error::Limit));
    assert_eq!(host.revision(), revision + 2);
    let failed = host.learned_generation_inspection().unwrap();
    assert!(failed.pending.is_none()); assert_eq!(failed.numerical.status, GenerationStatus::Failed(Error::Limit));
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, failed.numerical);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.resume_learned_generation(host.revision(), failed.numerical.actor_revision, failed.numerical.position).is_err());
    assert_eq!(host.inspect().executions, 0);
}
