//! Operator files feed the existing campaign parser and bounded weight reader.
//! No alternate request schema, estimator, trainer or serializer is introduced.
use super::super::decoder::plan::{ProbeRunPlan, PlanError, MAX_CAMPAIGN_PLAN_BYTES};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderModel;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::{CheckpointError, CheckpointFileLimits};
use crate::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct TrainingFileLimits {
    pub plan_bytes: usize,
    pub checkpoint: CheckpointFileLimits,
}
impl Default for TrainingFileLimits {
    fn default() -> Self { Self { plan_bytes: MAX_CAMPAIGN_PLAN_BYTES, checkpoint: CheckpointFileLimits::default() } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingFileStage { Metadata, Open, Read }
#[derive(Debug)]
pub enum TrainingFileError {
    Limit,
    NotRegular,
    Io { stage: TrainingFileStage, kind: io::ErrorKind },
    Plan(PlanError),
    Checkpoint(CheckpointError),
    Admission(Error),
}
impl fmt::Display for TrainingFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for TrainingFileError {}

/// Read at most limit+1 bytes, then use the ORIGINAL complete plan parser.
/// Path and ancestors must be operator-controlled immutable inputs; metadata
/// checks do not promise hostile-path race resistance or authentication.
pub fn read_training_plan(path: impl AsRef<Path>, limit: usize) -> Result<ProbeRunPlan, TrainingFileError> {
    if limit > MAX_CAMPAIGN_PLAN_BYTES { return Err(TrainingFileError::Limit); }
    let failure = |stage, error: io::Error| TrainingFileError::Io { stage, kind: error.kind() };
    let path = path.as_ref();
    let before = fs::symlink_metadata(path).map_err(|e| failure(TrainingFileStage::Metadata, e))?;
    if !before.is_file() || before.file_type().is_symlink() { return Err(TrainingFileError::NotRegular); }
    if before.len() > limit as u64 { return Err(TrainingFileError::Limit); }
    let file = File::open(path).map_err(|e| failure(TrainingFileStage::Open, e))?;
    let opened = file.metadata().map_err(|e| failure(TrainingFileStage::Metadata, e))?;
    if !opened.is_file() { return Err(TrainingFileError::NotRegular); }
    if opened.len() > limit as u64 { return Err(TrainingFileError::Limit); }
    let mut bytes = Vec::new(); bytes.try_reserve_exact(opened.len() as usize).map_err(|_| TrainingFileError::Limit)?;
    file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|e| failure(TrainingFileStage::Read, e))?;
    if bytes.len() > limit { return Err(TrainingFileError::Limit); }
    ProbeRunPlan::from_json(&bytes).map_err(TrainingFileError::Plan)
}

/// Parse the full plan before opening configuration or weights. Then load the
/// exact operator-selected files through the existing streamed loader and admit
/// BOTH complete capture and fit/evaluation budgets before numerical execution.
/// Numeric plan identities are declarations, never parameter authentication.
pub fn load_training_inputs(configuration: impl AsRef<Path>, weights: impl AsRef<Path>,
    plan: impl AsRef<Path>, limits: TrainingFileLimits) -> Result<(DecoderModel, ProbeRunPlan), TrainingFileError>
{
    let plan = read_training_plan(plan, limits.plan_bytes)?;
    let (model, _) = DecoderModel::from_llama_files(plan.identity(), plan.context(), configuration, weights,
        limits.checkpoint).map_err(TrainingFileError::Checkpoint)?;
    plan.estimate(&model).map_err(TrainingFileError::Admission)?;
    Ok((model, plan))
}
