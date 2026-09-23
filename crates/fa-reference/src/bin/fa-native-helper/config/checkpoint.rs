//! Operator manifest projection into the original checkpoint owners. No paths
//! are derived from an index, request, model name or tokenizer filename.
use super::{LaunchError, Manifest, NativeTokenizerFormat, Json, object, path};
use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluator;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::{
    NativeAssetReadBudget, NativeFileBootstrap, NativeHelperFiles, NativeHelperFileLimits,
};
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::sharded::{
    NativeHelperShardFiles, NativeShardFileBootstrap,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadBudget;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::shards::{
    MAX_WEIGHT_INDEX_BYTES, MAX_WEIGHT_SHARDS,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) enum CheckpointFiles {
    Single(PathBuf),
    Sharded { index: PathBuf, shards: BTreeMap<String, PathBuf> },
}
impl CheckpointFiles {
    pub(super) fn parse(value: &Json) -> Result<Self, LaunchError> {
        match value.get("kind").and_then(Json::as_str) {
            Some("single") => {
                let values = object(value, &["kind", "path"], "weights")?;
                Ok(Self::Single(path(&values["path"], "weights.path")?))
            }
            Some("sharded") => {
                let values = object(value, &["kind", "index", "shards"], "weights")?;
                let declared = values["shards"].as_object().ok_or(LaunchError::Field("weights.shards"))?;
                if declared.is_empty() { return Err(LaunchError::Field("weights.shards")); }
                if declared.len() > MAX_WEIGHT_SHARDS { return Err(LaunchError::Limit); }
                let mut shards = BTreeMap::new();
                for (label, value) in declared {
                    if label.is_empty() { return Err(LaunchError::Field("weights.shards")); }
                    if label.len() > 256 { return Err(LaunchError::Limit); }
                    // These are labels, not paths. The original index parser
                    // later enforces their syntax and exact complete inventory
                    // before any weight path can open. Do not duplicate it here.
                    shards.insert(label.clone(), path(value, "weights.shards.path")?);
                }
                Ok(Self::Sharded { index: path(&values["index"], "weights.index")?, shards })
            }
            _ => Err(LaunchError::Field("weights.kind")),
        }
    }
}

pub(super) fn tokenizer_format(value: &Json) -> Result<NativeTokenizerFormat, LaunchError> {
    match value.as_str() {
        Some("native-archive") => Ok(NativeTokenizerFormat::NativeArchive),
        Some("huggingface-raw-bytelevel") => Ok(NativeTokenizerFormat::HuggingFaceRawByteLevel),
        _ => Err(LaunchError::Field("tokenizer_format")),
    }
}

impl Manifest {
    pub(super) fn asset_budget(&self) -> Result<NativeAssetReadBudget, LaunchError> {
        match &self.checkpoint {
            CheckpointFiles::Single(_) => NativeAssetReadBudget::new(self.asset_bytes, self.asset_calls),
            CheckpointFiles::Sharded { .. } => NativeAssetReadBudget::for_shards(self.asset_bytes, self.asset_calls),
        }.map_err(LaunchError::Contract)
    }

    /// Both legacy and version-two manifests use these same admitted loaders.
    /// The caller retains spent budgets on failure. The returned evaluator is
    /// fresh and has performed no inference; only the original worker consumes it.
    pub(super) fn load(&self, assets: &mut NativeAssetReadBudget, weights: &mut WeightReadBudget)
        -> Result<NativeEvaluator, LaunchError>
    {
        match &self.checkpoint {
            CheckpointFiles::Single(weight_path) => {
                let request = NativeFileBootstrap { policy: &self.policy, stream: self.stream,
                    files: NativeHelperFiles { configuration: &self.files[0], tokenizer: &self.files[1],
                        monitoring: &self.files[2], sampling: &self.files[3], weights: weight_path },
                    limits: NativeHelperFileLimits::default() };
                NativeEvaluator::from_llama_files_with_tokenizer_format(request, assets, weights, self.tokenizer_format)
                    .map(|(evaluator, _)| evaluator).map_err(LaunchError::Startup)
            }
            CheckpointFiles::Sharded { index, shards } => {
                let request = NativeShardFileBootstrap { policy: &self.policy, stream: self.stream,
                    files: NativeHelperShardFiles { configuration: &self.files[0], tokenizer: &self.files[1],
                        monitoring: &self.files[2], sampling: &self.files[3], index, shards },
                    limits: NativeHelperFileLimits::default(), index_bytes: MAX_WEIGHT_INDEX_BYTES };
                NativeEvaluator::from_llama_shard_files_with_tokenizer_format(request, assets, weights, self.tokenizer_format)
                    .map(|(evaluator, _)| evaluator).map_err(LaunchError::ShardedStartup)
            }
        }
    }
}
