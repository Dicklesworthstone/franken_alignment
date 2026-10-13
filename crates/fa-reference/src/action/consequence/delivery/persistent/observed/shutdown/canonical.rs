//! Passive resolution of ambiguous per-domain shutdown acknowledgments.
pub(super) mod replay;
use replay::CanonicalRead;
use super::*;
use super::super::{Event, storage};
use super::super::decoder::DecoderEvent;
use super::super::consistency::ConsistencyEvent;

impl FileShutdownCampaign {
    /// Read one canonical image, including beside a live or faulted owner. This
    /// is NOT a recovered role, writer acknowledgment or remote process halt.
    /// Inspect every fixed member explicitly; a missing file never means stopped.
    /// A prior successful visit pins its entire later prefix, not just counters.
    pub fn inspect_canonical(&mut self, domain: u64) -> Result<FileShutdownObservation, JournalError> {
        let (index, attempt) = self.enter(domain, FileShutdownStep::InspectCanonical)?;
        let result = self.read_canonical(index);
        self.complete(index, attempt, result)
    }

    pub(super) fn read_canonical(&self, index: usize)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        self.begin_canonical_read(index)?.complete()
    }

    pub(super) fn begin_canonical_read(&self, index: usize) -> Result<CanonicalRead, JournalError> {
        let domain = &self.plan.domains[index];
        let path = storage::identity(&domain.directory)?;
        if path != domain.directory { return Err(Error::Binding.into()); }
        let bytes = storage::read(&path.join(storage::CANONICAL), domain.profile.delivery.limits.bytes)?;
        self.check_head_size(index, bytes.len())?;
        let events = journal::decode(&domain.profile, &path, &bytes)?;
        self.check_canonical_events(index, &path, &events)?;
        CanonicalRead::new(domain.clone(), bytes, events)
    }

    /// Shared passive admission. A registered recovery must not execute an
    /// unacknowledged numerical bootstrap merely because it holds the writer lock.
    pub(super) fn check_canonical_events(&self, index: usize, path: &Path, events: &[Event])
        -> Result<(), Error>
    {
        let domain = &self.plan.domains[index];
        self.check_prefix(index, &domain.profile, path, events)?;
        // Never execute tokens or predictions under a newly supplied numerical
        // configuration outside the independently retained/acknowledged prefix.
        // Register after numerical bootstrap, or acknowledge the actual live
        // owner first. Native replay still validates EVERY existing transition.
        if events[self.slots[index].revision..].iter().any(|event| matches!(event,
            Event::Decoder(DecoderEvent::Enable(_))
            | Event::Consistency(ConsistencyEvent::Enable(_))))
        { return Err(Error::Binding); }
        Ok(())
    }
}
