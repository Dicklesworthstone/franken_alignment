//! One receipt-bound continuation of the original finite learned text source.
use super::{DecoderEvent, Event, FileOversight, JournalError, LearnedEvent};
use crate::Error;

/// Frozen numerical predecessor and exact original request identities. This is
/// an input intent, not output, an additional budget, an approval or an effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedTextContinuation {
    pub(in super::super::super) after_request: u64,
    pub(in super::super::super) request: u64,
    pub(in super::super::super) actor_revision: u64,
    pub(in super::super::super) position: u64,
}
impl FileLearnedTextContinuation {
    pub fn after_request(&self) -> u64 { self.after_request }
    pub fn request(&self) -> u64 { self.request }
    pub fn actor_revision(&self) -> u64 { self.actor_revision }
    pub fn position(&self) -> u64 { self.position }
}

impl FileOversight {
    /// Pure preparation is available while recovery is paused. Only the latest
    /// ORIGINAL receipt-confirmed message can supply the predecessor. The next
    /// complete message must fit the stream and original finite numerical caps.
    /// Exact retry returns the retained intent, without re-deriving its position.
    pub fn prepare_learned_text_stream_continuation(&self, after_request: u64, request: u64)
        -> Result<FileLearnedTextContinuation, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.prepare_learned_message_continuation(after_request, request)?)
    }

    /// Commit one original continuation intent before any additional sample.
    /// Fresh time/source, explicit resume and the complete current predecessor
    /// are required for a new intent. An exact retained retry writes nothing,
    /// changes no status or offset, and never replenishes any work allowance.
    pub fn begin_learned_text_stream_continuation(&mut self, revision: u64,
        command: FileLearnedTextContinuation) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if self.machine.current_learned_message_continuation() == Some(command) { return Ok(()); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let expected = self.prepare_learned_text_stream_continuation(command.after_request, command.request)?;
        if expected != command { return Err(Error::Binding.into()); }
        self.transact(revision, Event::Decoder(DecoderEvent::Learned(
            LearnedEvent::ContinueMessage(command))))?;
        Ok(())
    }

    /// Historical current message intent. Reads provide no source eligibility,
    /// numerical resume, request admission or publication key.
    pub fn current_learned_text_continuation(&self)
        -> Result<Option<FileLearnedTextContinuation>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !self.learned_text_stream_required() { return Err(Error::Binding.into()); }
        Ok(self.machine.current_learned_message_continuation())
    }
}
