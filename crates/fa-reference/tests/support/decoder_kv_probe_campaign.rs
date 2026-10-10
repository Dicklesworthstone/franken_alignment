//! Separable real decoder K/V with GQA, not a pretrained detector quality claim.
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget},
};
use fa_reference::action::consequence::activation::probe::training::{
    CaseLabel, CaseOrigin, DataSplit, FitPolicy,
    calibration::{CalibrationPolicy, ScreeningCriteria},
    decoder::{LabelledPrefix, LayerPolicy, kv::{KvCaptureBudget, KvCampaignBudget,
        KvDecoderCorpus, KvDecoderCampaign, KvMonitorSettings}},
};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel,
        DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
        monitoring::LearnedStreamRetention,
        sampling::{SamplingPolicy, SamplingStart, monitored::GenerationSpec}},
    experiment::KvSide, model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use std::collections::{BTreeMap, BTreeSet};

pub fn identity() -> DecoderIdentity {
    DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }
}
pub fn model() -> DecoderModel { configured(true, false, identity()) }
pub fn model_without_value_signal() -> DecoderModel { configured(false, false, identity()) }
pub fn model_with_output_overflow() -> DecoderModel { configured(true, true, identity()) }
pub fn model_with_identity(identity: DecoderIdentity) -> DecoderModel { configured(true, false, identity) }
fn configured(value_signal: bool, output_overflow: bool, identity: DecoderIdentity) -> DecoderModel {
    let profile = DecoderProfile::new(identity, DecoderShape {
        vocabulary: 10, hidden: 4, intermediate: 4, layers: 2,
        query_heads: 2, cache_heads: 1, context: 8,
    }, 0.00001, 10000.0).unwrap();
    let projection = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let layers = (0..2).map(|index| DecoderLayerWeights {
        attention_norm: vec![1.0; 4], queries: vec![0.0; 16], keys: projection.clone(),
        values: if index == 1 && !value_signal { vec![0.0; 8] } else { projection.clone() },
        attention_output: vec![0.0; 16], feed_forward_norm: vec![1.0; 4],
        gate: vec![0.0; 16], up: vec![0.0; 16], down: vec![0.0; 16],
    }).collect();
    let mut embeddings = [-1.0, 0.0, 0.0, 0.0].repeat(10);
    embeddings[4] = 1.0;       // Token 1 is the positive K/V class.
    embeddings[4 * 4 + 2] = 1.0;  // Seed 4 samples benign token 0.
    embeddings[5 * 4 + 2] = -1.0; // Seed 5 samples violation token 1.
    let mut output = vec![0.0; 40];
    output[2] = 1.0;
    output[4 + 2] = -1.0;
    output[2 * 4] = -0.5; // After token 0, the original sampler selects stop 2.
    if output_overflow { output[0] = f32::MAX; }
    DecoderModel::new(profile, embeddings, layers, vec![1.0; 4], output).unwrap()
}

pub fn cases() -> Vec<LabelledPrefix> {
    (1_u64..=6).map(|id| LabelledPrefix {
        origin: CaseOrigin { task: id, lineage: id + 100 },
        split: match id { 1 | 2 => DataSplit::Training, 3 | 4 => DataSplit::Calibration,
            _ => DataSplit::Evaluation },
        label: if id % 2 == 0 { CaseLabel::Violation } else { CaseLabel::Benign },
        // Prefix IDs make exact histories disjoint across the three splits.
        tokens: vec![id as u32 + 3, u32::from(id % 2 == 0)],
    }).collect()
}
pub fn policies() -> BTreeMap<KvTap, LayerPolicy> {
    let criteria = ScreeningCriteria::new(1, 0).unwrap();
    (1..=2).flat_map(|layer| [KvSide::Key, KvSide::Value].map(move |side| (layer, side)))
        .map(|(layer, side)| {
            let id = layer * 2 + u64::from(side == KvSide::Value);
            (KvTap { layer, side }, LayerPolicy {
                fit: FitPolicy::new(id, 1, 64, 0.25, 0.01, 0.001).unwrap(),
                calibration: CalibrationPolicy::new(id, 1, &[-10.0, 0.0, 10.0], criteria, criteria).unwrap(),
            })
        }).collect()
}
pub fn capture(model: &DecoderModel, cases: &[LabelledPrefix]) -> KvDecoderCorpus {
    let mut budget = KvCaptureBudget::new(KvDecoderCorpus::estimate(model, cases).unwrap()).unwrap();
    KvDecoderCorpus::capture(model, 1, 1, cases, &mut budget).unwrap()
}
pub fn campaign(corpus: &KvDecoderCorpus) -> KvDecoderCampaign {
    let policies = policies();
    let mut budget = KvCampaignBudget::new(corpus.estimate_campaign(&policies).unwrap()).unwrap();
    corpus.run(policies, &mut budget).unwrap()
}
pub fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
pub fn codec(model: &DecoderModel) -> LearnedKvCodec { codec_from(model, 1000, 1010) }
pub fn codec_from(model: &DecoderModel, stream: u64, lineage: u64) -> LearnedKvCodec {
    let source = model.recompute(stream, &[0, 1], inference()).unwrap().cache_image().unwrap();
    LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(lineage, source)]), FitBudget::default()).unwrap()
}
pub fn settings(model: &DecoderModel) -> KvMonitorSettings {
    let taps = model.cache_profile().layers().keys().flat_map(|layer|
        [KvSide::Key, KvSide::Value].map(|side| (KvTap { layer: *layer, side }, LearnedMonitorBudget::default())))
        .collect();
    KvMonitorSettings { taps, audit: LearnedAuditBudget::default(),
        preparation: LearnedAuditPreparationBudget::default(), inference: inference(),
        retention: LearnedStreamRetention::All }
}
pub fn spec(seed_token: u32) -> GenerationSpec {
    GenerationSpec::new(vec![seed_token], 2, BTreeSet::from([2]), SamplingStart {
        policy: SamplingPolicy::new(7, 2, 10, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
    }).unwrap()
}
