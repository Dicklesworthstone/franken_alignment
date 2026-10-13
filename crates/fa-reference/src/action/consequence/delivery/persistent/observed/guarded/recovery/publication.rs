//! Exact independent publication configuration on the original frozen cursor.
//! This selects no live evidence and changes no journal or recovery transition.
use super::{Error, Event, FileGuardedRecovery, FileGuardedRecoveryStatus, JournalError};
use super::super::super::credibility::{CredibilityEvent, held_out_joint::HeldOutJointPolicy};
use super::super::super::credibility::held_out_joint::publication::JointPublicationFeed;
use super::super::super::publication::witness_gate::{WitnessEvent, freshness::FreshnessEvent};
use crate::action::consequence::delivery::publication_gate::PublicationLimits;

/// Independently selected ORIGINAL publication gates, in addition to the full
/// FileRecoveryRequirements. None requires absence, never a journal-selected
/// default. A feed pins source, initial cut, routing budget, freshness and the
/// exact-snapshot alternative. This profile uses the original prefix routing;
/// a stored subtree-routing profile is not silently interpreted as equivalent.
///
/// These are bootstrap requirements, not current evidence or permission. Saved
/// captures, leases, qualification and approvals retain their original recovery
/// withdrawal rules. Learned/evaluated/predictive/mediated profiles still require
/// their separate original openers; this does not bypass the base guard checker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePublicationRecoveryRequirements {
    pub validation: Option<PublicationLimits>,
    pub feed: Option<JointPublicationFeed>,
    pub joint: Option<HeldOutJointPolicy>,
}

impl FileGuardedRecovery {
    /// Require the exact publication bootstrap BEFORE any original event is
    /// replayed. A mismatch or late selection consumes this cursor without a
    /// clock, fence, staging cleanup or role. Failed admission releases its lock.
    ///
    /// The decoded canonical events are private and immutable, so this check
    /// remains bound to the same image through advance and every finish route.
    /// Full semantic replay still validates uniqueness, order, observations and
    /// all independent guards/floors; matching configuration never skips it.
    /// No history is replayed, no budget installed and no source file opened here.
    pub fn require_publication_profile(self, expected: FilePublicationRecoveryRequirements)
        -> Result<Self, JournalError>
    {
        if self.attempted != 0 || self.status != FileGuardedRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        if let Some(feed) = expected.feed {
            if expected.validation.is_none() { return Err(Error::InvalidInput.into()); }
            feed.freshness.check()?;
            if feed.freshness.clock_domain != self.profile.delivery.clock_domain {
                return Err(Error::Binding.into());
            }
        }
        let validation = self.events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Enable(limits)) => Some(*limits),
            _ => None,
        });
        let changes = self.events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::ChangeProfile(policy)) => Some(*policy),
            _ => None,
        });
        let freshness = self.events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::Enable(policy))) => Some(*policy),
            _ => None,
        });
        let joint = self.events.iter().filter_map(|event| match event {
            Event::Credibility(CredibilityEvent::EnableHeldOutJoint(policy)) => Some(*policy),
            _ => None,
        });
        let fallback = self.events.iter().filter(|event| matches!(event,
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback)))).count();
        if !validation.eq(expected.validation)
            || !changes.eq(expected.feed.map(|feed| feed.changes))
            || !freshness.eq(expected.feed.map(|feed| feed.freshness))
            || !joint.eq(expected.joint)
            || fallback != usize::from(expected.feed.is_some_and(|feed| feed.snapshot_fallback))
            || self.events.iter().any(|event| matches!(event,
                Event::PublicationWitness(WitnessEvent::SubtreeRouting)))
        { return Err(Error::Binding.into()); }
        Ok(self)
    }
}

#[cfg(test)]
mod tests;
