//! Ascending-rank binary64 products, one final binary32 rounding per changed weight.
use super::{LoraConfig, LoraError, LoraTarget};
use super::super::super::{DecoderLayerWeights, DecoderModel, DecoderProfile};
use super::super::super::weights::rounded;
use crate::Error;
use std::collections::BTreeMap;

pub(super) fn construct(base: &DecoderModel, profile: DecoderProfile, config: &LoraConfig,
    mut parameters: BTreeMap<String, Vec<f32>>) -> Result<DecoderModel, LoraError>
{
    let mut layers = Vec::new();
    layers.try_reserve_exact(base.data.layers.len()).map_err(|_| LoraError::Limit)?;
    for (index, layer) in base.data.layers.iter().enumerate() {
        let w = &layer.weights;
        let mut apply = |target, values: &[f32]| tensor(index, target, values,
            &profile, config, &mut parameters);
        layers.push(DecoderLayerWeights {
            attention_norm: copy(&w.attention_norm)?,
            queries: apply(LoraTarget::Query, &w.queries)?,
            keys: apply(LoraTarget::Key, &w.keys)?,
            values: apply(LoraTarget::Value, &w.values)?,
            attention_output: apply(LoraTarget::AttentionOutput, &w.attention_output)?,
            feed_forward_norm: copy(&w.feed_forward_norm)?,
            gate: apply(LoraTarget::Gate, &w.gate)?,
            up: apply(LoraTarget::Up, &w.up)?,
            down: apply(LoraTarget::Down, &w.down)?,
        });
    }
    if !parameters.is_empty() { return Err(LoraError::Model(Error::Binding)); }
    DecoderModel::new(profile, copy(&base.data.embeddings)?, layers,
        copy(&base.data.final_norm)?, copy(&base.data.output)?).map_err(LoraError::Model)
}
fn tensor(layer: usize, target: LoraTarget, original: &[f32], profile: &DecoderProfile,
    config: &LoraConfig, parameters: &mut BTreeMap<String, Vec<f32>>) -> Result<Vec<f32>, LoraError>
{
    if !config.targets().contains(&target) { return copy(original); }
    let prefix = target.prefix(layer);
    let a = parameters.remove(&format!("{prefix}.lora_A.weight")).ok_or(LoraError::Model(Error::Binding))?;
    let b = parameters.remove(&format!("{prefix}.lora_B.weight")).ok_or(LoraError::Model(Error::Binding))?;
    let (output, input) = target.dimensions(profile);
    let rank = config.rank();
    if original.len() != output * input || a.len() != rank * input || b.len() != output * rank {
        return Err(LoraError::Model(Error::Binding));
    }
    let scale = config.alpha() / rank as f64;
    let mut merged = Vec::new();
    merged.try_reserve_exact(original.len()).map_err(|_| LoraError::Limit)?;
    for row in 0..output {
        for column in 0..input {
            let mut sum = 0.0_f64;
            for inner in 0..rank {
                sum += f64::from(b[row * rank + inner]) * f64::from(a[inner * input + column]);
            }
            merged.push(rounded(f64::from(original[row * input + column]) + scale * sum)
                .map_err(LoraError::Model)?);
        }
    }
    Ok(merged)
}
fn copy(values: &[f32]) -> Result<Vec<f32>, LoraError> {
    let mut result = Vec::new();
    result.try_reserve_exact(values.len()).map_err(|_| LoraError::Limit)?;
    result.extend_from_slice(values);
    Ok(result)
}
