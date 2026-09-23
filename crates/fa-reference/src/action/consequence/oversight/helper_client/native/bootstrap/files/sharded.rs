//! Cold startup from explicitly registered shard files into the ORIGINAL helper.
//! The index chooses labels, never paths. No raw concatenation or second decoder.

use super::{NativeAsset, NativeAssetReadBudget, NativeAssetReadUsage, NativeFileBootstrapError,
    NativeHelperFileLimits, NativeHelperPolicy, NativeEvaluator, NativeHelperBootstrap,
    NativeBootstrapError, WeightReadBudget, MAX_ASSET_READ_BYTES, MAX_ASSET_READ_CALLS,
    open_regular, read_asset};
use super::super::PretrainedShardReceipt;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::LlamaConfig;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadError;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::shards::{
    check_shard_labels, MAX_WEIGHT_INDEX_BYTES, MAX_WEIGHT_SHARDS,
};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub const MAX_SHARDED_ASSET_READ_BYTES: usize = MAX_ASSET_READ_BYTES + MAX_WEIGHT_INDEX_BYTES;

/// Provisioned assets and a COMPLETE label-to-path map. The path spelling need
/// not equal its label; the index cannot append a filename to a directory or
/// cause an unregistered file to open. Paths and all source bytes remain trusted
/// operator inputs, not authenticated artifacts or a cross-file snapshot.
pub struct NativeHelperShardFiles<'a> {
    pub configuration: &'a Path,
    pub tokenizer: &'a Path,
    pub monitoring: &'a Path,
    pub sampling: &'a Path,
    pub index: &'a Path,
    pub shards: &'a BTreeMap<String, PathBuf>,
}

pub struct NativeShardFileBootstrap<'a> {
    pub policy: &'a NativeHelperPolicy,
    pub stream: u64,
    pub files: NativeHelperShardFiles<'a>,
    /// weight_bytes is the SUM of all shard headers, prefixes and tensor bodies,
    /// not a per-shard allowance. The other four limits retain their meanings.
    pub limits: NativeHelperFileLimits,
    /// The index also consumes the same auxiliary read budget as the four assets.
    pub index_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeShardFileBootstrapError {
    Asset(NativeFileBootstrapError),
    Bootstrap(NativeBootstrapError),
    /// Only a bounded, admitted label is reported, never an untrusted path.
    Shard { label: String, error: NativeFileBootstrapError },
    Weights(WeightReadError),
}
impl fmt::Display for NativeShardFileBootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeShardFileBootstrapError {}
impl From<NativeFileBootstrapError> for NativeShardFileBootstrapError {
    fn from(error: NativeFileBootstrapError) -> Self { Self::Asset(error) }
}

impl NativeAssetReadBudget {
    /// Explicitly provision ONE auxiliary allowance that also includes the shard
    /// index. This is a constructor, not an expansion/reset of an existing budget.
    /// The original new constructor and its ceiling are unchanged. Read attempts,
    /// failures and EOF probes use the original accounting implementation.
    pub fn for_shards(bytes: usize, calls: usize) -> Result<Self, Error> {
        if bytes == 0 || calls == 0 { return Err(Error::InvalidInput); }
        if bytes > MAX_SHARDED_ASSET_READ_BYTES || calls > MAX_ASSET_READ_CALLS {
            return Err(Error::Limit);
        }
        Ok(Self { bytes, calls, usage: NativeAssetReadUsage::default() })
    }
}

impl NativeEvaluator {
    /// Load a sharded checkpoint into a fresh, uncomputed one-request helper.
    ///
    /// Native policy/config/tokenizer/sampler preflight precedes index loading.
    /// The ORIGINAL index parser must match every supplied source label before
    /// ANY weight path is inspected. Every weight handle is opened and checked
    /// for regular-file type and aggregate size before the first weight read.
    /// The original reader then validates ALL tensor directories before reading
    /// any scalar, enforces the same aggregate limit against the actual headers,
    /// and probes actual EOF on every handle. No Take-created EOF is accepted.
    ///
    /// Model-dependent monitor checks and exact NativeEvaluator admission still
    /// run before an owner escapes. Startup never computes a token or a verdict.
    /// Every failure drops partial model/handles, returns no helper and preserves
    /// both caller-owned budgets. There is no internal fresh-budget retry.
    ///
    /// As with from_llama_files, final-component symlink checks plus opened-file
    /// checks are NOT a race-free filesystem sandbox. Protect every path component
    /// and immutable asset set independently. This API grants no effect authority.
    pub fn from_llama_shard_files(request: NativeShardFileBootstrap<'_>,
        assets: &mut NativeAssetReadBudget, weights: &mut WeightReadBudget)
        -> Result<(Self, PretrainedShardReceipt), NativeShardFileBootstrapError>
    {
        request.limits.check()?;
        if request.stream == 0 || request.files.shards.is_empty() {
            return Err(NativeFileBootstrapError::Contract(Error::InvalidInput).into());
        }
        if request.index_bytes == 0 || request.index_bytes > MAX_WEIGHT_INDEX_BYTES
            || request.files.shards.len() > MAX_WEIGHT_SHARDS
            || request.files.shards.keys().any(|label| label.len() > 256)
        { return Err(NativeFileBootstrapError::Contract(Error::Limit).into()); }
        let files = request.files;
        let configuration = read_asset(files.configuration, request.limits.configuration_bytes,
            NativeAsset::Configuration, assets)?;
        let tokenizer = read_asset(files.tokenizer, request.limits.tokenizer_bytes, NativeAsset::Tokenizer, assets)?;
        let monitoring = read_asset(files.monitoring, request.limits.monitoring_bytes, NativeAsset::Monitoring, assets)?;
        let sampling = read_asset(files.sampling, request.limits.sampling_bytes, NativeAsset::Sampling, assets)?;
        let bootstrap = NativeHelperBootstrap { policy: request.policy, stream: request.stream,
            configuration: &configuration, tokenizer: &tokenizer, monitoring: &monitoring, sampling: &sampling };
        let tokenizer = bootstrap.preflight().map_err(NativeShardFileBootstrapError::Bootstrap)?;
        let profile = &request.policy.decoder_profile;
        // Retain the ORIGINAL config receipt and its explicit head-sharing law.
        // Both label preflight and actual directory reads must use that same law.
        let configuration_receipt = LlamaConfig::decode(profile.identity(), profile.shape().context, &configuration)
            .map_err(|error| NativeShardFileBootstrapError::Bootstrap(NativeBootstrapError::Checkpoint(error)))?;
        let index = read_asset(files.index, request.index_bytes, NativeAsset::WeightIndex, assets)?;
        check_shard_labels(profile, &index, files.shards.keys().map(String::as_str), configuration_receipt.output_head())
            .map_err(|error| NativeShardFileBootstrapError::Weights(WeightReadError::Refused(error)))?;

        let mut sources = BTreeMap::new();
        let mut total = 0_usize;
        for (label, path) in files.shards {
            let (file, size) = open_regular(path, request.limits.weight_bytes, NativeAsset::Weights)
                .map_err(|error| NativeShardFileBootstrapError::Shard { label: label.clone(), error })?;
            total = total.checked_add(size).ok_or(NativeFileBootstrapError::Limit(NativeAsset::Weights))?;
            if total > request.limits.weight_bytes {
                return Err(NativeFileBootstrapError::Limit(NativeAsset::Weights).into());
            }
            sources.insert(label.clone(), file);
        }
        let (model, loaded) = DecoderModel::read_safetensors_shards_bounded(
            profile.clone(), &index, &mut sources, weights, request.limits.weight_bytes, configuration_receipt.output_head(),
        ).map_err(NativeShardFileBootstrapError::Weights)?;
        let evaluator = bootstrap.finish(model, tokenizer).map_err(NativeShardFileBootstrapError::Bootstrap)?;
        Ok((evaluator, PretrainedShardReceipt { configuration: configuration_receipt, weights: loaded }))
    }
}

#[cfg(test)]
mod tests;
