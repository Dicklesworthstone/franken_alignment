use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::*;
use fa_reference::action::consequence::activation::tensor::kv::experiment::{KvCell, KvEdit, KvEditScope, KvSide};
use fa_reference::Error;
use std::collections::BTreeMap;
#[path = "support/decoder_fixture.rs"] mod fixture;
#[path = "support/decoder_observation.rs"] mod observation;
use observation::words as source_words;

fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn scope(count: usize) -> KvEditScope {
    KvEditScope { first_position: 0, token_count: count, keys: true, values: true }
}
fn cell(side: KvSide, position: u64, channel: usize) -> KvCell { KvCell { side, position, head: 0, channel } }
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn edit(source: &DecoderCheckpoint, layer: u64, cell: KvCell, replacement: f32) -> KvEdit {
    let token = source.cache().layer(layer).unwrap().token(cell.position).unwrap();
    let frame = match cell.side { KvSide::Key => token.key(), KvSide::Value => token.value() };
    KvEdit { cell, expected_bits: source_words(frame)[cell.head * source.model().profile().head_width() + cell.channel],
        replacement_bits: replacement.to_bits() }
}
fn changes(source: &DecoderCheckpoint, layer: u64, change: KvEdit) -> BTreeMap<u64, DecoderLayerIntervention> {
    BTreeMap::from([(layer, DecoderLayerIntervention { scope: scope(source.tokens().len()), edits: vec![change] })])
}
fn compare_cache(experiment: &DecoderExperimentSession, original: &DecoderSession) {
    let image = original.cache_image().unwrap();
    let s = original.model().profile().shape();
    for layer in 1..=s.layers as u64 {
        for position in 0..original.position() {
            let token = image.layer(layer).unwrap().token(position).unwrap();
            let keys = source_words(token.key()); let values = source_words(token.value());
            for head in 0..s.cache_heads {
                for channel in 0..original.model().profile().head_width() {
                    let index = head * original.model().profile().head_width() + channel;
                    for (side, expected) in [(KvSide::Key, keys[index]), (KvSide::Value, values[index])] {
                        assert_eq!(experiment.bits(layer, KvCell { side, position, head, channel }).unwrap(), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn unchanged_control_and_treatment_match_original_all_layer_execution_for_32_steps() {
    let model = fixture::model(fixture::profile(40));
    let mut original = model.recompute(7, &[0, 3, 1, 5], budget()).unwrap();
    let checkpoint = original.checkpoint().unwrap();
    let snapshot = checkpoint.cache().encode().unwrap();
    let no_op = edit(&checkpoint, 2, cell(KvSide::Value, 2, 1),
        f32::from_bits(source_words(checkpoint.cache().layer(2).unwrap().token(2).unwrap().value())[1]));
    let plan = checkpoint.intervene(11, changes(&checkpoint, 2, no_op), 1).unwrap();
    assert_eq!(plan.proposed_edits(), 1); assert_eq!(plan.effective_edits(), 0);
    let mut control = plan.session(DecoderExperimentArm::Control);
    let mut treatment = plan.session(DecoderExperimentArm::Intervention);
    for _ in 0..32 {
        let position = original.position();
        let token = original.greedy_token().unwrap();
        let expected = original.advance(position, token, budget()).unwrap();
        let a = control.advance(position, token, budget()).unwrap();
        let b = treatment.advance(position, token, budget()).unwrap();
        assert_eq!(bits(&expected.logits), bits(&a.logits));
        assert_eq!(bits(&a.logits), bits(&b.logits));
        assert_eq!(a.work, expected.work); assert_eq!(a.work, b.work);
        assert_eq!(control.greedy_token().unwrap(), original.greedy_token().unwrap());
        assert_eq!(treatment.greedy_token().unwrap(), original.greedy_token().unwrap());
        compare_cache(&control, &original); compare_cache(&treatment, &original);
    }
    assert_eq!(control.work().tokens, 32);
    assert_eq!(checkpoint.cache().encode().unwrap(), snapshot);
}

#[test]
fn old_logits_are_not_exposed_as_outputs_of_an_edited_prefix() {
    let model = fixture::model(fixture::profile(8));
    let source = model.recompute(1, &[0], budget()).unwrap().checkpoint().unwrap();
    assert!(source.logits().is_some());
    let plan = source.intervene(2, BTreeMap::new(), 0).unwrap();
    for arm in [DecoderExperimentArm::Control, DecoderExperimentArm::Intervention] {
        let mut session = plan.session(arm);
        assert_eq!(session.logits(), Err(Error::Incomplete));
        assert!(matches!(session.advance_greedy(1, budget()), Err(Error::Incomplete)));
        assert_eq!(session.position(), 1);
        session.advance(1, 3, budget()).unwrap();
        session.advance_greedy(2, budget()).unwrap();
        assert_eq!(session.continuation_tokens().len(), 2);
    }
}

fn uniform_attention_model() -> DecoderModel { uniform_attention_model_with_scale(1.0) }
fn uniform_attention_model_with_scale(scale: f32) -> DecoderModel {
    let p = DecoderProfile::new(fixture::profile(8).identity(), DecoderShape {
        vocabulary: 2, hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 8,
    }, 1e-5, 10000.0).unwrap();
    let identity = vec![1.0, 0.0, 0.0, 1.0];
    let mut layers = fixture::zero_layers(&p);
    layers[0].values = identity.clone(); layers[0].attention_output = identity.iter().map(|value| value * scale).collect();
    DecoderModel::new(p, identity.clone(), layers, vec![1.0; 2], identity).unwrap()
}

#[test]
fn value_intervention_matches_analytic_uniform_attention_and_changes_actual_logits() {
    let model = uniform_attention_model();
    let original = model.recompute(9, &[0], budget()).unwrap();
    let source = original.checkpoint().unwrap();
    let cached = f32::from_bits(source_words(source.cache().layer(1).unwrap().token(0).unwrap().value())[0]);
    let change = edit(&source, 1, cell(KvSide::Value, 0, 0), -cached);
    let plan = source.intervene(4, changes(&source, 1, change), 1).unwrap();
    let mut control = plan.session(DecoderExperimentArm::Control);
    let mut treatment = plan.session(DecoderExperimentArm::Intervention);
    let a = control.advance(1, 1, budget()).unwrap();
    let b = treatment.advance(1, 1, budget()).unwrap();
    // Q and K are zero: each of the two cached values has weight exactly 1/2.
    // The unchanged second coordinate and changed first coordinate then pass
    // through the same residual, zero FFN, final RMSNorm and identity output.
    let x = f64::from(cached) / 2.0;
    let y = f64::from((1.0 + x) as f32);
    let denominator = ((x*x + y*y) / 2.0 + 1e-5).sqrt();
    assert_eq!(a.logits[0].to_bits(), ((x / denominator) as f32).to_bits());
    assert_eq!(b.logits[0].to_bits(), ((-x / denominator) as f32).to_bits());
    assert_eq!(a.logits[1].to_bits(), ((y / denominator) as f32).to_bits());
    assert_eq!(a.logits[1].to_bits(), b.logits[1].to_bits());
    assert!(a.logits[0] > 0.0 && b.logits[0] < 0.0);
    assert_eq!(control.bits(1, change.cell).unwrap(), change.expected_bits);
    assert_eq!(treatment.bits(1, change.cell).unwrap(), change.replacement_bits);
    assert_eq!(source_words(source.cache().layer(1).unwrap().token(0).unwrap().value())[0], change.expected_bits);
    // Later steps read each arm's own newly computed history, not just old edits.
    for position in 2..6 {
        control.advance_greedy(position, budget()).unwrap();
        treatment.advance_greedy(position, budget()).unwrap();
    }
    assert_eq!(original.position(), 1);
}

#[test]
fn all_layer_admission_refuses_stale_duplicate_nonfinite_and_out_of_scope_edits() {
    let model = fixture::model(fixture::profile(8));
    let source = model.recompute(1, &[0, 2], budget()).unwrap().checkpoint().unwrap();
    let original = source.cache().encode().unwrap();
    let first = edit(&source, 1, cell(KvSide::Key, 0, 0), 7.0);
    let second = edit(&source, 2, cell(KvSide::Value, 1, 1), 8.0);
    let mut set = changes(&source, 1, first);
    set.extend(changes(&source, 2, second));
    let plan = source.intervene(2, set.clone(), 2).unwrap();
    let session = plan.session(DecoderExperimentArm::Intervention);
    assert_eq!(session.bits(1, first.cell).unwrap(), first.replacement_bits);
    assert_eq!(session.bits(2, second.cell).unwrap(), second.replacement_bits);
    assert!(matches!(source.intervene(2, set.clone(), 1), Err(Error::Limit)));
    for invalid in [
        KvEdit { expected_bits: second.expected_bits ^ 1, ..second },
        KvEdit { replacement_bits: f32::INFINITY.to_bits(), ..second },
        KvEdit { cell: cell(KvSide::Value, 2, 1), ..second },
        KvEdit { cell: cell(KvSide::Value, 1, 2), ..second },
    ] {
        let mut bad = set.clone(); bad.get_mut(&2).unwrap().edits = vec![invalid];
        assert!(source.intervene(3, bad, 2).is_err());
    }
    let mut duplicate = set.clone(); duplicate.get_mut(&2).unwrap().edits.push(second);
    assert!(matches!(source.intervene(3, duplicate, 3), Err(Error::Duplicate)));
    let mut restricted = set; restricted.get_mut(&2).unwrap().scope.values = false;
    assert!(matches!(source.intervene(3, restricted, 2), Err(Error::Binding)));
    assert_eq!(source.cache().encode().unwrap(), original);
}

#[test]
fn refused_step_does_not_publish_partial_state_and_can_be_retried() {
    let model = fixture::model(fixture::profile(4));
    let source = model.recompute(1, &[0, 2], budget()).unwrap().checkpoint().unwrap();
    let mut session = source.intervene(2, BTreeMap::new(), 0).unwrap().session(DecoderExperimentArm::Intervention);
    let work = model.estimate(2, 1).unwrap().scalar_products().unwrap();
    assert!(matches!(session.advance(2, 1, DecoderBudget { scalar_products: work - 1 }), Err(Error::Limit)));
    assert!(matches!(session.advance(1, 1, budget()), Err(Error::Stale)));
    assert!(matches!(session.advance(2, 6, budget()), Err(Error::InvalidInput)));
    assert!(session.continuation_tokens().is_empty()); assert_eq!(session.work(), DecoderWork::default());
    session.advance(2, 1, DecoderBudget { scalar_products: work }).unwrap();
    session.advance_greedy(3, budget()).unwrap();
    let before = bits(session.logits().unwrap());
    assert!(matches!(session.advance_greedy(4, budget()), Err(Error::Limit)));
    assert_eq!(session.position(), 4); assert_eq!(bits(session.logits().unwrap()), before);
}

#[test]
fn experiments_pin_original_weights_and_survive_source_owner_drop() {
    let model = fixture::model(fixture::profile(8));
    let source = model.recompute(1, &[0, 2], budget()).unwrap().checkpoint().unwrap();
    let plan = source.intervene(2, BTreeMap::new(), 0).unwrap();
    let mut expected = model.recompute(3, &[0, 2], budget()).unwrap();
    let mut a = plan.session(DecoderExperimentArm::Intervention);
    let mut b = plan.session(DecoderExperimentArm::Intervention);
    drop(plan); drop(source); drop(model);
    let ea = a.advance(2, 1, budget()).unwrap();
    let eb = b.advance(2, 1, budget()).unwrap();
    let original = expected.advance(2, 1, budget()).unwrap();
    assert_eq!(bits(&ea.logits), bits(&eb.logits)); assert_eq!(bits(&ea.logits), bits(&original.logits));
    a.advance(3, 4, budget()).unwrap();
    assert_eq!(b.position(), 3); assert_eq!(bits(b.logits().unwrap()), bits(&eb.logits));
}

#[test]
fn empty_prefix_requires_explicit_input_and_rejects_nonexistent_edit_scope() {
    let model = fixture::model(fixture::profile(4));
    let mut original = model.session(1).unwrap();
    let source = original.checkpoint().unwrap();
    let plan = source.intervene(2, BTreeMap::new(), 0).unwrap();
    let mut session = plan.session(DecoderExperimentArm::Control);
    let expected = original.advance(0, 1, budget()).unwrap();
    let actual = session.advance(0, 1, budget()).unwrap();
    assert_eq!(bits(&expected.logits), bits(&actual.logits)); compare_cache(&session, &original);
    let layers = BTreeMap::from([(1, DecoderLayerIntervention { scope: scope(1), edits: vec![] })]);
    assert!(source.intervene(3, layers, 0).is_err());
    assert!(matches!(source.intervene(0, BTreeMap::new(), 0), Err(Error::InvalidInput)));
    assert!(matches!(source.intervene(3, BTreeMap::new(), MAX_DECODER_INTERVENTION_EDITS + 1), Err(Error::Limit)));
}

#[test]
fn numerical_overflow_after_admitted_edit_is_atomic_and_never_changes_control() {
    let model = uniform_attention_model_with_scale(4.0);
    let source = model.recompute(1, &[0], budget()).unwrap().checkpoint().unwrap();
    let change = edit(&source, 1, cell(KvSide::Value, 0, 0), f32::MAX);
    let plan = source.intervene(2, changes(&source, 1, change), 1).unwrap();
    let mut treatment = plan.session(DecoderExperimentArm::Intervention);
    for _ in 0..2 {
        assert!(matches!(treatment.advance(1, 1, budget()), Err(Error::Overflow)));
        assert_eq!(treatment.position(), 1);
        assert!(treatment.continuation_tokens().is_empty());
        assert_eq!(treatment.logits(), Err(Error::Incomplete));
        assert_eq!(treatment.work(), DecoderWork::default());
        assert_eq!(treatment.bits(1, change.cell).unwrap(), change.replacement_bits);
    }
    let mut control = plan.session(DecoderExperimentArm::Control);
    control.advance(1, 1, budget()).unwrap();
    assert_eq!(control.bits(1, change.cell).unwrap(), change.expected_bits);
}
