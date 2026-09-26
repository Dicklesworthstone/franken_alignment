//! Explicit operator-selected sources. The index assigns tensors to LABELS;
//! it never supplies paths, a glob, a download or a single-file fallback.
use super::{BTreeMap, DecoderBindingLimits, Fields, FileDecoderConfig, Json, Path, PathBuf, debug, read_regular};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderProfile;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{
    OutputHead, MAX_WEIGHT_FILE_BYTES, shards::{MAX_WEIGHT_INDEX_BYTES, MAX_WEIGHT_SET_BYTES, MAX_WEIGHT_SHARDS},
};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderShardInputs;

pub(super) enum WeightFiles {
    Single(PathBuf),
    Sharded { index: PathBuf, sources: BTreeMap<String, PathBuf> },
}
pub(super) enum LoadedWeights { Single(Vec<u8>), Sharded(FileDecoderShardInputs) }

impl WeightFiles {
    pub(super) fn parse(value: Json, base: &Path, sharded: bool) -> Result<Self, String> {
        if !sharded { return Ok(Self::Single(selected_path(value, base)?)); }
        let mut fields = Fields::new(value)?;
        let index = selected_path(fields.take("index")?, base)?;
        let raw = fields.take("shards")?;
        fields.end()?;
        let Json::Object(raw) = raw else { return Err("shards must be an explicit label/path object".into()); };
        if raw.is_empty() || raw.len() > MAX_WEIGHT_SHARDS { return Err("invalid native shard count".into()); }
        let mut sources = BTreeMap::new();
        for (label, value) in raw {
            if label.len() > 256 { return Err("native shard label exceeds its bound".into()); }
            sources.insert(label, selected_path(value, base)?);
        }
        Ok(Self::Sharded { index, sources })
    }

    pub(super) fn read(&self, profile: &DecoderProfile, head: OutputHead,
        remaining: &mut usize) -> Result<LoadedWeights, String>
    {
        match self {
            Self::Single(path) => read_charged(path, MAX_WEIGHT_FILE_BYTES, remaining).map(LoadedWeights::Single),
            Self::Sharded { index, sources } => {
                let index = read_charged(index, MAX_WEIGHT_INDEX_BYTES, remaining)?;
                // Complete semantic index admission BEFORE opening ANY shard.
                // Read only paths from the operator's map, never the index text.
                FileDecoderShardInputs::check_labels(profile, &index,
                    sources.keys().map(String::as_str), head)
                    .map_err(|error| format!("native shard index/labels refused: {error:?}"))?;
                let mut available = MAX_WEIGHT_SET_BYTES;
                let mut loaded = BTreeMap::new();
                for (label, path) in sources {
                    let bytes = read_charged(path, available, remaining)?;
                    available = available.checked_sub(bytes.len()).ok_or("native shard set exceeds its bound")?;
                    loaded.insert(label.clone(), bytes);
                }
                Ok(LoadedWeights::Sharded(FileDecoderShardInputs { index, sources: loaded }))
            }
        }
    }
}
impl LoadedWeights {
    pub(super) fn configure(self, profile: DecoderProfile, monitor: Vec<u8>, sampling: Vec<u8>,
        stream: u64, limits: DecoderBindingLimits, head: OutputHead) -> Result<FileDecoderConfig, String>
    {
        match self {
            Self::Single(bytes) => FileDecoderConfig::new_with_output_head(profile, bytes, monitor, sampling, stream, limits, head),
            Self::Sharded(inputs) => FileDecoderConfig::new_sharded(profile, inputs, monitor, sampling, stream, limits, head),
        }.map_err(debug)
    }
}
fn selected_path(value: Json, base: &Path) -> Result<PathBuf, String> {
    let text = value.as_str().ok_or("native weight path must be text")?;
    if text.is_empty() || text.as_bytes().contains(&0) { return Err("invalid native weight path".into()); }
    let path = PathBuf::from(text);
    Ok(if path.is_absolute() { path } else { base.join(path) })
}
fn read_charged(path: &Path, limit: usize, remaining: &mut usize) -> Result<Vec<u8>, String> {
    let bytes = read_regular(path, limit.min(*remaining))?;
    *remaining = remaining.checked_sub(bytes.len()).ok_or("native input allowance exhausted")?;
    Ok(bytes)
}
