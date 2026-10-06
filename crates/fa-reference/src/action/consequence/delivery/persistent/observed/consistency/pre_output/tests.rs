//! Original numerical sampling and journal bytes, not supplied model verdicts.
use super::FilePreOutputForecast;
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, FileHumanReviewer, machine::Machine, journal, storage,
    consistency::{FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver},
    decoder::learned::{FileLearnedConfig, bind_history}};
use crate::action::consequence::delivery::persistent::{JournalError, JournalIo};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{GenerationEvent, GenerationStatus};
use crate::Error;
use std::rc::Rc;
include!("../../decoder/learned/tests/fixture.rs");

fn predictor() -> FileConsistencyConfig {
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model().residual_contract(1).unwrap().profile(), weights: vec![1.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"visible".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap()
}
fn installed(root: &Directory, alarm: bool) -> (FileOversight, FileConsistencyObserver, FileLearnedConfig) {
    let config = config(alarm, 1); let (mut host, _) = owner(root, &config);
    let role = host.enable_action_consistency(host.revision(), predictor()).unwrap();
    (host, role, config)
}
fn begin(role: &FileConsistencyObserver, host: &mut FileOversight) -> FilePreOutputForecast {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    role.begin_pre_output_request(host, revision, 71, n.actor_revision, n.position).unwrap().unwrap()
}
fn disk(host: &FileOversight) -> Vec<u8> { host.store.read(host.profile.delivery.limits.bytes).unwrap() }
fn same(host: &FileOversight, config: &FileLearnedConfig) {
    let original = read_machine(host, config);
    assert_eq!(original.snapshot(host.events.len()), host.inspect());
    assert_eq!(original.broker.hosted_learned_generation().unwrap(), host.learned_generation_inspection().unwrap().numerical);
    assert_eq!(original.consistency_snapshot(host.revision()).unwrap(), host.action_consistency_snapshot().unwrap());
}

#[test]
fn forecast_acknowledges_before_sampling_and_continuation_matches_original_steps_byte_for_byte() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, role, config) = installed(&root, false);
    let (mut control, control_role, _) = installed(&other, false);
    let prompt = step(&mut host).unwrap(); step(&mut control).unwrap();
    let expected = predictor().build().unwrap().model.predict(prompt.accepted().unwrap().layers[0].residual.source()).unwrap();
    let before = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    let mut run = begin(&role, &mut host);
    control_role.forecast_hosted_request(&mut control, revision, 71, before.actor_revision).unwrap().unwrap();
    assert_eq!(run.prediction(), &expected); assert_eq!(run.request(), 71);
    assert_eq!(run.deadline().source_sequence, 1);
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
    assert_eq!(before.cumulative_work.sampling_attempts, 0);
    assert_eq!(host.retained_requests(), 0);
    while run.numerical().status.is_active() {
        let revision = host.revision(); let event = run.advance(&mut host, revision).unwrap().unwrap();
        let original = step(&mut control).unwrap();
        assert_eq!(event.status(), original.status()); assert_eq!(event.sample(), original.sample());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            control.learned_generation_inspection().unwrap().numerical);
        assert_eq!(disk(&host), journal::encode(&host.profile, host.store.identity(), &control.events).unwrap());
        same(&host, &config);
    }
    let before = disk(&host); let revision = host.revision();
    assert_eq!(run.advance(&mut host, revision).err(), Some(Error::WrongState.into()));
    assert_eq!(disk(&host), before);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn incomplete_prompt_and_any_prior_sampling_refuse_without_a_forecast_transaction() {
    for samples in [None, Some(1), Some(3)] {
        let root = Directory::new(); let (mut host, role, _) = installed(&root, false);
        if let Some(samples) = samples { step(&mut host).unwrap(); for _ in 0..samples { step(&mut host).unwrap(); } }
        let n = host.learned_generation_inspection().unwrap().numerical;
        let bytes = disk(&host); let revision = host.revision();
        assert!(role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).is_err());
        assert_eq!(disk(&host), bytes); assert_eq!(host.revision(), revision);
        assert!(host.action_consistency_snapshot().unwrap().pending_attempt.is_none());
    }
    let root = Directory::new(); let (mut host, role, _) = installed(&root, false);
    step(&mut host).unwrap(); let mut run = begin(&role, &mut host); let revision = host.revision();
    assert!(run.advance(&mut host, revision).unwrap().unwrap().sample().is_some());
}

#[test]
fn stale_or_foreign_calls_and_an_unfinished_intent_cannot_establish_a_pre_output_handle() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, role, _) = installed(&root, false); let (_, foreign, _) = installed(&other, false);
    step(&mut host).unwrap(); let n = host.learned_generation_inspection().unwrap().numerical;
    let bytes = disk(&host); let revision = host.revision();
    assert_eq!(foreign.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).err(), Some(Error::Binding.into()));
    assert_eq!(role.begin_pre_output_request(&mut host, revision - 1, 71, n.actor_revision, n.position).err(), Some(Error::Stale.into()));
    assert_eq!(role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision + 1, n.position).err(), Some(Error::Stale.into()));
    assert_eq!(disk(&host), bytes);
    host.begin_learned_step(revision, n.actor_revision, n.position).unwrap();
    let revision = host.revision(); let bytes = disk(&host);
    assert_eq!(role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).err(), Some(Error::Incomplete.into()));
    assert_eq!(disk(&host), bytes); assert!(host.learned_generation_inspection().unwrap().pending.is_some());
}

#[test]
fn interleaved_time_is_allowed_but_unobserved_numerical_progress_is_not_adopted() {
    let root = Directory::new(); let (mut host, role, config) = installed(&root, false);
    step(&mut host).unwrap(); let mut run = begin(&role, &mut host);
    let stale = host.revision(); host.observe_time(stale, ElapsedTick(2)).unwrap();
    assert_eq!(run.advance(&mut host, stale).err(), Some(Error::Stale.into()));
    let revision = host.revision(); run.advance(&mut host, revision).unwrap().unwrap();
    step(&mut host).unwrap(); let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
    let revision = host.revision(); assert_eq!(run.advance(&mut host, revision).err(), Some(Error::Stale.into()));
    assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), before); same(&host, &config);
}

#[test]
fn consuming_the_keyed_forecast_prevents_further_continuation_and_cannot_score_twice() {
    let root = Directory::new(); let (mut host, role, config) = installed(&root, false);
    step(&mut host).unwrap(); let mut run = begin(&role, &mut host);
    let spec = action_spec(&host); let bytes = disk(&host);
    assert!(host.submit_request(host.revision(), 72, spec.clone(), snapshot()).is_err());
    assert_eq!(disk(&host), bytes);
    host.submit_request(host.revision(), 71, spec.clone(), snapshot()).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    assert_eq!(run.advance(&mut host, revision).err(), Some(Error::Binding.into()));
    host.submit_request(host.revision(), 71, spec, snapshot()).unwrap();
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n); same(&host, &config);
}

#[test]
fn expired_or_abandoned_prediction_keeps_coverage_loss_instead_of_running_more_inference() {
    for expired in [false, true] {
        let root = Directory::new(); let (mut host, role, config) = installed(&root, false);
        step(&mut host).unwrap(); let mut run = begin(&role, &mut host);
        host.observe_time(host.revision(), ElapsedTick(if expired { 11 } else { 10 })).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
        let result = run.advance(&mut host, revision);
        if expired {
            assert!(result.is_err()); assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
            assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
        } else { assert!(result.unwrap().unwrap().sample().is_some()); }
        drop(run); assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(1));
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
        assert!(host.learned_generation_inspection().unwrap().paused);
    }
}

#[test]
fn held_original_sample_stays_withheld_and_cannot_be_retried_through_the_forecast() {
    let root = Directory::new(); let (mut host, role, config) = installed(&root, true);
    step(&mut host).unwrap(); let mut run = begin(&role, &mut host); let revision = host.revision();
    let held = run.advance(&mut host, revision).unwrap().unwrap();
    assert!(matches!(held.status(), GenerationStatus::Held(_)));
    assert!(held.sample().is_none()); assert!(held.accepted().is_none());
    assert_eq!(run.numerical().cumulative_work.sampling_attempts, 1);
    let before = host.learned_generation_inspection().unwrap(); let revision = host.revision();
    assert_eq!(run.advance(&mut host, revision).err(), Some(Error::WrongState.into()));
    assert_eq!(host.learned_generation_inspection().unwrap(), before); same(&host, &config);
}

#[test]
fn every_forecast_write_barrier_returns_no_handle_and_recovered_owners_reject_old_roles() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, role, config) = installed(&root, false);
        step(&mut host).unwrap(); let n = host.learned_generation_inspection().unwrap().numerical;
        let revision = host.revision(); let before = host.inspect(); host.store.fail_once(barrier);
        assert!(matches!(role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some()); drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
        assert!(host.learned_generation_inspection().unwrap().paused);
        let revision = host.revision();
        assert_eq!(role.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).err(), Some(Error::Binding.into()));
    }
}

#[test]
fn both_generation_write_stages_keep_the_forecast_obligation_across_every_storage_failure() {
    for completion in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let (mut host, role, config) = installed(&root, false);
            step(&mut host).unwrap(); let mut run = begin(&role, &mut host);
            if completion { host.begin_learned_step(host.revision(), run.numerical().actor_revision, run.numerical().position).unwrap(); }
            let before = host.inspect(); let revision = host.revision(); host.store.fail_once(barrier);
            assert!(matches!(run.advance(&mut host, revision), Err(JournalError::Io(failure)) if failure.operation == barrier));
            assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some()); drop(host);
            let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
            assert!(recovered.action_consistency_snapshot().unwrap().coverage_lost);
            assert_eq!(recovered.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert!(recovered.learned_generation_inspection().unwrap().paused);
            let revision = recovered.revision(); assert_eq!(run.advance(&mut recovered, revision).err(), Some(Error::Binding.into()));
            assert_eq!(recovered.inspect().executions, 0);
        }
    }
}
