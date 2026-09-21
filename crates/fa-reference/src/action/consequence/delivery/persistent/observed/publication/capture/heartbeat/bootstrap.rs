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
        Self::create_publication_feed_profile(directory.as_ref(), profile, validation, changes, freshness, false)
    }

    /// The first canonical image explicitly enables exact snapshot fallback.
    /// This never marks retained gaps complete or increases comparison budgets.
    pub fn create_with_publication_snapshot_fallback(directory: impl AsRef<Path>,
        profile: FileOversightProfile, validation: PublicationLimits,
        changes: PublicationChangePolicy, freshness: PublicationFreshnessPolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        Self::create_publication_feed_profile(directory.as_ref(), profile, validation, changes, freshness, true)
    }

    fn create_publication_feed_profile(directory: &Path, profile: FileOversightProfile,
        validation: PublicationLimits, changes: PublicationChangePolicy,
        freshness: PublicationFreshnessPolicy, snapshot_fallback: bool)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        freshness.check()?;
        if freshness.clock_domain != profile.delivery.clock_domain { return Err(Error::Binding.into()); }
        let mut events = vec![
            Event::PublicationWitness(WitnessEvent::Enable(validation)),
            Event::PublicationWitness(WitnessEvent::ChangeProfile(changes)),
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::Enable(freshness))),
        ];
        if snapshot_fallback {
            events.push(Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback)));
        }
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory)?;
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
        Self::open_publication_feed_profile(directory.as_ref(), profile, expected_validation,
            expected_changes, expected_freshness, false)
    }

    /// Pin the exact fallback selection BEFORE cleanup or fencing, as well as
    /// all original validation/feed/clock settings. Generic open retains the
    /// stored selection; the strict pinned opener rejects this opt-in profile.
    pub fn open_with_publication_snapshot_fallback(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected_validation: PublicationLimits,
        expected_changes: PublicationChangePolicy, expected_freshness: PublicationFreshnessPolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        Self::open_publication_feed_profile(directory.as_ref(), profile, expected_validation,
            expected_changes, expected_freshness, true)
    }

    fn open_publication_feed_profile(directory: &Path, profile: FileOversightProfile,
        expected_validation: PublicationLimits, expected_changes: PublicationChangePolicy,
        expected_freshness: PublicationFreshnessPolicy, snapshot_fallback: bool)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory)?;
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
        let fallbacks = events.iter().filter(|event| matches!(event,
            Event::PublicationWitness(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback)))).count();
        if fallbacks != usize::from(snapshot_fallback)
            || validation.next() != Some(expected_validation) || validation.next().is_some()
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
