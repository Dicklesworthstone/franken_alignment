#[path = "support/probe_corpus.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::HEADER_BYTES;
use fa_reference::action::consequence::activation::probe::{ProbeOutcome, training::*};
use fa_reference::action::consequence::activation::probe::training::calibration::*;
use fa_reference::Error;

fn allowance(cases: usize, thresholds: usize) -> ScoringBudget {
    ScoringBudget { encoded_bytes: cases * (HEADER_BYTES + 8), probe_coordinates: cases * 2,
        threshold_comparisons: cases * thresholds }
}
fn criteria() -> ScreeningCriteria { ScreeningCriteria::new(2, 0).unwrap() }
fn campaign() -> CalibrationPolicy {
    CalibrationPolicy::new(4, 1, &[-100.0, 0.0, 100.0], criteria(), criteria()).unwrap()
}
fn input(reverse_evaluation: bool, zero_calibration: bool) -> SealedCorpus {
    let mut builder = ProbeCorpus::new(1, 1, profile(), 2, assignments()).unwrap();
    for id in 1..=12 {
        let mut x = if id % 2 == 0 { 2.0 } else { -2.0 };
        if reverse_evaluation && split(id) == DataSplit::Evaluation { x = -x; }
        if zero_calibration && split(id) == DataSplit::Calibration { x = 0.0; }
        builder.capture(origin(id), label(id), source(id, &[x, 7.0])).unwrap();
    }
    builder.seal().unwrap()
}

#[test]
fn calibration_selects_usable_operating_point_and_final_report_exports_original_probe() {
    let fitted = fit(&input(false, false));
    let run = fitted.calibrate(campaign(), allowance(4, 3)).unwrap();
    assert_eq!(run.selected_threshold(), Some(0.0));
    assert_eq!(run.trials().len(), 3);
    assert!(!run.trials()[0].accepted()); assert!(run.trials()[1].accepted()); assert!(!run.trials()[2].accepted());
    assert_eq!(run.trials()[0].counts().benign_alarm, 2);
    assert_eq!(run.trials()[2].counts().violation_quiet, 2);
    assert_eq!(run.scores().iter().map(|s| s.origin()).collect::<Vec<_>>(), (5..=8).map(origin).collect::<Vec<_>>());
    let evaluated = run.evaluate(allowance(4, 1)).unwrap();
    assert!(evaluated.accepted());
    assert_eq!(evaluated.counts().classes(), ClassCounts { benign: 2, violation: 2 });
    assert_eq!(evaluated.counts().violation_alarm, 2);
    assert_eq!(evaluated.counts().benign_quiet, 2);
    assert_eq!(evaluated.scores().iter().map(|s| s.origin()).collect::<Vec<_>>(), (9..=12).map(origin).collect::<Vec<_>>());
    assert_eq!(outcome(&evaluated.probe().unwrap(), &[1.0, 7.0]), ProbeOutcome::CertifiedAlarm);
    assert_eq!(outcome(&evaluated.probe().unwrap(), &[-1.0, 7.0]), ProbeOutcome::CertifiedQuiet);
}

#[test]
fn changed_final_population_cannot_change_training_or_threshold_and_failure_is_retained() {
    let good = fit(&input(false, false)).calibrate(campaign(), allowance(4, 3)).unwrap();
    let bad = fit(&input(true, false)).calibrate(campaign(), allowance(4, 3)).unwrap();
    assert_eq!(good.fitted().weights(), bad.fitted().weights());
    assert_eq!(good.fitted().bias(), bad.fitted().bias());
    assert_eq!(good.scores(), bad.scores()); assert_eq!(good.trials(), bad.trials());
    assert_eq!(good.selected_threshold(), bad.selected_threshold());
    assert!(good.evaluate(allowance(4, 1)).unwrap().accepted());
    let failed = bad.evaluate(allowance(4, 1)).unwrap();
    assert!(!failed.accepted()); assert_eq!(failed.counts().violation_quiet, 2);
    assert_eq!(failed.counts().benign_alarm, 2);
    assert_eq!(failed.probe().unwrap_err(), Error::WrongState);
    assert_eq!(failed.calibration().selected_threshold(), Some(0.0));
    assert_eq!(failed.scores().len(), 4);
}

#[test]
fn exact_threshold_equality_is_a_hold_not_quiet_or_true_alarm() {
    let fitted = fit(&input(false, true));
    let policy = CalibrationPolicy::new(1, 1, &[fitted.bias()], criteria(), criteria()).unwrap();
    let run = fitted.calibrate(policy, allowance(4, 1)).unwrap();
    let counts = run.trials()[0].counts();
    assert_eq!(counts.benign_boundary, 2); assert_eq!(counts.violation_boundary, 2);
    assert_eq!(counts.benign_holds(), 2); assert_eq!(counts.violation_alarm, 0);
    assert_eq!(counts.classes().total(), 4);
    assert_eq!(run.selected_threshold(), None);
    assert!(matches!(run.evaluate(allowance(4, 1)), Err(Error::WrongState)));
    let permissive = ScreeningCriteria::new(2, 2).unwrap();
    let control = fitted.calibrate(CalibrationPolicy::new(2, 1, &[fitted.bias() - 1.0], permissive, criteria()).unwrap(), allowance(4, 1)).unwrap();
    assert!(control.selected_threshold().is_some());
    assert_eq!(control.trials()[0].counts().violation_alarm, 2);
}

#[test]
fn no_eligible_threshold_retains_all_denominators_and_does_not_fabricate_a_candidate() {
    let fitted = fit(&corpus(1.0, false));
    let p = CalibrationPolicy::new(1, 1, &[100.0, 200.0], criteria(), criteria()).unwrap();
    let run = fitted.calibrate(p, allowance(4, 2)).unwrap();
    assert_eq!(run.selected_threshold(), None);
    assert_eq!(run.trials().len(), 2);
    for trial in run.trials() {
        assert_eq!(trial.counts().violation_quiet, 2);
        assert_eq!(trial.counts().classes().total(), 4);
    }
}

#[test]
fn whole_grid_and_final_population_work_are_admitted_before_scoring() {
    let fitted = fit(&corpus(1.0, false));
    let full = allowance(4, 3);
    for which in 0..3 {
        let mut short = full;
        match which { 0 => short.encoded_bytes -= 1, 1 => short.probe_coordinates -= 1, _ => short.threshold_comparisons -= 1 }
        assert!(matches!(fitted.calibrate(campaign(), short), Err(Error::Limit)));
    }
    let run = fitted.calibrate(campaign(), full).unwrap();
    assert_eq!(run.work(), ScoringWork { cases: 4, encoded_bytes: full.encoded_bytes,
        probe_coordinates: full.probe_coordinates, threshold_comparisons: full.threshold_comparisons });
    let mut short = allowance(4, 1); short.probe_coordinates -= 1;
    assert!(matches!(run.evaluate(short), Err(Error::Limit)));
    assert!(run.evaluate(allowance(4, 1)).unwrap().accepted());
}

#[test]
fn frozen_evaluation_rule_can_fail_even_when_threshold_selection_succeeded() {
    let fitted = fit(&corpus(1.0, false));
    let strict = ScreeningCriteria::new(3, 0).unwrap();
    let p = CalibrationPolicy::new(1, 1, &[0.0], criteria(), strict).unwrap();
    let run = fitted.calibrate(p, allowance(4, 1)).unwrap();
    assert_eq!(run.selected_threshold(), Some(0.0));
    let evaluated = run.evaluate(allowance(4, 1)).unwrap();
    assert!(!evaluated.accepted()); assert_eq!(evaluated.counts().violation_alarm, 2);
    assert_eq!(evaluated.calibration().policy().evaluation_criteria(), strict);
    assert!(evaluated.probe().is_err());
}

#[test]
fn threshold_ties_and_malformed_policies_have_explicit_deterministic_behavior() {
    let fitted = fit(&corpus(1.0, false));
    let p = CalibrationPolicy::new(1, 1, &[-1.0, 0.0, 1.0], criteria(), criteria()).unwrap();
    let run = fitted.calibrate(p, allowance(4, 3)).unwrap();
    assert!(run.trials().iter().all(|trial| trial.accepted()));
    assert_eq!(run.selected_threshold(), Some(-1.0));
    for grid in [vec![], vec![0.0, -0.0], vec![1.0, 0.0], vec![f32::NAN], vec![f32::INFINITY]] {
        assert!(CalibrationPolicy::new(1, 1, &grid, criteria(), criteria()).is_err());
    }
    assert!(ScreeningCriteria::new(0, 0).is_err());
    assert!(CalibrationPolicy::new(0, 1, &[0.0], criteria(), criteria()).is_err());
}

#[test]
fn optimizer_overflow_returns_no_candidate_and_does_not_mutate_retained_input() {
    let input = corpus(1.0, false);
    let policy = FitPolicy::new(1, 1, 512, 1.0, 1000.0, 0.001).unwrap();
    let work = input.estimate_fit(&policy).unwrap();
    assert!(matches!(input.fit(policy, TrainingBudget { source_coordinate_visits: work.source_coordinate_visits }), Err(Error::Overflow)));
    let fitted = fit(&input);
    assert_eq!(outcome(&fitted.probe(0.0).unwrap(), &[1.0, 7.0]), ProbeOutcome::CertifiedAlarm);
}
