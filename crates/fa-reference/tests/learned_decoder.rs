//! Actual decoder computations, not a surrogate compression-only score.
//! Fixture weights are synthetic; these tests do not qualify a serving model.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderCheckpoint, DecoderIdentity, DecoderModel, DecoderProfile,
    DecoderShape, MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::{
    DecoderExperimentArm, DecoderIntervention, DecoderLayerIntervention,
    comparison::{DecoderContinuationPolicy, MAX_COMPARISON_LOGIT_VALUES},
    learned::{LearnedDecoder, LearnedDecoderBudget, MAX_RECONSTRUCTION_PRODUCTS,
        comparison::LearnedComparisonBudget},
};
use fa_reference::action::consequence::activation::tensor::kv::experiment::{KvCell, KvSide, KvEdit, KvEditScope};
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{
    CompressionBudget, FitBudget, GroupKey, LearnedKvCodec, LearnedKvPolicy,
};
use fa_reference::Error;
use std::collections::BTreeMap;

fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn allowance() -> LearnedDecoderBudget {
    LearnedDecoderBudget { decoder: budget(), reconstruction_products: MAX_RECONSTRUCTION_PRODUCTS }
}
fn paired_budget() -> LearnedComparisonBudget {
    LearnedComparisonBudget { compression: CompressionBudget::default(), scalar_products: MAX_DECODER_PRODUCTS,
        reconstruction_products: MAX_RECONSTRUCTION_PRODUCTS, retained_logit_values: MAX_COMPARISON_LOGIT_VALUES }
}
fn words(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn train(model: &DecoderModel, tokens: &[u32], rank: usize) -> LearnedKvCodec {
    let cache = model.recompute(11, tokens, budget()).unwrap().cache_image().unwrap();
    LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, rank, 8).unwrap(),
        &BTreeMap::from([(101, cache)]), FitBudget::default()).unwrap()
}
fn checkpoint(model: &DecoderModel, tokens: &[u32]) -> DecoderCheckpoint {
    model.recompute(21, tokens, budget()).unwrap().checkpoint().unwrap()
}
fn cells(model: &DecoderModel, end: u64) -> Vec<(u64, KvCell)> {
    let mut result = Vec::new();
    for layer in 1..=model.profile().shape().layers as u64 {
        for position in 0..end {
            for side in [KvSide::Key, KvSide::Value] {
                for head in 0..model.profile().shape().cache_heads {
                    for channel in 0..model.profile().head_width() {
                        result.push((layer, KvCell { side, position, head, channel }));
                    }
                }
            }
        }
    }
    result
}
fn scalar_oracle(source: &DecoderCheckpoint, learned: &LearnedDecoder) -> DecoderIntervention {
    let unchanged = source.intervene(700, BTreeMap::new(), 0).unwrap();
    let original = unchanged.session(DecoderExperimentArm::Control);
    let mut layers = BTreeMap::new(); let mut count = 0;
    for (layer, cell) in cells(source.model(), source.tokens().len() as u64) {
        let entry = layers.entry(layer).or_insert_with(|| DecoderLayerIntervention {
            scope: KvEditScope { first_position: 0, token_count: source.tokens().len(), keys: true, values: true },
            edits: Vec::new(),
        });
        entry.edits.push(KvEdit { cell, expected_bits: original.bits(layer, cell).unwrap(),
            replacement_bits: learned.image().bits(layer, cell).unwrap() });
        count += 1;
    }
    source.intervene(701, layers, count).unwrap()
}
fn small_profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 4, hidden: 2, intermediate: 2, layers: 1,
            query_heads: 1, cache_heads: 1, context: 64 }, 0.00001, 10000.0).unwrap()
}
fn rare_model() -> DecoderModel {
    let p = small_profile(); let mut layers = fixture::zero_layers(&p);
    layers[0].values = vec![1.0, 0.0, 0.0, 1.0];
    layers[0].attention_output = vec![1.0, 0.0, 0.0, 1.0];
    DecoderModel::new(p, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 1.0, 0.0], layers,
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}
fn late_failure_model() -> DecoderModel {
    let p = small_profile(); let layers = fixture::zero_layers(&p);
    DecoderModel::new(p, vec![1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0], layers,
        vec![1.0; 2], vec![f32::MAX, f32::MAX, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}

#[test]
fn learned_prefix_matches_explicit_scalar_interventions_for_32_forced_and_greedy_steps() {
    let model = fixture::model(fixture::profile(64));
    let codec = train(&model, &[0, 1, 2, 3, 4, 5, 1, 4], 1);
    let source = checkpoint(&model, &[4, 2, 0]);
    let original_bytes = source.cache().encode().unwrap();
    let learned = source.learned_experiment(500, 201, &codec, CompressionBudget::default()).unwrap();
    assert!(!learned.report().training_source_overlap);
    let edits = scalar_oracle(&source, &learned);
    for greedy in [false, true] {
        let mut actual = learned.session();
        let mut oracle = edits.session(DecoderExperimentArm::Intervention);
        assert_eq!(actual.greedy_token(), Err(Error::Incomplete));
        for offset in 0..32 {
            let token = if greedy && offset != 0 {
                assert_eq!(actual.greedy_token().unwrap(), oracle.greedy_token().unwrap());
                actual.greedy_token().unwrap()
            } else { (offset % 6) as u32 };
            let position = actual.position();
            let step = actual.advance(position, token, allowance()).unwrap();
            let expected = oracle.advance(position, token, budget()).unwrap();
            assert_eq!(words(&step.logits), words(&expected.logits));
            assert_eq!(step.work, expected.work);
            assert_eq!(step.reconstruction_products, learned.reconstruction_products_for(1).unwrap());
            assert_eq!(actual.reconstruction_products(), learned.reconstruction_products_for(offset + 1).unwrap());
            assert_eq!(actual.work(), oracle.work());
            for (layer, cell) in cells(&model, actual.position()) {
                assert_eq!(actual.bits(layer, cell).unwrap(), oracle.bits(layer, cell).unwrap());
            }
        }
        assert_eq!(actual.continuation_tokens(), oracle.continuation_tokens());
    }
    assert_eq!(source.cache().encode().unwrap(), original_bytes);
}

#[test]
fn learned_erasure_changes_an_actual_next_token_and_full_rank_preserves_it() {
    let model = rare_model();
    let low_rank = train(&model, &[0, 1], 1); let full_rank = train(&model, &[0, 1], 2);
    let source = checkpoint(&model, &[2]);
    let result = source.compare_learned_greedy(1, 201, &low_rank, 3, 2, paired_budget()).unwrap();
    assert_eq!(result.steps()[0].control_token, 3);
    assert_eq!(result.steps()[0].intervention_token, 3);
    assert_eq!(result.steps()[0].control_next_token, 1);
    assert_eq!(result.steps()[0].intervention_next_token, 0);
    assert!(result.steps()[0].control_logits[1] > 0.0);
    assert_eq!(result.steps()[0].intervention_logits[1], 0.0);
    assert_eq!(result.first_different_logits(), Some(1));
    assert_eq!(result.first_different_next_choice(), Some(2));
    assert_eq!(result.first_different_consumed_token(), Some(2));
    let error = result.learned().report().groups[&GroupKey { layer: 1, side: KvSide::Value, head: 0 }];
    assert_eq!(error.nonzero_to_zero, 1);
    let control = source.compare_learned_greedy(2, 201, &full_rank, 3, 16, paired_budget()).unwrap();
    assert!(control.steps().iter().all(|step| step.changed_logit_words == 0));
    assert_eq!(control.first_different_logits(), None);
    assert_eq!(control.first_different_consumed_token(), None);
    assert_eq!(control.first_different_next_choice(), None);
    assert_eq!(control.steps()[0].intervention_next_token, 1);
}

#[test]
fn diagnostic_choices_are_not_misreported_as_consumed_tokens_and_forcing_stays_explicit() {
    let model = rare_model(); let codec = train(&model, &[0, 1], 1); let source = checkpoint(&model, &[2]);
    let one = source.compare_learned_greedy(1, 201, &codec, 3, 1, paired_budget()).unwrap();
    assert_eq!(one.steps().len(), 1);
    assert_eq!(one.first_different_next_choice(), Some(2));
    assert_eq!(one.first_different_consumed_token(), None);
    let forced = source.compare_learned_forced(2, 201, &codec, &[3, 3, 0, 1], paired_budget()).unwrap();
    assert_eq!(forced.policy(), DecoderContinuationPolicy::TeacherForced);
    assert_eq!(forced.first_different_consumed_token(), None);
    assert!(forced.steps().iter().all(|step| step.control_token == step.intervention_token));
    assert_eq!(forced.reconstruction_products(), forced.reserved_reconstruction_products());
    assert_eq!(forced.reconstruction_products(), forced.learned().reconstruction_products_for(4).unwrap());
}

#[test]
fn the_paired_control_agrees_bitwise_with_uninterrupted_original_inference() {
    let model = fixture::model(fixture::profile(64));
    let codec = train(&model, &[0, 1, 2, 3, 4, 5], 1);
    let mut original = model.recompute(21, &[4, 2, 0], budget()).unwrap();
    let source = original.checkpoint().unwrap();
    let suffix: Vec<u32> = (0..32).map(|i| (i % 6) as u32).collect();
    let paired = source.compare_learned_forced(5, 201, &codec, &suffix, paired_budget()).unwrap();
    for step in paired.steps() {
        let raw = original.advance(step.position, step.control_token, budget()).unwrap();
        assert_eq!(words(&raw.logits), words(&step.control_logits));
    }
    assert_eq!(paired.work().tokens, 64);
    assert_eq!(paired.work().scalar_products().unwrap(), model.estimate(3, 32).unwrap().scalar_products().unwrap() * 2);
    assert_eq!(paired.reconstruction_products(), 2 * 2 * 4 * 3 * 32);
}

#[test]
fn a_late_vocabulary_overflow_leaves_suffix_logits_and_reconstruction_counters_unchanged() {
    let model = late_failure_model(); let codec = train(&model, &[0, 1], 1);
    let source = checkpoint(&model, &[0]);
    let plan = source.learned_experiment(1, 201, &codec, CompressionBudget::default()).unwrap();
    let mut session = plan.session();
    session.advance(1, 3, allowance()).unwrap();
    let logits = words(session.logits().unwrap()); let work = session.work();
    let counts = session.reconstruction_products(); let tokens = session.continuation_tokens().to_vec();
    let cached: Vec<_> = cells(&model, session.position()).into_iter()
        .map(|(layer, cell)| (layer, cell, session.bits(layer, cell).unwrap())).collect();
    assert_eq!(session.advance(2, 2, allowance()).unwrap_err(), Error::Overflow);
    assert_eq!(session.position(), 2); assert_eq!(session.work(), work);
    assert_eq!(session.reconstruction_products(), counts);
    assert_eq!(session.continuation_tokens(), tokens); assert_eq!(words(session.logits().unwrap()), logits);
    for (layer, cell, bits) in cached { assert_eq!(session.bits(layer, cell).unwrap(), bits); }
    assert_eq!(session.advance(2, 2, allowance()).unwrap_err(), Error::Overflow);
    session.advance(2, 3, allowance()).unwrap();
    assert_eq!(session.position(), 3);
    assert_eq!(session.reconstruction_products(), counts + plan.reconstruction_products_for(1).unwrap());
}

#[test]
fn encoding_inference_logit_retention_and_reconstruction_budgets_each_have_a_real_boundary() {
    let model = rare_model(); let codec = train(&model, &[0, 1], 1); let source = checkpoint(&model, &[2]);
    let (_, compression) = codec.compress(source.cache(), CompressionBudget::default()).unwrap();
    let plan = source.learned_experiment(1, 201, &codec, CompressionBudget::default()).unwrap();
    let exact = LearnedComparisonBudget {
        compression: CompressionBudget { source_values: source.cache().normalized_values(),
            encoded_bytes: compression.encoded_bytes, work_units: compression.work_units_reserved },
        scalar_products: model.estimate(1, 3).unwrap().scalar_products().unwrap() * 2,
        reconstruction_products: plan.reconstruction_products_for(3).unwrap(),
        retained_logit_values: 3 * 4 * 2,
    };
    assert!(source.compare_learned_forced(1, 201, &codec, &[3, 0, 1], exact).is_ok());
    let before = source.cache().encode().unwrap();
    for smaller in [LearnedComparisonBudget { scalar_products: exact.scalar_products - 1, ..exact },
        LearnedComparisonBudget { reconstruction_products: exact.reconstruction_products - 1, ..exact },
        LearnedComparisonBudget { retained_logit_values: exact.retained_logit_values - 1, ..exact },
        LearnedComparisonBudget { compression: CompressionBudget {
            encoded_bytes: exact.compression.encoded_bytes - 1, ..exact.compression }, ..exact }]
    { assert_eq!(source.compare_learned_forced(1, 201, &codec, &[3, 0, 1], smaller).unwrap_err(), Error::Limit); }
    assert_eq!(source.cache().encode().unwrap(), before);
    let mut session = plan.session();
    let step_budget = LearnedDecoderBudget { decoder: DecoderBudget {
        scalar_products: model.estimate(1, 1).unwrap().scalar_products().unwrap() },
        reconstruction_products: plan.reconstruction_products_for(1).unwrap() };
    assert_eq!(session.advance(1, 3, LearnedDecoderBudget { reconstruction_products: step_budget.reconstruction_products - 1,
        ..step_budget }).unwrap_err(), Error::Limit);
    assert_eq!(session.position(), 1); assert_eq!(session.reconstruction_products(), 0);
    assert_eq!(session.advance(1, 3, LearnedDecoderBudget { decoder: DecoderBudget {
        scalar_products: step_budget.decoder.scalar_products - 1 }, ..step_budget }).unwrap_err(), Error::Limit);
    assert_eq!(session.logits(), Err(Error::Incomplete));
    session.advance(1, 3, step_budget).unwrap();
}

#[test]
fn source_and_training_owners_can_drop_while_siblings_keep_independent_continuations() {
    let model = fixture::model(fixture::profile(64));
    let codec = train(&model, &[0, 1, 2, 3, 4, 5], 1);
    let source = checkpoint(&model, &[4, 2, 0]);
    let plan = source.learned_experiment(1, 201, &codec, CompressionBudget::default()).unwrap();
    let encoded = plan.image().encode().unwrap();
    let mut left = plan.session(); let mut right = plan.session();
    drop(plan); drop(source); drop(codec); drop(model);
    let first = left.advance(3, 0, allowance()).unwrap();
    assert_eq!(right.position(), 3); assert_eq!(right.reconstruction_products(), 0);
    assert_eq!(right.logits(), Err(Error::Incomplete));
    let second = right.advance(3, 0, allowance()).unwrap();
    assert_eq!(words(&first.logits), words(&second.logits));
    left.advance(4, 1, allowance()).unwrap();
    assert_eq!(right.position(), 4); assert_eq!(right.continuation_tokens(), &[0]);
    right.advance(4, 2, allowance()).unwrap();
    assert_eq!(left.continuation_tokens(), &[0, 1]); assert_eq!(right.continuation_tokens(), &[0, 2]);
    assert_eq!(left.plan().image().encode().unwrap(), encoded);
    assert_eq!(right.plan().image().encode().unwrap(), encoded);
}

#[test]
fn stale_positions_unseen_tokens_training_overlap_and_context_bounds_refuse_without_advance() {
    let model = rare_model(); let codec = train(&model, &[0, 1], 1);
    let training = model.recompute(11, &[0, 1], budget()).unwrap().checkpoint().unwrap();
    assert_eq!(training.learned_experiment(1, 201, &codec, CompressionBudget::default()).unwrap_err(), Error::Duplicate);
    let source = checkpoint(&model, &[2]);
    assert_eq!(source.learned_experiment(1, 101, &codec, CompressionBudget::default()).unwrap_err(), Error::Duplicate);
    let plan = source.learned_experiment(1, 201, &codec, CompressionBudget::default()).unwrap();
    let mut session = plan.session();
    assert!(source.logits().is_some());
    assert_eq!(session.advance_greedy(1, allowance()).unwrap_err(), Error::Incomplete);
    assert_eq!(session.advance(0, 3, allowance()).unwrap_err(), Error::Stale);
    assert_eq!(session.advance(1, 4, allowance()).unwrap_err(), Error::InvalidInput);
    assert_eq!(session.position(), 1); assert_eq!(session.reconstruction_products(), 0);
    assert_eq!(source.compare_learned_forced(1, 201, &codec, &[3, 4], paired_budget()).unwrap_err(), Error::InvalidInput);
    assert_eq!(source.compare_learned_greedy(1, 201, &codec, 3, 64, paired_budget()).unwrap_err(), Error::Limit);
    assert_eq!(source.compare_learned_greedy(1, 201, &codec, 3, 0, paired_budget()).unwrap_err(), Error::InvalidInput);
    session.advance(1, 3, allowance()).unwrap();
    assert_eq!(session.position(), 2);
}
