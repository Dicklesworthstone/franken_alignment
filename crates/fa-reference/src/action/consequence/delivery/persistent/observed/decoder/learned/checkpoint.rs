//! Durable handles for original paired learned numerical/authority checkpoints.
//! Recovery reconstructs the pair from original execution, never imported state.
use super::{DecoderEvent, Event, FileOversight, JournalError, LearnedEvent, Machine,
    MAX_WITNESS_BYTES, Reader, Transition, Writer, journal};
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedRecoveryUsage;
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedCheckpointInfo {
    pub checkpoint: u64,
    pub actor_revision: u64,
    pub authority_epoch: u64,
    pub control_sequence: u64,
    pub stream: u64,
    pub position: u64,
    pub sampled_draws: u64,
}

/// Reacquire from the recovered owner. This contains no cache, sampler or rights.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedCheckpoint;
/// fn grant(saved: FileLearnedCheckpoint) -> FilePermit { saved }
/// ```
#[derive(Clone, Debug)]
pub struct FileLearnedCheckpoint { issuer: Rc<()>, info: FileLearnedCheckpointInfo }
impl FileLearnedCheckpoint {
    pub fn id(&self) -> u64 { self.info.checkpoint }
    pub fn info(&self) -> &FileLearnedCheckpointInfo { &self.info }
}

#[derive(Clone)]
pub(in super::super::super) enum CheckpointEvent {
    Capture { checkpoint: u64, actor_revision: u64, epoch: u64, witness: Rc<[u8]> },
}

impl FileOversight {
    /// Capture the actual original quiet prefix and authority together. Exact
    /// retries acknowledge that historical cut; they never capture a later one.
    pub fn capture_learned_checkpoint(&mut self, revision: u64, checkpoint: u64,
        actor_revision: u64, authority_epoch: u64) -> Result<FileLearnedCheckpoint, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        match self.machine.learned_checkpoint_info(checkpoint) {
            Ok(info) => {
                if info.actor_revision != actor_revision || info.authority_epoch != authority_epoch {
                    return Err(Error::Binding.into());
                }
                return self.learned_checkpoint(checkpoint);
            }
            Err(Error::Missing) => {}
            Err(error) => return Err(error.into()),
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.check_learned_capture(checkpoint, actor_revision, authority_epoch)?;
        let shape = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
            CheckpointEvent::Capture { checkpoint, actor_revision, epoch: authority_epoch, witness: Rc::from(&b""[..]) })));
        self.check_source_admission(&shape)?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let event = candidate.prepare_learned_capture(checkpoint, actor_revision, authority_epoch)?;
        let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(event)));
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        self.persist_candidate(event, bytes, candidate, Transition::Unit)?;
        self.learned_checkpoint(checkpoint)
    }

    /// Historical identity only. No current observation or approval is revived.
    pub fn learned_checkpoint(&self, checkpoint: u64) -> Result<FileLearnedCheckpoint, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FileLearnedCheckpoint { issuer: Rc::clone(&self.issuer),
            info: self.machine.learned_checkpoint_info(checkpoint)? })
    }

    /// Check this historical handle's owner and original capture, not present
    /// effect eligibility. A pre-recovery or foreign handle must be reacquired.
    pub fn check_learned_checkpoint(&self, checkpoint: &FileLearnedCheckpoint) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &checkpoint.issuer)
            || self.machine.learned_checkpoint_info(checkpoint.id())? != checkpoint.info {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    pub fn learned_recovery_usage(&self) -> Result<HostedLearnedRecoveryUsage, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.hosted_learned_recovery_usage()?)
    }
}

pub(in super::super::super) fn write(w: &mut Writer, event: &CheckpointEvent) -> Result<(), Error> {
    match event {
        CheckpointEvent::Capture { checkpoint, actor_revision, epoch, witness } => {
            if *checkpoint == 0 { return Err(Error::InvalidInput); }
            check_witness(witness)?;
            w.u8(0)?; w.u64(*checkpoint)?; w.u64(*actor_revision)?; w.u64(*epoch)?; w.blob(witness)?;
        }
    }
    Ok(())
}
pub(in super::super::super) fn read(r: &mut Reader<'_>) -> Result<CheckpointEvent, Error> {
    match r.u8()? {
        0 => {
            let checkpoint = r.u64()?; let actor_revision = r.u64()?; let epoch = r.u64()?;
            if checkpoint == 0 { return Err(Error::InvalidInput); }
            let witness = r.blob(MAX_WITNESS_BYTES)?;
            check_witness(witness)?;
            Ok(CheckpointEvent::Capture { checkpoint, actor_revision, epoch, witness: Rc::from(witness) })
        }
        _ => Err(Error::InvalidInput),
    }
}
fn check_witness(witness: &[u8]) -> Result<(), Error> {
    if witness.is_empty() { return Err(Error::Incomplete); }
    if witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
    Ok(())
}
