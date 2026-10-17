//! Explicit regular-file inputs, original bounded readers, exclusive outputs.
use super::{Failure, Plan, contract, fit, invalid};
use super::checkpoint_source::{CheckpointSource, MAX_MANIFEST_BYTES};
use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::{ByteBpe, PreTokenization},
    tensor::kv::{decoder::{DecoderModel,
        safetensors::{MAX_WEIGHT_HEADER_BYTES,
            pretrained::{LlamaConfig, MAX_CONFIG_BYTES}, reader::WeightReadBudget}},
        model::learned::replay::archive::{LearnedKvFitArchive, LearnedKvFitBinding,
            reader::FitArchiveReadBudget}}};
use fa_reference::action::consequence::{
    delivery::persistent::observed::decoder::text::MAX_FILE_TOKENIZER_BYTES,
    oversight::helper_client::native::bootstrap::NativeTokenizerFormat};
use fa_reference::Error;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub(super) struct InputWork {
    pub auxiliary_bytes: usize,
    pub weight_bytes: usize,
    pub weight_read_calls: usize,
    pub codec_bytes: usize,
    pub codec_read_calls: usize,
    pub checkpoint_source: bool,
    pub base_files: usize,
    pub adapter_parameters: usize,
    pub updated_parameters: usize,
    pub merge_products: u64,
}
pub(super) struct Inputs {
    pub model: DecoderModel,
    pub tokenizer: ByteBpe,
    pub binding: LearnedKvFitBinding,
    pub archive: LearnedKvFitArchive,
    pub work: InputWork,
}

#[cfg(test)]
pub(super) fn load(args: &[OsString]) -> Result<(Plan, Inputs), Failure> {
    load_with_source(args, false)
}

pub(super) fn load_with_source(args: &[OsString], source: bool) -> Result<(Plan, Inputs), Failure> {
    let plan_bytes = read_regular(Path::new(&args[3]), super::plan::MAX_PLAN_BYTES).map_err(invalid)?;
    let plan = Plan::decode(&plan_bytes).map_err(invalid)?;
    let mut remaining = plan.input_limits.auxiliary_bytes.checked_sub(plan_bytes.len())
        .ok_or_else(|| invalid("aggregate auxiliary input allowance exhausted by campaign"))?;
    let mut auxiliary = |path: &Path, maximum: usize| -> Result<Vec<u8>, String> {
        let bytes = read_regular(path, maximum.min(remaining))?;
        remaining = remaining.checked_sub(bytes.len()).ok_or("aggregate auxiliary input allowance exhausted")?;
        Ok(bytes)
    };
    let binding = fit::binding(&auxiliary(Path::new(&args[4]), super::fields::MAX_DATA_BYTES)?).map_err(invalid)?;
    let configuration = auxiliary(Path::new(&args[0]), MAX_CONFIG_BYTES)?;
    let negotiated = LlamaConfig::decode(plan.identity, plan.context, &configuration)?;
    // Every planned numerical term and final score is affordable before weights
    // open. No placeholder model or unobserved source frames establish this.
    plan.estimate(negotiated.profile()).map_err(contract)?;
    let maximum = match plan.tokenizer_format {
        NativeTokenizerFormat::NativeArchive => MAX_FILE_TOKENIZER_BYTES,
        NativeTokenizerFormat::HuggingFaceRawByteLevel
            | NativeTokenizerFormat::HuggingFaceGpt2Unicode17
            | NativeTokenizerFormat::HuggingFaceLlama3Unicode17
            | NativeTokenizerFormat::HuggingFaceLlama3Unicode17Bos => ByteBpe::MAX_HUGGINGFACE_JSON_BYTES,
    };
    let tokenizer_bytes = auxiliary(Path::new(&args[2]), maximum)?;
    let tokenizer = match plan.tokenizer_format {
        NativeTokenizerFormat::NativeArchive => ByteBpe::from_bytes(negotiated.profile(), &tokenizer_bytes),
        NativeTokenizerFormat::HuggingFaceRawByteLevel => ByteBpe::from_huggingface_json(
            negotiated.profile(), &tokenizer_bytes, maximum),
        NativeTokenizerFormat::HuggingFaceGpt2Unicode17 => ByteBpe::from_huggingface_json_with_pre_tokenization(
            negotiated.profile(), &tokenizer_bytes, maximum, PreTokenization::Gpt2Unicode17),
        NativeTokenizerFormat::HuggingFaceLlama3Unicode17 => ByteBpe::from_huggingface_json_with_pre_tokenization(
            negotiated.profile(), &tokenizer_bytes, maximum, PreTokenization::Llama3Unicode17),
        NativeTokenizerFormat::HuggingFaceLlama3Unicode17Bos => ByteBpe::from_huggingface_llama3_bos_json(
            negotiated.profile(), &tokenizer_bytes, maximum),
    }.map_err(contract)?;
    // The independently retained codec source inventory must describe the exact
    // original cache profile and be disjoint from every declared probe origin.
    // This metadata is available before any checkpoint tensors are opened.
    let cache_profile = negotiated.profile().cache_profile().map_err(contract)?;
    for (lineage, descriptor) in &binding.sources {
        if descriptor.profile() != &cache_profile { return Err(contract(Error::Binding).into()); }
        if plan.cases.iter().any(|case| case.origin.lineage == *lineage
            || descriptor.layers().values().any(|source| source.stream == case.origin.task)) {
            return Err(contract(Error::Duplicate).into());
        }
    }
    let (model, mut work) = if source {
        let manifest = auxiliary(Path::new(&args[1]), MAX_MANIFEST_BYTES).map_err(invalid)?;
        let source = CheckpointSource::decode(&manifest).map_err(invalid)?;
        let prepared = source.prepare(plan.identity, plan.context, &configuration, &mut auxiliary).map_err(invalid)?;
        if prepared.profile() != negotiated.profile() { return Err(contract(Error::Binding).into()); }
        let physical = plan.input_limits.weight_bytes.min(prepared.weight_bytes_limit());
        let calls = plan.input_limits.weight_read_calls.min(prepared.weight_read_calls_limit());
        let mut budget = WeightReadBudget::new(physical + 1, calls)?;
        let loaded = prepared.load(&mut budget, physical)?;
        let work = loaded.work;
        (loaded.model, InputWork { auxiliary_bytes: 0, weight_bytes: work.weight_bytes,
            weight_read_calls: work.weight_read_calls, codec_bytes: 0, codec_read_calls: 0,
            checkpoint_source: true, base_files: work.base_files,
            adapter_parameters: work.adapter_parameters, updated_parameters: work.updated_parameters,
            merge_products: work.merge_products })
    } else {
        let weight_bound = plan.input_limits.weight_bytes.min(8 + MAX_WEIGHT_HEADER_BYTES + negotiated.profile().parameter_count() * 4);
        let mut weight_reader = open_regular(Path::new(&args[1]), weight_bound)?;
        let mut weight_budget = WeightReadBudget::new(weight_bound + 1, plan.input_limits.weight_read_calls)?;
        let (model, receipt) = DecoderModel::read_llama_safetensors(plan.identity, plan.context,
            &configuration, &mut weight_reader, &mut weight_budget)?;
        if receipt.configuration != negotiated { return Err(contract(Error::Binding).into()); }
        (model, InputWork { auxiliary_bytes: 0, weight_bytes: weight_budget.usage().bytes_read,
            weight_read_calls: weight_budget.usage().read_calls, codec_bytes: 0, codec_read_calls: 0,
            checkpoint_source: false, base_files: 1, adapter_parameters: 0,
            updated_parameters: 0, merge_products: 0 })
    };
    if model.profile() != negotiated.profile() || model.cache_profile() != &cache_profile { return Err(contract(Error::Binding).into()); }
    let mut codec_reader = open_regular(Path::new(&args[5]), plan.input_limits.codec_bytes)?;
    let mut codec_budget = FitArchiveReadBudget::new(plan.input_limits.codec_bytes + 1, plan.input_limits.codec_read_calls).map_err(contract)?;
    let archive = LearnedKvFitArchive::read(&mut codec_reader, &binding, &mut codec_budget)?;
    work.auxiliary_bytes = plan.input_limits.auxiliary_bytes - remaining;
    work.codec_bytes = codec_budget.consumed_bytes();
    work.codec_read_calls = codec_budget.used_calls();
    Ok((plan, Inputs { model, tokenizer, binding, archive, work }))
}

/// Operator-controlled paths and parents are a required assumption. Metadata
/// checks reject observed links/devices; they do not promise hostile-path races
/// are impossible. Every successful read has the complete regular-file tail.
pub(crate) fn read_regular(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let mut file = open_regular(path, maximum).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let size = usize::try_from(file.metadata().map_err(|e| e.to_string())?.len()).map_err(|e| e.to_string())?;
    bytes.try_reserve_exact(size.min(maximum)).map_err(|e| e.to_string())?;
    let mut block = [0_u8; 8192];
    for _ in 0..65_536 {
        let capacity = block.len().min(maximum.saturating_sub(bytes.len()) + 1);
        match file.read(&mut block[..capacity]) {
            Ok(0) => return Ok(bytes),
            Ok(count) => {
                if count > maximum.saturating_sub(bytes.len()) { return Err("auxiliary input exceeds its byte allowance".into()); }
                bytes.try_reserve(count).map_err(|e| e.to_string())?;
                bytes.extend_from_slice(&block[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {},
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("auxiliary input read-call allowance exhausted".into())
}
fn open_regular(path: &Path, maximum: usize) -> io::Result<File> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.file_type().is_symlink() { return Err(invalid("input must be a regular non-symlink file")); }
    if before.len() > maximum as u64 { return Err(invalid("input exceeds its byte allowance")); }
    let file = File::open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() > maximum as u64 { return Err(invalid("opened input violates regular-file byte allowance")); }
    Ok(file)
}

pub(super) fn require_absent(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::new(io::ErrorKind::AlreadyExists, "output already exists; choose a new path")),
    }
}
fn create_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new(); options.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
pub(super) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The complete diagnostic stream is a requested product of the offline
/// experiment. Exclusive creation and byte caps preserve failures/retries.
/// A write/sync failure can leave a partial file; it is never removed or success.
pub(super) struct SavedReport { file: File, bytes: usize, maximum: usize }
impl SavedReport {
    pub fn new(path: &Path, maximum: usize) -> io::Result<Self> {
        Ok(Self { file: create_new(path)?, bytes: 0, maximum })
    }
    pub fn bytes(&self) -> usize { self.bytes }
    pub fn sync(&mut self) -> io::Result<()> { self.flush()?; self.file.sync_all() }
}
impl Write for SavedReport {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes) { return Err(invalid("diagnostic output byte allowance exhausted")); }
        let count = self.file.write(bytes)?;
        self.bytes = self.bytes.checked_add(count).ok_or_else(|| invalid("diagnostic byte count overflow"))?;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { self.file.flush() }
}
