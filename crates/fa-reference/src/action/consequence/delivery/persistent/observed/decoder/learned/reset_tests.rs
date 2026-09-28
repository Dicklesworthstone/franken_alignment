//! Real original numerical/review engines and canonical storage failure cuts.
use super::*;
use super::super::checkpoint::FileLearnedCheckpoint;
use crate::action::consequence::activation::tensor::kv::decoder::monitoring::restart::KvRestartBudget;
use crate::action::consequence::activation::tensor::kv::model::MAX_MODEL_KV_VALUES;
use crate::action::consequence::delivery::{EndpointOutcome, StopRequest};
use crate::action::consequence::delivery::persistent::{Reconciliation, observed::containment::FileResetRequest};
use crate::action::consequence::gate::ReviewBinding;
use crate::action::consequence::oversight::{decoder_host::HostedStopPolicy,
    decoder_monitoring::LearnedDecoderBindingLimits, learned_source::LearnedAvailability};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;
use crate::action::ActionState;

fn budget(host: &FileOversight) -> KvRestartBudget {
    KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() }
}
fn control(host: &FileOversight, operation: u64) -> FileResetRequest {
    let state = host.inspect();
    FileResetRequest { operation, expected_control_sequence: state.control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: state.control.ledger.epoch,
        binding: ReviewBinding { round: operation, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![state.target] }
}
fn reset(host: &mut FileOversight, saved: &FileLearnedCheckpoint, operation: u64)
    -> Result<crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedResetReceipt, Error>
{
    host.reset_learned_checkpoint(host.revision(), saved, control(host, operation), budget(host)).unwrap()
}
fn same(actual: &GenerationEvent, expected: &GenerationEvent) {
    assert_eq!(actual.sample(), expected.sample());
    assert_eq!(actual.accepted().unwrap().logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        expected.accepted().unwrap().logits.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
}
fn resume(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
}

#[test]
fn original_sampled_continuation_and_conserved_reset_state_survive_disk_reconstruction() {
    let root = Directory::new(); let model = model(); let source = source(&model, false, 3);
    let config = FileLearnedConfig::new(model.clone(), source.clone(), LearnedDecoderBindingLimits::default()).unwrap();
    let mut original = model.monitored_generation_with_telemetry(source.stream, source.evaluation_origin,
        source.spec, source.policy, source.budget, source.telemetry).unwrap();
    let (mut host, _) = owner(&root, &config);
    same(&step(&mut host).unwrap(), &original.advance(0).unwrap());
    let saved = capture(&mut host, 1);
    let historical = host.learned_generation_inspection().unwrap().numerical;
    let expected = original.advance(1).unwrap();
    same(&step(&mut host).unwrap(), &expected);
    let before = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    let receipt = reset(&mut host, &saved, 900).unwrap();
    assert_eq!(host.revision(), revision + 2);
    assert!(receipt.control.restored); assert_eq!(receipt.control.incident_count, 1);
    assert_eq!(receipt.resumed_stream, Some(22));
    assert_eq!(receipt.restart.historical_work(), historical.work);
    let restored = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(restored.work, historical.work);
    assert_eq!(restored.cumulative_work, before.cumulative_work);
    assert!(restored.cumulative_telemetry.source_check_values > before.cumulative_telemetry.source_check_values);
    assert_eq!(restored.availability, LearnedAvailability::Empty);
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    let event = step(&mut host).unwrap(); same(&event, &expected);
    assert!(event.audit().source().descriptor().layers().values().all(|layer| layer.stream == 22));
    let after = host.learned_generation_inspection().unwrap().numerical;
    let usage = host.learned_recovery_usage().unwrap();
    let replay = read_machine(&host, &config);
    assert_eq!(replay.broker.hosted_learned_generation().unwrap(), after);
    assert_eq!(replay.broker.hosted_learned_recovery_usage().unwrap(), usage);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, after);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.learned_reset_result(900).unwrap().unwrap().control, receipt.control);
    assert!(host.check_learned_checkpoint(&saved).is_err());
    resume(&mut host);
    same(&step(&mut host).unwrap(), &original.advance(2).unwrap());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn operation_retries_are_historical_and_changed_or_foreign_instructions_cannot_repeat_reset() {
    let root = Directory::new(); let other_root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config); let (mut other, _) = owner(&other_root, &config);
    step(&mut host).unwrap(); step(&mut other).unwrap();
    let saved = capture(&mut host, 1); let foreign = capture(&mut other, 1);
    let request = control(&host, 900); let allowance = budget(&host);
    assert!(host.reset_learned_checkpoint(host.revision(), &foreign, request.clone(), allowance).is_err());
    let before = host.revision();
    let result = host.reset_learned_checkpoint(before, &saved, request.clone(), allowance).unwrap().unwrap();
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let usage = host.learned_recovery_usage().unwrap();
    let state = host.learned_generation_inspection().unwrap();
    for revision in [0, before, host.revision()] {
        let retry = host.reset_learned_checkpoint(revision, &saved, request.clone(), allowance).unwrap().unwrap();
        assert_eq!(retry.control, result.control);
        assert_eq!(host.complete_learned_reset(revision, 900).unwrap().unwrap().control, result.control);
    }
    let mut changed = request.clone(); changed.binding.evidence_root[0] ^= 1;
    assert_eq!(host.reset_learned_checkpoint(host.revision(), &saved, changed, allowance).err(),
        Some(JournalError::Contract(Error::Binding)));
    let mut changed_budget = allowance; changed_budget.audit.monitoring.refinements -= 1;
    assert_eq!(host.reset_learned_checkpoint(host.revision(), &saved, request.clone(), changed_budget).err(),
        Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
    assert_eq!(host.learned_generation_inspection().unwrap(), state);
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let recovered = host.learned_checkpoint(1).unwrap();
    let revision = host.revision();
    assert_eq!(host.reset_learned_checkpoint(0, &recovered, request, allowance).unwrap().unwrap().control, result.control);
    assert_eq!(host.revision(), revision); assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
}

#[test]
fn reset_needs_a_fresh_original_observation_and_both_keys_before_publication() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap(); let saved = capture(&mut host, 1); step(&mut host).unwrap();
    reset(&mut host, &saved, 900).unwrap();
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
    step(&mut host).unwrap();
    let (action, input, automatic, request) = prepared(&mut host);
    assert!(host.publish(host.revision(), 1).is_err());
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_err());
    let result = host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    drop(host);
    let retained = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(retained.executions, 1); assert_eq!(retained.payload, b"visible");
    assert_eq!(retained.control.ledger.charged, 16);
}

#[test]
fn old_keys_and_unknown_effect_liabilities_never_rewind_with_the_numerical_checkpoint() {
    for cut in 0..3 {
        let dispatched = cut > 0; let executed = cut == 2;
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, reviewer) = owner(&root, &config);
        step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        let (action, input, automatic, request) = prepared(&mut host);
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        if dispatched { host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap(); }
        if executed { host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap(); }
        let result = reset(&mut host, &saved, 900).unwrap();
        assert_eq!(result.control.refunded_units, if dispatched { 0 } else { 16 });
        assert_eq!(result.control.cancelled, if dispatched { Vec::new() } else { vec![1] });
        assert_eq!(host.inspect().control.ledger.charged, if dispatched { 16 } else { 0 });
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
        let revision = host.revision(); assert!(reviewer.approve(&mut host, revision, &request).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1], if dispatched { ActionState::Unknown } else { ActionState::Cancelled });
        assert_eq!(host.inspect().control.ledger.charged, if dispatched { 16 } else { 0 });
        if !dispatched {
            assert_eq!(host.inspect().control.ledger.available, 100);
        } else if executed {
            let outcome = host.reconcile(host.revision(), 1).unwrap();
            assert!(matches!(outcome, Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
            assert_eq!(host.inspect().control.ledger.charged, 16);
        } else {
            let outcome = host.reconcile(host.revision(), 1).unwrap();
            assert_eq!(outcome, Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            host.seal_unexecuted(host.revision(), 1).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, 0);
        }
        assert_eq!(host.inspect().executions, u64::from(executed));
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    }
}

#[test]
fn repeated_durable_rewinds_and_reopening_do_not_restore_spent_sampling_or_telemetry_allowance() {
    for telemetry in [false, true] {
        let root = Directory::new(); let model = model(); let mut source = source(&model, false, 1);
        if telemetry { source.telemetry.source_check_values = model.cache_profile().values_per_token() as u64 * 3; }
        else { source.budget.vocabulary_scores = 9; }
        let config = FileLearnedConfig::new(model, source, LearnedDecoderBindingLimits::default()).unwrap();
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        for operation in [900, 901] { step(&mut host).unwrap(); reset(&mut host, &saved, operation).unwrap(); }
        let usage = host.learned_recovery_usage().unwrap();
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        resume(&mut host);
        if !telemetry { step(&mut host).unwrap(); }
        let before = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(step(&mut host).err(), Some(JournalError::Contract(Error::Limit)));
        let failed = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(failed.status, GenerationStatus::Failed(Error::Limit));
        assert_eq!(failed.position, before.position); assert_eq!(failed.sampled_draws, before.sampled_draws);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(usage.restart_attempts, 2);
        assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
        let recovered = host.learned_checkpoint(1).unwrap();
        assert!(host.reset_learned_checkpoint(host.revision(), &recovered, control(&host, 902), budget(&host)).is_err());
    }
}

#[test]
fn original_incident_escalation_and_terminal_stop_are_not_cleared_by_durable_recovery() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    for incident in 1..=3 {
        step(&mut host).unwrap();
        let result = reset(&mut host, &saved, 899 + incident).unwrap();
        assert_eq!(result.control.incident_count, incident);
        assert_eq!(result.control.restored, incident < 3);
        assert_eq!(result.resumed_stream.is_some(), incident < 3);
    }
    assert!(host.inspect().control.suspended);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let saved = host.learned_checkpoint(1).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 3);
    assert!(host.reset_learned_checkpoint(host.revision(), &saved, control(&host, 903), budget(&host)).is_err());
    assert!(step(&mut host).is_err());
    assert!(!host.learned_reset_result(902).unwrap().unwrap().control.restored);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn pending_reset_is_a_durable_barrier_not_a_resumable_pre_recovery_instruction() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let (action, input, automatic, request) = prepared(&mut host);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    let command = control(&host, 900); let allowance = budget(&host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_reset(host.revision(), &saved, command.clone(), allowance).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
    assert!(!host.pending_learned_reset().unwrap().unwrap().interrupted);
    assert!(step(&mut host).is_err());
    assert!(host.propose(host.revision(), 2, action_spec(&host), snapshot()).is_err());
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).is_err());
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let recovered = host.learned_checkpoint(1).unwrap();
    let pending = host.pending_learned_reset().unwrap().unwrap();
    assert!(pending.interrupted); assert_eq!(pending.intent.control(), &command);
    assert_eq!(pending.intent.budget(), allowance);
    let before = host.revision();
    assert!(host.complete_learned_reset(before, 900).is_err());
    assert!(host.begin_learned_reset(before, &recovered, command, allowance).is_err());
    assert!(host.begin_learned_reset(before, &recovered, control(&host, 901), allowance).is_err());
    assert!(host.resume_learned_generation(before, n.actor_revision, n.position).is_err());
    assert!(step(&mut host).is_err()); assert_eq!(host.revision(), before);
    assert_eq!(host.learned_reset_result(900).err(), Some(JournalError::Contract(Error::Missing)));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.seal_unexecuted(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.pending_learned_reset().unwrap().unwrap(), pending);
    let control = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 700,
        expected_control_sequence: control.sequence, expected_authority_epoch: control.ledger.epoch }).unwrap();
    assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn storage_failures_expose_only_acknowledged_completion_or_an_irrevocable_interrupted_intent() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let saved = capture(&mut host, 1); step(&mut host).unwrap();
        let old = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_reset(host.revision(), &saved, control(&host, 900), budget(&host)).unwrap();
        host.store.fail_once(barrier);
        let error = host.complete_learned_reset(host.revision(), 900).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("selected canonical storage fault expected"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.learned_reset_result(900).err(), Some(JournalError::Unavailable));
        assert_eq!(host.pending_learned_reset().err(), Some(JournalError::Unavailable));
        assert!(host.complete_learned_reset(host.revision(), 900).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        if barrier == JournalIo::DirectorySync {
            assert!(host.pending_learned_reset().unwrap().is_none());
            let result = host.learned_reset_result(900).unwrap().unwrap();
            assert!(result.control.restored); assert_eq!(n.position, 1);
            assert_eq!(n.cumulative_work, old.cumulative_work);
            assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
            host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
            step(&mut host).unwrap();
        } else {
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
            assert_eq!(n, old); assert_eq!(host.actor_snapshot().unwrap().incident_count, 0);
            assert!(host.complete_learned_reset(host.revision(), 900).is_err());
            assert!(host.resume_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
            assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn original_audit_failure_is_recorded_and_cannot_retry_from_older_quiet_state() {
    let root = Directory::new(); let config = config(false, 1);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let saved = capture(&mut host, 1);
    let mut allowance = budget(&host); allowance.audit.monitoring.probe_coordinates = 0;
    let command = control(&host, 900);
    let result = host.reset_learned_checkpoint(host.revision(), &saved, command.clone(), allowance).unwrap();
    assert!(result.is_err()); assert!(host.pending_learned_reset().unwrap().is_none());
    let usage = host.learned_recovery_usage().unwrap(); assert_eq!(usage.restart_attempts, 1);
    assert!(host.learned_generation_inspection().unwrap().numerical.host_failure.is_some());
    assert_eq!(host.reset_learned_checkpoint(0, &saved, command, allowance).unwrap().err(), result.err());
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(step(&mut host).is_err());
    assert!(host.propose(host.revision(), 1, action_spec(&host), snapshot()).is_err());
}

#[test]
fn a_fresh_recovery_instruction_can_restore_a_held_owner_but_never_clear_a_fixed_stop() {
    for automatic in [false, true] {
        let root = Directory::new(); let base = config(true, 1);
        let config = if automatic { base.with_automatic_stop(HostedStopPolicy::new(1, 1, 700).unwrap()).unwrap() } else { base };
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let _saved = capture(&mut host, 1);
        assert!(step(&mut host).unwrap().accepted().is_none());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let saved = host.learned_checkpoint(1).unwrap();
        let result = host.reset_learned_checkpoint(host.revision(), &saved, control(&host, 900), budget(&host));
        if automatic {
            assert!(result.is_err()); assert!(host.learned_host_stop_incident().unwrap().is_some());
            assert!(host.pending_learned_reset().unwrap().is_none());
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        } else {
            assert!(result.unwrap().unwrap().control.restored);
            let n = host.learned_generation_inspection().unwrap();
            assert!(n.paused); assert_eq!(n.numerical.availability, LearnedAvailability::Empty);
            assert!(step(&mut host).is_err());
            host.resume_learned_generation(host.revision(), n.numerical.actor_revision, n.numerical.position).unwrap();
            assert!(step(&mut host).unwrap().accepted().is_none());
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn reset_capacity_and_stale_control_refuse_before_creating_an_intent() {
    for cap in [6, 7] {
        let root = Directory::new(); let config = config(false, 1);
        let mut p = profile(); p.delivery.limits.events = cap;
        let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.enable_learned_generation(host.revision(), config).unwrap();
        step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        let n = host.learned_generation_inspection().unwrap(); let request = control(&host, 900);
        for kind in 0..3 {
            let mut bad = request.clone();
            match kind { 0 => bad.expected_actor_revision += 1, 1 => bad.expected_authority_epoch += 1,
                _ => bad.expected_control_sequence += 1 }
            assert_eq!(host.begin_learned_reset(host.revision(), &saved, bad, budget(&host)).err(),
                Some(JournalError::Contract(Error::Stale)));
        }
        assert_eq!(host.learned_generation_inspection().unwrap(), n);
        let result = host.begin_learned_reset(host.revision(), &saved, request, budget(&host));
        if cap == 6 {
            assert_eq!(result.err(), Some(JournalError::Contract(Error::Limit)));
            assert!(host.pending_learned_reset().unwrap().is_none());
            assert_eq!(host.learned_generation_inspection().unwrap(), n);
        } else {
            result.unwrap(); host.complete_learned_reset(host.revision(), 900).unwrap().unwrap();
            assert_eq!(host.revision(), 7);
        }
    }
}

#[test]
fn valid_journal_framing_cannot_replace_reset_witness_or_remove_its_write_ahead_intent() {
    let root = Directory::new(); let config = config(false, 3);
    let (mut host, _) = owner(&root, &config);
    step(&mut host).unwrap(); let saved = capture(&mut host, 1); step(&mut host).unwrap();
    reset(&mut host, &saved, 900).unwrap();
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    bind_history(&mut events, &config).unwrap();
    Machine::replay(&host.profile, &events).unwrap();
    let index = events.iter().position(|event| matches!(event,
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(CheckpointEvent::Reset { .. }))))).unwrap();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
        CheckpointEvent::Reset { witness, .. }))) = &events[index] else { unreachable!() };
    let original = witness.to_vec();
    for offset in [0, 8, original.len() / 2, original.len() - 1] {
        let mut altered = events.clone(); let mut changed = original.clone(); changed[offset] ^= 1;
        let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
            CheckpointEvent::Reset { witness, .. }))) = &mut altered[index] else { unreachable!() };
        *witness = changed.into();
        let encoded = journal::encode(&host.profile, host.store.identity(), &altered).unwrap();
        let mut decoded = journal::decode(&host.profile, host.store.identity(), &encoded).unwrap();
        bind_history(&mut decoded, &config).unwrap();
        assert_eq!(Machine::replay(&host.profile, &decoded).err(), Some(Error::Binding));
    }
    // Changing a valid requested ceiling is detected even when the same quiet
    // source could finish under either ceiling: intent bytes are in the witness.
    let mut changed_intent = events.clone();
    let begin = changed_intent.iter_mut().find_map(|event| match event {
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(CheckpointEvent::BeginReset(intent)))) => Some(intent),
        _ => None,
    }).unwrap();
    Rc::make_mut(begin).budget.audit.monitoring.refinements -= 1;
    let encoded = journal::encode(&host.profile, host.store.identity(), &changed_intent).unwrap();
    let mut decoded = journal::decode(&host.profile, host.store.identity(), &encoded).unwrap();
    bind_history(&mut decoded, &config).unwrap();
    assert_eq!(Machine::replay(&host.profile, &decoded).err(), Some(Error::Binding));
    events.retain(|event| !matches!(event,
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(CheckpointEvent::BeginReset(_))))));
    assert_eq!(Machine::replay(&host.profile, &events).err(), Some(Error::Incomplete));
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
}


#[test]
fn pending_reset_stops_an_already_started_source_bound_helper_before_first_socket_disclosure() {
    use crate::action::consequence::delivery::persistent::observed::helpers::learned_sockets::{
        LearnedSocketLaunch, LearnedSocketStatus,
    };
    use crate::action::consequence::oversight::{ReviewWindow, helper_workers::HelperLimits,
        learned_host::sidecar::LearnedSidecarRequest, sidecar::{SidecarIdentity, SidecarCongressBudget}};
    use std::collections::BTreeMap;
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    for blocked in [false, true] {
        let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let saved = capture(&mut host, 1);
        host.propose(host.revision(), 1, action_spec(&host), snapshot()).unwrap();
        let revision = host.actor_snapshot().unwrap().actor_revision;
        let plan = host.begin_learned_sidecar_plan(host.revision(), 1, revision, LearnedSidecarRequest {
            identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default(),
        }).unwrap();
        host.current_learned_sidecar(&plan).unwrap();
        let (server, mut peer) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap();
        let mut round = host.begin_learned_socket_round(host.revision(), plan, LearnedSocketLaunch {
            round: 101, evidence_root: [9; 32],
            window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
            streams: BTreeMap::from([("reviewer".to_owned(), server)]), limits: HelperLimits::default(),
        }, snapshot()).unwrap();
        if blocked {
            host.begin_learned_reset(host.revision(), &saved, control(&host, 900), budget(&host)).unwrap();
            let before = host.revision();
            let error = round.pump(&mut host, 0, ElapsedTick(1)).unwrap_err();
            assert_eq!(error.error, JournalError::Contract(Error::Incomplete));
            assert_eq!(round.status(), LearnedSocketStatus::Failed);
            assert_eq!(round.connection_steps(), 0); assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
            assert_eq!(host.revision(), before); assert!(host.pending_learned_reset().unwrap().is_some());
            let receipt = host.complete_learned_reset(host.revision(), 900).unwrap().unwrap();
            assert_eq!(receipt.control.cancelled, vec![1]);
            assert!(round.pump(&mut host, round.revision(), ElapsedTick(1)).is_err());
        } else {
            let mut disclosed = false;
            for _ in 0..128 {
                round.pump(&mut host, round.revision(), ElapsedTick(1)).unwrap();
                match peer.read(&mut [0; 1]) {
                    Ok(n) if n > 0 => { disclosed = true; break; }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    other => panic!("expected actual original request bytes, got {other:?}"),
                }
            }
            assert!(disclosed); assert!(round.connection_steps() > 0);
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn no_new_reset_audit_precedes_acknowledgment_of_its_write_ahead_instruction() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1);
        let (mut host, _) = owner(&root, &config);
        step(&mut host).unwrap(); let saved = capture(&mut host, 1); step(&mut host).unwrap();
        let before = host.learned_generation_inspection().unwrap().numerical;
        host.store.fail_once(barrier);
        let error = host.begin_learned_reset(host.revision(), &saved, control(&host, 900), budget(&host)).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("selected intent storage fault expected"); };
        assert_eq!(failure.operation, barrier);
        assert!(host.complete_learned_reset(host.revision(), 900).is_err());
        drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 0);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        if barrier == JournalIo::DirectorySync {
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
            assert!(host.complete_learned_reset(host.revision(), 900).is_err());
        } else {
            assert!(host.pending_learned_reset().unwrap().is_none());
            let saved = host.learned_checkpoint(1).unwrap();
            assert!(reset(&mut host, &saved, 900).unwrap().control.restored);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        }
        assert_eq!(host.inspect().executions, 0);
    }
}
