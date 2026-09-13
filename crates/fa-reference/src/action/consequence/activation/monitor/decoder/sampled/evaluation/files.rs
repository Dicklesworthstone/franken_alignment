//! Explicit local checkpoint evaluation and no-overwrite monitor publication.
//! No paths are read from plan data. These are operator-controlled regular files.
use super::{RolloutBuildError, RolloutReport};
use super::plan::{RolloutPlan, RolloutPlanError, MAX_ROLLOUT_PLAN_BYTES};
use crate::action::consequence::activation::monitor::decoder::config::MAX_MONITOR_CONFIG_BYTES;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::{
    CheckpointError, CheckpointFileLimits,
};
use crate::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Debug)]
pub enum RolloutFileError {
    Plan(RolloutPlanError), Build(RolloutBuildError), Checkpoint(CheckpointError), Contract(Error),
    Io { operation: &'static str, kind: io::ErrorKind },
}
impl fmt::Display for RolloutFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for RolloutFileError {}
fn io_error(operation: &'static str, error: io::Error) -> RolloutFileError {
    RolloutFileError::Io { operation, kind: error.kind() }
}
fn read_regular(path: &Path, maximum: usize, input: &'static str) -> Result<Vec<u8>, RolloutFileError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(input, error))?;
    if !metadata.file_type().is_file() { return Err(io_error(input, io::ErrorKind::InvalidInput.into())); }
    if metadata.len() > maximum as u64 { return Err(RolloutFileError::Contract(Error::Limit)); }
    let mut file = File::open(path).map_err(|error| io_error(input, error))?;
    let metadata = file.metadata().map_err(|error| io_error(input, error))?;
    if !metadata.is_file() || metadata.len() > maximum as u64 { return Err(RolloutFileError::Contract(Error::Limit)); }
    let mut bytes = Vec::new(); let mut scratch = [0_u8; 4096];
    // Interrupted calls consume this finite allowance too; byte caps alone do
    // not prevent a reader from forcing an unbounded retry loop.
    for _ in 0..4096 {
        let offered = scratch.len().min(maximum - bytes.len() + 1);
        match file.read(&mut scratch[..offered]) {
            Ok(0) => return Ok(bytes),
            Ok(count) => {
                let next = bytes.len().checked_add(count).ok_or(RolloutFileError::Contract(Error::Overflow))?;
                if next > maximum { return Err(RolloutFileError::Contract(Error::Limit)); }
                bytes.try_reserve(count).map_err(|_| RolloutFileError::Contract(Error::Limit))?;
                bytes.extend_from_slice(&scratch[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(io_error(input, error)),
        }
    }
    Err(RolloutFileError::Contract(Error::Limit))
}
fn destination_absent(path: &Path) -> Result<(), RolloutFileError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(io_error("destination", io::ErrorKind::AlreadyExists.into())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("destination", error)),
    }
}

/// Report all observations and flush BEFORE creating a file. A rejected report
/// creates no artifact. After creation, write/sync failure may leave that new file;
/// it is not removed, relabelled successful, retrained or overwritten on retry.
/// This syncs the file, not its parent directory; no crash-durability claim follows.
pub fn publish_report<W: Write + ?Sized>(report: &RolloutReport, destination: &Path,
    output: &mut W) -> Result<bool, RolloutFileError>
{
    destination_absent(destination)?;
    report.write_ndjson(output).map_err(|error| io_error("report", error))?;
    if !report.accepted() { return Ok(false); }
    let bytes = report.monitor_json().map_err(RolloutFileError::Contract)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(destination)
        .map_err(|error| io_error("create_monitor", error))?;
    file.write_all(bytes).map_err(|error| io_error("write_monitor", error))?;
    file.sync_all().map_err(|error| io_error("sync_monitor", error))?;
    Ok(true)
}

/// Five explicit paths, one frozen experiment. The existing checkpoint parser
/// and bounded tensor reader determine architecture support; there is no fallback
/// model, downloaded tokenizer, tool execution, inferred EOS or automatic approval.
pub fn evaluate_checkpoint_files<W: Write + ?Sized>(configuration: &Path, weights: &Path,
    monitor: &Path, plan: &Path, destination: &Path, output: &mut W) -> Result<bool, RolloutFileError>
{
    destination_absent(destination)?;
    let bytes = read_regular(plan, MAX_ROLLOUT_PLAN_BYTES, "plan")?;
    let plan = RolloutPlan::decode(&bytes).map_err(RolloutFileError::Plan)?;
    let monitor = read_regular(monitor, MAX_MONITOR_CONFIG_BYTES, "monitor")?;
    let (model, _) = DecoderModel::from_llama_files(plan.identity(), plan.context(), configuration,
        weights, CheckpointFileLimits::default()).map_err(RolloutFileError::Checkpoint)?;
    let mut prepared = plan.bind(model, &monitor).map_err(RolloutFileError::Build)?;
    let report = prepared.run().map_err(RolloutFileError::Contract)?;
    publish_report(&report, destination, output)
}
