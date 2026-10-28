//! All explicit handles precede scalar reads; one original budget spans them.
use super::{CheckpointLoadError, CheckpointWork, DecoderModel, LoadedCheckpoint,
    PreparedBase, PreparedCheckpoint, WeightReadBudget, debug};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{
    MAX_WEIGHT_HEADER_BYTES, lora::MAX_LORA_WEIGHT_BYTES,
};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::Path;

impl PreparedCheckpoint {
    pub(crate) fn load(self, reads: &mut WeightReadBudget, physical_limit: usize)
        -> Result<LoadedCheckpoint, CheckpointLoadError>
    {
        self.load_with_path_admission(reads, physical_limit, |_| Ok(()))
    }

    /// Apply a caller's independent custody boundary to every explicit weight
    /// handle, including nested shards and the static adapter, before opening it.
    /// Existing numerical readers and cumulative physical-read accounting remain
    /// the sole owners of checkpoint ingestion.
    pub(crate) fn load_with_path_admission(mut self, reads: &mut WeightReadBudget,
        physical_limit: usize, mut admit: impl FnMut(&Path) -> Result<(), String>)
        -> Result<LoadedCheckpoint, CheckpointLoadError>
    {
        let before = reads.usage();
        let mut work = CheckpointWork::default();
        let result = (|| -> Result<DecoderModel, String> {
            let limit = physical_limit.min(self.weight_bytes);
            if limit == 0 || reads.remaining_bytes() > limit + 1
                || reads.remaining_calls() > self.weight_read_calls {
                return Err("caller must conjoin checkpoint and existing read/byte allowances".into());
            }
            let mut total = 0_usize;
            let mut single = None;
            let mut shards = BTreeMap::new();
            match &self.base {
                PreparedBase::Single(path) => {
                    admit(path)?;
                    single = Some(open(path, limit, &mut total)?);
                    work.base_files = 1;
                }
                PreparedBase::Sharded { shards: paths, .. } => {
                    for (label, path) in paths {
                        admit(path)?;
                        shards.insert(label.clone(), open(path, limit, &mut total)?);
                        work.base_files += 1;
                    }
                }
            }
            let mut adapter_file = None;
            if let Some(adapter) = &self.adapter {
                admit(&adapter.weights)?;
                let file = open(&adapter.weights, limit, &mut total)?;
                let bound = (8 + MAX_WEIGHT_HEADER_BYTES + 4 * adapter.work.adapter_parameters)
                    .min(MAX_LORA_WEIGHT_BYTES);
                if file.metadata().map_err(debug)?.len() > bound as u64 {
                    return Err("adapter file exceeds its original parameter/header bound".into());
                }
                adapter_file = Some(file);
            }
            // All explicit files have passed type and aggregate-size admission.
            // The original sharded reader checks every base directory before any
            // base scalar. The original adapter header is consumed after loading
            // the base, not as a fictional all-file atomic snapshot.
            let model = match &self.base {
                PreparedBase::Single(_) => {
                    let (model, receipt) = DecoderModel::read_llama_safetensors(
                        self.base_configuration.profile().identity(), self.profile.shape().context,
                        &self.configuration, single.as_mut().ok_or("missing admitted base handle")?, reads)
                        .map_err(debug)?;
                    if receipt.configuration != self.base_configuration {
                        return Err("base configuration changed during original import".into());
                    }
                    model
                }
                PreparedBase::Sharded { index, .. } => DecoderModel::read_safetensors_shards_bounded(
                    self.base_configuration.profile().clone(), index, &mut shards, reads, limit,
                    self.base_configuration.output_head()).map_err(debug)?.0,
            };
            let model = if let Some(adapter) = &mut self.adapter {
                model.read_lora_safetensors(self.profile.identity(), &adapter.configuration,
                    adapter_file.as_mut().ok_or("missing admitted adapter handle")?, reads, &mut adapter.products)
                    .map_err(debug)?.0
            } else { model };
            if model.profile() != &self.profile || reads.usage().bytes_read - before.bytes_read > limit {
                return Err("effective checkpoint profile or aggregate actual bytes changed".into());
            }
            Ok(model)
        })();
        let after = reads.usage();
        work.weight_bytes = after.bytes_read - before.bytes_read;
        work.weight_read_calls = after.read_calls - before.read_calls;
        if let Some(adapter) = &self.adapter {
            work.merge_products = adapter.products.reserved_products();
            if work.merge_products != 0 {
                work.adapter_parameters = adapter.work.adapter_parameters;
                work.updated_parameters = adapter.work.updated_parameters;
            }
        }
        match result {
            Ok(model) => Ok(LoadedCheckpoint { model, work }),
            Err(reason) => Err(CheckpointLoadError { reason, work }),
        }
    }
}

fn open(path: &Path, maximum: usize, total: &mut usize) -> Result<File, String> {
    // Inputs and every ancestor must remain operator controlled. These observed
    // final-component checks are not a hostile-filesystem race guarantee.
    let metadata = fs::symlink_metadata(path).map_err(debug)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > maximum as u64 {
        return Err("checkpoint source must be a bounded regular non-symlink file".into());
    }
    let file = File::open(path).map_err(debug)?;
    let opened = file.metadata().map_err(debug)?;
    let size = usize::try_from(opened.len()).map_err(debug)?;
    let next = total.checked_add(size).ok_or("checkpoint aggregate byte count overflow")?;
    if !opened.is_file() || next > maximum {
        return Err("checkpoint files exceed one aggregate byte allowance".into());
    }
    *total = next;
    Ok(file)
}
