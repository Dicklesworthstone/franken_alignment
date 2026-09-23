//! Bounded original cursor transitions behind ONE canonical acknowledgment.
//! The persisted history contains ordinary per-step events, not a batch format.
use super::{DecoderEvent, Event, FileGenerationProgress, FileOversight, JournalError,
    JournalFailure, JournalIo, Machine, check_progress_key, journal};
use crate::Error;
use std::rc::Rc;

/// Maximum original cursor transitions in one synchronous publication batch.
/// A transition may finish without a token (for example, budget exhaustion).
/// This is not a wall-clock deadline, a larger generation budget or a byte cap.
pub const MAX_FILE_GENERATION_BATCH_STEPS: usize = 64;

impl FileOversight {
    /// Advance up to `max_steps` ORIGINAL incremental transitions with one replay
    /// of the acknowledged prefix and one canonical journal replacement.
    ///
    /// Each step uses the original cursor, mandatory monitor, sampler, work
    /// accounting, automatic-stop handling and exact per-step witness. The first
    /// terminal result ends the batch immediately, even with unused step slots.
    /// No input, stop override, seed, budget or caller-supplied result is accepted.
    /// The resulting history is byte-identical to sequential single-step calls
    /// at the same storage identity; logical revisions advance by actual steps.
    /// Existing single-step, cancellation and recovery entry points remain valid.
    ///
    /// `max_steps` is a scheduling bound in 1..=MAX_FILE_GENERATION_BATCH_STEPS,
    /// not part of the frozen request. Older generation revisions read CURRENT
    /// progress without work, including retries with a different valid bound.
    /// Future revisions refuse. New work requires BOTH current revisions and all
    /// original clock, source and numerical-readiness predicates. A stale retry
    /// does not refresh evidence or assert a fresh cancellation opportunity.
    ///
    /// The full declared event allowance is checked before the first transition,
    /// as in advance_decoder_generation. Later batches require their worst-case
    /// step slots before computation, even if a stop might use fewer. These are
    /// not reservations against other supervisor operations. The ORIGINAL encoder
    /// checks byte capacity and recovery reserves at EVERY logical prefix, not
    /// merely the final total. No terminal reserve is spent as ordinary work.
    ///
    /// All new output is withheld until the sole replacement is acknowledged.
    /// A native hold/failure is a recorded outcome; a witness, encoding, allocation
    /// or storage failure after entry poisons the live owner and exposes no batch
    /// prefix. Reopen sees the old image or the entire batch, then requires fresh
    /// time and explicit resume. Unacknowledged physical work can repeat.
    ///
    /// This is synchronous: cancellation can run BETWEEN calls, not preempt one.
    /// Every original full-state witness remains stored, and old event metadata
    /// is cloned for encoding. This reduces replay/replacement count, not journal
    /// size; no throughput, hard deadline or exactly-once CPU claim is made.
    pub fn advance_decoder_generation_batch(&mut self, journal_revision: u64, id: u64,
        generation_revision: u64, max_steps: usize) -> Result<FileGenerationProgress, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        check_batch_steps(max_steps)?;
        let current = self.decoder_generation_progress(id)?;
        if generation_revision > current.generation_revision() { return Err(Error::Stale.into()); }
        if current.is_complete() || generation_revision < current.generation_revision() { return Ok(current); }
        if journal_revision != self.revision() { return Err(Error::Stale.into()); }
        check_progress_key(id, generation_revision)?;
        self.check_source_admission(&Event::Decoder(DecoderEvent::AdvanceGeneration {
            id, revision: generation_revision, witness: Rc::from(&b""[..]),
        }))?;
        self.machine.check_decoder_generation_progress(id, generation_revision)?;
        let requested = current.command().check()?;
        let completed = usize::try_from(generation_revision).map_err(|_| Error::Limit)?;
        let remaining = requested.checked_sub(completed).ok_or(Error::Stale)?;
        let steps = max_steps.min(remaining);
        if steps == 0 { return Err(Error::Binding.into()); }
        let required = if generation_revision == 0 { requested } else { steps };
        if self.events.len().checked_add(required).ok_or(Error::Overflow)? > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        let capacity = self.events.len().checked_add(steps).ok_or(Error::Overflow)?;
        let mut events = Vec::new();
        events.try_reserve_exact(capacity).map_err(|_| Error::Limit)?;
        events.extend(self.events.iter().cloned());
        // Exactly one replay, not one replay for each prospective token. The
        // acknowledged live Machine remains untouched until the final receipt.
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let mut witness_bytes = 0_usize;
        for offset in 0..steps {
            let revision = generation_revision.checked_add(offset as u64).ok_or(Error::Overflow)?;
            let step = candidate.prepare_decoder_generation_progress(id, revision)?;
            let DecoderEvent::AdvanceGeneration { witness, .. } = &step else {
                return Err(Error::Binding.into());
            };
            // A necessary aggregate bound BEFORE retaining the next witness.
            // The original encoder below still charges ALL framing, old records
            // and reserves exactly; this is not a substitute admission formula.
            // At most one additional bounded witness is transient on refusal.
            witness_bytes = witness_bytes.checked_add(witness.len()).ok_or(Error::Limit)?;
            if witness_bytes > self.profile.delivery.limits.bytes { return Err(Error::Limit.into()); }
            events.push(Event::Decoder(step));
            if candidate.recorded_decoder_generation(id).is_some() { break; }
        }
        let progress = candidate.decoder_generation_progress(id)?;
        // encode checks EVERY event prefix through the original Admission and
        // HistoryBudget; no intermediate oversized prefix is hidden by batching.
        let bytes = journal::encode(&self.profile, self.store.identity(), &events)?;
        // Same Store and conservative fault/acknowledgment law as persist_candidate.
        // This closed path contains ONLY original AdvanceGeneration events. It
        // cannot import arbitrary events, acknowledge a source, or publish effects.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: true });
        if let Err(error) = self.store.replace(&bytes) {
            self.fault = Some(match &error {
                JournalError::Io(failure) => failure.clone(),
                _ => JournalFailure { operation: JournalIo::Stage,
                    kind: std::io::ErrorKind::Other, replacement_may_be_visible: true },
            });
            return Err(error);
        }
        // Nothing fallible remains after acknowledgment. In particular no text
        // decoding, vector growth, role provisioning or additional token runs.
        self.events = events;
        self.machine = candidate;
        self.fault = None;
        Ok(progress)
    }
}

pub(in super::super) fn check_batch_steps(steps: usize) -> Result<(), Error> {
    if steps == 0 { return Err(Error::InvalidInput); }
    if steps > MAX_FILE_GENERATION_BATCH_STEPS { return Err(Error::Limit); }
    Ok(())
}

#[cfg(test)]
mod tests;
