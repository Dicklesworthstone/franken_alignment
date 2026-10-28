//! Explicit recovery of an acknowledged reset intent before the recovery fence.
//! Only the original command, original reset and original canonical cut are used.
use super::{BaseEvent, Event, FileHumanReviewer, FileLearnedRecovery, FileLearnedRecoveryStatus,
    FileOversight, JournalError, journal};
use super::super::{DecoderEvent, LearnedEvent, checkpoint::{CheckpointEvent, FileLearnedResetIntent}};
use crate::action::consequence::delivery::persistent::JournalCapacity;
use crate::action::consequence::delivery::persistent::observed::containment::FileResetRequest;
use crate::action::consequence::activation::tensor::kv::decoder::monitoring::restart::KvRestartBudget;
use crate::Error;
use std::rc::Rc;

impl FileLearnedResetIntent {
    /// Independently retain the exact intended instruction for recovery. This
    /// validates DATA only: it captures no checkpoint, records no intent, and
    /// cannot supply a native handle or install a replacement numerical state.
    pub fn for_recovery(checkpoint: u64, control: FileResetRequest, budget: KvRestartBudget)
        -> Result<Self, Error>
    {
        let intent = Self { checkpoint, control, budget };
        intent.validate()?;
        Ok(intent)
    }
}

impl FileLearnedRecovery {
    /// Complete one independently selected, already journaled reset at its EXACT
    /// original predecessor, then publish completion and the recovery fence in
    /// ONE canonical replacement. No intermediate live owner or old key escapes.
    ///
    /// Full history replay must be Ready. Already fenced/stopped intents and
    /// changed commands refuse; there is no rebase to a newer authority epoch.
    /// An original reset refusal is recorded too, never replaced with success.
    /// The returned owner is paused and requires fresh time and original resume.
    /// Query learned_reset_result on that owner for the acknowledged result.
    ///
    /// After an ambiguous write, an exact already-completed command takes only
    /// the usual fence path. It cannot repeat an audit or count another incident.
    /// Ordinary finish/open retain their original quarantine-only behavior.
    /// This consumes the recovery on failure/unwind, releasing its exclusive lock
    /// without exposing candidate state. Original replay/audit and sync remain
    /// synchronous; logical work is not a bound on repeated physical recovery.
    ///
    /// ```compile_fail,E0382
    /// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{
    ///     FileLearnedRecovery, checkpoint::FileLearnedResetIntent,
    /// };
    /// fn retry_candidate(recovery: FileLearnedRecovery, intent: &FileLearnedResetIntent) {
    ///     let _ = recovery.finish_pending_reset(intent);
    ///     let _ = recovery.finish();
    /// }
    /// ```
    pub fn finish_pending_reset(self, expected: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        self.prepare_pending_reset(expected)?.finish()
    }

    /// Complete one exact, acknowledged STREAM reset before publishing the
    /// original recovery fence. The independently retained command and incident
    /// declaration must both match the original stream-kind intent. The current
    /// target, confirmed publication boundary and text offset remain unchanged.
    ///
    /// Full original numerical replay must be Ready. A prior Fence, Stop, changed
    /// predecessor, different pending instruction or legacy reset kind refuses;
    /// this cannot rebase an unresolved operation to a newer authority epoch.
    /// Installed predictors retain the original stream-reset refusal.
    ///
    /// The original restart audit runs once in this private candidate, retaining
    /// its success OR recorded failure, real work charges and incident count.
    /// Completion uses ordinary journal capacity; only the following Fence may
    /// spend terminal recovery space. Both records share one acknowledged
    /// canonical replacement, after rechecking the original locked bytes.
    ///
    /// An exact already-completed command and incident declaration take only the
    /// usual Fence path, with no new audit or incident. Missing instructions are
    /// never synthesized. The returned owner is paused, with old keys withdrawn,
    /// and requires original time/resume before new work. Its learned_reset_result
    /// reports the acknowledged reset outcome; no live owner or reviewer escapes
    /// a failure or unwind. Physical replay/audit and storage remain synchronous.
    ///
    /// ```compile_fail,E0382
    /// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{
    ///     FileLearnedRecovery, checkpoint::FileLearnedResetIntent,
    /// };
    /// fn retry_candidate(recovery: FileLearnedRecovery, intent: &FileLearnedResetIntent) {
    ///     let _ = recovery.finish_pending_stream_reset(intent, 1);
    ///     let _ = recovery.finish();
    /// }
    /// ```
    pub fn finish_pending_stream_reset(self, expected: &FileLearnedResetIntent,
        first_incident_sequence: u64) -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        self.prepare_pending_stream_reset(expected, first_incident_sequence)?.finish()
    }

    // Guarded wrappers check their complete independently selected contracts
    // both before and after this consuming preparation. No role or mutable
    // partially reconstructed machine is made available to those callers.
    pub(in super::super::super::super) fn prepare_pending_stream_reset(mut self,
        expected: &FileLearnedResetIntent, first_incident_sequence: u64)
        -> Result<Self, JournalError>
    {
        match self.status {
            FileLearnedRecoveryStatus::Ready => {}
            FileLearnedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileLearnedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            FileLearnedRecoveryStatus::Replaying => return Err(Error::Incomplete.into()),
        }
        expected.validate()?;
        if first_incident_sequence == 0 { return Err(Error::InvalidInput.into()); }
        if let Some(pending) = self.machine.pending_learned_reset() {
            if &pending.intent != expected
                || self.machine.pending_learned_stream_reset_sequence() != Some(first_incident_sequence)
            { return Err(Error::Binding.into()); }
            let operation = expected.control().operation;
            self.machine.check_pending_learned_stream_reset(operation)?;
            let shape = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::StreamReset { operation, witness: Rc::from(&b""[..]) })));
            // This cursor contains only the acknowledged canonical prefix; the
            // live owner's unacknowledged source_interrupted latch is not a
            // durable input and no live owner is constructed to clear it here.
            // Preserve the original machine's source, route, predecessor and
            // stream-boundary checks before new audit work. Guarded mediation
            // separately rechecks its complete independent source contract.
            self.machine.preflight_consistency(&shape)?;
            let bytes = journal::encode(&self.profile, self.store.identity(), &self.events)?.len();
            let reserve = self.events.iter().find_map(|event| match event {
                Event::Core(BaseEvent::ReserveRecovery(reserve)) => Some(*reserve), _ => None,
            });
            let capacity = JournalCapacity::new(self.profile.delivery.limits, reserve,
                self.events.len(), bytes);
            // The completion is ordinary work; the final Fence alone is a
            // terminal operation. Reserve both record slots before auditing,
            // using the same capacity law as live learned-reset completion.
            // Actual witness bytes are still checked by the original encoder.
            if capacity.ordinary_remaining().events == 0 || capacity.remaining().events < 2 {
                return Err(Error::Limit.into());
            }
            self.events.try_reserve(2).map_err(|_| Error::Limit)?;
            if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
                return Err(Error::Binding.into());
            }
            self.status = FileLearnedRecoveryStatus::Interrupted;
            let completed = self.machine.prepare_learned_stream_reset(operation)?;
            let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(completed)));
            // Admission is checked at the ordinary completion prefix, even
            // though finish will store it together with a terminal Fence. A
            // recovery tail cannot hide an oversized completion witness.
            journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
            self.events.push(event);
            self.replayed = self.events.len();
            self.status = FileLearnedRecoveryStatus::Ready;
        } else if self.machine.learned_stream_reset_retry(expected, first_incident_sequence)?.is_none() {
            return Err(Error::Missing.into());
        }
        // The original finish applies the Fence to this SAME private machine,
        // rereads the original canonical image under the SAME lock and makes
        // the sole replacement before returning either owner or reviewer.
        Ok(self)
    }

    // Only the composed guarded owner may recheck its independently held gate
    // inventory around this SAME private transition. No mutable machine or
    // unfenced candidate can escape through the public recovery interfaces.
    pub(in super::super::super::super) fn prepare_pending_reset(mut self,
        expected: &FileLearnedResetIntent) -> Result<Self, JournalError>
    {
        match self.status {
            FileLearnedRecoveryStatus::Ready => {}
            FileLearnedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileLearnedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            FileLearnedRecoveryStatus::Replaying => return Err(Error::Incomplete.into()),
        }
        expected.validate()?;
        if let Some(pending) = self.machine.pending_learned_reset() {
            if &pending.intent != expected { return Err(Error::Binding.into()); }
            self.machine.check_pending_learned_reset(expected.control().operation)?;
            // Reserve BOTH completion and fencing before new original audit work.
            if self.events.len().checked_add(2).ok_or(Error::Limit)? > self.profile.delivery.limits.events {
                return Err(Error::Limit.into());
            }
            self.events.try_reserve(2).map_err(|_| Error::Limit)?;
            if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
                return Err(Error::Binding.into());
            }
            let completed = self.machine.prepare_learned_reset(expected.control().operation)?;
            self.events.push(Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(completed))));
            self.replayed = self.events.len();
        } else if self.machine.learned_reset_retry(expected)?.is_none() {
            // A newly constructed expectation cannot create its own BeginReset.
            return Err(Error::Missing.into());
        }
        // The existing finish uses this verified machine, compares the SAME
        // canonical cut again, and persists the sole complete image. Until that
        // barrier succeeds the added event exists only in this private candidate.
        Ok(self)
    }
}
