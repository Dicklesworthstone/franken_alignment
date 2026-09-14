//! Synthetic checked captures and independent exact-source probe controls.
#[path = "support/learned_kv.rs"] mod fixture;
use fa_reference::action::consequence::activation::{SourceFrame, ProgressiveFrame};
use fa_reference::action::consequence::activation::probe::{LinearProbe, ProbeOutcome, ExactScore};
use fa_reference::action::consequence::activation::probe::learned::{
    CheckedKvBudget, CheckedLearnedKv, KvGroup, KvRow, KvRefinementBudget, ResidualRetention,
};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::{ModelKvImage, learned::{
    CompressionBudget, FitBudget, LearnedKvCodec, LearnedKvPolicy,
}};
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::cmp::Ordering;

fn codec(training: ModelKvImage, rank: usize) -> LearnedKvCodec {
    LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, rank, 8).unwrap(),
        &BTreeMap::from([(1, training)]), FitBudget::default()).unwrap()
}
fn checked(codec: &LearnedKvCodec, source: &ModelKvImage, retention: ResidualRetention) -> CheckedLearnedKv {
    let (image, _) = codec.compress(source, CompressionBudget::default()).unwrap();
    CheckedLearnedKv::new(image, source, retention, CheckedKvBudget::default()).unwrap()
}
fn row(side: KvSide, position: u64) -> KvRow { KvRow { layer: 1, side, position } }
fn group(side: KvSide, position: u64, head: usize) -> KvGroup { KvGroup { row: row(side, position), head } }
fn detector(evidence: &CheckedLearnedKv, row: KvRow, weights: &[f32], threshold: f32) -> LinearProbe {
    LinearProbe::new(1, 1, evidence.row_shape(row).unwrap().0.profile, weights, 0.0, threshold).unwrap()
}
fn exact_score(evidence: &CheckedLearnedKv, row: KvRow, probe: &LinearProbe, original: &[f32]) -> ExactScore {
    let source = SourceFrame::capture(evidence.row_shape(row).unwrap().0, original).unwrap();
    let block = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
    probe.evaluate(&ProgressiveFrame::from_initial(&block).unwrap()).unwrap().interval().lower.clone()
}
fn refine(view: &mut fa_reference::action::consequence::activation::probe::learned::LearnedKvView, group: KvGroup) {
    let block = view.source().verify_residual(group, view.source().residual_bytes(group).unwrap()).unwrap();
    view.refine(view.revision(), &block, KvRefinementBudget::default()).unwrap();
}

#[test]
fn a_learned_erased_signal_is_ambiguous_until_exact_residual_recovery_not_certified_quiet() {
    let codec = codec(fixture::line(1, 3, 5), 1);
    let source = fixture::image(2, 1, 3, &[vec![1.0, 2.0, 0.25]]);
    let evidence = checked(&codec, &source, ResidualRetention::All);
    let row = row(KvSide::Value, 0); let group = KvGroup { row, head: 0 };
    let probe = detector(&evidence, row, &[0.0, 0.0, 1.0], 0.125);
    let mut view = evidence.view();
    let coarse = probe.evaluate_learned(&view, row).unwrap();
    assert_eq!(coarse.outcome(), ProbeOutcome::NeedsRefinement);
    assert_eq!(coarse.work().coordinates, 3);
    assert_eq!(coarse.work().reconstruction_products, 1);
    let expected = exact_score(&evidence, row, &probe, &[1.0, 2.0, 0.25]);
    assert!(coarse.interval().lower <= expected && expected <= coarse.interval().upper);
    refine(&mut view, group);
    let resolved = probe.evaluate_learned(&view, row).unwrap();
    assert_eq!(resolved.outcome(), ProbeOutcome::CertifiedAlarm);
    assert_eq!(resolved.exact_score(), Some(&expected));
    assert_eq!(resolved.work().reconstruction_products, 0);
    assert_eq!(view.interval(group, 2).unwrap(), [0.25, 0.25]);
    assert_eq!(coarse.view().revision(), 0, "old observation retains its original refinement snapshot");
    assert_eq!(coarse.outcome(), ProbeOutcome::NeedsRefinement);
    assert_eq!(resolved.view().revision(), 1);
}

#[test]
fn easy_quiet_and_exact_equality_need_no_residual_but_equality_is_never_quiet() {
    let codec = codec(fixture::line(1, 3, 5), 1);
    let source = fixture::image(2, 1, 3, &[vec![0.0; 3]]);
    let evidence = checked(&codec, &source, ResidualRetention::None);
    let row = row(KvSide::Key, 0); let view = evidence.view();
    assert_eq!(evidence.radius(KvGroup { row, head: 0 }).unwrap(), 0);
    assert_eq!(detector(&evidence, row, &[1.0, 0.0, 0.0], 1.0).evaluate_learned(&view, row).unwrap().outcome(), ProbeOutcome::CertifiedQuiet);
    assert_eq!(detector(&evidence, row, &[1.0, 0.0, 0.0], 0.0).evaluate_learned(&view, row).unwrap().outcome(), ProbeOutcome::AtThreshold);
    let constant = detector(&evidence, row, &[0.0, -0.0, 0.0], 1.0).evaluate_learned(&view, row).unwrap();
    assert_eq!(constant.outcome(), ProbeOutcome::CertifiedQuiet);
    assert_eq!(constant.work().reconstruction_products, 0);
    assert_eq!(evidence.report().retained_residual_bytes, 0);
}

#[test]
fn smallest_product_survives_huge_cancellation_after_learned_residual_recovery() {
    let codec = codec(fixture::image(1, 1, 3, &[vec![0.0; 3], vec![0.0; 3]]), 1);
    let tiny = f32::from_bits(1);
    let original = [f32::MAX, tiny, -f32::MAX];
    let source = fixture::image(2, 1, 3, &[original.to_vec()]);
    let evidence = checked(&codec, &source, ResidualRetention::All);
    let row = row(KvSide::Key, 0); let probe = detector(&evidence, row, &[f32::MAX, tiny, f32::MAX], 0.0);
    let mut view = evidence.view();
    refine(&mut view, KvGroup { row, head: 0 });
    let score = probe.evaluate_learned(&view, row).unwrap();
    let expected = exact_score(&evidence, row, &probe, &original);
    assert_eq!(score.exact_score(), Some(&expected));
    assert_eq!(expected.sign(), Ordering::Greater);
    assert_eq!(expected.magnitude_words()[0], 1);
    assert!(expected.magnitude_words()[1..].iter().all(|word| *word == 0));
    assert_eq!(score.outcome(), ProbeOutcome::CertifiedAlarm);
}

#[test]
fn exact_escape_preserves_signed_zeros_subnormals_and_every_original_word() {
    let codec = codec(fixture::image(1, 1, 4, &[vec![0.0; 4], vec![0.0; 4]]), 1);
    let original = [-0.0, f32::from_bits(1), -f32::from_bits(1), f32::MIN_POSITIVE];
    let source = fixture::image(2, 1, 4, &[original.to_vec()]);
    let evidence = checked(&codec, &source, ResidualRetention::All);
    let mut view = evidence.view();
    for group in evidence.groups() {
        for (channel, value) in original.iter().enumerate() {
            let [lo, hi] = view.interval(group, channel).unwrap();
            assert!(lo <= *value && *value <= hi);
        }
        refine(&mut view, group);
        for (channel, value) in original.iter().enumerate() {
            let [lo, hi] = view.interval(group, channel).unwrap();
            assert_eq!(lo.to_bits(), value.to_bits()); assert_eq!(hi.to_bits(), value.to_bits());
        }
    }
}

#[test]
fn bounded_generated_envelopes_enclose_independent_exact_mixed_sign_probe_scores() {
    let codec = codec(fixture::image(1, 1, 3, &[vec![0.0; 3], vec![0.0; 3]]), 1);
    let mut seed = 0x1234_5678_u32;
    for case in 0..96 {
        let original: Vec<f32> = (0..3).map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let bits = if (seed >> 23) & 255 == 255 { seed ^ (1 << 23) } else { seed };
            f32::from_bits(bits)
        }).collect();
        let source = fixture::image(2 + case, 1, 3, std::slice::from_ref(&original));
        let evidence = checked(&codec, &source, ResidualRetention::All);
        let row = row(KvSide::Value, 0); let group = KvGroup { row, head: 0 };
        let probe = detector(&evidence, row, &[f32::from_bits(1), -f32::MAX, 2.0], 0.25);
        let expected = exact_score(&evidence, row, &probe, &original);
        let mut view = evidence.view();
        let coarse = probe.evaluate_learned(&view, row).unwrap();
        assert!(coarse.interval().lower <= expected && expected <= coarse.interval().upper);
        refine(&mut view, group);
        let exact = probe.evaluate_learned(&view, row).unwrap();
        assert_eq!(exact.exact_score(), Some(&expected));
        assert!(coarse.interval().lower <= exact.interval().lower && exact.interval().upper <= coarse.interval().upper);
    }
}

#[test]
fn corruption_cross_source_blocks_stale_revisions_and_each_refinement_limit_refuse_atomically() {
    let codec = codec(fixture::line(1, 3, 5), 1);
    let source = fixture::image(2, 1, 3, &[vec![1.0, 2.0, 0.25]]);
    let evidence = checked(&codec, &source, ResidualRetention::All);
    let independent = checked(&codec, &source, ResidualRetention::All);
    let group = group(KvSide::Value, 0, 0); let bytes = evidence.residual_bytes(group).unwrap();
    let block = evidence.verify_residual(group, bytes).unwrap();
    let foreign = independent.verify_residual(group, independent.residual_bytes(group).unwrap()).unwrap();
    let mut view = evidence.view(); let initial = view.interval(group, 2).unwrap();
    let mut corrupt = bytes.to_vec(); let last = corrupt.len() - 1; corrupt[last] ^= 1;
    assert!(matches!(evidence.verify_residual(group, &corrupt), Err(Error::Binding)));
    assert!(matches!(evidence.verify_residual(group, &bytes[..bytes.len() - 1]), Err(Error::Binding)));
    assert_eq!(view.refine(0, &foreign, KvRefinementBudget::default()), Err(Error::Binding));
    assert_eq!(view.refine(1, &block, KvRefinementBudget::default()), Err(Error::Stale));
    let exact = KvRefinementBudget { encoded_bytes: bytes.len(), materialized_values: 3, reconstruction_products: 3 };
    for smaller in [KvRefinementBudget { encoded_bytes: exact.encoded_bytes - 1, ..exact },
        KvRefinementBudget { materialized_values: 2, ..exact }, KvRefinementBudget { reconstruction_products: 2, ..exact }]
    { assert_eq!(view.refine(0, &block, smaller), Err(Error::Limit)); }
    assert_eq!(view.revision(), 0); assert_eq!(view.materialized_values(), 0);
    assert_eq!(view.interval(group, 2).unwrap(), initial);
    let receipt = view.refine(0, &block, exact).unwrap();
    assert_eq!(receipt.encoded_bytes, bytes.len()); assert_eq!(receipt.materialized_values, 3);
    assert_eq!(receipt.reconstruction_products, 3);
    assert_eq!(view.refine(1, &block, exact), Err(Error::Duplicate));
}

#[test]
fn base_and_retained_residual_byte_counts_are_actual_exports_and_each_source_check_limit_binds() {
    let codec = codec(fixture::line(1, 3, 5), 1); let source = fixture::line(2, 3, 8);
    let (image, _) = codec.compress(&source, CompressionBudget::default()).unwrap();
    let evidence = CheckedLearnedKv::new(image.clone(), &source, ResidualRetention::All, CheckedKvBudget::default()).unwrap();
    let report = evidence.report();
    assert_eq!(evidence.encode_base().unwrap().len(), report.base_encoded_bytes);
    assert_eq!(evidence.encode().unwrap().len(), report.total_encoded_bytes);
    assert_eq!(report.total_encoded_bytes, report.base_encoded_bytes + report.retained_residual_bytes);
    assert_eq!(report.retained_residual_bytes, evidence.groups().map(|group| evidence.residual_bytes(group).unwrap().len()).sum::<usize>());
    assert_eq!(report.source_coordinate_visits, source.normalized_values() * 2);
    assert_eq!(report.reconstruction_products, report.source_coordinate_visits as u64);
    let exact = CheckedKvBudget { source_values: report.source_values, encoded_bytes: report.total_encoded_bytes,
        reconstruction_products: report.reconstruction_products };
    assert!(CheckedLearnedKv::new(image.clone(), &source, ResidualRetention::All, exact).is_ok());
    for smaller in [CheckedKvBudget { source_values: exact.source_values - 1, ..exact },
        CheckedKvBudget { encoded_bytes: exact.encoded_bytes - 1, ..exact },
        CheckedKvBudget { reconstruction_products: exact.reconstruction_products - 1, ..exact }]
    { assert!(matches!(CheckedLearnedKv::new(image.clone(), &source, ResidualRetention::All, smaller), Err(Error::Limit))); }
}

#[test]
fn selective_heads_do_not_refine_other_heads_sides_or_positions_and_missing_residuals_stay_missing() {
    let codec = codec(fixture::image(1, 2, 2, &[vec![-1.0, 0.0, -2.0, 0.0], vec![0.0; 4], vec![1.0, 0.0, 2.0, 0.0]]), 1);
    let source = fixture::image(2, 2, 2, &[vec![1.0, 0.0, 0.0, 0.25], vec![0.0, 0.5, 0.0, 0.75]]);
    let selected = group(KvSide::Value, 0, 1);
    let evidence = checked(&codec, &source, ResidualRetention::Groups(BTreeSet::from([selected])));
    let probe = detector(&evidence, selected.row, &[0.0, 0.0, 0.0, 1.0], 0.125);
    let mut view = evidence.view(); assert_eq!(evidence.report().retained_groups, 1);
    assert_eq!(probe.evaluate_learned(&view, selected.row).unwrap().outcome(), ProbeOutcome::NeedsRefinement);
    refine(&mut view, selected);
    assert_eq!(probe.evaluate_learned(&view, selected.row).unwrap().outcome(), ProbeOutcome::CertifiedAlarm);
    assert_eq!(view.materialized_values(), 2);
    for other in [group(KvSide::Value, 0, 0), group(KvSide::Key, 0, 1), group(KvSide::Value, 1, 1)] {
        assert!(!view.is_refined(other).unwrap()); assert_eq!(evidence.residual_bytes(other), Err(Error::Missing));
    }
}

#[test]
fn equal_descriptors_do_not_substitute_a_different_sources_mse_for_an_actual_source_check() {
    let codec = codec(fixture::line(1, 3, 5), 1);
    let first = fixture::image(2, 1, 3, &[vec![1.0, 2.0, 0.25]]);
    let changed = fixture::image(2, 1, 3, &[vec![1.0, 2.0, 10.0]]);
    assert_eq!(first.descriptor(), changed.descriptor());
    let (image, _) = codec.compress(&first, CompressionBudget::default()).unwrap();
    let evidence = CheckedLearnedKv::new(image.clone(), &changed, ResidualRetention::All, CheckedKvBudget::default()).unwrap();
    let group = group(KvSide::Value, 0, 0); let probe = detector(&evidence, group.row, &[0.0, 0.0, 1.0], 5.0);
    let mut view = evidence.view();
    assert_eq!(probe.evaluate_learned(&view, group.row).unwrap().outcome(), ProbeOutcome::NeedsRefinement);
    refine(&mut view, group);
    assert_eq!(probe.evaluate_learned(&view, group.row).unwrap().outcome(), ProbeOutcome::CertifiedAlarm);
    let other_stream = fixture::image(3, 1, 3, &[vec![1.0, 2.0, 10.0]]);
    assert!(matches!(CheckedLearnedKv::new(image, &other_stream, ResidualRetention::All, CheckedKvBudget::default()), Err(Error::Binding)));
}

#[test]
fn dropping_sources_preserves_recovery_and_cloned_views_do_not_share_mutable_promotions() {
    let codec = codec(fixture::line(1, 3, 5), 1);
    let source = fixture::image(2, 1, 3, &[vec![1.0, 2.0, 0.25]]);
    let evidence = checked(&codec, &source, ResidualRetention::All);
    let group = group(KvSide::Value, 0, 0);
    let mut left = evidence.view(); let right = left.clone();
    let before = right.interval(group, 2).unwrap();
    drop(source); drop(codec); drop(evidence);
    refine(&mut left, group);
    assert_eq!(left.interval(group, 2).unwrap(), [0.25, 0.25]);
    assert_eq!(right.interval(group, 2).unwrap(), before); assert_eq!(right.revision(), 0);
}
