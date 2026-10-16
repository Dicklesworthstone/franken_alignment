//! Prepare an original paired checkpoint with the SAME bounded replay cursor.
use super::{DecoderEvent, Error, Event, FileLearnedStepPreparation,
    FileLearnedStepPreparationStatus, FileOversight, JournalError, LearnedEvent,
    LearnedStepIntent, Machine, Operation, Rc, fmt};
use super::super::{Transition, journal, checkpoint::{CheckpointEvent, FileLearnedCheckpoint}};

/// No capture or numerical step has run yet. The position/epoch are fixed;
/// explicit catch-up may adopt later source/time events, never a new numerical
/// predecessor. The final checkpoint records that fully verified control cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedCheckpointPreparationProgress {
    pub journal_revision: u64,
    pub checkpoint: u64,
    pub actor_revision: u64,
    pub authority_epoch: u64,
    pub position: u64,
    pub replayed_events: usize,
    pub total_events: usize,
    pub status: FileLearnedStepPreparationStatus,
}

/// Private reconstruction for ONE fresh original checkpoint. This is not an
/// owner, restore instruction, checkpoint image, numerical step or effect key.
/// Dropping it leaves no capture, incident, refund or journal entry. Its issuer
/// marker does not keep the original authority's Store lock alive.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedCheckpointPreparation;
/// fn duplicate(run: FileLearnedCheckpointPreparation) { run.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedCheckpointPreparation;
/// fn bypass(run: FileLearnedCheckpointPreparation) { run.sample(); }
/// ```
#[must_use = "finish the original checkpoint preparation or drop it without a capture"]
pub struct FileLearnedCheckpointPreparation { replay: FileLearnedStepPreparation }
impl fmt::Debug for FileLearnedCheckpointPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedCheckpointPreparation").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Check a quiet original numerical/authority pair and one ORDINARY event
    /// slot before starting cold reconstruction. Does not take the live control
    /// replay cache. For a previously captured ID, use learned_checkpoint or the
    /// original capture_learned_checkpoint exact-retry API, not a fresh task.
    /// The original capture engine still excludes text-stream owners and
    /// audit-only restart profiles; this scheduler does not admit either one.
    pub fn prepare_learned_checkpoint(&self, revision: u64, checkpoint: u64,
        actor_revision: u64, authority_epoch: u64)
        -> Result<FileLearnedCheckpointPreparation, JournalError>
    {
        let position = self.learned_generation_inspection()?.numerical.position;
        let intent = LearnedStepIntent { actor_revision, position };
        self.check_learned_checkpoint_preparation(revision, checkpoint, intent, authority_epoch)?;
        Ok(FileLearnedCheckpointPreparation { replay: FileLearnedStepPreparation {
            issuer: Rc::clone(&self.issuer), revision, intent, total: self.events.len(), replayed: 0,
            status: FileLearnedStepPreparationStatus::Replaying,
            candidate: Machine::new(&self.profile)?,
            operation: Operation::Checkpoint { checkpoint, authority_epoch },
        } })
    }

    pub(super) fn check_learned_checkpoint_preparation(&self, revision: u64,
        checkpoint: u64, intent: LearnedStepIntent, authority_epoch: u64) -> Result<(), JournalError>
    {
        self.check_learned_event_capacity(revision, 1)?;
        let numerical = self.learned_generation_inspection()?.numerical;
        if numerical.actor_revision != intent.actor_revision || numerical.position != intent.position {
            return Err(Error::Stale.into());
        }
        self.machine.check_learned_capture(checkpoint, intent.actor_revision, authority_epoch)?;
        self.check_source_admission(&Event::Decoder(DecoderEvent::Learned(
            shape(checkpoint, intent.actor_revision, authority_epoch))))?;
        Ok(())
    }
}

pub(super) fn shape(checkpoint: u64, actor_revision: u64, authority_epoch: u64) -> LearnedEvent {
    LearnedEvent::Checkpoint(CheckpointEvent::Capture { checkpoint, actor_revision,
        epoch: authority_epoch, witness: Rc::from(&b""[..]) })
}

impl FileLearnedCheckpointPreparation {
    fn selection(&self) -> (u64, u64) {
        match self.replay.operation {
            Operation::Checkpoint { checkpoint, authority_epoch } => (checkpoint, authority_epoch),
            _ => unreachable!("private checkpoint preparation"),
        }
    }
    pub fn progress(&self) -> FileLearnedCheckpointPreparationProgress {
        let progress = self.replay.progress();
        let (checkpoint, authority_epoch) = self.selection();
        FileLearnedCheckpointPreparationProgress { journal_revision: progress.journal_revision,
            checkpoint, actor_revision: progress.intent.actor_revision, authority_epoch,
            position: progress.intent.position, replayed_events: progress.replayed_events,
            total_events: progress.total_events, status: progress.status }
    }

    /// Strict owner/cut/cursor admission and at most max_events original reducer
    /// calls. No capture, checkpoint bytes or handle escapes while replaying.
    pub fn advance(&mut self, host: &FileOversight, expected_events: usize, max_events: usize)
        -> Result<FileLearnedCheckpointPreparationProgress, JournalError>
    {
        self.replay.advance(host, expected_events, max_events)?;
        Ok(self.progress())
    }

    /// Explicitly replay an appended tail on the SAME owner. Recheck the original
    /// quiet-capture, position, epoch, source and capacity laws before adopting
    /// anything. Changed inference, reset, pause or fencing cannot be rebased.
    /// No guarantee of progress under unbounded source/control write churn.
    pub fn catch_up(&mut self, host: &FileOversight, expected_revision: u64,
        expected_events: usize, max_events: usize) -> Result<FileLearnedCheckpointPreparationProgress, JournalError>
    {
        self.replay.catch_up(host, expected_revision, expected_events, max_events)?;
        Ok(self.progress())
    }

    /// Capture the actual paired state on the fully replayed predecessor, then
    /// persist its ORIGINAL witness through the existing acknowledgment boundary.
    /// There is no second historical replay or new sampled token. The handle is
    /// returned only after persistence; errors retain original storage poisoning.
    /// Capture, encoding and I/O are synchronous, not hard-latency-bounded work.
    pub fn finish(self, host: &mut FileOversight) -> Result<FileLearnedCheckpoint, JournalError> {
        let (checkpoint, authority_epoch) = self.selection();
        let replay = self.replay;
        replay.check_status()?;
        replay.check_owner_cut(host)?;
        if replay.status != FileLearnedStepPreparationStatus::Ready { return Err(Error::Incomplete.into()); }
        host.check_learned_checkpoint_preparation(replay.revision, checkpoint, replay.intent, authority_epoch)?;
        host.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = replay.candidate;
        let capture = candidate.prepare_learned_capture(checkpoint, replay.intent.actor_revision, authority_epoch)?;
        let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(capture)));
        let bytes = journal::encode_appended(&host.profile, host.store.identity(), &host.events, &event)?;
        host.persist_candidate(event, bytes, candidate, Transition::Unit)?;
        host.learned_checkpoint(checkpoint)
    }
}
