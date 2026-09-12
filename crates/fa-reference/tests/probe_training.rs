#[path = "support/probe_corpus.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::{FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::probe::ProbeOutcome;
use fa_reference::action::consequence::activation::probe::training::*;
use fa_reference::Error;
use std::collections::BTreeMap;

#[test]
fn actual_class_balanced_fit_learns_direction_and_constant_coordinate_is_neutral() {
    let corpus = corpus(1.0, false);
    let trained = fit(&corpus);
    assert!(trained.weights()[0] > 0.0);
    assert_eq!(trained.weights()[1], 0.0);
    assert_eq!(trained.training_mean(), &[0.0, 7.0]);
    assert_eq!(trained.training_scale(), &[2.0, 0.001]);
    let probe = trained.probe(0.0).unwrap();
    assert_eq!(outcome(&probe, &[-1.0, 7.0]), ProbeOutcome::CertifiedQuiet);
    assert_eq!(outcome(&probe, &[1.0, 7.0]), ProbeOutcome::CertifiedAlarm);
    assert_eq!(outcome(&trained.probe(trained.bias()).unwrap(), &[0.0, 7.0]), ProbeOutcome::AtThreshold);
    assert_eq!(probe.identity().profile, corpus.profile());
    assert_eq!(probe.identity().dimensions, 2);
    assert_eq!(probe.identity().id, 30);
    assert_eq!(trained.work().classes, ClassCounts { benign: 2, violation: 2 });
}

#[test]
fn changing_heldout_values_does_not_change_normalization_or_fitted_coefficients() {
    let first = fit(&corpus(1.0, false));
    let changed = fit(&corpus(-1e12, false));
    assert_eq!(first.weights().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        changed.weights().iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    assert_eq!(first.bias().to_bits(), changed.bias().to_bits());
    assert_eq!(first.training_mean(), changed.training_mean());
    assert_eq!(first.training_scale(), changed.training_scale());
    assert_eq!(first.work(), changed.work());
}

#[test]
fn training_labels_not_a_hardcoded_direction_determine_the_result() {
    let first = fit(&corpus(1.0, false));
    let changed = fit(&corpus(1.0, true));
    assert!(first.weights()[0] > 0.0 && changed.weights()[0] < 0.0);
    assert_eq!(outcome(&changed.probe(0.0).unwrap(), &[1.0, 7.0]), ProbeOutcome::CertifiedQuiet);
    assert_eq!(outcome(&changed.probe(0.0).unwrap(), &[-1.0, 7.0]), ProbeOutcome::CertifiedAlarm);
}

#[test]
fn original_task_and_attack_lineage_each_cannot_cross_or_duplicate_splits() {
    for same_task in [false, true] {
        let mut planned = assignments();
        planned.remove(&origin(5));
        let alias = if same_task { CaseOrigin { task: 1, lineage: 900 } }
            else { CaseOrigin { task: 900, lineage: 101 } };
        planned.insert(alias, DataSplit::Calibration);
        assert_eq!(ProbeCorpus::new(1, 1, profile(), 2, planned).unwrap_err(), Error::Duplicate);
    }
    let mut missing_split = assignments();
    missing_split.retain(|_, split| *split != DataSplit::Evaluation);
    assert_eq!(ProbeCorpus::new(1, 1, profile(), 2, missing_split).unwrap_err(), Error::Incomplete);
    assert!(ProbeCorpus::new(1, 1, profile(), 2, assignments()).is_ok());
}

#[test]
fn missing_case_cannot_be_dropped_and_late_completion_keeps_its_declared_split() {
    let mut builder = ProbeCorpus::new(1, 1, profile(), 2, assignments()).unwrap();
    for id in 1..12 { builder.capture(origin(id), label(id), source(id, &[id as f32, 7.0])).unwrap(); }
    assert_eq!(builder.seal().unwrap_err(), Error::Incomplete);
    assert_eq!(builder.planned_cases(), 12); assert_eq!(builder.captured_cases(), 11);
    assert_eq!(builder.capture(origin(99), CaseLabel::Benign, source(99, &[1.0, 7.0])), Err(Error::Missing));
    builder.capture(origin(12), label(12), source(12, &[12.0, 7.0])).unwrap();
    let corpus = builder.seal().unwrap();
    assert_eq!(corpus.counts(DataSplit::Evaluation), ClassCounts { benign: 2, violation: 2 });
    assert_eq!(corpus.assignments().collect::<BTreeMap<_, _>>(), assignments());
}

#[test]
fn duplicate_labels_and_same_capture_under_another_origin_cannot_replace_evidence() {
    let mut builder = ProbeCorpus::new(1, 1, profile(), 2, assignments()).unwrap();
    let frame = source(1, &[-2.0, 7.0]);
    builder.capture(origin(1), CaseLabel::Benign, frame.clone()).unwrap();
    assert_eq!(builder.capture(origin(1), CaseLabel::Violation, frame.clone()), Err(Error::Duplicate));
    assert_eq!(builder.capture(origin(2), CaseLabel::Violation, frame), Err(Error::Duplicate));
    assert_eq!(builder.captured_cases(), 1);
    builder.capture(origin(2), CaseLabel::Violation, source(2, &[2.0, 7.0])).unwrap();
    assert_eq!(builder.captured_cases(), 2);
}

#[test]
fn profile_dimension_and_aggregate_caps_are_checked_before_retention() {
    let mut builder = ProbeCorpus::new(1, 1, profile(), 2, assignments()).unwrap();
    assert_eq!(builder.capture(origin(1), label(1), source(1, &[1.0])), Err(Error::Binding));
    let mut wrong = profile(); wrong.model_generation += 1;
    let frame = SourceFrame::capture(FrameIdentity { profile: wrong, stream: 1, sequence: 1, position: 0 }, &[1.0, 2.0]).unwrap();
    assert_eq!(builder.capture(origin(1), label(1), frame), Err(Error::Binding));
    assert_eq!(builder.captured_cases(), 0);
    let planned: BTreeMap<_, _> = (1..=17).map(|id| (origin(id), split(id))).collect();
    assert_eq!(ProbeCorpus::new(1, 1, profile(), 65_536, planned).unwrap_err(), Error::Limit);
    let planned: BTreeMap<_, _> = (1..=16).map(|id| (origin(id), split(id))).collect();
    assert!(ProbeCorpus::new(1, 1, profile(), 65_536, planned).is_ok());
}

#[test]
fn both_class_denominators_are_required_in_every_split() {
    for deficient in [DataSplit::Training, DataSplit::Calibration, DataSplit::Evaluation] {
        let mut builder = ProbeCorpus::new(1, 1, profile(), 2, assignments()).unwrap();
        for id in 1..=12 {
            let class = if split(id) == deficient { CaseLabel::Benign } else { label(id) };
            builder.capture(origin(id), class, source(id, &[id as f32, 7.0])).unwrap();
        }
        assert_eq!(builder.seal().unwrap_err(), Error::Incomplete);
    }
}

#[test]
fn complete_fit_budget_exact_boundary_and_one_under_have_same_immutable_input() {
    let corpus = corpus(1.0, false); let policy = policy();
    let work = corpus.estimate_fit(&policy).unwrap();
    assert_eq!(work.source_coordinate_visits, 4 * 2 * (1 + 2 * 100));
    assert_eq!(work.parameter_updates, 200); assert_eq!(work.sigmoid_evaluations, 400);
    assert_eq!(corpus.fit(policy.clone(), TrainingBudget {
        source_coordinate_visits: work.source_coordinate_visits - 1,
    }).unwrap_err(), Error::Limit);
    assert_eq!(corpus.fit(policy.clone(), TrainingBudget {
        source_coordinate_visits: MAX_TRAINING_VISITS + 1,
    }).unwrap_err(), Error::Limit);
    let trained = corpus.fit(policy, TrainingBudget { source_coordinate_visits: work.source_coordinate_visits }).unwrap();
    assert_eq!(trained.work(), work);
    assert_eq!(trained.weights(), fit(&corpus).weights());
}

#[test]
fn policy_and_emitted_threshold_validation_do_not_create_fallback_detectors() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(FitPolicy::new(1, 1, 10, bad, 0.1, 0.001).is_err());
    }
    assert!(FitPolicy::new(1, 1, 0, 0.1, 0.1, 0.001).is_err());
    assert!(FitPolicy::new(1, 1, 4097, 0.1, 0.1, 0.001).is_err());
    assert!(FitPolicy::new(1, 1, 10, 0.1, -0.1, 0.001).is_err());
    let fitted = fit(&corpus(1.0, false));
    assert!(fitted.probe(f32::NAN).is_err());
    assert!(fitted.probe(0.0).is_ok());
}

#[test]
fn immutable_fit_survives_all_builder_and_corpus_owner_drops() {
    let fitted = { let input = corpus(1.0, false); fit(&input) };
    let copy = fitted.clone(); drop(fitted);
    assert_eq!(copy.corpus().id(), 1);
    assert_eq!(copy.corpus().counts(DataSplit::Training).total(), 4);
    assert_eq!(outcome(&copy.probe(0.0).unwrap(), &[1.0, 7.0]), ProbeOutcome::CertifiedAlarm);
}
