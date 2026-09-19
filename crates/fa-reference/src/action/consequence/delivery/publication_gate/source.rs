//! Source-bound, single-comparison observations in the ORIGINAL publication gate.
//! Producer identity/generation are trusted observations, not authentication.
use super::{DeliveryBroker, PublicationInputs, Slot};
use crate::action::ActionState;
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationSourceStatus {
    pub source: u64,
    pub generation: u64,
    pub capture_pending: bool,
    pub fresh: bool,
}

#[derive(Debug)]
pub(super) struct SourceState {
    status: PublicationSourceStatus,
    // Keep exact generation contents through withdrawal, not just a digest.
    last: PublicationInputs,
}

impl DeliveryBroker {
    /// Pin one producer after binding the original reviewed judgment. This is a
    /// one-way strengthening, while Reviewing and before any automatic permit.
    /// It does not make the original observations current or grant effect rights.
    pub fn bind_publication_source(&mut self, attempt: u64, source: u64, generation: u64,
        original: PublicationInputs) -> Result<(), Error>
    {
        if source == 0 || generation == 0 { return Err(Error::InvalidInput); }
        if self.inspect().ledger.stages.get(&attempt) != Some(&ActionState::Reviewing) {
            return Err(Error::WrongState);
        }
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get_mut(&attempt).ok_or(Error::Missing)?;
        if slot.judgment.is_none() { return Err(Error::Incomplete); }
        if slot.source.is_some() { return Err(Error::Duplicate); }
        // A pre-existing current observation must never survive strengthening.
        // Check the original cut against any earlier recorded floor first.
        slot.check_floor(Some(&original))?;
        slot.floor = cut(&original).or(slot.floor);
        slot.current = None;
        slot.source = Some(SourceState { status: PublicationSourceStatus {
            source, generation, capture_pending: false, fresh: false,
        }, last: original });
        Ok(())
    }

    pub fn publication_source(&self, attempt: u64) -> Result<Option<PublicationSourceStatus>, Error> {
        let gate = self.publication.as_ref().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get(&attempt).ok_or(Error::Missing)?;
        Ok(slot.source.as_ref().map(|source| source.status))
    }

    /// Complete an explicit withdrawal/read cycle. Repeated generation numbers
    /// require exact contents; rollback and producer substitution fail closed.
    /// One successful capture supports only ONE validation boundary. Native hosts
    /// remain responsible for actually reading; the durable adapter owns that I/O.
    pub fn record_captured_publication_inputs(&mut self, attempt: u64, expected_revision: u64,
        source: u64, generation: u64, inputs: PublicationInputs) -> Result<u64, Error>
    {
        let gate = self.publication.as_mut().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get_mut(&attempt).ok_or(Error::Missing)?;
        if slot.revision != expected_revision { return Err(Error::Stale); }
        let retained = slot.source.as_ref().ok_or(Error::Incomplete)?;
        if !retained.status.capture_pending || slot.current.is_some() { return Err(Error::WrongState); }
        if retained.status.source != source { return Err(Error::Binding); }
        if generation < retained.status.generation { return Err(Error::Stale); }
        if generation == retained.status.generation && inputs != retained.last { return Err(Error::Binding); }
        // All fallible validation/allocation precedes changes to the floor.
        let copy = inputs.clone();
        let revision = slot.replace_inputs(expected_revision, Some(inputs))?;
        let retained = slot.source.as_mut().expect("checked source");
        retained.last = copy;
        retained.status.generation = generation;
        retained.status.capture_pending = false;
        retained.status.fresh = true;
        Ok(revision)
    }
}

fn cut(inputs: &PublicationInputs) -> Option<(u64, u64, u64)> {
    inputs.structured.as_ref().map(|(snapshot, _)| {
        (snapshot.revision(), snapshot.control_cut(), snapshot.semantic_epoch())
    })
}

impl Slot {
    fn check_floor(&self, inputs: Option<&PublicationInputs>) -> Result<(), Error> {
        if let (Some(old), Some(new)) = (self.floor, inputs.and_then(cut))
            && (new.0 < old.0 || new.1 < old.1 || new.2 < old.2)
        { return Err(Error::Stale); }
        Ok(())
    }

    pub(super) fn replace_inputs(&mut self, expected_revision: u64,
        inputs: Option<PublicationInputs>) -> Result<u64, Error>
    {
        if self.revision != expected_revision { return Err(Error::Stale); }
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.check_floor(inputs.as_ref())?;
        self.floor = inputs.as_ref().and_then(cut).or(self.floor);
        self.current = inputs;
        self.revision = revision;
        Ok(revision)
    }

    pub(super) fn record_inputs(&mut self, expected_revision: u64,
        inputs: Option<PublicationInputs>) -> Result<u64, Error>
    {
        if self.revision != expected_revision { return Err(Error::Stale); }
        if self.source.is_some() && inputs.is_some() { return Err(Error::Binding); }
        let revision = self.replace_inputs(expected_revision, inputs)?;
        if let Some(source) = &mut self.source {
            source.status.capture_pending = true;
            source.status.fresh = false;
        }
        Ok(revision)
    }

    pub(super) fn consume_capture(&mut self) -> bool {
        let Some(source) = &mut self.source else { return true; };
        source.status.capture_pending = false;
        std::mem::replace(&mut source.status.fresh, false)
    }
}
