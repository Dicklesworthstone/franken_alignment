//! Pinned plain-PEFT subset. No module regex, base lookup or training operation.
use super::{LoraError, LoraTarget, MAX_LORA_CONFIG_BYTES, MAX_LORA_RANK};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::ConfigIssue;
use crate::strict_json::{self, ErrorKind, Json, Limits};
use std::collections::{BTreeMap, BTreeSet};

/// One immutable plain LoRA configuration. Targets are exact short names, applied
/// to every original layer. Saved adapter names are absent from tensor keys.
/// Defaults below are pinned PEFT v0.14 plain-LoRA semantics, never model guesses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoraConfig {
    rank: usize,
    alpha_bits: u64,
    targets: Vec<LoraTarget>,
}
impl LoraConfig {
    pub fn rank(&self) -> usize { self.rank }
    pub fn alpha(&self) -> f64 { f64::from_bits(self.alpha_bits) }
    pub fn targets(&self) -> &[LoraTarget] { &self.targets }

    pub fn decode(bytes: &[u8]) -> Result<Self, LoraError> {
        let parsed = strict_json::parse(bytes, Limits { max_bytes: MAX_LORA_CONFIG_BYTES,
            max_depth: 5, max_items: 2048, max_string_bytes: 4096 })
            .map_err(|error| match error.kind {
                ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit
                    | ErrorKind::StringLimit => LoraError::Limit,
                _ => bad("$", ConfigIssue::Syntax),
            })?;
        let root = parsed.as_object().ok_or_else(|| bad("$", ConfigIssue::Type))?;
        string(root, "peft_type", "LORA")?;
        string(root, "task_type", "CAUSAL_LM")?;
        if required(root, "inference_mode")?.as_bool() != Some(true) {
            return Err(bad("inference_mode", ConfigIssue::Unsupported));
        }
        let rank = usize::try_from(required(root, "r")?.as_u64()
            .ok_or_else(|| bad("r", ConfigIssue::Type))?).map_err(|_| LoraError::Limit)?;
        if rank == 0 { return Err(bad("r", ConfigIssue::Unsupported)); }
        if rank > MAX_LORA_RANK { return Err(LoraError::Limit); }
        let alpha = number(required(root, "lora_alpha")?, "lora_alpha")?;
        if alpha <= 0.0 || alpha > 1_000_000.0 || alpha / rank as f64 == 0.0 {
            return Err(bad("lora_alpha", ConfigIssue::Unsupported));
        }
        let declared = required(root, "target_modules")?.as_array()
            .ok_or_else(|| bad("target_modules", ConfigIssue::Unsupported))?;
        if declared.is_empty() { return Err(bad("target_modules", ConfigIssue::Unsupported)); }
        if declared.len() > 7 { return Err(LoraError::Limit); }
        let mut targets = BTreeSet::new();
        for value in declared {
            let target = match value.as_str() {
                Some("q_proj") => LoraTarget::Query,
                Some("k_proj") => LoraTarget::Key,
                Some("v_proj") => LoraTarget::Value,
                Some("o_proj") => LoraTarget::AttentionOutput,
                Some("gate_proj") => LoraTarget::Gate,
                Some("up_proj") => LoraTarget::Up,
                Some("down_proj") => LoraTarget::Down,
                _ => return Err(bad("target_modules", ConfigIssue::Unsupported)),
            };
            if !targets.insert(target) { return Err(bad("target_modules", ConfigIssue::Unsupported)); }
        }
        for (key, value) in root {
            let allowed = match key.as_str() {
                "peft_type" | "task_type" | "inference_mode" | "r" | "lora_alpha"
                    | "target_modules" => true,
                "fan_in_fan_out" | "use_rslora" | "use_dora" | "lora_bias" => value.as_bool() == Some(false),
                "bias" => value.as_str() == Some("none"),
                // Dropout is inactive during this declared inference-only merge.
                "lora_dropout" => {
                    let dropout = number(value, "lora_dropout")?;
                    (0.0..1.0).contains(&dropout)
                }
                // These initializations leave the supplied base weights intact.
                // PiSSA/OLoRA/LoftQ/EVA and future string modes require distinct contracts.
                "init_lora_weights" => value.as_bool().is_some() || value.as_str() == Some("gaussian"),
                "modules_to_save" | "layers_to_transform" | "layers_pattern"
                    | "megatron_config" | "layer_replication" | "exclude_modules"
                    | "eva_config" | "auto_mapping" => value.is_null(),
                "rank_pattern" | "alpha_pattern" | "loftq_config" => value.as_object().is_some_and(BTreeMap::is_empty),
                "megatron_core" => value.as_str() == Some("megatron.core"),
                // Provenance labels only: never read as a path or treated as authentication.
                "base_model_name_or_path" | "revision" => value.is_null() || value.as_str().is_some(),
                _ => false,
            };
            if !allowed { return Err(bad(field(key), ConfigIssue::Unsupported)); }
        }
        Ok(Self { rank, alpha_bits: alpha.to_bits(), targets: targets.into_iter().collect() })
    }
}
fn required<'a>(root: &'a BTreeMap<String, Json>, key: &'static str) -> Result<&'a Json, LoraError> {
    root.get(key).ok_or_else(|| bad(key, ConfigIssue::Missing))
}
fn string(root: &BTreeMap<String, Json>, key: &'static str, expected: &str) -> Result<(), LoraError> {
    if required(root, key)?.as_str() != Some(expected) { return Err(bad(key, ConfigIssue::Unsupported)); }
    Ok(())
}
fn number(value: &Json, key: &'static str) -> Result<f64, LoraError> {
    let value = match value { Json::Number(n) => n.lexeme().parse::<f64>().ok(), _ => None };
    value.filter(|n| n.is_finite()).ok_or_else(|| bad(key, ConfigIssue::Type))
}
fn bad(field: &'static str, issue: ConfigIssue) -> LoraError {
    LoraError::Configuration { field, issue }
}
// Error labels are bounded static contract fields, never hostile source strings.
fn field(key: &str) -> &'static str {
    match key {
        "fan_in_fan_out" => "fan_in_fan_out", "use_rslora" => "use_rslora",
        "use_dora" => "use_dora", "lora_bias" => "lora_bias", "bias" => "bias",
        "lora_dropout" => "lora_dropout", "init_lora_weights" => "init_lora_weights",
        "modules_to_save" => "modules_to_save", "layers_to_transform" => "layers_to_transform",
        "layers_pattern" => "layers_pattern", "megatron_config" => "megatron_config",
        "layer_replication" => "layer_replication", "exclude_modules" => "exclude_modules",
        "eva_config" => "eva_config", "auto_mapping" => "auto_mapping",
        "rank_pattern" => "rank_pattern", "alpha_pattern" => "alpha_pattern",
        "loftq_config" => "loftq_config", "megatron_core" => "megatron_core",
        "base_model_name_or_path" => "base_model_name_or_path", "revision" => "revision",
        _ => "$",
    }
}
