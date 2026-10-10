//! Static one-adapter startup of the original helper, under its original budgets.
//! The base is one explicit checkpoint; sharding remains a separate existing API.
use super::{NativeAsset, NativeAssetReadBudget, NativeEvaluator, NativeFileBootstrap,
    NativeFileBootstrapError, NativeHelperFiles, NativeHelperBootstrap,
    NativeTokenizerFormat, NativeBootstrapError, PretrainedReceipt, WeightReadBudget,
    MAX_WEIGHT_HEADER_BYTES, open_regular, read_asset};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderModel, DecoderProfile};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::lora::{
    LoraConfig, LoraError, LoraMergeBudget, LoraMergeReceipt, adapted_profile,
    MAX_LORA_CONFIG_BYTES, MAX_LORA_WEIGHT_BYTES,
};
use crate::Error;
use std::fmt;
use std::io::Read;
use std::path::Path;

/// All identities/files are independent operator input. The policy, tokenizer,
/// sampling, monitor and packet contract belong to the ADAPTED generation.
/// The base identity cannot select files or establish parameter authenticity.
pub struct NativeLoraFileBootstrap<'a> {
    pub base: NativeFileBootstrap<'a>,
    pub base_identity: DecoderIdentity,
    pub adapter_configuration: &'a Path,
    pub adapter_weights: &'a Path,
    pub adapter_weight_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLoraBootstrapReceipt {
    pub base: PretrainedReceipt,
    pub adapter: LoraMergeReceipt,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeLoraFileBootstrapError {
    File(NativeFileBootstrapError),
    Adapter(LoraError),
}
impl fmt::Display for NativeLoraFileBootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeLoraFileBootstrapError {}
impl From<NativeFileBootstrapError> for NativeLoraFileBootstrapError {
    fn from(error: NativeFileBootstrapError) -> Self { Self::File(error) }
}
impl From<LoraError> for NativeLoraFileBootstrapError {
    fn from(error: LoraError) -> Self { Self::Adapter(error) }
}

impl NativeEvaluator {
    /// No inference occurs before all base/adapter/monitor admission succeeds.
    /// Auxiliary config and original assets share the SAME asset budget. Base
    /// and adapter tensors share the SAME original scalar-reader budget. The
    /// caller's merge reservation remains spent on every late failure.
    pub fn from_llama_lora_files(request: NativeLoraFileBootstrap<'_>,
        assets: &mut NativeAssetReadBudget, weights: &mut WeightReadBudget,
        products: &mut LoraMergeBudget, format: NativeTokenizerFormat)
        -> Result<(Self, NativeLoraBootstrapReceipt), NativeLoraFileBootstrapError>
    {
        let base_request = request.base;
        base_request.limits.check()?;
        if base_request.stream == 0 {
            return Err(NativeFileBootstrapError::Contract(Error::InvalidInput).into());
        }
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
        let NativeHelperFiles { configuration, tokenizer, monitoring, sampling,
            weights: base_path } = base_request.files;
        let configuration = read_asset(configuration, base_request.limits.configuration_bytes,
            NativeAsset::Configuration, assets)?;
        let tokenizer = read_asset(tokenizer, base_request.limits.tokenizer_bytes, NativeAsset::Tokenizer, assets)?;
        let monitoring = read_asset(monitoring, base_request.limits.monitoring_bytes, NativeAsset::Monitoring, assets)?;
        let sampling = read_asset(sampling, base_request.limits.sampling_bytes, NativeAsset::Sampling, assets)?;
        let adapter_configuration = read_asset(request.adapter_configuration, MAX_LORA_CONFIG_BYTES,
            NativeAsset::AdapterConfiguration, assets)?;
        let config = LoraConfig::decode(&adapter_configuration)?;
        let work = config.estimate(&base_profile)?;
        products.require(work)?;
        let bootstrap = NativeHelperBootstrap { policy: base_request.policy, stream: base_request.stream,
            configuration: &configuration, tokenizer: &tokenizer, monitoring: &monitoring, sampling: &sampling };
        // This checks the exact final numerical profile and independently bound
        // tokenizer/policy before either weight file can open. The same Llama
        // configuration is subsequently loaded under the distinct base identity.
        let tokenizer = bootstrap.preflight(format).map_err(NativeFileBootstrapError::Bootstrap)?;
        let maximum = base_request.limits.weight_bytes
            .min(8 + MAX_WEIGHT_HEADER_BYTES + 4 * base_profile.parameter_count());
        let (base, _) = open_regular(base_path, maximum, NativeAsset::Weights)?;
        let mut base = base.take(maximum as u64 + 1);
        let (base, receipt) = DecoderModel::read_llama_safetensors(request.base_identity,
            base_profile.shape().context, &configuration, &mut base, weights)
            .map_err(|error| NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Checkpoint(error)))?;
        if base.profile() != &base_profile { return Err(LoraError::Identity.into()); }
        let maximum = request.adapter_weight_bytes
            .min(8 + MAX_WEIGHT_HEADER_BYTES + 4 * work.adapter_parameters);
        let (adapter, _) = open_regular(request.adapter_weights, maximum, NativeAsset::AdapterWeights)?;
        let mut adapter = adapter.take(maximum as u64 + 1);
        let (model, adapter) = base.read_lora_safetensors(expected.identity(),
            &adapter_configuration, &mut adapter, weights, products)?;
        let evaluator = bootstrap.finish(model, tokenizer).map_err(NativeFileBootstrapError::Bootstrap)?;
        Ok((evaluator, NativeLoraBootstrapReceipt { base: receipt, adapter }))
    }
}
