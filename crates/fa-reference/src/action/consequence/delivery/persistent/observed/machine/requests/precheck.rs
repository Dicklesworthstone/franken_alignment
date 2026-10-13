//! A source-bound read projection for explicit exact-predicate disclosure.
//! Capture no helper data and return no authority, worker or mutable source.
use super::super::super::{FileOversight, Event};
use crate::action::consequence::delivery::persistent::{JournalError,
    requests::actor::precheck::PrecheckProjection};
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
