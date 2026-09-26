//! Prepare another message on the ORIGINAL native stream and numerical owner.
//! This is a new-intent precondition, not a new journal format or grant.
use super::{FileOversight, JournalError, Error, GenerationFinish, MonitoringStatus};
use crate::action::ActionState;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::text::TextGenerationRequest;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::stream::ReleaseFrame;

impl FileOversight {
    /// Prepare a NEW native generation after the last receipt-confirmed message.
    /// The raw prompt is additional input to the retained numerical context; no
    /// chat template, reset, model replacement, output replay or seed is inferred.
    ///
    /// Require the exact native source, original Executed receipt, complete latest
    /// message boundaries, no outstanding stream/generation, and the unchanged
    /// numerical predecessor. Admit the entire requested horizon and maximum
    /// output against REMAINING context/disclosure space before new computation.
    /// A published-but-unreconciled message, cancelled request, older prefix,
    /// finished stream or superseding numerical work cannot supply this cut.
    ///
    /// Pure preparation also works while recovery is paused and time/source or
    /// qualification is stale. The command is NOT permission to run: the original
    /// source/clock/qualification checks and explicit resume still apply at begin.
    /// All lifetime numerical/monitoring spend survives; only the new request has
    /// a new explicitly supplied per-request budget. No storage or I/O occurs.
    ///
    /// Generation IDs are never reusable, even for identical input. Once an intent
    /// exists, recover/retry that ORIGINAL command instead of preparing it again.
    pub fn prepare_decoder_text_continuation(&self, after_request: u64, generation: u64,
        request: TextGenerationRequest) -> Result<FileTextGenerationCommand, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if after_request == 0 || generation == 0 { return Err(Error::InvalidInput.into()); }
        if !self.generated_text_stream_required()? { return Err(Error::Binding.into()); }
        match self.decoder_generation_progress(generation) {
            Ok(_) => return Err(Error::Duplicate.into()),
            Err(JournalError::Contract(Error::Missing)) => {},
            Err(error) => return Err(error),
        }
        if self.pending_decoder_generation()?.is_some() { return Err(Error::Incomplete.into()); }
        let state = self.stream_snapshot()?;
        if state.pending.is_some() || state.confirmed != state.published {
            return Err(Error::Incomplete.into());
        }
        if state.confirmed.finished() || state.publication.stop.is_some()
            || state.publication.control.suspended { return Err(Error::WrongState.into()); }
        let source = self.decoder_text_message_request(after_request)?;
        if !matches!(self.request_status(after_request)?.disposition,
            FileRequestDisposition::Admitted { stage: ActionState::Confirmed, .. }) {
            return Err(Error::Incomplete.into());
        }
        match self.request_resolution(after_request)? {
            Some(EndpointOutcome::Executed { resulting_version })
                if resulting_version == state.confirmed_target.expected_version => {},
            _ => return Err(Error::Stale.into()),
        }
        let action = self.request_action(after_request)?;
        let frame = ReleaseFrame::decode(&action.spec().payload)?;
        if frame.is_finish() || frame.profile() != state.confirmed.profile()
            || !frame.prior_messages().iter().copied().chain(frame.message())
                .eq(state.confirmed.messages())
            || state.confirmed_target != state.publication.target {
            return Err(Error::Binding.into());
        }
        let previous = self.decoder_text_generation(source.generation)?;
        let report = previous.result()?.generation();
        if report.finish() != GenerationFinish::StopToken
            || self.decoder_generation_progress(source.generation)?.generation_revision() != source.generation_revision {
            return Err(Error::Binding.into());
        }
        let steps = report.end_position().checked_sub(previous.command().position()).ok_or(Error::Binding)?;
        let revision = previous.command().actor_revision().checked_add(steps).ok_or(Error::Overflow)?;
        let numerical = self.decoder_inspection()?.numerical;
        if numerical.status != MonitoringStatus::Ready { return Err(Error::WrongState.into()); }
        if numerical.position != report.end_position() || numerical.actor_revision != revision {
            return Err(Error::Stale.into());
        }
        let profile = state.confirmed.profile();
        if request.max_new_tokens == 0 || request.max_output_bytes == 0 || request.stop_tokens.is_empty() {
            return Err(Error::InvalidInput.into());
        }
        if state.confirmed.message_count() >= profile.max_messages()
            || request.max_output_bytes > profile.max_message_bytes()
            || request.max_output_bytes > profile.max_stream_bytes().checked_sub(state.confirmed.visible().len())
                .ok_or(Error::Binding)? { return Err(Error::Limit.into()); }
        let command = FileTextGenerationCommand::new(generation, revision, numerical.position, request)?;
        let tokenizer = self.machine.decoder_tokenizer()?;
        for id in &command.request().stop_tokens {
            if !tokenizer.is_control(*id)? { return Err(Error::Binding.into()); }
        }
        let (_, compiled) = command.compile(tokenizer)?;
        let horizon = compiled.request().prompt.len().checked_add(compiled.request().max_new_tokens)
            .ok_or(Error::Overflow)?;
        let horizon = u64::try_from(horizon).map_err(|_| Error::Limit)?;
        let context = self.machine.decoder_contract().ok_or(Error::Incomplete)?.profile().shape().context;
        if numerical.position.checked_add(horizon).ok_or(Error::Overflow)? > context as u64 {
            return Err(Error::Limit.into());
        }
        revision.checked_add(horizon).ok_or(Error::Overflow)?;
        Ok(command)
    }

    /// Recheck the complete continuation cut and freeze the ORIGINAL text intent
    /// in its existing one-image transaction. No token is computed by this call.
    /// A prepared command cannot survive intervening inference or publication.
    /// This is fresh initiation, not a second idempotency book: after commitment,
    /// use begin_decoder_text's exact retry or ordinary text recovery by ID.
    pub fn begin_decoder_text_continuation(&mut self, revision: u64, after_request: u64,
        command: FileTextGenerationCommand) -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let expected = self.prepare_decoder_text_continuation(after_request, command.id(), command.request().clone())?;
        if expected != command { return Err(Error::Binding.into()); }
        self.begin_decoder_text(revision, command)
    }
}

#[cfg(test)]
mod tests;
