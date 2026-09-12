//! Bounded operator-supplied probes for the actual decoder residual inventory.
//! Config fields declare identities; they do not authenticate a learned detector.

use super::MonitoredDecoder;
use super::super::{RefinementBudget, RefinementMonitor, MAX_LEVELS, MAX_PROBES};
use super::super::super::probe::LinearProbe;
use super::super::super::tensor::kv::decoder::{DecoderIdentity, DecoderModel};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_MONITOR_CONFIG_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonitorConfigError {
    Syntax,
    Limit,
    Field(String),
    ModelIdentity,
    Monitor(Error),
}
impl fmt::Display for MonitorConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for MonitorConfigError {}

impl MonitoredDecoder {
    /// Strict schema fa.decoder-monitor/1. Every layer, ladder, probe, coefficient
    /// and both local/global budgets are explicit. No defaults or learned weights
    /// are guessed. Decimals round to finite binary32, just as LinearProbe accepts.
    /// The JSON cannot select a file, executable, model, stream or effect authority.
    /// The exact supplied DecoderModel is retained, not reloaded by a model label.
    pub fn from_json(model: DecoderModel, stream: u64, bytes: &[u8]) -> Result<Self, MonitorConfigError> {
        let parsed = strict_json::parse(bytes, Limits {
            max_bytes: MAX_MONITOR_CONFIG_BYTES, max_depth: 8,
            max_items: 262_144, max_string_bytes: 256,
        }).map_err(|error| match error.kind {
            ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => MonitorConfigError::Limit,
            _ => MonitorConfigError::Syntax,
        })?;
        let root = object(&parsed, &["schema", "generation", "identity", "budget", "layers"], "$")?;
        if root["schema"].as_str() != Some("fa.decoder-monitor/1") { return Err(field("schema")); }
        let generation = positive(&root["generation"], "generation")?;
        if identity(&root["identity"])? != model.profile().identity() { return Err(MonitorConfigError::ModelIdentity); }
        let budget = allowance(&root["budget"], "budget")?;
        let layers = array(&root["layers"], "layers")?;
        if layers.len() != model.profile().shape().layers { return Err(field("layers")); }
        let mut monitors = BTreeMap::new();
        for (index, value) in layers.iter().enumerate() {
            let path = format!("layers[{index}]");
            let layer = object(value, &["layer", "levels", "budget", "probes"], &path)?;
            let id = positive(&layer["layer"], &format!("{path}.layer"))?;
            if monitors.contains_key(&id) { return Err(MonitorConfigError::Monitor(Error::Duplicate)); }
            let contract = model.residual_contract(id).map_err(MonitorConfigError::Monitor)?;
            let budget = allowance(&layer["budget"], &format!("{path}.budget"))?;
            let rung_values = array(&layer["levels"], &format!("{path}.levels"))?;
            if rung_values.is_empty() || rung_values.len() > MAX_LEVELS { return Err(field(&format!("{path}.levels"))); }
            let levels = rung_values.iter().map(|value| value.as_u64().and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| field(&format!("{path}.levels")))).collect::<Result<Vec<_>, _>>()?;
            let probe_values = array(&layer["probes"], &format!("{path}.probes"))?;
            if probe_values.is_empty() || probe_values.len() > MAX_PROBES { return Err(field(&format!("{path}.probes"))); }
            let mut probes = Vec::new();
            probes.try_reserve_exact(probe_values.len()).map_err(|_| MonitorConfigError::Limit)?;
            for (index, value) in probe_values.iter().enumerate() {
                let path = format!("{path}.probes[{index}]");
                let probe = object(value, &["id", "generation", "weights", "bias", "threshold"], &path)?;
                let weights = array(&probe["weights"], &format!("{path}.weights"))?;
                if weights.len() != contract.dimensions() { return Err(field(&format!("{path}.weights"))); }
                let weights = weights.iter().map(|value| scalar(value, &format!("{path}.weights")))
                    .collect::<Result<Vec<_>, _>>()?;
                probes.push(LinearProbe::new(
                    positive(&probe["id"], &format!("{path}.id"))?,
                    positive(&probe["generation"], &format!("{path}.generation"))?,
                    contract.profile(), &weights, scalar(&probe["bias"], &format!("{path}.bias"))?,
                    scalar(&probe["threshold"], &format!("{path}.threshold"))?,
                ).map_err(MonitorConfigError::Monitor)?);
            }
            monitors.insert(id, RefinementMonitor::new(probes, levels, budget).map_err(MonitorConfigError::Monitor)?);
        }
        Self::new(model, stream, generation, monitors, budget).map_err(MonitorConfigError::Monitor)
    }
}

fn field(path: &str) -> MonitorConfigError { MonitorConfigError::Field(path.to_owned()) }
fn object<'a>(json: &'a Json, keys: &[&str], path: &str) -> Result<&'a BTreeMap<String, Json>, MonitorConfigError> {
    let object = json.as_object().ok_or_else(|| field(path))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) { return Err(field(path)); }
    Ok(object)
}
fn array<'a>(json: &'a Json, path: &str) -> Result<&'a [Json], MonitorConfigError> {
    json.as_array().ok_or_else(|| field(path))
}
fn positive(json: &Json, path: &str) -> Result<u64, MonitorConfigError> {
    json.as_u64().filter(|value| *value > 0).ok_or_else(|| field(path))
}
fn allowance(json: &Json, path: &str) -> Result<RefinementBudget, MonitorConfigError> {
    let object = object(json, &["encoded_bytes", "probe_coordinates"], path)?;
    let get = |key: &str| object[key].as_u64().and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| field(&format!("{path}.{key}")));
    Ok(RefinementBudget { encoded_bytes: get("encoded_bytes")?, probe_coordinates: get("probe_coordinates")? })
}
fn scalar(json: &Json, path: &str) -> Result<f32, MonitorConfigError> {
    let value = match json { Json::Number(number) => number.lexeme().parse::<f32>().ok(), _ => None };
    value.filter(|value| value.is_finite()).ok_or_else(|| field(path))
}
fn identity(json: &Json) -> Result<DecoderIdentity, MonitorConfigError> {
    let object = object(json, &["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"], "identity")?;
    let get = |key: &str| positive(&object[key], &format!("identity.{key}"));
    Ok(DecoderIdentity { tenant: get("tenant")?, model: get("model")?, model_generation: get("model_generation")?,
        tokenizer_generation: get("tokenizer_generation")?, profile_generation: get("profile_generation")? })
}
