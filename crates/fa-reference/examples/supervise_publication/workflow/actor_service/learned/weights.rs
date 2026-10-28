//! Explicit file/source selection for the original learned actor and helpers.
#[path = "../../../../support/checkpoint_source.rs"]
mod checkpoint_source;

use super::{Assets, Fields};
use super::super::fields::path;
use checkpoint_source::{CheckpointSource, MAX_MANIFEST_BYTES};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderModel, safetensors::{pretrained::LlamaConfig, reader::WeightReadBudget},
};
use fa_reference::strict_json::Json;
use std::path::PathBuf;

pub(in super::super) enum WeightsInput { Single(PathBuf), Source(PathBuf) }
impl WeightsInput {
    pub(in super::super) fn parse(value: Json, source_enabled: bool) -> Result<Self, String> {
        if let Some(value) = value.as_str() { return Ok(Self::Single(path(value)?)); }
        if !source_enabled { return Err("this recipe version requires a single weight path".into()); }
        let mut fields = Fields::new(value)?;
        if fields.text("format")? != "checkpoint_source" {
            return Err("unsupported explicit learned checkpoint format".into());
        }
        let path = fields.path("path")?;
        fields.end()?;
        Ok(Self::Source(path))
    }
    pub(in super::super) fn load(&self, configuration: &[u8], negotiated: &LlamaConfig,
        assets: &mut Assets, ceiling: usize) -> Result<DecoderModel, String>
    {
        match self {
            Self::Single(path) => {
                let weights = assets.read(path, ceiling)?;
                DecoderModel::from_llama_safetensors(negotiated.profile().identity(),
                    negotiated.profile().shape().context, configuration, &weights)
                    .map(|(model, _)| model).map_err(|error| format!("{error:?}"))
            }
            Self::Source(path) => {
                let source = CheckpointSource::decode(&assets.read(path, MAX_MANIFEST_BYTES)?)?;
                let prepared = source.prepare(negotiated.profile().identity(),
                    negotiated.profile().shape().context, configuration,
                    |path, limit| assets.read(path, limit))?;
                if prepared.profile() != negotiated.profile() {
                    return Err("checkpoint source changed the negotiated final profile".into());
                }
                let physical = ceiling.min(prepared.weight_bytes_limit()).min(assets.remaining());
                if physical == 0 { return Err("no learned asset allowance remains for checkpoint weights".into()); }
                let mut reads = WeightReadBudget::new(physical + 1, prepared.weight_read_calls_limit())
                    .map_err(|error| format!("{error:?}"))?;
                let result = prepared.load_with_path_admission(&mut reads, physical,
                    |path| assets.admit_path(path));
                let charged = assets.charge(reads.usage().bytes_read);
                match result {
                    Ok(loaded) => { charged?; Ok(loaded.model) }
                    Err(error) => match charged {
                        Ok(()) => Err(error.to_string()),
                        Err(charge) => Err(format!("{error}; {charge}")),
                    },
                }
            }
        }
    }
}
