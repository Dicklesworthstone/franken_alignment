//! Rebuild original paired handles and compare state; no checkpoint importer.
use super::{Machine, Transition};
use super::super::super::decoder::{DecoderEvent, MAX_WITNESS_BYTES, learned::{LearnedEvent,
    checkpoint::{CheckpointEvent, FileLearnedCheckpointInfo}}};
use super::super::super::Event;
use super::super::super::super::codec::shared::Writer;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationTelemetryWork, GenerationWork,
};
use crate::action::consequence::gate::containment::MAX_CHECKPOINTS;
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedCheckpointHandle;
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

mod reset;

#[derive(Default)]
pub(super) struct CheckpointHistory {
    resets: reset::ResetHistory,
    captures: BTreeMap<u64, (FileLearnedCheckpointInfo, HostedLearnedCheckpointHandle)>,
}

impl Machine {
    pub(in super::super::super) fn learned_checkpoint_info(&self, id: u64)
        -> Result<FileLearnedCheckpointInfo, Error>
    {
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        Ok(state.checkpoints.captures.get(&id).ok_or(Error::Missing)?.0)
    }
    pub(in super::super::super) fn check_learned_capture(&self, id: u64, revision: u64, epoch: u64)
        -> Result<(), Error>
    {
        if id == 0 { return Err(Error::InvalidInput); }
        if self.pending_learned_step().is_some() || self.pending_learned_reset().is_some() { return Err(Error::Incomplete); }
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        if state.paused || !self.clock_ready { return Err(Error::Incomplete); }
        if state.checkpoints.captures.contains_key(&id) { return Err(Error::Duplicate); }
        if state.checkpoints.captures.len() >= MAX_CHECKPOINTS { return Err(Error::Limit); }
        let actual = self.broker.hosted_learned_generation()?;
        if actual.actor_revision != revision || self.broker.inspect().ledger.epoch != epoch { return Err(Error::Stale); }
        if actual.position == 0 || !actual.status.is_active() || actual.host_failure.is_some()
            || actual.availability != LearnedAvailability::Ready || self.broker.inspect().suspended
            || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
        Ok(())
    }
    pub(in super::super::super) fn prepare_learned_capture(&mut self, id: u64, revision: u64, epoch: u64)
        -> Result<CheckpointEvent, Error>
    {
        self.check_decoder_admission(&Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
            CheckpointEvent::Capture { checkpoint: id, actor_revision: revision, epoch, witness: Rc::from(&b""[..]) }))))?;
        self.execute_learned_capture(id, revision, epoch)?;
        let witness = self.learned_capture_witness(id)?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(CheckpointEvent::Capture { checkpoint: id, actor_revision: revision, epoch, witness: witness.into() })
    }
    pub(super) fn apply_learned_checkpoint(&mut self, event: &CheckpointEvent) -> Result<Transition, Error> {
        match event {
            CheckpointEvent::BeginReset(intent) => self.apply_learned_reset_intent(intent),
            CheckpointEvent::Reset { operation, witness } => self.apply_learned_reset(*operation, witness),
            CheckpointEvent::Capture { checkpoint, actor_revision, epoch, witness } => {
                self.execute_learned_capture(*checkpoint, *actor_revision, *epoch)?;
                if self.learned_capture_witness(*checkpoint)?.as_slice() != witness.as_ref() { return Err(Error::Binding); }
                Ok(Transition::Unit)
            }
        }
    }
    fn execute_learned_capture(&mut self, id: u64, revision: u64, epoch: u64) -> Result<(), Error> {
        self.check_learned_capture(id, revision, epoch)?;
        let native = self.broker.capture_hosted_learned_checkpoint(id, revision)?;
        let actual = self.broker.hosted_learned_generation()?;
        let info = FileLearnedCheckpointInfo { checkpoint: id, actor_revision: revision, authority_epoch: epoch,
            control_sequence: self.broker.inspect().sequence, position: actual.position,
            stream: self.broker.hosted_learned_observation()?.stream(), sampled_draws: actual.sampled_draws };
        self.learned.as_mut().expect("checked learned source").checkpoints.captures.insert(id, (info, native));
        Ok(())
    }
    fn learned_capture_witness(&self, id: u64) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_WITNESS_BYTES);
        w.raw(b"FALCP\0\0\x01")?;
        self.write_learned_state(&mut w)?;
        self.write_learned_recovery_state(&mut w)?;
        let info = self.learned_checkpoint_info(id)?;
        for value in [info.checkpoint, info.actor_revision, info.authority_epoch, info.control_sequence,
            info.stream, info.position, info.sampled_draws] { w.u64(value)?; }
        Ok(w.finish())
    }
    fn write_learned_recovery_state(&self, w: &mut Writer) -> Result<(), Error> {
        let original = self.broker.hosted_learned_original()?;
        let actual = self.broker.hosted_learned_generation()?;
        let budget = original.budget(); let t = original.telemetry_budget();
        w.u64(budget.decoder_products)?; w.u64(budget.vocabulary_scores)?;
        for value in [t.compression_source_values, t.compression_encoded_bytes, t.compression_work_units,
            t.source_check_values, t.source_check_encoded_bytes, t.source_check_reconstruction_products,
            t.monitor_encoded_bytes, t.monitor_probe_coordinates, t.monitor_reconstruction_products,
            t.monitor_materialized_values, t.monitor_refinements] { w.u64(value)?; }
        generation_work(w, actual.cumulative_work)?;
        telemetry(w, actual.cumulative_telemetry)?;
        let usage = self.broker.hosted_learned_recovery_usage()?;
        w.count(usage.checkpoints)?; w.count(usage.checkpoint_bytes)?; w.count(usage.restart_attempts)?;
        w.u64(usage.reserved_cache_values)?;
        telemetry(w, usage.reserved_audit)?; telemetry(w, usage.completed_audit)?;
        for value in [usage.values_restored, usage.bytes_written, usage.bytes_recaptured, usage.staged_write_bytes] { w.u64(value)?; }
        w.u64(self.broker.incident_count())?;
        w.u64(self.broker.inspect().sequence)?; w.u64(self.broker.inspect().ledger.epoch)?;
        Ok(())
    }
}
fn generation_work(w: &mut Writer, g: GenerationWork) -> Result<(), Error> {
    let d = g.accepted_decoder;
    for value in [g.admitted_tokens, g.reserved_decoder_products, g.sampling_attempts,
        g.reserved_vocabulary_scores, d.tokens, d.matrix_products, d.attention_products,
        d.attention_exponentials, d.normalization_coordinates, d.rotary_pairs,
        d.gate_coordinates, d.cache_values_appended] { w.u64(value)?; }
    Ok(())
}
fn telemetry(w: &mut Writer, t: GenerationTelemetryWork) -> Result<(), Error> {
    for value in [t.compression_source_values, t.compression_encoded_bytes, t.compression_work_units,
        t.source_check_values, t.source_check_encoded_bytes, t.source_check_reconstruction_products,
        t.monitor_encoded_bytes, t.monitor_probe_coordinates, t.monitor_reconstruction_products,
        t.monitor_materialized_values, t.monitor_refinements] { w.u64(value)?; }
    Ok(())
}
