//! Real numerical residuals, not caller-declared activation identity.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/investigation_decoder.rs"] mod numerical;
use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
use fa_reference::action::consequence::activation::{SourceFrame, FrameIdentity};
use fa_reference::action::consequence::activation::consistency::{
    BinaryForecast, ErrorBudget, ForecastModel, ForecastRegistration,
};
use fa_reference::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderModel, DecoderLayerWeights};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingStart, SamplingPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{TargetCeiling, ReviewBinding};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::OversightBroker;
use fa_reference::action::consequence::oversight::consistency::ConsistencyConfig;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::decoder_host::HostedResetRequest;
use fa_reference::Error;
use std::collections::BTreeMap;

fn predictor(model: &DecoderModel) -> ForecastModel {
    ForecastModel::new(LinearProbe::new(7, 1, model.residual_contract(1).unwrap().profile(),
        &[1.0, 0.0], 0.0, 1.5).unwrap(), ForecastRegistration {
        domain: 8, generation: 1, policy_generation: 1, event_prefix: b"risk".to_vec(),
        negative: BinaryForecast::new(16384, 49152).unwrap(),
        at_threshold: BinaryForecast::new(32768, 32768).unwrap(),
        positive: BinaryForecast::new(49152, 16384).unwrap(),
    }).unwrap()
}
fn owner(model: DecoderModel, alarm: f32, forecast: ForecastModel) -> OversightBroker {
    owner_stream(model, alarm, forecast, 5)
}
fn owner_stream(model: DecoderModel, alarm: f32, forecast: ForecastModel, stream: u64) -> OversightBroker {
    let p = ordinary::profile(); let d = p.delivery;
    let mut endpoint = PublicationEndpoint::new(d.target, d.initial_payload,
        d.retention_ticks, d.max_deliveries).unwrap();
    let mut b = OversightBroker::new(ControllerConfig {
        scope: d.scope, total: d.total, max_attempts: d.max_attempts, actor: d.actor,
        suspend_at_incident: d.suspend_at_incident, policy: d.policy, congress: d.congress,
        narrowed_targets: TargetCeiling::new(&d.narrowed_targets).unwrap(),
    }, &mut endpoint, p.committee).unwrap();
    b.confirm_fence(endpoint.install_fence(b.fence_request()).unwrap()).unwrap();
    let budget = RefinementBudget { encoded_bytes: 10000, probe_coordinates: 10000 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(),
        &[1.0, 0.0], 0.0, alarm).unwrap();
    let run = MonitoredSampledDecoder::new(model, 5, 1,
        BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], budget).unwrap())]),
        budget, SamplingStart { policy: SamplingPolicy::new(1, 1, 2, 1.0, 1, 1.0).unwrap(),
            stream: 19, seed: 3 }).unwrap();
    b.own_sampled_decoder(run, DecoderBindingLimits::default()).unwrap();
    b.enable_action_consistency(ConsistencyConfig { model: forecast, alpha: ErrorBudget::new(1, 4).unwrap(),
        stream, max_predictions: 8, max_prediction_age_ticks: 10 }).unwrap();
    b.observe_time(ElapsedTick(1)).unwrap();
    b
}
fn force(b: &mut OversightBroker, token: u32) -> Result<MonitoredStep, Error> {
    let n = b.hosted_decoder().unwrap();
    b.advance_hosted_forced(n.actor_revision, n.position, token, numerical::budget())
}
fn source(step: &MonitoredStep) -> &SourceFrame {
    let MonitoredStep::Released(step) = step else { panic!("expected fully reviewed token"); };
    step.step().layers[0].residual.source()
}
fn spec(b: &OversightBroker) -> ActionSpec {
    let p = ordinary::profile().delivery;
    ActionSpec { version: VERSION, scope: p.scope, target: Some(p.target), payload: b"ordinary".to_vec(),
        required_witnesses: vec![], policy_epoch: b.inspect().ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}

#[test]
fn forecasts_match_original_predictor_on_actual_residuals_without_recomputing_or_drawing() {
    let m = numerical::model(); let f = predictor(&m); let mut b = owner(m, 100.0, f.clone());
    b.require_hosted_action_consistency(1).unwrap();
    assert_eq!(b.forecast_hosted_action(1, b.actor_revision()), Err(Error::Incomplete));
    for (attempt, token, expected_null) in [(1, 0, 16384), (2, 1, 49152)] {
        let step = force(&mut b, token).unwrap();
        let before = b.hosted_decoder().unwrap();
        let predicted = b.forecast_hosted_action(attempt, b.actor_revision()).unwrap();
        assert_eq!(predicted, f.predict(source(&step)).unwrap());
        assert_eq!(predicted.forecast().null_numerator(), expected_null);
        assert_eq!(predicted.observation().frame().sequence, before.position);
        assert_eq!(b.hosted_decoder().unwrap(), before);
        b.propose(attempt, spec(&b), &ordinary::snapshot()).unwrap();
        b.cancel(attempt).unwrap();
    }
    assert_eq!(b.consistency_evidence().unwrap().samples(), 2);
    assert_eq!(b.hosted_decoder().unwrap().sampled_draws, 0);
}

#[test]
fn identical_metadata_cannot_substitute_values_or_even_supply_an_exact_copied_frame() {
    let m = numerical::model(); let mut b = owner(m.clone(), 100.0, predictor(&m));
    b.require_hosted_action_consistency(1).unwrap();
    let step = force(&mut b, 1).unwrap();
    let actual = source(&step);
    let fake = SourceFrame::capture(actual.identity(), &[-100.0, 0.0]).unwrap();
    let before = b.hosted_decoder().unwrap();
    for supplied in [&fake, actual] {
        assert_eq!(b.forecast_action(1, b.actor_revision(), supplied), Err(Error::Binding));
        assert_eq!(b.pending_forecast().unwrap(), None);
    }
    assert_eq!(b.forecast_hosted_action(1, before.actor_revision - 1), Err(Error::Stale));
    assert_eq!(b.forecast_hosted_action(1, before.actor_revision).unwrap().forecast().null_numerator(), 49152);
    assert_eq!(b.hosted_decoder().unwrap(), before);
}

#[test]
fn binding_checks_actual_tap_shape_and_stream_and_cannot_be_installed_after_work() {
    let m = numerical::model(); let mut b = owner(m.clone(), 100.0, predictor(&m));
    assert!(b.require_hosted_action_consistency(2).is_err());
    assert_eq!(b.hosted_consistency_layer(), None);
    b.require_hosted_action_consistency(1).unwrap();
    assert_eq!(b.require_hosted_action_consistency(1), Err(Error::Duplicate));
    let mut wrong_stream = owner_stream(m.clone(), 100.0, predictor(&m), 17);
    assert_eq!(wrong_stream.require_hosted_action_consistency(1), Err(Error::Binding));
    assert_eq!(wrong_stream.hosted_consistency_layer(), None);
    let mut late = owner(m.clone(), 100.0, predictor(&m));
    force(&mut late, 0).unwrap();
    assert_eq!(late.require_hosted_action_consistency(1), Err(Error::WrongState));
    assert_eq!(late.hosted_consistency_layer(), None);
    let mut wrong_profile = m.residual_contract(1).unwrap().profile(); wrong_profile.tap += 1;
    let reg = ForecastRegistration { domain: 1, generation: 1, policy_generation: 1, event_prefix: vec![1],
        negative: BinaryForecast::new(1, 2).unwrap(), at_threshold: BinaryForecast::new(1, 2).unwrap(),
        positive: BinaryForecast::new(1, 2).unwrap() };
    for (profile, weights) in [(wrong_profile, vec![1.0, 0.0]),
        (m.residual_contract(1).unwrap().profile(), vec![1.0])] {
        let f = ForecastModel::new(LinearProbe::new(1, 1, profile, &weights, 0.0, 0.0).unwrap(), reg.clone()).unwrap();
        let mut wrong = owner(m.clone(), 100.0, f);
        assert_eq!(wrong.require_hosted_action_consistency(1), Err(Error::Binding));
        assert_eq!(wrong.hosted_consistency_layer(), None);
    }
}

#[test]
fn precomputation_refusal_keeps_current_source_but_a_later_hold_withdraws_it() {
    let m = numerical::model(); let mut b = owner(m.clone(), 1.5, predictor(&m));
    b.require_hosted_action_consistency(1).unwrap();
    let first = force(&mut b, 0).unwrap();
    assert!(matches!(force(&mut b, 2), Err(Error::InvalidInput)));
    b.forecast_hosted_action(1, b.actor_revision()).unwrap();
    b.propose(1, spec(&b), &ordinary::snapshot()).unwrap(); b.cancel(1).unwrap();
    assert!(matches!(force(&mut b, 1).unwrap(), MonitoredStep::Held(_)));
    let n = b.hosted_decoder().unwrap(); assert_eq!(n.status, MonitoringStatus::Held);
    assert_eq!(b.forecast_hosted_action(2, b.actor_revision()), Err(Error::Incomplete));
    assert_eq!(b.forecast_action(2, b.actor_revision(), source(&first)), Err(Error::Binding));
    assert_eq!(b.pending_forecast().unwrap(), None);
    assert_eq!(b.consistency_evidence().unwrap().samples(), 1);
    assert_eq!(b.hosted_decoder().unwrap(), n);
}

#[test]
fn actual_numerical_failure_cannot_forecast_from_the_last_quiet_prefix() {
    let m = DecoderModel::new(numerical::profile(), vec![0.0, 0.0, 1.0, 1.0], vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![f32::MAX; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], vec![0.0; 4]).unwrap();
    let mut b = owner(m.clone(), 100.0, predictor(&m)); b.require_hosted_action_consistency(1).unwrap();
    force(&mut b, 0).unwrap();
    assert!(matches!(force(&mut b, 1), Err(Error::Overflow)));
    assert_eq!(b.hosted_decoder().unwrap().status, MonitoringStatus::Failed(Error::Overflow));
    assert_eq!(b.forecast_hosted_action(1, b.actor_revision()), Err(Error::Incomplete));
    assert_eq!(b.pending_forecast().unwrap(), None);
}

#[test]
fn numerical_rewind_does_not_reopen_the_raw_source_api_or_reset_the_predictor() {
    let m = numerical::model(); let mut b = owner(m.clone(), 100.0, predictor(&m));
    b.require_hosted_action_consistency(1).unwrap();
    let first = force(&mut b, 0).unwrap();
    let cp = b.capture_hosted_checkpoint(7, b.actor_revision()).unwrap();
    force(&mut b, 1).unwrap();
    let before = b.consistency_evidence().unwrap().clone();
    let control = b.inspect();
    let reset = b.reset_hosted_decoder(HostedResetRequest { checkpoint: cp,
        expected_control_sequence: control.sequence, expected_actor_revision: b.actor_revision(),
        expected_authority_epoch: control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&ordinary::profile().delivery.narrowed_targets).unwrap(),
        replay_budget: numerical::budget(),
    }).unwrap();
    assert!(reset.control.restored);
    assert_eq!(b.hosted_consistency_layer(), Some(1));
    assert_eq!(b.forecast_action(1, b.actor_revision(), source(&first)), Err(Error::Binding));
    assert!(b.forecast_hosted_action(1, b.actor_revision()).is_err());
    assert_eq!(b.consistency_evidence().unwrap(), &before);
    // A separately declared same-shaped capture does not repair this boundary.
    let id = source(&first).identity();
    let fake = SourceFrame::capture(FrameIdentity { stream: reset.resumed_stream.unwrap(), ..id }, &[1.0, 0.0]).unwrap();
    assert_eq!(b.forecast_action(1, b.actor_revision(), &fake), Err(Error::Binding));
}
