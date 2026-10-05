//! Original replay with live append-only control changes, not a shadow ledger.
use super::*;
use crate::action::consequence::delivery::persistent::Event as BaseEvent;

enum Task {
    Intent(FileLearnedIntentPreparation),
    Completion(FileLearnedStepPreparation),
}
impl Task {
    fn new(host: &mut FileOversight, completion: bool) -> Self {
        let state = host.learned_generation_inspection().unwrap();
        let n = state.numerical;
        if completion {
            if state.pending.is_none() {
                host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
            }
            Self::Completion(host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap())
        } else {
            Self::Intent(host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap())
        }
    }
    fn progress(&self) -> FileLearnedStepPreparationProgress {
        match self { Self::Intent(task) => task.progress(), Self::Completion(task) => task.progress() }
    }
    fn advance(&mut self, host: &FileOversight, cursor: usize, budget: usize)
        -> Result<FileLearnedStepPreparationProgress, JournalError>
    {
        match self {
            Self::Intent(task) => task.advance(host, cursor, budget),
            Self::Completion(task) => task.advance(host, cursor, budget),
        }
    }
    fn catch_up(&mut self, host: &FileOversight, revision: u64, cursor: usize, budget: usize)
        -> Result<FileLearnedStepPreparationProgress, JournalError>
    {
        match self {
            Self::Intent(task) => task.catch_up(host, revision, cursor, budget),
            Self::Completion(task) => task.catch_up(host, revision, cursor, budget),
        }
    }
    fn ready(&mut self, host: &FileOversight) {
        while self.progress().status == FileLearnedStepPreparationStatus::Replaying {
            self.advance(host, self.progress().replayed_events, 1).unwrap();
        }
        assert_eq!(self.progress().status, FileLearnedStepPreparationStatus::Ready);
    }
    fn finish(self, host: &mut FileOversight) -> Result<Option<Rc<GenerationEvent>>, JournalError> {
        match self {
            Self::Intent(task) => { task.finish(host)?; Ok(None) }
            Self::Completion(task) => Ok(Some(task.finish(host)??)),
        }
    }
}

#[test]
fn both_stages_progress_under_clock_writes_and_match_every_original_sample_and_cache() {
    let root = Directory::new(); let model = model(); let source = source(&model, false, 3);
    let config = FileLearnedConfig::new(model.clone(), source.clone(), LearnedDecoderBindingLimits::default()).unwrap();
    let mut original = model.monitored_generation_with_telemetry(source.stream, source.evaluation_origin,
        source.spec, source.policy, source.budget, source.telemetry).unwrap();
    let (mut host, _) = owner(&root, &config); let mut tick = 1;
    while original.status().is_active() {
        let expected = original.advance(original.position()).unwrap();
        for completion in [false, true] {
            let mut task = Task::new(&mut host, completion);
            task.advance(&host, 0, 1).unwrap();
            let mut visited = 1;
            for _ in 0..MAX_JOURNAL_EVENTS {
                tick += 1; host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
                let before = task.progress(); let state = host.learned_generation_inspection().unwrap();
                let bytes = disk(&host);
                let after = task.catch_up(&host, host.revision(), before.replayed_events, 3).unwrap();
                assert_eq!(after.replayed_events, (before.replayed_events + 3).min(after.total_events));
                visited += after.replayed_events - before.replayed_events;
                assert_eq!(visited, after.replayed_events, "catch-up cannot reset the replay cursor");
                assert_eq!(after.journal_revision, host.revision());
                assert_eq!(after.intent, before.intent);
                assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), state);
                if after.status == FileLearnedStepPreparationStatus::Ready { break; }
            }
            let progress = task.progress();
            assert_eq!(progress.status, FileLearnedStepPreparationStatus::Ready);
            assert_eq!(visited, progress.total_events);
            assert_eq!(task.catch_up(&host, host.revision(), progress.replayed_events, 1).unwrap(), progress);
            let before = host.learned_generation_inspection().unwrap();
            let result = task.finish(&mut host).unwrap();
            assert_eq!(host.revision(), before.journal_revision + 1);
            if completion {
                let actual = result.unwrap();
                assert_eq!(actual.status(), expected.status()); assert_eq!(actual.sample(), expected.sample());
                let after = host.learned_generation_inspection().unwrap();
                assert!(after.pending.is_none()); assert_eq!(after.numerical.work, original.work());
                assert_eq!(after.numerical.telemetry, original.telemetry_work());
                let actor = host.machine.broker.retained_actor_state();
                assert_eq!(actor.tokens(), original.accepted_tokens());
                assert_eq!(actor.cache(), original.accepted_cache_image().unwrap().encode().unwrap());
                assert_eq!(actor.sampler(), original.sampler_state().encode());
            } else {
                assert!(result.is_none());
                assert_eq!(host.learned_generation_inspection().unwrap().numerical, before.numerical);
                assert_eq!(host.learned_generation_inspection().unwrap().pending, Some(progress.intent));
            }
        }
    }
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn strict_advance_and_finish_cannot_skip_unadopted_or_partially_replayed_tail() {
    for completion in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let mut task = Task::new(&mut host, completion); task.ready(&host);
        let cursor = task.progress().replayed_events;
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        assert_eq!(task.advance(&host, cursor, 1).err(), Some(JournalError::Contract(Error::Stale)));
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
        let progress = task.catch_up(&host, host.revision(), cursor, 1).unwrap();
        assert_eq!(progress.status, FileLearnedStepPreparationStatus::Replaying);
        assert_eq!(progress.replayed_events, cursor + 1);
        assert_eq!(task.finish(&mut host).err(), Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), before);
        let mut control = Task::new(&mut host, completion); control.ready(&host); control.finish(&mut host).unwrap();
        assert_eq!(host.revision(), before.journal_revision + 1);
    }
}

#[test]
fn failed_catch_up_admission_leaves_the_old_cut_and_cursor_unchanged() {
    for completion in [false, true] {
        let a = Directory::new(); let b = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&a, &config); let (mut other, _) = owner(&b, &config);
        let mut task = Task::new(&mut host, completion); let _other = Task::new(&mut other, completion);
        task.advance(&host, 0, 1).unwrap();
        let before = task.progress();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let bytes = disk(&host);
        for (revision, cursor, budget, error) in [
            (before.journal_revision, 1, 1, Error::Stale),
            (host.revision() + 1, 1, 1, Error::Stale),
            (host.revision(), 0, 1, Error::Stale),
            (host.revision(), 1, 0, Error::InvalidInput),
            (host.revision(), 1, MAX_JOURNAL_EVENTS + 1, Error::Limit),
        ] {
            assert_eq!(task.catch_up(&host, revision, cursor, budget).err(), Some(JournalError::Contract(error)));
            assert_eq!(task.progress(), before); assert_eq!(disk(&host), bytes);
        }
        assert_eq!(task.catch_up(&other, other.revision(), 1, 1).err(), Some(JournalError::Contract(Error::Binding)));
        // Isolate the live interruption latch; it changes no journal revision.
        host.source_interrupted = true;
        assert_eq!(task.catch_up(&host, host.revision(), 1, 1).err(), Some(JournalError::Contract(Error::Incomplete)));
        assert_eq!(task.progress(), before); assert_eq!(disk(&host), bytes);
        host.source_interrupted = false;
        let progress = task.catch_up(&host, host.revision(), 1, MAX_JOURNAL_EVENTS).unwrap();
        assert_eq!(progress.status, FileLearnedStepPreparationStatus::Ready);
        task.finish(&mut host).unwrap();
    }
}

#[test]
fn competing_numerics_and_recovery_cannot_be_relabelled_as_the_same_operation() {
    for completion in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let mut task = Task::new(&mut host, completion); task.ready(&host);
        let before = task.progress(); step(&mut host).unwrap();
        let bytes = disk(&host); let numerical = host.learned_generation_inspection().unwrap();
        assert!(task.catch_up(&host, host.revision(), before.replayed_events, 1).is_err());
        assert_eq!(task.progress(), before); assert_eq!(disk(&host), bytes);
        assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
        drop(task);
        let mut orphan = Task::new(&mut host, completion); orphan.ready(&host);
        let progress = orphan.progress();
        host.fence(host.revision()).unwrap();
        assert!(orphan.catch_up(&host, host.revision(), progress.replayed_events, 1).is_err());
        assert_eq!(orphan.progress(), progress);
        drop(host);
        let (reopened, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let bytes = disk(&reopened);
        assert_eq!(orphan.catch_up(&reopened, reopened.revision(), progress.replayed_events, 1).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert_eq!(disk(&reopened), bytes);
    }
}

#[test]
fn cancellation_and_receipt_tails_keep_executed_or_unknown_effects_and_full_charges() {
    for executed in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
        step(&mut host).unwrap(); let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        if executed {
            host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
        }
        let mut task = prepare(&mut host); ready(&mut task, &host); let cursor = task.progress().replayed_events;
        if executed { host.reconcile(host.revision(), 1).unwrap(); }
        else { host.cancel(host.revision(), 1).unwrap(); }
        let state = host.inspect(); let bytes = disk(&host);
        task.catch_up(&host, host.revision(), cursor, MAX_JOURNAL_EVENTS).unwrap();
        assert_eq!(disk(&host), bytes); assert_eq!(host.inspect(), state);
        task.finish(&mut host).unwrap().unwrap();
        let after = host.inspect();
        assert_eq!(after.executions, u64::from(executed)); assert_eq!(after.control.ledger.available, 84);
        assert_eq!(after.control.ledger.stages, state.control.ledger.stages);
        assert_eq!(after.payload, state.payload);
        if !executed { assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution); }
    }
}

#[test]
fn adopted_candidates_still_obey_every_original_intent_and_completion_storage_barrier() {
    for completion in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
            step(&mut host).unwrap(); let mut task = Task::new(&mut host, completion); task.ready(&host);
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            task.catch_up(&host, host.revision(), task.progress().replayed_events, 1).unwrap();
            let state = host.inspect(); let numerical = host.learned_generation_inspection().unwrap().numerical;
            host.store.fail_once(barrier);
            assert!(matches!(task.finish(&mut host), Err(JournalError::Io(failure)) if failure.operation == barrier));
            assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), state);
            assert_eq!(host.machine.broker.hosted_learned_generation().unwrap(), numerical);
            drop(host);
            let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
            let recovered = host.learned_generation_inspection().unwrap();
            assert!(recovered.paused); assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().control.ledger.available, 100);
            if completion && recovered.pending.is_none() { assert_eq!(recovered.numerical.position, numerical.position + 1); }
            else { assert_eq!(recovered.numerical, numerical); }
        }
    }
}

#[test]
fn catch_up_rechecks_capacity_after_other_writes_consume_the_remaining_slots() {
    for completion in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let mut p = profile();
        p.delivery.limits.events = if completion { 5 } else { 4 };
        let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.enable_learned_generation(host.revision(), config).unwrap();
        let mut task = Task::new(&mut host, completion); task.ready(&host);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        if completion { host.observe_time(host.revision(), ElapsedTick(3)).unwrap(); }
        let before = task.progress(); let bytes = disk(&host);
        assert_eq!(task.catch_up(&host, host.revision(), before.replayed_events, 1).err(),
            Some(JournalError::Contract(Error::Limit)));
        assert_eq!(task.progress(), before); assert_eq!(disk(&host), bytes);
        assert!(host.storage_failure().is_none());
    }
}

#[test]
fn malformed_tail_fails_in_original_reducer_without_rolling_back_the_verified_prefix() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let mut task = prepare(&mut host); ready(&mut task, &host);
    let cursor = task.progress().replayed_events; let bytes = disk(&host);
    // Deliberately corrupt only private test history: committed public APIs never
    // append this backward Time. The actual canonical file remains untouched.
    host.events.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    assert_eq!(task.catch_up(&host, host.revision(), cursor, 1).err(), Some(JournalError::Contract(Error::Stale)));
    let failed = task.progress();
    assert_eq!(failed.replayed_events, cursor);
    assert_eq!(failed.status, FileLearnedStepPreparationStatus::Failed(Error::Stale));
    host.events.pop();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(task.catch_up(&host, host.revision(), cursor, 1).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(task.progress(), failed); assert!(host.storage_failure().is_none());
    assert_ne!(disk(&host), bytes, "only the acknowledged positive Time changed storage");
    let mut control = Task::new(&mut host, true); control.ready(&host); control.finish(&mut host).unwrap();
}

#[test]
fn original_actor_ticket_cancellation_is_replayed_without_restarting_either_stage() {
    use crate::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
    for completion in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap();
        let target = host.inspect().target; let epoch = host.inspect().control.ledger.epoch;
        let (port, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let ticket = port.submit(71, &ActorProposal { target, payload: b"visible".to_vec(),
            expected_policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }).unwrap();
        let mut task = { let mut host = supervisor.host_mut().unwrap(); Task::new(&mut host, completion) };
        task.ready(&supervisor.host().unwrap());
        assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
        let cursor = task.progress().replayed_events;
        port.cancel(&ticket).unwrap();
        {
            let host = supervisor.host().unwrap();
            let state = host.inspect(); let bytes = disk(&host);
            let progress = task.catch_up(&host, host.revision(), cursor, 1).unwrap();
            assert_eq!(progress.replayed_events, cursor + 1);
            assert_eq!(progress.status, FileLearnedStepPreparationStatus::Ready);
            assert_eq!(disk(&host), bytes); assert_eq!(host.inspect(), state);
        }
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        {
            let mut host = supervisor.host_mut().unwrap();
            task.finish(&mut host).unwrap();
            if !completion { step(&mut host).unwrap(); }
            assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
        }
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
}

#[cfg(unix)]
mod policy;
