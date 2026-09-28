//! Write-ahead reset instructions and acknowledged original outcomes.
use super::{CheckpointEvent, FileLearnedCheckpoint, FileOversight, JournalError,
    Machine, Transition, Event, DecoderEvent, LearnedEvent, Reader, Writer, journal};
use super::super::{JournalFailure, JournalIo};
use super::super::super::super::containment::{FileResetRequest, codec as control_codec};
use crate::action::consequence::activation::{
    monitor::learned::{LearnedMonitorBudget, model::LearnedAuditPreparationBudget},
    probe::learned::CheckedKvBudget,
    tensor::kv::{model::{MAX_MODEL_KV_VALUES, learned::CompressionBudget},
        decoder::monitoring::{LearnedDecoderAllowance, restart::KvRestartBudget}},
};
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedResetReceipt;
use crate::Error;
use std::rc::Rc;

/// Immutable supervisor instruction. These data never import a checkpoint or
/// confer authority. The original fixed policy intersects all audit ceilings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedResetIntent {
    pub(in super::super::super::super) checkpoint: u64,
    pub(in super::super::super::super) control: FileResetRequest,
    pub(in super::super::super::super) budget: KvRestartBudget,
}
impl FileLearnedResetIntent {
    pub fn checkpoint(&self) -> u64 { self.checkpoint }
    pub fn control(&self) -> &FileResetRequest { &self.control }
    pub fn budget(&self) -> KvRestartBudget { self.budget }
    pub(in super::super::super::super) fn validate(&self) -> Result<(), Error> {
        self.control.validate()?;
        if self.checkpoint == 0 { return Err(Error::InvalidInput); }
        if self.budget.cache_values > MAX_MODEL_KV_VALUES { return Err(Error::Limit); }
        Ok(())
    }
}

/// An acknowledged intent is not an acknowledged restore. Once a fence/stop
/// interrupts it, no new numerical/effect work can abandon the unknown outcome.
/// Its fixed allowance bounds any unreported partial work; it is not a measured
/// completed-work report. Settlement of existing effects remains available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingLearnedReset {
    pub intent: FileLearnedResetIntent,
    pub interrupted: bool,
}

impl FileOversight {
    pub fn pending_learned_reset(&self) -> Result<Option<PendingLearnedReset>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.pending_learned_reset().cloned())
    }

    /// Freeze the exact original control predecessor BEFORE any new audit or
    /// restore. No positive work may pass an unresolved intent. A recovered or
    /// explicitly fenced intent is never upgraded to the newer authority epoch.
    pub fn begin_learned_reset(&mut self, revision: u64, checkpoint: &FileLearnedCheckpoint,
        control: FileResetRequest, budget: KvRestartBudget) -> Result<(), JournalError>
    {
        self.check_learned_checkpoint(checkpoint)?;
        let intent = FileLearnedResetIntent { checkpoint: checkpoint.id(), control, budget };
        intent.validate()?;
        if self.machine.learned_reset_retry(&intent)?.is_some() { return Ok(()); }
        if let Some(pending) = self.machine.pending_learned_reset() {
            if pending.intent != intent { return Err(Error::Binding.into()); }
            if pending.interrupted { return Err(Error::WrongState.into()); }
            return Ok(());
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.check_learned_reset_intent(&intent)?;
        if self.events.len().checked_add(2).ok_or(Error::Limit)? > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        self.transact(revision, event(CheckpointEvent::BeginReset(Rc::new(intent))))?;
        Ok(())
    }

    /// Outer Err means no acknowledged completion. Inner Err is an acknowledged
    /// ORIGINAL refusal/failure, including its retained work and containment.
    /// Exact completed-operation retries return that historical result without
    /// more audits, incidents, key issuance or filesystem replacement.
    pub fn reset_learned_checkpoint(&mut self, revision: u64, checkpoint: &FileLearnedCheckpoint,
        control: FileResetRequest, budget: KvRestartBudget)
        -> Result<Result<HostedLearnedResetReceipt, Error>, JournalError>
    {
        self.check_learned_checkpoint(checkpoint)?;
        let intent = FileLearnedResetIntent { checkpoint: checkpoint.id(), control, budget };
        intent.validate()?;
        if let Some(result) = self.machine.learned_reset_retry(&intent)? { return Ok(result); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let operation = intent.control.operation;
        self.begin_learned_reset(revision, checkpoint, intent.control, budget)?;
        self.complete_learned_reset(self.revision(), operation)
    }

    /// Complete only the unchanged, uninterrupted original intent. A failed
    /// canonical replacement returns no candidate receipt and poisons this live
    /// owner. Recovery either finds the actual completion or an interrupted
    /// barrier; it cannot silently use the previously quiet source.
    pub fn complete_learned_reset(&mut self, revision: u64, operation: u64)
        -> Result<Result<HostedLearnedResetReceipt, Error>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(result) = self.machine.learned_reset_result_optional(operation)? { return Ok(result); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.check_pending_learned_reset(operation)?;
        self.check_source_admission(&event(CheckpointEvent::Reset { operation, witness: Rc::from(&b""[..]) }))?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let completed = candidate.prepare_learned_reset(operation)?;
        let completed = event(completed);
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &completed)?;
        self.persist_candidate(completed, bytes, candidate, Transition::Unit)?;
        self.learned_reset_result(operation)
    }

    /// Historical result, never new permission or evidence that an effect ran.
    pub fn learned_reset_result(&self, operation: u64)
        -> Result<Result<HostedLearnedResetReceipt, Error>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.learned_reset_result_optional(operation)?.ok_or(Error::Missing)?)
    }
}

fn event(value: CheckpointEvent) -> Event { Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(value))) }

pub(in super::super::super::super) fn write_intent(w: &mut Writer, intent: &FileLearnedResetIntent) -> Result<(), Error> {
    intent.validate()?;
    w.u64(intent.checkpoint)?; control_codec::write_reset(w, &intent.control)?;
    w.count(intent.budget.cache_values)?;
    let p = intent.budget.audit.preparation; let m = intent.budget.audit.monitoring;
    w.count(p.compression.source_values)?; w.count(p.compression.encoded_bytes)?; w.u64(p.compression.work_units)?;
    w.count(p.source_check.source_values)?; w.count(p.source_check.encoded_bytes)?; w.u64(p.source_check.reconstruction_products)?;
    w.count(m.encoded_bytes)?; w.count(m.probe_coordinates)?; w.u64(m.reconstruction_products)?;
    w.count(m.materialized_values)?; w.count(m.refinements)?;
    Ok(())
}
pub(in super::super::super::super) fn read_intent(r: &mut Reader<'_>) -> Result<FileLearnedResetIntent, Error> {
    let checkpoint = r.u64()?; let control = control_codec::read_reset(r)?;
    let cache_values = r.count(MAX_MODEL_KV_VALUES)?;
    // Fixed-size scalar fields allocate nothing. The original policy, not these
    // parsed requests, caps every actual audit dimension before any numerical work.
    let compression = CompressionBudget { source_values: r.count(usize::MAX)?,
        encoded_bytes: r.count(usize::MAX)?, work_units: r.u64()? };
    let source_check = CheckedKvBudget { source_values: r.count(usize::MAX)?,
        encoded_bytes: r.count(usize::MAX)?, reconstruction_products: r.u64()? };
    let monitoring = LearnedMonitorBudget { encoded_bytes: r.count(usize::MAX)?,
        probe_coordinates: r.count(usize::MAX)?, reconstruction_products: r.u64()?,
        materialized_values: r.count(usize::MAX)?, refinements: r.count(usize::MAX)? };
    let intent = FileLearnedResetIntent { checkpoint, control, budget: KvRestartBudget { cache_values,
        audit: LearnedDecoderAllowance { preparation: LearnedAuditPreparationBudget { compression, source_check }, monitoring } } };
    intent.validate()?;
    Ok(intent)
}
