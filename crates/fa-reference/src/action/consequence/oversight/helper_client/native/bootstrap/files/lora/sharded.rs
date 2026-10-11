//! The original all-header shard reader followed by the original static merger.
//! No shard labels become paths, no base helper is exposed, and no budget resets.

use super::{NativeLoraFileBootstrapError, NativeAsset, NativeAssetReadBudget, NativeEvaluator,
    NativeFileBootstrapError, NativeHelperBootstrap, NativeTokenizerFormat, NativeBootstrapError,
    WeightReadBudget, DecoderIdentity, DecoderModel, DecoderProfile, LoraConfig, LoraError,
    LoraMergeBudget, LoraMergeReceipt, adapted_profile, MAX_LORA_CONFIG_BYTES,
    MAX_LORA_WEIGHT_BYTES, MAX_WEIGHT_HEADER_BYTES, open_regular, read_asset};
use super::super::sharded::{NativeShardFileBootstrap, NativeShardFileBootstrapError};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::{
    LlamaConfig, PretrainedShardReceipt,
};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadError;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::shards::{
    check_shard_labels, MAX_WEIGHT_INDEX_BYTES, MAX_WEIGHT_SHARDS,
};
use crate::Error;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

/// The complete operator-selected base shard map and one static adapter. All
/// policy/tokenizer/monitor identities describe the resulting ADAPTED model.
/// Base weight_bytes bounds the SUM of its shards; adapter_weight_bytes bounds
/// the one adapter. Actual reads across both consume the same supplied budget.
pub struct NativeLoraShardFileBootstrap<'a> {
    pub base: NativeShardFileBootstrap<'a>,
    pub base_identity: DecoderIdentity,
    pub adapter_configuration: &'a Path,
    pub adapter_weights: &'a Path,
    pub adapter_weight_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLoraShardBootstrapReceipt {
    pub base: PretrainedShardReceipt,
    pub adapter: LoraMergeReceipt,
}

impl NativeEvaluator {
    /// Admit an adapted native worker from a COMPLETE explicit base shard set.
    /// Configs, final tokenizer/policy, adapter shape/work and index labels are
    /// checked before any weight read. Every base file and the adapter are opened
    /// and bounded before scalar ingestion; no index entry discovers a path.
    ///
    /// The original shard reader validates every base directory before any base
    /// scalar, under one aggregate base bound. The original adapter reader then
    /// validates its own directory/scalars/EOF and reserves the complete merge
    /// work before numerical merging. This is not an all-assets transaction or
    /// an all-base-and-adapter-header preflight. Late adapter failure retains the
    /// consumed base reads; late merge or monitor failure retains its reservation.
    ///
    /// No unadapted evaluator, partial model or inference token escapes. The
    /// original auxiliary and weight budgets retain all reads/failed attempts;
    /// no retry, fallback, new allowance, alternate sampler or engine is created.
    pub fn from_llama_lora_shard_files(request: NativeLoraShardFileBootstrap<'_>,
        assets: &mut NativeAssetReadBudget, weights: &mut WeightReadBudget,
        products: &mut LoraMergeBudget, format: NativeTokenizerFormat)
        -> Result<(Self, NativeLoraShardBootstrapReceipt), NativeLoraFileBootstrapError>
    {
        let base_request = request.base;
        base_request.limits.check()?;
        if base_request.stream == 0 || base_request.files.shards.is_empty() {
            return Err(NativeFileBootstrapError::Contract(Error::InvalidInput).into());
        }
        if base_request.index_bytes == 0 || base_request.index_bytes > MAX_WEIGHT_INDEX_BYTES
            || base_request.files.shards.len() > MAX_WEIGHT_SHARDS
            || base_request.files.shards.keys().any(|label| label.len() > 256)
        { return Err(NativeFileBootstrapError::Contract(Error::Limit).into()); }
        if request.adapter_weight_bytes == 0 || request.adapter_weight_bytes > MAX_LORA_WEIGHT_BYTES {
            return Err(NativeFileBootstrapError::Limit(NativeAsset::AdapterWeights).into());
        }
        let expected = &base_request.policy.decoder_profile;
        let base_profile = DecoderProfile::new(request.base_identity, expected.shape(),
            expected.epsilon(), expected.theta())
            .and_then(|profile| profile.with_rotary_scaling(expected.rotary_scaling()))
            .map_err(LoraError::Model)?;
        if adapted_profile(&base_profile, expected.identity())? != *expected {
            return Err(LoraError::Identity.into());
        }
        let files = base_request.files;
        let configuration = read_asset(files.configuration, base_request.limits.configuration_bytes,
            NativeAsset::Configuration, assets)?;
        let tokenizer = read_asset(files.tokenizer, base_request.limits.tokenizer_bytes, NativeAsset::Tokenizer, assets)?;
        let monitoring = read_asset(files.monitoring, base_request.limits.monitoring_bytes, NativeAsset::Monitoring, assets)?;
        let sampling = read_asset(files.sampling, base_request.limits.sampling_bytes, NativeAsset::Sampling, assets)?;
        let adapter_configuration = read_asset(request.adapter_configuration, MAX_LORA_CONFIG_BYTES,
            NativeAsset::AdapterConfiguration, assets)?;
        let config = LoraConfig::decode(&adapter_configuration)?;
        let work = config.estimate(&base_profile)?;
        products.require(work)?;
        let bootstrap = NativeHelperBootstrap { policy: base_request.policy, stream: base_request.stream,
            configuration: &configuration, tokenizer: &tokenizer, monitoring: &monitoring, sampling: &sampling };
        let tokenizer = bootstrap.preflight(format).map_err(NativeFileBootstrapError::Bootstrap)?;
        let configuration_receipt = LlamaConfig::decode(request.base_identity,
            base_profile.shape().context, &configuration).map_err(|error|
                NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Checkpoint(error)))?;
        if configuration_receipt.profile() != &base_profile { return Err(LoraError::Identity.into()); }
        let index = read_asset(files.index, base_request.index_bytes, NativeAsset::WeightIndex, assets)?;
        check_shard_labels(&base_profile, &index, files.shards.keys().map(String::as_str),
            configuration_receipt.output_head()).map_err(|error|
                NativeShardFileBootstrapError::Weights(WeightReadError::Refused(error)))?;

        let mut sources = BTreeMap::new();
        let mut total = 0_usize;
        for (label, path) in files.shards {
            let (file, size) = open_regular(path, base_request.limits.weight_bytes, NativeAsset::Weights)
                .map_err(|error| NativeShardFileBootstrapError::Shard { label: label.clone(), error })?;
            total = total.checked_add(size).ok_or(NativeFileBootstrapError::Limit(NativeAsset::Weights))?;
            if total > base_request.limits.weight_bytes {
                return Err(NativeFileBootstrapError::Limit(NativeAsset::Weights).into());
            }
            sources.insert(label.clone(), file);
        }
        let maximum = request.adapter_weight_bytes
            .min(8 + MAX_WEIGHT_HEADER_BYTES + 4 * work.adapter_parameters);
        let (adapter, _) = open_regular(request.adapter_weights, maximum, NativeAsset::AdapterWeights)?;
        let mut adapter = adapter.take(maximum as u64 + 1);
        let (base, loaded) = DecoderModel::read_safetensors_shards_bounded(base_profile,
            &index, &mut sources, weights, base_request.limits.weight_bytes,
            configuration_receipt.output_head()).map_err(NativeShardFileBootstrapError::Weights)?;
        let (model, adapter) = base.read_lora_safetensors(expected.identity(),
            &adapter_configuration, &mut adapter, weights, products)?;
        let evaluator = bootstrap.finish(model, tokenizer).map_err(NativeFileBootstrapError::Bootstrap)?;
        Ok((evaluator, NativeLoraShardBootstrapReceipt {
            base: PretrainedShardReceipt { configuration: configuration_receipt, weights: loaded }, adapter,
        }))
    }
}
