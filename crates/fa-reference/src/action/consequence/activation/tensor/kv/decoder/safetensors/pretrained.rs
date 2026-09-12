//! Explicit Llama configuration negotiation and bounded local-file ingestion.
//! This never imports remote code, follows a model name, or guesses an architecture.

use super::{WeightError, WeightLoadReceipt, MAX_WEIGHT_FILE_BYTES, MAX_WEIGHT_HEADER_BYTES};
use super::super::{DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

pub const MAX_CONFIG_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigIssue { Syntax, Missing, Type, Unsupported }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointInput { Configuration, Weights }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStage { Metadata, Open, Read }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointError {
    Configuration { field: String, issue: ConfigIssue },
    Limit,
    Profile(Error),
    Weights(WeightError),
    NotRegular(CheckpointInput),
    Io { input: CheckpointInput, stage: FileStage, kind: io::ErrorKind },
}
impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for CheckpointError {}

/// Accepted numerical configuration and all defaulted/ignored field names.
/// Identity is supplied independently, not trusted from a checkpoint's strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlamaConfig {
    profile: DecoderProfile,
    trained_context: usize,
    bytes: usize,
    defaults: BTreeSet<String>,
    ignored: BTreeSet<String>,
}
impl LlamaConfig {
    pub fn profile(&self) -> &DecoderProfile { &self.profile }
    pub fn trained_context(&self) -> usize { self.trained_context }
    pub fn config_bytes(&self) -> usize { self.bytes }
    pub fn defaulted_fields(&self) -> &BTreeSet<String> { &self.defaults }
    pub fn ignored_metadata_fields(&self) -> &BTreeSet<String> { &self.ignored }

    /// Requires core dimensions, model_type and RMS epsilon. Recognized optional
    /// fields use the explicitly pinned Llama defaults and are listed in receipt.
    /// Execution context must be positive and no greater than the trained limit.
    /// Unsupported scaling/bias/tying/custom code is rejected, never approximated.
    pub fn decode(identity: DecoderIdentity, context: usize, bytes: &[u8]) -> Result<Self, CheckpointError> {
        let parsed = strict_json::parse(bytes, Limits {
            max_bytes: MAX_CONFIG_BYTES, max_depth: 5, max_items: 2048, max_string_bytes: 4096,
        }).map_err(|error| match error.kind {
            ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => CheckpointError::Limit,
            _ => config_error("$", ConfigIssue::Syntax),
        })?;
        let root = parsed.as_object().ok_or_else(|| config_error("$", ConfigIssue::Type))?;
        let mut defaults = BTreeSet::new();
        let mut ignored = BTreeSet::new();
        for (key, value) in root {
            if CORE_FIELDS.contains(&key.as_str()) { continue; }
            validate_metadata(key, value)?;
            ignored.insert(key.clone());
        }
        if required(root, "model_type")?.as_str() != Some("llama") {
            return Err(config_error("model_type", ConfigIssue::Unsupported));
        }
        if let Some(value) = root.get("architectures") {
            let items = value.as_array().ok_or_else(|| config_error("architectures", ConfigIssue::Type))?;
            if items.len() != 1 || items[0].as_str() != Some("LlamaForCausalLM") {
                return Err(config_error("architectures", ConfigIssue::Unsupported));
            }
        } else { defaults.insert("architectures".to_owned()); }
        for key in ["attention_bias", "mlp_bias", "tie_word_embeddings", "is_encoder_decoder", "add_cross_attention"] {
            if boolean(root, key, false, &mut defaults)? { return Err(config_error(key, ConfigIssue::Unsupported)); }
        }
        match root.get("hidden_act") {
            Some(value) if value.as_str() == Some("silu") => {},
            Some(_) => return Err(config_error("hidden_act", ConfigIssue::Unsupported)),
            None => { defaults.insert("hidden_act".to_owned()); }
        }
        match root.get("pretraining_tp") {
            Some(value) if value.as_u64() == Some(1) => {},
            Some(_) => return Err(config_error("pretraining_tp", ConfigIssue::Unsupported)),
            None => { defaults.insert("pretraining_tp".to_owned()); }
        }
        for (key, expected) in [("attention_dropout", 0.0), ("partial_rotary_factor", 1.0)] {
            if let Some(value) = root.get(key) {
                if number(key, value)? != expected { return Err(config_error(key, ConfigIssue::Unsupported)); }
            } else { defaults.insert(key.to_owned()); }
        }
        if root.get("rope_scaling").is_some_and(|value| !value.is_null()) {
            return Err(config_error("rope_scaling", ConfigIssue::Unsupported));
        }
        let theta = rope_theta(root, &mut defaults)?;
        let query_heads = integer(root, "num_attention_heads")?;
        let cache_heads = match root.get("num_key_value_heads") {
            None | Some(Json::Null) => { defaults.insert("num_key_value_heads".to_owned()); query_heads }
            Some(value) => usize_value("num_key_value_heads", value)?,
        };
        let trained_context = integer(root, "max_position_embeddings")?;
        if context == 0 || context > trained_context { return Err(config_error("max_position_embeddings", ConfigIssue::Unsupported)); }
        let profile = DecoderProfile::new(identity, DecoderShape {
            vocabulary: integer(root, "vocab_size")?, hidden: integer(root, "hidden_size")?,
            intermediate: integer(root, "intermediate_size")?, layers: integer(root, "num_hidden_layers")?,
            query_heads, cache_heads, context,
        }, number("rms_norm_eps", required(root, "rms_norm_eps")?)?, theta).map_err(CheckpointError::Profile)?;
        match root.get("head_dim") {
            None | Some(Json::Null) => { defaults.insert("head_dim".to_owned()); }
            Some(value) if usize_value("head_dim", value)? == profile.head_width() => {},
            Some(_) => return Err(config_error("head_dim", ConfigIssue::Unsupported)),
        }
        Ok(Self { profile, trained_context, bytes: bytes.len(), defaults, ignored })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PretrainedReceipt { pub configuration: LlamaConfig, pub weights: WeightLoadReceipt }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointFileLimits { pub config_bytes: usize, pub weight_bytes: usize }
impl Default for CheckpointFileLimits {
    fn default() -> Self { Self { config_bytes: MAX_CONFIG_BYTES, weight_bytes: MAX_WEIGHT_FILE_BYTES } }
}

impl DecoderModel {
    pub fn from_llama_safetensors(
        identity: DecoderIdentity, context: usize, configuration: &[u8], weights: &[u8],
    ) -> Result<(Self, PretrainedReceipt), CheckpointError> {
        let configuration = LlamaConfig::decode(identity, context, configuration)?;
        load(configuration, weights)
    }

    /// Two explicit operator-controlled regular files. Config admission completes
    /// BEFORE opening weights; a malformed config cannot provoke a weight read.
    /// No pickle fallback, inferred filename, download, shard traversal or writes.
    pub fn from_llama_files(
        identity: DecoderIdentity, context: usize, configuration: impl AsRef<Path>,
        weights: impl AsRef<Path>, limits: CheckpointFileLimits,
    ) -> Result<(Self, PretrainedReceipt), CheckpointError> {
        if limits.config_bytes == 0 || limits.weight_bytes == 0
            || limits.config_bytes > MAX_CONFIG_BYTES || limits.weight_bytes > MAX_WEIGHT_FILE_BYTES
        { return Err(CheckpointError::Limit); }
        let bytes = read_file(configuration.as_ref(), limits.config_bytes, CheckpointInput::Configuration)?;
        let configuration = LlamaConfig::decode(identity, context, &bytes)?;
        let model_bound = 8 + MAX_WEIGHT_HEADER_BYTES + configuration.profile.parameter_count() * 4;
        let weights = read_file(weights.as_ref(), limits.weight_bytes.min(model_bound), CheckpointInput::Weights)?;
        load(configuration, &weights)
    }
}

fn load(configuration: LlamaConfig, weights: &[u8]) -> Result<(DecoderModel, PretrainedReceipt), CheckpointError> {
    let (model, weights) = DecoderModel::from_safetensors(configuration.profile.clone(), weights).map_err(CheckpointError::Weights)?;
    Ok((model, PretrainedReceipt { configuration, weights }))
}
fn config_error(field: &str, issue: ConfigIssue) -> CheckpointError {
    CheckpointError::Configuration { field: field.to_owned(), issue }
}
fn required<'a>(root: &'a BTreeMap<String, Json>, key: &str) -> Result<&'a Json, CheckpointError> {
    root.get(key).ok_or_else(|| config_error(key, ConfigIssue::Missing))
}
fn usize_value(key: &str, value: &Json) -> Result<usize, CheckpointError> {
    value.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| config_error(key, ConfigIssue::Type))
}
fn integer(root: &BTreeMap<String, Json>, key: &str) -> Result<usize, CheckpointError> { usize_value(key, required(root, key)?) }
fn number(key: &str, value: &Json) -> Result<f64, CheckpointError> {
    let parsed = match value { Json::Number(n) => n.lexeme().parse::<f64>().ok(), _ => None };
    parsed.filter(|n| n.is_finite()).ok_or_else(|| config_error(key, ConfigIssue::Type))
}
fn boolean(root: &BTreeMap<String, Json>, key: &str, fallback: bool, defaults: &mut BTreeSet<String>) -> Result<bool, CheckpointError> {
    match root.get(key) {
        Some(value) => value.as_bool().ok_or_else(|| config_error(key, ConfigIssue::Type)),
        None => { defaults.insert(key.to_owned()); Ok(fallback) }
    }
}
fn rope_theta(root: &BTreeMap<String, Json>, defaults: &mut BTreeSet<String>) -> Result<f64, CheckpointError> {
    let legacy = root.get("rope_theta").map(|value| number("rope_theta", value)).transpose()?;
    if let Some(value) = root.get("rope_parameters").filter(|value| !value.is_null()) {
        let object = value.as_object().ok_or_else(|| config_error("rope_parameters", ConfigIssue::Type))?;
        if object.keys().any(|key| !["rope_type", "rope_theta"].contains(&key.as_str())) {
            return Err(config_error("rope_parameters", ConfigIssue::Unsupported));
        }
        if let Some(kind) = object.get("rope_type") {
            if kind.as_str() != Some("default") { return Err(config_error("rope_parameters.rope_type", ConfigIssue::Unsupported)); }
        } else { defaults.insert("rope_parameters.rope_type".to_owned()); }
        let theta = number("rope_parameters.rope_theta", required(object, "rope_theta")?)?;
        if legacy.is_some_and(|old| old.to_bits() != theta.to_bits()) {
            return Err(config_error("rope_theta", ConfigIssue::Unsupported));
        }
        return Ok(theta);
    }
    Ok(legacy.unwrap_or_else(|| { defaults.insert("rope_theta".to_owned()); 10000.0 }))
}

const CORE_FIELDS: &[&str] = &[
    "model_type", "architectures", "vocab_size", "hidden_size", "intermediate_size",
    "num_hidden_layers", "num_attention_heads", "num_key_value_heads", "max_position_embeddings",
    "rms_norm_eps", "rope_theta", "rope_parameters", "rope_scaling", "hidden_act", "head_dim",
    "attention_bias", "mlp_bias", "tie_word_embeddings", "pretraining_tp", "attention_dropout",
    "partial_rotary_factor", "is_encoder_decoder", "add_cross_attention",
];
fn validate_metadata(key: &str, value: &Json) -> Result<(), CheckpointError> {
    let valid = match key {
        "_name_or_path" | "_commit_hash" | "transformers_version" | "torch_dtype" | "dtype" => value.is_null() || value.as_str().is_some(),
        "use_cache" | "return_dict" | "output_attentions" | "output_hidden_states" => value.as_bool().is_some(),
        "initializer_range" => number(key, value)? >= 0.0,
        "bos_token_id" | "pad_token_id" => value.is_null() || value.as_u64().is_some(),
        "eos_token_id" => value.is_null() || value.as_u64().is_some()
            || value.as_array().is_some_and(|items| !items.is_empty() && items.iter().all(|item| item.as_u64().is_some())),
        _ => return Err(config_error(key, ConfigIssue::Unsupported)),
    };
    if !valid { return Err(config_error(key, ConfigIssue::Type)); }
    Ok(())
}
fn read_file(path: &Path, limit: usize, input: CheckpointInput) -> Result<Vec<u8>, CheckpointError> {
    let failure = |stage, error: io::Error| CheckpointError::Io { input, stage, kind: error.kind() };
    let before = fs::symlink_metadata(path).map_err(|e| failure(FileStage::Metadata, e))?;
    if !before.is_file() || before.file_type().is_symlink() { return Err(CheckpointError::NotRegular(input)); }
    if before.len() > limit as u64 { return Err(CheckpointError::Limit); }
    let file = File::open(path).map_err(|e| failure(FileStage::Open, e))?;
    let opened = file.metadata().map_err(|e| failure(FileStage::Metadata, e))?;
    if !opened.is_file() { return Err(CheckpointError::NotRegular(input)); }
    if opened.len() > limit as u64 { return Err(CheckpointError::Limit); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(opened.len() as usize).map_err(|_| CheckpointError::Limit)?;
    file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|e| failure(FileStage::Read, e))?;
    if bytes.len() > limit { return Err(CheckpointError::Limit); }
    Ok(bytes)
}
