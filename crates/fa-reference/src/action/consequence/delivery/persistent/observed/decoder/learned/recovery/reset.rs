//! Explicit recovery of an acknowledged reset intent before the recovery fence.
//! Only the original command, original reset and original canonical cut are used.
use super::{Event, FileHumanReviewer, FileLearnedRecovery, FileLearnedRecoveryStatus,
    FileOversight, JournalError};
use super::super::{DecoderEvent, LearnedEvent, checkpoint::FileLearnedResetIntent};
use crate::action::consequence::delivery::persistent::observed::containment::FileResetRequest;
use crate::action::consequence::activation::tensor::kv::decoder::monitoring::restart::KvRestartBudget;
use crate::Error;

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
    pub fn finish_pending_reset(mut self, expected: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
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
        self.finish()
    }
}
