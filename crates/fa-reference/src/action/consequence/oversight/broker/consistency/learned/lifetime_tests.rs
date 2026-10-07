//! Real checked-codec controls using the existing decoder/broker/endpoint fixture.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::consistency::ErrorBudget;
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::activation::probe::learned::ResidualRetention;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::oversight::consistency::{ConsistencyStopCause, ConsistencyStopPolicy};
// This existing shared fixture also contains endpoint helpers used by the
// original learned-lane tests, but not by every accounting control here.
#[allow(dead_code)]
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;

fn alpha() -> ErrorBudget { ErrorBudget::new(1, 4).unwrap() }
fn registration(checked: &CheckedLearnedKv, weights: &[f32]) -> LearnedConsistencyConfig {
    LearnedConsistencyConfig { consistency: config(checked, weights, alpha()),
        layer: 1, side: KvSide::Key, budget: LearnedMonitorBudget::default() }
}

#[test]
fn completed_refusals_preserve_actual_work_and_stop_evidence() {
    for missing in [false, true] {
        let retention = if missing { ResidualRetention::None } else { ResidualRetention::All };
        let (checked, _) = source([1.0, 1.0], retention, 1, 21);
        let (mut broker, endpoint) = owner();
        let mut lifetime = LearnedMonitorBudget::default();
        if !missing { lifetime.refinements = 0; }
        broker.enable_learned_action_consistency_with_limits(registration(&checked, &[0.0, 1.0]),
            lifetime, checked.report().total_encoded_bytes).unwrap();
        broker.enable_consistency_stop(ConsistencyStopPolicy::new(11, 1, 7007).unwrap()).unwrap();
        let report = broker.forecast_learned_action(1, 0, &checked, row()).unwrap();
        assert!(report.prediction().is_err());
        assert!(report.work().probe_coordinates > 0);
        assert_eq!(broker.learned_consistency_work().unwrap(), report.work());
        assert_eq!(broker.learned_consistency_report(1).unwrap().work(), report.work());
        assert_eq!(broker.learned_consistency_report(1).unwrap().monitor().outcome(), report.monitor().outcome());
        assert_eq!(broker.learned_consistency_retained_source_bytes().unwrap(), checked.report().total_encoded_bytes);
        assert!(!broker.learned_consistency_has_unreported_work().unwrap());
        assert!(broker.consistency_coverage_lost().unwrap());
        assert_eq!(broker.consistency_evidence().unwrap().samples(), 0);
        assert_eq!(broker.pending_forecast().unwrap(), None);
        assert_eq!(broker.consistency_stop_incident().unwrap().cause, ConsistencyStopCause::CoverageLost);
        assert!(broker.forecast_learned_action(2, 0, &checked, row()).is_err());
        assert_eq!(broker.learned_consistency_work().unwrap(), report.work());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn full_inventory_including_unused_residuals_has_an_independent_cap() {
    let (checked, _) = source([1.0, 1.0], ResidualRetention::All, 1, 21);
    assert!(checked.report().total_encoded_bytes > checked.report().base_encoded_bytes);
    for short in [false, true] {
        let (mut broker, endpoint) = owner();
        let bytes = checked.report().total_encoded_bytes;
        broker.enable_learned_action_consistency_with_limits(registration(&checked, &[0.0, 0.0]),
            LearnedMonitorBudget::default(), bytes - usize::from(short)).unwrap();
        let result = broker.forecast_learned_action(1, 0, &checked, row());
        assert_eq!(broker.consistency.as_ref().unwrap().jobs, 1);
        assert_eq!(broker.consistency_evidence().unwrap().samples(), 0);
        if short {
            assert!(matches!(result, Err(Error::Limit)));
            assert!(broker.consistency_coverage_lost().unwrap());
            assert_eq!(broker.learned_consistency_retained_source_bytes().unwrap(), 0);
            assert_eq!(broker.learned_consistency_work().unwrap(), LearnedMonitorWork::default());
            assert!(matches!(broker.learned_consistency_report(1), Err(Error::Missing)));
        } else {
            let report = result.unwrap(); report.prediction().unwrap();
            assert_eq!(report.work().refinements, 0);
            assert_eq!(report.work().encoded_bytes, checked.report().base_encoded_bytes);
            assert_eq!(broker.learned_consistency_retained_source_bytes().unwrap(), bytes);
            assert!(!broker.consistency_coverage_lost().unwrap());
        }
        assert!(!broker.learned_consistency_has_unreported_work().unwrap());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn each_lifetime_work_cap_is_enforced_by_the_original_numerical_monitor() {
    let (checked, _) = source([1.0, 1.0], ResidualRetention::All, 1, 21);
    let model = config(&checked, &[0.0, 1.0], alpha()).model
        .into_learned(LearnedMonitorBudget::default()).unwrap();
    let complete = model.predict(&checked, row()).unwrap(); complete.prediction().unwrap();
    let work = complete.work();
    assert!(work.encoded_bytes > 0 && work.probe_coordinates > 0 && work.reconstruction_products > 0
        && work.materialized_values > 0 && work.refinements > 0);
    for field in 0..5 {
        let (mut broker, _) = owner();
        let mut lifetime = LearnedMonitorBudget::default();
        match field {
            0 => lifetime.encoded_bytes = work.encoded_bytes - 1,
            1 => lifetime.probe_coordinates = work.probe_coordinates - 1,
            2 => lifetime.reconstruction_products = work.reconstruction_products - 1,
            3 => lifetime.materialized_values = work.materialized_values - 1,
            _ => lifetime.refinements = work.refinements - 1,
        }
        broker.enable_learned_action_consistency_with_limits(registration(&checked, &[0.0, 1.0]),
            lifetime, checked.report().total_encoded_bytes).unwrap();
        let report = broker.forecast_learned_action(1, 0, &checked, row()).unwrap();
        assert_eq!(report.prediction().unwrap_err(), Error::Limit);
        assert_eq!(report.monitor().outcome(), MonitorOutcome::BudgetExhausted);
        assert_eq!(broker.learned_consistency_work().unwrap(), report.work());
        allowance(lifetime, report.work(), model.budget()).unwrap();
        assert!(broker.consistency_coverage_lost().unwrap());
        assert!(!broker.learned_consistency_has_unreported_work().unwrap());
        assert_eq!(broker.consistency_evidence().unwrap().samples(), 0);
    }
}

#[test]
fn second_forecast_uses_the_lifetime_remainder_not_a_fresh_budget() {
    let (first, _) = source([1.0, 1.0], ResidualRetention::All, 1, 21);
    let (second, image) = source([1.0, 1.0], ResidualRetention::All, 2, 21);
    let second_row = KvRow { position: 1, ..row() };
    let model = config(&first, &[0.0, 0.0], alpha()).model
        .into_learned(LearnedMonitorBudget::default()).unwrap();
    let first_cost = model.predict(&first, row()).unwrap().work();
    let second_cost = model.predict(&second, second_row).unwrap().work();
    let total = add_work(first_cost, second_cost).unwrap();
    let bytes = first.report().total_encoded_bytes + second.report().total_encoded_bytes;
    for short in [false, true] {
        let (mut broker, endpoint) = owner();
        let mut lifetime = LearnedMonitorBudget::default();
        lifetime.encoded_bytes = total.encoded_bytes - usize::from(short);
        broker.enable_learned_action_consistency_with_limits(registration(&first, &[0.0, 0.0]),
            lifetime, bytes).unwrap();
        broker.forecast_learned_action(1, 0, &first, row()).unwrap().prediction().unwrap();
        broker.propose(1, spec(0, b"safe"), &snapshot()).unwrap();
        assert_eq!(broker.learned_consistency_work().unwrap(), first_cost);
        // Advance through the original actor replacement path. This remains a
        // supplied reference capture, not an authenticated owned-decoder test.
        let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![2; image.len()],
            image.encode().unwrap(), vec![3], image.len() as u64).unwrap();
        broker.delivery.replace_actor_state(broker.actor_revision(), actor).unwrap();
        let report = broker.forecast_learned_action(2, broker.actor_revision(), &second, second_row).unwrap();
        assert_eq!(broker.learned_consistency_retained_source_bytes().unwrap(), bytes);
        assert_eq!(broker.learned_consistency_work().unwrap(), add_work(first_cost, report.work()).unwrap());
        assert_eq!(broker.learned_consistency_report(1).unwrap().work(), first_cost);
        assert_eq!(broker.learned_consistency_report(2).unwrap().work(), report.work());
        assert_eq!(broker.consistency_evidence().unwrap().samples(), 1);
        if short {
            assert_eq!(report.prediction().unwrap_err(), Error::Limit);
            assert_eq!(report.work(), LearnedMonitorWork::default());
            assert!(broker.consistency_coverage_lost().unwrap());
            assert_eq!(broker.pending_forecast().unwrap(), None);
            assert!(broker.propose(2, spec(0, b"safe"), &snapshot()).is_err());
        } else {
            report.prediction().unwrap();
            assert_eq!(broker.learned_consistency_work().unwrap(), total);
            broker.propose(2, spec(0, b"safe"), &snapshot()).unwrap();
            assert_eq!(broker.consistency_evidence().unwrap().samples(), 2);
            assert!(!broker.consistency_coverage_lost().unwrap());
        }
        assert!(!broker.learned_consistency_has_unreported_work().unwrap());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn invalid_configuration_is_atomic_and_lifetime_cannot_be_refilled() {
    let (checked, original) = source([1.0, 0.0], ResidualRetention::All, 1, 21);
    for invalid in 0..3 {
        let (mut broker, _) = owner();
        let mut settings = registration(&checked, &[1.0, 0.0]);
        match invalid {
            0 => settings.layer = 0,
            1 => settings.budget.encoded_bytes = MAX_CHECKED_KV_BYTES + 1,
            _ => settings.consistency.max_predictions = 0,
        }
        assert!(broker.enable_learned_action_consistency_with_limits(settings,
            LearnedMonitorBudget::default(), usize::MAX).is_err());
        assert!(!broker.action_consistency_required());
        broker.enable_action_consistency(config(&checked, &[1.0, 0.0], alpha())).unwrap();
        assert_eq!(broker.enable_learned_action_consistency_with_limits(registration(&checked, &[1.0, 0.0]),
            LearnedMonitorBudget::default(), usize::MAX), Err(Error::Duplicate));
        assert!(!broker.learned_action_consistency_required());
        broker.forecast_action(1, 0, raw(&original)).unwrap();
    }
    let (mut broker, _) = owner();
    let zero = scale_budget(LearnedMonitorBudget::default(), 0).unwrap();
    broker.enable_learned_action_consistency_with_limits(registration(&checked, &[1.0, 0.0]),
        zero, checked.report().total_encoded_bytes).unwrap();
    let report = broker.forecast_learned_action(1, 0, &checked, row()).unwrap();
    assert_eq!(report.prediction().unwrap_err(), Error::Limit);
    assert_eq!(broker.enable_learned_action_consistency(registration(&checked, &[1.0, 0.0])), Err(Error::Duplicate));
    assert!(broker.consistency_coverage_lost().unwrap());
    assert_eq!(broker.learned_consistency_report(1).unwrap().work(), report.work());
}
