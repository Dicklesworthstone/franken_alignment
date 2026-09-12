//! Compression is tested through real numerical continuation, not MSE alone.
#[path = "support/decoder_fixture.rs"] mod fixture;
#[path = "support/decoder_observation.rs"] mod observation;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::DecoderComparisonBudget;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::quantized::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::quantized::comparison::*;
use fa_reference::action::consequence::activation::tensor::kv::experiment::{KvCell, KvEdit, KvEditScope, KvSide};
use fa_reference::action::consequence::activation::tensor::kv::model::quantized::*;
use fa_reference::Error;
use std::collections::BTreeMap;

fn full() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn codec() -> KvQuantization { KvQuantization::new(20, 1).unwrap() }
fn budget(source: &DecoderCheckpoint) -> QuantizationBudget {
    QuantizationBudget { values: source.cache().normalized_values(), encoded_bytes: source.cache().quantized_len().unwrap() }
}
fn paired(source: &DecoderCheckpoint, count: usize) -> QuantizedComparisonBudget {
    QuantizedComparisonBudget { quantization: budget(source), scalar_products: 2 * source.model()
        .estimate(source.tokens().len(), count).unwrap().scalar_products().unwrap(),
        retained_logit_values: 2 * count * source.model().profile().shape().vocabulary }
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|x| x.to_bits()).collect() }
fn equivalent_edits(source: &DecoderCheckpoint, q: &QuantizedDecoder) -> DecoderIntervention {
    let mut specifications = BTreeMap::new(); let mut count = 0;
    for layer in 1..=source.model().profile().shape().layers as u64 {
        let mut edits = Vec::new();
        for position in 0..source.tokens().len() as u64 {
            let row = source.cache().layer(layer).unwrap().token(position).unwrap();
            for side in [KvSide::Key, KvSide::Value] {
                let frame = if side == KvSide::Key { row.key() } else { row.value() };
                let words = observation::words(frame);
                for (index, expected_bits) in words.into_iter().enumerate() {
                    let width = source.model().profile().head_width();
                    let cell = KvCell { side, position, head: index / width, channel: index % width };
                    edits.push(KvEdit { cell, expected_bits, replacement_bits: q.image().bits(layer, cell).unwrap() });
                }
            }
        }
        count += edits.len();
        specifications.insert(layer, DecoderLayerIntervention {
            scope: KvEditScope { first_position: 0, token_count: source.tokens().len(), keys: true, values: true }, edits,
        });
    }
    source.intervene(30, specifications, count).unwrap()
}

#[test]
fn quantized_prefix_matches_independent_explicit_scalar_interventions_for_32_steps() {
    let model = fixture::model(fixture::profile(48));
    let source = model.recompute(1, &[0, 3, 1, 5], full()).unwrap().checkpoint().unwrap();
    let before = source.cache().encode().unwrap();
    let q = source.quantized_experiment(10, codec(), budget(&source)).unwrap();
    assert!(q.report().layers.values().any(|e| e.keys.changed_words + e.values.changed_words > 0));
    let edited = equivalent_edits(&source, &q);
    let mut sparse = edited.session(DecoderExperimentArm::Intervention); let mut compact = q.session();
    assert_eq!(compact.greedy_token(), Err(Error::Incomplete));
    for offset in 0..32 {
        let token = if offset == 0 { 2 } else { sparse.greedy_token().unwrap() };
        if offset > 0 { assert_eq!(compact.greedy_token().unwrap(), token); }
        let position = compact.position();
        let left = sparse.advance(position, token, full()).unwrap();
        let right = compact.advance(position, token, full()).unwrap();
        assert_eq!(bits(&left.logits), bits(&right.logits)); assert_eq!(left.work, right.work);
        for layer in 1..=model.profile().shape().layers as u64 {
            for side in [KvSide::Key, KvSide::Value] {
                for channel in 0..model.profile().head_width() {
                    let cell = KvCell { side, position, head: 0, channel };
                    assert_eq!(sparse.bits(layer, cell).unwrap(), compact.bits(layer, cell).unwrap());
                }
            }
        }
    }
    assert_eq!(compact.work(), sparse.work()); assert_eq!(source.cache().encode().unwrap(), before);
}

#[test]
fn exact_zero_cache_control_recovers_uninterrupted_nontrivial_logits() {
    let p = fixture::profile(48); let s = p.shape();
    let model = DecoderModel::new(p.clone(), fixture::values(s.vocabulary * s.hidden, 11), fixture::zero_layers(&p),
        vec![1.0; s.hidden], fixture::values(s.vocabulary * s.hidden, 12)).unwrap();
    // Position zero keeps this exact-control prefix free of rotary signed-zero changes.
    let mut raw = model.recompute(1, &[0], full()).unwrap(); let source = raw.checkpoint().unwrap();
    let q = source.quantized_experiment(10, codec(), budget(&source)).unwrap(); let mut run = q.session();
    assert!(q.report().layers.values().all(|e| e.keys.changed_words + e.values.changed_words == 0));
    for i in 0..32 {
        let token = if i == 0 { 5 } else { raw.greedy_token().unwrap() };
        let expected = raw.advance(raw.position(), token, full()).unwrap();
        let actual = run.advance(run.position(), token, full()).unwrap();
        assert_eq!(bits(&actual.logits), bits(&expected.logits));
    }
}

fn sensitive_model(small: f32) -> DecoderModel {
    let p = DecoderProfile::new(fixture::profile(8).identity(), DecoderShape { vocabulary: 2,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 8 }, 0.00001, 10000.0).unwrap();
    let mut layers = fixture::zero_layers(&p);
    layers[0].values = vec![127.0, 0.0, small, 0.0];
    layers[0].attention_output = vec![0.0, 1.0, 0.0, 0.0];
    DecoderModel::new(p, vec![1.0, 0.0, 0.0, -1.0], layers, vec![1.0; 2], vec![0.0, 0.0, 1.0, 0.0]).unwrap()
}
#[test]
fn an_erased_small_cache_signal_changes_the_actual_next_token_choice() {
    for small in [0.25, 1.0] {
        let model = sensitive_model(small); let source = model.recompute(1, &[0], full()).unwrap().checkpoint().unwrap();
        let result = source.compare_quantized_forced(10, codec(), &[1], paired(&source, 1)).unwrap();
        let step = &result.steps()[0];
        assert_eq!(step.control_token, 1); assert_eq!(step.intervention_token, 1);
        assert_eq!(step.control_next_token, 1);
        if small == 0.25 {
            assert_eq!(result.quantized().report().layers[&1].values.nonzero_to_zero, 1);
            assert_eq!(step.intervention_next_token, 0); assert!(step.max_abs_logit_delta > 0.0);
            assert_eq!(result.first_different_logits(), Some(1));
            assert_eq!(result.first_different_next_choice(), Some(2));
            assert_eq!(result.first_different_consumed_token(), None);
        } else { assert_eq!(step.intervention_next_token, 1); assert_eq!(result.first_different_next_choice(), None); }
    }
}

#[test]
fn greedy_feedback_records_consumed_divergence_separately_from_predicted_divergence() {
    let model = sensitive_model(0.25); let source = model.recompute(1, &[0], full()).unwrap().checkpoint().unwrap();
    let result = source.compare_quantized_greedy(10, codec(), 1, 4, paired(&source, 4)).unwrap();
    assert_eq!(result.first_different_next_choice(), Some(2));
    assert_eq!(result.first_different_consumed_token(), Some(2));
    assert_eq!(result.work().tokens, 8);
    assert_eq!(result.work().scalar_products().unwrap(), paired(&source, 4).scalar_products);
    assert_eq!(result.steps()[1].control_token, 1); assert_eq!(result.steps()[1].intervention_token, 0);
}

#[test]
fn shared_comparison_budget_cannot_pay_for_only_one_arm_or_omit_retained_scores() {
    let model = fixture::model(fixture::profile(12));
    let source = model.recompute(1, &[1, 3], full()).unwrap().checkpoint().unwrap(); let before = source.cache().encode().unwrap();
    let b = paired(&source, 3);
    for short in [QuantizedComparisonBudget { scalar_products: b.scalar_products / 2, ..b },
        QuantizedComparisonBudget { retained_logit_values: b.retained_logit_values - 1, ..b },
        QuantizedComparisonBudget { quantization: QuantizationBudget { encoded_bytes: b.quantization.encoded_bytes - 1,
            ..b.quantization }, ..b }]
    { assert!(matches!(source.compare_quantized_forced(10, codec(), &[0, 1, 2], short), Err(Error::Limit))); }
    assert!(source.compare_quantized_forced(10, codec(), &[0, 1, 2], b).is_ok());
    assert!(matches!(source.compare_quantized_forced(10, codec(), &[0, 1, 99], b), Err(Error::InvalidInput)));
    assert_eq!(source.cache().encode().unwrap(), before);
}

#[test]
fn late_numerical_failure_retains_no_partial_suffix_and_retry_is_not_another_state() {
    let p = fixture::profile(8); let s = p.shape(); let mut embeddings = vec![0.0; s.vocabulary * s.hidden];
    embeddings[..4].copy_from_slice(&[1.0, -1.0, 0.0, 0.0]); embeddings[4..8].fill(1.0);
    let model = DecoderModel::new(p.clone(), embeddings, fixture::zero_layers(&p),
        vec![1.0; s.hidden], vec![f32::MAX; s.vocabulary * s.hidden]).unwrap();
    let source = model.recompute(1, &[0], full()).unwrap().checkpoint().unwrap();
    let q = source.quantized_experiment(10, codec(), budget(&source)).unwrap(); let mut run = q.session();
    for _ in 0..2 {
        assert!(matches!(run.advance(1, 1, full()), Err(Error::Overflow)));
        assert_eq!(run.position(), 1); assert!(run.continuation_tokens().is_empty());
        assert_eq!(run.work(), DecoderWork::default()); assert_eq!(run.logits(), Err(Error::Incomplete));
    }
    run.advance(1, 0, full()).unwrap(); assert_eq!(run.position(), 2);
    assert_eq!(source.tokens(), &[0]);
}

#[test]
fn compact_branches_outlive_source_owners_and_never_share_mutable_suffixes() {
    let q = { let model = fixture::model(fixture::profile(8));
        let source = model.recompute(1, &[2, 4], full()).unwrap().checkpoint().unwrap();
        source.quantized_experiment(10, codec(), budget(&source)).unwrap() };
    let saved = q.image().encode().unwrap(); let mut left = q.session(); let mut right = q.session(); drop(q);
    left.advance(2, 1, full()).unwrap(); assert_eq!(right.position(), 2); assert!(right.logits().is_err());
    right.advance(2, 5, full()).unwrap(); assert_eq!(left.continuation_tokens(), &[1]); assert_eq!(right.continuation_tokens(), &[5]);
    assert_eq!(left.plan().image().encode().unwrap(), saved);
    assert_eq!(right.plan().image().encode().unwrap(), saved);
    assert_ne!(bits(left.logits().unwrap()), bits(right.logits().unwrap()));
}

#[test]
fn empty_prefix_full_context_and_stale_positions_keep_exact_boundaries() {
    let model = fixture::model(fixture::profile(1)); let source = model.session(1).unwrap().checkpoint().unwrap();
    let q = source.quantized_experiment(10, codec(), budget(&source)).unwrap(); let mut run = q.session();
    assert_eq!(run.greedy_token(), Err(Error::Incomplete));
    assert!(matches!(run.advance(1, 0, full()), Err(Error::Stale)));
    assert!(matches!(run.advance(0, 0, DecoderBudget { scalar_products: 0 }), Err(Error::Limit)));
    assert_eq!(run.position(), 0); run.advance(0, 0, full()).unwrap();
    let before = bits(run.logits().unwrap()); assert!(matches!(run.advance_greedy(1, full()), Err(Error::Limit)));
    assert_eq!(bits(run.logits().unwrap()), before); assert_eq!(run.position(), 1);
}

#[test]
fn imported_weights_and_original_sparse_comparison_share_the_same_forward_engine() {
    let (model, _) = DecoderModel::from_safetensors(fixture::profile(12), include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap();
    let source = model.recompute(1, &[0, 2, 3], full()).unwrap().checkpoint().unwrap();
    let q = source.quantized_experiment(10, codec(), budget(&source)).unwrap(); let explicit = equivalent_edits(&source, &q);
    let expected = explicit.compare_forced(&[1, 4, 2], DecoderComparisonBudget {
        scalar_products: paired(&source, 3).scalar_products, retained_logit_values: 36,
    }).unwrap();
    let actual = source.compare_quantized_forced(10, codec(), &[1, 4, 2], paired(&source, 3)).unwrap();
    for (a, b) in actual.steps().iter().zip(expected.steps()) {
        assert_eq!(bits(&a.control_logits), bits(&b.control_logits));
        assert_eq!(bits(&a.intervention_logits), bits(&b.intervention_logits));
        assert_eq!(a.changed_logit_words, b.changed_logit_words);
    }
}

#[test]
fn comparison_failure_cannot_return_a_partial_report_or_mutate_the_baseline() {
    let p = fixture::profile(8); let s = p.shape(); let mut e = vec![0.0; s.vocabulary * s.hidden];
    e[..4].copy_from_slice(&[1.0, -1.0, 0.0, 0.0]); e[4..8].fill(1.0);
    let model = DecoderModel::new(p.clone(), e, fixture::zero_layers(&p), vec![1.0; s.hidden],
        vec![f32::MAX; s.vocabulary * s.hidden]).unwrap();
    let source = model.recompute(1, &[0], full()).unwrap().checkpoint().unwrap(); let saved = source.cache().encode().unwrap();
    assert!(matches!(source.compare_quantized_forced(10, codec(), &[0, 1], paired(&source, 2)), Err(Error::Overflow)));
    assert!(source.compare_quantized_forced(10, codec(), &[0, 0], paired(&source, 2)).is_ok());
    assert_eq!(source.cache().encode().unwrap(), saved);
}
