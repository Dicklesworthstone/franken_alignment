//! Original reducer, numerical model and two-key endpoint behind warm controls.
//! Structural prefix assertions are not elapsed-time performance measurements.
use crate::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedConfig, FileLearnedRecoveryStatus, FileHumanReviewer, FileOversightProfile,
    JournalFailure, JournalIo, bind_history, journal, storage,
};
use crate::action::consequence::delivery::persistent::{Event as BaseEvent, Reconciliation};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::ActionState;
// Compile the same original numerical and authority fixture, not copied models.
include!("../../tests/fixture.rs");

fn prefix(host: &FileOversight) -> Option<usize> {
    host.learned_replay.as_ref().map(FileLearnedReplayContinuation::verified_events)
}
fn disk(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn cold_step(host: &mut FileOversight) {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut intent = host.prepare_learned_step_intent(host.revision(), n.actor_revision, n.position).unwrap();
    while intent.progress().status == FileLearnedStepPreparationStatus::Replaying {
        intent.advance(host, intent.progress().replayed_events, 1).unwrap();
    }
    intent.finish(host).unwrap();
    let mut task = host.prepare_learned_step_completion(host.revision(), n.actor_revision, n.position).unwrap();
    while task.progress().status == FileLearnedStepPreparationStatus::Replaying {
        task.advance(host, task.progress().replayed_events, 1).unwrap();
    }
    task.finish(host).unwrap().unwrap();
}
fn same(warm: &FileOversight, cold: &FileOversight, config: &FileLearnedConfig) {
    assert_eq!(warm.inspect(), cold.inspect());
    assert_eq!(warm.learned_generation_inspection().unwrap(), cold.learned_generation_inspection().unwrap());
    assert_eq!(disk(warm), journal::encode(&warm.profile, warm.store.identity(), &cold.events).unwrap());
    let replay = read_machine(warm, config);
    assert_eq!(replay.snapshot(warm.events.len()), warm.inspect());
    assert_eq!(replay.broker.hosted_learned_generation().unwrap(), warm.learned_generation_inspection().unwrap().numerical);
    assert_eq!(replay.broker.retained_actor_state().cache(), warm.machine.broker.retained_actor_state().cache());
    assert_eq!(replay.broker.retained_actor_state().sampler(), warm.machine.broker.retained_actor_state().sampler());
    assert_eq!(prefix(warm), Some(warm.events.len() - 1));
    assert_eq!(prefix(cold), None);
}

#[test]
fn ordinary_review_approval_dispatch_and_publication_reuse_one_event_tails_with_exact_cold_parity() {
    let root = Directory::new(); let other = Directory::new(); let config = config(false, 3);
    let (mut warm, reviewer) = owner(&root, &config); let (mut cold, cold_reviewer) = owner(&other, &config);
    step(&mut warm).unwrap(); cold_step(&mut cold); same(&warm, &cold, &config);
    for tick in 2..4 {
        let before = warm.inspect();
        warm.observe_time(warm.revision(), ElapsedTick(tick)).unwrap();
        cold.observe_time(cold.revision(), ElapsedTick(tick)).unwrap();
        assert_eq!(warm.learned_replay.as_ref().unwrap().candidate.snapshot(before.revision as usize), before);
        same(&warm, &cold, &config);
    }
    // This is the existing real commit/reveal fixture, not a manufactured permit.
    let (action, input, automatic, request) = prepared(&mut warm);
    let (cold_action, cold_input, cold_automatic, cold_request) = prepared(&mut cold);
    same(&warm, &cold, &config);
    let revision = warm.revision(); let human = reviewer.approve(&mut warm, revision, &request).unwrap();
    let revision = cold.revision(); let cold_human = cold_reviewer.approve(&mut cold, revision, &cold_request).unwrap();
    same(&warm, &cold, &config);
    warm.dispatch(warm.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    cold.dispatch(cold.revision(), &cold_automatic, &cold_human, &cold_action, &cold_input, snapshot()).unwrap();
    same(&warm, &cold, &config); assert_eq!(warm.inspect().executions, 0);
    let actual = warm.publish_checked(warm.revision(), 1, Some(&input), snapshot(), ElapsedTick(4)).unwrap();
    let expected = cold.publish_checked(cold.revision(), 1, Some(&cold_input), snapshot(), ElapsedTick(4)).unwrap();
    assert_eq!(actual, expected); assert_eq!(actual.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    same(&warm, &cold, &config);
    assert_eq!(warm.reconcile(warm.revision(), 1).unwrap(), cold.reconcile(cold.revision(), 1).unwrap());
    same(&warm, &cold, &config); assert_eq!(warm.inspect().control.ledger.charged, 16);
}

#[test]
fn cache_clear_and_recovery_leave_no_implicit_second_replay_owner() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    assert_eq!(prefix(&host), None); step(&mut host).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let before = host.inspect(); let bytes = disk(&host);
    assert_eq!(prefix(&host), Some(host.events.len() - 1)); host.clear_learned_replay_cache();
    assert_eq!(prefix(&host), None); assert_eq!(host.inspect(), before); assert_eq!(disk(&host), bytes);
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap(); assert_eq!(prefix(&host), None);
    drop(host);
    // Cooperative recovery's retired placeholder must NEVER seed a cache.
    let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), &config).unwrap();
    while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
        recovery.advance(recovery.progress().replayed_events, 1).unwrap();
    }
    let (mut host, _) = recovery.finish().unwrap();
    assert_eq!(prefix(&host), None); assert!(host.learned_generation_inspection().unwrap().paused);
    host.observe_time(host.revision(), ElapsedTick(4)).unwrap(); assert_eq!(prefix(&host), None);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(prefix(&host), None); step(&mut host).unwrap();
    assert_eq!(prefix(&host), Some(host.events.len() - 1));
    assert_eq!(read_machine(&host, &config).broker.hosted_learned_generation().unwrap(),
        host.learned_generation_inspection().unwrap().numerical);
    let plain_root = Directory::new(); let (mut plain, _) = FileOversight::create(plain_root.store(), profile()).unwrap();
    plain.observe_time(plain.revision(), ElapsedTick(1)).unwrap(); assert_eq!(prefix(&plain), None);
}

#[test]
fn preflight_refusal_preserves_cache_but_failed_new_transition_discards_only_the_candidate() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let before = host.inspect(); let bytes = disk(&host); let saved = prefix(&host);
    assert_eq!(host.observe_time(host.revision() - 1, ElapsedTick(2)), Err(Error::Stale.into()));
    assert_eq!(prefix(&host), saved);
    host.source_interrupted = true;
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    assert_eq!(prefix(&host), saved); host.source_interrupted = false;
    // The real clock reducer refuses a backward tick after replaying the tail.
    let event = Event::Core(BaseEvent::Time(ElapsedTick(0)));
    let expected = Machine::replay(&host.profile, &host.events).and_then(|mut machine| machine.apply(&event)).err();
    assert!(expected.is_some());
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(0)).err(), expected.map(JournalError::from));
    assert_eq!(prefix(&host), None); assert_eq!(host.inspect(), before); assert_eq!(disk(&host), bytes);
    assert!(host.storage_failure().is_none());
    // Explicit retry with a valid clock is cold and retains the original state.
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(prefix(&host), None); assert_eq!(host.inspect().executions, 0);
    assert_eq!(read_machine(&host, &config).snapshot(host.events.len()), host.inspect());
}

#[test]
fn invalid_cached_binding_and_corrupt_unverified_witness_never_fall_back_inside_a_control_call() {
    for foreign in [false, true] {
        let root = Directory::new(); let other = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config); let (mut other, _) = owner(&other, &config);
        step(&mut host).unwrap(); step(&mut other).unwrap();
        let original = host.events.last().unwrap().clone();
        if foreign {
            // Controlled private-custody violation: equal-looking different owner.
            host.learned_replay = other.learned_replay.take();
        } else {
            // Mutate the one UNVERIFIED tail event, not the already trusted prefix.
            let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = host.events.last_mut().unwrap()
                else { panic!("test needs the original witnessed numerical tail"); };
            let mut altered = witness.to_vec(); altered[0] ^= 1; *witness = altered.into();
        }
        let before = host.inspect(); let bytes = disk(&host);
        assert_eq!(host.observe_time(host.revision(), ElapsedTick(2)), Err(Error::Binding.into()));
        assert_eq!(prefix(&host), None); assert_eq!(host.inspect(), before); assert_eq!(disk(&host), bytes);
        assert!(host.storage_failure().is_none());
        *host.events.last_mut().unwrap() = original;
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(read_machine(&host, &config).snapshot(host.events.len()), host.inspect());
    }
}

#[test]
fn every_control_write_barrier_withholds_candidate_and_preserves_original_recovery_outcome() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let before = host.inspect();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        host.store.fail_once(barrier);
        assert!(matches!(host.observe_time(host.revision(), ElapsedTick(2)),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(prefix(&host), None); assert_eq!(host.inspect(), before);
        assert!(host.storage_failure().is_some());
        assert_eq!(host.observe_time(host.revision(), ElapsedTick(3)), Err(JournalError::Unavailable));
        drop(host);
        let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(prefix(&recovered), None); assert_eq!(recovered.inspect().executions, 0);
        assert_eq!(recovered.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(recovered.learned_generation_inspection().unwrap().paused);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert!(matches!(recovered.inspect().control.ledger.elapsed, Some(ElapsedTick(1) | ElapsedTick(2))));
    }
}

#[test]
fn warm_fences_and_receipt_settlement_do_not_resend_or_refund_unknown_effects() {
    for executed in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
        step(&mut host).unwrap(); let (action, input, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
        if executed { host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap(); }
        host.fence(host.revision()).unwrap(); assert_eq!(prefix(&host), Some(host.events.len() - 1));
        assert!(host.learned_generation_inspection().unwrap().paused);
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        let outcome = host.reconcile(host.revision(), 1).unwrap();
        if executed {
            assert_eq!(outcome, Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
        } else { assert_eq!(outcome, Reconciliation::AwaitingResolution); }
        assert_eq!(host.inspect().executions, u64::from(executed));
        assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(read_machine(&host, &config).snapshot(host.events.len()), host.inspect());
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
        assert_eq!(host.inspect().executions, u64::from(executed));
    }
}

#[test]
fn replay_reconstruction_does_not_clear_an_original_pre_application_loss_latch() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let before = host.inspect(); let bytes = disk(&host);
    // Exercise the internal phase used after mediation/consistency loss preflight.
    // Public entry points still reject a pre-existing fault before this helper.
    host.fault = Some(JournalFailure { operation: JournalIo::Stage,
        kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
    let (candidate, retain) = host.replay_control_candidate().unwrap();
    assert!(retain); assert_eq!(candidate.snapshot(host.events.len()), before);
    drop(candidate); assert!(host.storage_failure().is_some());
    assert_eq!(disk(&host), bytes); assert_eq!(host.inspect(), before);
    assert_eq!(host.observe_time(host.revision(), ElapsedTick(2)), Err(JournalError::Unavailable));
    let event = Event::Core(BaseEvent::Time(ElapsedTick(2)));
    assert!(host.transact(host.revision(), event).is_err());
}
