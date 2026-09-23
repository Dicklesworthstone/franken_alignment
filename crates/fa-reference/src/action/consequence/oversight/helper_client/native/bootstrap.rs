//! Cold checkpoint startup of the ORIGINAL native helper. Operator-owned assets
//! are parsed as data, never downloaded, executed or inferred from a model name.
//! Model, tokenizer and evaluation authenticity remain independent host duties.

pub mod files;

/// Operator-selected data format, never inferred from a filename or a failed
/// parser. JSON supports only the existing strict raw ByteLevel BPE contract,
/// including its explicitly registered literal controls, not arbitrary pipelines.
/// Selecting a format does not authenticate the tokenizer or its trained model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTokenizerFormat { NativeArchive, HuggingFaceRawByteLevel }

use super::{NativeEvaluator, NativeHelperPolicy, TextDecoder};
use crate::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use crate::action::consequence::activation::monitor::decoder::sampled::config::{
    SamplingConfig, SamplingConfigError, MAX_SAMPLING_CONFIG_BYTES,
};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::{
    CheckpointError, LlamaConfig, PretrainedReceipt, PretrainedShardReceipt,
};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadBudget;
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;

/// Exact independently provisioned assets for one cold helper owner. All slices
/// must remain stable during startup. The policy supplies the EXPECTED complete
/// decoder profile; a checkpoint's configuration cannot choose a weaker profile.
/// Sampling seed/stream are explicit configuration, not actor input or entropy.
/// There is no default monitor, tokenizer, stop set, sampler or effect credential.
pub struct NativeHelperBootstrap<'a> {
    pub policy: &'a NativeHelperPolicy,
    pub stream: u64,
    pub configuration: &'a [u8],
    pub tokenizer: &'a [u8],
    pub monitoring: &'a [u8],
    pub sampling: &'a [u8],
}
impl fmt::Debug for NativeHelperBootstrap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeHelperBootstrap").field("stream", &self.stream)
            .field("profile", &self.policy.decoder_profile).finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeBootstrapError {
    Contract(Error),
    Tokenizer(Error),
    Checkpoint(CheckpointError),
    Sampling(SamplingConfigError),
}
impl fmt::Display for NativeBootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeBootstrapError {}

impl NativeEvaluator {
    /// Load actual F32/F16/BF16 tensors through the existing bounded streaming
    /// loader, then attach its ORIGINAL all-layer monitor, sampler and exact BPE.
    /// Returns a fresh, uncomputed one-request evaluator plus the original weight
    /// interpretation receipt. A receipt is not authentication or qualification.
    ///
    /// Profile, tokenizer, native policy, sampling and monitor SIZE admission run
    /// before any weight read. Model-dependent monitor validation runs after the
    /// model loads, before returning an owner. Every failure returns no evaluator.
    /// The supplied reader position and shared budget remain consumed on failure;
    /// there is no seek, hidden retry, partial model, seed replacement or fallback.
    pub fn read_llama_checkpoint<R: Read + ?Sized>(
        bootstrap: NativeHelperBootstrap<'_>, source: &mut R, budget: &mut WeightReadBudget,
    ) -> Result<(Self, PretrainedReceipt), NativeBootstrapError> {
        Self::read_llama_checkpoint_with_tokenizer_format(
            bootstrap, source, budget, NativeTokenizerFormat::NativeArchive)
    }

    /// Select an existing tokenizer parser before any weight I/O. NativeArchive
    /// preserves the original API; JSON admission uses the same independently
    /// bound profile, full-input policy and control-only stop checks. A refused
    /// format is never retried through another parser. All later loading,
    /// monitoring, sampling and one-request evaluation remain the original path.
    pub fn read_llama_checkpoint_with_tokenizer_format<R: Read + ?Sized>(
        bootstrap: NativeHelperBootstrap<'_>, source: &mut R, budget: &mut WeightReadBudget,
        format: NativeTokenizerFormat,
    ) -> Result<(Self, PretrainedReceipt), NativeBootstrapError> {
        let tokenizer = bootstrap.preflight(format)?;
        let profile = &bootstrap.policy.decoder_profile;
        let (model, receipt) = DecoderModel::read_llama_safetensors(profile.identity(),
            profile.shape().context, bootstrap.configuration, source, budget)
            .map_err(NativeBootstrapError::Checkpoint)?;
        let evaluator = bootstrap.finish(model, tokenizer)?;
        Ok((evaluator, receipt))
    }

    /// Shard labels select ONLY the already supplied readers. The original index
    /// parser checks exact inventory and ALL headers before scalar ingestion.
    /// All shards and failed attempts share the caller's same WeightReadBudget.
    /// No shard filename in an index is opened, joined to a path or downloaded.
    pub fn read_llama_checkpoint_shards<R: Read>(
        bootstrap: NativeHelperBootstrap<'_>, index: &[u8], sources: &mut BTreeMap<String, R>,
        budget: &mut WeightReadBudget,
    ) -> Result<(Self, PretrainedShardReceipt), NativeBootstrapError> {
        Self::read_llama_checkpoint_shards_with_tokenizer_format(
            bootstrap, index, sources, budget, NativeTokenizerFormat::NativeArchive)
    }

    /// The same explicit tokenizer admission for a complete set of supplied
    /// shard readers. Invalid tokenizer/policy data consumes no weight-reader
    /// allowance. Tied-head semantics still come from the original Llama config;
    /// neither the format choice nor the shard index can change that contract.
    pub fn read_llama_checkpoint_shards_with_tokenizer_format<R: Read>(
        bootstrap: NativeHelperBootstrap<'_>, index: &[u8], sources: &mut BTreeMap<String, R>,
        budget: &mut WeightReadBudget, format: NativeTokenizerFormat,
    ) -> Result<(Self, PretrainedShardReceipt), NativeBootstrapError> {
        let tokenizer = bootstrap.preflight(format)?;
        let profile = &bootstrap.policy.decoder_profile;
        let (model, receipt) = DecoderModel::read_llama_shards(profile.identity(),
            profile.shape().context, bootstrap.configuration, index, sources, budget)
            .map_err(NativeBootstrapError::Checkpoint)?;
        let evaluator = bootstrap.finish(model, tokenizer)?;
        Ok((evaluator, receipt))
    }
}

impl NativeHelperBootstrap<'_> {
    fn preflight(&self, format: NativeTokenizerFormat) -> Result<ByteBpe, NativeBootstrapError> {
        if self.stream == 0 { return Err(NativeBootstrapError::Contract(Error::InvalidInput)); }
        if self.monitoring.len() > MAX_MONITOR_CONFIG_BYTES
            || self.sampling.len() > MAX_SAMPLING_CONFIG_BYTES
        { return Err(NativeBootstrapError::Contract(Error::Limit)); }
        // Do not infer identity/shape/context from permissive model metadata.
        let expected = &self.policy.decoder_profile;
        let parsed = LlamaConfig::decode(expected.identity(), expected.shape().context, self.configuration)
            .map_err(NativeBootstrapError::Checkpoint)?;
        if parsed.profile() != expected { return Err(NativeBootstrapError::Contract(Error::Binding)); }
        let tokenizer = match format {
            NativeTokenizerFormat::NativeArchive => ByteBpe::from_bytes(expected, self.tokenizer),
            NativeTokenizerFormat::HuggingFaceRawByteLevel => ByteBpe::from_huggingface_json(
                expected, self.tokenizer, ByteBpe::MAX_HUGGINGFACE_JSON_BYTES),
        }.map_err(NativeBootstrapError::Tokenizer)?;
        self.policy.check_tokenizer(&tokenizer).map_err(NativeBootstrapError::Contract)?;
        SamplingConfig::decode(self.sampling, expected.shape().vocabulary)
            .map_err(NativeBootstrapError::Sampling)?;
        Ok(tokenizer)
    }

    fn finish(&self, model: DecoderModel, tokenizer: ByteBpe) -> Result<NativeEvaluator, NativeBootstrapError> {
        if model.profile() != &self.policy.decoder_profile {
            return Err(NativeBootstrapError::Contract(Error::Binding));
        }
        // Reuse the original parsers and owners; never replace missing monitors
        // with quiet probes or turn a byte-loaded checkpoint into a trusted vote.
        let decoder = MonitoredSampledDecoder::from_json(model, self.stream, self.monitoring, self.sampling)
            .map_err(NativeBootstrapError::Sampling)?;
        let text = TextDecoder::new(decoder, tokenizer).map_err(NativeBootstrapError::Contract)?;
        NativeEvaluator::new(text, self.policy.clone()).map_err(NativeBootstrapError::Contract)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tokenizer_tests;
