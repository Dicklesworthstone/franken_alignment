//! Bind an immutable producer image to the change prefix it actually includes.
//! Acquisition time and producer generation alone do not establish this relation.
//! The reference producer still asserts completeness; this is not authentication.
use super::{DeliveryBroker, PublicationInputs, Slot, SourceState};
use super::super::changes::PublicationChangeStatus;
use crate::Error;

/// Source is the independently configured CHANGE feed, not the snapshot producer.
/// A producer must advance its image generation when this field changes, even if
/// all payload bytes remain equal. Never synthesize this from a consumer's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationInputCut {
    pub source: u64,
    pub through: u64,
}
impl PublicationInputCut {
    pub fn check(self) -> Result<(), Error> {
        if self.source == 0 { return Err(Error::InvalidInput); }
        Ok(())
    }
}

/// Historical installed image and minimum coverage demanded by invalidations.
/// Neither value grants effect rights or replaces the exact witness comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationInputCutStatus {
    pub last: PublicationInputCut,
    pub required_through: u64,
}

impl DeliveryBroker {
    /// Bind the original producer image at the current COMPLETE change prefix.
    /// This is part of initial source binding, not an upgrade of an existing
    /// binding or a rebase of the reviewed judgment. No observation is made fresh.
    pub fn bind_publication_source_at_cut(&mut self, attempt: u64, source: u64,
        generation: u64, original: PublicationInputs, input_cut: PublicationInputCut)
        -> Result<(), Error>
    {
        input_cut.check()?;
        let feed = self.publication_change_status()?;
        if input_cut.source != feed.source { return Err(Error::Binding); }
        if !feed.complete() || input_cut.through > feed.through { return Err(Error::Incomplete); }
        if input_cut.through < feed.through { return Err(Error::Stale); }
        self.bind_publication_source(attempt, source, generation, original)?;
        // The original binder performed all fallible work and installed this slot.
        let retained = self.publication.as_mut().expect("bound gate").slots
            .get_mut(&attempt).expect("bound attempt").source.as_mut().expect("bound source");
        retained.input_cut = Some(PublicationInputCutStatus {
            last: input_cut, required_through: input_cut.through,
        });
        Ok(())
    }

    pub fn publication_input_cut(&self, attempt: u64) -> Result<Option<PublicationInputCutStatus>, Error> {
        let gate = self.publication.as_ref().ok_or(Error::Incomplete)?;
        let slot = gate.slots.get(&attempt).ok_or(Error::Missing)?;
        Ok(slot.source.as_ref().and_then(|source| source.input_cut))
    }

    /// The original withdrawal/read cycle, additionally bound to a covered feed
    /// cut. A newer image generation with an OLD cut remains stale. A future cut
    /// cannot fill a missing notification tail or excuse exact final validation.
    pub fn record_captured_publication_inputs_at_cut(&mut self, attempt: u64,
        expected_revision: u64, source: u64, generation: u64, inputs: PublicationInputs,
        input_cut: PublicationInputCut) -> Result<u64, Error>
    {
        self.record_capture(attempt, expected_revision, source, generation, inputs, Some(input_cut))
    }
}

impl SourceState {
    pub(super) fn check_input_cut(&self, supplied: Option<PublicationInputCut>,
        generation: u64, feed: Option<PublicationChangeStatus>) -> Result<(), Error>
    {
        let (bound, supplied) = match (self.input_cut, supplied) {
            (None, None) => return Ok(()),
            (None, Some(_)) => return Err(Error::Binding),
            (Some(_), None) => return Err(Error::Incomplete),
            (Some(bound), Some(supplied)) => (bound, supplied),
        };
        supplied.check()?;
        let feed = feed.ok_or(Error::Incomplete)?;
        if supplied.source != bound.last.source || supplied.source != feed.source { return Err(Error::Binding); }
        if generation == self.status.generation && supplied != bound.last { return Err(Error::Binding); }
        if supplied.through < bound.required_through || supplied.through < bound.last.through { return Err(Error::Stale); }
        if !feed.complete() || supplied.through > feed.through { return Err(Error::Incomplete); }
        Ok(())
    }
}

impl Slot {
    /// Called only for selected invalidations, including ALL slots during gaps,
    /// repair and conservative routing. It survives None, re-reads and recovery.
    pub(in super::super) fn require_capture_through(&mut self, through: u64) {
        if let Some(source) = &mut self.source
            && let Some(cut) = &mut source.input_cut {
                cut.required_through = cut.required_through.max(through);
            }
    }
}

#[cfg(test)]
mod tests;
