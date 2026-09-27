//! Tiny actual decoder with history-sensitive attention and a deterministic token
//! chain. These weights and zero probes are controls, not detector qualification.
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, Merge, TokenBytes, TokenizationBudget,
};
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS, monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::{SamplingPolicy, SamplingStart,
            monitored::{GenerationBudget, GenerationTelemetryBudget}}},
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use fa_reference::action::consequence::oversight::learned_source::text::{
    LearnedTextConfig, LearnedTextOutputPolicy, LearnedTextCompletion,
};
use std::collections::{BTreeMap, BTreeSet};

pub const END: u32 = 256;
pub const OTHER_CONTROL: u32 = 257;
pub const MERGED_PROMPT: u32 = 258;
pub const PROMPT: u32 = b'P' as u32;
pub fn model(output: &[u32]) -> DecoderModel {
    assert!(!output.is_empty() && output.len() <= 3);
    assert_eq!(output.iter().copied().collect::<BTreeSet<_>>().len(), output.len());
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 259, hidden: 4, intermediate: 4, layers: 1,
            query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let diagonal = |scale| {
        let mut matrix = vec![0.0; 16]; for i in 0..4 { matrix[i * 4 + i] = scale; } matrix
    };
    let mut embedding = vec![0.0; 259 * 4];
    embedding[PROMPT as usize * 4] = 1.0;
    embedding[MERGED_PROMPT as usize * 4] = 1.0;
    let mut head = vec![0.0; 259 * 4];
    for (index, token) in output.iter().enumerate() {
        head[*token as usize * 4 + index] = 8.0;
        embedding[*token as usize * 4 + index + 1] = 1.0;
    }
    DecoderModel::new(profile, embedding, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 4], queries: diagonal(0.1), keys: diagonal(0.1),
        values: diagonal(1.0), attention_output: diagonal(0.01),
        feed_forward_norm: vec![1.0; 4], gate: vec![0.0; 16], up: vec![0.0; 16], down: vec![0.0; 16],
    }], vec![1.0; 4], head).unwrap()
}
pub fn tokenizer(model: &DecoderModel) -> ByteBpe {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.extend([TokenBytes::Control, TokenBytes::Control, TokenBytes::Content(b"PP".to_vec())]);
    ByteBpe::new(model.profile().clone(), vocabulary,
        vec![Merge { left: PROMPT, right: PROMPT, result: MERGED_PROMPT }]).unwrap()
}
pub fn policy(model: &DecoderModel, alarm: bool) -> LearnedDecoderPolicy {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[PROMPT, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut weights = vec![0.0; tensor.dimensions()];
            if alarm && side == KvSide::Value { weights[1] = 1.0; }
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, 0.5).unwrap();
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    LearnedDecoderPolicy::new(codec,
        LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap(),
        LearnedStreamRetention::All, LearnedAuditPreparationBudget::default(), inference).unwrap()
}
pub fn config(model: &DecoderModel) -> LearnedTextConfig {
    LearnedTextConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        prompt: "PP".to_owned(), tokenization: TokenizationBudget::default(),
        max_new_tokens: 3, stop_tokens: BTreeSet::from([END]),
        sampling: SamplingStart { policy: SamplingPolicy::new(1, 1, 259, 1.0, 1, 1.0).unwrap(),
            stream: 71, seed: 173 }, policy: policy(model, false),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default(),
        output: LearnedTextOutputPolicy { max_bytes: 64, completion: LearnedTextCompletion::StopRequired } }
}
