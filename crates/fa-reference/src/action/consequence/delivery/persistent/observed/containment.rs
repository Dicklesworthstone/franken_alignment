//! Durable actor-state capture and reset through the original containment owner.
//! Cache/sampler semantics remain a trusted host profile, not inferred from bytes.
pub(super) mod codec;

use super::{Event, FileOversight, JournalError, Transition};
use crate::action::consequence::gate::{ReviewBinding, TargetCeiling};
use crate::action::consequence::gate::containment::{ActorState, CheckpointHandle, ResetReceipt, RestartProfile};
use crate::action::ResolvedTarget;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

pub const MAX_FILE_STATE_UPDATES: usize = 128;
pub const MAX_FILE_STATE_UPDATE_BYTES: usize = 8 * 1024 * 1024;

/// Trusted host observation. Epoch binding rejects delayed pre-recovery writes
/// even when recovery itself did not advance the actor's state revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStateUpdate {
    pub operation: u64,
    pub expected_actor_revision: u64,
    pub expected_authority_epoch: u64,
    pub state: ActorState,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStateReceipt {
    pub operation: u64,
    pub actor_revision: u64,
    pub authority_epoch: u64,
    pub next_position: u64,
}

/// Privileged data read from the original actor, never the actor-wire projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileActorSnapshot {
    pub actor_revision: u64,
    pub incident_count: u64,
    pub state: ActorState,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCheckpointInfo {
    pub checkpoint: u64,
    pub actor_revision: u64,
    pub authority_epoch: u64,
    pub control_sequence: u64,
    pub profile: RestartProfile,
    pub next_position: u64,
}

/// Data handle branded to this writable file owner. Reopening requires explicit
/// reacquisition; neither this handle nor its numeric ID contains effect rights.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::containment::FileCheckpoint;
/// fn grant(checkpoint: FileCheckpoint) -> FilePermit { checkpoint }
/// ```
#[derive(Clone, Debug)]
pub struct FileCheckpoint { pub(super) issuer: Rc<()>, pub(super) info: FileCheckpointInfo }
impl FileCheckpoint {
    pub fn id(&self) -> u64 { self.info.checkpoint }
    pub fn info(&self) -> &FileCheckpointInfo { &self.info }
}

/// Independent supervisor instruction, not a model/helper request. The original
/// reducer owns incident escalation, monotone narrowing and cancellation rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileResetRequest {
    pub operation: u64,
    pub expected_control_sequence: u64,
    pub expected_actor_revision: u64,
    pub expected_authority_epoch: u64,
    pub binding: ReviewBinding,
    pub retained_targets: Vec<ResolvedTarget>,
}
impl FileResetRequest {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.operation == 0 || self.binding.round == 0 || self.binding.reducer_generation == 0
            || self.binding.evidence_root == [0; 32] { return Err(Error::InvalidInput); }
        TargetCeiling::new(&self.retained_targets)?;
        Ok(())
    }
}

pub(super) struct CapturedCheckpoint { pub(super) info: FileCheckpointInfo, pub(super) native: CheckpointHandle }
/// Request deduplication and original handles only. No rights, balances, verdict
/// reducer, current actor copy or caller-asserted restoration lives in this map.
#[derive(Default)]
pub(super) struct ContainmentHistory {
    pub(super) updates: BTreeMap<u64, (FileStateUpdate, FileStateReceipt)>,
    pub(super) checkpoints: BTreeMap<u64, CapturedCheckpoint>,
    pub(super) resets: BTreeMap<u64, (u64, FileResetRequest, ResetReceipt)>,
    pub(super) update_bytes: usize,
}

impl FileOversight {
    pub fn actor_snapshot(&self) -> Result<FileActorSnapshot, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FileActorSnapshot { actor_revision: self.machine.broker.actor_revision(),
            incident_count: self.machine.broker.incident_count(), state: self.machine.broker.retained_actor_state().clone() })
    }

    /// An exact retry is historical acknowledgment recovery, not another write.
    /// New updates use both the journal predecessor and the original actor/epoch.
    pub fn record_actor_state(&mut self, revision: u64, update: FileStateUpdate) -> Result<FileStateReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some((previous, receipt)) = self.machine.containment.updates.get(&update.operation) {
            return if previous == &update { Ok(receipt.clone()) } else { Err(Error::Binding.into()) };
        }
        match self.transact(revision, Event::ActorState(update))? {
            Transition::ActorRecorded(receipt) => Ok(receipt), _ => unreachable!("original state update"),
        }
    }

    /// Capture the original actor NOW, not caller-supplied checkpoint bytes.
    /// Exact retry does not replace that state with a later continuation.
    pub fn capture_actor_checkpoint(&mut self, revision: u64, checkpoint: u64,
        expected_actor_revision: u64, expected_authority_epoch: u64) -> Result<FileCheckpoint, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(previous) = self.machine.containment.checkpoints.get(&checkpoint) {
            if previous.info.actor_revision != expected_actor_revision
                || previous.info.authority_epoch != expected_authority_epoch { return Err(Error::Binding.into()); }
            return self.actor_checkpoint(checkpoint);
        }
        self.transact(revision, Event::ActorCheckpoint(checkpoint, expected_actor_revision, expected_authority_epoch))?;
        self.actor_checkpoint(checkpoint)
    }

    /// Reacquire only checkpoint data identity after recovery. The native handle
    /// is rebuilt by replaying its original capture, never deserialized from here.
    pub fn actor_checkpoint(&self, checkpoint: u64) -> Result<FileCheckpoint, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let captured = self.machine.containment.checkpoints.get(&checkpoint).ok_or(Error::Missing)?;
        Ok(FileCheckpoint { issuer: Rc::clone(&self.issuer), info: captured.info.clone() })
    }

    /// Commit the original reset and human-key withdrawal together. Unknown and
    /// executed effects remain charged. Existing envelopes retain their ordered
    /// dispatch; the optional publication guard and stop/fence stay independent.
    pub fn reset_actor(&mut self, revision: u64, checkpoint: &FileCheckpoint,
        request: FileResetRequest) -> Result<ResetReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &checkpoint.issuer) { return Err(Error::Binding.into()); }
        if let Some((id, previous, receipt)) = self.machine.containment.resets.get(&request.operation) {
            return if *id == checkpoint.id() && previous == &request {
                Ok(receipt.clone())
            } else { Err(Error::Binding.into()) };
        }
        match self.transact(revision, Event::ActorReset(checkpoint.id(), request))? {
            Transition::ActorReset(receipt) => Ok(receipt), _ => unreachable!("original containment reset"),
        }
    }

    /// Historical receipt only. This never increments an incident or resets the
    /// actor again, and is unavailable while storage has an unacknowledged cut.
    pub fn actor_reset_receipt(&self, operation: u64) -> Result<&ResetReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(&self.machine.containment.resets.get(&operation).ok_or(Error::Missing)?.2)
    }
}
