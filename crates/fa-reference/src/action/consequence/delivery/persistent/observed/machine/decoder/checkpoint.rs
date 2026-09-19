//! Retain original paired handles and exact request/results, never another
//! checkpoint evaluator, balance ledger or saved-state import implementation.
use super::{Machine, DecoderEvent, Writer, MAX_WITNESS_BYTES, error_tag};
use super::super::super::decoder::checkpoint::{CheckpointRequest, FileDecoderCheckpointInfo};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::MAX_CHECKPOINTS;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderWork;
use crate::action::consequence::oversight::decoder_host::{HostedCheckpointHandle, HostedResetRequest, HostedResetReceipt};
use crate::Error;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct CheckpointHistory {
    captures: BTreeMap<u64, (FileDecoderCheckpointInfo, HostedCheckpointHandle)>,
    resets: BTreeMap<u64, (CheckpointRequest, Result<HostedResetReceipt, Error>)>,
}

impl Machine {
    pub(in super::super::super) fn decoder_experiment_source(&self, id: u64)
        -> Result<&crate::action::consequence::activation::tensor::kv::decoder::DecoderCheckpoint, Error>
    {
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        let (_, native) = state.checkpoints.captures.get(&id).ok_or(Error::Missing)?;
        self.broker.hosted_experiment_source(native)
    }

    pub(in super::super::super) fn decoder_checkpoint_info(&self, id: u64) -> Result<FileDecoderCheckpointInfo, Error> {
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        Ok(state.checkpoints.captures.get(&id).ok_or(Error::Missing)?.0.clone())
    }
    pub(in super::super::super) fn decoder_reset_result(&self, operation: u64)
        -> Result<Result<HostedResetReceipt, Error>, Error>
    {
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        Ok(state.checkpoints.resets.get(&operation).ok_or(Error::Missing)?.1.clone())
    }
    pub(in super::super::super) fn decoder_reset_retry(&self, request: &CheckpointRequest)
        -> Result<Option<Result<HostedResetReceipt, Error>>, Error>
    {
        let CheckpointRequest::Reset { control, .. } = request else { return Err(Error::InvalidInput); };
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        match state.checkpoints.resets.get(&control.operation) {
            Some((original, result)) if original == request => Ok(Some(result.clone())),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }

    pub(in super::super::super) fn check_decoder_checkpoint_request(&self, request: &CheckpointRequest) -> Result<(), Error> {
        request.validate()?;
        // Both live preparation and semantic replay must preserve the exact
        // interrupted generation. A rewind cannot erase or substitute its input.
        if self.pending_decoder_generation().is_some() { return Err(Error::Incomplete); }
        if !self.clock_ready { return Err(Error::Incomplete); }
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        match request {
            CheckpointRequest::Capture { checkpoint, epoch, .. } => {
                if state.paused { return Err(Error::Incomplete); }
                if *epoch != self.broker.inspect().ledger.epoch { return Err(Error::Stale); }
                if state.checkpoints.captures.contains_key(checkpoint) { return Err(Error::Duplicate); }
                if state.checkpoints.captures.len() >= MAX_CHECKPOINTS { return Err(Error::Limit); }
            }
            CheckpointRequest::Reset { checkpoint, control, .. } => {
                if !state.checkpoints.captures.contains_key(checkpoint) { return Err(Error::Missing); }
                if state.checkpoints.resets.contains_key(&control.operation) { return Err(Error::Duplicate); }
                // Bound all retained outcomes, including preflight refusals which
                // do not consume the native numerical replay-attempt allowance.
                if state.checkpoints.resets.len() >= MAX_CHECKPOINTS { return Err(Error::Limit); }
            }
        }
        Ok(())
    }

    pub(in super::super::super) fn prepare_decoder_checkpoint(&mut self, request: CheckpointRequest)
        -> Result<DecoderEvent, Error>
    {
        self.execute_decoder_checkpoint(&request)?;
        let expected = self.decoder_checkpoint_witness(&request)?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(DecoderEvent::Checkpoint(request, expected.into()))
    }

    pub(super) fn execute_decoder_checkpoint(&mut self, request: &CheckpointRequest) -> Result<(), Error> {
        self.check_decoder_checkpoint_request(request)?;
        match request {
            CheckpointRequest::Capture { checkpoint, actor_revision, epoch } => {
                let handle = self.broker.capture_hosted_checkpoint(*checkpoint, *actor_revision)?;
                let n = self.broker.hosted_decoder()?;
                let info = FileDecoderCheckpointInfo { checkpoint: *checkpoint, actor_revision: *actor_revision,
                    authority_epoch: *epoch, control_sequence: self.broker.inspect().sequence,
                    position: n.position, sampled_draws: n.sampled_draws };
                self.decoder.as_mut().expect("checked decoder").checkpoints.captures.insert(*checkpoint, (info, handle));
            }
            CheckpointRequest::Reset { checkpoint, control, budget } => {
                let handle = self.decoder.as_ref().expect("checked decoder")
                    .checkpoints.captures[checkpoint].1.clone();
                let before = self.broker.hosted_recovery_usage()?;
                let sequence = self.broker.inspect().sequence;
                let already_stopped = self.broker.stop_receipt().is_some();
                let result = self.broker.reset_hosted_decoder(HostedResetRequest {
                    checkpoint: handle, expected_control_sequence: control.expected_control_sequence,
                    expected_actor_revision: control.expected_actor_revision,
                    expected_authority_epoch: control.expected_authority_epoch, binding: control.binding,
                    retained_targets: TargetCeiling::new(&control.retained_targets)?, replay_budget: *budget,
                });
                self.finish_decoder_stop(already_stopped)?;
                if self.broker.stop_receipt().is_none() && (result.is_ok()
                    || before != self.broker.hosted_recovery_usage()?
                    || sequence != self.broker.inspect().sequence) {
                    // A failed admitted replay can poison the original numerical
                    // source. Keep its failure/cost and retire stale review keys.
                    self.withdraw_keys()?;
                    self.sessions.clear();
                    self.automatic.clear();
                }
                // The original controller alone decides cancellation/refunds.
                // Keep envelopes for guarded sealing and original reconciliation;
                // never erase an unknown charge or an already published outcome.
                self.decoder.as_mut().expect("checked decoder").checkpoints.resets
                    .insert(control.operation, (request.clone(), result));
                // Deliberately preserve paused: reset cannot bypass explicit
                // post-recovery resume, identity/source revalidation or stop.
            }
        }
        Ok(())
    }

    pub(super) fn decoder_checkpoint_witness(&self, request: &CheckpointRequest) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_WITNESS_BYTES);
        w.raw(b"FADCP\0\0\x01")?;
        w.blob(&self.broker.hosted_replay_bytes()?)?;
        let actor = self.broker.retained_actor_state();
        w.u64(self.broker.actor_revision())?; w.u64(actor.next_position())?;
        w.count(actor.tokens().len())?;
        for token in actor.tokens() { w.u32(*token)?; }
        w.blob(actor.cache())?; w.blob(actor.sampler())?;
        w.u8(u8::from(self.decoder_paused()))?;
        let usage = self.broker.hosted_recovery_usage()?;
        for value in [usage.checkpoints, usage.checkpoint_bytes, usage.replay_attempts] { w.count(value)?; }
        w.u64(usage.admitted_products)?;
        write_work(&mut w, self.broker.hosted_decoder()?.numerical)?;
        match request {
            CheckpointRequest::Capture { checkpoint, .. } => {
                w.u8(0)?;
                let info = self.decoder_checkpoint_info(*checkpoint)?;
                for value in [info.checkpoint, info.actor_revision, info.authority_epoch,
                    info.control_sequence, info.position, info.sampled_draws] { w.u64(value)?; }
            }
            CheckpointRequest::Reset { control, .. } => match self.decoder_reset_result(control.operation)? {
                Err(error) => { w.u8(1)?; w.u8(error_tag(error))?; }
                Ok(receipt) => {
                    w.u8(2)?;
                    let c = &receipt.control;
                    w.scope(c.scope)?;
                    for value in [c.sequence, c.binding.round, c.binding.reducer_generation,
                        c.checkpoint, c.checkpoint_actor_revision, c.checkpoint_control_sequence,
                        c.actor_revision, c.incident_count, c.refunded_units, c.revocation_floor,
                        receipt.actor_revision, receipt.position, receipt.sampled_draws] { w.u64(value)?; }
                    w.raw(&c.binding.evidence_root)?; w.raw(&c.consequence.encode())?;
                    w.u8(u8::from(c.restored))?;
                    w.count(c.cancelled.len())?; for id in &c.cancelled { w.u64(*id)?; }
                    // The original reset intersects its old ceiling with the
                    // requested set. Encode that exact subset in request order;
                    // this is comparison only, never a decoded target grant.
                    w.count(control.retained_targets.len())?;
                    for target in &control.retained_targets { w.u8(u8::from(c.ceiling.contains(*target)))?; }
                    match receipt.resumed_stream { None => w.u8(0)?, Some(stream) => { w.u8(1)?; w.u64(stream)?; } }
                    write_work(&mut w, receipt.replay_numerical)?;
                    w.u64(receipt.monitoring.frame_reviews)?;
                    for count in [receipt.monitoring.encoded_bytes, receipt.monitoring.probe_coordinates,
                        receipt.monitoring.codec_coordinates] { w.count(count)?; }
                }
            },
        }
        self.write_decoder_stop_witness(&mut w)?;
        Ok(w.finish())
    }
}
fn write_work(w: &mut Writer, work: DecoderWork) -> Result<(), Error> {
    for value in [work.tokens, work.matrix_products, work.attention_products,
        work.attention_exponentials, work.normalization_coordinates, work.rotary_pairs,
        work.gate_coordinates, work.cache_values_appended] { w.u64(value)?; }
    Ok(())
}
