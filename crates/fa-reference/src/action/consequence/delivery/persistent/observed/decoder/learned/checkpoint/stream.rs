//! Durable stream-kind checkpoint and containment within one confirmed boundary.
use super::{CheckpointEvent, DecoderEvent, Event, FileLearnedCheckpointInfo,
    FileLearnedResetIntent, FileOversight, JournalError, LearnedEvent, Machine,
    Transition, journal};
use super::super::{JournalFailure, JournalIo};
use super::super::super::super::containment::FileResetRequest;
use crate::action::consequence::activation::tensor::kv::decoder::monitoring::restart::KvRestartBudget;
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedResetReceipt;
use crate::Error;
use std::rc::Rc;

/// Original stream-kind identity without cache, sampler, actor or effect rights.
/// Reacquire after recovery; the old process-local owner brand never transfers.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedStreamCheckpoint;
/// fn grant(checkpoint: FileLearnedStreamCheckpoint) -> FilePermit { checkpoint }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::{FileLearnedCheckpoint, FileLearnedStreamCheckpoint};
/// fn downgrade(checkpoint: FileLearnedStreamCheckpoint) -> FileLearnedCheckpoint { checkpoint }
/// ```
#[derive(Clone, Debug)]
pub struct FileLearnedStreamCheckpoint { issuer: Rc<()>, info: FileLearnedCheckpointInfo }
impl FileLearnedStreamCheckpoint {
    pub fn id(&self) -> u64 { self.info.checkpoint }
    pub fn info(&self) -> &FileLearnedCheckpointInfo { &self.info }
}

impl FileOversight {
    /// Capture a quiet active original prefix within the exact confirmed stream
    /// target, profile, complete-message history and original text output offset.
    /// Pending/visible-but-unconfirmed/finished output and installed predictors
    /// refuse. Existing legacy capture/reset APIs keep refusing stream owners.
    pub fn capture_learned_stream_checkpoint(&mut self, revision: u64, checkpoint: u64,
        actor_revision: u64, authority_epoch: u64) -> Result<FileLearnedStreamCheckpoint, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        match self.machine.learned_stream_checkpoint_info(checkpoint) {
            Ok(info) => {
                if info.actor_revision != actor_revision || info.authority_epoch != authority_epoch {
                    return Err(Error::Binding.into());
                }
                return self.learned_stream_checkpoint(checkpoint);
            }
            Err(Error::Missing) => {},
            Err(error) => return Err(error.into()),
        }
        self.check_learned_event_capacity(revision, 1)?;
        self.machine.check_learned_stream_capture(checkpoint, actor_revision, authority_epoch)?;
        self.check_source_admission(&event(CheckpointEvent::StreamCapture { checkpoint, actor_revision,
            epoch: authority_epoch, witness: Rc::from(&b""[..]) }))?;
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let capture = event(candidate.prepare_learned_stream_capture(checkpoint, actor_revision, authority_epoch)?);
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &capture)?;
        self.persist_candidate(capture, bytes, candidate, Transition::Unit)?;
        self.learned_stream_checkpoint(checkpoint)
    }

    pub fn learned_stream_checkpoint(&self, checkpoint: u64) -> Result<FileLearnedStreamCheckpoint, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FileLearnedStreamCheckpoint { issuer: Rc::clone(&self.issuer),
            info: self.machine.learned_stream_checkpoint_info(checkpoint)? })
    }

    /// Historical identity only; this does not prove current reset eligibility.
    pub fn check_learned_stream_checkpoint(&self, checkpoint: &FileLearnedStreamCheckpoint)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &checkpoint.issuer)
            || self.machine.learned_stream_checkpoint_info(checkpoint.id())? != checkpoint.info {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    /// Freeze a stream-kind instruction before any audit. Strictly require
    /// capture.control_sequence < first_incident_sequence <= current sequence.
    /// The incident frontier is a SUPERVISOR declaration, not authenticated
    /// provenance; numerical steps alone need not advance this control clock.
    /// The original target, publication boundary and message offset cannot move.
    pub fn begin_learned_stream_reset(&mut self, revision: u64, checkpoint: &FileLearnedStreamCheckpoint,
        control: FileResetRequest, budget: KvRestartBudget, first_incident_sequence: u64)
        -> Result<(), JournalError>
    {
        self.check_learned_stream_checkpoint(checkpoint)?;
        let intent = FileLearnedResetIntent { checkpoint: checkpoint.id(), control, budget };
        intent.validate()?;
        if self.machine.learned_stream_reset_retry(&intent, first_incident_sequence)?.is_some() { return Ok(()); }
        if let Some(pending) = self.machine.pending_learned_reset() {
            if pending.intent != intent
                || self.machine.pending_learned_stream_reset_sequence() != Some(first_incident_sequence) {
                return Err(Error::Binding.into());
            }
            if pending.interrupted { return Err(Error::WrongState.into()); }
            return Ok(());
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.check_learned_stream_reset_intent(&intent, first_incident_sequence)?;
        self.check_learned_event_capacity(revision, 2)?;
        self.transact(revision, event(CheckpointEvent::BeginStreamReset {
            intent: Rc::new(intent), first_incident_sequence }))?;
        Ok(())
    }

    /// Outer Err acknowledges no completion. Inner Err is the original recorded
    /// refusal/failure and retains its real audit costs and containment state.
    /// Exact completed-operation retries return history without another audit,
    /// incident, source observation, journal replacement or restored authority.
    pub fn reset_learned_stream_checkpoint(&mut self, revision: u64, checkpoint: &FileLearnedStreamCheckpoint,
        control: FileResetRequest, budget: KvRestartBudget, first_incident_sequence: u64)
        -> Result<Result<HostedLearnedResetReceipt, Error>, JournalError>
    {
        self.check_learned_stream_checkpoint(checkpoint)?;
        let intent = FileLearnedResetIntent { checkpoint: checkpoint.id(), control, budget };
        intent.validate()?;
        if let Some(result) = self.machine.learned_stream_reset_retry(&intent, first_incident_sequence)? {
            return Ok(result);
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let operation = intent.control.operation;
        self.begin_learned_stream_reset(revision, checkpoint, intent.control, budget, first_incident_sequence)?;
        self.complete_learned_stream_reset(self.revision(), operation)
    }

    /// Complete only the unchanged, uninterrupted stream intent. Recovery
    /// preserves the unresolved barrier; it cannot rebase to a newer epoch.
    /// The original reset engine conserves cumulative numerical/audit work,
    /// publication history, spent rights, incident count and authority floors.
    /// Restored evidence starts Empty and requires a fresh accepted token plus
    /// ordinary congress and both configured keys before any new publication.
    pub fn complete_learned_stream_reset(&mut self, revision: u64, operation: u64)
        -> Result<Result<HostedLearnedResetReceipt, Error>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.check_learned_reset_kind(operation, true)?;
        if let Some(result) = self.machine.learned_reset_result_optional(operation)? { return Ok(result); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.check_pending_learned_stream_reset(operation)?;
        self.check_source_admission(&event(CheckpointEvent::StreamReset { operation, witness: Rc::from(&b""[..]) }))?;
        self.check_learned_event_capacity(revision, 1)?;
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let (mut candidate, retain_replay) = self.replay_control_candidate()?;
        // Poison BEFORE fallible new audit/restoration/encoding/I/O, exactly as
        // the original completion path. No old ready machine can escape a loss.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let completed = event(candidate.prepare_learned_stream_reset(operation)?);
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &completed)?;
        self.persist_control_candidate(completed, bytes, candidate, Transition::Unit, retain_replay)?;
        self.learned_reset_result(operation)
    }
}

fn event(value: CheckpointEvent) -> Event { Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(value))) }
