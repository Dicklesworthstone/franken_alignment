//! Original reset execution, comparison-only receipts and an unresolved-work barrier.
use super::{Machine, Transition, CheckpointEvent, Event, DecoderEvent, LearnedEvent,
    Writer, MAX_WITNESS_BYTES, MAX_CHECKPOINTS, generation_work, telemetry};
use super::super::super::super::{BaseEvent, journal::HumanDecision,
    decoder::learned::checkpoint::{FileLearnedResetIntent, PendingLearnedReset, write_intent}};
use super::super::witness::{report, status};
use crate::action::consequence::activation::monitor::decoder::sampled::host::replay::error_tag;
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::oversight::learned_host::checkpoint::{
    HostedLearnedResetRequest, HostedLearnedResetReceipt,
};
use crate::action::consequence::oversight::learned_source::LearnedAvailability;
use crate::Error;
use std::collections::BTreeMap;

/// Aggregate logical serialized receipt/state material, not allocator or RSS.
/// The canonical journal's independent byte bound still applies to every event.
const MAX_RESET_RECORD_BYTES: usize = MAX_WITNESS_BYTES;
#[derive(Default)]
pub(super) struct ResetHistory {
    pending: Option<PendingLearnedReset>,
    completed: BTreeMap<u64, (FileLearnedResetIntent, Result<HostedLearnedResetReceipt, Error>)>,
    bytes: usize,
}

impl Machine {
    pub(in super::super::super::super) fn pending_learned_reset(&self) -> Option<&PendingLearnedReset> {
        self.learned.as_ref().and_then(|state| state.checkpoints.resets.pending.as_ref())
    }
    pub(in super::super::super) fn interrupt_learned_reset(&mut self) {
        if let Some(state) = &mut self.learned {
            if let Some(pending) = &mut state.checkpoints.resets.pending { pending.interrupted = true; }
        }
    }
    pub(in super::super::super::super) fn learned_reset_result_optional(&self, operation: u64)
        -> Result<Option<Result<HostedLearnedResetReceipt, Error>>, Error>
    {
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        Ok(state.checkpoints.resets.completed.get(&operation).map(|(_, result)| result.clone()))
    }
    pub(in super::super::super::super) fn learned_reset_retry(&self, intent: &FileLearnedResetIntent)
        -> Result<Option<Result<HostedLearnedResetReceipt, Error>>, Error>
    {
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        match state.checkpoints.resets.completed.get(&intent.control.operation) {
            Some((prior, result)) if prior == intent => Ok(Some(result.clone())),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }

    /// This guard runs in the ORIGINAL numerical admission path, including
    /// semantic replay. All other source, identity and policy gates still apply.
    pub(in super::super::super) fn learned_reset_admission(&self, event: &Event) -> Option<Result<(), Error>> {
        if self.pending_learned_reset().is_some() {
            if let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::Reset { operation, .. }))) = event {
                return Some(self.check_pending_learned_reset(*operation));
            }
            // No new inference, report, review, approval, dispatch, publication
            // or replacement intent can erase an unknown reset. These original
            // restrictive/settlement operations do not manufacture completion.
            return Some(if matches!(event,
                Event::Core(BaseEvent::Time(_) | BaseEvent::Cancel(_) | BaseEvent::Fence
                    | BaseEvent::Stop(_) | BaseEvent::StopProgress(_) | BaseEvent::Reconcile(_)
                    | BaseEvent::Seal(_) | BaseEvent::Sweep | BaseEvent::ReplacePolicy(_)
                    | BaseEvent::ReserveRecovery(_))
                | Event::InputsUnavailable(..) | Event::RevokeHumans
                | Event::Human(_, HumanDecision::Reject | HumanDecision::Revoke)
                | Event::Source(_) | Event::Identity(_) | Event::Campaign(_)
                | Event::CredentialRotate(_) | Event::CredentialRevoke(_)) { Ok(()) }
                else { Err(Error::Incomplete) });
        }
        match event {
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::BeginReset(intent)))) => Some(self.check_learned_reset_intent(intent)),
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::Reset { .. }))) => Some(Err(Error::Incomplete)),
            _ => None,
        }
    }

    pub(in super::super::super::super) fn check_learned_reset_intent(&self, intent: &FileLearnedResetIntent)
        -> Result<(), Error>
    {
        intent.validate()?;
        let state = self.learned.as_ref().ok_or(Error::Incomplete)?;
        let history = &state.checkpoints.resets;
        if history.pending.is_some() || self.pending_learned_step().is_some() { return Err(Error::Incomplete); }
        if history.completed.contains_key(&intent.control.operation) { return Err(Error::Duplicate); }
        if history.completed.len() >= MAX_CHECKPOINTS || history.bytes >= MAX_RESET_RECORD_BYTES { return Err(Error::Limit); }
        if !state.checkpoints.captures.contains_key(&intent.checkpoint) { return Err(Error::Missing); }
        self.check_learned_reset_predecessor(intent)
    }
    fn check_learned_reset_predecessor(&self, intent: &FileLearnedResetIntent) -> Result<(), Error> {
        if !self.clock_ready { return Err(Error::Incomplete); }
        let control = self.broker.inspect(); let actual = self.broker.hosted_learned_generation()?;
        if intent.control.expected_control_sequence != control.sequence
            || intent.control.expected_authority_epoch != control.ledger.epoch
            || intent.control.expected_actor_revision != actual.actor_revision { return Err(Error::Stale); }
        if actual.host_failure.is_some() || control.suspended || self.broker.stop_receipt().is_some()
            || !matches!(actual.availability, LearnedAvailability::Ready | LearnedAvailability::Held)
        { return Err(Error::WrongState); }
        // An explicit fresh instruction can reset a recovered held owner while
        // it is paused. Restoration preserves that pause; only separate original
        // resume, after successful restore, can admit new computation/effects.
        Ok(())
    }
    pub(in super::super::super::super) fn check_pending_learned_reset(&self, operation: u64) -> Result<(), Error> {
        let pending = self.pending_learned_reset().ok_or(Error::Incomplete)?;
        if pending.intent.control.operation != operation { return Err(Error::Binding); }
        if pending.interrupted { return Err(Error::WrongState); }
        if self.pending_learned_step().is_some() { return Err(Error::Incomplete); }
        self.check_learned_reset_predecessor(&pending.intent)
    }
    pub(super) fn apply_learned_reset_intent(&mut self, intent: &FileLearnedResetIntent) -> Result<Transition, Error> {
        self.check_learned_reset_intent(intent)?;
        self.learned.as_mut().expect("checked learned source").checkpoints.resets.pending =
            Some(PendingLearnedReset { intent: intent.clone(), interrupted: false });
        Ok(Transition::Unit)
    }
    pub(in super::super::super::super) fn prepare_learned_reset(&mut self, operation: u64)
        -> Result<CheckpointEvent, Error>
    {
        let (intent, result) = self.execute_learned_reset(operation)?;
        let witness = self.learned_reset_witness(&intent, &result)?;
        self.record_learned_reset(intent, result, witness.len())?;
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok(CheckpointEvent::Reset { operation, witness: witness.into() })
    }
    pub(super) fn apply_learned_reset(&mut self, operation: u64, expected: &[u8]) -> Result<Transition, Error> {
        let (intent, result) = self.execute_learned_reset(operation)?;
        let witness = self.learned_reset_witness(&intent, &result)?;
        if witness.as_slice() != expected { return Err(Error::Binding); }
        self.record_learned_reset(intent, result, witness.len())?;
        Ok(Transition::Unit)
    }
    fn execute_learned_reset(&mut self, operation: u64)
        -> Result<(FileLearnedResetIntent, Result<HostedLearnedResetReceipt, Error>), Error>
    {
        self.check_pending_learned_reset(operation)?;
        let intent = self.pending_learned_reset().expect("checked reset intent").intent.clone();
        let handle = self.learned.as_ref().expect("checked learned source").checkpoints.captures
            .get(&intent.checkpoint).ok_or(Error::Missing)?.1.clone();
        let before = self.broker.hosted_learned_recovery_usage()?;
        let sequence = self.broker.inspect().sequence;
        let result = self.broker.reset_hosted_learned(HostedLearnedResetRequest {
            checkpoint: handle, expected_control_sequence: intent.control.expected_control_sequence,
            expected_actor_revision: intent.control.expected_actor_revision,
            expected_authority_epoch: intent.control.expected_authority_epoch, binding: intent.control.binding,
            retained_targets: TargetCeiling::new(&intent.control.retained_targets)?, restart_budget: intent.budget,
        });
        if let Some(receipt) = self.broker.stop_receipt() {
            let request = receipt.request();
            self.apply_core(&BaseEvent::Stop(request))?;
        } else if result.is_ok() || before != self.broker.hosted_learned_recovery_usage()?
            || sequence != self.broker.inspect().sequence {
            self.withdraw_keys()?;
            self.sessions.clear();
            self.automatic.clear();
        }
        // The original authority alone decides refunds and escalation. Keep
        // historical sidecars and sent envelopes for original reconciliation;
        // changed source/epoch invalidates old evidence without erasing its cost.
        Ok((intent, result))
    }
    fn record_learned_reset(&mut self, intent: FileLearnedResetIntent,
        result: Result<HostedLearnedResetReceipt, Error>, bytes: usize) -> Result<(), Error>
    {
        let history = &mut self.learned.as_mut().expect("checked learned source").checkpoints.resets;
        let total = history.bytes.checked_add(bytes).ok_or(Error::Limit)?;
        if total > MAX_RESET_RECORD_BYTES { return Err(Error::Limit); }
        if history.completed.contains_key(&intent.control.operation) { return Err(Error::Duplicate); }
        history.completed.insert(intent.control.operation, (intent, result));
        history.bytes = total;
        history.pending = None;
        Ok(())
    }
    fn learned_reset_witness(&self, intent: &FileLearnedResetIntent,
        result: &Result<HostedLearnedResetReceipt, Error>) -> Result<Vec<u8>, Error>
    {
        let mut w = Writer::new(MAX_WITNESS_BYTES);
        w.raw(b"FALRST\0\x01")?;
        write_intent(&mut w, intent)?;
        self.write_learned_state(&mut w)?;
        self.write_learned_recovery_state(&mut w)?;
        match result {
            Err(error) => { w.u8(0)?; w.u8(error_tag(*error))?; }
            Ok(receipt) => {
                w.u8(1)?;
                let c = &receipt.control;
                w.scope(c.scope)?;
                for value in [c.sequence, c.binding.round, c.binding.reducer_generation,
                    c.checkpoint, c.checkpoint_actor_revision, c.checkpoint_control_sequence,
                    c.actor_revision, c.incident_count, c.refunded_units, c.revocation_floor,
                    receipt.actor_revision, receipt.position, receipt.sampled_draws] { w.u64(value)?; }
                w.raw(&c.binding.evidence_root)?; w.raw(&c.consequence.encode())?;
                w.u8(u8::from(c.restored))?;
                w.count(c.cancelled.len())?; for id in &c.cancelled { w.u64(*id)?; }
                w.count(intent.control.retained_targets.len())?;
                for target in &intent.control.retained_targets { w.u8(u8::from(c.ceiling.contains(*target)))?; }
                match receipt.resumed_stream { None => w.u8(0)?, Some(stream) => { w.u8(1)?; w.u64(stream)?; } }
                let g = &receipt.restart;
                status(&mut w, g.status())?; w.u64(g.sampler_draws())?;
                generation_work(&mut w, g.historical_work())?;
                telemetry(&mut w, g.historical_telemetry())?;
                let kv = g.kv(); let restored = kv.restoration();
                w.u64(kv.evaluation_origin())?; w.u64(restored.resumed_stream)?; w.u64(restored.position)?;
                for value in [restored.values_restored, restored.bytes_written, restored.bytes_recaptured,
                    restored.staged_write_bytes] { w.count(value)?; }
                let audit = kv.audit().ok_or(Error::Incomplete)?;
                // The serialized original checked source binds the restorer's
                // exact SOURCE descriptor too, without relabeling its old stream.
                if audit.monitoring().source().descriptor() != &restored.source { return Err(Error::Binding); }
                w.count(audit.compression().encoded_bytes)?; w.u64(audit.compression().work_units_reserved)?;
                report(&mut w, audit.monitoring())?;
            }
        }
        Ok(w.finish())
    }
}
