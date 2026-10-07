//! Select an original unsubmitted generation for explicit publication recovery.
//! Preparation is read-only; fresh source/time, resume and both keys remain native.
use super::{Error, Event, FileOversight, JournalError};
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, text::TextGenerationRequest,
};
use crate::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;

impl FileOversight {
    /// Select the EXACT recorded text intent at the current numerical cut, either
    /// still pending or naturally stopped before its first message submission.
    /// No intent/cursor/budget is imported or recreated, and no output is returned.
    /// `expected` is the independently retained complete input, not a new budget.
    ///
    /// Refuse any generation already linked to a message request, including a
    /// rejected/cancelled one; receipt recovery is a different operation. A newer
    /// numerical position, a different pending generation, incomplete stream
    /// settlement, a terminal stop or a non-StopToken result cannot supply this
    /// cut. The full remaining disclosure allowance is checked without changing
    /// the original generation allowance or charging acknowledged work twice.
    ///
    /// This works while the owner is recovery-paused or its source/time is stale.
    /// It is NOT permission to continue: original fresh observations and explicit
    /// resume are still needed, and each token and eventual message submission
    /// rechecks its native predecessor. No live human/automatic key is restored.
    /// Before submission there is no recorded effect deadline or request binding;
    /// the operator must separately choose a fresh, unused publication request.
    pub fn prepare_decoder_text_publication_recovery(&self, generation: u64,
        expected: &TextGenerationRequest) -> Result<FileTextGenerationCommand, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if generation == 0 { return Err(Error::InvalidInput.into()); }
        if !self.generated_text_stream_required()? { return Err(Error::Binding.into()); }
        let progress = self.decoder_text_progress(generation)?;
        let command = progress.command();
        if command.request() != expected { return Err(Error::Binding.into()); }
        // Scan the ORIGINAL bounded event sequence, not a second mutable index.
        if self.events.iter().any(|event| matches!(event,
            Event::TextMessage(source, _) if source.generation == generation))
        { return Err(Error::Duplicate.into()); }
        let stream = self.stream_snapshot()?;
        if stream.publication.stop.is_some() || stream.publication.control.suspended
            || stream.confirmed.finished() { return Err(Error::WrongState.into()); }
        if stream.pending.is_some() || stream.confirmed != stream.published
            || stream.confirmed_target != stream.publication.target
        { return Err(Error::Incomplete.into()); }
        let profile = stream.confirmed.profile();
        if expected.max_new_tokens == 0 || expected.max_output_bytes == 0 || expected.stop_tokens.is_empty() {
            return Err(Error::InvalidInput.into());
        }
        if stream.confirmed.message_count() >= profile.max_messages()
            || expected.max_output_bytes > profile.max_message_bytes()
            || expected.max_output_bytes > profile.max_stream_bytes()
                .checked_sub(stream.confirmed.visible().len()).ok_or(Error::Binding)?
        { return Err(Error::Limit.into()); }
        let tokenizer = self.machine.decoder_tokenizer()?;
        for token in &expected.stop_tokens {
            if !tokenizer.is_control(*token)? { return Err(Error::Binding.into()); }
        }
        let pending = self.pending_decoder_text()?;
        let position = if let Some(receipt) = progress.numerical().receipt() {
            if self.pending_decoder_generation()?.is_some() { return Err(Error::Incomplete.into()); }
            let report = receipt.result()?;
            if report.finish() != GenerationFinish::StopToken
                || report.reviewed_prompt_tokens() != report.requested_prompt_tokens()
            { return Err(Error::WrongState.into()); }
            report.end_position()
        } else {
            if pending.as_ref() != Some(command) { return Err(Error::Incomplete.into()); }
            // Generation revision counts transitions, NOT numerical positions.
            // In particular, an unstarted intent has no partial report yet.
            progress.numerical().partial().map_or(command.position(), |partial| partial.position())
        };
        let numerical = self.decoder_inspection()?.numerical;
        if numerical.status != MonitoringStatus::Ready { return Err(Error::WrongState.into()); }
        let steps = position.checked_sub(command.position()).ok_or(Error::Binding)?;
        let actor_revision = command.actor_revision().checked_add(steps).ok_or(Error::Overflow)?;
        if numerical.position != position || numerical.actor_revision != actor_revision {
            return Err(Error::Stale.into());
        }
        Ok(command.clone())
    }
}

#[cfg(test)]
mod tests;
