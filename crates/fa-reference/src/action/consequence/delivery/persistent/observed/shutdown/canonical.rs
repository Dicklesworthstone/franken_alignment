//! Passive resolution of ambiguous per-domain shutdown acknowledgments.
use super::*;
use super::campaign::observe;
use super::super::{Event, Machine, Transition, storage};
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

    fn read_canonical(&self, index: usize)
        -> Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>
    {
        let domain = &self.plan.domains[index];
        let path = storage::identity(&domain.directory)?;
        if path != domain.directory { return Err(Error::Binding.into()); }
        let bytes = storage::read(&path.join(storage::CANONICAL), domain.profile.delivery.limits.bytes)?;
        self.check_head_size(index, bytes.len())?;
        let events = journal::decode(&domain.profile, &path, &bytes)?;
        self.check_prefix(index, &domain.profile, &path, &events)?;
        // Never execute tokens or predictions under a newly supplied numerical
        // configuration outside the independently retained/acknowledged prefix.
        // Register after numerical bootstrap, or acknowledge the actual live
        // owner first. Native replay still validates EVERY existing transition.
        if events[self.slots[index].revision..].iter().any(|event| matches!(event,
            Event::Decoder(DecoderEvent::Enable(_))
            | Event::Consistency(ConsistencyEvent::Enable(_))))
        { return Err(Error::Binding.into()); }
        let mut machine = Machine::new(&domain.profile)?;
        let mut last_drain = None;
        for (position, event) in events.iter().enumerate() {
            if let Transition::StopProgressed(sweep) = machine.apply(event)? {
                last_drain = Some(FileShutdownDrain { journal_revision: (position + 1) as u64, sweep });
            }
        }
        let observation = observe(domain, &machine, &events, FileShutdownSource::CanonicalImage, last_drain)?;
        Ok((observation, bytes.into(), events.len()))
    }
}
