//! A source-bound read projection for explicit exact-predicate disclosure.
//! Capture no helper data and return no authority, worker or mutable source.
use super::super::super::{FileOversight, Event};
use crate::action::consequence::delivery::persistent::{JournalError,
    requests::actor::precheck::PrecheckProjection};
use crate::action::{ActionSpec, ElapsedTick};
use crate::action::consequence::delivery::persistent::requests::actor::LearnedTextRelease;
use crate::Error;

impl FileOversight {
    pub(in crate::action::consequence::delivery::persistent) fn actor_precheck_projection(&self)
        -> Result<PrecheckProjection, JournalError>
    {
        if self.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        // A raw actor proposal is not a valid origin for the specialized model,
        // forecast and stream lanes. No approximation of their prerequisites.
        if self.events.iter().any(|event| matches!(event,
            Event::Decoder(_) | Event::Consistency(_) | Event::StreamBootstrap(_)
            | Event::GeneratedStreamBootstrap(_))) { return Err(Error::WrongState.into()); }
        self.captured_actor_precheck_projection()
    }

    /// Specialized source-only disclosure never accepts a raw action from the
    /// actor. Derive the one selected release through the native builder, only
    /// before request admission and after the original generation is complete.
    pub(in crate::action::consequence::delivery::persistent) fn learned_actor_precheck_projection(
        &self, request: u64, release: LearnedTextRelease, deadline: ElapsedTick,
    ) -> Result<(PrecheckProjection, ActionSpec), JournalError> {
        if !self.policy_only_file_source_required() { return Err(Error::WrongState.into()); }
        match self.request_status(request) {
            Err(JournalError::Contract(Error::Missing)) => {}
            Ok(_) => return Err(Error::WrongState.into()),
            Err(error) => return Err(error),
        }
        let action = self.prepare_learned_text_stream_request(request, release, deadline)?;
        let projection = self.captured_actor_precheck_projection()?;
        Ok((projection, action))
    }

    fn captured_actor_precheck_projection(&self) -> Result<PrecheckProjection, JournalError> {
        if self.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        let captured = self.capture_file_policy_state()?;
        let current = self.inspect();
        Ok(PrecheckProjection {
            revision: self.revision(), scope: self.profile.delivery.scope,
            target: current.target,
            policy: self.machine.policy_updates.current(&self.profile.delivery.policy).clone(),
            snapshot: captured.snapshot().clone(),
            epoch: current.control.ledger.epoch,
            now: current.control.ledger.elapsed.ok_or(Error::Incomplete)?,
            available: current.control.ledger.available,
            ceiling: current.control.ceiling,
            stopped: current.control.suspended || current.stop.is_some(),
        })
    }
}
