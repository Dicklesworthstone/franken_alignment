//! Separable synthetic residuals, not evidence about a pretrained detector.
use fa_reference::action::consequence::activation::probe::training::{
    CaseLabel, CaseOrigin, DataSplit, FitPolicy,
};
use fa_reference::action::consequence::activation::probe::training::calibration::{CalibrationPolicy, ScreeningCriteria};
use fa_reference::action::consequence::activation::probe::training::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape,
};
use std::collections::BTreeMap;

pub fn model() -> DecoderModel { model_with_output(false) }
pub fn model_with_output(overflow: bool) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape {
        vocabulary: 2, hidden: 2, intermediate: 2, layers: 2, query_heads: 1, cache_heads: 1, context: 8,
    }, 0.00001, 10000.0).unwrap();
    let layers = (0..2).map(|_| DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }).collect();
    DecoderModel::new(profile, vec![-1.0, 0.0, 1.0, 0.0], layers, vec![1.0; 2],
        if overflow { vec![f32::MAX; 4] } else { vec![-1.0, 0.0, 1.0, 0.0] }).unwrap()
}
pub fn cases() -> Vec<LabelledPrefix> {
    (1_u64..=6).map(|id| LabelledPrefix {
        origin: CaseOrigin { task: id, lineage: id + 100 },
        split: match id { 1 | 2 => DataSplit::Training, 3 | 4 => DataSplit::Calibration, _ => DataSplit::Evaluation },
        label: if id % 2 == 0 { CaseLabel::Violation } else { CaseLabel::Benign },
        tokens: vec![u32::from(id % 2 == 0)],
    }).collect()
}
pub fn policies() -> BTreeMap<u64, LayerPolicy> {
    let criteria = ScreeningCriteria::new(1, 0).unwrap();
    (1..=2).map(|id| (id, LayerPolicy {
        fit: FitPolicy::new(id, 1, 64, 0.25, 0.01, 0.001).unwrap(),
        calibration: CalibrationPolicy::new(id, 1, &[-10.0, 0.0, 10.0], criteria, criteria).unwrap(),
    })).collect()
}
pub fn capture(model: &DecoderModel, cases: &[LabelledPrefix]) -> DecoderCorpus {
    let mut budget = CaptureBudget::new(DecoderCorpus::estimate(model, cases).unwrap()).unwrap();
    DecoderCorpus::capture(model, 1, 1, cases, &mut budget).unwrap()
}
pub fn campaign(corpus: &DecoderCorpus) -> DecoderCampaign {
    let plans = policies();
    let mut budget = CampaignBudget::new(corpus.estimate_campaign(&plans).unwrap()).unwrap();
    corpus.run(plans, &mut budget).unwrap()
}
