//! Durable pairing of original authority and monitored numerical checkpoints.
//! Handles contain no state/rights; reset inputs replay through the native owner.
use super::{DecoderEvent, FileOversight, JournalError, Machine, Transition, Event, journal};
use super::super::containment::{FileResetRequest, codec as control_codec};
use super::super::super::{JournalFailure, JournalIo, codec::shared::{Reader, Writer}};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use crate::action::consequence::oversight::decoder_host::{HostedRecoveryUsage, HostedResetReceipt};
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDecoderCheckpointInfo {
    pub checkpoint: u64,
    pub actor_revision: u64,
    pub authority_epoch: u64,
    pub control_sequence: u64,
    pub position: u64,
    pub sampled_draws: u64,
}

/// Reacquire from a recovered owner, never import bytes into a running decoder.
/// Cloning identifies the same historical capture; it cannot copy effect rights.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::checkpoint::FileDecoderCheckpoint;
/// fn grant(saved: FileDecoderCheckpoint) -> FilePermit { saved }
/// ```
#[derive(Clone, Debug)]
pub struct FileDecoderCheckpoint { issuer: Rc<()>, info: FileDecoderCheckpointInfo }
impl FileDecoderCheckpoint {
    pub fn id(&self) -> u64 { self.info.checkpoint }
    pub fn info(&self) -> &FileDecoderCheckpointInfo { &self.info }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum CheckpointRequest {
    Capture { checkpoint: u64, actor_revision: u64, epoch: u64 },
    Reset { checkpoint: u64, control: FileResetRequest, budget: DecoderBudget },
}
impl CheckpointRequest {
    pub(in super::super) fn validate(&self) -> Result<(), Error> {
        let id = match self {
            Self::Capture { checkpoint, .. } => *checkpoint,
            Self::Reset { checkpoint, control, budget } => {
                control.validate()?;
                if budget.scalar_products > MAX_DECODER_PRODUCTS { return Err(Error::Limit); }
                *checkpoint
            }
        };
        if id == 0 { return Err(Error::InvalidInput); }
        Ok(())
    }
}

impl FileOversight {
    /// Capture the SAME native quiet, nonempty owner and its original authority
    /// checkpoint in one journal replacement. Exact retries never capture later
    /// state. New captures require a fresh clock and an explicitly resumed owner.
    pub fn capture_decoder_checkpoint(&mut self, revision: u64, checkpoint: u64,
        actor_revision: u64, authority_epoch: u64) -> Result<FileDecoderCheckpoint, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        match self.machine.decoder_checkpoint_info(checkpoint) {
            Ok(info) => {
                if info.actor_revision != actor_revision || info.authority_epoch != authority_epoch {
                    return Err(Error::Binding.into());
                }
                return self.decoder_checkpoint(checkpoint);
            }
            Err(Error::Missing) => {}
            Err(error) => return Err(error.into()),
        }
        self.transact_checkpoint(revision, CheckpointRequest::Capture {
            checkpoint, actor_revision, epoch: authority_epoch,
        })?;
        self.decoder_checkpoint(checkpoint)
    }

    /// Historical handle only. Recovery rebuilds the native pair by replaying its
    /// original capture, then fences old approvals before this method is available.
    pub fn decoder_checkpoint(&self, checkpoint: u64) -> Result<FileDecoderCheckpoint, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(FileDecoderCheckpoint { issuer: Rc::clone(&self.issuer),
            info: self.machine.decoder_checkpoint_info(checkpoint)? })
    }

    /// Supervisor-only rewind through the ORIGINAL recomputation/review/reset.
    /// Outer Err is unacknowledged; inner Err is a committed native refusal with
    /// any replay charges and numerical failure retained. Exact operation retries
    /// return that result without replay, additional incidents or replacement keys.
    /// A recovered owner stays paused, including after a successful reset.
    pub fn reset_decoder_checkpoint(&mut self, revision: u64, checkpoint: &FileDecoderCheckpoint,
        control: FileResetRequest, budget: DecoderBudget)
        -> Result<Result<HostedResetReceipt, Error>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &checkpoint.issuer)
            || self.machine.decoder_checkpoint_info(checkpoint.id())? != checkpoint.info {
            return Err(Error::Binding.into());
        }
        let operation = control.operation;
        let request = CheckpointRequest::Reset { checkpoint: checkpoint.id(), control, budget };
        request.validate()?;
        if let Some(result) = self.machine.decoder_reset_retry(&request)? { return Ok(result); }
        self.transact_checkpoint(revision, request)?;
        self.decoder_reset_result(operation)
    }

    pub fn decoder_reset_result(&self, operation: u64)
        -> Result<Result<HostedResetReceipt, Error>, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.decoder_reset_result(operation)?)
    }
    pub fn decoder_recovery_usage(&self) -> Result<HostedRecoveryUsage, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.hosted_recovery_usage()?)
    }

    fn transact_checkpoint(&mut self, revision: u64, request: CheckpointRequest) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        request.validate()?;
        self.check_source_admission(&Event::Decoder(DecoderEvent::Checkpoint(request.clone(), Rc::from(&b""[..]))))?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        candidate.check_decoder_checkpoint_request(&request)?;
        if matches!(&request, CheckpointRequest::Reset { .. }) {
            // Once numerical replay starts, an unwind or failure to encode its
            // result cannot leave the old permitting owner usable. Explicit
            // recovery is required even if no replacement was attempted.
            self.fault = Some(JournalFailure { operation: JournalIo::Stage,
                kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        }
        let event = Event::Decoder(candidate.prepare_decoder_checkpoint(request)?);
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        self.persist_candidate(event, bytes, candidate, Transition::Unit)?;
        Ok(())
    }
}

pub(in super::super) fn write_request(w: &mut Writer, request: &CheckpointRequest) -> Result<(), Error> {
    request.validate()?;
    match request {
        CheckpointRequest::Capture { checkpoint, actor_revision, epoch } => {
            w.u8(0)?; w.u64(*checkpoint)?; w.u64(*actor_revision)?; w.u64(*epoch)?;
        }
        CheckpointRequest::Reset { checkpoint, control, budget } => {
            w.u8(1)?; w.u64(*checkpoint)?; control_codec::write_reset(w, control)?;
            w.u64(budget.scalar_products)?;
        }
    }
    Ok(())
}
pub(in super::super) fn read_request(r: &mut Reader<'_>) -> Result<CheckpointRequest, Error> {
    let request = match r.u8()? {
        0 => CheckpointRequest::Capture { checkpoint: r.u64()?, actor_revision: r.u64()?, epoch: r.u64()? },
        1 => CheckpointRequest::Reset { checkpoint: r.u64()?, control: control_codec::read_reset(r)?,
            budget: DecoderBudget { scalar_products: r.u64()? } },
        _ => return Err(Error::InvalidInput),
    };
    request.validate()?;
    Ok(request)
}

#[cfg(test)]
mod tests;
