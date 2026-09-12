//! Synthetic decoder/probe controls. No trained-model or runtime qualification.
#[path = "support/decoder_fixture.rs"]
mod fixture;
use fa_reference::action::consequence::activation::{HEADER_BYTES, ProgressiveFrame, SourceFrame};
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::monitor::decoder::*;
use fa_reference::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::*;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::*;
use fa_reference::Error;
use std::collections::BTreeMap;

fn allowance() -> RefinementBudget { RefinementBudget { encoded_bytes: 1_000_000, probe_coordinates: 1_000_000 } }
fn compute() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn budget(vocabulary: usize) -> SampleBudget { SampleBudget { decoder: compute(), sampling: SamplingBudget { vocabulary } } }
fn start(vocabulary: usize, top_k: usize) -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(13, 2, vocabulary, 0.7, top_k, 1.0).unwrap(), stream: 99, seed: 0 }
}
fn detector(model: &DecoderModel, layer: u64, threshold: f32) -> RefinementMonitor {
    let contract = model.residual_contract(layer).unwrap();
    let mut weights = vec![0.0; contract.dimensions()]; weights[0] = 1.0;
    RefinementMonitor::new(vec![LinearProbe::new(1, 1, contract.profile(), &weights, 0.0, threshold).unwrap()],
        vec![23], allowance()).unwrap()
}
fn monitors(model: &DecoderModel) -> BTreeMap<u64, RefinementMonitor> {
    (1..=model.profile().shape().layers as u64).map(|layer| (layer, detector(model, layer, 1_000_000.0))).collect()
}
fn guarded(model: DecoderModel, roster: BTreeMap<u64, RefinementMonitor>, allowance: RefinementBudget,
    start: SamplingStart) -> MonitoredSampledDecoder
{ MonitoredSampledDecoder::new(model, 7, 11, roster, allowance, start).unwrap() }
fn released(step: MonitoredSampledStep) -> ReviewedSampledStep {
    match step { MonitoredSampledStep::Released(step) => step, other => panic!("expected release: {other:?}") }
}
fn forced(step: MonitoredStep) -> ReviewedStep {
    match step { MonitoredStep::Released(step) => step, other => panic!("expected release: {other:?}") }
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }
fn source_bits(source: &SourceFrame) -> Vec<u32> {
    let encoded = source.encode_initial(23).unwrap();
    bits(&ProgressiveFrame::from_initial(&source.verify_block(&encoded).unwrap()).unwrap().exact_values().unwrap())
}
fn same_step(actual: &ReviewedStep, expected: &DecoderStep) {
    assert_eq!(actual.step().token, expected.token); assert_eq!(actual.step().position, expected.position);
    assert_eq!(bits(&actual.step().logits), bits(&expected.logits));
    assert_eq!(actual.review().outcome(), MonitorOutcome::NoAlarm);
    assert_eq!(actual.review().unreviewed_layers(), 0);
    for (a, b) in actual.step().layers.iter().zip(&expected.layers) {
        assert_eq!(source_bits(a.query.source()), source_bits(b.query.source()));
        assert_eq!(source_bits(a.residual.source()), source_bits(b.residual.source()));
    }
}
fn two_token_model(overflow: bool) -> DecoderModel {
    let p = DecoderProfile::new(fixture::profile(8).identity(), DecoderShape {
        vocabulary: 2, hidden: 2, intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 8,
    }, 1e-5, 10000.0).unwrap();
    let layers = fixture::zero_layers(&p);
    let embeddings = if overflow { vec![0.0, 1.0, 1.0, 1.0] } else { vec![0.0, 1.0, 1.0, 0.0] };
    let output = if overflow { vec![f32::MAX, 0.0, f32::MAX, 0.0] } else { vec![0.0; 4] };
    DecoderModel::new(p, embeddings, layers, if overflow { vec![2.0, 1.0] } else { vec![1.0; 2] }, output).unwrap()
}

#[test]
fn monitored_sampling_matches_every_raw_choice_logit_and_tap_for_32_steps() {
    let model = fixture::model(fixture::profile(48));
    let mut raw = model.sampled_session(7, start(6, 4)).unwrap();
    let mut run = guarded(model.clone(), monitors(&model), allowance(), start(6, 4));
    let source = run.observation();
    assert_eq!(source.availability(), DecoderAvailability::Empty);
    for token in [0, 3, 1, 5] {
        let position = run.position();
        same_step(&forced(run.advance_forced(position, token, compute()).unwrap()),
            &raw.advance_forced(position, token, compute()).unwrap());
    }
    assert_eq!(run.sampled_draws(), 0);
    for _ in 0..32 {
        let old = source.capture().unwrap(); let position = run.position();
        let expected = raw.advance_sampled(position, budget(6)).unwrap();
        let actual = released(run.advance_sampled(position, budget(6)).unwrap());
        assert_eq!(actual.choice(), &expected.choice); same_step(actual.reviewed(), &expected.computation);
        assert_eq!(source.validate(&old), Err(Error::Stale));
        let current = source.capture().unwrap(); source.validate(&current).unwrap();
        assert_eq!(current.tokens(), raw.tokens());
    }
    assert_eq!(run.sampled_draws(), 32); assert_eq!(run.position(), 36);
    assert_eq!(run.decoder_work(), raw.work()); assert_eq!(run.monitoring_work().frame_reviews, 72);
}

#[test]
fn forced_positions_do_not_advance_the_sampled_lineage() {
    let model = fixture::model(fixture::profile(16));
    let mut raw = model.sampled_session(7, start(6, 0)).unwrap();
    let mut run = guarded(model.clone(), monitors(&model), allowance(), start(6, 0));
    for position in 0..12 {
        if position % 3 == 0 {
            let token = (position % 6) as u32;
            same_step(&forced(run.advance_forced(position, token, compute()).unwrap()),
                &raw.advance_forced(position, token, compute()).unwrap());
        } else {
            let a = released(run.advance_sampled(position, budget(6)).unwrap());
            let b = raw.advance_sampled(position, budget(6)).unwrap();
            assert_eq!(a.choice(), &b.choice); same_step(a.reviewed(), &b.computation);
        }
        assert_eq!(run.sampled_draws(), raw.sampler_state().draws());
    }
    assert_eq!(run.sampled_draws(), 8);
}

#[test]
fn sampled_alarm_consumes_the_computed_draw_but_cannot_release_or_reroll() {
    let model = two_token_model(false);
    for alarm_layer in [1, 2] {
        let mut roster = monitors(&model); roster.insert(alarm_layer, detector(&model, alarm_layer, 0.5));
        let mut run = guarded(model.clone(), roster, allowance(), start(2, 0));
        forced(run.advance_forced(0, 0, compute()).unwrap());
        let source = run.observation(); let old = source.capture().unwrap();
        let result = run.advance_sampled(1, budget(2)).unwrap();
        let MonitoredSampledStep::Held(review) = result else { panic!("sampled token escaped its alarm"); };
        assert_eq!(review.outcome(), MonitorOutcome::Alarm);
        assert_eq!(review.layers().len(), alarm_layer as usize);
        assert_eq!(review.unreviewed_layers(), 2 - alarm_layer as usize);
        assert_eq!(run.position(), 2); assert_eq!(run.sampled_draws(), 1);
        assert_eq!(run.decoder_work().tokens, 2); assert_eq!(run.status(), MonitoringStatus::Held);
        assert_eq!(source.availability(), DecoderAvailability::Held);
        assert_eq!(source.validate(&old), Err(Error::Incomplete)); assert!(source.capture().is_err());
        assert!(matches!(run.advance_sampled(2, budget(2)), Err(Error::WrongState)));
        assert!(matches!(run.advance_forced(2, 0, compute()), Err(Error::WrongState)));
        assert_eq!(run.sampled_draws(), 1);
    }
    let mut control = guarded(model.clone(), monitors(&model), allowance(), start(2, 0));
    forced(control.advance_forced(0, 0, compute()).unwrap());
    assert_eq!(released(control.advance_sampled(1, budget(2)).unwrap()).choice().token, 1);
}

#[test]
fn exhausted_shared_monitor_budget_withholds_a_computed_sample_without_refund() {
    let model = fixture::model(fixture::profile(8));
    let bytes = HEADER_BYTES + 4 * model.profile().shape().hidden;
    let mut run = guarded(model.clone(), monitors(&model),
        RefinementBudget { encoded_bytes: 2 * bytes, probe_coordinates: 8 }, start(6, 0));
    forced(run.advance_forced(0, 0, compute()).unwrap()); let old = run.observation().capture().unwrap();
    let MonitoredSampledStep::Held(review) = run.advance_sampled(1, budget(6)).unwrap() else { panic!("quota was renewed"); };
    assert_eq!(review.outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(review.unreviewed_layers(), 1); assert_eq!(run.sampled_draws(), 1);
    assert_eq!(run.position(), 2); assert_eq!(run.monitoring_work().encoded_bytes, 2 * bytes);
    assert_eq!(run.remaining_budget(), RefinementBudget { encoded_bytes: 0, probe_coordinates: 0 });
    assert_eq!(run.observation().validate(&old), Err(Error::Incomplete));
}

#[test]
fn late_vocabulary_failure_consumes_no_draw_and_has_no_greedy_fallback() {
    let model = two_token_model(true);
    let mut run = guarded(model.clone(), monitors(&model), allowance(), start(2, 0));
    forced(run.advance_forced(0, 0, compute()).unwrap());
    let old = run.observation().capture().unwrap(); let work = run.decoder_work();
    assert!(matches!(run.advance_sampled(1, budget(2)), Err(Error::Overflow)));
    assert_eq!(run.position(), 1); assert_eq!(run.sampled_draws(), 0); assert_eq!(run.decoder_work(), work);
    assert_eq!(run.status(), MonitoringStatus::Failed(Error::Overflow));
    assert_eq!(run.observation().validate(&old), Err(Error::Incomplete));
    assert!(matches!(run.advance_sampled(1, budget(2)), Err(Error::WrongState)));
    let mut control = guarded(model.clone(), monitors(&model), allowance(), start(2, 1));
    forced(control.advance_forced(0, 0, compute()).unwrap());
    assert_eq!(released(control.advance_sampled(1, budget(2)).unwrap()).choice().token, 0);
    assert_eq!(control.sampled_draws(), 1);
}

#[test]
fn invalid_predecessor_and_compute_or_sampling_limits_leave_quiet_state_current() {
    let model = fixture::model(fixture::profile(8));
    let mut run = guarded(model.clone(), monitors(&model), allowance(), start(6, 0));
    assert!(matches!(run.advance_sampled(0, budget(6)), Err(Error::Incomplete)));
    assert_eq!(run.sampled_draws(), 0); assert_eq!(run.status(), MonitoringStatus::Ready);
    forced(run.advance_forced(0, 0, compute()).unwrap());
    let source = run.observation(); let old = source.capture().unwrap(); let work = run.decoder_work();
    assert!(matches!(run.advance_sampled(0, budget(6)), Err(Error::Stale)));
    for products in [0, MAX_DECODER_PRODUCTS + 1] {
        assert!(matches!(run.advance_sampled(1, SampleBudget { decoder: DecoderBudget { scalar_products: products },
            sampling: SamplingBudget { vocabulary: 6 } }), Err(Error::Limit)));
    }
    for vocabulary in [5, MAX_DECODER_VOCABULARY + 1] {
        assert!(matches!(run.advance_sampled(1, budget(vocabulary)), Err(Error::Limit)));
    }
    source.validate(&old).unwrap(); assert_eq!(run.decoder_work(), work); assert_eq!(run.sampled_draws(), 0);
    released(run.advance_sampled(1, budget(6)).unwrap()); assert_eq!(run.sampled_draws(), 1);
}

#[test]
fn sampling_configuration_and_complete_monitor_roster_are_both_required() {
    let model = fixture::model(fixture::profile(8));
    assert!(matches!(MonitoredSampledDecoder::new(model.clone(), 7, 11, monitors(&model), allowance(), start(5, 0)), Err(Error::Binding)));
    let mut invalid = start(6, 0); invalid.stream = 0;
    assert!(matches!(MonitoredSampledDecoder::new(model.clone(), 7, 11, monitors(&model), allowance(), invalid), Err(Error::InvalidInput)));
    let mut roster = monitors(&model); roster.remove(&2);
    assert!(matches!(MonitoredSampledDecoder::new(model.clone(), 7, 11, roster, allowance(), start(6, 0)), Err(Error::Binding)));
    let mut valid = guarded(model.clone(), monitors(&model), allowance(), start(6, 0));
    forced(valid.advance_forced(0, 0, compute()).unwrap());
    released(valid.advance_sampled(1, budget(6)).unwrap());
}

#[test]
fn reviewed_observer_is_owner_bound_and_drop_withdraws_eligibility() {
    let model = fixture::model(fixture::profile(8));
    let mut a = guarded(model.clone(), monitors(&model), allowance(), start(6, 0));
    let mut b = guarded(model.clone(), monitors(&model), allowance(), start(6, 0));
    forced(a.advance_forced(0, 0, compute()).unwrap()); forced(b.advance_forced(0, 0, compute()).unwrap());
    let source = a.observation(); let evidence = source.capture().unwrap();
    assert_eq!(b.observation().validate(&evidence), Err(Error::Binding));
    drop(a); assert_eq!(source.availability(), DecoderAvailability::Closed);
    assert_eq!(source.validate(&evidence), Err(Error::Incomplete));
    assert_eq!(evidence.tokens(), &[0]);
    released(b.advance_sampled(1, budget(6)).unwrap());
}

#[test]
fn imported_mixed_weights_and_released_conversion_keep_the_original_results() {
    let (model, _) = DecoderModel::from_safetensors(fixture::profile(16), include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap();
    let mut raw = model.sampled_session(7, start(6, 3)).unwrap();
    let mut run = guarded(model.clone(), monitors(&model), allowance(), start(6, 3)); drop(model);
    for token in [1, 2, 0] {
        let position = run.position();
        same_step(&forced(run.advance_forced(position, token, compute()).unwrap()), &raw.advance_forced(position, token, compute()).unwrap());
    }
    for _ in 0..8 {
        let position = run.position(); let expected = raw.advance_sampled(position, budget(6)).unwrap();
        let actual = run.advance_sampled(position, budget(6)).unwrap().into_monitored();
        same_step(&forced(actual), &expected.computation);
    }
}

#[test]
fn debug_and_full_context_refusals_cannot_reveal_or_select_another_token() {
    let model = fixture::model(fixture::profile(2));
    let mut run = guarded(model.clone(), monitors(&model), allowance(), start(6, 0));
    forced(run.advance_forced(0, 0, compute()).unwrap());
    let step = run.advance_sampled(1, budget(6)).unwrap();
    let text = format!("{run:?} {step:?}");
    for secret in ["random_word", "logits:", "seed:", "state:", "token:"] { assert!(!text.contains(secret), "{secret}"); }
    let source = run.observation(); let old = source.capture().unwrap();
    assert!(matches!(run.advance_sampled(2, budget(6)), Err(Error::Limit)));
    assert_eq!(run.sampled_draws(), 1); assert_eq!(run.status(), MonitoringStatus::Ready);
    source.validate(&old).unwrap();
}
