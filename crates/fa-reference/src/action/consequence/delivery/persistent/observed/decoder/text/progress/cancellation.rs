//! Byte-exact projection of the original durable cancellation, not a new cursor.
use super::{FileOversight, FileTextGenerationProgress, JournalError, Error, Rc};

impl FileOversight {
    /// Cancel one durable TEXT intent and retain its exact reviewed output prefix.
    /// The original numerical cancellation controls identity, revisions, replay,
    /// work retention, pause/fresh-clock requirements and storage acknowledgment.
    /// A bare-ID generation is rejected before any mutation: its result cannot
    /// be retroactively assigned a tokenizer or converted into a text intent.
    ///
    /// Tokenization binding and output capacity are checked BEFORE the native
    /// transaction. Only its acknowledged released IDs become bytes afterward.
    /// The original text projection keeps any decoding error alongside the native
    /// result; split UTF-8 stays split, and cancellation never samples a repair.
    /// Cancelled is distinct from a fully reviewed prompt or a natural EOS.
    ///
    /// Existing terminal outcomes remain read-only, even with a stale journal
    /// revision. Active cancellations require both exact current revisions; no
    /// stale request can cancel a newer cursor or pause a different active job.
    /// No current clock/source, callback, new budget, token override or effect
    /// permission is accepted here. Storage failure returns no candidate text.
    pub fn cancel_decoder_text(&mut self, journal_revision: u64, id: u64,
        generation_revision: u64) -> Result<FileTextGenerationProgress, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let command = Rc::clone(self.machine.decoder_text_command(id).ok_or(Error::Missing)?);
        let current = self.decoder_generation_progress(id)?;
        if generation_revision > current.generation_revision() { return Err(Error::Stale.into()); }
        if !current.is_complete() && (generation_revision != current.generation_revision()
            || journal_revision != self.revision()) { return Err(Error::Stale.into()); }
        let (prepared, expected) = command.compile(self.machine.decoder_tokenizer()?)?;
        if current.command() != &expected { return Err(Error::Binding.into()); }
        let next = self.cancel_decoder_generation(journal_revision, id, generation_revision)?;
        Ok(FileTextGenerationProgress::from_prepared(command, prepared, next))
    }
}
