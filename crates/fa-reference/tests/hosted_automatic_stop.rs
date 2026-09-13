//! Actual bounded numerical inference and the ORIGINAL authority/endpoint paths.
//! The weights, labels and human/helper fixtures are not detector qualification.
#[path = "support/hosted_decoder.rs"]
#[allow(dead_code)]
mod support;
use support::*;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderModel, DecoderProfile, DecoderShape};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::SamplingBudget;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::oversight::decoder_host::{HostedCheckpointHandle, HostedResetRequest, HostedStopCause, HostedStopPolicy};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::Error;

fn policy() -> HostedStopPolicy { HostedStopPolicy::new(7, 3, 900).unwrap() }
fn reset(f: &Fixture, checkpoint: HostedCheckpointHandle, round: u64) -> HostedResetRequest {
    let state = f.broker.inspect();
    HostedResetRequest { checkpoint, expected_control_sequence: state.sequence,
        expected_actor_revision: f.broker.actor_revision(), expected_authority_epoch: state.ledger.epoch,
        binding: ReviewBinding { round, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&[f.endpoint.target()]).unwrap(), replay_budget: compute() }
}
fn forced(f: &mut Fixture, token: u32) -> Result<MonitoredStep, Error> {
    let state = f.broker.hosted_decoder().unwrap();
    f.broker.advance_hosted_forced(state.actor_revision, state.position, token, compute())
}
fn binary(overflow: bool) -> DecoderModel {
    let p = DecoderProfile::new(model().profile().identity(), DecoderShape { vocabulary: 2, hidden: 2,
        intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 1e-5, 10000.0).unwrap();
    let layers = control::numerical::fixture::zero_layers(&p);
    DecoderModel::new(p, if overflow { vec![0.0, 0.0, 1.0, 1.0] } else { vec![0.0, 1.0, 1.0, 0.0] },
        layers, vec![1.0; 2], if overflow { vec![f32::MAX; 4] } else { vec![0.0; 4] }).unwrap()
}

#[test]
fn quiet_inference_still_reaches_real_single_and_two_key_publication() {
    for two in [false, true] {
        let mut f = Fixture::new(two);
        f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
        f.broker.enable_hosted_stop(policy()).unwrap();
        assert!(matches!(forced(&mut f, 1).unwrap(), MonitoredStep::Released(_)));
        assert!(f.broker.hosted_stop_incident().is_none());
        assert_eq!(f.broker.enforce_hosted_stop().unwrap(), None);
        let envelope = f.dispatch(1, two.then_some(50));
        let executed = f.endpoint.deliver(&envelope).unwrap();
        f.broker.accept_receipt(executed).unwrap();
        assert_eq!(f.endpoint.execution_count(), 1);
        assert_eq!(f.endpoint.payload(), b"publish");
        assert_eq!(f.broker.inspect().ledger.charged, 16);
        assert!(!f.broker.inspect().suspended);
    }
}

#[test]
fn alarm_closes_original_authority_refunds_only_reservations_and_cannot_reset_away() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(alarm(), DecoderBindingLimits::default()).unwrap();
    f.broker.enable_hosted_stop(policy()).unwrap();
    forced(&mut f, 0).unwrap();
    let checkpoint = f.broker.capture_hosted_checkpoint(1, f.broker.actor_revision()).unwrap();
    let (action, input, permit) = approved(&mut f, 1);
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    assert!(matches!(forced(&mut f, 1).unwrap(), MonitoredStep::Held(_)));
    let incident = f.broker.hosted_stop_incident().unwrap().clone();
    assert_eq!(incident.cause(), HostedStopCause::Monitoring(MonitorOutcome::Alarm));
    assert_eq!(incident.policy(), policy()); assert_eq!(incident.position(), 2);
    assert_eq!(incident.sampled_draws(), 0);
    let stop = incident.stop_receipt().unwrap();
    assert_eq!(stop.request().operation, 900);
    assert_eq!(stop.cancelled(), &[1]); assert_eq!(stop.refunded_units(), 16);
    assert!(f.broker.inspect().suspended);
    assert_eq!(f.broker.inspect().ledger.available, 100);
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Cancelled);
    let before = f.broker.inspect();
    assert_eq!(f.broker.enforce_hosted_stop().unwrap().as_ref(), Some(stop));
    assert_eq!(f.broker.inspect(), before);
    assert!(f.broker.dispatch(&permit, &action, Some(&input), &snapshot()).is_err());
    assert!(matches!(f.broker.reset_hosted_decoder(reset(&f, checkpoint, 81)), Err(Error::WrongState)));
    assert_eq!(f.broker.hosted_recovery_usage().unwrap().replay_attempts, 0);
    assert!(matches!(forced(&mut f, 0), Err(Error::WrongState)));
    assert_eq!(f.broker.hosted_stop_incident().unwrap(), &incident);
    assert!(!f.broker.stop_progress().unwrap().endpoint_fenced);
    assert!(f.broker.progress_stop(&mut f.endpoint).unwrap().progress.drained());
}

#[test]
fn sent_obligations_survive_automatic_stop_and_only_endpoint_evidence_releases_them() {
    for two in [false, true] {
        let mut f = Fixture::new(two);
        f.broker.own_sampled_decoder(alarm(), DecoderBindingLimits::default()).unwrap();
        f.broker.enable_hosted_stop(policy()).unwrap(); forced(&mut f, 0).unwrap();
        let first = f.dispatch(1, two.then_some(50));
        let executed = f.endpoint.deliver(&first).unwrap();
        let missing = f.dispatch(2, two.then_some(50));
        assert_eq!(f.broker.inspect().ledger.charged, 32);
        forced(&mut f, 1).unwrap();
        assert_eq!(f.broker.inspect().ledger.charged, 32);
        assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Unknown);
        assert_eq!(f.broker.inspect().ledger.stages[&2], ActionState::Unknown);
        for id in [1, 2] { f.broker.inputs_unavailable(id, f.broker.input_revision(id).unwrap()).unwrap(); }
        let sweep = f.broker.progress_stop(&mut f.endpoint).unwrap();
        assert!(sweep.progress.drained()); assert_eq!(sweep.outcomes.len(), 2);
        assert_eq!(sweep.progress.charged_units, 16);
        assert_eq!(f.broker.inspect().ledger.available, 84);
        assert_eq!(f.endpoint.deliver(&missing), Err(Error::Stale));
        assert!(!f.broker.accept_receipt(executed).unwrap());
        assert_eq!(f.endpoint.execution_count(), 1);
    }
}

#[test]
fn malformed_stale_and_underbudget_calls_do_not_stop_or_spend_a_healthy_prefix() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    f.broker.enable_hosted_stop(policy()).unwrap(); forced(&mut f, 1).unwrap();
    let before = f.broker.hosted_decoder().unwrap(); let r = before.actor_revision; let p = before.position;
    assert!(matches!(f.broker.advance_hosted_forced(r - 1, p, 1, compute()), Err(Error::Stale)));
    assert!(matches!(f.broker.advance_hosted_forced(r, p + 1, 1, compute()), Err(Error::Stale)));
    assert!(matches!(f.broker.advance_hosted_forced(r, p, 99, compute()), Err(Error::InvalidInput)));
    assert!(matches!(f.broker.advance_hosted_forced(r, p, 1, DecoderBudget { scalar_products: 0 }), Err(Error::Limit)));
    let mut short = budget(6); short.sampling = SamplingBudget { vocabulary: 0 };
    assert!(matches!(f.broker.advance_hosted_sampled(r, p, short), Err(Error::Limit)));
    assert_eq!(f.broker.hosted_decoder().unwrap(), before);
    assert!(f.broker.stop_receipt().is_none()); assert!(f.broker.hosted_stop_incident().is_none());
    assert!(matches!(f.broker.advance_hosted_sampled(r, p, budget(6)).unwrap(), MonitoredSampledStep::Released(_)));
    assert_eq!(f.broker.hosted_decoder().unwrap().sampled_draws, 1);
}

#[test]
fn threshold_equality_and_monitor_capacity_are_not_reported_as_detected_alarms() {
    for capacity in [false, true] {
        let mut f = Fixture::new(false);
        let allowance = if capacity { RefinementBudget { encoded_bytes: 0, probe_coordinates: 0 } }
            else { control::numerical::allowance() };
        f.broker.own_sampled_decoder(monitored(binary(false), 0.0, allowance), DecoderBindingLimits::default()).unwrap();
        f.broker.enable_hosted_stop(policy()).unwrap();
        assert!(matches!(forced(&mut f, 0).unwrap(), MonitoredStep::Held(_)));
        let expected = if capacity { MonitorOutcome::BudgetExhausted } else { MonitorOutcome::AtThreshold };
        assert_eq!(f.broker.hosted_stop_incident().unwrap().cause(), HostedStopCause::Monitoring(expected));
        assert!(f.broker.inspect().suspended); assert_eq!(f.broker.inspect().ledger.charged, 0);
        assert_eq!(f.endpoint.execution_count(), 0);
    }
}

#[test]
fn admitted_overflow_with_unchanged_cache_still_stops_the_original_reserved_action() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(monitored(binary(true), 0.5, control::numerical::allowance()), DecoderBindingLimits::default()).unwrap();
    f.broker.enable_hosted_stop(policy()).unwrap(); forced(&mut f, 0).unwrap();
    let _authorized = approved(&mut f, 1);
    let before = f.broker.hosted_decoder().unwrap();
    assert!(matches!(forced(&mut f, 1), Err(Error::Overflow)));
    let after = f.broker.hosted_decoder().unwrap();
    assert_eq!(after.position, before.position); assert_eq!(after.actor_revision, before.actor_revision);
    assert_eq!(after.status, MonitoringStatus::Failed(Error::Overflow));
    assert_eq!(f.broker.hosted_stop_incident().unwrap().cause(), HostedStopCause::Numerical(Error::Overflow));
    assert_eq!(f.broker.inspect().ledger.available, 100); assert_eq!(f.broker.inspect().ledger.reserved, 0);
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Cancelled);
}

#[test]
fn policy_is_fixed_before_proposals_and_opt_out_keeps_explicit_reset_recovery() {
    let mut f = Fixture::new(false);
    assert_eq!(f.broker.enable_hosted_stop(policy()), Err(Error::Incomplete));
    for ids in [[0, 1, 1], [1, 0, 1], [1, 1, 0]] {
        assert_eq!(HostedStopPolicy::new(ids[0], ids[1], ids[2]), Err(Error::InvalidInput));
    }
    f.broker.own_sampled_decoder(alarm(), DecoderBindingLimits::default()).unwrap();
    forced(&mut f, 0).unwrap();
    let checkpoint = f.broker.capture_hosted_checkpoint(1, f.broker.actor_revision()).unwrap();
    let _authorized = approved(&mut f, 1);
    assert_eq!(f.broker.enable_hosted_stop(policy()), Err(Error::WrongState));
    forced(&mut f, 1).unwrap();
    assert!(!f.broker.inspect().suspended); assert!(f.broker.hosted_stop_incident().is_none());
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    assert!(f.broker.reset_hosted_decoder(reset(&f, checkpoint, 41)).unwrap().control.restored);
    assert_eq!(f.broker.hosted_stop_policy(), None);
    assert_eq!(f.broker.hosted_decoder().unwrap().status, MonitoringStatus::Ready);
}

#[test]
fn healthy_reset_keeps_the_policy_but_an_admitted_failed_replay_applies_it() {
    for exhausted in [false, true] {
        let mut f = Fixture::new(false);
        let limit = if exhausted { RefinementBudget { encoded_bytes: 10000, probe_coordinates: 2 } }
            else { control::numerical::allowance() };
        f.broker.own_sampled_decoder(monitored(binary(false), 0.5, limit), DecoderBindingLimits::default()).unwrap();
        f.broker.enable_hosted_stop(policy()).unwrap();
        assert_eq!(f.broker.enable_hosted_stop(HostedStopPolicy::new(8, 4, 901).unwrap()), Err(Error::Duplicate));
        forced(&mut f, 0).unwrap();
        let checkpoint = f.broker.capture_hosted_checkpoint(1, f.broker.actor_revision()).unwrap();
        let _authorized = approved(&mut f, 1);
        let result = f.broker.reset_hosted_decoder(reset(&f, checkpoint, 101));
        assert_eq!(f.broker.hosted_stop_policy(), Some(policy()));
        assert_eq!(f.broker.hosted_recovery_usage().unwrap().replay_attempts, 1);
        assert_eq!(f.broker.inspect().ledger.available, 100);
        if exhausted {
            assert!(matches!(result, Err(Error::Incomplete)));
            assert_eq!(f.broker.incident_count(), 0);
            assert_eq!(f.broker.hosted_stop_incident().unwrap().cause(), HostedStopCause::Numerical(Error::Incomplete));
            assert!(f.broker.inspect().suspended);
        } else {
            assert!(result.unwrap().control.restored);
            assert!(!f.broker.inspect().suspended); assert!(f.broker.hosted_stop_incident().is_none());
            let message = f.dispatch(2, None);
            let receipt = f.endpoint.deliver(&message).unwrap(); f.broker.accept_receipt(receipt).unwrap();
            assert_eq!(f.endpoint.execution_count(), 1);
        }
    }
}

#[test]
fn intentional_incident_suspension_is_not_relabelled_as_a_new_numerical_fault() {
    let mut f = Fixture::new(false);
    f.broker.own_sampled_decoder(quiet(), DecoderBindingLimits::default()).unwrap();
    f.broker.enable_hosted_stop(policy()).unwrap(); forced(&mut f, 1).unwrap();
    let checkpoint = f.broker.capture_hosted_checkpoint(1, f.broker.actor_revision()).unwrap();
    for round in [101, 102, 103] {
        let receipt = f.broker.reset_hosted_decoder(reset(&f, checkpoint.clone(), round)).unwrap();
        assert_eq!(receipt.control.restored, round != 103);
    }
    assert!(f.broker.inspect().suspended); assert_eq!(f.broker.incident_count(), 3);
    assert_eq!(f.broker.enforce_hosted_stop().unwrap(), None);
    assert!(f.broker.hosted_stop_incident().is_none()); assert!(f.broker.stop_receipt().is_none());
    assert!(matches!(forced(&mut f, 1), Err(Error::WrongState)));
    f.broker.observe_time(ElapsedTick(2)).unwrap();
    assert_eq!(f.broker.incident_count(), 3);
}
