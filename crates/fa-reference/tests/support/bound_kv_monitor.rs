//! Complete byte vocabulary with the existing real, separable GQA K/V signal.
#[path = "decoder_kv_probe_campaign.rs"]
#[allow(dead_code)]
pub mod trained;
pub use trained::{cases, codec, codec_from, inference, policies, settings};
use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::{ByteBpe, Merge, TokenBytes},
    probe::training::decoder::kv::KvDecoderCampaign,
    tensor::kv::decoder::{DecoderIdentity, DecoderLayerWeights, DecoderModel,
        DecoderProfile, DecoderShape, sampling::{SamplingPolicy, SamplingStart,
            monitored::GenerationSpec}},
};
use std::collections::{BTreeMap, BTreeSet};

pub fn model() -> DecoderModel { configured(trained::identity(), true, None, false) }
pub fn changed_unused_weight() -> DecoderModel {
    configured(trained::identity(), true, Some(0.25), false)
}
pub fn changed_signed_zero() -> DecoderModel {
    configured(trained::identity(), true, Some(-0.0), false)
}
pub fn model_with_identity(identity: DecoderIdentity) -> DecoderModel {
    configured(identity, true, None, false)
}
pub fn failed_model() -> DecoderModel { configured(trained::identity(), false, None, false) }
pub fn scaled_model() -> DecoderModel { configured(trained::identity(), true, None, true) }
fn configured(identity: DecoderIdentity, value_signal: bool, unused: Option<f32>, scaled: bool)
    -> DecoderModel
{
    let mut profile = DecoderProfile::new(identity, DecoderShape {
        vocabulary: 260, hidden: 4, intermediate: 4, layers: 2,
        query_heads: 2, cache_heads: 1, context: 8,
    }, 0.00001, 10000.0).unwrap();
    if scaled {
        use fa_reference::action::consequence::activation::tensor::kv::decoder::RotaryScaling;
        profile = profile.with_rotary_scaling(RotaryScaling::linear(2.0).unwrap()).unwrap();
    }
    let projection = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let layers = (0..2).map(|index| DecoderLayerWeights {
        attention_norm: vec![1.0; 4], queries: vec![0.0; 16], keys: projection.clone(),
        values: if index == 1 && !value_signal { vec![0.0; 8] } else { projection.clone() },
        attention_output: vec![0.0; 16], feed_forward_norm: vec![1.0; 4],
        gate: vec![0.0; 16], up: vec![0.0; 16], down: vec![0.0; 16],
    }).collect();
    let mut embeddings = [-1.0, 0.0, 0.0, 0.0].repeat(260);
    embeddings[4] = 1.0;
    embeddings[4 * 4 + 2] = 1.0;
    embeddings[5 * 4 + 2] = -1.0;
    if let Some(value) = unused { embeddings[250 * 4 + 1] = value; }
    let mut output = vec![0.0; 260 * 4];
    output[2] = 1.0;
    output[4 + 2] = -1.0;
    output[256 * 4] = -0.5;
    DecoderModel::new(profile, embeddings, layers, vec![1.0; 4], output).unwrap()
}
pub fn campaign() -> KvDecoderCampaign { trained::campaign(&trained::capture(&model(), &cases())) }
pub fn tokenizer(model: &DecoderModel) -> ByteBpe { changed_tokenizer(model, 0) }
/// 1 changes unused byte IDs, 2 changes merge ranks, 3 changes a named control.
pub fn changed_tokenizer(model: &DecoderModel, variant: usize) -> ByteBpe {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.extend([TokenBytes::Control, TokenBytes::Content(b"ab".to_vec()),
        TokenBytes::Content(b"bc".to_vec()), TokenBytes::Control]);
    if variant == 1 { vocabulary.swap(0, 1); }
    let mut merges = vec![Merge { left: 97, right: 98, result: 257 },
        Merge { left: 98, right: 99, result: 258 }];
    if variant == 2 { merges.reverse(); }
    let stop = if variant == 3 { b"<|halt|>".to_vec() } else { b"<|stop|>".to_vec() };
    ByteBpe::new_with_special_tokens(model.profile().clone(), vocabulary, merges,
        BTreeMap::from([(256, stop), (259, b"<|reserved|>".to_vec())])).unwrap()
}
pub fn spec(seed_token: u32) -> GenerationSpec {
    GenerationSpec::new(vec![seed_token], 2, BTreeSet::from([256]), SamplingStart {
        policy: SamplingPolicy::new(7, 2, 260, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
    }).unwrap()
}
