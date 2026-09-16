//! Counterfactual computation from a durable ORIGINAL paired checkpoint.
//! No writable owner, approval, actor reset, effect endpoint or RNG is exported.

use super::{FileDecoderCheckpoint, FileDecoderCheckpointInfo, FileOversight, JournalError, Machine};
use super::super::{DecoderEvent, FileDecoderConfig};
use super::super::super::{Event, FileOversightProfile, journal, storage};
use crate::action::{Purpose, Scope};
use crate::action::consequence::activation::tensor::kv::decoder::experiment::DecoderLayerIntervention;
use crate::action::consequence::activation::tensor::kv::decoder::experiment::comparison::{
    DecoderComparisonBudget, DecoderContinuationComparison, DecoderContinuationStep,
    cursor::{DecoderComparisonCursor, DecoderComparisonStatus, DecoderComparisonWork},
};
use crate::Error;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileInvestigationContinuation {
    TeacherForced(Vec<u32>),
    Greedy { first_token: u32, steps: usize },
}

/// A fixed experimental question, never a proposed effect or helper judgment.
/// Native validators check every exact edit preimage and the full paired budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInvestigationRequest {
    pub experiment: u64,
    pub layers: BTreeMap<u64, DecoderLayerIntervention>,
    pub edit_limit: usize,
    pub continuation: FileInvestigationContinuation,
    pub budget: DecoderComparisonBudget,
}

/// Historical origin of the branch, not proof that this remains the current
/// canonical head. Configuration equality includes actual parameter input bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInvestigationOrigin {
    directory: PathBuf,
    journal_revision: u64,
    source_scope: Scope,
    checkpoint: FileDecoderCheckpointInfo,
    configuration: FileDecoderConfig,
}
impl FileInvestigationOrigin {
    pub fn directory(&self) -> &Path { &self.directory }
    pub fn journal_revision(&self) -> u64 { self.journal_revision }
    pub fn source_scope(&self) -> Scope { self.source_scope }
    pub fn checkpoint(&self) -> &FileDecoderCheckpointInfo { &self.checkpoint }
    pub fn configuration(&self) -> &FileDecoderConfig { &self.configuration }
}

/// This report cannot apply a congress decision, reset an actor or clear a hold.
/// A successful contrast is evidence about the chosen edit and horizon only.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::checkpoint::investigation::FileInvestigationReport;
/// fn grant(report: FileInvestigationReport) -> FilePermit { report }
/// ```
#[derive(Clone, Debug)]
pub struct FileInvestigationReport {
    origin: FileInvestigationOrigin,
    request: FileInvestigationRequest,
    comparison: DecoderContinuationComparison,
}
impl FileInvestigationReport {
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn origin(&self) -> &FileInvestigationOrigin { &self.origin }
    pub fn request(&self) -> &FileInvestigationRequest { &self.request }
    pub fn comparison(&self) -> &DecoderContinuationComparison { &self.comparison }
}

/// Owns only immutable checkpoint data and private experimental suffixes. No
/// FileOversight, native broker, endpoint, role or saved sampler is retained.
/// Experiments may outlive the live owner without becoming recovery handles.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::checkpoint::investigation::FileDecoderInvestigation;
/// fn duplicate(run: FileDecoderInvestigation) { let _ = run.clone(); }
/// ```
#[derive(Debug)]
pub struct FileDecoderInvestigation {
    origin: FileInvestigationOrigin,
    request: FileInvestigationRequest,
    cursor: DecoderComparisonCursor,
}
impl FileDecoderInvestigation {
    fn prepare(directory: &Path, revision: u64, profile: &FileOversightProfile,
        machine: &Machine, checkpoint: u64, mut request: FileInvestigationRequest) -> Result<Self, Error>
    {
        let origin = FileInvestigationOrigin { directory: directory.to_owned(), journal_revision: revision,
            source_scope: profile.delivery.scope, checkpoint: machine.decoder_checkpoint_info(checkpoint)?,
            configuration: machine.decoder_contract().ok_or(Error::Incomplete)?.clone() };
        let source = machine.decoder_experiment_source(checkpoint)?;
        // Move unvalidated edits into the original validator. Clone only its
        // successfully bounded specification, never a caller's unbounded map.
        let plan = source.intervene(request.experiment, std::mem::take(&mut request.layers), request.edit_limit)?;
        let cursor = match &request.continuation {
            FileInvestigationContinuation::TeacherForced(tokens) => plan.begin_forced_comparison(tokens, request.budget)?,
            FileInvestigationContinuation::Greedy { first_token, steps } =>
                plan.begin_greedy_comparison(*first_token, *steps, request.budget)?,
        };
        request.layers = plan.specification().clone();
        Ok(Self { origin, request, cursor })
    }
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn origin(&self) -> &FileInvestigationOrigin { &self.origin }
    pub fn request(&self) -> &FileInvestigationRequest { &self.request }
    pub fn status(&self) -> DecoderComparisonStatus { self.cursor.status() }
    pub fn work(&self) -> Result<DecoderComparisonWork, Error> { self.cursor.work() }
    pub fn completed_pairs(&self) -> &[DecoderContinuationStep] { self.cursor.completed_pairs() }
    pub fn advance(&mut self) -> Result<DecoderComparisonStatus, Error> { self.cursor.advance() }
    pub fn cancel(&mut self) -> Result<(), Error> { self.cursor.cancel() }
    pub fn finish(&self) -> Result<FileInvestigationReport, Error> {
        let comparison = self.cursor.finish()?;
        Ok(FileInvestigationReport { origin: self.origin.clone(), request: self.request.clone(), comparison })
    }
}

impl FileOversight {
    /// Branch from this exact original saved pair, not today's actor state.
    /// An acknowledged held/stopped/recovery-paused owner can be investigated
    /// without fresh permitting evidence. Its state, costs and rights do not change.
    pub fn investigate_decoder_checkpoint(&self, checkpoint: &FileDecoderCheckpoint,
        request: FileInvestigationRequest) -> Result<FileDecoderInvestigation, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &checkpoint.issuer)
            || self.machine.decoder_checkpoint_info(checkpoint.id())? != checkpoint.info {
            return Err(Error::Binding.into());
        }
        Ok(FileDecoderInvestigation::prepare(self.store.identity(), self.revision(), &self.profile,
            &self.machine, checkpoint.id(), request)?)
    }

    /// Inspect one full canonical image beside a locked or faulted live owner.
    /// Pin exact numerical inputs BEFORE token replay, then validate the ENTIRE
    /// original history. No writer lock, cleanup, clock, fence or role provisioning
    /// occurs. Reconstruction costs are separate from the experimental budget.
    pub fn read_decoder_investigation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected_decoder: &FileDecoderConfig, checkpoint: u64, request: FileInvestigationRequest)
        -> Result<FileDecoderInvestigation, JournalError>
    {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &identity, &bytes)?;
        let mut configurations = events.iter().filter_map(|event| match event {
            Event::Decoder(DecoderEvent::Enable(config)) => Some(config.as_ref()),
            _ => None,
        });
        if configurations.next() != Some(expected_decoder) || configurations.next().is_some() {
            return Err(Error::Binding.into());
        }
        let machine = Machine::replay(profile, &events)?;
        Ok(FileDecoderInvestigation::prepare(&identity, events.len() as u64, profile,
            &machine, checkpoint, request)?)
    }
}
