//! Request intake consumes the original learned-stream frame builder unchanged.
use super::{FileOversight, JournalError, ActionSpec, VERSION, Error};
use crate::action::{ActionState, ElapsedTick, Scope};
use crate::action::consequence::delivery::persistent::observed::Machine;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
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
        Ok(self.machine.learned_text_stream_message_request(self.profile.delivery.scope, request)?)
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
        Ok(self.machine.learned_text_stream_finish_request(
            self.profile.delivery.scope, after_request, request)?)
    }

    pub(in crate::action::consequence::delivery::persistent) fn prepare_learned_text_stream_request(
        &self, request: u64, release: LearnedTextRelease, deadline: ElapsedTick,
    ) -> Result<ActionSpec, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        // Shared exact historical validation also serves the sealed recovery
        // cursor. Absence is the only case allowed to reach live derivation.
        if let Some(original) = self.machine.retained_learned_stream_request(
            self.profile.delivery.scope, request, release)? {
            if original.deadline != deadline { return Err(Error::Binding.into()); }
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

// Pure reads of the ORIGINAL request book, source contract and accepted receipts.
// Live ticket intake and fully verified recovery use one validator. No mutable
// candidate, second outcome book or current source/authority claim is created.
impl Machine {
    fn retained_learned_stream_request(&self, scope: Scope, request: u64,
        release: LearnedTextRelease) -> Result<Option<&ActionSpec>, Error>
    {
        if request == 0 { return Err(Error::InvalidInput); }
        let profile = self.learned_contract().and_then(|config| config.text_stream_profile())
            .ok_or(Error::Binding)?;
        let Some(original) = self.requests.original_spec(request) else { return Ok(None); };
        let frame = ReleaseFrame::decode(&original.payload).map_err(|_| Error::Binding)?;
        if original.version != VERSION || original.scope != scope
            || !original.required_witnesses.is_empty()
            || original.units != original.payload.len() as u64 || frame.profile() != profile
            || frame.is_finish() != (release == LearnedTextRelease::Finish) {
            return Err(Error::Binding);
        }
        Ok(Some(original))
    }

    pub(in crate::action::consequence::delivery::persistent::observed)
    fn learned_text_stream_message_request(&self, scope: Scope, request: u64)
        -> Result<FileStreamProposal, Error>
    {
        let original = self.retained_learned_stream_request(scope, request,
            LearnedTextRelease::Message)?.ok_or(Error::Missing)?;
        let frame = ReleaseFrame::decode(&original.payload)?;
        Ok(FileStreamProposal { target: original.target.ok_or(Error::Binding)?,
            expected_policy_epoch: original.policy_epoch, deadline: original.deadline,
            message: Some(frame.message().ok_or(Error::Binding)?.to_owned()) })
    }

    pub(in crate::action::consequence::delivery::persistent::observed)
    fn learned_text_stream_finish_request(&self, scope: Scope, after_request: u64, request: u64)
        -> Result<FileStreamProposal, Error>
    {
        if request == 0 || after_request == 0 || request == after_request {
            return Err(Error::InvalidInput);
        }
        let original = self.retained_learned_stream_request(scope, request,
            LearnedTextRelease::Finish)?.ok_or(Error::Missing)?;
        let finish = ReleaseFrame::decode(&original.payload)?;
        let FileRequestDisposition::Admitted { attempt, stage } = self.requests.status(after_request)?.disposition
            else { return Err(Error::WrongState); };
        let previous = self.actions.get(&attempt).ok_or(Error::Missing)?;
        let message = ReleaseFrame::decode(&previous.spec().payload)?;
        let mut target = previous.spec().target.ok_or(Error::Binding)?;
        target.expected_version = target.expected_version.checked_add(1).ok_or(Error::Overflow)?;
        // Pre-dispatch outcomes have no accepted endpoint receipt. Neither
        // published-but-unconfirmed data nor equal message text proves execution.
        if matches!(stage, ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing
            | ActionState::Authorized | ActionState::Denied | ActionState::Cancelled) {
            return Err(Error::Incomplete);
        }
        if self.broker.resolution(attempt)?.map(|receipt| receipt.outcome())
            != Some(EndpointOutcome::Executed { resulting_version: target.expected_version }) {
            return Err(Error::Incomplete);
        }
        if message.is_finish() || message.profile() != finish.profile()
            || original.target != Some(target)
            || !message.prior_messages().iter().copied().chain(message.message())
                .eq(finish.prior_messages().iter().copied()) {
            return Err(Error::Binding);
        }
        Ok(FileStreamProposal { target, expected_policy_epoch: original.policy_epoch,
            deadline: original.deadline, message: None })
    }
}
