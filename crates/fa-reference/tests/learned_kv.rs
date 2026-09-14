//! Learned compression through original checked captures. Numerical fixtures are
//! synthetic; no learned harmfulness or production-continuation claim follows.
#[path = "support/learned_kv.rs"] mod fixture;
#[path = "support/model_kv_fixture.rs"] mod mixed;
use fixture::{image, line};
use fa_reference::action::consequence::activation::tensor::kv::experiment::{KvCell, KvSide};
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{
    CompressionBudget, FitBudget, GroupKey, LearnedKvCodec, LearnedKvPolicy,
};
use fa_reference::action::consequence::activation::tensor::kv::model::{ModelKvBudget, ModelKvCapture};
use fa_reference::Error;
use std::collections::BTreeMap;

fn policy(rank: usize) -> LearnedKvPolicy { LearnedKvPolicy::new(1, 1, rank, 8).unwrap() }
fn codec(rank: usize, data: &BTreeMap<u64, fa_reference::action::consequence::activation::tensor::kv::model::ModelKvImage>) -> LearnedKvCodec {
    LearnedKvCodec::fit(policy(rank), data, FitBudget::default()).unwrap()
}
fn scalar(image: &fa_reference::action::consequence::activation::tensor::kv::model::learned::LearnedKvImage,
    position: u64, channel: usize) -> f32
{
    f32::from_bits(image.bits(1, KvCell { side: KvSide::Key, position, head: 0, channel }).unwrap())
}

#[test]
fn learned_rotated_rank_one_basis_reconstructs_an_unseen_point_in_the_training_subspace() {
    let training = BTreeMap::from([(7, line(1, 3, 5))]); let learned = codec(1, &training);
    let key = GroupKey { layer: 1, side: KvSide::Key, head: 0 };
    let basis = &learned.groups()[&key];
    assert!(basis.axes()[0].abs() > 0.4); assert!(basis.axes()[1].abs() > 0.8);
    assert_eq!(basis.axes()[2], 0.0);
    let held_out = image(2, 1, 3, &[vec![3.0, 6.0, 0.0], vec![-4.0, -8.0, 0.0]]);
    let (compressed, report) = learned.evaluate_held_out(8, &held_out, CompressionBudget::default()).unwrap();
    assert!(!report.training_source_overlap);
    assert!((scalar(&compressed, 0, 0) - 3.0).abs() < 1e-5);
    assert!((scalar(&compressed, 0, 1) - 6.0).abs() < 1e-5);
    assert!((scalar(&compressed, 1, 0) + 4.0).abs() < 1e-5);
    assert!(report.groups.values().all(|error| error.max_absolute_error < 1e-5));
    assert_eq!(learned.fit_report().rows_per_group, 5);
    assert_eq!(learned.fit_report().training_values, 30);
    assert_eq!(learned.fit_report().source_coordinate_visits, 60);
    assert_eq!(learned.fit_report().groups[&key].rotations, 1);
    assert_eq!(learned.fit_report().groups[&key].remaining_off_diagonal_squared, 0.0);
}

#[test]
fn rare_out_of_subspace_signal_is_reported_erased_while_a_full_rank_control_preserves_it() {
    let training = BTreeMap::from([(7, line(1, 3, 5))]);
    let rare = image(2, 1, 3, &[vec![1.0, 2.0, 0.25]]);
    let (lossy, report) = codec(1, &training).evaluate_held_out(8, &rare, CompressionBudget::default()).unwrap();
    assert_eq!(scalar(&lossy, 0, 2), 0.0);
    assert_eq!(report.groups[&GroupKey { layer: 1, side: KvSide::Key, head: 0 }].nonzero_to_zero, 1);
    assert!(report.groups.values().all(|error| error.squared_error_sum >= 0.0625));
    let (full, report) = codec(3, &training).evaluate_held_out(8, &rare, CompressionBudget::default()).unwrap();
    assert_eq!(scalar(&full, 0, 2), 0.25);
    assert!(report.groups.values().all(|error| error.nonzero_to_zero == 0));
    assert!(report.encoded_bytes > report.original_image_bytes, "full-rank metadata is not free compression");
}

#[test]
fn complete_export_counts_codebooks_lineage_and_headers_and_can_really_shrink_a_long_prefix() {
    let training = BTreeMap::from([(10, line(1, 32, 8))]); let learned = codec(1, &training);
    let source = line(2, 32, 128);
    let (compressed, report) = learned.compress(&source, CompressionBudget::default()).unwrap();
    let bytes = compressed.encode().unwrap();
    assert_eq!(&bytes[..8], b"FAKVLR\0\x01");
    assert_eq!(bytes.len(), learned.encoded_len_for(&source).unwrap());
    assert_eq!(bytes.len(), report.encoded_bytes);
    assert_eq!(report.latent_scalar_bytes, 128 * 2 * 4);
    assert_eq!(report.codebook_scalar_bytes, 2 * 32 * 2 * 4);
    assert_eq!(bytes.len(), report.metadata_bytes + report.latent_scalar_bytes + report.codebook_scalar_bytes);
    assert!(report.metadata_bytes > source.descriptor().descriptor_len());
    assert!(bytes.len() < report.original_image_bytes);
    assert_eq!(compressed.latent_values(), 256);
    assert_eq!(compressed.source_descriptor(), &source.descriptor());
    assert_eq!(learned.fit_report().sources[&10].descriptor, training[&10].descriptor());
}

#[test]
fn same_origin_or_stream_cannot_be_called_held_out_and_evaluation_never_retunes() {
    let source = line(1, 3, 5); let training = BTreeMap::from([(7, source.clone())]);
    let learned = codec(1, &training); let before = learned.fit_report().clone();
    let axes: Vec<_> = learned.groups().values().map(|basis| basis.axes().to_vec()).collect();
    assert_eq!(learned.evaluate_held_out(8, &source, CompressionBudget::default()).unwrap_err(), Error::Duplicate);
    let unseen = image(2, 1, 3, &[vec![1.0, 1.0, 1000.0]]);
    assert_eq!(learned.evaluate_held_out(7, &unseen, CompressionBudget::default()).unwrap_err(), Error::Duplicate);
    assert_eq!(learned.evaluate_held_out(0, &unseen, CompressionBudget::default()).unwrap_err(), Error::InvalidInput);
    let (_, in_sample) = learned.compress(&source, CompressionBudget::default()).unwrap();
    assert!(in_sample.training_source_overlap);
    let (_, evaluation) = learned.evaluate_held_out(8, &unseen, CompressionBudget::default()).unwrap();
    assert!(evaluation.groups.values().all(|error| error.squared_error_sum >= 1_000_000.0));
    assert_eq!(learned.fit_report(), &before);
    assert_eq!(learned.groups().values().map(|basis| basis.axes().to_vec()).collect::<Vec<_>>(), axes);
    let duplicate = BTreeMap::from([(7, source.clone()), (8, source)]);
    assert_eq!(LearnedKvCodec::fit(policy(1), &duplicate, FitBudget::default()).unwrap_err(), Error::Duplicate);
}

#[test]
fn exact_fit_and_encoding_budgets_admit_but_each_one_below_refuses_without_mutating_sources() {
    let training = BTreeMap::from([(7, line(1, 3, 5))]); let source = line(2, 3, 3);
    let original = source.encode().unwrap(); let fitted = codec(1, &training); let cost = fitted.fit_report();
    let exact = FitBudget { source_values: cost.training_values, parameter_values: cost.parameter_values,
        scratch_values: cost.scratch_values_reserved, work_units: cost.work_units_reserved };
    let fitted = LearnedKvCodec::fit(policy(1), &training, exact).unwrap();
    for smaller in [FitBudget { source_values: exact.source_values - 1, ..exact },
        FitBudget { parameter_values: exact.parameter_values - 1, ..exact },
        FitBudget { scratch_values: exact.scratch_values - 1, ..exact },
        FitBudget { work_units: exact.work_units - 1, ..exact }]
    { assert_eq!(LearnedKvCodec::fit(policy(1), &training, smaller).unwrap_err(), Error::Limit); }
    let (_, report) = fitted.compress(&source, CompressionBudget::default()).unwrap();
    let exact = CompressionBudget { source_values: source.normalized_values(), encoded_bytes: report.encoded_bytes,
        work_units: report.work_units_reserved };
    assert!(fitted.compress(&source, exact).is_ok());
    for smaller in [CompressionBudget { source_values: exact.source_values - 1, ..exact },
        CompressionBudget { encoded_bytes: exact.encoded_bytes - 1, ..exact },
        CompressionBudget { work_units: exact.work_units - 1, ..exact }]
    { assert_eq!(fitted.compress(&source, smaller).unwrap_err(), Error::Limit); }
    assert_eq!(source.encode().unwrap(), original);
}

#[test]
fn independently_fitted_heads_do_not_duplicate_query_heads_or_merge_different_subspaces() {
    let data = image(1, 2, 2, &[vec![-2.0, 0.0, 0.0, -4.0], vec![0.0; 4], vec![2.0, 0.0, 0.0, 4.0]]);
    let learned = codec(1, &BTreeMap::from([(1, data)]));
    assert_eq!(learned.groups().len(), 4);
    assert_eq!(learned.groups()[&GroupKey { layer: 1, side: KvSide::Key, head: 0 }].axes(), &[1.0, 0.0]);
    assert_eq!(learned.groups()[&GroupKey { layer: 1, side: KvSide::Key, head: 1 }].axes(), &[0.0, 1.0]);
    let target = image(2, 2, 2, &[vec![3.0, 0.0, 0.0, 5.0]]);
    let (_, report) = learned.evaluate_held_out(2, &target, CompressionBudget::default()).unwrap();
    assert_eq!(report.latent_scalar_bytes, 16);
    assert!(report.groups.values().all(|error| error.changed_words == 0));
}

#[test]
fn existing_mixed_precision_multi_layer_capture_is_consumed_without_reinterpreting_storage() {
    let training = BTreeMap::from([(1, mixed::image())]); let learned = codec(1, &training);
    let mut capture = ModelKvCapture::new(mixed::profile(), 12, 0, 0, 1,
        ModelKvBudget { positions: 4, normalized_values: 32 }).unwrap();
    capture.append(0, mixed::Buffers::new().requests(0, 2, 1)).unwrap();
    let target = capture.snapshot(1).unwrap();
    let (_, report) = learned.evaluate_held_out(2, &target, CompressionBudget::default()).unwrap();
    assert_eq!(report.groups.len(), 6);
    assert!(report.groups.values().all(|error| error.max_absolute_error < 1e-5));
    assert_eq!(report.original_image_bytes, target.encode().unwrap().len());
}

#[test]
fn fitted_data_survives_source_destruction_and_failed_projection_does_not_replace_it() {
    let training = BTreeMap::from([(1, line(1, 2, 3))]); let learned = codec(1, &training);
    let source = line(2, 2, 4);
    let (compressed, _) = learned.compress(&source, CompressionBudget::default()).unwrap();
    let before = compressed.encode().unwrap();
    drop(training); drop(source);
    let large = image(3, 1, 2, &[vec![f32::MAX, f32::MAX]]); let original = large.encode().unwrap();
    assert_eq!(learned.compress(&large, CompressionBudget::default()).unwrap_err(), Error::Overflow);
    assert_eq!(large.encode().unwrap(), original);
    let allowed = image(4, 1, 2, &[vec![1e30, 1e30]]);
    assert!(learned.compress(&allowed, CompressionBudget::default()).is_ok());
    assert_eq!(compressed.encode().unwrap(), before);
    assert!(scalar(&compressed, 0, 0).is_finite());
}

#[test]
fn profile_rank_and_training_coverage_refusals_have_matching_permitted_controls() {
    assert_eq!(LearnedKvPolicy::new(0, 1, 1, 1), Err(Error::InvalidInput));
    assert_eq!(LearnedKvPolicy::new(1, 1, 1, 33), Err(Error::Limit));
    let one = BTreeMap::from([(1, line(1, 2, 1))]);
    assert_eq!(LearnedKvCodec::fit(policy(1), &one, FitBudget::default()).unwrap_err(), Error::Incomplete);
    let valid = BTreeMap::from([(1, line(1, 2, 3))]);
    assert!(LearnedKvCodec::fit(policy(2), &valid, FitBudget::default()).is_ok());
    assert_eq!(LearnedKvCodec::fit(policy(3), &valid, FitBudget::default()).unwrap_err(), Error::Binding);
    let learned = codec(1, &valid);
    assert_eq!(learned.compress(&line(2, 3, 3), CompressionBudget::default()).unwrap_err(), Error::Binding);
    assert!(learned.compress(&line(2, 2, 3), CompressionBudget::default()).is_ok());
    let (compressed, _) = learned.compress(&line(2, 2, 3), CompressionBudget::default()).unwrap();
    assert_eq!(compressed.bits(1, KvCell { side: KvSide::Key, position: 3, head: 0, channel: 0 }), Err(Error::Missing));
    assert_eq!(compressed.bits(1, KvCell { side: KvSide::Key, position: 0, head: 1, channel: 0 }), Err(Error::InvalidInput));
}

#[test]
fn canonical_origin_order_and_constant_training_produce_reproducible_fitted_bytes() {
    let a = image(1, 1, 2, &[vec![7.0, -3.0], vec![7.0, -3.0]]);
    let b = image(2, 1, 2, &[vec![7.0, -3.0], vec![7.0, -3.0]]);
    let first = BTreeMap::from([(20, a.clone()), (10, b.clone())]);
    let second = BTreeMap::from([(10, b), (20, a)]);
    let target = image(3, 1, 2, &[vec![7.0, -3.0]]);
    let (one, report) = codec(1, &first).compress(&target, CompressionBudget::default()).unwrap();
    let (two, _) = codec(1, &second).compress(&target, CompressionBudget::default()).unwrap();
    assert_eq!(one.encode().unwrap(), two.encode().unwrap());
    assert!(report.groups.values().all(|error| error.changed_words == 0));
    assert!(one.codec().fit_report().groups.values().all(|group| group.rotations == 0));
}
