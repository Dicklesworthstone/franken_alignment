//! Persist the original routing algorithm as a bootstrap policy choice.
use super::{BaseEvent, Error, Event, FileHumanReviewer, FileOversight, FileOversightProfile,
    JournalError, Machine, Path, PublicationChangePolicy, PublicationLimits, WitnessEvent, journal, storage};
use crate::witness::refinement::index::routing::RoutingStrategy;

impl FileOversight {
    /// Atomic initialization of validation, the change feed and subtree routing.
    /// Other independent profiles (for example producer freshness) are still
    /// explicit bootstrap operations; this does not imply an authentication lease.
    pub fn create_with_publication_subtree_routing(directory: impl AsRef<Path>,
        profile: FileOversightProfile, validation: PublicationLimits, changes: PublicationChangePolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let events = vec![
            Event::PublicationWitness(WitnessEvent::Enable(validation)),
            Event::PublicationWitness(WitnessEvent::ChangeProfile(changes)),
            Event::PublicationWitness(WitnessEvent::SubtreeRouting),
        ];
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Pin validation limits, the complete change policy and exactly one subtree
    /// selection BEFORE cleanup or a recovery write. Additional journaled gates
    /// are replayed unchanged, not removed by this focused pin.
    pub fn open_with_publication_subtree_routing(directory: impl AsRef<Path>,
        profile: FileOversightProfile, validation: PublicationLimits, changes: PublicationChangePolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        let mut validations = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::Enable(value)) => Some(*value), _ => None,
        });
        let mut feeds = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(WitnessEvent::ChangeProfile(value)) => Some(*value), _ => None,
        });
        let selections = events.iter().filter(|event| matches!(event,
            Event::PublicationWitness(WitnessEvent::SubtreeRouting))).count();
        if selections != 1 || validations.next() != Some(validation) || validations.next().is_some()
            || feeds.next() != Some(changes) || feeds.next().is_some() { return Err(Error::Binding.into()); }
        let machine = Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }

    /// Explicit bootstrap for an existing owner, before any proposal/change.
    pub fn enable_publication_subtree_routing(&mut self, revision: u64) -> Result<(), JournalError> {
        self.transact(revision, Event::PublicationWitness(WitnessEvent::SubtreeRouting))?;
        Ok(())
    }
    pub fn publication_routing_strategy(&self) -> Result<RoutingStrategy, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.publication_routing_strategy()?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{read, write, Reader, Writer};
    use super::*;
    use crate::witness::refinement::index::routing::RoutingBudget;

    #[test]
    fn subtree_selection_has_its_own_bootstrap_tag_and_legacy_bytes_do_not_change() {
        let mut writer = Writer::new(100);
        write(&mut writer, &WitnessEvent::SubtreeRouting).unwrap();
        assert_eq!(writer.finish(), vec![10]);
        let mut reader = Reader::new(&[10]);
        let selection = read(&mut reader).unwrap();
        assert!(matches!(selection, WitnessEvent::SubtreeRouting));
        assert!(selection.bootstrap()); reader.end().unwrap();
        assert!(read(&mut Reader::new(&[])).is_err());
        assert!(read(&mut Reader::new(&[255])).is_err());
        let mut trailing = Reader::new(&[10, 0]);
        read(&mut trailing).unwrap(); assert!(trailing.end().is_err());
        let policy = PublicationChangePolicy { source: 41, after: 9,
            lookup: RoutingBudget { steps: 100, bytes: 8000 } };
        let mut writer = Writer::new(100);
        write(&mut writer, &WitnessEvent::ChangeProfile(policy)).unwrap();
        let bytes = writer.finish();
        let mut expected = vec![5];
        for value in [41_u64, 9, 100, 8000] { expected.extend_from_slice(&value.to_be_bytes()); }
        assert_eq!(bytes, expected);
        let mut reader = Reader::new(&bytes);
        match read(&mut reader).unwrap() { WitnessEvent::ChangeProfile(value) => assert_eq!(value, policy),
            _ => panic!("legacy change profile tag changed") }
        reader.end().unwrap();
    }
}
