//! Private passive replay: one admitted file image, no Store lock or domain writes.
use super::super::{FileShutdownDomain, FileShutdownDrain, FileShutdownObservation,
    FileShutdownSource, JournalError, campaign::observe};
use super::super::super::{Event, Machine, Transition};
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS,
    observed::shutdown::recovery::{FileStopRecoveryProgress, FileStopRecoveryStatus}};
use crate::Error;
use std::rc::Rc;

type Observation = (FileShutdownObservation, Rc<[u8]>, usize);

/// Shared by ordinary passive inspection and resumable visit resolution. The
/// original registered prefix, numerical-bootstrap and head-space checks run
/// before construction. The entire suffix must pass before observation export.
pub(in super::super) struct CanonicalRead {
    domain: FileShutdownDomain,
    bytes: Rc<[u8]>,
    events: Vec<Event>,
    machine: Machine,
    last_drain: Option<FileShutdownDrain>,
    replayed: usize,
    attempted: usize,
    status: FileStopRecoveryStatus,
}
impl CanonicalRead {
    pub(super) fn new(domain: FileShutdownDomain, bytes: Vec<u8>, events: Vec<Event>)
        -> Result<Self, JournalError>
    {
        let machine = Machine::new(&domain.profile)?;
        Ok(Self { domain, bytes: bytes.into(), events, machine, last_drain: None,
            replayed: 0, attempted: 0, status: FileStopRecoveryStatus::Replaying })
    }

    pub(in super::super) fn progress(&self) -> FileStopRecoveryProgress {
        FileStopRecoveryProgress { replayed_events: self.replayed, attempted_events: self.attempted,
            total_events: self.events.len(), status: self.status }
    }

    pub(in super::super) fn advance(&mut self, expected: usize, maximum: usize)
        -> Result<FileStopRecoveryProgress, JournalError>
    {
        self.advance_with(expected, maximum, Machine::apply)
    }

    fn advance_with(&mut self, expected: usize, maximum: usize,
        mut apply: impl FnMut(&mut Machine, &Event) -> Result<Transition, Error>)
        -> Result<FileStopRecoveryProgress, JournalError>
    {
        match self.status {
            FileStopRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            FileStopRecoveryStatus::Failed(error) => return Err(error.into()),
            FileStopRecoveryStatus::Cancelled => return Err(Error::WrongState.into()),
            _ => {}
        }
        if expected != self.replayed { return Err(Error::Stale.into()); }
        if maximum == 0 { return Err(Error::InvalidInput.into()); }
        if maximum > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        let end = self.replayed + maximum.min(self.events.len() - self.replayed);
        while self.replayed < end {
            self.status = FileStopRecoveryStatus::Interrupted;
            self.attempted += 1;
            let result = match apply(&mut self.machine, &self.events[self.replayed]) {
                Ok(result) => result,
                Err(error) => {
                    self.status = FileStopRecoveryStatus::Failed(error);
                    return Err(error.into());
                }
            };
            if let Transition::StopProgressed(sweep) = result {
                self.last_drain = Some(FileShutdownDrain {
                    journal_revision: (self.replayed + 1) as u64, sweep,
                });
            }
            self.replayed += 1;
        }
        self.status = if self.replayed == self.events.len() { FileStopRecoveryStatus::Ready }
            else { FileStopRecoveryStatus::Replaying };
        Ok(self.progress())
    }

    pub(in super::super) fn finish(self) -> Result<Observation, JournalError> {
        match self.status {
            FileStopRecoveryStatus::Ready => {}
            FileStopRecoveryStatus::Failed(error) => return Err(error.into()),
            FileStopRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        let observation = observe(&self.domain, &self.machine, &self.events,
            FileShutdownSource::CanonicalImage, self.last_drain)?;
        Ok((observation, self.bytes, self.events.len()))
    }

    pub(in super::super) fn complete(mut self) -> Result<Observation, JournalError> {
        while self.status == FileStopRecoveryStatus::Replaying {
            self.advance(self.replayed, MAX_JOURNAL_EVENTS)?;
        }
        self.finish()
    }

    #[cfg(test)]
    pub(in super::super) fn interrupt_after_original_event(&mut self) {
        let _ = self.advance_with(self.replayed, 1, |machine, event| {
            let _ = machine.apply(event)?;
            panic!("interrupted passive shutdown replay after original event");
        });
    }
}
