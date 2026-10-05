//! Original numerical models, canonical bytes and two-key effect accounting.
//! Retained-prefix counts are structural work assertions, not timing results.
use super::*;
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::delivery::{EndpointOutcome, persistent::Reconciliation};
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;

type Outcome = Result<Rc<GenerationEvent>, Error>;
fn bytes(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn intent_ready(task: &mut FileLearnedIntentPreparation, host: &FileOversight) {
    while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
        task.advance(host, task.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(task.progress().status, FileLearnedStepPreparationStatus::Ready);
}
fn completion_ready(task: &mut FileLearnedStepPreparation, host: &FileOversight) {
    while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
        task.advance(host, task.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(task.progress().status, FileLearnedStepPreparationStatus::Ready);
}
fn pair(host: &mut FileOversight, carry: Option<FileLearnedReplayContinuation>)
    -> (Outcome, FileLearnedReplayContinuation)
{
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut intent = match carry {
        Some(carry) => carry.prepare_intent(host, host.revision(), n.actor_revision, n.position).unwrap(),
        None => host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap(),
    };
    intent_ready(&mut intent, host);
    let carry = intent.finish_with_continuation(host).unwrap();
    let mut completion = carry.prepare_completion(host, host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(completion.progress().total_events - completion.progress().replayed_events, 1);
    completion_ready(&mut completion, host);
    completion.finish_with_continuation(host).unwrap()
}
fn cold(host: &mut FileOversight) -> Outcome {
    // Force the original empty-machine route, independent of continuation use.
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut intent = host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(intent.progress().replayed_events, 0);
    intent_ready(&mut intent, host); intent.finish(host).unwrap();
    let mut completion = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(completion.progress().replayed_events, 0);
    completion_ready(&mut completion, host); completion.finish(host).unwrap()
}

#[test]
fn carried_predecessors_match_independent_generator_and_cold_journal_at_every_token() {
    let root = Directory::new(); let cold_root = Directory::new();
    let model = model(); let source = source(&model, false, 3);
    let config = FileLearnedConfig::new(model.clone(), source.clone(), LearnedDecoderBindingLimits::default()).unwrap();
    let mut original = model.monitored_generation_with_telemetry(source.stream, source.evaluation_origin,
        source.spec, source.policy, source.budget, source.telemetry).unwrap();
    let (mut host, _) = owner(&root, &config); let (mut control, _) = owner(&cold_root, &config);
    let mut carry: Option<FileLearnedReplayContinuation> = None;
    while original.status().is_active() {
        if let Some(previous) = &carry {
            assert_eq!(previous.verified_events() + 1, host.revision() as usize);
        }
        let expected = original.advance(original.position()).unwrap();
        let baseline = cold(&mut control).unwrap();
        let (actual, next) = pair(&mut host, carry.take()); let actual = actual.unwrap();
        assert_eq!(actual.status(), expected.status()); assert_eq!(actual.sample(), expected.sample());
        assert_eq!(actual.sample(), baseline.sample());
        let n = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(n.status, original.status()); assert_eq!(n.work, original.work());
        assert_eq!(n.telemetry, original.telemetry_work());
        let actor = host.machine.broker.retained_actor_state();
        assert_eq!(actor.tokens(), original.accepted_tokens());
        assert_eq!(actor.cache(), original.accepted_cache_image().unwrap().encode().unwrap());
        assert_eq!(actor.sampler(), original.sampler_state().encode());
        // Normalize ONLY the independent file identity; original event encodings
        // and numerical witnesses must be byte-identical, not merely same length.
        assert_eq!(bytes(&host), journal::encode(&host.profile, host.store.identity(), &control.events).unwrap());
        assert_eq!(read_machine(&host, &config).broker.hosted_learned_generation().unwrap(), n);
        assert_eq!(host.inspect().executions, 0);
        carry = Some(next);
    }
}

#[test]
fn continuation_must_replay_the_just_acknowledged_event_and_strict_finish_stays_stale() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut intent = host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap();
    intent_ready(&mut intent, &host); let old = host.revision();
    let carry = intent.finish_with_continuation(&mut host).unwrap();
    assert_eq!(carry.verified_events(), old as usize); assert_eq!(host.revision(), old + 1);
    let before = host.learned_generation_inspection().unwrap(); let disk = bytes(&host);
    let mut task = carry.prepare_completion(&host, host.revision(), n.actor_revision, n.position).unwrap();
    let progress = task.progress();
    assert_eq!(progress.replayed_events + 1, progress.total_events);
    assert_eq!(progress.status, FileLearnedStepPreparationStatus::Replaying);
    assert_eq!(task.advance(&host, progress.replayed_events + 1, 1).err(), Some(Error::Stale.into()));
    assert_eq!(task.progress(), progress); assert_eq!(bytes(&host), disk);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(task.advance(&host, progress.replayed_events, 1).err(), Some(Error::Stale.into()));
    let caught = task.catch_up(&host, host.revision(), progress.replayed_events, 1).unwrap();
    assert_eq!(caught.replayed_events, progress.replayed_events + 1);
    assert_eq!(caught.status, FileLearnedStepPreparationStatus::Replaying);
    assert_eq!(task.finish_with_continuation(&mut host).err(), Some(Error::Incomplete.into()));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before.numerical);
    assert_eq!(host.learned_generation_inspection().unwrap().pending, before.pending);
    // A discarded incomplete task did not abandon its acknowledged intent.
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
}

#[test]
fn carried_machine_has_no_store_lock_and_cannot_cross_owner_recovery_or_a_foreign_host() {
    for reopen in [false, true] {
        let root = Directory::new(); let other_root = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config); let (_, carry) = pair(&mut host, None);
        let other = if reopen {
            drop(host);
            FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap().0
        } else { owner(&other_root, &config).0 };
        let n = other.learned_generation_inspection().unwrap().numerical; let disk = bytes(&other);
        assert_eq!(carry.prepare_intent(&other, other.revision(), n.actor_revision, n.position).err(),
            Some(Error::Binding.into()));
        assert_eq!(bytes(&other), disk); assert_eq!(other.inspect().executions, 0);
    }
}

#[test]
fn continuation_rechecks_original_source_pause_position_and_ordinary_capacity() {
    for defect in 0..5 {
        let root = Directory::new(); let config = config(false, 1);
        let mut profile = profile(); if defect == 4 { profile.delivery.limits.events = 5; }
        let (mut host, _) = FileOversight::create(root.store(), profile).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.enable_learned_generation(host.revision(), config).unwrap();
        let (_, carry) = pair(&mut host, None);
        let mut n = host.learned_generation_inspection().unwrap().numerical;
        match defect {
            0 => host.source_interrupted = true,
            1 => host.fence(host.revision()).unwrap(),
            2 => n.actor_revision += 1,
            3 => n.position += 1,
            _ => {}
        }
        let before = host.inspect(); let disk = bytes(&host);
        let expected = host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).err();
        assert!(expected.is_some());
        assert_eq!(carry.prepare_intent(&host, host.revision(), n.actor_revision, n.position).err(), expected);
        assert_eq!(host.inspect(), before); assert_eq!(bytes(&host), disk);
        assert!(host.storage_failure().is_none());
    }
}

#[test]
fn neither_failed_write_returns_a_continuation_or_candidate_output_and_recovery_stays_original() {
    for intent_stage in [true, false] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
            let n = host.learned_generation_inspection().unwrap().numerical;
            let mut task = host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap();
            intent_ready(&mut task, &host);
            let before;
            let result = if intent_stage {
                before = host.inspect(); host.store.fail_once(barrier);
                task.finish_with_continuation(&mut host).map(|_| ())
            } else {
                let carry = task.finish_with_continuation(&mut host).unwrap();
                let mut task = carry.prepare_completion(&host, host.revision(), n.actor_revision, n.position).unwrap();
                completion_ready(&mut task, &host); before = host.inspect(); host.store.fail_once(barrier);
                task.finish_with_continuation(&mut host).map(|_| ())
            };
            assert!(matches!(result, Err(JournalError::Io(failure)) if failure.operation == barrier));
            assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), before);
            assert_eq!(host.machine.broker.hosted_learned_generation().unwrap(), n);
            drop(host);
            let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
            let state = recovered.learned_generation_inspection().unwrap();
            assert!(state.paused); assert_eq!(recovered.inspect().executions, 0);
            assert_eq!(recovered.inspect().control.ledger.available, 100);
            if intent_stage || state.pending.is_some() { assert_eq!(state.numerical, n); }
            else { assert_eq!(state.numerical.position, n.position + 1); }
        }
    }
}

#[test]
fn retained_prefix_does_not_hide_a_corrupt_new_numerical_witness() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let (_, carry) = pair(&mut host, None); let index = carry.verified_events();
    let original = host.events[index].clone(); let disk = bytes(&host);
    let before = host.learned_generation_inspection().unwrap();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = &mut host.events[index]
        else { panic!("retired predecessor must still be missing its witnessed Step"); };
    let mut bad = witness.to_vec(); bad[0] ^= 1; *witness = bad.into();
    let n = &before.numerical;
    let mut task = carry.prepare_intent(&host, host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(task.advance(&host, index, 1).err(), Some(Error::Binding.into()));
    assert_eq!(task.progress().status, FileLearnedStepPreparationStatus::Failed(Error::Binding));
    assert_eq!(task.advance(&host, index, 1).err(), Some(Error::Binding.into()));
    assert_eq!(task.finish_with_continuation(&mut host).err(), Some(Error::Binding.into()));
    assert_eq!(host.learned_generation_inspection().unwrap(), before); assert_eq!(bytes(&host), disk);
    host.events[index] = original;
    cold(&mut host).unwrap();
}

#[test]
fn carried_replay_preserves_executed_receipts_and_unknown_charges_without_publishing_again() {
    for executed in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
        let (_, carry) = pair(&mut host, None);
        let (action, input, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
        if executed {
            assert_eq!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
                EndpointOutcome::Executed { resulting_version: 2 });
            host.reconcile(host.revision(), 1).unwrap();
        } else { host.cancel(host.revision(), 1).unwrap(); }
        let prior = host.inspect(); let disk = bytes(&host);
        let n = host.learned_generation_inspection().unwrap().numerical;
        let mut task = carry.prepare_intent(&host, host.revision(), n.actor_revision, n.position).unwrap();
        while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
            task.advance(&host, task.progress().replayed_events, 1).unwrap();
            assert_eq!(host.inspect(), prior); assert_eq!(bytes(&host), disk);
        }
        let carry = task.finish_with_continuation(&mut host).unwrap();
        let mut task = carry.prepare_completion(&host, host.revision(), n.actor_revision, n.position).unwrap();
        completion_ready(&mut task, &host); task.finish_with_continuation(&mut host).unwrap().0.unwrap();
        assert_eq!(host.inspect().executions, u64::from(executed));
        assert_eq!(host.inspect().control.ledger.available, 84);
        if executed {
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
            assert_eq!(host.inspect().payload, b"visible");
        } else { assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution); }
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
    }
}

#[test]
fn actual_monitor_hold_keeps_sample_private_and_cannot_restart_from_a_carried_quiet_prefix() {
    let root = Directory::new(); let config = config(true, 1); let (mut host, _) = owner(&root, &config);
    let (first, carry) = pair(&mut host, None); first.unwrap();
    let accepted = host.learned_generation_inspection().unwrap().numerical;
    let (held, carry) = pair(&mut host, Some(carry)); let held = held.unwrap();
    assert!(held.accepted().is_none() && held.sample().is_none());
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert!(!n.status.is_active()); assert_eq!(n.position, accepted.position);
    assert_eq!(n.work.admitted_tokens, accepted.work.admitted_tokens + 1);
    assert_eq!(carry.prepare_intent(&host, host.revision(), n.actor_revision, n.position).err(),
        Some(Error::WrongState.into()));
    drop(host);
    let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(recovered.learned_generation_inspection().unwrap().numerical, n);
    assert_eq!(recovered.inspect().executions, 0);
}
