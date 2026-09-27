//! Actual native fitting and held-out compression; synthetic model, not calibration.
#[allow(dead_code)]
#[path = "support/restart_model.rs"]
mod support;
use fa_reference::Error;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS},
    model::{ModelKvImage, learned::{CompressionBudget, FitBudget, LearnedKvCodec, LearnedKvPolicy}},
};
use std::collections::BTreeMap;

fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn policy() -> LearnedKvPolicy { LearnedKvPolicy::new(7, 2, 1, 8).unwrap() }
fn corpus(model: &DecoderModel) -> BTreeMap<u64, ModelKvImage> {
    BTreeMap::from([
        (101, model.recompute(11, &[0, 1], inference()).unwrap().cache_image().unwrap()),
        (102, model.recompute(12, &[1, 0, 2], inference()).unwrap().cache_image().unwrap()),
    ])
}
fn same(a: &LearnedKvCodec, b: &LearnedKvCodec) {
    assert_eq!(a.policy(), b.policy()); assert_eq!(a.profile(), b.profile());
    assert_eq!(a.fit_report(), b.fit_report());
    assert!(a.groups().keys().eq(b.groups().keys()));
    for (key, basis) in a.groups() {
        let other = &b.groups()[key];
        assert_eq!(support::logits(basis.mean()), support::logits(other.mean()));
        assert_eq!(support::logits(basis.axes()), support::logits(other.axes()));
        let left = &a.fit_report().groups[key]; let right = &b.fit_report().groups[key];
        assert_eq!(left.covariance_trace.to_bits(), right.covariance_trace.to_bits());
        assert_eq!(left.remaining_off_diagonal_squared.to_bits(), right.remaining_off_diagonal_squared.to_bits());
        assert_eq!(left.emitted_orthogonality_max_error.to_bits(), right.emitted_orthogonality_max_error.to_bits());
        assert_eq!(left.selected_diagonal.iter().map(|n| n.to_bits()).collect::<Vec<_>>(),
            right.selected_diagonal.iter().map(|n| n.to_bits()).collect::<Vec<_>>());
    }
}

#[test]
fn fit_checkpoint_replays_original_coefficients_reports_and_held_out_bytes() {
    let model = support::model(); let training = corpus(&model);
    let independently_fitted = LearnedKvCodec::fit(policy(), &training, FitBudget::default()).unwrap();
    let (codec, checkpoint) = LearnedKvCodec::fit_with_checkpoint(policy(), &training, FitBudget::default()).unwrap();
    assert_eq!(checkpoint.input_bytes(), training.values().map(|image| image.encode().unwrap().len()).sum());
    assert!(checkpoint.comparison_bytes() > codec.fit_report().parameter_values * 4);
    same(&codec, &independently_fitted);
    drop(training);
    let replayed = checkpoint.replay(FitBudget::default()).unwrap();
    same(&replayed, &independently_fitted);
    let held_out = model.recompute(21, &[2, 0, 1], inference()).unwrap().cache_image().unwrap();
    let (a, report_a) = independently_fitted.evaluate_held_out(201, &held_out, CompressionBudget::default()).unwrap();
    let (b, report_b) = replayed.evaluate_held_out(201, &held_out, CompressionBudget::default()).unwrap();
    assert_eq!(a.encode().unwrap(), b.encode().unwrap());
    assert_eq!(report_a, report_b);
}

#[test]
fn frozen_fit_inputs_do_not_follow_a_replaced_training_map() {
    let model = support::model(); let mut training = corpus(&model);
    let (codec, saved) = LearnedKvCodec::fit_with_checkpoint(policy(), &training, FitBudget::default()).unwrap();
    training.insert(101, model.recompute(11, &[2, 2, 2, 2], inference()).unwrap().cache_image().unwrap());
    let changed = LearnedKvCodec::fit(policy(), &training, FitBudget::default()).unwrap();
    let key = codec.groups().keys().next().unwrap();
    assert_ne!(support::logits(codec.groups()[key].mean()), support::logits(changed.groups()[key].mean()));
    same(&saved.replay(FitBudget::default()).unwrap(), &codec);
    assert_eq!(saved.profile(), codec.profile()); assert_eq!(saved.policy(), policy());
}

#[test]
fn fit_replay_requires_each_original_allowance_without_enlarging_it() {
    let model = support::model(); let training = corpus(&model);
    let baseline = LearnedKvCodec::fit(policy(), &training, FitBudget::default()).unwrap();
    let report = baseline.fit_report();
    let exact = FitBudget { source_values: report.training_values, parameter_values: report.parameter_values,
        scratch_values: report.scratch_values_reserved, work_units: report.work_units_reserved };
    let (codec, saved) = LearnedKvCodec::fit_with_checkpoint(policy(), &training, exact).unwrap();
    for index in 0..4 {
        for enlarge in [false, true] {
            let mut budget = exact;
            let adjust = |n: usize| if enlarge { n + 1 } else { n - 1 };
            match index {
                0 => budget.source_values = adjust(budget.source_values),
                1 => budget.parameter_values = adjust(budget.parameter_values),
                2 => budget.scratch_values = adjust(budget.scratch_values),
                _ => budget.work_units = if enlarge { budget.work_units + 1 } else { budget.work_units - 1 },
            }
            assert!(matches!(saved.replay(budget), Err(Error::Limit)));
        }
    }
    same(&saved.replay(exact).unwrap(), &codec);
    assert_eq!(saved.budget(), exact);
}

#[test]
fn fit_replay_keeps_origin_and_same_stream_exclusions() {
    let model = support::model(); let training = corpus(&model);
    let (_, saved) = LearnedKvCodec::fit_with_checkpoint(policy(), &training, FitBudget::default()).unwrap();
    let codec = saved.replay(FitBudget::default()).unwrap();
    let fresh = model.recompute(21, &[2], inference()).unwrap().cache_image().unwrap();
    assert!(matches!(codec.evaluate_held_out(101, &fresh, CompressionBudget::default()), Err(Error::Duplicate)));
    assert!(matches!(codec.evaluate_held_out(201, &training[&101], CompressionBudget::default()), Err(Error::Duplicate)));
    assert!(codec.evaluate_held_out(201, &fresh, CompressionBudget::default()).is_ok());
    assert_eq!(codec.fit_report().sources.len(), 2);
}

#[test]
fn checkpoint_creation_keeps_original_corpus_admission_and_has_a_valid_control() {
    let model = support::model(); let training = corpus(&model);
    assert!(matches!(LearnedKvCodec::fit_with_checkpoint(policy(), &BTreeMap::new(), FitBudget::default()),
        Err(Error::InvalidInput)));
    let repeated = BTreeMap::from([(101, training[&101].clone()), (102, training[&101].clone())]);
    assert!(matches!(LearnedKvCodec::fit_with_checkpoint(policy(), &repeated, FitBudget::default()), Err(Error::Duplicate)));
    let too_wide = LearnedKvPolicy::new(7, 2, 3, 8).unwrap();
    assert!(matches!(LearnedKvCodec::fit_with_checkpoint(too_wide, &training, FitBudget::default()), Err(Error::Binding)));
    assert!(LearnedKvCodec::fit_with_checkpoint(policy(), &training, FitBudget::default()).is_ok());
}

#[test]
fn repeated_fit_replay_reports_fresh_work_and_does_not_consume_the_checkpoint() {
    let model = support::model(); let training = corpus(&model);
    let (codec, saved) = LearnedKvCodec::fit_with_checkpoint(policy(), &training, FitBudget::default()).unwrap();
    let shared = saved.clone();
    let mut reported_reservations = 0_u64;
    for _ in 0..3 {
        let replayed = shared.replay(FitBudget::default()).unwrap();
        same(&codec, &replayed);
        reported_reservations += replayed.fit_report().work_units_reserved;
    }
    assert_eq!(reported_reservations, 3 * codec.fit_report().work_units_reserved);
    assert_eq!(saved.input_bytes(), shared.input_bytes());
    assert_eq!(saved.comparison_bytes(), shared.comparison_bytes());
}
