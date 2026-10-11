//! The original request book and accepted endpoint receipt bind each boundary.
use super::{Machine, Transition};
use super::super::super::decoder::learned::FileLearnedTextContinuation;
use super::super::super::{Event, BaseEvent};
use crate::action::ActionState;
use crate::action::consequence::delivery::{EndpointOutcome, stream::ReleaseFrame,
    persistent::requests::FileRequestDisposition};
use crate::Error;

impl Machine {
    pub(in super::super) fn check_learned_message_request(&self, event: &Event) -> Result<(), Error> {
        let Some(command) = self.current_learned_message_continuation() else { return Ok(()); };
        match event {
            Event::Core(BaseEvent::SubmitRequest(request, spec, _)) => {
                if command.request() != *request && self.requests.original_spec(*request).is_none()
                    && !ReleaseFrame::decode(&spec.payload)?.is_finish() { return Err(Error::Binding); }
            }
            // A raw attempt cannot stand in for the request named by the
            // recorded message intent. The request reducer uses the broker
            // directly and does not pass through this unkeyed Propose route.
            Event::Core(BaseEvent::Propose(_, spec, _)) => {
                if !ReleaseFrame::decode(&spec.payload)?.is_finish() { return Err(Error::Binding); }
            }
            _ => {},
        }
        Ok(())
    }

    pub(in super::super::super) fn current_learned_message_continuation(&self)
        -> Option<FileLearnedTextContinuation> {
        self.learned.as_ref().and_then(|state| state.message_continuation)
    }

    pub(in super::super::super) fn prepare_learned_message_continuation(&self,
        after_request: u64, request: u64) -> Result<FileLearnedTextContinuation, Error> {
        if request == 0 || after_request == 0 || request == after_request { return Err(Error::InvalidInput); }
        let learned = self.learned.as_ref().ok_or(Error::Binding)?;
        if !learned.config.allows_text_stream_continuation() { return Err(Error::Binding); }
        if let Some(current) = learned.message_continuation {
            if current.request() == request {
                return if current.after_request() == after_request { Ok(current) } else { Err(Error::Binding) };
            }
        }
        self.check_pre_output_message_continuation()?;
        match self.requests.status(request) {
            Err(Error::Missing) => {},
            Ok(_) => return Err(Error::Duplicate),
            Err(error) => return Err(error),
        }
        if learned.pending.is_some() || self.pending_learned_reset().is_some()
            || self.broker.stream_pending().is_some() { return Err(Error::Incomplete); }
        let (target, confirmed) = self.broker.stream_state().ok_or(Error::Binding)?;
        if self.endpoint.stream_view() != Some(confirmed) { return Err(Error::Incomplete); }
        if confirmed.finished() || self.broker.inspect().suspended || self.broker.stop_receipt().is_some() {
            return Err(Error::WrongState);
        }
        let FileRequestDisposition::Admitted { attempt, stage: ActionState::Confirmed } =
            self.requests.status(after_request)?.disposition else { return Err(Error::Incomplete); };
        if self.broker.resolution(attempt)?.map(|receipt| receipt.outcome())
            != Some(EndpointOutcome::Executed { resulting_version: target.expected_version }) {
            return Err(Error::Stale);
        }
        let previous = self.actions.get(&attempt).ok_or(Error::Missing)?;
        let frame = ReleaseFrame::decode(&previous.spec().payload)?;
        let mut expected_target = previous.spec().target.ok_or(Error::Binding)?;
        expected_target.expected_version = expected_target.expected_version.checked_add(1).ok_or(Error::Overflow)?;
        if expected_target != target || frame.is_finish() || frame.profile() != confirmed.profile()
            || !frame.prior_messages().iter().copied().chain(frame.message()).eq(confirmed.messages()) {
            return Err(Error::Binding);
        }
        let numerical = self.broker.hosted_learned_generation()?;
        self.broker.check_learned_text_stream_continuation(numerical.actor_revision, numerical.position)?;
        Ok(FileLearnedTextContinuation { after_request, request,
            actor_revision: numerical.actor_revision, position: numerical.position })
    }

    pub(super) fn apply_learned_message_continuation(&mut self, command: FileLearnedTextContinuation)
        -> Result<Transition, Error> {
        // An exact API retry is a read. Duplicate canonical records are refused.
        if self.current_learned_message_continuation() == Some(command) { return Err(Error::Duplicate); }
        if self.learned_paused() || !self.clock_ready { return Err(Error::Incomplete); }
        let expected = self.prepare_learned_message_continuation(command.after_request(), command.request())?;
        if expected != command { return Err(Error::Binding); }
        self.broker.continue_learned_text_stream(command.actor_revision(), command.position())?;
        self.learned.as_mut().expect("checked original learned stream").message_continuation = Some(command);
        Ok(Transition::Unit)
    }
}
