//! Original inference and real canonical files. No trained detector, process
//! independence, hardware durability or executed-result claim is made here.
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;

fn begin_next(host: &mut FileOversight) -> LearnedStepIntent {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let intent = LearnedStepIntent { actor_revision: n.actor_revision, position: n.position };
    host.begin_learned_step(host.revision(), intent.actor_revision, intent.position).unwrap();
    intent
}

fn recovered(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, LearnedStepIntent) {
    let (mut host, _) = owner(root, config);
    step(&mut host).unwrap();
    let intent = begin_next(&mut host);
    drop(host);
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    (host, intent)
}

fn resume(host: &mut FileOversight, intent: LearnedStepIntent)
    -> Result<Result<Rc<GenerationEvent>, Error>, JournalError>
{
    host.resume_pending_learned_step_at(host.revision(), intent.actor_revision, intent.position, ElapsedTick(2))
}

#[test]
fn atomic_resume_matches_the_three_original_public_operations_and_record_bytes() {
    let root = Directory::new(); let manual_root = Directory::new();
    let config = config(false, 3);
    let (mut host, intent) = recovered(&root, &config);
    let (mut manual, manual_intent) = recovered(&manual_root, &config);
    assert_eq!(intent, manual_intent);
    let before = host.learned_generation_inspection().unwrap();
    let epoch = host.inspect().control.ledger.epoch;
    manual.observe_time(manual.revision(), ElapsedTick(2)).unwrap();
    manual.resume_learned_generation(manual.revision(), intent.actor_revision, intent.position).unwrap();
    let expected = manual.complete_learned_step(manual.revision(), intent.actor_revision, intent.position).unwrap().unwrap();
    let actual = resume(&mut host, intent).unwrap().unwrap();
    assert_eq!(actual.status(), expected.status());
    assert_eq!(actual.sample(), expected.sample());
    assert_eq!(host.learned_generation_inspection().unwrap(), manual.learned_generation_inspection().unwrap());
    // Only the bootstrap's directory identity differs. Encode the original
    // manual history at this owner's identity to compare ALL record bytes.
    let expected_bytes = journal::encode(&host.profile, host.store.identity(), &manual.events).unwrap();
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), expected_bytes);
    let after = host.learned_generation_inspection().unwrap();
    assert_eq!(after.journal_revision, before.journal_revision + 3);
    assert!(!after.paused && after.pending.is_none() && host.clock_ready());
    assert_eq!(after.numerical.position, before.numerical.position + 1);
    assert_eq!(after.numerical.sampled_draws, before.numerical.sampled_draws + 1);
    assert_eq!(host.inspect().control.ledger.epoch, epoch);
    assert_eq!(host.inspect().executions, 0);
    step(&mut host).unwrap(); step(&mut manual).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap(), manual.learned_generation_inspection().unwrap());
}

#[test]
fn wrong_predecessor_or_clock_cannot_commit_a_lone_time_or_resume() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, intent) = recovered(&root, &config);
    let before = host.learned_generation_inspection().unwrap();
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    assert_eq!(host.resume_pending_learned_step_at(host.revision() - 1,
        intent.actor_revision, intent.position, ElapsedTick(2)).err(), Some(JournalError::Contract(Error::Stale)));
    for wrong in [LearnedStepIntent { actor_revision: intent.actor_revision + 1, ..intent },
        LearnedStepIntent { position: intent.position + 1, ..intent }] {
        assert_eq!(resume(&mut host, wrong).err(), Some(JournalError::Contract(Error::Binding)));
    }
    assert!(host.resume_pending_learned_step_at(host.revision(), intent.actor_revision,
        intent.position, ElapsedTick(0)).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
    assert!(!host.clock_ready() && host.storage_failure().is_none());
    resume(&mut host, intent).unwrap().unwrap();
}

#[test]
fn a_nonpaused_or_missing_intent_never_becomes_a_new_step() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    let n = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(host.resume_pending_learned_step_at(host.revision(), n.actor_revision,
        n.position, ElapsedTick(2)).err(), Some(JournalError::Contract(Error::Incomplete)));
    step(&mut host).unwrap();
    let intent = begin_next(&mut host);
    let before = host.learned_generation_inspection().unwrap();
    assert_eq!(resume(&mut host, intent).err(), Some(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    host.complete_learned_step(host.revision(), intent.actor_revision, intent.position).unwrap().unwrap();
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let before = host.learned_generation_inspection().unwrap();
    assert!(before.paused && before.pending.is_none());
    assert_eq!(resume(&mut host, intent).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
}

#[test]
fn all_three_ordinary_slots_must_fit_without_using_the_terminal_tail() {
    for reserve in [false, true] {
        for slots in [2, 3] {
            let root = Directory::new(); let config = config(false, 1);
            let mut profile = profile();
            let tail = if reserve { RecoveryReserve::terminal().events } else { 0 };
            // time + optional reserve + enable + prompt intent/outcome + next
            // intent + reopening fence, followed by the available ordinary slots.
            profile.delivery.limits.events = 6 + usize::from(reserve) + tail + slots;
            let (mut host, _) = FileOversight::create(root.store(), profile.clone()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            if reserve { host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap(); }
            host.enable_learned_generation(host.revision(), config.clone()).unwrap();
            step(&mut host).unwrap(); let intent = begin_next(&mut host);
            drop(host);
            let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile, &config).unwrap();
            assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, slots);
            let before = host.learned_generation_inspection().unwrap();
            let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
            if slots == 2 {
                assert_eq!(resume(&mut host, intent).err(), Some(JournalError::Contract(Error::Limit)));
                assert_eq!(host.learned_generation_inspection().unwrap(), before);
                assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
                assert!(host.storage_failure().is_none());
            } else {
                resume(&mut host, intent).unwrap().unwrap();
                assert!(host.learned_generation_inspection().unwrap().pending.is_none());
                assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
                assert_eq!(host.journal_capacity().unwrap().remaining().events, tail);
            }
        }
    }
}

#[test]
fn every_storage_barrier_exposes_only_an_old_intent_or_a_complete_new_outcome() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 3);
        let (mut host, intent) = recovered(&root, &config);
        let before = host.learned_generation_inspection().unwrap();
        let snapshot = host.inspect();
        let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        host.store.fail_once(barrier);
        let error = resume(&mut host, intent).err().unwrap();
        let JournalError::Io(failure) = error else { panic!("expected injected storage error"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible,
            matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(host.revision(), before.journal_revision);
        assert_eq!(host.inspect(), snapshot);
        assert_eq!(resume(&mut host, intent).err(), Some(JournalError::Unavailable));
        let visible = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
        if barrier == JournalIo::DirectorySync { assert_ne!(visible, bytes); }
        else { assert_eq!(visible, bytes); }
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let after = host.learned_generation_inspection().unwrap();
        assert!(after.paused && !host.clock_ready());
        if barrier == JournalIo::DirectorySync {
            assert!(after.pending.is_none());
            assert_eq!(after.numerical.position, before.numerical.position + 1);
            assert_eq!(resume(&mut host, intent).err(), Some(JournalError::Contract(Error::Incomplete)));
        } else {
            assert_eq!(after.pending, before.pending);
            assert_eq!(after.numerical, before.numerical);
            resume(&mut host, intent).unwrap().unwrap();
        }
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn original_alarm_is_acknowledged_without_releasing_the_held_sample() {
    let root = Directory::new(); let config = config(true, 1);
    let (mut host, intent) = recovered(&root, &config);
    let before = host.learned_generation_inspection().unwrap();
    let held = resume(&mut host, intent).unwrap().unwrap();
    assert_eq!(held.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert!(held.accepted().is_none() && held.sample().is_none());
    let after = host.learned_generation_inspection().unwrap();
    assert!(after.pending.is_none());
    assert_eq!(after.numerical.position, before.numerical.position);
    assert_eq!(after.numerical.sampled_draws, before.numerical.sampled_draws);
    assert_eq!(after.numerical.work.admitted_tokens, before.numerical.work.admitted_tokens + 1);
    assert_eq!(after.journal_revision, before.journal_revision + 3);
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, after.numerical);
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    assert!(host.resume_learned_generation(host.revision(), intent.actor_revision, intent.position).is_err());
}

#[test]
fn original_numerical_failure_is_a_persisted_inner_error_not_a_retryable_old_prefix() {
    use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;
    let model = model(); let mut source = source(&model, false, 1);
    let mut control = model.observed_learned_generation(source.clone()).unwrap();
    control.advance(0).unwrap();
    source.telemetry.source_check_values = control.telemetry_work().source_check_values;
    let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    let root = Directory::new(); let (mut host, intent) = recovered(&root, &config);
    let before = host.revision();
    assert_eq!(resume(&mut host, intent).unwrap().err(), Some(Error::Limit));
    let after = host.learned_generation_inspection().unwrap();
    assert_eq!(after.journal_revision, before + 3);
    assert!(after.pending.is_none() && host.storage_failure().is_none());
    assert_eq!(after.numerical.status, GenerationStatus::Failed(Error::Limit));
    drop(host);
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, after.numerical);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn resuming_numerics_does_not_refund_an_unknown_effect_or_revive_either_old_key() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap();
    let (action, input, automatic, request) = prepared(&mut host);
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    let intent = begin_next(&mut host);
    let old_epoch = host.inspect().control.ledger.epoch;
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let epoch = host.inspect().control.ledger.epoch;
    assert!(epoch > old_epoch);
    resume(&mut host, intent).unwrap().unwrap();
    assert_eq!(host.inspect().control.ledger.epoch, epoch);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).is_err());
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 0);
}
