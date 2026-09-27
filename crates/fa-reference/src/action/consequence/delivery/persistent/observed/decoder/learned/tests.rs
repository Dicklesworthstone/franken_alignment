//! Original numerical/control engines and real local journal replacements.
//! Synthetic coefficients and helper ballots are not a trained-detector claim.
mod fixture;
mod recovery;
mod sidecar;
use fixture::*;
use super::*;
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{GenerationSpec, GenerationStatus, GenerationStop};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::Reconciliation;
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;
use crate::action::consequence::oversight::learned_source::LearnedAvailability;

#[test]
fn every_acknowledged_step_matches_the_original_generator_and_never_publishes_by_itself() {
    let root = Directory::new();
    let model = model(); let source = source(&model, false, 3);
    let config = FileLearnedConfig::new(model.clone(), source.clone(), LearnedDecoderBindingLimits::default()).unwrap();
    let mut original = model.monitored_generation_with_telemetry(source.stream, source.evaluation_origin,
        source.spec, source.policy, source.budget, source.telemetry).unwrap();
    let (mut host, _) = owner(&root, &config);
    assert!(host.decoder_required() && host.learned_generation_required() && host.publication_guard_required());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.availability, LearnedAvailability::Empty);
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    while original.status().is_active() {
        let before = host.revision();
        let expected = original.advance(original.position()).unwrap();
        let actual = step(&mut host).unwrap();
        assert_eq!(host.revision(), before + 2);
        assert_eq!(actual.status(), expected.status());
        assert_eq!(actual.sample(), expected.sample());
        let view = host.learned_generation_inspection().unwrap();
        assert!(view.pending.is_none());
        assert_eq!(view.numerical.status, original.status());
        assert_eq!(view.numerical.work, original.work());
        assert_eq!(view.numerical.telemetry, original.telemetry_work());
        let actor = host.machine.broker.retained_actor_state();
        assert_eq!(actor.tokens(), original.accepted_tokens());
        assert_eq!(actor.cache(), original.accepted_cache_image().unwrap().encode().unwrap());
        assert_eq!(actor.sampler(), original.sampler_state().encode());
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
    assert_eq!(original.status(), GenerationStatus::Finished(GenerationStop::TokenLimit));
    let history = read_machine(&host, &config);
    assert_eq!(history.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
    assert_eq!(step(&mut host).err(), Some(JournalError::Contract(Error::WrongState)));
}

#[test]
fn recovered_generation_is_paused_then_continues_original_sampler_and_spent_budgets() {
    let root = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); step(&mut host).unwrap();
    let before = host.learned_generation_inspection().unwrap();
    let epoch = host.inspect().control.ledger.epoch;
    drop(host);
    // A generic decoder cannot interpret disk bytes as bootstrap authority.
    assert!(FileOversight::open(root.store(), profile()).is_err());
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let recovered = host.learned_generation_inspection().unwrap();
    assert!(recovered.paused && !host.clock_ready());
    assert_eq!(recovered.numerical, before.numerical);
    assert!(host.inspect().control.ledger.epoch > epoch);
    assert_eq!(step(&mut host).err(), Some(JournalError::Contract(Error::Incomplete)));
    let numerical = &recovered.numerical;
    assert!(host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before.numerical);
    step(&mut host).unwrap();
    let after = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(after.position, before.numerical.position + 1);
    assert_eq!(after.sampled_draws, before.numerical.sampled_draws + 1);
    assert!(after.telemetry.source_check_values > before.numerical.telemetry.source_check_values);
}

#[test]
fn interrupted_intent_survives_recovery_and_resume_cannot_abandon_it() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    let before = host.learned_generation_inspection().unwrap();
    assert_eq!(before.pending, Some(LearnedStepIntent { actor_revision: numerical.actor_revision, position: numerical.position }));
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    assert!(host.begin_learned_step(host.revision(), numerical.actor_revision, numerical.position + 1).is_err());
    assert!(host.complete_learned_step(host.revision(), numerical.actor_revision, numerical.position + 1).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    host.complete_learned_step(host.revision(), numerical.actor_revision, numerical.position).unwrap().unwrap();
    assert!(host.learned_generation_inspection().unwrap().pending.is_none());
    host.propose(host.revision(), 1, action_spec(&host), snapshot()).unwrap();
}

#[test]
fn stale_calls_and_unintended_completion_do_not_compute_or_replace_history() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    let before = host.learned_generation_inspection().unwrap();
    let n = &before.numerical;
    assert_eq!(host.complete_learned_step(host.revision(), n.actor_revision, n.position).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.advance_learned_generation(host.revision() - 1, n.actor_revision, n.position).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(host.begin_learned_step(host.revision(), n.actor_revision, n.position + 1).err(), Some(JournalError::Contract(Error::Stale)));
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    step(&mut host).unwrap();
    assert!(host.advance_learned_generation(before.journal_revision, n.actor_revision, n.position).is_err());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.admitted_tokens, 1);
}

#[test]
fn actual_monitor_hold_is_durable_and_cannot_resume_the_older_quiet_prefix() {
    let root = Directory::new(); let config = config(true, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    let accepted = host.learned_generation_inspection().unwrap().numerical;
    let held = step(&mut host).unwrap();
    assert_eq!(held.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    assert!(held.accepted().is_none() && held.sample().is_none());
    let stopped = host.learned_generation_inspection().unwrap();
    assert_eq!(stopped.numerical.position, accepted.position);
    assert_eq!(stopped.numerical.sampled_draws, accepted.sampled_draws);
    assert_eq!(stopped.numerical.work.admitted_tokens, accepted.work.admitted_tokens + 1);
    assert!(stopped.pending.is_none());
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, stopped.numerical);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.resume_learned_generation(host.revision(), accepted.actor_revision, accepted.position).is_err());
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn aggregate_telemetry_exhaustion_is_replayed_as_failure_not_refilled_by_restart() {
    let model = model(); let mut source = source(&model, false, 1);
    let mut control = model.observed_learned_generation(source.clone()).unwrap();
    control.advance(0).unwrap();
    source.telemetry.source_check_values = control.telemetry_work().source_check_values;
    let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    let root = Directory::new(); let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    let result = host.advance_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(result.err(), Some(Error::Limit));
    let failed = host.learned_generation_inspection().unwrap();
    assert_eq!(failed.numerical.status, GenerationStatus::Failed(Error::Limit));
    assert_eq!(failed.numerical.availability, LearnedAvailability::Failed);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, failed.numerical);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.resume_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
}

#[test]
fn exact_independent_recipe_is_required_even_when_labels_match_and_prefix_is_empty() {
    let root = Directory::new(); let expected = config(false, 1);
    let (host, _) = owner(&root, &expected);
    let original_bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    drop(host);
    let model = model();
    let mut changed = source(&model, false, 1);
    changed.telemetry.source_check_values -= 1;
    let changed_budget = FileLearnedConfig::new(model.clone(), changed, LearnedDecoderBindingLimits::default()).unwrap();
    let mut changed = source(&model, false, 1);
    changed.monitor_generation += 1;
    let changed_generation = FileLearnedConfig::new(model.clone(), changed, LearnedDecoderBindingLimits::default()).unwrap();
    let mut changed = source(&model, false, 1);
    let mut sampling = changed.spec.sampling().clone(); sampling.seed += 1;
    changed.spec = GenerationSpec::new(changed.spec.prompt().to_vec(), changed.spec.max_new_tokens(),
        changed.spec.stop_tokens().clone(), sampling).unwrap();
    let changed_seed = FileLearnedConfig::new(model, changed, LearnedDecoderBindingLimits::default()).unwrap();
    let other_model = model_with_output(0.125);
    let changed_weights = FileLearnedConfig::new(other_model.clone(), source(&other_model, false, 1),
        LearnedDecoderBindingLimits::default()).unwrap();
    for wrong in [config(true, 1), config(false, 3), changed_budget, changed_generation, changed_seed, changed_weights] {
        assert_ne!(wrong, expected);
        assert_eq!(FileOversight::open_with_learned_generation(root.store(), profile(), &wrong).err(), Some(JournalError::Contract(Error::Binding)));
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original_bytes);
    }
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &expected).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 0);
}

#[test]
fn corrupt_expected_witness_and_completion_without_intent_cannot_reconstruct_a_machine() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap();
    let original = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &original).unwrap();
    bind_history(&mut events, &config).unwrap();
    Machine::replay(&host.profile, &events).unwrap();
    let index = events.iter().position(|event| matches!(event, Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { .. })))).unwrap();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = &events[index] else { unreachable!() };
    let original_witness = witness.to_vec();
    for offset in [0, 9, 20, original_witness.len() - 1] {
        let mut altered = events.clone();
        let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) = &mut altered[index] else { unreachable!() };
        let mut bytes = original_witness.clone(); bytes[offset] ^= 1; *witness = bytes.into();
        // Well-framed journal bytes are not numerical proof.
        let bytes = journal::encode(&host.profile, host.store.identity(), &altered).unwrap();
        let mut decoded = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
        bind_history(&mut decoded, &config).unwrap();
        assert_eq!(Machine::replay(&host.profile, &decoded).err(), Some(Error::Binding));
    }
    events.retain(|event| !matches!(event, Event::Decoder(DecoderEvent::Learned(LearnedEvent::Begin(_)))));
    assert_eq!(Machine::replay(&host.profile, &events).err(), Some(Error::Incomplete));
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), original);
}

#[test]
fn full_original_congress_human_and_final_publication_gates_still_control_quiet_generation() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap();
    let (action, inputs, automatic, request) = prepared(&mut host);
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_err());
    let result = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).is_err());
    assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().executions, 1);
}

#[test]
fn pending_inference_blocks_final_publication_but_does_not_refund_unknown_effects() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap();
    let (action, inputs, automatic, request) = prepared(&mut host);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 0);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.seal_unexecuted(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(host.learned_generation_inspection().unwrap().pending.is_some());
}

#[test]
fn a_new_quiet_prefix_cannot_reuse_the_previous_publication_basis() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap();
    let (action, inputs, automatic, request) = prepared(&mut host);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    step(&mut host).unwrap();
    let result = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::NotExecuted { .. }));
    assert_eq!(host.inspect().executions, 0);
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn storage_failure_after_intent_returns_no_candidate_and_recovers_only_actual_durable_state() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(true, 1);
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
        host.store.fail_once(barrier);
        let error = host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected selected storage failure"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.learned_generation_inspection().err(), Some(JournalError::Unavailable));
        assert!(host.complete_learned_step(host.revision(), n.actor_revision, n.position).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let recovered = host.learned_generation_inspection().unwrap();
        if barrier == JournalIo::DirectorySync {
            assert!(recovered.pending.is_none());
            assert_eq!(recovered.numerical.status, GenerationStatus::Held(MonitorOutcome::Alarm));
        } else {
            assert!(recovered.pending.is_some());
            assert_eq!(recovered.numerical, n);
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
            assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
            let held = host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
            assert_eq!(held.status(), GenerationStatus::Held(MonitorOutcome::Alarm));
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn event_capacity_admission_leaves_room_for_one_completion_and_refuses_without_inference() {
    for cap in [3, 4] {
        let root = Directory::new(); let config = config(false, 1);
        let mut p = profile(); p.delivery.limits.events = cap;
        let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.enable_learned_generation(host.revision(), config).unwrap();
        let before = host.learned_generation_inspection().unwrap();
        let result = host.begin_learned_step(host.revision(), before.numerical.actor_revision, 0);
        if cap == 3 {
            assert_eq!(result.err(), Some(JournalError::Contract(Error::Limit)));
            assert_eq!(host.learned_generation_inspection().unwrap(), before);
        } else {
            result.unwrap();
            host.complete_learned_step(host.revision(), before.numerical.actor_revision, 0).unwrap().unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 1);
            assert_eq!(host.revision(), 4);
        }
    }
}
