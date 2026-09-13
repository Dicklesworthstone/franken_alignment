//! Governed recovery of the original bounded file-observation stream.
//! A replacement changes the capture incarnation, never the producer identity,
//! fixed freshness/retention limits, journal budget or already charged effects.
use super::{FileOversight, JournalError, SourceEvent};
use super::super::Event;
use crate::action::consequence::delivery::PolicySourceChange;
use crate::Error;

/// Trusted host request, not an actor command or observation-provider privilege.
/// Operation IDs are retained across reopen. Exact retries return the original
/// historical result without replacing again or cancelling newer work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSourceReplacement {
    pub operation: u64,
    pub expected_generation: u64,
    pub expected_authority_epoch: u64,
    pub next_generation: u64,
}

impl FileOversight {
    /// Repair an exhausted/poisoned stream through the ORIGINAL broker's bounded
    /// replacement operation. The source ID, scope, age and per-generation limits
    /// cannot change. The new stream is unavailable until a fresh full file read.
    /// Producer generation, exact producer bytes and semantic/time floors survive.
    ///
    /// Only undispatched reservations are cancelled/refunded. Existing dispatches
    /// retain their envelopes for guarded nonexecution or historical reconciliation;
    /// no previously admitted effect is retried, refunded or declared successful.
    pub fn replace_file_source(&mut self, revision: u64, request: FileSourceReplacement)
        -> Result<PolicySourceChange, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request.operation == 0 { return Err(Error::InvalidInput.into()); }
        if let Some((original, receipt)) = self.machine.source_replacement(request.operation) {
            if original != &request { return Err(Error::Binding.into()); }
            return Ok(receipt.clone());
        }
        self.transact(revision, Event::Source(SourceEvent::Replace(request)))?;
        self.file_source_replacement(request.operation)
    }

    /// Inspect an acknowledged historical replacement. This grants no source
    /// eligibility and never clears a later interrupted-observation latch.
    pub fn file_source_replacement(&self, operation: u64) -> Result<PolicySourceChange, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.source_replacement(operation).map(|(_, receipt)| receipt.clone())
            .ok_or_else(|| Error::Missing.into())
    }
}
