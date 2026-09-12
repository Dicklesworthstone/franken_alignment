use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::*;
use fa_reference::action::consequence::activation::tensor::kv::experiment::{KvCell, KvEdit, KvEditScope, KvSide};
use fa_reference::Error;
use std::collections::BTreeMap;
#[path = "support/decoder_fixture.rs"] mod fixture;
#[path = "support/pretrained_fixture.rs"] mod pretrained;
#[path = "support/decoder_observation.rs"] mod observation;
use observation::words as source_words;

fn full() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn budget(plan: &DecoderIntervention, count: usize) -> DecoderComparisonBudget {
    DecoderComparisonBudget {
        scalar_products: 2 * plan.source().model().estimate(plan.source().tokens().len(), count).unwrap().scalar_products().unwrap(),
        retained_logit_values: 2 * count * plan.source().model().profile().shape().vocabulary,
    }
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn choice_model(layers: usize, scale: f32) -> DecoderModel {
    let p = DecoderProfile::new(fixture::profile(8).identity(), DecoderShape {
        vocabulary: 2, hidden: 2, intermediate: 2, layers, query_heads: 1, cache_heads: 1, context: 8,
    }, 1e-5, 10000.0).unwrap();
    let identity = vec![1.0, 0.0, 0.0, 1.0];
    let mut weights = fixture::zero_layers(&p);
    for (index, layer) in weights.iter_mut().enumerate() {
        layer.values = identity.clone();
        layer.attention_output = identity.iter().map(|value| value * scale).collect();
        if index > 0 { layer.keys = identity.clone(); }
    }
    DecoderModel::new(p, identity, weights, vec![1.0; 2], vec![1.0, 0.0, -1.0, 0.0]).unwrap()
}
fn changed(model: &DecoderModel, overflow: bool) -> DecoderIntervention {
    let source = model.recompute(1, &[0], full()).unwrap().checkpoint().unwrap();
    let old = source_words(source.cache().layer(1).unwrap().token(0).unwrap().value())[0];
    let replacement = if overflow { f32::MAX } else { -f32::from_bits(old) };
    let edits = BTreeMap::from([(1, DecoderLayerIntervention {
        scope: KvEditScope { first_position: 0, token_count: 1, keys: false, values: true },
        edits: vec![KvEdit { cell: KvCell { side: KvSide::Value, position: 0, head: 0, channel: 0 },
            expected_bits: old, replacement_bits: replacement.to_bits() }],
    })]);
    source.intervene(11, edits, 1).unwrap()
}

#[test]
fn paired_control_and_noop_match_original_greedy_execution_and_combined_work() {
    let model = fixture::model(fixture::profile(16));
    let mut original = model.recompute(1, &[0, 2, 1], full()).unwrap();
    let plan = original.checkpoint().unwrap().intervene(2, BTreeMap::new(), 0).unwrap();
    let result = plan.compare_greedy(3, 8, budget(&plan, 8)).unwrap();
    assert_eq!(result.policy(), DecoderContinuationPolicy::GreedyAfterFirstToken);
    assert_eq!(result.work().tokens, 16);
    assert_eq!(result.work().scalar_products().unwrap(), budget(&plan, 8).scalar_products);
    assert_eq!(result.retained_logit_values(), 2 * 8 * 6);
    assert_eq!(result.first_different_logits(), None);
    assert_eq!(result.first_different_consumed_token(), None);
    assert_eq!(result.first_different_next_choice(), None);
    for (offset, step) in result.steps().iter().enumerate() {
        let token = if offset == 0 { 3 } else { original.greedy_token().unwrap() };
        let expected = original.advance(original.position(), token, full()).unwrap();
        assert_eq!(step.control_token, token); assert_eq!(step.intervention_token, token);
        assert_eq!(bits(&step.control_logits), bits(&expected.logits));
        assert_eq!(bits(&step.control_logits), bits(&step.intervention_logits));
        assert_eq!(step.changed_logit_words, 0); assert_eq!(step.l2_logit_delta, 0.0);
        assert_eq!(step.control_next_token, original.greedy_token().unwrap());
    }
}

#[test]
fn greedy_feedback_divergence_is_not_confused_with_the_common_first_token() {
    let model = choice_model(1, 1.0); let plan = changed(&model, false);
    let result = plan.compare_greedy(1, 3, budget(&plan, 3)).unwrap();
    assert_eq!(result.first_different_logits(), Some(1));
    assert_eq!(result.first_different_next_choice(), Some(2));
    assert_eq!(result.first_different_consumed_token(), Some(2));
    assert_eq!((result.steps()[0].control_token, result.steps()[0].intervention_token), (1, 1));
    assert_eq!((result.steps()[0].control_next_token, result.steps()[0].intervention_next_token), (0, 1));
    assert_eq!((result.steps()[1].control_token, result.steps()[1].intervention_token), (0, 1));
    assert_eq!(result.steps()[0].changed_logit_words, 2);
    assert!(result.steps()[0].max_abs_logit_delta > 0.0);
    let expected_l2 = result.steps()[0].control_logits.iter().zip(result.steps()[0].intervention_logits.iter())
        .map(|(a,b)| (f64::from(*b) - f64::from(*a)).powi(2)).sum::<f64>().sqrt();
    assert_eq!(result.steps()[0].l2_logit_delta, expected_l2);
}

#[test]
fn teacher_forcing_preserves_identical_inputs_despite_different_next_choices() {
    let model = choice_model(1, 1.0); let plan = changed(&model, false);
    let tokens = [1, 0, 1, 1];
    let result = plan.compare_forced(&tokens, budget(&plan, tokens.len())).unwrap();
    assert_eq!(result.policy(), DecoderContinuationPolicy::TeacherForced);
    assert_eq!(result.first_different_consumed_token(), None);
    assert_eq!(result.first_different_next_choice(), Some(2));
    let mut control = plan.session(DecoderExperimentArm::Control);
    let mut intervention = plan.session(DecoderExperimentArm::Intervention);
    for (token, step) in tokens.into_iter().zip(result.steps()) {
        let a = control.advance(step.position, token, full()).unwrap();
        let b = intervention.advance(step.position, token, full()).unwrap();
        assert_eq!(step.control_token, token); assert_eq!(step.intervention_token, token);
        assert_eq!(bits(&step.control_logits), bits(&a.logits));
        assert_eq!(bits(&step.intervention_logits), bits(&b.logits));
    }
}

#[test]
fn diagnostic_final_choice_is_not_reported_as_a_consumed_token() {
    let plan = changed(&choice_model(1, 1.0), false);
    let result = plan.compare_greedy(1, 1, budget(&plan, 1)).unwrap();
    assert_eq!(result.steps().len(), 1);
    assert_eq!(result.first_different_next_choice(), Some(2));
    assert_eq!(result.first_different_consumed_token(), None);
}

#[test]
fn full_pair_preflight_precedes_numerical_failure_and_no_partial_report_escapes() {
    let plan = changed(&choice_model(1, 4.0), true);
    let exact = budget(&plan, 2);
    let before = plan.source().cache().encode().unwrap();
    assert!(matches!(plan.compare_greedy(1, 2, DecoderComparisonBudget { scalar_products: exact.scalar_products - 1, ..exact }), Err(Error::Limit)));
    assert!(matches!(plan.compare_greedy(1, 2, DecoderComparisonBudget { retained_logit_values: exact.retained_logit_values - 1, ..exact }), Err(Error::Limit)));
    assert!(matches!(plan.compare_forced(&[1, 2], exact), Err(Error::InvalidInput)));
    assert!(matches!(plan.compare_greedy(1, 2, exact), Err(Error::Overflow)));
    assert_eq!(plan.source().cache().encode().unwrap(), before);
    let mut control = plan.session(DecoderExperimentArm::Control);
    control.advance(1, 1, full()).unwrap();
}

#[test]
fn bounded_horizon_and_retention_limits_are_checked_before_rollout() {
    let model = choice_model(1, 1.0);
    let source = model.recompute(1, &[0], full()).unwrap().checkpoint().unwrap();
    let plan = source.intervene(2, BTreeMap::new(), 0).unwrap();
    let exact = budget(&plan, 7);
    let result = plan.compare_greedy(1, 7, exact).unwrap();
    assert_eq!(result.steps().last().unwrap().position, 7);
    assert!(matches!(plan.compare_greedy(1, 8, exact), Err(Error::Limit)));
    assert!(plan.compare_greedy(1, usize::MAX, exact).is_err());
    assert!(matches!(plan.compare_greedy(1, 0, exact), Err(Error::InvalidInput)));
    assert!(matches!(plan.compare_forced(&[], exact), Err(Error::InvalidInput)));
    assert!(matches!(plan.compare_greedy(1, 1, DecoderComparisonBudget {
        retained_logit_values: MAX_COMPARISON_LOGIT_VALUES + 1, ..exact }), Err(Error::Limit)));
}

#[test]
fn an_early_layer_edit_changes_later_layers_new_kv_not_only_old_attention() {
    let plan = changed(&choice_model(2, 1.0), false);
    let mut control = plan.session(DecoderExperimentArm::Control);
    let mut intervention = plan.session(DecoderExperimentArm::Intervention);
    control.advance(1, 1, full()).unwrap(); intervention.advance(1, 1, full()).unwrap();
    assert!([KvSide::Key, KvSide::Value].into_iter().all(|side| {
        (0..2).any(|channel| {
            let cell = KvCell { side, position: 1, head: 0, channel };
            control.bits(2, cell).unwrap() != intervention.bits(2, cell).unwrap()
        })
    }));
    let result = plan.compare_forced(&[1, 0, 1], budget(&plan, 3)).unwrap();
    assert_eq!(bits(&result.steps()[0].control_logits), bits(control.logits().unwrap()));
    assert_eq!(bits(&result.steps()[0].intervention_logits), bits(intervention.logits().unwrap()));
    assert_eq!(result.first_different_logits(), Some(1));
}

#[test]
fn imported_weight_model_flows_through_original_checkpoint_and_paired_continuation() {
    let (model, _) = DecoderModel::from_safetensors(pretrained::profile(16),
        include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap();
    let mut original = model.recompute(1, &[0, 2], full()).unwrap();
    let plan = original.checkpoint().unwrap().intervene(2, BTreeMap::new(), 0).unwrap();
    let result = plan.compare_forced(&[1, 3, 0], budget(&plan, 3)).unwrap();
    drop(plan); drop(model);
    for step in result.steps() {
        let expected = original.advance(step.position, step.control_token, full()).unwrap();
        assert_eq!(bits(&step.control_logits), bits(&expected.logits));
        assert_eq!(bits(&step.control_logits), bits(&step.intervention_logits));
    }
    assert_eq!(result.plan().source().tokens(), &[0, 2]);
}
