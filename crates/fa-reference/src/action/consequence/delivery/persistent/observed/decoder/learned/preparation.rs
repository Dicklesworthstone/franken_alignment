//! Cooperative reconstruction for one ORIGINAL pending numerical completion.
//! The live owner remains available; any journal mutation invalidates this cut.
use super::{DecoderEvent, Error, Event, FileOversight, GenerationEvent,
    JournalError, LearnedEvent, LearnedStepIntent, Machine};
use super::super::super::super::MAX_JOURNAL_EVENTS;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileLearnedStepPreparationStatus {
    Replaying,
    /// History is verified, but the next numerical step has NOT executed.
    Ready,
    /// A reducer did not return. This private candidate cannot be resumed.
    Interrupted,
    Failed(Error),
}

/// Historical work only. Counts exclude the next numerical operation and are
/// not CPU, physical-computation escrow, wall-clock or cancellation-latency bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedStepPreparationProgress {
    pub journal_revision: u64,
    pub intent: LearnedStepIntent,
    pub replayed_events: usize,
    pub total_events: usize,
    pub status: FileLearnedStepPreparationStatus,
}

/// One private candidate, bound to the exact live owner and journal revision.
/// No live host borrow survives an advance: actor polling and supervisory
/// control remain possible between quanta. No source, key or output escapes.
/// Dropping this value leaves the ORIGINAL durable intent pending and changes
/// neither the canonical file nor the original numerical/effect accounting.
/// The identity marker does not hold the owner's store lock alive.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedStepPreparation;
/// fn early_output(prepared: FileLearnedStepPreparation) { prepared.sample(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedStepPreparation;
/// fn duplicate(prepared: FileLearnedStepPreparation) { prepared.clone(); }
/// ```
#[must_use = "advance and finish this preparation, or drop it leaving its original intent pending"]
pub struct FileLearnedStepPreparation {
    issuer: Rc<()>,
    revision: u64,
    intent: LearnedStepIntent,
    total: usize,
    replayed: usize,
    status: FileLearnedStepPreparationStatus,
    candidate: Machine,
}
impl fmt::Debug for FileLearnedStepPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedStepPreparation").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Start reconstructing the exact pending step's predecessor, with no
    /// historical inference or journal write yet. Capacity is checked before
    /// work; the promise is event slots, not future witness bytes or disk space.
    /// The original pending intent must already have been acknowledged.
    pub fn prepare_learned_step_completion(&self, revision: u64, actor_revision: u64,
        position: u64) -> Result<FileLearnedStepPreparation, JournalError>
    {
        self.check_learned_completion(revision, actor_revision, position)?;
        Ok(FileLearnedStepPreparation {
            issuer: Rc::clone(&self.issuer), revision,
            intent: LearnedStepIntent { actor_revision, position },
            total: self.events.len(), replayed: 0,
            status: FileLearnedStepPreparationStatus::Replaying,
            candidate: Machine::new(&self.profile)?,
        })
    }
}

impl FileLearnedStepPreparation {
    pub fn progress(&self) -> FileLearnedStepPreparationProgress {
        FileLearnedStepPreparationProgress { journal_revision: self.revision,
            intent: self.intent, replayed_events: self.replayed, total_events: self.total,
            status: self.status }
    }

    // Even a READY candidate cannot cross a changed owner, journal cut, live
    // source-interruption latch or failed write. Pending identity never changes
    // without a journal transition, so the original completion checks suffice.
    fn check_owner_cut(&self, host: &FileOversight) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if host.revision() != self.revision { return Err(Error::Stale.into()); }
        host.check_source_admission(&Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step {
            actor_revision: self.intent.actor_revision, position: self.intent.position,
            witness: Rc::from(&b""[..]),
        })))?;
        Ok(())
    }

    fn check_status(&self) -> Result<(), JournalError> {
        match self.status {
            FileLearnedStepPreparationStatus::Failed(error) => Err(error.into()),
            FileLearnedStepPreparationStatus::Interrupted => Err(JournalError::Unavailable),
            _ => Ok(()),
        }
    }

    /// Replay at most max_events ORIGINAL journal transitions, including their
    /// witness comparisons. No cached tensor, verdict or serialized candidate
    /// becomes authoritative. One event remains synchronous. Zero/oversized
    /// quanta and stale cursors do no work; the first reducer failure is sticky.
    /// Any intervening live mutation requires a new preparation, never rebasing
    /// this candidate onto an unseen tail or applying it over cancellation.
    pub fn advance(&mut self, host: &FileOversight, expected_events: usize,
        max_events: usize) -> Result<FileLearnedStepPreparationProgress, JournalError>
    {
        self.check_status()?;
        if expected_events != self.replayed { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        self.check_owner_cut(host)?;
        if self.status == FileLearnedStepPreparationStatus::Ready { return Ok(self.progress()); }
        let end = self.replayed.saturating_add(max_events).min(self.total);
        while self.replayed < end {
            self.status = FileLearnedStepPreparationStatus::Interrupted;
            if let Err(error) = self.candidate.apply(&host.events[self.replayed]) {
                self.status = FileLearnedStepPreparationStatus::Failed(error);
                return Err(error.into());
            }
            self.replayed += 1;
            self.status = FileLearnedStepPreparationStatus::Replaying;
        }
        if self.replayed == self.total { self.status = FileLearnedStepPreparationStatus::Ready; }
        Ok(self.progress())
    }

    /// Execute the ONE original next numerical step and persist its original
    /// witness through the existing poison/acknowledgment boundary. There is no
    /// second history replay here. All original admission checks run again.
    /// Encoding, one numerical step and storage replacement remain synchronous.
    /// Only acknowledged completion returns output (or an acknowledged numerical
    /// error); persistence failure exposes neither the candidate nor its result.
    pub fn finish(self, host: &mut FileOversight)
        -> Result<Result<Rc<GenerationEvent>, Error>, JournalError>
    {
        self.check_status()?;
        self.check_owner_cut(host)?;
        if self.status != FileLearnedStepPreparationStatus::Ready { return Err(Error::Incomplete.into()); }
        host.persist_prepared_learned_step(self.revision, self.intent.actor_revision,
            self.intent.position, self.candidate)
    }
}

