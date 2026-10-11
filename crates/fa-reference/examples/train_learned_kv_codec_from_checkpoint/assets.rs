//! Explicit original checkpoint readers and exclusive bounded output files.
use super::{Failure, Plan, contract, invalid};
use super::plan::Estimate;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderModel,
    safetensors::{MAX_WEIGHT_HEADER_BYTES, pretrained::{LlamaConfig, MAX_CONFIG_BYTES},
        reader::WeightReadBudget}};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub(super) struct InputWork { pub auxiliary_bytes: usize, pub weight_bytes: usize, pub weight_read_calls: usize }
pub(super) struct Inputs { pub model: DecoderModel, pub estimate: Estimate, pub work: InputWork }

pub(super) fn load(args: &[OsString]) -> Result<(Plan, Inputs), Failure> {
    let bytes = read_regular(Path::new(&args[2]), super::plan::MAX_PLAN_BYTES).map_err(invalid)?;
    let plan = Plan::decode(&bytes).map_err(invalid)?;
    let remaining = plan.inputs.auxiliary_bytes.checked_sub(bytes.len())
        .ok_or_else(|| invalid("aggregate auxiliary allowance exhausted by training plan"))?;
    let configuration = read_regular(Path::new(&args[0]), remaining.min(MAX_CONFIG_BYTES)).map_err(invalid)?;
    let negotiated = LlamaConfig::decode(plan.identity, plan.context, &configuration)?;
    // Original inference AND fitting admission, complete source retention and
    // conservative output reservations all precede the first weight-file open.
    let estimate = plan.estimate(negotiated.profile()).map_err(contract)?;
    let weight_bound = plan.inputs.weight_bytes.min(8 + MAX_WEIGHT_HEADER_BYTES + negotiated.profile().parameter_count() * 4);
    let mut reader = open_regular(Path::new(&args[1]), weight_bound)?;
    let mut budget = WeightReadBudget::new(weight_bound + 1, plan.inputs.weight_read_calls)?;
    let (model, receipt) = DecoderModel::read_llama_safetensors(plan.identity, plan.context,
        &configuration, &mut reader, &mut budget)?;
    if model.profile() != negotiated.profile() || receipt.configuration != negotiated
        || model.cache_profile() != &negotiated.profile().cache_profile().map_err(contract)? {
        return Err(contract(fa_reference::Error::Binding).into());
    }
    let work = InputWork { auxiliary_bytes: bytes.len() + configuration.len(),
        weight_bytes: budget.usage().bytes_read, weight_read_calls: budget.usage().read_calls };
    Ok((plan, Inputs { model, estimate, work }))
}

/// Metadata checks reject observed links/devices. Paths and ancestors remain
/// immutable operator-controlled inputs; this is not hostile-path authentication.
pub(crate) fn read_regular(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let mut file = open_regular(path, maximum).map_err(|e| e.to_string())?;
    let size = usize::try_from(file.metadata().map_err(|e| e.to_string())?.len()).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new(); bytes.try_reserve_exact(size).map_err(|e| e.to_string())?;
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

pub(super) struct OutputPaths { pub binding: PathBuf, pub archive: PathBuf, pub report: PathBuf }
impl OutputPaths {
    pub fn new(binding: &Path, archive: &Path, report: &Path) -> io::Result<Self> {
        if binding == archive || binding == report || archive == report {
            return Err(invalid("binding, archive and report need distinct new paths"));
        }
        Ok(Self { binding: binding.to_owned(), archive: archive.to_owned(), report: report.to_owned() })
    }
    pub fn require_absent(&self) -> io::Result<()> {
        for path in [&self.binding, &self.archive, &self.report] {
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                Err(error) => return Err(error),
                Ok(_) => return Err(io::Error::new(io::ErrorKind::AlreadyExists, "output already exists; choose a new path")),
            }
        }
        Ok(())
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
    let mut file = create_new(path)?; file.write_all(bytes)?; file.sync_all()
}

pub(super) struct SavedReport { file: File, bytes: usize, maximum: usize }
impl SavedReport {
    pub fn new(path: &Path, maximum: usize) -> io::Result<Self> {
        Ok(Self { file: create_new(path)?, bytes: 0, maximum })
    }
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
