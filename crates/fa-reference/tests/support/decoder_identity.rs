#![allow(dead_code)]
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelManifest, ModelPassport};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile, DecoderShape,
};

pub fn model(changed: bool, overflow: bool) -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 1, tokenizer_generation: 1, profile_generation: 1 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 8 }, 1e-5, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
        keys: vec![0.0; 4], values: vec![0.0; 4], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    // Independent analytic oracle: zero residual branches leave the embedding.
    // Changing one actual parameter keeps every declared identity and shape fixed.
    let embeddings = if overflow { vec![1.0; 6] }
        else { vec![1.0, 0.0, 0.0, 1.0, if changed { -2.0 } else { -1.0 }, 0.0] };
    DecoderModel::new(profile, embeddings, vec![layer.clone(), layer], vec![1.0; 2],
        vec![if overflow { f32::MAX } else { 0.0 }; 6]).unwrap()
}
pub fn manifest() -> ModelManifest {
    ModelManifest { tenant: 1, model: 2, model_generation: 1, host_generation: 1,
        tokenizer_generation: 1, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] }
}
pub fn passport() -> ModelPassport {
    let model = model(false, false);
    ModelPassport::new(1, 1, manifest(), vec![
        IdentityAnchor::new(10, model.residual_contract(1).unwrap().profile(), 21,
            vec![0, 1], &[[0.0, 0.0], [1.0, 1.0]]).unwrap(),
        IdentityAnchor::new(20, model.residual_contract(2).unwrap().profile(), 22,
            vec![1, 0, 2], &[[-1.0, -1.0], [0.0, 0.0]]).unwrap(),
    ]).unwrap()
}
