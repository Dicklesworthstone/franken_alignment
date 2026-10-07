//! Original inference/fit/compression/refinement and exact forecast controls.
use super::*;
use crate::action::consequence::activation::{ProgressiveFrame,
    probe::learned::{KvGroup, ResidualRetention},
    tensor::kv::experiment::{KvCell, KvSide}};
use super::super::{ErrorBudget, LikelihoodEvidence, progressive::ProgressiveForecastPolicy};
mod fixture;
use fixture::*;

#[test]
fn certified_coarse_bands_use_the_original_table_without_materializing_residuals() {
    let (source, original) = source([1.0, 0.0], ResidualRetention::None);
    for side in [KvSide::Key, KvSide::Value] {
        for weights in [[1.0, 0.0], [-1.0, 0.0], [0.0, 0.0]] {
            let row = row(side); let exact = model(&source, row, &weights);
            let expected = exact.predict(raw(&original, row)).unwrap();
            let learned = exact.into_learned(LearnedMonitorBudget::default()).unwrap();
            let report = learned.predict(&source, row).unwrap();
            let actual = report.prediction().unwrap();
            assert_eq!(actual.forecast(), expected.forecast());
            assert_eq!(actual.observation().outcome(), expected.observation().outcome());
            assert_eq!(actual.observation().frame(), expected.observation().frame());
            assert_eq!(actual.domain(), expected.domain()); assert_eq!(actual.generation(), expected.generation());
            assert_eq!(actual.policy_generation(), expected.policy_generation());
            assert_eq!(actual.encoded_bytes(), source.report().base_encoded_bytes);
            assert_eq!(actual.work().refinements, 0); assert_eq!(actual.work().materialized_values, 0);
            assert_eq!(report.monitor().steps().len(), 1);
            assert_eq!(report.work(), actual.work());
        }
    }
}

#[test]
fn lost_latent_coordinate_is_refined_before_positive_negative_or_equality_forecast() {
    let (source, original) = source([1.0, 1.0], ResidualRetention::All);
    let row = row(KvSide::Key);
    assert_eq!(source.image().bits(1, KvCell { side: KvSide::Key, position: 0, head: 0, channel: 1 }).unwrap(), 0);
    let before = source.encode().unwrap();
    for weights in [[0.0, 1.0], [0.0, -1.0], [1.0, -1.0]] {
        let exact = model(&source, row, &weights);
        let expected = exact.predict(raw(&original, row)).unwrap();
        let native = LearnedRefinementMonitor::new(vec![exact.probe.clone()], LearnedMonitorBudget::default()).unwrap()
            .analyze(&source, row).unwrap();
        let learned = exact.into_learned(LearnedMonitorBudget::default()).unwrap();
        let report = learned.predict(&source, row).unwrap(); let actual = report.prediction().unwrap();
        assert_eq!(report.monitor().steps()[0].observations[0].outcome(), ProbeOutcome::NeedsRefinement);
        assert_eq!(actual.forecast(), expected.forecast());
        assert_eq!(actual.observation().interval(), expected.observation().interval());
        assert_eq!(report.work(), native.work()); assert_eq!(report.monitor().outcome(), native.outcome());
        assert_eq!(report.work().refinements, 1); assert_eq!(report.work().materialized_values, 2);
        let group = KvGroup { row, head: 0 };
        assert_eq!(actual.encoded_bytes(), source.report().base_encoded_bytes + source.residual_bytes(group).unwrap().len());
        assert!(actual.observation().view().is_refined(group).unwrap());
        assert!(!actual.observation().view().is_refined(KvGroup { row: fixture::row(KvSide::Value), head: 0 }).unwrap());
        assert_eq!(source.view().revision(), 0, "forecast refinement is local, not a source mutation");
        assert_eq!(source.encode().unwrap(), before);
    }
}

#[test]
fn absent_exact_escape_preserves_unresolved_work_instead_of_a_neutral_forecast() {
    let (source, original) = source([1.0, 1.0], ResidualRetention::None);
    let row = row(KvSide::Key); let exact = model(&source, row, &[0.0, 1.0]);
    assert_eq!(exact.predict(raw(&original, row)).unwrap().observation().outcome(), ProbeOutcome::CertifiedAlarm);
    let learned = exact.into_learned(LearnedMonitorBudget::default()).unwrap();
    let report = learned.predict(&source, row).unwrap();
    assert_eq!(report.prediction().err(), Some(Error::Incomplete));
    assert_eq!(report.monitor().outcome(), MonitorOutcome::Unresolved);
    assert_eq!(report.monitor().unavailable_groups(), &[KvGroup { row, head: 0 }]);
    assert_eq!(report.work().encoded_bytes, source.report().base_encoded_bytes);
    assert!(report.work().probe_coordinates > 0); assert!(report.work().reconstruction_products > 0);
    assert_eq!(report.work().refinements, 0);
}

#[test]
fn each_frozen_allowance_refuses_before_the_next_original_operation_and_keeps_partial_work() {
    let (source, _) = source([1.0, 1.0], ResidualRetention::All); let row = row(KvSide::Key);
    let exact = model(&source, row, &[0.0, 1.0]);
    let generous = exact.clone().into_learned(LearnedMonitorBudget::default()).unwrap();
    let complete = generous.predict(&source, row).unwrap(); let work = complete.work();
    assert!(complete.prediction().is_ok());
    for limit in 0..5 {
        let mut budget = LearnedMonitorBudget::default();
        match limit {
            0 => budget.encoded_bytes = work.encoded_bytes - 1,
            1 => budget.probe_coordinates = work.probe_coordinates - 1,
            2 => budget.reconstruction_products = work.reconstruction_products - 1,
            3 => budget.materialized_values = work.materialized_values - 1,
            _ => budget.refinements = work.refinements - 1,
        }
        let learned = exact.clone().into_learned(budget).unwrap();
        let report = learned.predict(&source, row).unwrap();
        assert_eq!(report.prediction().err(), Some(Error::Limit));
        assert_eq!(report.monitor().outcome(), MonitorOutcome::BudgetExhausted);
        let used = report.work();
        assert!(used.encoded_bytes <= budget.encoded_bytes && used.probe_coordinates <= budget.probe_coordinates);
        assert!(used.reconstruction_products <= budget.reconstruction_products);
        assert!(used.materialized_values <= budget.materialized_values && used.refinements <= budget.refinements);
        assert_eq!(source.view().revision(), 0);
    }
    let zero = exact.clone().into_learned(LearnedMonitorBudget { encoded_bytes: 0, ..LearnedMonitorBudget::default() }).unwrap();
    let report = zero.predict(&source, row).unwrap();
    assert_eq!(report.prediction().err(), Some(Error::Limit));
    assert_eq!(report.work(), LearnedMonitorWork::default()); assert!(report.monitor().steps().is_empty());
    // A distinct construction is a numerical control, not a live retry or a
    // permission to increase a once-admitted request's allowance.
    assert_eq!(generous.predict(&source, row).unwrap().prediction().unwrap().work(), work);
}

#[test]
fn actual_subnormal_signal_cannot_be_replaced_by_the_codecs_zero_reconstruction() {
    let (source, original) = source([1.0, f32::from_bits(1)], ResidualRetention::All);
    let row = row(KvSide::Value); let raw = raw(&original, row);
    let values = ProgressiveFrame::from_initial(&raw.verify_block(&raw.encode_initial(23).unwrap()).unwrap())
        .unwrap().exact_values().unwrap();
    assert!(values[1] > 0.0); assert!(values[1] < f32::MIN_POSITIVE);
    assert_eq!(source.image().bits(1, KvCell { side: KvSide::Value, position: 0, head: 0, channel: 1 }).unwrap(), 0);
    let exact = model(&source, row, &[0.0, f32::from_bits(1)]);
    let expected = exact.predict(raw).unwrap();
    let report = exact.into_learned(LearnedMonitorBudget::default()).unwrap().predict(&source, row).unwrap();
    assert_eq!(report.prediction().unwrap().observation().interval(), expected.observation().interval());
    assert_eq!(report.prediction().unwrap().observation().outcome(), ProbeOutcome::CertifiedAlarm);
    assert!(report.work().refinements > 0);
}

#[test]
fn different_spaces_missing_rows_and_mantissa_policies_cannot_be_relabelled_as_learned_evidence() {
    let (source, _) = source([1.0, 1.0], ResidualRetention::All); let row = row(KvSide::Key);
    let exact = model(&source, row, &[0.0, 1.0]);
    let learned = exact.clone().into_learned(LearnedMonitorBudget::default()).unwrap();
    assert_eq!(learned.predict(&source, KvRow { position: 1, ..row }).err(), Some(Error::Missing));
    assert_eq!(learned.predict(&source, KvRow { side: KvSide::Value, ..row }).err(), Some(Error::Binding));
    let wrong_width = model(&source, row, &[1.0]).into_learned(LearnedMonitorBudget::default()).unwrap();
    assert_eq!(wrong_width.predict(&source, row).err(), Some(Error::Binding));
    let policy = ProgressiveForecastPolicy::new(0, 1, 23, 4096).unwrap();
    assert!(matches!(exact.clone().with_progressive(policy).unwrap().into_learned(LearnedMonitorBudget::default()),
        Err(Error::Binding)));
    let invalid = LearnedMonitorBudget { refinements: usize::MAX, ..LearnedMonitorBudget::default() };
    assert!(matches!(exact.into_learned(invalid), Err(Error::Limit)));
    assert!(learned.predict(&source, row).unwrap().prediction().is_ok());
}

#[test]
fn original_likelihood_factors_counts_and_crossing_match_the_exact_source_control() {
    let (source, original) = source([1.0, 1.0], ResidualRetention::All); let row = row(KvSide::Key);
    let exact = model(&source, row, &[0.0, 1.0]);
    let learned = exact.clone().into_learned(LearnedMonitorBudget::default()).unwrap();
    let mut observed = LikelihoodEvidence::new(ErrorBudget::new(1, 4).unwrap());
    let mut control = observed.clone();
    for (index, payload) in [b"publish x".as_slice(), b"publish y", b"other", b"publish z"].into_iter().enumerate() {
        let report = learned.predict(&source, row).unwrap(); let predicted = report.prediction().unwrap();
        let expected = exact.predict(raw(&original, row)).unwrap();
        assert_eq!(learned.event(payload), exact.event(payload));
        assert_eq!(observed.observe(predicted.forecast(), learned.event(payload)).unwrap(),
            control.observe(expected.forecast(), exact.event(payload)).unwrap());
        assert_eq!(observed, control); assert_eq!(observed.samples(), index + 1);
        assert!(report.monitor().steps().len() > 1, "internal refinement is not an extra evidence sample");
    }
    assert_eq!(observed.first_crossing(), Some(2));
}
