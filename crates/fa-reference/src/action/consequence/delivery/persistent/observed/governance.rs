//! Trusted policy changes, retained in the original full-input/two-key journal.
use super::{BaseEvent, Event, FileOversight, JournalError, Transition};
use super::super::governance::{PolicyUpdate, PolicyUpdateReceipt};
use crate::action::consequence::gate::containment::session::policy::Policy;
use crate::action::consequence::policy_campaign::{PolicyReplayReport, ReplayLimits};
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

    /// Compare a candidate with ALL retained complete proposal/review cases
    /// under the current policy, including exact denials. No caller-selected
    /// archive, fresh source read, helper execution, approval or mutation occurs.
    /// New observations outside the retained witnesses require shadow evaluation.
    pub fn replay_candidate_policy(&self, candidate: Policy, limits: ReplayLimits)
        -> Result<PolicyReplayReport, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.replay_candidate_policy(candidate, limits)?)
    }

    /// Read one canonical journal image and reconstruct its original policy
    /// corpus in RAM. Unlike open(), this acquires no writer, appends no fence,
    /// performs no cleanup and returns no authority. The accompanying snapshot
    /// identifies the exact historical cut used; it is not a live-source claim.
    /// The directory must satisfy the same operator trust contract as read_publication.
    pub fn read_policy_replay(directory: impl AsRef<std::path::Path>,
        profile: &super::FileOversightProfile, candidate: Policy, limits: ReplayLimits)
        -> Result<(super::FileDeliverySnapshot, PolicyReplayReport), JournalError>
    {
        super::super::codec::validate_profile(&profile.delivery)?;
        let identity = super::storage::identity(directory.as_ref())?;
        let bytes = super::storage::read(&identity.join(super::storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = super::journal::decode(profile, &identity, &bytes)?;
        let machine = super::Machine::replay(profile, &events)?;
        let report = machine.broker.replay_candidate_policy(candidate, limits)?;
        Ok((machine.snapshot(events.len()), report))
    }
}
