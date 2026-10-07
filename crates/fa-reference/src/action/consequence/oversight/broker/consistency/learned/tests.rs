//! Real-codec positive and causal-negative controls on the original effect path.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::consistency::ErrorBudget;
use crate::action::consequence::activation::probe::learned::ResidualRetention;
use crate::action::consequence::oversight::consistency::{ConsistencyStopCause, ConsistencyStopPolicy};
mod fixture;
use fixture::*;

fn alpha() -> ErrorBudget { ErrorBudget::new(1, 4).unwrap() }
fn stop() -> ConsistencyStopPolicy { ConsistencyStopPolicy::new(11, 1, 7007).unwrap() }

#[test]
fn learned_forecasts_reach_the_original_endpoint_only_after_congress() {
    for (value, weights) in [([1.0, 0.0], [1.0, 0.0]), ([1.0, 1.0], [0.0, 1.0])] {
        let (checked, original) = source(value, ResidualRetention::All, 1, 21);
        let (mut baseline, _) = owner();
        baseline.enable_action_consistency(config(&checked, &weights, alpha())).unwrap();
        let exact = baseline.forecast_action(1, 0, raw(&original)).unwrap();
        baseline.propose(1, spec(0, b"safe"), &snapshot()).unwrap();
        let (mut broker, mut endpoint) = owner();
        enable(&mut broker, &checked, &weights, LearnedMonitorBudget::default(), alpha());
        assert!(broker.propose(1, spec(0, b"safe"), &snapshot()).is_err());
        let report = broker.forecast_learned_action(1, 0, &checked, row()).unwrap();
        assert_eq!(report.prediction().unwrap().forecast(), exact.forecast());
        if weights[1] != 0.0 { assert!(report.work().refinements > 0); }
        else { assert_eq!(report.work().refinements, 0); }
        let deadline = broker.consistency_deadline().unwrap().unwrap();
        assert_eq!(deadline.source_sequence, checked.row_shape(row()).unwrap().0.sequence);
        let action = broker.propose(1, spec(0, b"safe"), &snapshot()).unwrap().action;
        let input = inputs(&broker, &action);
        assert!(broker.authorize(1, Some(&input), &snapshot()).is_err());
        let input = review(&mut broker, &action, 1);
        let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
        let message = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
        broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
        assert_eq!(endpoint.payload(), b"safe"); assert_eq!(endpoint.execution_count(), 1);
        assert_eq!(broker.consistency_evidence().unwrap(), baseline.consistency_evidence().unwrap());
        let learned = broker.learned_consistency_observation(1).unwrap();
        let raw = baseline.consistency_observation(1).unwrap();
        assert_eq!(learned.factor(), raw.factor()); assert_eq!(learned.sample(), raw.sample());
        assert_eq!(learned.policy_epoch(), raw.policy_epoch());
        assert_eq!(learned.prediction().work(), report.work());
        assert_eq!(learned.forecast_at(), raw.forecast_at());
        assert_eq!(learned.observed_actor_revision(), raw.observed_actor_revision());
        assert!(matches!(broker.consistency_observation(1), Err(Error::Binding)));
        assert!(matches!(baseline.learned_consistency_observation(1), Err(Error::Binding)));
        assert_eq!(broker.pending_forecast().unwrap(), None);
        assert!(matches!(broker.expire_consistency_deadline(deadline), Err(Error::Missing)));
    }
}

#[test]
fn missing_residual_and_exhausted_refinement_spend_a_job_but_never_a_sample() {
    for missing in [false, true] {
        let retention = if missing { ResidualRetention::None } else { ResidualRetention::All };
        let (checked, original) = source([1.0, 1.0], retention, 1, 21);
        let mut budget = LearnedMonitorBudget::default();
        if !missing { budget.refinements = 0; }
        let (mut broker, endpoint) = owner();
        enable(&mut broker, &checked, &[0.0, 1.0], budget, alpha());
        broker.enable_consistency_stop(stop()).unwrap();
        let report = broker.forecast_learned_action(1, 0, &checked, row()).unwrap();
        assert_eq!(report.prediction().unwrap_err(), if missing { Error::Incomplete } else { Error::Limit });
        assert!(report.work().probe_coordinates > 0);
        assert_eq!(broker.pending_forecast().unwrap(), None);
        assert!(broker.consistency_coverage_lost().unwrap());
        assert_eq!(broker.consistency_evidence().unwrap().samples(), 0);
        let incident = broker.consistency_stop_incident().unwrap();
        assert_eq!(incident.cause, ConsistencyStopCause::CoverageLost);
        assert_eq!(incident.prediction_jobs, 1); assert!(incident.receipt.is_some());
        assert!(broker.forecast_learned_action(2, 0, &checked, row()).is_err());
        assert_eq!(broker.forecast_action(2, 0, raw(&original)), Err(Error::Binding));
        assert!(broker.propose(1, spec(0, b"safe"), &snapshot()).is_err());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn source_and_attempt_preflight_refusals_do_not_consume_the_valid_capture() {
    let (checked, original) = source([1.0, 0.0], ResidualRetention::All, 1, 21);
    let (foreign, _) = source([1.0, 0.0], ResidualRetention::All, 1, 22);
    let (future, _) = source([1.0, 0.0], ResidualRetention::All, 2, 21);
    let (mut broker, _) = owner();
    enable(&mut broker, &checked, &[1.0, 0.0], LearnedMonitorBudget::default(), alpha());
    assert_eq!(broker.forecast_action(1, 0, raw(&original)), Err(Error::Binding));
    assert_eq!(broker.require_hosted_action_consistency(1), Err(Error::Binding));
    for (attempt, revision, image, selected, expected) in [
        (0, 0, &checked, row(), Error::InvalidInput),
        (1, 1, &checked, row(), Error::Stale),
        (1, 0, &foreign, row(), Error::Binding),
        (1, 0, &future, row(), Error::Binding),
        (1, 0, &checked, KvRow { side: KvSide::Value, ..row() }, Error::Binding),
        (1, 0, &checked, KvRow { layer: 2, ..row() }, Error::Binding),
    ] {
        assert_eq!(broker.forecast_learned_action(attempt, revision, image, selected).unwrap_err(), expected);
        assert_eq!(broker.consistency.as_ref().unwrap().jobs, 0);
        assert!(!broker.consistency_coverage_lost().unwrap());
    }
    broker.forecast_learned_action(1, 0, &checked, row()).unwrap().prediction().unwrap();
    assert!(matches!(broker.forecast_learned_action(2, 0, &checked, row()), Err(Error::WrongState)));
    assert!(matches!(broker.propose(2, spec(0, b"safe"), &snapshot()), Err(Error::Binding)));
    assert_eq!(broker.pending_forecast().unwrap(), Some(1));
    broker.propose(1, spec(0, b"safe"), &snapshot()).unwrap();
    assert!(matches!(broker.forecast_learned_action(1, 0, &checked, row()), Err(Error::Duplicate)));
    assert!(matches!(broker.forecast_learned_action(2, 0, &checked, row()), Err(Error::Stale)));
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
}

#[test]
fn original_threshold_stop_and_silent_deadline_apply_to_learned_evidence() {
    let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1, 21);
    for silence in [false, true] {
        let (mut broker, endpoint) = owner();
        enable(&mut broker, &checked, &[1.0, 0.0], LearnedMonitorBudget::default(), ErrorBudget::new(1, 2).unwrap());
        broker.enable_consistency_stop(stop()).unwrap();
        broker.forecast_learned_action(1, 0, &checked, row()).unwrap().prediction().unwrap();
        if silence {
            let deadline = broker.consistency_deadline().unwrap().unwrap();
            assert!(!broker.expire_consistency_deadline(deadline).unwrap());
            broker.observe_time(deadline.expires_at).unwrap();
            assert!(broker.expire_consistency_deadline(deadline).unwrap());
            assert_eq!(broker.pending_forecast().unwrap(), Some(1));
            assert_eq!(broker.consistency_evidence().unwrap().samples(), 0);
            assert_eq!(broker.consistency_stop_incident().unwrap().cause, ConsistencyStopCause::CoverageLost);
        } else {
            assert!(broker.propose(1, spec(0, b"risk"), &snapshot()).is_err());
            assert_eq!(broker.consistency_evidence().unwrap().first_crossing(), Some(1));
            assert!(broker.learned_consistency_observation(1).unwrap().crossed());
            assert_eq!(broker.consistency_stop_incident().unwrap().cause,
                ConsistencyStopCause::ThresholdCrossed { first_sample: 1 });
        }
        assert!(broker.stop_receipt().is_some()); assert!(broker.inspect().suspended);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn later_native_admission_failure_cannot_erase_or_repeat_the_observed_category() {
    let (checked, _) = source([1.0, 0.0], ResidualRetention::All, 1, 21);
    let (mut broker, _) = owner();
    enable(&mut broker, &checked, &[1.0, 0.0], LearnedMonitorBudget::default(), alpha());
    broker.forecast_learned_action(1, 0, &checked, row()).unwrap().prediction().unwrap();
    let mut forbidden = spec(0, b"risk"); forbidden.units = 101;
    assert!(broker.propose(1, forbidden, &snapshot()).is_err());
    assert!(broker.learned_consistency_observation(1).unwrap().event());
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
    assert_eq!(broker.pending_forecast().unwrap(), None);
    assert!(matches!(broker.propose(1, spec(0, b"safe"), &snapshot()), Err(Error::Duplicate)));
    assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
}
