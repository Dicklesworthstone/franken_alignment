//! Trusted policy changes, retained in the original full-input/two-key journal.
use super::{BaseEvent, Event, FileOversight, JournalError, Transition};
use super::super::governance::{PolicyUpdate, PolicyUpdateReceipt};
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::Error;

impl FileOversight {
    /// Commit the original policy transition AND pending human-key withdrawal
    /// together before returning any cancellation or reservation-refund evidence.
    /// This cannot remove mandatory human review, widen the original target
    /// ceiling, resume suspension, or revoke already-admitted dispatches.
    pub fn replace_policy(&mut self, revision: u64, update: &PolicyUpdate)
        -> Result<PolicyUpdateReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(receipt) = self.machine.policy_updates.retry(update)? { return Ok(receipt); }
        match self.transact(revision, Event::Core(BaseEvent::ReplacePolicy(update.clone())))? {
            Transition::PolicyUpdated(receipt) => Ok(receipt),
            _ => unreachable!("observed policy update transition"),
        }
    }
    pub fn policy_update_receipt(&self, operation: u64) -> Result<&PolicyUpdateReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.policy_updates.receipt(operation).ok_or_else(|| Error::Missing.into())
    }
    /// An immutable policy is data, not a usable receipt or a recovered key.
    /// Refuse rather than call the last acknowledged cut current after I/O fault.
    pub fn current_policy(&self) -> Result<&Policy, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.policy_updates.current(&self.profile.delivery.policy))
    }
}
