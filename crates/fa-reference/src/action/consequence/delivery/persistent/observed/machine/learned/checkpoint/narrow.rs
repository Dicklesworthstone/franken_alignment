//! Recompute original scoped narrowing; serialized receipts only compare it.
use super::{CheckpointEvent, DecoderEvent, Event, LearnedEvent, MAX_CHECKPOINTS,
    MAX_WITNESS_BYTES, Machine, Transition, Writer};
use super::super::super::super::decoder::learned::checkpoint::{
    FileLearnedStreamNarrowRequest, write_narrow_request};
use crate::action::consequence::gate::containment::CheckpointNarrowReceipt;
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedStreamNarrowRequest;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct NarrowHistory {
    pub(super) completed: BTreeMap<u64, (FileLearnedStreamNarrowRequest, CheckpointNarrowReceipt, Rc<[u8]>)>,
    bytes: usize,
}

impl Machine {
    pub(in super::super::super::super) fn learned_stream_narrow_retry(&self,
        request: &FileLearnedStreamNarrowRequest) -> Result<Option<(CheckpointNarrowReceipt, Rc<[u8]>)>, Error>
    {
        request.validate()?;
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        if state.checkpoints.resets.completed.contains_key(&request.control().operation)
            || state.checkpoints.resets.pending.as_ref()
                .is_some_and(|pending| pending.intent.control().operation == request.control().operation) {
            return Err(Error::Binding);
        }
        match state.checkpoints.narrows.completed.get(&request.control().operation) {
            Some((prior, receipt, witness)) if prior == request => Ok(Some((receipt.clone(), Rc::clone(witness)))),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }

    pub(in super::super::super::super) fn check_learned_stream_narrow(&self,
        request: &FileLearnedStreamNarrowRequest) -> Result<(), Error>
    {
        request.validate()?;
        if self.mediation.is_some()
            || !self.learned_contract().is_some_and(|recipe| recipe.requires_owned_forecast_stream_restart_audit()) {
            return Err(Error::Binding);
        }
        if self.learned_stream_narrow_retry(request)?.is_some() { return Err(Error::Duplicate); }
        if self.pending_learned_step().is_some() || self.pending_learned_reset().is_some() {
            return Err(Error::Incomplete);
        }
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        if state.paused || !self.clock_ready { return Err(Error::Incomplete); }
        if state.checkpoints.narrows.completed.len() >= MAX_CHECKPOINTS
            || state.checkpoints.narrows.bytes >= MAX_WITNESS_BYTES { return Err(Error::Limit); }
        let info = request.checkpoint();
        if self.learned_stream_checkpoint_info(info.checkpoint)? != info { return Err(Error::Binding); }
        self.check_learned_stream_reset_boundary()?;
        let actual = self.broker.hosted_learned_generation()?;
        if actual.actor_revision != info.actor_revision || actual.position != info.position
            || actual.sampled_draws != info.sampled_draws
            || self.broker.hosted_learned_observation()?.stream() != info.stream { return Err(Error::Binding); }
        self.broker.check_hosted_learned_stream_narrow(&self.native_stream_narrow_request(request)?)
    }

    fn native_stream_narrow_request(&self, request: &FileLearnedStreamNarrowRequest)
        -> Result<HostedLearnedStreamNarrowRequest, Error>
    {
        let checkpoint = self.learned.as_ref().ok_or(Error::Incomplete)?.checkpoints.stream_captures
            .get(&request.checkpoint().checkpoint).ok_or(Error::Missing)?.1.clone();
        let c = request.control();
        Ok(HostedLearnedStreamNarrowRequest { checkpoint,
            expected_control_sequence: c.expected_control_sequence,
            expected_actor_revision: c.expected_actor_revision,
            expected_authority_epoch: c.expected_authority_epoch, binding: c.binding,
            retained_targets: c.retained_targets.clone(),
        })
    }

    pub(in super::super::super::super) fn prepare_learned_stream_narrow(&mut self,
        request: &FileLearnedStreamNarrowRequest) -> Result<CheckpointEvent, Error>
    {
        self.check_decoder_admission(&Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
            CheckpointEvent::StreamNarrow { request: Rc::new(request.clone()), witness: Rc::from(&b""[..]) }))))?;
        let receipt = self.execute_learned_stream_narrow(request)?;
        let witness: Rc<[u8]> = self.learned_stream_narrow_witness(request, &receipt)?.into();
        self.record_learned_stream_narrow(request, receipt, Rc::clone(&witness))?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(CheckpointEvent::StreamNarrow { request: Rc::new(request.clone()), witness })
    }

    pub(super) fn apply_learned_stream_narrow(&mut self, request: &FileLearnedStreamNarrowRequest,
        expected: &[u8]) -> Result<Transition, Error>
    {
        let receipt = self.execute_learned_stream_narrow(request)?;
        let witness = self.learned_stream_narrow_witness(request, &receipt)?;
        if witness.as_slice() != expected { return Err(Error::Binding); }
        self.record_learned_stream_narrow(request, receipt, witness.into())?;
        Ok(Transition::Unit)
    }

    fn execute_learned_stream_narrow(&mut self, request: &FileLearnedStreamNarrowRequest)
        -> Result<CheckpointNarrowReceipt, Error>
    {
        self.check_learned_stream_narrow(request)?;
        let native = self.native_stream_narrow_request(request)?;
        let receipt = self.broker.narrow_hosted_learned_stream(native)?;
        self.withdraw_keys()?;
        self.sessions.clear();
        self.automatic.clear();
        // Keep envelopes, endpoint outcomes, statistical evidence and every
        // numerical/source/audit budget; narrowing cannot un-dispatch them.
        Ok(receipt)
    }

    fn record_learned_stream_narrow(&mut self, request: &FileLearnedStreamNarrowRequest,
        receipt: CheckpointNarrowReceipt, witness: Rc<[u8]>) -> Result<(), Error>
    {
        let history = &mut self.learned.as_mut().expect("checked learned source").checkpoints.narrows;
        let bytes = history.bytes.checked_add(witness.len()).ok_or(Error::Limit)?;
        if bytes > MAX_WITNESS_BYTES { return Err(Error::Limit); }
        if history.completed.contains_key(&request.control().operation) { return Err(Error::Duplicate); }
        history.completed.insert(request.control().operation, (request.clone(), receipt, witness));
        history.bytes = bytes;
        Ok(())
    }

    fn learned_stream_narrow_witness(&self, request: &FileLearnedStreamNarrowRequest,
        receipt: &CheckpointNarrowReceipt) -> Result<Vec<u8>, Error>
    {
        let mut w = Writer::new(MAX_WITNESS_BYTES);
        w.raw(b"FALSNW\0\x01")?;
        write_narrow_request(&mut w, request)?;
        self.write_learned_state(&mut w)?;
        self.write_learned_recovery_state(&mut w)?;
        self.write_learned_stream_basis(&mut w, request.checkpoint().checkpoint)?;
        w.scope(receipt.scope)?;
        for value in [receipt.sequence, receipt.binding.round, receipt.binding.reducer_generation,
            receipt.checkpoint, receipt.checkpoint_actor_revision, receipt.checkpoint_control_sequence,
            receipt.actor_revision, receipt.incident_count, receipt.refunded_units, receipt.revocation_floor] {
            w.u64(value)?;
        }
        w.raw(&receipt.binding.evidence_root)?; w.raw(&receipt.consequence.encode())?;
        w.count(receipt.cancelled.len())?;
        for id in &receipt.cancelled { w.u64(*id)?; }
        w.count(request.control().retained_targets.len())?;
        for target in &request.control().retained_targets { w.u8(u8::from(receipt.ceiling.contains(*target)))?; }
        Ok(w.finish())
    }
}
