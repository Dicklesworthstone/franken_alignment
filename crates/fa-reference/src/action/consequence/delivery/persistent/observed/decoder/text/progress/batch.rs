//! Byte projection of one acknowledged batch of ORIGINAL numerical transitions.
use super::{FileOversight, FileTextGenerationProgress, JournalError};
use super::super::super::progress::check_batch_steps;
use crate::Error;
use std::rc::Rc;

impl FileOversight {
    /// Advance a bounded batch of the ORIGINAL durable text request, then return
    /// its cumulative byte-exact progress after ONE acknowledged replacement.
    /// The step bound has the same meaning as advance_decoder_generation_batch.
    ///
    /// This requires an original text intent. Exact tokenizer/command binding
    /// and the complete output allocation are checked before any new inference.
    /// A bare numerical request cannot acquire text semantics retroactively.
    /// No callback receives intermediate bytes, and no second write, decoder,
    /// EOS, Unicode repair, prompt truncation or fresh budget is introduced.
    ///
    /// Old-revision retries return current bytes without another draw; use the
    /// existing delta_from byte cursor to avoid appending them twice. A batch may
    /// end inside a Unicode character. Recovery/cancellation preserve that exact
    /// prefix and the original spent work. Holds and native admission errors stay
    /// distinct from empty successful output. This is not effect authorization.
    pub fn advance_decoder_text_batch(&mut self, journal_revision: u64, id: u64,
        generation_revision: u64, max_steps: usize) -> Result<FileTextGenerationProgress, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        check_batch_steps(max_steps)?;
        let command = Rc::clone(self.machine.decoder_text_command(id).ok_or(Error::Missing)?);
        let current = self.decoder_generation_progress(id)?;
        if generation_revision > current.generation_revision() { return Err(Error::Stale.into()); }
        if !current.is_complete() && generation_revision == current.generation_revision()
            && journal_revision != self.revision() { return Err(Error::Stale.into()); }
        let (prepared, numerical_command) = command.compile(self.machine.decoder_tokenizer()?)?;
        if current.command() != &numerical_command { return Err(Error::Binding.into()); }
        let next = self.advance_decoder_generation_batch(journal_revision, id, generation_revision, max_steps)?;
        Ok(FileTextGenerationProgress::from_prepared(command, prepared, next))
    }
}
