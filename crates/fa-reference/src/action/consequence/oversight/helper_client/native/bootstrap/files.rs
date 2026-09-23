//! Explicit local-file startup and provisioned Unix peer integration. No path is
//! taken from model metadata, an index, a helper request or a generated response.
//! Operator-owned immutable files/private directories remain a host assumption.

pub mod sharded;

use super::{NativeTokenizerFormat, NativeBootstrapError, NativeEvaluator, NativeHelperBootstrap, NativeHelperPolicy,
    PretrainedReceipt, WeightReadBudget, MAX_MONITOR_CONFIG_BYTES, MAX_SAMPLING_CONFIG_BYTES};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::{
    MAX_WEIGHT_FILE_BYTES, MAX_WEIGHT_HEADER_BYTES,
};
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::MAX_CONFIG_BYTES;
use crate::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

/// Outer file-read bound; the original byte-BPE parser imposes its own tighter
/// inventory/encoding limits. This does not enlarge the supported token profile.
pub const MAX_TOKENIZER_ASSET_BYTES: usize = 16 * 1_048_576;
pub const MAX_ASSET_READ_BYTES: usize = MAX_CONFIG_BYTES + MAX_TOKENIZER_ASSET_BYTES
    + MAX_MONITOR_CONFIG_BYTES + MAX_SAMPLING_CONFIG_BYTES + 1;
pub const MAX_ASSET_READ_CALLS: usize = 65_536;
const READ_CHUNK: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAsset { Configuration, Tokenizer, Monitoring, Sampling, Weights, WeightIndex }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFileStage { Metadata, Open, Read }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeFileBootstrapError {
    Contract(Error),
    Bootstrap(NativeBootstrapError),
    NotRegular(NativeAsset),
    Limit(NativeAsset),
    Io { asset: NativeAsset, stage: NativeFileStage, kind: io::ErrorKind },
}
impl fmt::Display for NativeFileBootstrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeFileBootstrapError {}

/// Caller-selected paths only. No implicit filenames, directory traversal from
/// index data, download, model cache search, pickle or executable deserialization.
pub struct NativeHelperFiles<'a> {
    pub configuration: &'a Path,
    pub tokenizer: &'a Path,
    pub monitoring: &'a Path,
    pub sampling: &'a Path,
    pub weights: &'a Path,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHelperFileLimits {
    pub configuration_bytes: usize,
    pub tokenizer_bytes: usize,
    pub monitoring_bytes: usize,
    pub sampling_bytes: usize,
    pub weight_bytes: usize,
}
impl Default for NativeHelperFileLimits {
    fn default() -> Self {
        Self { configuration_bytes: MAX_CONFIG_BYTES, tokenizer_bytes: MAX_TOKENIZER_ASSET_BYTES,
            monitoring_bytes: MAX_MONITOR_CONFIG_BYTES, sampling_bytes: MAX_SAMPLING_CONFIG_BYTES,
            weight_bytes: MAX_WEIGHT_FILE_BYTES }
    }
}
impl NativeHelperFileLimits {
    fn check(self) -> Result<(), NativeFileBootstrapError> {
        for (limit, maximum) in [
            (self.configuration_bytes, MAX_CONFIG_BYTES), (self.tokenizer_bytes, MAX_TOKENIZER_ASSET_BYTES),
            (self.monitoring_bytes, MAX_MONITOR_CONFIG_BYTES), (self.sampling_bytes, MAX_SAMPLING_CONFIG_BYTES),
            (self.weight_bytes, MAX_WEIGHT_FILE_BYTES),
        ] {
            if limit == 0 || limit > maximum { return Err(NativeFileBootstrapError::Contract(Error::Limit)); }
        }
        Ok(())
    }
}
pub struct NativeFileBootstrap<'a> {
    pub policy: &'a NativeHelperPolicy,
    pub stream: u64,
    pub files: NativeHelperFiles<'a>,
    pub limits: NativeHelperFileLimits,
}

/// Actual auxiliary-asset Read attempts/bytes, separate from the original weight
/// loader's usage. Failed attempts and Interrupted calls remain charged. Metadata
/// and open operations are not mislabeled as reads. No wall-clock guarantee.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeAssetReadUsage { pub bytes_read: usize, pub read_calls: usize }
#[derive(Debug)]
pub struct NativeAssetReadBudget {
    bytes: usize,
    calls: usize,
    usage: NativeAssetReadUsage,
}
impl NativeAssetReadBudget {
    pub fn new(bytes: usize, calls: usize) -> Result<Self, Error> {
        if bytes == 0 || calls == 0 { return Err(Error::InvalidInput); }
        if bytes > MAX_ASSET_READ_BYTES || calls > MAX_ASSET_READ_CALLS { return Err(Error::Limit); }
        Ok(Self { bytes, calls, usage: NativeAssetReadUsage::default() })
    }
    pub fn usage(&self) -> NativeAssetReadUsage { self.usage }
    pub fn remaining_bytes(&self) -> usize { self.bytes - self.usage.bytes_read }
    pub fn remaining_calls(&self) -> usize { self.calls - self.usage.read_calls }
    fn read<R: Read>(&mut self, source: &mut R, buffer: &mut [u8], asset: NativeAsset)
        -> Result<usize, NativeFileBootstrapError>
    {
        let offered = buffer.len().min(self.remaining_bytes());
        if offered == 0 || self.remaining_calls() == 0 { return Err(NativeFileBootstrapError::Limit(asset)); }
        self.usage.read_calls += 1;
        let count = source.read(&mut buffer[..offered]).map_err(|error| io_error(asset, NativeFileStage::Read, error))?;
        if count > offered {
            return Err(NativeFileBootstrapError::Io { asset, stage: NativeFileStage::Read, kind: io::ErrorKind::InvalidData });
        }
        self.usage.bytes_read += count;
        Ok(count)
    }
}

impl NativeEvaluator {
    /// Read four bounded regular asset files, preflight their native bindings,
    /// then open and stream the explicit regular weight file. An invalid policy,
    /// tokenizer or sampling config cannot open weights. Success computes no token.
    /// Both caller-owned budgets survive every failure; no internal retry budget.
    ///
    /// Symlinks/nonregular final components refuse, and the opened handle is
    /// checked again. These checks are NOT a race-free filesystem sandbox: the
    /// operator must protect all path components and source bytes during startup.
    /// A complete cross-file set is not claimed to be an atomic filesystem snapshot.
    pub fn from_llama_files(request: NativeFileBootstrap<'_>, assets: &mut NativeAssetReadBudget,
        weights: &mut WeightReadBudget) -> Result<(Self, PretrainedReceipt), NativeFileBootstrapError>
    {
        Self::from_llama_files_with_tokenizer_format(
            request, assets, weights, NativeTokenizerFormat::NativeArchive)
    }

    /// Explicit parser selection over the SAME bounded regular-file reads. The
    /// existing per-file and aggregate asset limits still apply to JSON; choosing
    /// JSON does not enlarge them. All bytes are read through the caller's
    /// original budget and true EOF checks before tokenizer admission. No file
    /// extension sniffing, native-to-JSON fallback or converted temp file exists.
    pub fn from_llama_files_with_tokenizer_format(request: NativeFileBootstrap<'_>,
        assets: &mut NativeAssetReadBudget, weights: &mut WeightReadBudget,
        format: NativeTokenizerFormat) -> Result<(Self, PretrainedReceipt), NativeFileBootstrapError>
    {
        request.limits.check()?;
        if request.stream == 0 { return Err(NativeFileBootstrapError::Contract(Error::InvalidInput)); }
        let NativeHelperFiles { configuration, tokenizer, monitoring, sampling, weights: weight_path } = request.files;
        let configuration = read_asset(configuration, request.limits.configuration_bytes, NativeAsset::Configuration, assets)?;
        let tokenizer = read_asset(tokenizer, request.limits.tokenizer_bytes, NativeAsset::Tokenizer, assets)?;
        let monitoring = read_asset(monitoring, request.limits.monitoring_bytes, NativeAsset::Monitoring, assets)?;
        let sampling = read_asset(sampling, request.limits.sampling_bytes, NativeAsset::Sampling, assets)?;
        let bootstrap = NativeHelperBootstrap { policy: request.policy, stream: request.stream,
            configuration: &configuration, tokenizer: &tokenizer, monitoring: &monitoring, sampling: &sampling };
        let tokenizer = bootstrap.preflight(format).map_err(NativeFileBootstrapError::Bootstrap)?;
        let profile = &request.policy.decoder_profile;
        let model_bound = 8 + MAX_WEIGHT_HEADER_BYTES + 4 * profile.parameter_count();
        let bound = request.limits.weight_bytes.min(model_bound);
        let (source, _) = open_regular(weight_path, bound, NativeAsset::Weights)?;
        // Never turn a byte ceiling into apparent EOF at a valid model boundary:
        // the original loader still sees an extra byte and rejects trailing data.
        let mut source = source.take(bound as u64 + 1);
        let (model, receipt) = super::DecoderModel::read_llama_safetensors(profile.identity(),
            profile.shape().context, &configuration, &mut source, weights)
            .map_err(|error| NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Checkpoint(error)))?;
        let evaluator = bootstrap.finish(model, tokenizer).map_err(NativeFileBootstrapError::Bootstrap)?;
        Ok((evaluator, receipt))
    }
}

fn io_error(asset: NativeAsset, stage: NativeFileStage, error: io::Error) -> NativeFileBootstrapError {
    NativeFileBootstrapError::Io { asset, stage, kind: error.kind() }
}
fn open_regular(path: &Path, maximum: usize, asset: NativeAsset)
    -> Result<(File, usize), NativeFileBootstrapError>
{
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(asset, NativeFileStage::Metadata, error))?;
    if !metadata.file_type().is_file() { return Err(NativeFileBootstrapError::NotRegular(asset)); }
    if metadata.len() > maximum as u64 { return Err(NativeFileBootstrapError::Limit(asset)); }
    let file = File::open(path).map_err(|error| io_error(asset, NativeFileStage::Open, error))?;
    let metadata = file.metadata().map_err(|error| io_error(asset, NativeFileStage::Metadata, error))?;
    if !metadata.is_file() { return Err(NativeFileBootstrapError::NotRegular(asset)); }
    if metadata.len() > maximum as u64 { return Err(NativeFileBootstrapError::Limit(asset)); }
    Ok((file, metadata.len() as usize))
}
fn read_asset(path: &Path, maximum: usize, asset: NativeAsset, budget: &mut NativeAssetReadBudget)
    -> Result<Vec<u8>, NativeFileBootstrapError>
{
    let (mut file, size) = open_regular(path, maximum, asset)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|_| NativeFileBootstrapError::Limit(asset))?;
    read_bytes(&mut file, &mut bytes, maximum, asset, budget)?;
    Ok(bytes)
}
fn read_bytes<R: Read>(source: &mut R, bytes: &mut Vec<u8>, maximum: usize, asset: NativeAsset,
    budget: &mut NativeAssetReadBudget) -> Result<(), NativeFileBootstrapError>
{
    let mut buffer = [0; READ_CHUNK];
    loop {
        let offered = READ_CHUNK.min(maximum - bytes.len() + 1);
        let count = match budget.read(source, &mut buffer[..offered], asset) {
            Ok(0) => return Ok(()),
            Ok(count) => count,
            Err(NativeFileBootstrapError::Io { kind: io::ErrorKind::Interrupted, .. }) => continue,
            Err(error) => return Err(error),
        };
        if count > maximum - bytes.len() { return Err(NativeFileBootstrapError::Limit(asset)); }
        bytes.try_reserve(count).map_err(|_| NativeFileBootstrapError::Limit(asset))?;
        bytes.extend_from_slice(&buffer[..count]);
    }
}

#[cfg(unix)]
mod peer;
#[cfg(unix)]
pub use peer::NativeFilePeerError;
#[cfg(test)]
mod tests;
