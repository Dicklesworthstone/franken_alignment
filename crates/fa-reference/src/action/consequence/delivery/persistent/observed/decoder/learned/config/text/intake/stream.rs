//! Request intake consumes the original learned-stream frame builder unchanged.
use super::{FileOversight, JournalError, ActionSpec, VERSION, Error};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::LearnedTextRelease;
use crate::action::consequence::delivery::stream::ReleaseFrame;
use crate::action::consequence::oversight::learned_host::text::stream::Release;
use crate::action::consequence::delivery::persistent::{EndpointOutcome,
    observed::stream::FileStreamProposal};

impl FileOversight {
    /// Read the ORIGINAL pre-policy message request, even when policy refused
    /// admission and no FrozenAction exists. This is historical proposal data,
    /// not evidence of execution or permission to publish. Exact retry binding
    /// uses the same source-only intake validator as the actor gateway.
    ///
    /// No current source, clock, forecast, helper or numerical resume is needed.
    /// The request must already exist: this accessor never derives a new message
    /// from the current generation or audience frontier.
    pub fn learned_text_stream_message_request(&self, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        let original = self.machine.requests.original_spec(request).ok_or(Error::Missing)?;
        let spec = self.prepare_learned_text_stream_request(
            request, LearnedTextRelease::Message, original.deadline)?;
        let frame = ReleaseFrame::decode(&spec.payload)?;
        Ok(FileStreamProposal { target: spec.target.ok_or(Error::Binding)?,
            expected_policy_epoch: spec.policy_epoch, deadline: spec.deadline,
            message: Some(frame.message().ok_or(Error::Binding)?.to_owned()) })
    }

    /// Inspect the exact retained learned finish, including a policy admission
    /// refusal, cancellation or expired request. The original message receipt
    /// and full cumulative boundaries determine its predecessor; equal text
    /// from a different unexecuted request is not an interchangeable source.
    ///
    /// This reads the original request book and accepted endpoint receipt only.
    /// It needs no current policy, clock, numerical resume or helper assets and
    /// returns no approval, reconciliation or effect authority. Recovery must
    /// independently pin the original complete learned recipe before this call.
    pub fn learned_text_stream_finish_request(&self, after_request: u64, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 || after_request == 0 || request == after_request {
            return Err(Error::InvalidInput.into());
        }
        let profile = self.machine.learned_contract().and_then(|config| config.text_stream_profile())
            .ok_or(Error::Binding)?;
        let original = self.machine.requests.original_spec(request).ok_or(Error::Missing)?;
        let finish = ReleaseFrame::decode(&original.payload)?;
        let previous = self.request_action(after_request)?;
        let message = ReleaseFrame::decode(&previous.spec().payload)?;
        let mut target = previous.spec().target.ok_or(Error::Binding)?;
        target.expected_version = target.expected_version.checked_add(1).ok_or(Error::Overflow)?;
        if self.request_resolution(after_request)?
            != Some(EndpointOutcome::Executed { resulting_version: target.expected_version }) {
            return Err(Error::Incomplete.into());
        }
        if !finish.is_finish() || message.is_finish() || finish.profile() != profile
            || message.profile() != profile || original.version != VERSION
            || original.scope != self.profile.delivery.scope || !original.required_witnesses.is_empty()
            || original.units != original.payload.len() as u64 || original.target != Some(target)
            || !message.prior_messages().iter().copied().chain(message.message())
                .eq(finish.prior_messages().iter().copied()) {
            return Err(Error::Binding.into());
        }
        Ok(FileStreamProposal { target, expected_policy_epoch: original.policy_epoch,
            deadline: original.deadline, message: None })
    }

    pub(in crate::action::consequence::delivery::persistent) fn prepare_learned_text_stream_request(
        &self, request: u64, release: LearnedTextRelease, deadline: ElapsedTick,
    ) -> Result<ActionSpec, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        let profile = self.machine.learned_contract().and_then(|config| config.text_stream_profile())
            .ok_or(Error::Binding)?;
        if let Some(original) = self.machine.requests.original_spec(request) {
            // This is the exact request-book pre-policy spec, including refused
            // admissions. Do not derive another payload or target from today's
            // source, pending dispatch or receipt-confirmed audience frontier.
            let frame = ReleaseFrame::decode(&original.payload).map_err(|_| Error::Binding)?;
            if original.deadline != deadline || original.version != VERSION
                || original.scope != self.profile.delivery.scope || !original.required_witnesses.is_empty()
                || original.units != original.payload.len() as u64 || frame.profile() != profile
                || frame.is_finish() != (release == LearnedTextRelease::Finish) {
                return Err(Error::Binding.into());
            }
            return Ok(original.clone());
        }
        if self.source_interrupted || self.machine.pending_learned_reset().is_some() {
            return Err(Error::Incomplete.into());
        }
        if release == LearnedTextRelease::Message
            && self.machine.current_learned_message_continuation()
                .is_some_and(|command| command.request() != request) {
            return Err(Error::Binding.into());
        }
        let current = self.learned_generation_inspection()?;
        if !self.clock_ready() || current.paused || current.pending.is_some() {
            return Err(Error::Incomplete.into());
        }
        // Only this existing builder decides which original release is ready,
        // the full cumulative payload, target, epoch and entire frame charge.
        // In particular dispatch/visible publication is NOT a confirmed append.
        Ok(self.machine.broker.learned_stream_spec(match release {
            LearnedTextRelease::Message => Release::Message,
            LearnedTextRelease::Finish => Release::Finish,
        }, deadline)?)
    }
}
