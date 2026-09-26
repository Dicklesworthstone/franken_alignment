//! Explicit closure of a confirmed native prefix, never an inferred EOS or Stop.
use super::{confirmed_cut, FileOversight, JournalError, Error, EndpointOutcome, ReleaseFrame};
use crate::action::{ElapsedTick, VERSION};
use crate::action::consequence::delivery::persistent::observed::stream::FileStreamProposal;

impl FileOversight {
    /// Prepare a separately reviewed finish after the latest confirmed native
    /// message. This adds NO text or numerical work, so it remains possible at
    /// the message/byte/context limit. The original full cumulative frame is
    /// still charged and must pass helpers, human approval and publication.
    ///
    /// The exact confirmed source cut must be usable: no pending generation or
    /// publication, no later numerical predecessor, and no Stop or suspension.
    /// Pure preparation can inspect a paused recovered owner; it grants no right
    /// to resume or publish. Fresh source/time, qualification and the original
    /// explicit resume remain prerequisites of the ordinary actor submission.
    /// Recheck preparation after obtaining those observations. Once a request is
    /// recorded, use decoder_text_finish_request instead of constructing a retry
    /// from today's epoch, deadline or prefix. No storage, token or I/O occurs.
    pub fn prepare_decoder_text_finish(&self, after_request: u64, request: u64,
        deadline: ElapsedTick) -> Result<FileStreamProposal, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 || after_request == 0 { return Err(Error::InvalidInput.into()); }
        match self.request_status(request) {
            Ok(_) => return Err(Error::Duplicate.into()),
            Err(JournalError::Contract(Error::Missing)) => {},
            Err(error) => return Err(error),
        }
        let (state, _) = confirmed_cut(self, after_request)?;
        Ok(FileStreamProposal { target: state.confirmed_target,
            expected_policy_epoch: state.publication.control.ledger.epoch,
            deadline, message: None })
    }

    /// Recover the EXACT retained finish intent, including an admission refusal,
    /// cancellation or expired deadline. Match its entire prefix and target to
    /// the supplied native message's original Executed receipt; another request
    /// containing the same text is not an interchangeable predecessor.
    ///
    /// Reads only the acknowledged journal-derived request book and receipts.
    /// It needs no current clock, source, qualification or numerical resume and
    /// cannot settle, resend or reauthorize an effect. A message request cannot
    /// acquire finish semantics. Faulted owners refuse rather than expose an old
    /// in-memory cut as current. Generic stream formats and retries are unchanged.
    pub fn decoder_text_finish_request(&self, after_request: u64, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 || after_request == 0 { return Err(Error::InvalidInput.into()); }
        if !self.generated_text_stream_required()? { return Err(Error::Binding.into()); }
        let original = self.machine.requests.original_spec(request).ok_or(Error::Missing)?;
        let finish = ReleaseFrame::decode(&original.payload)?;
        if !finish.is_finish() || original.version != VERSION
            || original.scope != self.profile.delivery.scope || !original.required_witnesses.is_empty()
            || original.units != original.payload.len() as u64 {
            return Err(Error::Binding.into());
        }
        let source = self.decoder_text_message_request(after_request)?;
        let previous = self.request_action(after_request)?;
        let message = ReleaseFrame::decode(&previous.spec().payload)?;
        let resulting_version = match self.request_resolution(after_request)? {
            Some(EndpointOutcome::Executed { resulting_version }) => resulting_version,
            _ => return Err(Error::Incomplete.into()),
        };
        let mut expected_target = source.target;
        expected_target.expected_version = resulting_version;
        if message.is_finish() || finish.profile() != message.profile()
            || finish.profile() != self.stream_snapshot()?.confirmed.profile()
            || original.target != Some(expected_target)
            || !message.prior_messages().iter().copied().chain(message.message())
                .eq(finish.prior_messages().iter().copied()) {
            return Err(Error::Binding.into());
        }
        Ok(FileStreamProposal { target: expected_target,
            expected_policy_epoch: original.policy_epoch, deadline: original.deadline, message: None })
    }
}

#[cfg(test)]
mod tests;
