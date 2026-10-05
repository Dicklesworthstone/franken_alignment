//! Original numerical engines, canonical journal files and causal controls.
//! Event quanta are not a proof of bounded physical work or wall-clock latency.
use super::*;
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::delivery::{EndpointOutcome};
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS, Reconciliation};
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;

fn prepare(host: &mut FileOversight) -> FileLearnedStepPreparation {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap()
}
fn ready(task: &mut FileLearnedStepPreparation, host: &FileOversight) {
    while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
        task.advance(host, task.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(task.progress().status, FileLearnedStepPreparationStatus::Ready);
}
fn disk(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}

#[test]
fn cooperative_completion_matches_independent_generator_at_every_original_token() {
    let root = Directory::new(); let model = model(); let source = source(&model, false, 3);
    let config = FileLearnedConfig::new(model.clone(), source.clone(), LearnedDecoderBindingLimits::default()).unwrap();
    let mut original = model.monitored_generation_with_telemetry(source.stream, source.evaluation_origin,
        source.spec, source.policy, source.budget, source.telemetry).unwrap();
    let (mut host, _) = owner(&root, &config);
    while original.status().is_active() {
        let expected = original.advance(original.position()).unwrap();
        let mut task = prepare(&mut host);
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
        assert_eq!(task.progress().replayed_events, 0);
        assert_eq!(task.progress().total_events as u64, host.revision());
        while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
            let cursor = task.progress().replayed_events;
            let progress = task.advance(&host, cursor, 1).unwrap();
            assert_eq!(progress.replayed_events, cursor + 1);
            assert_eq!(host.learned_generation_inspection().unwrap(), before);
            assert_eq!(disk(&host), bytes);
        }
        let progress = task.progress();
        assert_eq!(task.advance(&host, progress.replayed_events, 1).unwrap(), progress);
        let actual = task.finish(&mut host).unwrap().unwrap();
        assert_eq!(actual.status(), expected.status()); assert_eq!(actual.sample(), expected.sample());
        let after = host.learned_generation_inspection().unwrap();
        assert_eq!(after.journal_revision, before.journal_revision + 1); assert!(after.pending.is_none());
        assert_eq!(after.numerical.status, original.status());
        assert_eq!(after.numerical.work, original.work());
        assert_eq!(after.numerical.telemetry, original.telemetry_work());
        let actor = host.machine.broker.retained_actor_state();
        assert_eq!(actor.tokens(), original.accepted_tokens());
        assert_eq!(actor.cache(), original.accepted_cache_image().unwrap().encode().unwrap());
        assert_eq!(actor.sampler(), original.sampler_state().encode());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn dropping_or_prematurely_finishing_preparation_keeps_the_durable_intent_recoverable() {
    for stage in 0..3 {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let mut task = prepare(&mut host);
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
        match stage {
            0 => { assert_eq!(task.finish(&mut host).err(), Some(JournalError::Contract(Error::Incomplete))); }
            1 => { task.advance(&host, 0, 1).unwrap(); drop(task); }
            _ => { ready(&mut task, &host); drop(task); }
        }
        assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), before);
        assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().pending, before.pending);
        assert!(host.learned_generation_inspection().unwrap().paused);
        let n = before.numerical;
        assert!(host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).is_err());
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
        let mut task = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
        ready(&mut task, &host); task.finish(&mut host).unwrap().unwrap();
        assert!(host.learned_generation_inspection().unwrap().pending.is_none());
    }
}

#[test]
fn foreign_owner_bad_quantum_and_stale_cursor_do_no_work_and_live_mutation_invalidates_the_cut() {
    let a = Directory::new(); let b = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&a, &config); let (mut other, _) = owner(&b, &config);
    let mut task = prepare(&mut host); let _other = prepare(&mut other);
    let initial = task.progress(); let bytes = disk(&host);
    assert_eq!(task.advance(&other, 0, 1).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(task.advance(&host, 1, 1).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(task.advance(&host, 0, 0).err(), Some(JournalError::Contract(Error::InvalidInput)));
    assert_eq!(task.advance(&host, 0, MAX_JOURNAL_EVENTS + 1).err(), Some(JournalError::Contract(Error::Limit)));
    assert_eq!(task.progress(), initial); assert_eq!(disk(&host), bytes);
    task.advance(&host, 0, 1).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let after = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
    assert_eq!(task.advance(&host, 1, 1).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(task.finish(&mut host).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), after);
    let n = after.numerical;
    let mut fresh = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
    ready(&mut fresh, &host); fresh.finish(&mut host).unwrap().unwrap();
}

#[test]
fn two_ready_preparations_cannot_complete_one_intent_twice_or_survive_owner_recovery() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let mut first = prepare(&mut host); let intent = first.progress().intent;
    let mut second = host.prepare_learned_step_completion(host.revision(), intent.actor_revision, intent.position).unwrap();
    ready(&mut first, &host); ready(&mut second, &host);
    first.finish(&mut host).unwrap().unwrap();
    let state = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
    assert_eq!(second.finish(&mut host).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(host.learned_generation_inspection().unwrap(), state); assert_eq!(disk(&host), bytes);
    let mut orphan = prepare(&mut host); ready(&mut orphan, &host); drop(host);
    let (mut reopened, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let bytes = disk(&reopened);
    assert_eq!(orphan.finish(&mut reopened).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(disk(&reopened), bytes);
}

#[test]
fn same_cut_source_interruption_is_checked_even_after_reconstruction_is_ready() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let mut task = prepare(&mut host); ready(&mut task, &host);
    let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
    // Exercise the live latch independently of a journal revision change.
    host.source_interrupted = true;
    assert_eq!(task.advance(&host, task.progress().replayed_events, 1).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(task.finish(&mut host).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), before);
    assert!(host.storage_failure().is_none());
}

#[test]
fn all_completion_storage_failures_keep_candidate_output_private_and_recovery_decides_visibility() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let mut task = prepare(&mut host); ready(&mut task, &host);
        let numerical = host.learned_generation_inspection().unwrap().numerical; let before = host.inspect();
        host.store.fail_once(barrier);
        assert!(matches!(task.finish(&mut host), Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), before);
        assert_eq!(host.machine.broker.hosted_learned_generation().unwrap(), numerical);
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let recovered = host.learned_generation_inspection().unwrap();
        assert!(recovered.paused); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
        if recovered.pending.is_some() { assert_eq!(recovered.numerical, numerical); }
        else { assert_eq!(recovered.numerical.position, numerical.position + 1); }
    }
}

#[test]
fn corrupt_historical_witness_is_rejected_by_the_original_reducer_and_failure_is_sticky() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let initial = prepare(&mut host); drop(initial);
    let bytes = disk(&host); let before = host.learned_generation_inspection().unwrap();
    let index = host.events.iter().position(|event| matches!(event,
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { .. })))).unwrap();
    let original = host.events[index].clone();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = &mut host.events[index]
        else { unreachable!() };
    let mut bad = witness.to_vec(); bad[0] ^= 1; *witness = bad.into();
    let intent = before.pending.unwrap();
    let mut task = host.prepare_learned_step_completion(host.revision(), intent.actor_revision, intent.position).unwrap();
    assert_eq!(task.advance(&host, 0, MAX_JOURNAL_EVENTS).err(), Some(JournalError::Contract(Error::Binding)));
    let progress = task.progress();
    assert_eq!(progress.replayed_events, index);
    assert_eq!(progress.status, FileLearnedStepPreparationStatus::Failed(Error::Binding));
    assert_eq!(task.advance(&host, index, 1).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(task.finish(&mut host).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.learned_generation_inspection().unwrap(), before); assert_eq!(disk(&host), bytes);
    assert!(host.storage_failure().is_none());
    host.events[index] = original;
    let mut control = host.prepare_learned_step_completion(host.revision(), intent.actor_revision, intent.position).unwrap();
    ready(&mut control, &host); control.finish(&mut host).unwrap().unwrap();
}

#[test]
fn history_reconstruction_neither_republishes_executed_effects_nor_refunds_unknown_dispatches() {
    for executed in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
        step(&mut host).unwrap(); let (action, inputs, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        if executed {
            assert_eq!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
                EndpointOutcome::Executed { resulting_version: 2 });
            host.reconcile(host.revision(), 1).unwrap();
        } else { host.cancel(host.revision(), 1).unwrap(); }
        let mut task = prepare(&mut host); let bytes = disk(&host);
        ready(&mut task, &host); assert_eq!(disk(&host), bytes);
        task.finish(&mut host).unwrap().unwrap();
        assert_eq!(host.inspect().executions, u64::from(executed));
        assert_eq!(host.inspect().control.ledger.available, 84);
        if executed {
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
            assert_eq!(host.inspect().payload, b"visible");
        } else {
            assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().payload, b"initial");
        }
    }
}

mod intent;
