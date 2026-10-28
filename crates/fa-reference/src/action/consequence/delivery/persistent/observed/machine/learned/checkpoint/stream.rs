//! Original stream-kind captures and replay-bound containment outcomes.
use super::{CheckpointEvent, DecoderEvent, Event, FileLearnedCheckpointInfo, LearnedEvent,
    Machine, MAX_WITNESS_BYTES, Transition, Writer};
use super::super::super::super::decoder::learned::checkpoint::{FileLearnedResetIntent, PendingLearnedReset};
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedResetReceipt;
use crate::Error;
use std::rc::Rc;

impl Machine {
    pub(in super::super::super::super) fn learned_stream_checkpoint_info(&self, id: u64)
        -> Result<FileLearnedCheckpointInfo, Error>
    {
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        if state.checkpoints.captures.contains_key(&id) { return Err(Error::Binding); }
        Ok(state.checkpoints.stream_captures.get(&id).ok_or(Error::Missing)?.0)
    }

    // Endpoint state can be ahead of the broker while a receipt is missing.
    // Neither that state nor an already consumed later-message request may be
    // rebound by restoring a numerical prefix under the same publication basis.
    fn check_learned_stream_reset_boundary(&self) -> Result<(), Error> {
        self.broker.check_hosted_learned_stream_capture_basis()?;
        let (_, confirmed) = self.broker.stream_state().ok_or(Error::Binding)?;
        if self.endpoint.stream_view() != Some(confirmed) { return Err(Error::Incomplete); }
        if let Some(command) = self.current_learned_message_continuation() {
            match self.requests.status(command.request()) {
                Err(Error::Missing) => {},
                Ok(_) => return Err(Error::Duplicate),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    pub(in super::super::super::super) fn check_learned_stream_capture(&self, id: u64,
        revision: u64, epoch: u64) -> Result<(), Error>
    {
        self.check_learned_capture(id, revision, epoch)?;
        self.check_learned_stream_reset_boundary()
    }

    pub(in super::super::super::super) fn prepare_learned_stream_capture(&mut self, id: u64,
        revision: u64, epoch: u64) -> Result<CheckpointEvent, Error>
    {
        let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
            CheckpointEvent::StreamCapture { checkpoint: id, actor_revision: revision,
                epoch, witness: Rc::from(&b""[..]) })));
        self.check_decoder_admission(&event)?;
        self.execute_learned_stream_capture(id, revision, epoch)?;
        let witness = self.learned_stream_capture_witness(id)?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(CheckpointEvent::StreamCapture { checkpoint: id, actor_revision: revision,
            epoch, witness: witness.into() })
    }

    pub(super) fn execute_learned_stream_capture(&mut self, id: u64, revision: u64, epoch: u64)
        -> Result<(), Error>
    {
        self.check_learned_stream_capture(id, revision, epoch)?;
        let native = self.broker.capture_hosted_learned_stream_checkpoint(id, revision)?;
        let actual = self.broker.hosted_learned_generation()?;
        let info = FileLearnedCheckpointInfo { checkpoint: id, actor_revision: revision,
            authority_epoch: epoch, control_sequence: self.broker.inspect().sequence,
            position: actual.position, stream: self.broker.hosted_learned_observation()?.stream(),
            sampled_draws: actual.sampled_draws };
        self.learned.as_mut().expect("checked learned source").checkpoints.stream_captures.insert(id, (info, native));
        Ok(())
    }

    pub(super) fn learned_stream_capture_witness(&self, id: u64) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_WITNESS_BYTES);
        w.raw(b"FALSCP\0\x01")?;
        self.write_learned_state(&mut w)?;
        self.write_learned_recovery_state(&mut w)?;
        let info = self.learned_stream_checkpoint_info(id)?;
        for value in [info.checkpoint, info.actor_revision, info.authority_epoch, info.control_sequence,
            info.stream, info.position, info.sampled_draws] { w.u64(value)?; }
        self.write_learned_stream_basis(&mut w, id)?;
        Ok(w.finish())
    }

    fn write_learned_stream_basis(&self, w: &mut Writer, id: u64) -> Result<(), Error> {
        let basis = self.broker.hosted_learned_stream_checkpoint_basis(id)?;
        w.target(basis.target)?;
        let profile = basis.view.profile();
        w.u64(profile.id())?; w.u64(profile.generation())?;
        w.count(profile.max_messages())?; w.count(profile.max_message_bytes())?; w.count(profile.max_stream_bytes())?;
        w.count(basis.output_start)?; w.u64(basis.control_sequence)?;
        w.u8(u8::from(basis.continuation))?; w.u8(u8::from(basis.view.finished()))?;
        w.count(basis.view.message_count())?;
        for message in basis.view.messages() { w.blob(message.as_bytes())?; }
        // The ORIGINAL restored text binding retains this exact offset. It is
        // not carried by GenerationKvCheckpoint and must be bound separately.
        w.count(self.broker.hosted_learned_stream_output_start()?)?;
        Ok(())
    }

    pub(in super::super::super::super) fn check_learned_stream_reset_intent(&self,
        intent: &FileLearnedResetIntent, first_incident_sequence: u64) -> Result<(), Error>
    {
        self.check_learned_reset_intent_kind(intent, true)?;
        self.check_learned_stream_reset_binding(intent, first_incident_sequence)
    }

    fn check_learned_stream_reset_binding(&self, intent: &FileLearnedResetIntent,
        first_incident_sequence: u64) -> Result<(), Error>
    {
        self.check_learned_stream_reset_boundary()?;
        let handle = &self.learned.as_ref().ok_or(Error::Incomplete)?.checkpoints.stream_captures
            .get(&intent.checkpoint).ok_or(Error::Missing)?.1;
        self.broker.check_hosted_learned_stream_checkpoint(handle, first_incident_sequence)
    }

    pub(in super::super::super::super) fn check_pending_learned_stream_reset(&self, operation: u64)
        -> Result<(), Error>
    {
        self.check_pending_learned_reset_kind(operation, true)?;
        let first = self.pending_learned_stream_reset_sequence().ok_or(Error::Binding)?;
        let intent = &self.pending_learned_reset().ok_or(Error::Incomplete)?.intent;
        self.check_learned_stream_reset_binding(intent, first)
    }

    pub(super) fn apply_learned_stream_reset_intent(&mut self, intent: &FileLearnedResetIntent,
        first_incident_sequence: u64) -> Result<Transition, Error>
    {
        self.check_learned_stream_reset_intent(intent, first_incident_sequence)?;
        let history = &mut self.learned.as_mut().expect("checked learned source").checkpoints.resets;
        history.pending = Some(PendingLearnedReset { intent: intent.clone(), interrupted: false });
        history.pending_stream = Some(first_incident_sequence);
        Ok(Transition::Unit)
    }

    pub(in super::super::super::super) fn learned_stream_reset_retry(&self,
        intent: &FileLearnedResetIntent, first_incident_sequence: u64)
        -> Result<Option<Result<HostedLearnedResetReceipt, Error>>, Error>
    {
        self.check_learned_reset_kind(intent.control.operation, true)?;
        let history = &self.learned.as_ref().ok_or(Error::Incomplete)?.checkpoints.resets;
        match history.completed.get(&intent.control.operation) {
            Some((prior, result)) if prior == intent
                && history.completed_stream.get(&intent.control.operation) == Some(&first_incident_sequence) =>
                    Ok(Some(result.clone())),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }

    pub(in super::super::super::super) fn prepare_learned_stream_reset(&mut self, operation: u64)
        -> Result<CheckpointEvent, Error>
    {
        let first = self.pending_learned_stream_reset_sequence().ok_or(Error::Binding)?;
        let (intent, result) = self.execute_learned_reset_kind(operation, Some(first))?;
        let witness = self.learned_stream_reset_witness(&intent, first, &result)?;
        self.record_learned_reset_kind(intent, result, witness.len(), Some(first))?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(CheckpointEvent::StreamReset { operation, witness: witness.into() })
    }

    pub(super) fn apply_learned_stream_reset(&mut self, operation: u64, expected: &[u8])
        -> Result<Transition, Error>
    {
        let first = self.pending_learned_stream_reset_sequence().ok_or(Error::Binding)?;
        let (intent, result) = self.execute_learned_reset_kind(operation, Some(first))?;
        let witness = self.learned_stream_reset_witness(&intent, first, &result)?;
        if witness.as_slice() != expected { return Err(Error::Binding); }
        self.record_learned_reset_kind(intent, result, witness.len(), Some(first))?;
        Ok(Transition::Unit)
    }

    fn learned_stream_reset_witness(&self, intent: &FileLearnedResetIntent,
        first_incident_sequence: u64, result: &Result<HostedLearnedResetReceipt, Error>) -> Result<Vec<u8>, Error>
    {
        let mut w = Writer::new(MAX_WITNESS_BYTES);
        w.raw(b"FALSRS\0\x01")?;
        w.u64(first_incident_sequence)?;
        // The legacy witness remains byte-for-byte stable and still binds ALL
        // original restart audit, work, accounting, control and result fields.
        w.blob(&self.learned_reset_witness(intent, result)?)?;
        self.write_learned_stream_basis(&mut w, intent.checkpoint)?;
        Ok(w.finish())
    }
}
