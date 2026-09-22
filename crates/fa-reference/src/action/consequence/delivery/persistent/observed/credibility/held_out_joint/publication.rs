//! Joint qualification and final-publication guards in the SAME first image.
//! The runnable supervisor consumes this configuration; it is not evidence or
//! an alternative authority. Existing bootstrap records and reducers are reused.
use super::{FileHeldOutJointSnapshot, HeldOutJointPolicy, Prepared};
use super::super::CredibilityEvent;
use super::super::super::{BaseEvent, Event, FileHumanReviewer, FileOversight,
    FileOversightProfile, JournalError, Machine, journal, storage};
use super::super::super::publication::witness_gate::{WitnessEvent, freshness::FreshnessEvent};
use crate::action::consequence::delivery::publication_gate::PublicationLimits;
use crate::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use crate::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use crate::Error;
use std::path::Path;

/// Exact selected native publication gates. None requires ABSENCE of that gate,
/// not acceptance of a journal-selected policy. Other independently configured
/// source/identity/credential guards retain their original enforcement; this is
/// not the broader FileGuardSet inventory or an independent rollback floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JointPublicationProfile {
    pub joint: HeldOutJointPolicy,
    pub validation: Option<PublicationLimits>,
    pub feed: Option<JointPublicationFeed>,
}

/// A feed requires witness validation and its original freshness contract.
/// The existing exact-snapshot alternative is an explicit, pinned selection.
/// This profile selects legacy prefix routing; it never silently reinterprets a
/// stored subtree-routing budget as an equivalent prefix-routing budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JointPublicationFeed {
    pub changes: PublicationChangePolicy,
    pub freshness: PublicationFreshnessPolicy,
    pub snapshot_fallback: bool,
}

impl JointPublicationProfile {
    fn validate(&self, profile: &FileOversightProfile) -> Result<(), Error> {
        if let Some(feed) = self.feed {
            if self.validation.is_none() { return Err(Error::InvalidInput); }
            feed.freshness.check()?;
            if feed.freshness.clock_domain != profile.delivery.clock_domain {
                return Err(Error::Binding);
            }
        }
        Ok(())
    }

    fn prepare(&self, profile: FileOversightProfile) -> Result<Prepared, JournalError> {
        self.validate(&profile)?;
        // Joint bootstrap also installs first-publication revalidation. All
        // subsequent events below are original bootstrap operations with no
        // clock, observation, helper invocation or effect authority transition.
        let mut prepared = Prepared::new(profile, self.joint)?;
        let mut additional = Vec::new();
        if let Some(limits) = self.validation {
            additional.push(WitnessEvent::Enable(limits));
        }
        if let Some(feed) = self.feed {
            additional.push(WitnessEvent::ChangeProfile(feed.changes));
            additional.push(WitnessEvent::Freshness(FreshnessEvent::Enable(feed.freshness)));
            if feed.snapshot_fallback {
                additional.push(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback));
            }
        }
        for witness in additional {
            witness.check_clock_domain(prepared.profile.delivery.clock_domain)?;
            let event = Event::PublicationWitness(witness);
            prepared.machine.apply(&event)?;
            prepared.events.push(event);
        }
        self.check_events(&prepared.events)?;
        if prepared.events.len() > prepared.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        Ok(prepared)
    }

    fn check_events(&self, events: &[Event]) -> Result<(), Error> {
        let joint = events.iter().filter_map(|event| match event {
            Event::Credibility(CredibilityEvent::EnableHeldOutJoint(policy)) => Some(*policy),
            _ => None,
        });
        let validation = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Enable(limits)) => Some(*limits),
            _ => None,
        });
        let changes = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::ChangeProfile(policy)) => Some(*policy),
            _ => None,
        });
        let freshness = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::Enable(policy))) => Some(*policy),
            _ => None,
        });
        let fallback = events.iter().filter(|event| matches!(event,
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback)))).count();
        if !joint.eq(std::iter::once(self.joint)) || !validation.eq(self.validation)
            || !changes.eq(self.feed.map(|feed| feed.changes))
            || !freshness.eq(self.feed.map(|feed| feed.freshness))
            || fallback != usize::from(self.feed.is_some_and(|feed| feed.snapshot_fallback))
            || events.iter().any(|event| matches!(event, Event::PublicationWitness(WitnessEvent::SubtreeRouting)))
        { return Err(Error::Binding); }
        Ok(())
    }

    fn checked_image(&self, profile: &FileOversightProfile, identity: &Path, bytes: &[u8])
        -> Result<(Vec<Event>, Machine), JournalError>
    {
        self.validate(profile)?;
        let events = journal::decode(profile, identity, bytes)?;
        // Check exact configuration before native replay, including recorded
        // numerical work. Semantic replay still checks uniqueness and ordering.
        self.check_events(&events)?;
        let machine = Machine::replay(profile, &events)?;
        if machine.broker.held_out_joint_policy() != Some(self.joint) { return Err(Error::Binding.into()); }
        Ok((events, machine))
    }
}

impl FileOversight {
    /// Match this live owner's complete selection before acquiring or rebinding
    /// a source. Success is configuration equality, not current qualification.
    pub fn check_joint_publication_profile(&self, expected: JointPublicationProfile)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        expected.validate(&self.profile)?;
        expected.check_events(&self.events).map_err(Into::into)
    }

    /// Configure all selected joint/witness/feed gates in ONE canonical image.
    /// A failed or ambiguous replacement returns neither owner nor reviewer.
    /// The initial baseline can still run; qualification constrains promotion,
    /// not the ability to acquire independently evaluated baseline observations.
    pub fn create_with_joint_publication(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: JointPublicationProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let prepared = expected.prepare(profile)?;
        prepared.publish(storage::Store::create(directory.as_ref())?)
    }

    /// Check every selected joint/publication field, including ABSENT gates,
    /// before cleanup or the original single recovery fence. No source is read.
    pub fn open_with_joint_publication(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: JointPublicationProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        Self::open_joint_publication_store(storage::Store::open(directory.as_ref())?, profile, expected)
    }

    fn open_joint_publication_store(store: storage::Store, profile: FileOversightProfile,
        expected: JointPublicationProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let (events, machine) = expected.checked_image(&profile, store.identity(), &bytes)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }

    /// One immutable historical cut with the same exact configuration checks.
    /// No writer lock, cleanup, fence, source acquisition or role issuance.
    pub fn read_joint_publication(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: JointPublicationProfile) -> Result<FileHeldOutJointSnapshot, JournalError>
    {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let (events, machine) = expected.checked_image(profile, &identity, &bytes)?;
        let promotions = machine.broker.credibility_changes().map(|change| {
            let report = machine.broker.held_out_joint_report(change.operation)?.ok_or(Error::Binding)?;
            Ok((change.operation, report.clone()))
        }).collect::<Result<Vec<_>, Error>>()?;
        Ok(FileHeldOutJointSnapshot { journal: machine.snapshot(events.len()), policy: expected.joint, promotions })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod supervisor_tests;
