//! Replay only original actor/reset operations; never import balances or permits.
use super::{Machine, Transition};
use super::super::containment::{CapturedCheckpoint, FileCheckpointInfo, FileResetRequest,
    FileStateReceipt, FileStateUpdate, MAX_FILE_STATE_UPDATES, MAX_FILE_STATE_UPDATE_BYTES};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::ResetRequest;
use crate::Error;

impl Machine {
    pub(super) fn record_actor_state(&mut self, update: &FileStateUpdate) -> Result<Transition, Error> {
        if update.operation == 0 { return Err(Error::InvalidInput); }
        if self.containment.updates.contains_key(&update.operation) { return Err(Error::Duplicate); }
        if update.expected_authority_epoch != self.broker.inspect().ledger.epoch { return Err(Error::Stale); }
        if self.containment.updates.len() >= MAX_FILE_STATE_UPDATES { return Err(Error::Limit); }
        let state = &update.state;
        let bytes = state.tokens().len().checked_mul(4)
            .and_then(|n| n.checked_add(state.cache().len()))
            .and_then(|n| n.checked_add(state.sampler().len()))
            .and_then(|n| n.checked_add(self.containment.update_bytes)).ok_or(Error::Limit)?;
        if bytes > MAX_FILE_STATE_UPDATE_BYTES { return Err(Error::Limit); }
        self.broker.replace_actor_state(update.expected_actor_revision, state.clone())?;
        let receipt = FileStateReceipt { operation: update.operation,
            actor_revision: self.broker.actor_revision(), authority_epoch: self.broker.inspect().ledger.epoch,
            next_position: self.broker.retained_actor_state().next_position() };
        self.containment.updates.insert(update.operation, (update.clone(), receipt.clone()));
        self.containment.update_bytes = bytes;
        Ok(Transition::ActorRecorded(receipt))
    }

    pub(super) fn capture_actor_checkpoint(&mut self, id: u64, revision: u64, epoch: u64) -> Result<Transition, Error> {
        if epoch != self.broker.inspect().ledger.epoch { return Err(Error::Stale); }
        let native = self.broker.capture_checkpoint(id, revision)?;
        let actor = self.broker.retained_actor_state();
        let info = FileCheckpointInfo { checkpoint: id, actor_revision: revision, authority_epoch: epoch,
            control_sequence: self.broker.inspect().sequence, profile: actor.profile(), next_position: actor.next_position() };
        self.containment.checkpoints.insert(id, CapturedCheckpoint { info, native });
        Ok(Transition::Unit)
    }

    pub(super) fn reset_actor(&mut self, id: u64, request: &FileResetRequest) -> Result<Transition, Error> {
        request.validate()?;
        if self.containment.resets.contains_key(&request.operation) { return Err(Error::Duplicate); }
        if request.expected_authority_epoch != self.broker.inspect().ledger.epoch { return Err(Error::Stale); }
        let checkpoint = self.containment.checkpoints.get(&id).ok_or(Error::Missing)?.native.clone();
        let receipt = self.broker.reset(ResetRequest { checkpoint,
            expected_control_sequence: request.expected_control_sequence,
            expected_actor_revision: request.expected_actor_revision, binding: request.binding,
            retained_targets: TargetCeiling::new(&request.retained_targets)?,
        })?;
        // Same speculative machine; all these transitions are published together
        // by the one existing canonical replacement or none is acknowledged.
        self.withdraw_keys()?;
        self.sessions.clear();
        self.automatic.clear();
        // Do NOT clear envelopes or endpoint outcomes: reset cannot un-dispatch.
        self.containment.resets.insert(request.operation, (id, request.clone(), receipt.clone()));
        Ok(Transition::ActorReset(receipt))
    }
}
