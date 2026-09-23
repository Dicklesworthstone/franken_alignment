//! Supervisor cancellation of one durable numerical request, not its effects.
use super::{DecoderEvent, Event, FileOversight, JournalError};
use super::progress::{FileGenerationProgress, check_progress_key};
use crate::Error;

impl FileOversight {
    /// Withdraw the remaining tokens of ONE already durable generation.
    /// The original command and released prefix become a Cancelled receipt.
    /// Requested history capacity, actual cache, spent draws, numerical work and
    /// monitor work remain retained. No token computes to complete a cancelled
    /// prompt, no output is repaired, and no effect reservation is refunded.
    ///
    /// Active cancellation requires BOTH current journal and generation revisions.
    /// Unlike an old advance retry, stale cancellation cannot pretend to have
    /// cancelled newer work. An already terminal ID returns its original outcome
    /// read-only (including holds/failures), even while another request is active;
    /// it does not pause that newer request. A future generation revision refuses.
    /// A faulted owner refuses even historical retries.
    ///
    /// No current clock/source observation is needed to stop computation. A real
    /// cancellation pauses the original numerical owner and withdraws clock
    /// readiness. Continuing under a NEW request requires a trusted observation
    /// and explicit resume at the retained actor/position, with every original
    /// guard still enforced. Cancellation clears neither holds, source gaps,
    /// pending forecasts, authority restrictions nor permanent shutdown.
    ///
    /// One ordinary canonical replacement acknowledges cancellation. The existing
    /// journal byte/event bounds and storage-failure latch apply; no recovery
    /// reserve or physical disk space is created. A failed write returns no
    /// candidate receipt, and exclusive recovery must inspect the actual cut.
    /// Historical replay may recompute older tokens; this transition computes no
    /// NEW token and provides no exactly-once CPU or external-effect guarantee.
    pub fn cancel_decoder_generation(&mut self, journal_revision: u64, id: u64,
        generation_revision: u64) -> Result<FileGenerationProgress, JournalError>
    {
        let current = self.decoder_generation_progress(id)?;
        if generation_revision > current.generation_revision() { return Err(Error::Stale.into()); }
        if current.is_complete() { return Ok(current); }
        if generation_revision != current.generation_revision() || journal_revision != self.revision() {
            return Err(Error::Stale.into());
        }
        check_progress_key(id, generation_revision)?;
        self.transact(journal_revision, Event::Decoder(DecoderEvent::CancelGeneration {
            id, revision: generation_revision,
        }))?;
        self.decoder_generation_progress(id)
    }
}

#[cfg(test)]
mod tests;
