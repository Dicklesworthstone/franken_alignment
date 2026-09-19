//! Atomic required-profile initialization and independently pinned recovery.
use super::*;
use super::super::super::super::{BaseEvent, Event, FileHumanReviewer, FileOversightProfile, Machine, journal, storage};
use super::super::super::witness_gate::WitnessEvent;
use super::super::super::witness_gate::freshness::FreshnessEvent;
use crate::action::consequence::delivery::publication_gate::PublicationLimits;
use crate::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use crate::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;

impl FileOversight {
    /// The FIRST canonical image contains all three required profiles. The
    /// returned owner has no heartbeat yet and cannot authorize a publication.
    pub fn create_with_publication_change_freshness(directory: impl AsRef<Path>,
        profile: FileOversightProfile, validation: PublicationLimits,
        changes: PublicationChangePolicy, freshness: PublicationFreshnessPolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        freshness.check()?;
        if freshness.clock_domain != profile.delivery.clock_domain { return Err(Error::Binding.into()); }
        let events = vec![
            Event::PublicationWitness(WitnessEvent::Enable(validation)),
            Event::PublicationWitness(WitnessEvent::ChangeProfile(changes)),
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::Enable(freshness))),
        ];
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Pin validation limits, source/initial change cut/lookup budget and the
    /// clock/age contract BEFORE replay, cleanup or recovery writes. No wrong
    /// profile returns an owner. Generic open also replays all stored requirements.
    pub fn open_with_publication_change_freshness(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected_validation: PublicationLimits,
        expected_changes: PublicationChangePolicy, expected_freshness: PublicationFreshnessPolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        let mut validation = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Enable(value)) => Some(*value), _ => None,
        });
        let mut changes = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::ChangeProfile(value)) => Some(*value), _ => None,
        });
        let mut freshness = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::Enable(value))) => Some(*value), _ => None,
        });
        if validation.next() != Some(expected_validation) || validation.next().is_some()
            || changes.next() != Some(expected_changes) || changes.next().is_some()
            || freshness.next() != Some(expected_freshness) || freshness.next().is_some()
        { return Err(Error::Binding.into()); }
        let machine = Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }
}
