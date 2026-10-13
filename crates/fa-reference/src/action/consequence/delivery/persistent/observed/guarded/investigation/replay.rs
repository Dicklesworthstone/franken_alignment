//! Read-only event-bounded diagnosis of one independently anchored canonical image.
//! Neither a partial proposal nor reconstructed live authority can escape.
use super::*;
use super::super::anchored::FileHistoryAnchor;
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileProposalReadStatus {
    Reading,
    Ready,
    Interrupted,
    Failed(Error),
    Cancelled,
}

/// Original transition attempts, not physical I/O, CPU time or a valid prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileProposalReadProgress {
    pub replayed_events: usize,
    pub attempted_events: usize,
    pub total_events: usize,
    pub status: FileProposalReadStatus,
}

/// A frozen read-only image, not a recovered owner. Opening reads and decodes
/// bounded canonical bytes and checks the independent anchor and numerical
/// profile. Subsequent calls execute at most the requested original events.
/// A single event, framing and guard validation remain synchronous.
///
/// No writer lock, source refresh, staging cleanup or journal write occurs.
/// Concurrent writers may advance after capture: finish describes THIS image,
/// not a latest-state or durability acknowledgment. Drop/cancel never refunds
/// work or exposes a partially validated proposal. Complete suffix validation
/// and the full independent ending guard contract are required before finish.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalRead;
/// fn no_fork(read: FileProposalRead) { let _ = read.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalRead;
/// fn no_partial(read: &FileProposalRead) { let _ = read.origin(); }
/// ```
#[must_use = "finish a complete read or discard its private reconstruction"]
pub struct FileProposalRead {
    directory: PathBuf,
    profile: FileOversightProfile,
    expected: FileRecoveryRequirements,
    events: Vec<Event>,
    machine: Machine,
    selection: Selection,
    replayed: usize,
    attempted: usize,
    status: FileProposalReadStatus,
}
impl fmt::Debug for FileProposalRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileProposalRead").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Capture one bounded image and require an independently retained exact
    /// prefix BEFORE executing any historical event. This does not acknowledge
    /// the image, acquire a writer lock, or infer requirements from journal data.
    pub fn begin_read_proposal_investigation_anchored(
        directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor,
        attempt: u64, scope: InterventionScope,
    ) -> Result<FileProposalRead, JournalError> {
        let selection = Selection::new(attempt, scope)?;
        profile.delivery.limits.check()?;
        expected.guards.validate()?;
        let directory = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&directory.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &directory, &bytes)?;
        anchor.check(profile, &directory, &events)?;
        expected.guards.check_decoder_config(&events)?;
        let machine = Machine::new(profile)?;
        Ok(FileProposalRead { directory, profile: profile.clone(), expected: expected.clone(),
            events, machine, selection, replayed: 0, attempted: 0, status: FileProposalReadStatus::Reading })
    }

    /// Convenience whole-image consumer of the same sealed read. The explicit
    /// cursor lets hosts check cancellation before each event and again at Ready.
    pub fn read_proposal_investigation_anchored(
        directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor,
        attempt: u64, scope: InterventionScope,
    ) -> Result<FileProposalInvestigation, JournalError> {
        let mut read = Self::begin_read_proposal_investigation_anchored(
            directory, profile, expected, anchor, attempt, scope)?;
        while read.status == FileProposalReadStatus::Reading {
            read.advance(read.replayed, MAX_JOURNAL_EVENTS)?;
        }
        read.finish()
    }
}

impl FileProposalRead {
    pub fn progress(&self) -> FileProposalReadProgress {
        FileProposalReadProgress { replayed_events: self.replayed, attempted_events: self.attempted,
            total_events: self.events.len(), status: self.status }
    }

    /// Stale or invalid polls do no work. Interrupted/failed execution latches
    /// permanently, including an unwind after the original reducer changed RAM.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileProposalReadProgress, JournalError>
    {
        self.advance_with(expected_events, max_events, Selection::apply)
    }

    fn advance_with(&mut self, expected_events: usize, max_events: usize,
        mut apply: impl FnMut(&mut Selection, &FileOversightProfile, &mut Machine, usize, &Event)
            -> Result<(), JournalError>) -> Result<FileProposalReadProgress, JournalError>
    {
        match self.status {
            FileProposalReadStatus::Failed(error) => return Err(error.into()),
            FileProposalReadStatus::Interrupted => return Err(JournalError::Unavailable),
            FileProposalReadStatus::Cancelled => return Err(Error::WrongState.into()),
            _ => {}
        }
        if expected_events != self.replayed { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        if self.status == FileProposalReadStatus::Ready { return Ok(self.progress()); }
        let end = self.replayed.saturating_add(max_events).min(self.events.len());
        while self.replayed < end {
            self.status = FileProposalReadStatus::Interrupted;
            self.attempted += 1;
            let result = apply(&mut self.selection, &self.profile, &mut self.machine,
                self.replayed, &self.events[self.replayed]);
            if let Err(error) = result {
                if let JournalError::Contract(contract) = &error {
                    self.status = FileProposalReadStatus::Failed(*contract);
                }
                return Err(error);
            }
            self.replayed += 1;
        }
        self.status = FileProposalReadStatus::Interrupted;
        if self.replayed == self.events.len() {
            let result = self.expected.check(&self.profile, &self.machine, &self.events)
                .and_then(|_| self.selection.require_selected());
            if let Err(error) = result {
                self.status = FileProposalReadStatus::Failed(error);
                return Err(error.into());
            }
            self.status = FileProposalReadStatus::Ready;
        } else { self.status = FileProposalReadStatus::Reading; }
        Ok(self.progress())
    }

    pub fn finish(self) -> Result<FileProposalInvestigation, JournalError> {
        match self.status {
            FileProposalReadStatus::Ready => {}
            FileProposalReadStatus::Failed(error) => return Err(error.into()),
            FileProposalReadStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        self.expected.check(&self.profile, &self.machine, &self.events)?;
        self.selection.finish(&self.directory, &self.machine, self.events.len())
    }

    pub fn cancel(self) -> FileProposalReadProgress {
        let mut progress = self.progress();
        if matches!(progress.status, FileProposalReadStatus::Reading | FileProposalReadStatus::Ready) {
            progress.status = FileProposalReadStatus::Cancelled;
        }
        progress
    }

    #[cfg(test)]
    pub(super) fn interrupt_after_event(&mut self) {
        let _ = self.advance_with(self.replayed, 1, |selection, profile, machine, index, event| {
            selection.apply(profile, machine, index, event)?;
            panic!("injected unwind after actual historical event");
        });
    }
}
