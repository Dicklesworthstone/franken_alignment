//! End-to-end numerical inference and the existing progressive probe evaluator.
//! Fixture parameters/probes are synthetic, never learned detector qualification.
#[path = "support/decoder_fixture.rs"]
mod fixture;
use fa_reference::action::consequence::activation::{ProgressiveFrame, SourceFrame};
use fa_reference::action::consequence::activation::monitor::{
    MonitorOutcome, RefinementBudget, RefinementMonitor,
};
use fa_reference::action::consequence::activation::monitor::decoder::*;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::Error;
use std::collections::BTreeMap;

fn allowance() -> RefinementBudget { RefinementBudget { encoded_bytes: 1_000_000, probe_coordinates: 1_000_000 } }
fn compute() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn monitors(model: &DecoderModel) -> BTreeMap<u64, RefinementMonitor> {
    (1..=model.profile().shape().layers as u64).map(|layer| {
        (layer, detector(model, layer, 1_000_000.0, allowance()))
    }).collect()
}
fn detector(model: &DecoderModel, layer: u64, threshold: f32, budget: RefinementBudget) -> RefinementMonitor {
    let contract = model.residual_contract(layer).unwrap();
    let mut weights = vec![0.0; contract.dimensions()]; weights[0] = 1.0;
    RefinementMonitor::new(vec![LinearProbe::new(1, 1, contract.profile(), &weights, 0.0, threshold).unwrap()],
        vec![23], budget).unwrap()
}
fn run(model: DecoderModel, roster: BTreeMap<u64, RefinementMonitor>, budget: RefinementBudget) -> MonitoredDecoder {
    MonitoredDecoder::new(model, 7, 11, roster, budget).unwrap()
}
fn release(result: MonitoredStep) -> ReviewedStep {
    match result { MonitoredStep::Released(step) => step, MonitoredStep::Held(report) => panic!("unexpected hold: {report:?}") }
}
fn held(result: MonitoredStep) -> std::rc::Rc<DecoderReview> {
    match result { MonitoredStep::Held(report) => report, MonitoredStep::Released(_) => panic!("unreviewed output escaped") }
}
fn values(source: &SourceFrame) -> Vec<f32> {
    let bytes = source.encode_initial(23).unwrap();
    ProgressiveFrame::from_initial(&source.verify_block(&bytes).unwrap()).unwrap().exact_values().unwrap()
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }

#[test]
fn full_prefix_and_32_generated_steps_equal_the_original_numerical_engine() {
    let model = fixture::model(fixture::profile(48));
    let mut raw = model.session(7).unwrap();
    let mut guarded = run(model.clone(), monitors(&model), allowance());
    for token in [0, 3, 1, 5] {
        let position = raw.position();
        let expected = raw.advance(position, token, compute()).unwrap();
        let actual = release(guarded.advance(position, token, compute()).unwrap());
        compare_steps(&actual, &expected);
    }
    for _ in 0..32 {
        let position = raw.position();
        let expected = raw.advance_greedy(position, compute()).unwrap();
        let actual = release(guarded.advance_greedy(position, compute()).unwrap());
        compare_steps(&actual, &expected);
    }
    assert_eq!(guarded.position(), 36);
    assert_eq!(guarded.decoder_work(), raw.work());
    assert_eq!(guarded.monitoring_work().frame_reviews, 72);
    assert_eq!(guarded.monitoring_work().probe_coordinates, 72 * 4);
    assert_eq!(guarded.monitoring_work().codec_coordinates, 72 * 4 * 3);
    assert_eq!(guarded.status(), MonitoringStatus::Ready);
}
fn compare_steps(actual: &ReviewedStep, expected: &DecoderStep) {
    assert_eq!(actual.step().token, expected.token);
    assert_eq!(bits(&actual.step().logits), bits(&expected.logits));
    let report = actual.review();
    assert_eq!(report.generation(), 11); assert_eq!(report.stream(), 7);
    assert_eq!(report.position(), expected.position);
    assert_eq!(report.required_layers(), 2); assert_eq!(report.unreviewed_layers(), 0);
    assert_eq!(report.outcome(), MonitorOutcome::NoAlarm);
    for (index, ((a, b), review)) in actual.step().layers.iter().zip(&expected.layers).zip(report.layers()).enumerate() {
        assert_eq!(a.layer, index as u64 + 1); assert_eq!(review.layer, a.layer);
        assert_eq!(bits(&values(a.residual.source())), bits(&values(b.residual.source())));
        assert_eq!(review.report.frame(), a.residual.source().identity());
        assert_eq!(review.report.frame().sequence, expected.position + 1);
    }
}

#[test]
fn actual_last_layer_residual_changes_release_to_alarm_or_threshold_hold() {
    let model = fixture::model(fixture::profile(8));
    let expected = model.session(7).unwrap().advance(0, 3, compute()).unwrap();
    let score = values(expected.layers[1].residual.source())[0];
    for (threshold, outcome) in [(score + 1.0, MonitorOutcome::NoAlarm),
        (score - 1.0, MonitorOutcome::Alarm), (score, MonitorOutcome::AtThreshold)]
    {
        let mut roster = monitors(&model); roster.insert(2, detector(&model, 2, threshold, allowance()));
        let mut guarded = run(model.clone(), roster, allowance());
        let result = guarded.advance(0, 3, compute()).unwrap();
        assert_eq!(result.review().outcome(), outcome);
        assert_eq!(result.review().layers().len(), 2);
        if outcome == MonitorOutcome::NoAlarm {
            assert_eq!(bits(&release(result).step().logits), bits(&expected.logits));
            assert_eq!(guarded.status(), MonitoringStatus::Ready);
        } else {
            let report = held(result);
            assert_eq!(guarded.last_review(), Some(report.as_ref()));
            assert_eq!(guarded.position(), 1); assert_eq!(guarded.decoder_work().tokens, 1);
            let before = (guarded.decoder_work(), guarded.monitoring_work());
            assert!(matches!(guarded.advance_greedy(1, compute()), Err(Error::WrongState)));
            assert!(matches!(guarded.advance(1, 0, compute()), Err(Error::WrongState)));
            assert_eq!((guarded.decoder_work(), guarded.monitoring_work()), before);
        }
    }
}

#[test]
fn early_alarm_exposes_unreviewed_layers_instead_of_claiming_complete_coverage() {
    let model = fixture::model(fixture::profile(8));
    let expected = model.session(7).unwrap().advance(0, 2, compute()).unwrap();
    let score = values(expected.layers[0].residual.source())[0];
    let mut roster = monitors(&model); roster.insert(1, detector(&model, 1, score - 1.0, allowance()));
    let mut guarded = run(model, roster, allowance());
    let report = held(guarded.advance(0, 2, compute()).unwrap());
    assert_eq!(report.outcome(), MonitorOutcome::Alarm);
    assert_eq!(report.layers().len(), 1); assert_eq!(report.unreviewed_layers(), 1);
    assert_eq!(guarded.monitoring_work().frame_reviews, 1);
}

#[test]
fn missing_extra_reassigned_wrong_generation_and_wrong_width_monitors_refuse() {
    let model = fixture::model(fixture::profile(8));
    let original = monitors(&model);
    let mut missing = original.clone(); missing.remove(&2);
    let mut extra = original.clone(); extra.insert(3, extra[&1].clone());
    let mut swapped = original.clone(); swapped.insert(1, original[&2].clone());
    for roster in [missing, extra, swapped] {
        assert!(matches!(MonitoredDecoder::new(model.clone(), 7, 11, roster, allowance()), Err(Error::Binding)));
    }
    for wrong_width in [false, true] {
        let contract = model.residual_contract(2).unwrap(); let mut profile = contract.profile();
        if !wrong_width { profile.model_generation += 1; }
        let weights = vec![1.0; contract.dimensions() + usize::from(wrong_width)];
        let probe = LinearProbe::new(1, 1, profile, &weights, 0.0, 1_000_000.0).unwrap();
        let mut roster = original.clone();
        roster.insert(2, RefinementMonitor::new(vec![probe], vec![23], allowance()).unwrap());
        assert!(matches!(MonitoredDecoder::new(model.clone(), 7, 11, roster, allowance()), Err(Error::Binding)));
    }
    assert!(model.residual_contract(0).is_err()); assert!(model.residual_contract(3).is_err());
    assert!(MonitoredDecoder::new(model.clone(), 0, 11, original.clone(), allowance()).is_err());
    assert!(MonitoredDecoder::new(model.clone(), 7, 0, original.clone(), allowance()).is_err());
    release(run(model, original, allowance()).advance(0, 0, compute()).unwrap());
}

#[test]
fn one_shared_budget_spans_layers_and_tokens_without_resetting() {
    let model = fixture::model(fixture::profile(8));
    let frame_bytes = fa_reference::action::consequence::activation::HEADER_BYTES + 4 * model.profile().shape().hidden;
    let first_only = RefinementBudget { encoded_bytes: frame_bytes, probe_coordinates: 4 };
    let mut guarded = run(model.clone(), monitors(&model), first_only);
    let report = held(guarded.advance(0, 0, compute()).unwrap());
    assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(report.layers()[0].report.outcome(), MonitorOutcome::NoAlarm);
    assert!(report.layers()[1].report.steps().is_empty());
    assert_eq!(guarded.monitoring_work().encoded_bytes, frame_bytes);
    assert_eq!(guarded.remaining_budget(), RefinementBudget { encoded_bytes: 0, probe_coordinates: 0 });
    let two_tokens = RefinementBudget { encoded_bytes: 4 * frame_bytes, probe_coordinates: 16 };
    let mut guarded = run(model.clone(), monitors(&model), two_tokens);
    release(guarded.advance(0, 0, compute()).unwrap());
    release(guarded.advance_greedy(1, compute()).unwrap());
    let report = held(guarded.advance_greedy(2, compute()).unwrap());
    assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(report.unreviewed_layers(), 1);
    assert_eq!(guarded.decoder_work().tokens, 3);
    assert_eq!(guarded.monitoring_work().encoded_bytes, two_tokens.encoded_bytes);
}

#[test]
fn shared_allowance_cannot_enlarge_a_monitor_local_limit() {
    let model = fixture::model(fixture::profile(8)); let mut roster = monitors(&model);
    roster.insert(1, detector(&model, 1, 1_000_000.0, RefinementBudget { encoded_bytes: 0, probe_coordinates: 0 }));
    let mut guarded = run(model.clone(), roster, allowance());
    let report = held(guarded.advance(0, 0, compute()).unwrap());
    assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(guarded.monitoring_work().encoded_bytes, 0);
    let source = model.session(7).unwrap().advance(0, 0, compute()).unwrap().layers.remove(0).residual;
    let monitor = detector(&model, 1, 1_000_000.0, allowance());
    assert_eq!(monitor.analyze(source.source()).unwrap(), monitor.analyze_with_budget(source.source(), allowance()).unwrap());
    assert_eq!(monitor.analyze_with_budget(source.source(), RefinementBudget { encoded_bytes: 0, probe_coordinates: 0 }).unwrap().outcome(),
        MonitorOutcome::BudgetExhausted);
}

#[test]
fn request_preflight_does_not_poison_or_charge_a_still_unused_session() {
    let model = fixture::model(fixture::profile(8)); let mut guarded = run(model.clone(), monitors(&model), allowance());
    assert!(matches!(guarded.advance_greedy(0, compute()), Err(Error::Incomplete)));
    assert!(matches!(guarded.advance(1, 0, compute()), Err(Error::Stale)));
    assert!(matches!(guarded.advance(0, 99, compute()), Err(Error::InvalidInput)));
    assert!(matches!(guarded.advance(0, 0, DecoderBudget { scalar_products: 0 }), Err(Error::Limit)));
    assert!(matches!(guarded.advance(0, 0, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS + 1 }), Err(Error::Limit)));
    assert_eq!(guarded.position(), 0); assert_eq!(guarded.decoder_work(), DecoderWork::default());
    assert_eq!(guarded.monitoring_work(), MonitoringWork::default());
    assert_eq!(guarded.status(), MonitoringStatus::Ready);
    release(guarded.advance(0, 0, compute()).unwrap());
    assert_eq!(guarded.estimate(2).unwrap(), model.estimate(1, 2).unwrap());
}

#[test]
fn admitted_arithmetic_failure_latches_instead_of_exposing_unreviewed_state() {
    let profile = fixture::profile(8); let shape = profile.shape(); let mut layers = fixture::zero_layers(&profile);
    layers[0].queries.fill(f32::MAX);
    let model = DecoderModel::new(profile, vec![1.0; shape.vocabulary * shape.hidden], layers,
        vec![1.0; shape.hidden], vec![0.0; shape.vocabulary * shape.hidden]).unwrap();
    let mut guarded = run(model.clone(), monitors(&model), allowance());
    assert!(matches!(guarded.advance(0, 0, compute()), Err(Error::Overflow)));
    assert_eq!(guarded.status(), MonitoringStatus::Failed(Error::Overflow));
    assert_eq!(guarded.position(), 0); assert_eq!(guarded.decoder_work(), DecoderWork::default());
    assert_eq!(guarded.monitoring_work(), MonitoringWork::default());
    assert!(guarded.last_review().is_none());
    assert!(matches!(guarded.advance(0, 1, compute()), Err(Error::WrongState)));
}

#[test]
fn imported_safetensors_use_the_same_monitor_path_after_source_owners_drop() {
    let bytes = include_bytes!("fixtures/decoder_mixed.safetensors");
    let (model, _) = DecoderModel::from_safetensors(fixture::profile(16), bytes).unwrap();
    let mut raw = model.session(7).unwrap();
    let mut guarded = run(model.clone(), monitors(&model), allowance()); drop(model);
    for token in [1, 2, 0, 4] {
        let expected = raw.advance(raw.position(), token, compute()).unwrap();
        let actual = release(guarded.advance(guarded.position(), token, compute()).unwrap());
        compare_steps(&actual, &expected);
    }
}
