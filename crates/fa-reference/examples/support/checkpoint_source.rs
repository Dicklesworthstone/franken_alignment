//! Explicit checkpoint sources for the existing offline and publication examples.
//! Numerical decoding, index validation and static adaptation keep their original
//! library owners. This module grants no authority and performs no path discovery.
#![allow(dead_code)]

#[path = "checkpoint_source/files.rs"]
mod files;

use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderModel, DecoderProfile,
    safetensors::{
        lora::{LoraConfig, LoraMergeBudget, LoraMergeWork, MAX_LORA_CONFIG_BYTES, adapted_profile},
        pretrained::LlamaConfig,
        reader::{MAX_WEIGHT_READ_CALLS, WeightReadBudget},
        shards::{MAX_WEIGHT_INDEX_BYTES, MAX_WEIGHT_SET_BYTES, MAX_WEIGHT_SHARDS, check_shard_labels},
    },
};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

pub(crate) const MAX_MANIFEST_BYTES: usize = 65_536;

#[derive(Debug)]
enum Base {
    Single(PathBuf),
    Sharded { index: PathBuf, shards: BTreeMap<String, PathBuf> },
}
#[derive(Debug)]
struct Adapter {
    base_identity: DecoderIdentity,
    configuration: PathBuf,
    weights: PathBuf,
    merge_products: u64,
}
#[derive(Debug)]
pub(crate) struct CheckpointSource {
    base: Base,
    adapter: Option<Adapter>,
    weight_bytes: usize,
    weight_read_calls: usize,
}
#[derive(Debug)]
enum PreparedBase {
    Single(PathBuf),
    Sharded { index: Vec<u8>, shards: BTreeMap<String, PathBuf> },
}
#[derive(Debug)]
struct PreparedAdapter {
    configuration: Vec<u8>,
    weights: PathBuf,
    work: LoraMergeWork,
    products: LoraMergeBudget,
}
/// A one-use plan. Preparing reads only bounded auxiliary inputs; loading consumes
/// this owner and the caller's original cumulative weight-read allowance.
#[derive(Debug)]
pub(crate) struct PreparedCheckpoint {
    profile: DecoderProfile,
    base_configuration: LlamaConfig,
    configuration: Vec<u8>,
    base: PreparedBase,
    adapter: Option<PreparedAdapter>,
    weight_bytes: usize,
    weight_read_calls: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CheckpointWork {
    pub weight_bytes: usize,
    pub weight_read_calls: usize,
    /// Successfully admitted base file handles, not successful numerical models.
    pub base_files: usize,
    /// Original merge footprint once its work is reserved. A failed merge need
    /// not have updated every parameter; these are not successful-update counts.
    pub adapter_parameters: usize,
    pub updated_parameters: usize,
    pub merge_products: u64,
}
pub(crate) struct LoadedCheckpoint {
    pub model: DecoderModel,
    pub work: CheckpointWork,
}
impl fmt::Debug for LoadedCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedCheckpoint").field("profile", self.model.profile())
            .field("work", &self.work).finish()
    }
}
#[derive(Debug)]
pub(crate) struct CheckpointLoadError {
    pub reason: String,
    pub work: CheckpointWork,
}
impl fmt::Display for CheckpointLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}; checkpoint work {:?}", self.reason, self.work)
    }
}
impl std::error::Error for CheckpointLoadError {}

impl CheckpointSource {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value = strict_json::parse(bytes, Limits { max_bytes: MAX_MANIFEST_BYTES,
            max_depth: 8, max_items: 4096, max_string_bytes: 4096 }).map_err(debug)?;
        let mut root = Fields::new(value)?;
        if root.text("schema")? != "fa.checkpoint-weights/1" {
            return Err("unsupported checkpoint source schema".into());
        }
        let mut base = root.object("base")?;
        let selected = match base.text("kind")?.as_str() {
            "single" => Base::Single(base.path("path")?),
            "sharded" => {
                let index = base.path("index")?;
                let entries = base.object("shards")?.0;
                if entries.is_empty() || entries.len() > MAX_WEIGHT_SHARDS {
                    return Err("checkpoint needs a nonempty bounded shard map".into());
                }
                let mut shards = BTreeMap::new();
                for (label, value) in entries {
                    if label.is_empty() || label.len() > 256 {
                        return Err("checkpoint shard label exceeds its bound".into());
                    }
                    shards.insert(label, path(value.as_str().ok_or("shard path must be text")?)?);
                }
                Base::Sharded { index, shards }
            }
            _ => return Err("unsupported checkpoint base kind".into()),
        };
        base.end()?;
        let adapter = match root.take("adapter")? {
            Json::Null => None,
            value => {
                let mut fields = Fields::new(value)?;
                let base_identity = identity(fields.object("base_identity")?)?;
                let configuration = fields.path("configuration")?;
                let weights = fields.path("weights")?;
                let merge_products = fields.number("merge_products")?;
                LoraMergeBudget::new(merge_products).map_err(debug)?;
                fields.end()?;
                Some(Adapter { base_identity, configuration, weights, merge_products })
            }
        };
        let mut limits = root.object("limits")?;
        let weight_bytes = limits.size("weight_bytes")?;
        let weight_read_calls = limits.size("weight_read_calls")?;
        limits.end()?; root.end()?;
        if weight_bytes == 0 || weight_bytes > MAX_WEIGHT_SET_BYTES
            || weight_read_calls == 0 || weight_read_calls > MAX_WEIGHT_READ_CALLS {
            return Err("checkpoint source weight bounds exceed original limits".into());
        }
        Ok(Self { base: selected, adapter, weight_bytes, weight_read_calls })
    }

    pub(crate) fn prepare(self, final_identity: DecoderIdentity, context: usize,
        configuration: &[u8], mut read_aux: impl FnMut(&Path, usize) -> Result<Vec<u8>, String>)
        -> Result<PreparedCheckpoint, String>
    {
        let final_configuration = LlamaConfig::decode(final_identity, context, configuration).map_err(debug)?;
        let base_identity = self.adapter.as_ref().map_or(final_identity, |adapter| adapter.base_identity);
        let base_configuration = LlamaConfig::decode(base_identity, context, configuration).map_err(debug)?;
        let adapter = match self.adapter {
            None => None,
            Some(adapter) => {
                if adapted_profile(base_configuration.profile(), final_identity).map_err(debug)?
                    != *final_configuration.profile() {
                    return Err("adapter does not bind the complete final decoder profile".into());
                }
                let configuration = read_aux(&adapter.configuration, MAX_LORA_CONFIG_BYTES)?;
                let decoded = LoraConfig::decode(&configuration).map_err(debug)?;
                let work = decoded.estimate(base_configuration.profile()).map_err(debug)?;
                let products = LoraMergeBudget::new(adapter.merge_products).map_err(debug)?;
                products.require(work).map_err(debug)?;
                Some(PreparedAdapter { configuration, weights: adapter.weights, work, products })
            }
        };
        let base = match self.base {
            Base::Single(path) => PreparedBase::Single(path),
            Base::Sharded { index, shards } => {
                let index = read_aux(&index, MAX_WEIGHT_INDEX_BYTES)?;
                check_shard_labels(base_configuration.profile(), &index,
                    shards.keys().map(String::as_str), base_configuration.output_head()).map_err(debug)?;
                PreparedBase::Sharded { index, shards }
            }
        };
        Ok(PreparedCheckpoint { profile: final_configuration.profile().clone(), base_configuration,
            configuration: configuration.to_vec(), base, adapter,
            weight_bytes: self.weight_bytes, weight_read_calls: self.weight_read_calls })
    }
}
impl PreparedCheckpoint {
    pub(crate) fn profile(&self) -> &DecoderProfile { &self.profile }
    pub(crate) fn weight_bytes_limit(&self) -> usize { self.weight_bytes }
    pub(crate) fn weight_read_calls_limit(&self) -> usize { self.weight_read_calls }
}

fn debug(error: impl fmt::Debug) -> String { format!("{error:?}") }
fn path(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.file_name().is_none()
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_))) {
        return Err("checkpoint paths must be absolute normalized file paths".into());
    }
    Ok(path)
}
fn identity(mut fields: Fields) -> Result<DecoderIdentity, String> {
    let identity = DecoderIdentity { tenant: fields.number("tenant")?, model: fields.number("model")?,
        model_generation: fields.number("model_generation")?, tokenizer_generation: fields.number("tokenizer_generation")?,
        profile_generation: fields.number("profile_generation")? };
    fields.end()?;
    Ok(identity)
}
struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value { Json::Object(fields) => Ok(Self(fields)), _ => Err("expected checkpoint object".into()) }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0.remove(name).ok_or_else(|| format!("missing checkpoint field {name}"))
    }
    fn object(&mut self, name: &str) -> Result<Self, String> { Self::new(self.take(name)?) }
    fn text(&mut self, name: &str) -> Result<String, String> {
        self.take(name)?.as_str().map(str::to_owned).ok_or_else(|| format!("{name} must be text"))
    }
    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?.as_u64().ok_or_else(|| format!("{name} must be an unsigned integer"))
    }
    fn size(&mut self, name: &str) -> Result<usize, String> { usize::try_from(self.number(name)?).map_err(debug) }
    fn path(&mut self, name: &str) -> Result<PathBuf, String> { path(&self.text(name)?) }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err("unsupported checkpoint field".into()) }
    }
}
