//! Execute the original decoder and fit the original codec on disjoint streams.
//! These constructed weights are causal controls, not trained-model evidence.
use super::*;
use crate::action::consequence::activation::{SourceFrame,
    tensor::kv::{decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights,
        DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS},
        experiment::KvSide, model::{ModelKvImage, learned::{CompressionBudget,
            FitBudget, LearnedKvCodec, LearnedKvPolicy}}}};
use crate::action::consequence::activation::probe::learned::{CheckedKvBudget, ResidualRetention};
use std::collections::BTreeMap;

pub(super) fn row(side: KvSide) -> KvRow { KvRow { layer: 1, side, position: 0 } }

pub(super) fn source(value: [f32; 2], retention: ResidualRetention)
    -> (CheckedLearnedKv, ModelKvImage)
{
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 4, hidden: 2, intermediate: 2, layers: 1,
            query_heads: 1, cache_heads: 1, context: 8 }, 0.00001, 10000.0).unwrap();
    let model = DecoderModel::new(profile,
        vec![1.0, 0.0, -1.0, 0.0, value[0], value[1], 0.0, 1.0],
        vec![DecoderLayerWeights {
            attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
            keys: vec![1.0, 0.0, 0.0, 1.0], values: vec![1.0, 0.0, 0.0, 1.0],
            attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
            gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
        }], vec![1.0; 2], vec![0.0; 8]).unwrap();
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let positive = model.recompute(11, &[0], inference).unwrap().cache_image().unwrap();
    let negative = model.recompute(12, &[1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, positive), (102, negative)]), FitBudget::default()).unwrap();
    let original = model.recompute(21, &[2], inference).unwrap().cache_image().unwrap();
    let (image, report) = codec.evaluate_held_out(202, &original, CompressionBudget::default()).unwrap();
    assert!(!report.training_source_overlap);
    let checked = CheckedLearnedKv::new(image, &original, retention, CheckedKvBudget::default()).unwrap();
    (checked, original)
}

pub(super) fn raw(source: &ModelKvImage, row: KvRow) -> &SourceFrame {
    let token = source.layer(row.layer).unwrap().token(row.position).unwrap();
    match row.side { KvSide::Key => token.key(), KvSide::Value => token.value() }
}

pub(super) fn model(source: &CheckedLearnedKv, row: KvRow, weights: &[f32]) -> ForecastModel {
    let profile = source.row_shape(row).unwrap().0.profile;
    ForecastModel::new(crate::action::consequence::activation::probe::LinearProbe::new(
        91, 1, profile, weights, 0.0, 0.0).unwrap(), super::super::super::ForecastRegistration {
        domain: 71, generation: 2, policy_generation: 1, event_prefix: b"publish".to_vec(),
        negative: BinaryForecast::new(49_152, 16_384).unwrap(),
        at_threshold: BinaryForecast::new(32_768, 32_768).unwrap(),
        positive: BinaryForecast::new(16_384, 49_152).unwrap(),
    }).unwrap()
}
