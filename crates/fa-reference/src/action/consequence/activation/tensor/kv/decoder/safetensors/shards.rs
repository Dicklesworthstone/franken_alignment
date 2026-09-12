//! Exact shard assignment from a data-only index. Names do not open files.
//! Model shape, execution profile and authority never come from the index.

use super::{DecoderModel, DecoderProfile, TensorLoad, WeightError, MAX_WEIGHT_FILE_BYTES,
    MAX_WEIGHT_TENSORS, construct, describe, inspect_subset, inventory, read_tensor};
use crate::strict_json::{self, ErrorKind, Limits};
use std::collections::BTreeMap;

pub const MAX_WEIGHT_INDEX_BYTES: usize = 1_048_576;
pub const MAX_WEIGHT_SHARDS: usize = 128;
/// Aggregate source bytes, not a separate cap available again to each shard.
pub const MAX_WEIGHT_SET_BYTES: usize = MAX_WEIGHT_FILE_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShardLoad {
    pub file_bytes: usize,
    pub header_bytes: usize,
    pub data_bytes: usize,
    pub tensors: BTreeMap<String, TensorLoad>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShardedWeightLoadReceipt {
    pub profile: DecoderProfile,
    pub index_bytes: usize,
    pub file_bytes: usize,
    pub data_bytes: usize,
    pub normalized_bytes: usize,
    pub shards: BTreeMap<String, ShardLoad>,
}

pub(super) struct ShardPlan {
    pub(super) partitions: BTreeMap<String, BTreeMap<String, Vec<usize>>>,
    pub(super) total_size: Option<usize>,
}
impl ShardPlan {
    pub(super) fn parse(profile: &DecoderProfile, index: &[u8]) -> Result<Self, WeightError> {
        let json = strict_json::parse(index, Limits { max_bytes: MAX_WEIGHT_INDEX_BYTES,
            max_depth: 3, max_items: MAX_WEIGHT_TENSORS * 4 + 32, max_string_bytes: 256,
        }).map_err(|error| match error.kind {
            ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => WeightError::Limit,
            _ => WeightError::Header,
        })?;
        let root = json.as_object().ok_or(WeightError::Header)?;
        if !root.contains_key("weight_map") || root.keys().any(|key| key != "weight_map" && key != "metadata") {
            return Err(WeightError::Header);
        }
        let total_size = match root.get("metadata") {
            None => None,
            Some(value) => {
                let map = value.as_object().ok_or(WeightError::Header)?;
                if map.keys().any(|key| key != "total_size") { return Err(WeightError::Header); }
                map.get("total_size").map(|size| usize::try_from(size.as_u64().ok_or(WeightError::Header)?)
                    .map_err(|_| WeightError::Limit)).transpose()?
            }
        };
        if total_size.is_some_and(|size| size > profile.parameter_count() * 4) { return Err(WeightError::Limit); }
        let weights = root["weight_map"].as_object().ok_or(WeightError::Header)?;
        let expected = inventory(profile);
        if !weights.keys().eq(expected.keys()) { return Err(WeightError::Inventory); }
        let mut partitions: BTreeMap<String, BTreeMap<String, Vec<usize>>> = BTreeMap::new();
        for (name, shape) in expected {
            let shard = weights[&name].as_str().ok_or(WeightError::Header)?;
            // Literal source-map labels. In particular, no URL, absolute path,
            // parent component, slash, drive prefix or control byte is accepted.
            if !valid_name(shard) { return Err(WeightError::Inventory); }
            partitions.entry(shard.to_owned()).or_default().insert(name, shape);
            if partitions.len() > MAX_WEIGHT_SHARDS { return Err(WeightError::Limit); }
        }
        Ok(Self { partitions, total_size })
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 256 && name.ends_with(".safetensors")
        && !name.starts_with('.') && !name.contains("..")
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

impl DecoderModel {
    /// Consume the exact complete weight_map and supplied shard set. Each shard
    /// is validated against ITS assigned inventory with the existing parser;
    /// duplicate/misplaced/unknown tensors are not resolved by last-writer-wins.
    /// Receipts retain actual source-file counts, not a fictional concatenation.
    pub fn from_safetensors_shards(
        profile: DecoderProfile, index: &[u8], sources: &BTreeMap<String, &[u8]>,
    ) -> Result<(Self, ShardedWeightLoadReceipt), WeightError> {
        let plan = ShardPlan::parse(&profile, index)?;
        if !plan.partitions.keys().eq(sources.keys()) { return Err(WeightError::Inventory); }
        let file_bytes = sources.values().try_fold(0_usize, |sum, bytes| sum.checked_add(bytes.len()))
            .ok_or(WeightError::Limit)?;
        if file_bytes > MAX_WEIGHT_SET_BYTES { return Err(WeightError::Limit); }
        let mut tensors = BTreeMap::new(); let mut shards = BTreeMap::new(); let mut data_bytes = 0;
        for (shard, expected) in &plan.partitions {
            let bytes = sources[shard];
            let (selected, header_bytes) = inspect_subset(expected, bytes)?;
            let raw = bytes.len() - 8 - header_bytes;
            data_bytes += raw;
            shards.insert(shard.clone(), ShardLoad { file_bytes: bytes.len(), header_bytes,
                data_bytes: raw, tensors: describe(&selected) });
            for (name, tensor) in selected {
                if tensors.insert(name, tensor).is_some() { return Err(WeightError::Inventory); }
            }
        }
        if plan.total_size.is_some_and(|total| total != data_bytes) { return Err(WeightError::Inventory); }
        let receipt = ShardedWeightLoadReceipt { normalized_bytes: profile.parameter_count() * 4,
            profile: profile.clone(), index_bytes: index.len(), file_bytes, data_bytes, shards };
        let model = construct(profile, |name| read_tensor(name, &tensors))?;
        Ok((model, receipt))
    }
}
