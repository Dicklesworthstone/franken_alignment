//! Full numerical capture, immutable three-way fitting and exact final scoring.
#[path = "support/decoder_probe_campaign.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/decoder_fixture.rs"]
#[allow(dead_code)]
mod numerical;

use fixture::*;
use fa_reference::action::consequence::activation::{ProgressiveFrame, HEADER_BYTES};
use fa_reference::action::consequence::activation::probe::{ProbeOutcome, training::{ClassCounts, DataSplit, FitPolicy}};
use fa_reference::action::consequence::activation::probe::training::calibration::{CalibrationPolicy, ScreeningCriteria};
use fa_reference::action::consequence::activation::probe::training::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use fa_reference::Error;

#[test]
fn complete_all_layer_campaign_uses_each_reserved_split_and_original_exact_probes() {
    let model = model();
    let cases = cases();
    let work = DecoderCorpus::estimate(&model, &cases).unwrap();
    assert_eq!(work, CaptureWork { cases: 6, original_tokens: 6, residual_coordinates: 24, scalar_products: 408 });
    let corpus = capture(&model, &cases);
    assert_eq!(corpus.work(), work);
    assert_eq!(corpus.cases().values().cloned().collect::<Vec<_>>(), cases);
    let report = campaign(&corpus);
    assert!(report.accepted());
    assert_eq!(report.layers().len(), 2);
    assert_eq!(report.admitted_work(), report.completed_work());
    assert_eq!(report.completed_work().scoring_bytes, 8 * (HEADER_BYTES + 8));
    for (layer, result) in report.layers() {
        assert_eq!(result.calibration().selected_threshold(), Some(0.0));
        assert_eq!(result.calibration().trials().len(), 3);
        let evaluation = result.evaluation().unwrap();
        assert_eq!(evaluation.counts().classes(), ClassCounts { benign: 1, violation: 1 });
        assert_eq!(evaluation.counts().benign_quiet, 1);
        assert_eq!(evaluation.counts().violation_alarm, 1);
        assert_eq!(report.probes().unwrap()[layer].identity().profile, model.residual_contract(*layer).unwrap().profile());
    }
}

#[test]
fn real_multitoken_residual_scores_match_separate_original_decoder_execution() {
    let model = numerical::model(numerical::profile(8));
    let mut cases = cases();
    for (index, case) in cases.iter_mut().enumerate() { case.tokens = vec![0, 3, index as u32]; }
    let corpus = capture(&model, &cases);
    let report = campaign(&corpus);
    for case in cases.iter().filter(|case| case.split == DataSplit::Calibration) {
        let mut direct = model.session(case.origin.task).unwrap();
        let mut last = None;
        for (position, token) in case.tokens.iter().copied().enumerate() {
            last = Some(direct.advance(position as u64, token,
                DecoderBudget { scalar_products: model.estimate(position, 1).unwrap().scalar_products().unwrap() }).unwrap());
        }
        for residual in last.unwrap().layers {
            let result = &report.layers()[&residual.layer];
            let source = residual.residual.source();
            let block = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
            let frame = ProgressiveFrame::from_initial(&block).unwrap();
            let direct = result.calibration().fitted().probe(0.0).unwrap().evaluate(&frame).unwrap();
            let stored = result.calibration().scores().iter().find(|score| score.origin() == case.origin).unwrap();
            assert_eq!(stored.frame(), source.identity());
            assert_eq!(stored.score(), &direct.interval().lower);
            assert_eq!(direct.interval().lower, direct.interval().upper);
        }
    }
}

#[test]
fn late_malformed_case_or_cross_split_lineage_cannot_consume_capture_allowance() {
    let model = model();
    let baseline = cases();
    let work = DecoderCorpus::estimate(&model, &baseline).unwrap();
    for failure in 0..5 {
        let mut malformed = baseline.clone();
        match failure {
            0 => malformed[5].tokens = vec![2],
            1 => malformed[5].tokens.clear(),
            2 => malformed[5].origin.lineage = malformed[0].origin.lineage,
            3 => malformed[5].tokens = vec![0; 9],
            _ => malformed[5].label = malformed[4].label,
        }
        let mut budget = CaptureBudget::new(work).unwrap();
        assert!(DecoderCorpus::capture(&model, 1, 1, &malformed, &mut budget).is_err());
        assert_eq!(budget.remaining(), work);
        assert!(DecoderCorpus::capture(&model, 1, 1, &baseline, &mut budget).is_ok());
    }
}

#[test]
fn capture_allowance_is_aggregate_atomic_and_persistent_not_renewed_per_case() {
    let model = model();
    let cases = cases();
    let work = DecoderCorpus::estimate(&model, &cases).unwrap();
    for field in 0..4 {
        let mut short = work;
        match field { 0 => short.cases -= 1, 1 => short.original_tokens -= 1,
            2 => short.residual_coordinates -= 1, _ => short.scalar_products -= 1 }
        let mut budget = CaptureBudget::new(short).unwrap();
        assert_eq!(DecoderCorpus::capture(&model, 1, 1, &cases, &mut budget).unwrap_err(), Error::Limit);
        assert_eq!(budget.remaining(), short);
    }
    let mut budget = CaptureBudget::new(work).unwrap();
    DecoderCorpus::capture(&model, 1, 1, &cases, &mut budget).unwrap();
    assert_eq!(budget.remaining(), CaptureWork::default());
    assert_eq!(DecoderCorpus::capture(&model, 2, 1, &cases, &mut budget).unwrap_err(), Error::Limit);
}

#[test]
fn admitted_numerical_failure_returns_no_corpus_and_does_not_restore_its_charge() {
    let model = model_with_output(true);
    let cases = cases();
    let mut budget = CaptureBudget::new(DecoderCorpus::estimate(&model, &cases).unwrap()).unwrap();
    assert_eq!(DecoderCorpus::capture(&model, 1, 1, &cases, &mut budget).unwrap_err(), Error::Overflow);
    assert_eq!(budget.remaining(), CaptureWork::default());
    assert!(campaign(&capture(&fixture::model(), &cases)).accepted());
}

#[test]
fn every_campaign_dimension_is_checked_before_fitting_any_layer() {
    let corpus = capture(&model(), &cases());
    let plans = policies();
    let work = corpus.estimate_campaign(&plans).unwrap();
    for field in 0..4 {
        let mut short = work;
        match field { 0 => short.training_visits -= 1, 1 => short.scoring_bytes -= 1,
            2 => short.scoring_coordinates -= 1, _ => short.threshold_comparisons -= 1 }
        let mut budget = CampaignBudget::new(short).unwrap();
        assert_eq!(corpus.run(plans.clone(), &mut budget).unwrap_err(), Error::Limit);
        assert_eq!(budget.remaining(), short);
    }
    let mut incomplete = plans.clone(); incomplete.remove(&2);
    let mut budget = CampaignBudget::new(work).unwrap();
    assert_eq!(corpus.run(incomplete, &mut budget).unwrap_err(), Error::Binding);
    assert_eq!(budget.remaining(), work);
    assert!(corpus.run(plans.clone(), &mut budget).unwrap().accepted());
    assert_eq!(budget.remaining(), CampaignWork::default());
    assert_eq!(corpus.run(plans, &mut budget).unwrap_err(), Error::Limit);
}

#[test]
fn final_labels_cannot_retune_coefficients_thresholds_or_hide_a_failed_layer() {
    let model = model();
    let baseline = cases();
    let good = campaign(&capture(&model, &baseline));
    let mut changed = baseline;
    for case in &mut changed {
        if case.split == DataSplit::Evaluation { case.tokens[0] ^= 1; }
    }
    let bad = campaign(&capture(&model, &changed));
    assert!(!bad.accepted());
    assert_eq!(bad.probes().unwrap_err(), Error::WrongState);
    for layer in 1..=2 {
        let a = good.layers()[&layer].calibration();
        let b = bad.layers()[&layer].calibration();
        assert_eq!(a.fitted().weights(), b.fitted().weights());
        assert_eq!(a.fitted().bias(), b.fitted().bias());
        assert_eq!(a.trials(), b.trials());
        let result = bad.layers()[&layer].evaluation().unwrap();
        assert_eq!(result.counts().violation_quiet, 1);
        assert_eq!(result.counts().benign_alarm, 1);
        assert_eq!(result.scores().len(), 2);
    }
}

#[test]
fn unqualified_layer_does_not_suppress_other_results_or_create_a_partial_roster() {
    let corpus = capture(&model(), &cases());
    for no_threshold in [false, true] {
        let mut plans = policies();
        let ordinary = ScreeningCriteria::new(1, 0).unwrap();
        let strict = ScreeningCriteria::new(2, 0).unwrap();
        plans.get_mut(&1).unwrap().calibration = CalibrationPolicy::new(1, 1,
            if no_threshold { &[100.0] } else { &[0.0] }, ordinary, strict).unwrap();
        let mut budget = CampaignBudget::new(corpus.estimate_campaign(&plans).unwrap()).unwrap();
        let result = corpus.run(plans, &mut budget).unwrap();
        assert!(!result.accepted());
        assert!(result.layers()[&2].accepted());
        assert_eq!(result.layers()[&1].evaluation().is_none(), no_threshold);
        assert_eq!(result.probes().unwrap_err(), Error::WrongState);
        assert_eq!(budget.remaining(), CampaignWork::default());
        assert_eq!(result.completed_work().scoring_coordinates < result.admitted_work().scoring_coordinates, no_threshold);
    }
}

#[test]
fn hard_fit_failure_keeps_admission_charge_and_successful_control_still_learns() {
    let corpus = capture(&model(), &cases());
    let mut plans = policies();
    plans.get_mut(&2).unwrap().fit = FitPolicy::new(2, 1, 512, 1.0, 1000.0, 0.001).unwrap();
    let mut budget = CampaignBudget::new(corpus.estimate_campaign(&plans).unwrap()).unwrap();
    assert_eq!(corpus.run(plans, &mut budget).unwrap_err(), Error::Overflow);
    assert_eq!(budget.remaining(), CampaignWork::default());
    assert!(campaign(&corpus).accepted());
}

#[test]
fn case_order_does_not_change_training_or_exact_results_and_quiet_is_not_a_constant() {
    let model = model();
    let cases = cases();
    let a = campaign(&capture(&model, &cases));
    let mut reversed = cases; reversed.reverse();
    let b = campaign(&capture(&model, &reversed));
    for layer in 1..=2 {
        assert_eq!(a.layers()[&layer].calibration().trials(), b.layers()[&layer].calibration().trials());
        assert_eq!(a.layers()[&layer].evaluation().unwrap().scores(), b.layers()[&layer].evaluation().unwrap().scores());
        for (token, expected) in [(0, ProbeOutcome::CertifiedQuiet), (1, ProbeOutcome::CertifiedAlarm)] {
            let mut session = model.session(100).unwrap();
            let step = session.advance(0, token, DecoderBudget { scalar_products: 68 }).unwrap();
            let source = step.layers[(layer - 1) as usize].residual.source();
            let block = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
            assert_eq!(a.probes().unwrap()[&layer].evaluate(&ProgressiveFrame::from_initial(&block).unwrap()).unwrap().outcome(), expected);
        }
    }
}
