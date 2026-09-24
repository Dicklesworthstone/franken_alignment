//! Bind a completed ORIGINAL text generation to one durable message proposal.
//! This is provenance-preserving admission, never automatic publication.

mod recovery;
mod required;
pub use recovery::FileTextMessageSnapshot;

use super::{BaseEvent, Event, FileOversight, JournalError, Machine, Reader, Writer};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_GENERATION_TOKENS,
};
use crate::action::consequence::delivery::persistent::requests::FileRequestStatus;
use crate::{Error, Snapshot};

/// Complete source and destination identity, with NO caller-supplied output.
/// The generation revision identifies its acknowledged terminal result; target,
/// policy epoch and deadline identify the proposed complete-message effect.
/// Every field participates in exact retry equality. The admission snapshot is
/// observed only on the first submission, like the original submit_request API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTextMessageRequest {
    pub request: u64,
    pub generation: u64,
    pub generation_revision: u64,
    pub target: ResolvedTarget,
    pub policy_epoch: u64,
    pub deadline: ElapsedTick,
}
impl FileTextMessageRequest {
    fn check(&self) -> Result<(), Error> {
        if self.request == 0 || self.generation == 0 { return Err(Error::InvalidInput); }
        if self.generation_revision > MAX_GENERATION_TOKENS as u64 { return Err(Error::Limit); }
        Ok(())
    }
}

impl FileOversight {
    /// Submit the exact nonempty UTF-8 output of a completed native text request
    /// to the ORIGINAL complete-message/two-key pipeline. No output argument,
    /// summary, suffix selection, inferred EOS or regenerated token is accepted.
    ///
    /// The entire prompt must have been reviewed and the generation must have
    /// stopped on a monitored Control token. TokenLimit, cancellation, holds,
    /// failures and exhausted budgets cannot be relabeled complete messages.
    /// Content stop IDs refuse: suppressing one could conceal a meaningful suffix.
    /// The original model/actor predecessor must still be current; a later token
    /// or reset cannot use an older result as evidence about the new live state.
    ///
    /// The original stream builder retains ALL confirmed messages, boundaries
    /// and full-frame resource charging. The original request book and proposal
    /// reducer then decide admission, including recorded policy refusals. This
    /// does not review, authorize, approve, dispatch or publish the message.
    ///
    /// One canonical record binds the source request to that admission. Replay
    /// recomputes the text from its original numerical history and invokes the
    /// SAME proposal reducer. There is no second outcome book or imported output.
    /// Exact retries read current retained status before clock/source/predecessor
    /// checks, without rebasing the message onto a different stream prefix. An
    /// unrelated ordinary request cannot acquire generated-message provenance.
    /// Storage failure returns no candidate status and poisons the original owner.
    pub fn submit_decoder_text_message(&mut self, revision: u64,
        request: FileTextMessageRequest, snapshot: Snapshot)
        -> Result<FileRequestStatus, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        request.check()?;
        if let Some(previous) = self.text_message_request(request.request) {
            if previous != &request { return Err(Error::Binding.into()); }
            return self.request_status(request.request);
        }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let id = request.request;
        self.transact(revision, Event::TextMessage(Box::new(request), snapshot))?;
        self.request_status(id)
    }

    /// Historical source linkage from the ORIGINAL canonical event, not a permit
    /// or a claim that the endpoint published. Read request_status/resolution for
    /// the current disposition. Generic submissions deliberately have no linkage.
    /// Lookup scans the bounded journal; no additional mutable index owns truth.
    pub fn decoder_text_message_request(&self, request: u64)
        -> Result<&FileTextMessageRequest, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.text_message_request(request).ok_or_else(|| Error::Missing.into())
    }

    fn text_message_request(&self, request: u64) -> Option<&FileTextMessageRequest> {
        self.events.iter().find_map(|event| match event {
            Event::TextMessage(source, _) if source.request == request => Some(source.as_ref()),
            _ => None,
        })
    }
}

impl Machine {
    /// Expand only this closed input into the preexisting SubmitRequest event.
    /// Both live consistency preflight and semantic replay use this same binding.
    /// No caller can supply an event, model report or replacement request book.
    pub(in super::super) fn decoder_text_message_event(&self,
        request: &FileTextMessageRequest, snapshot: &Snapshot) -> Result<Event, Error>
    {
        request.check()?;
        // A preexisting ordinary request cannot be recast as a generated one,
        // even if today's stream builder would now reject its old predecessor.
        match self.requests.status(request.request) {
            Ok(_) => return Err(Error::Duplicate),
            Err(Error::Missing) => {}
            Err(error) => return Err(error),
        }
        if !self.clock_ready || self.decoder_paused() { return Err(Error::Incomplete); }
        let command = self.decoder_text_command(request.generation).ok_or(Error::Missing)?;
        let progress = self.decoder_generation_progress(request.generation)?;
        if progress.generation_revision() != request.generation_revision { return Err(Error::Stale); }
        let receipt = progress.receipt().ok_or(Error::Incomplete)?;
        let report = receipt.result()?;
        if report.finish() != GenerationFinish::StopToken
            || report.reviewed_prompt_tokens() != report.requested_prompt_tokens()
        { return Err(Error::Incomplete); }
        let tokenizer = self.decoder_tokenizer()?;
        for token in &command.request().stop_tokens {
            if !tokenizer.is_control(*token)? { return Err(Error::Binding); }
        }
        let numerical = self.broker.hosted_decoder()?;
        if numerical.status != MonitoringStatus::Ready { return Err(Error::WrongState); }
        let steps = report.end_position().checked_sub(command.position()).ok_or(Error::Binding)?;
        let actor_revision = command.actor_revision().checked_add(steps).ok_or(Error::Overflow)?;
        if report.start_position() != command.position() || numerical.position != report.end_position()
            || numerical.actor_revision != actor_revision
        { return Err(Error::Stale); }
        // Reuse exact text admission and decoding, not another tokenizer or a
        // reinterpreted numerical request. Only acknowledged released IDs enter.
        let (prepared, original) = command.compile(tokenizer)?;
        if receipt.command() != &original { return Err(Error::Binding); }
        let bytes = prepared.decode_output(report.tokens())?;
        let message = std::str::from_utf8(&bytes).map_err(|_| Error::InvalidInput)?;
        let spec = self.broker.stream_message_spec(message, request.deadline)?;
        if spec.target != Some(request.target) || spec.policy_epoch != request.policy_epoch {
            return Err(Error::Stale);
        }
        Ok(Event::Core(BaseEvent::SubmitRequest(request.request, spec, snapshot.clone())))
    }


}

pub(in super::super) fn write_request(w: &mut Writer, request: &FileTextMessageRequest)
    -> Result<(), Error>
{
    request.check()?;
    w.u64(request.request)?; w.u64(request.generation)?; w.u64(request.generation_revision)?;
    w.target(request.target)?; w.u64(request.policy_epoch)?; w.u64(request.deadline.0)
}
pub(in super::super) fn read_request(r: &mut Reader<'_>) -> Result<FileTextMessageRequest, Error> {
    let request = FileTextMessageRequest { request: r.u64()?, generation: r.u64()?,
        generation_revision: r.u64()?, target: r.target()?, policy_epoch: r.u64()?,
        deadline: ElapsedTick(r.u64()?) };
    request.check()?;
    Ok(request)
}

#[cfg(test)]
mod tests;
