//! Original learned inference, exact predictor, durable request book and two keys.
//! Synthetic models and registration probabilities are controls, not calibration.
use crate::action::consequence::delivery::persistent::observed::{FileHumanReviewer,
    FileOversightProfile, machine::Machine, journal, storage};
use crate::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedConfig, bind_history};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationEvent;
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::delivery::{EndpointOutcome, persistent::{JournalIo, Reconciliation}};
use std::rc::Rc;
// Same original numerical/congress fixture, without copying or replacing engines.
include!("../../decoder/learned/tests/fixture.rs");

fn parameters(crossing: bool) -> FileConsistencyParameters {
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    FileConsistencyParameters { probe_id: 91, probe_generation: 1,
        profile: model().residual_contract(1).unwrap().profile(), weights: vec![1.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"visible".to_vec(), negative: BinaryForecast::new(49_152, 16_384).unwrap(),
            at_threshold: neutral, positive: if crossing { BinaryForecast::new(16_384, 49_152).unwrap() } else { neutral } },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10 }
}
fn predictor(crossing: bool) -> FileConsistencyConfig {
    FileConsistencyConfig::new(parameters(crossing)).unwrap().with_hosted_residual(1).unwrap()
}
fn install(host: &mut FileOversight, crossing: bool) -> FileConsistencyObserver {
    host.enable_action_consistency(host.revision(), predictor(crossing)).unwrap()
}
fn disk(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}
fn forecast(role: &FileConsistencyObserver, host: &mut FileOversight, attempt: u64) -> Prediction {
    let n = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    role.forecast_hosted_action(host, revision, attempt, n.actor_revision).unwrap().unwrap()
}

#[test]
fn learned_forecast_matches_original_predictor_on_actual_accepted_residual_without_new_inference() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let role = install(&mut host, false);
    assert_eq!(host.hosted_consistency_layer().unwrap(), Some(1));
    let empty = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.machine.broker.forecast_hosted_action(1, empty.actor_revision).is_err());
    let event = step(&mut host).unwrap();
    let frame = event.accepted().unwrap().layers[0].residual.source();
    let expected = predictor(false).build().unwrap().model.predict(frame).unwrap();
    let before = host.learned_generation_inspection().unwrap().numerical;
    // Even the exact genuine frame cannot enter the supplied-frame API once
    // owned-source mode is selected. Pair it with a working owned-source call.
    let revision = host.revision();
    assert!(role.forecast_action(&mut host, revision, 1, before.actor_revision, frame)
        .and_then(|result| result.map_err(Into::into)).is_err());
    let prediction = forecast(&role, &mut host, 1);
    assert_eq!(prediction, expected);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(1));
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    let replayed = read_machine(&host, &config);
    assert_eq!(replayed.consistency_snapshot(host.revision()).unwrap(), host.action_consistency_snapshot().unwrap());
}

#[test]
fn learned_binding_refuses_wrong_model_layer_dimensions_stream_and_late_registration() {
    for defect in 0..5 {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let mut p = parameters(false); let mut layer = 1;
        match defect {
            0 => p.profile.model_generation += 1,
            1 => layer = 3,
            2 => { p.weights.pop(); },
            3 => p.stream += 1,
            _ => { step(&mut host).unwrap(); },
        }
        let before = host.learned_generation_inspection().unwrap(); let bytes = disk(&host);
        let config = FileConsistencyConfig::new(p).unwrap().with_hosted_residual(layer).unwrap();
        assert!(host.enable_action_consistency(host.revision(), config).is_err());
        assert!(!host.action_consistency_required());
        assert_eq!(disk(&host), bytes); assert_eq!(host.learned_generation_inspection().unwrap(), before);
        assert!(host.storage_failure().is_none());
        if defect != 4 {
            let role = install(&mut host, false); step(&mut host).unwrap(); forecast(&role, &mut host, 1);
        }
    }
}

#[test]
fn learned_forecast_binds_external_request_and_uses_original_observed_payload_once() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let role = install(&mut host, false); step(&mut host).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    role.forecast_hosted_request(&mut host, revision, 71, n.actor_revision).unwrap().unwrap();
    let spec = action_spec(&host); let bytes = disk(&host);
    assert!(host.submit_request(host.revision(), 72, spec.clone(), snapshot()).is_err());
    assert_eq!(disk(&host), bytes);
    host.submit_request(host.revision(), 71, spec.clone(), snapshot()).unwrap();
    let evidence = host.action_consistency_snapshot().unwrap();
    assert_eq!(evidence.pending_attempt, None); assert_eq!(evidence.evidence.samples(), 1);
    assert!(!evidence.evidence.crossed());
    host.submit_request(host.revision(), 71, spec, snapshot()).unwrap();
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence.evidence);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
}

#[test]
fn learned_prediction_agreement_still_requires_congress_and_both_original_effect_keys() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, reviewer) = owner(&root, &config);
    let role = install(&mut host, false); step(&mut host).unwrap(); forecast(&role, &mut host, 1);
    let (action, inputs, automatic, request) = prepared(&mut host);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0);
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let result = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let evidence = host.action_consistency_snapshot().unwrap().evidence;
    drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
    assert_eq!(host.inspect().executions, 1);
    assert!(host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).is_err());
}

#[test]
fn actual_likelihood_crossing_holds_the_effect_and_survives_reconstruction() {
    for crossing in [false, true] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let role = install(&mut host, crossing); step(&mut host).unwrap(); forecast(&role, &mut host, 1);
        let spec = action_spec(&host);
        let result = host.propose_consistent(host.revision(), 1, spec, snapshot()).unwrap();
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        assert_eq!(evidence.samples(), 1); assert_eq!(evidence.crossed(), crossing);
        assert_eq!(result.is_err(), crossing);
        assert_eq!(host.inspect().executions, 0);
        let replayed = read_machine(&host, &config);
        assert_eq!(replayed.consistency_snapshot(host.revision()).unwrap().evidence, evidence);
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
    }
}

#[test]
fn learned_monitor_hold_cannot_supply_an_older_quiet_forecast_residual() {
    let root = Directory::new(); let config = config(true, 1); let (mut host, _) = owner(&root, &config);
    let role = install(&mut host, false); step(&mut host).unwrap();
    let quiet = host.learned_generation_inspection().unwrap().numerical;
    let held = step(&mut host).unwrap(); assert!(held.accepted().is_none());
    let current = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(current.position, quiet.position);
    let before = host.action_consistency_snapshot().unwrap();
    assert!(host.machine.broker.forecast_hosted_action(1, current.actor_revision).is_err());
    assert_eq!(host.action_consistency_snapshot().unwrap(), before);
    let revision = host.revision();
    assert!(role.forecast_hosted_action(&mut host, revision, 1, current.actor_revision)
        .and_then(|result| result.map_err(Into::into)).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn pending_learned_forecast_and_lost_observer_do_not_regain_coverage_after_recovery() {
    let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
    let role = install(&mut host, false); step(&mut host).unwrap(); forecast(&role, &mut host, 1);
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(1)); drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let before = host.action_consistency_snapshot().unwrap();
    assert!(before.coverage_lost); assert_eq!(before.pending_attempt, Some(1));
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    assert_eq!(role.forecast_hosted_action(&mut host, revision, 1, n.actor_revision).err(), Some(Error::Binding.into()));
    assert_eq!(host.action_consistency_snapshot().unwrap(), before);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn every_forecast_write_barrier_returns_no_prediction_and_recovery_retains_only_canonical_coverage() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1); let (mut host, _) = owner(&root, &config);
        let role = install(&mut host, false); step(&mut host).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let before = host.inspect(); let revision = host.revision(); host.store.fail_once(barrier);
        assert!(matches!(role.forecast_hosted_action(&mut host, revision, 1, numerical.actor_revision),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), before); drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let recovered = host.action_consistency_snapshot().unwrap();
        assert_eq!(recovered.evidence.samples(), 0);
        if recovered.pending_attempt.is_some() {
            assert_eq!(recovered.pending_attempt, Some(1)); assert!(recovered.coverage_lost);
        }
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
        let revision = host.revision();
        assert_eq!(role.forecast_hosted_action(&mut host, revision, 1, numerical.actor_revision).err(), Some(Error::Binding.into()));
    }
}
