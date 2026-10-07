//! Shared caller allowances reuse original intersections and original costs.
use super::*;
use crate::action::consequence::activation::{probe::learned::ResidualRetention,
    tensor::kv::experiment::KvSide};
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;

fn allowance(work: LearnedMonitorWork) -> LearnedMonitorBudget {
    LearnedMonitorBudget { encoded_bytes: work.encoded_bytes, probe_coordinates: work.probe_coordinates,
        reconstruction_products: work.reconstruction_products, materialized_values: work.materialized_values,
        refinements: work.refinements }
}
fn less(mut budget: LearnedMonitorBudget, coordinate: usize) -> LearnedMonitorBudget {
    match coordinate {
        0 => budget.encoded_bytes -= 1, 1 => budget.probe_coordinates -= 1,
        2 => budget.reconstruction_products -= 1, 3 => budget.materialized_values -= 1,
        _ => budget.refinements -= 1,
    }
    budget
}

#[test]
fn exact_shared_allowance_matches_both_original_monitor_and_default_forecast() {
    let (source, original) = source([1.0, 1.0], ResidualRetention::All);
    let row = row(KvSide::Key); let raw_model = model(&source, row, &[0.0, 1.0]);
    let learned = raw_model.clone().into_learned(LearnedMonitorBudget::default()).unwrap();
    let baseline = learned.predict(&source, row).unwrap(); let budget = allowance(baseline.work());
    let limited = learned.predict_with_budget(&source, row, budget).unwrap();
    let control = LearnedRefinementMonitor::new(vec![raw_model.probe.clone()], LearnedMonitorBudget::default()).unwrap()
        .analyze_with_budget(&source, row, budget).unwrap();
    assert_eq!(limited.work(), control.work()); assert_eq!(limited.work(), baseline.work());
    assert_eq!(limited.monitor().steps().len(), control.steps().len());
    assert_eq!(limited.prediction().unwrap().observation().interval(), baseline.prediction().unwrap().observation().interval());
    assert_eq!(limited.prediction().unwrap().forecast(), raw_model.predict(raw(&original, row)).unwrap().forecast());
    assert_eq!(limited.prediction().unwrap().encoded_bytes(), budget.encoded_bytes);
    assert_eq!(source.view().revision(), 0);
}

#[test]
fn larger_caller_allowances_cannot_expand_any_frozen_model_limit() {
    let (source, _) = source([1.0, 1.0], ResidualRetention::All); let row = row(KvSide::Value);
    let raw_model = model(&source, row, &[0.0, 1.0]);
    let full = raw_model.clone().into_learned(LearnedMonitorBudget::default()).unwrap().predict(&source, row).unwrap();
    assert!(full.prediction().is_ok());
    for field in 0..5 {
        let frozen = less(allowance(full.work()), field);
        let model = raw_model.clone().into_learned(frozen).unwrap();
        let report = model.predict_with_budget(&source, row, LearnedMonitorBudget::default()).unwrap();
        let native = LearnedRefinementMonitor::new(vec![raw_model.probe.clone()], frozen).unwrap()
            .analyze_with_budget(&source, row, LearnedMonitorBudget::default()).unwrap();
        assert_eq!(report.prediction().err(), Some(Error::Limit));
        assert_eq!(report.monitor().outcome(), native.outcome()); assert_eq!(report.work(), native.work());
        assert_eq!(model.budget(), frozen, "per-call allowances never rewrite registration");
    }
}

#[test]
fn smaller_shared_limits_stop_the_original_operation_without_erasing_partial_work() {
    let (source, _) = source([1.0, 1.0], ResidualRetention::All); let row = row(KvSide::Key);
    let raw_model = model(&source, row, &[0.0, 1.0]);
    let model = raw_model.clone().into_learned(LearnedMonitorBudget::default()).unwrap();
    let full = model.predict(&source, row).unwrap(); let exact = allowance(full.work());
    for field in 0..5 {
        let remaining = less(exact, field);
        let report = model.predict_with_budget(&source, row, remaining).unwrap();
        let native = LearnedRefinementMonitor::new(vec![raw_model.probe.clone()], model.budget()).unwrap()
            .analyze_with_budget(&source, row, remaining).unwrap();
        assert_eq!(report.prediction().err(), Some(Error::Limit)); assert_eq!(report.work(), native.work());
        assert_eq!(report.monitor().steps().len(), native.steps().len());
        assert_eq!(model.budget(), LearnedMonitorBudget::default());
        assert!(report.work().encoded_bytes > 0 && report.work().probe_coordinates > 0);
    }
    let empty = allowance(LearnedMonitorWork::default());
    let report = model.predict_with_budget(&source, row, empty).unwrap();
    assert_eq!(report.prediction().err(), Some(Error::Limit));
    assert!(report.monitor().steps().is_empty()); assert_eq!(report.work(), LearnedMonitorWork::default());
}

#[test]
fn repeated_checked_source_acquisition_charges_base_and_residual_again_without_hidden_cache_credit() {
    let (source, _) = source([1.0, 1.0], ResidualRetention::All); let row = row(KvSide::Key);
    let model = model(&source, row, &[0.0, 1.0]).into_learned(LearnedMonitorBudget::default()).unwrap();
    let first = model.predict(&source, row).unwrap(); let work = first.work();
    assert!(first.prediction().is_ok()); assert_eq!(work.refinements, 1);
    let mut remaining = LearnedMonitorBudget { encoded_bytes: 2 * work.encoded_bytes,
        probe_coordinates: 2 * work.probe_coordinates, reconstruction_products: 2 * work.reconstruction_products,
        materialized_values: 2 * work.materialized_values, refinements: 2 * work.refinements };
    // The caller owns this shared allowance. Two complete acquisitions cost two
    // original reports; an old report cannot be installed as an uncharged view.
    for _ in 0..2 {
        let report = model.predict_with_budget(&source, row, remaining).unwrap();
        assert!(report.prediction().is_ok()); assert_eq!(report.work(), work);
        remaining.encoded_bytes -= work.encoded_bytes; remaining.probe_coordinates -= work.probe_coordinates;
        remaining.reconstruction_products -= work.reconstruction_products;
        remaining.materialized_values -= work.materialized_values; remaining.refinements -= work.refinements;
    }
    assert_eq!(remaining, allowance(LearnedMonitorWork::default()));
    let refused = model.predict_with_budget(&source, row, remaining).unwrap();
    assert_eq!(refused.prediction().err(), Some(Error::Limit));
    assert_eq!(refused.work(), LearnedMonitorWork::default());
    assert_eq!(source.view().revision(), 0); assert_eq!(first.work(), work);
}

#[test]
fn missing_evidence_and_malformed_remaining_caps_do_not_turn_into_forecasts() {
    let (source, _) = source([1.0, 1.0], ResidualRetention::None); let row = row(KvSide::Value);
    let raw_model = model(&source, row, &[0.0, 1.0]);
    let learned = raw_model.into_learned(LearnedMonitorBudget::default()).unwrap();
    let before = source.encode().unwrap();
    let invalid = LearnedMonitorBudget { encoded_bytes: usize::MAX, ..learned.budget() };
    assert_eq!(learned.predict_with_budget(&source, row, invalid).err(), Some(Error::Limit));
    let report = learned.predict_with_budget(&source, row, learned.budget()).unwrap();
    assert_eq!(report.prediction().err(), Some(Error::Incomplete));
    assert_eq!(report.monitor().unavailable_groups().len(), 1);
    assert!(report.work().encoded_bytes > 0 && report.work().probe_coordinates > 0);
    assert_eq!(source.encode().unwrap(), before);
    // Exact constant-equality is a valid control even without residuals. It
    // still pays the original base and every registered coefficient coordinate.
    let constant = model(&source, row, &[0.0, 0.0]).into_learned(LearnedMonitorBudget::default()).unwrap();
    let report = constant.predict_with_budget(&source, row, LearnedMonitorBudget {
        encoded_bytes: source.report().base_encoded_bytes, probe_coordinates: 2,
        reconstruction_products: 0, materialized_values: 0, refinements: 0 }).unwrap();
    assert_eq!(report.prediction().unwrap().observation().outcome(), ProbeOutcome::AtThreshold);
    assert_eq!(report.work().encoded_bytes, source.report().base_encoded_bytes);
    assert_eq!(report.work().probe_coordinates, 2);
}
