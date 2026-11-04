//! Bounded replay before completing the ORIGINAL acknowledged reset intent.
use super::{Error, FileLearnedStepPreparation, FileLearnedStepPreparationStatus,
    FileOversight, JournalError, LearnedStepIntent, Machine, Operation, Rc, fmt};
use super::super::{LearnedEvent, checkpoint::{CheckpointEvent, FileLearnedResetIntent}};
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedResetReceipt;

/// Historical replay only; Ready does not mean that the reset audit ran or that
/// the actor was restored. The immutable intent still bounds all future work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedResetPreparationProgress {
    pub journal_revision: u64,
    pub operation: u64,
    pub checkpoint: u64,
    pub replayed_events: usize,
    pub total_events: usize,
    pub status: FileLearnedStepPreparationStatus,
}

/// Private reconstruction for one pending reset. No checkpoint image, restored
/// state, live owner, refund or approval escapes before original acknowledgment.
/// Dropping a task leaves the same durable reset pending. Stop/fence can still
/// interrupt the original owner between replay quanta; this task cannot undo it.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedResetPreparation;
/// fn duplicate(task: FileLearnedResetPreparation) { task.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedResetPreparation;
/// fn authority(task: FileLearnedResetPreparation) { task.owner(); }
/// ```
#[must_use = "finish the original reset preparation or drop it leaving the intent pending"]
pub struct FileLearnedResetPreparation {
    replay: FileLearnedStepPreparation,
    expected: FileLearnedResetIntent,
}
impl fmt::Debug for FileLearnedResetPreparation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedResetPreparation").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Begin cold bounded reconstruction, without taking the live control cache
    /// or executing any original reset audit yet. The exact intent must already
    /// be durably acknowledged, uninterrupted and still admissible. Completed
    /// operation retries use complete_learned_reset, not a new preparation.
    pub fn prepare_learned_reset_completion(&self, revision: u64, operation: u64)
        -> Result<FileLearnedResetPreparation, JournalError>
    {
        self.check_learned_reset_completion(revision, operation)?;
        let expected = self.machine.pending_learned_reset().ok_or(Error::Incomplete)?.intent.clone();
        let numerical = self.learned_generation_inspection()?.numerical;
        Ok(FileLearnedResetPreparation {
            expected,
            replay: FileLearnedStepPreparation {
                issuer: Rc::clone(&self.issuer), revision,
                intent: LearnedStepIntent { actor_revision: numerical.actor_revision, position: numerical.position },
                total: self.events.len(), replayed: 0,
                status: FileLearnedStepPreparationStatus::Replaying,
                candidate: Machine::new(&self.profile)?, operation: Operation::ResetCompletion { operation },
            },
        })
    }
}

pub(super) fn shape(operation: u64) -> LearnedEvent {
    LearnedEvent::Checkpoint(CheckpointEvent::Reset { operation, witness: Rc::from(&b""[..]) })
}

impl FileLearnedResetPreparation {
    pub fn progress(&self) -> FileLearnedResetPreparationProgress {
        let progress = self.replay.progress();
        FileLearnedResetPreparationProgress { journal_revision: progress.journal_revision,
            operation: self.expected.control().operation, checkpoint: self.expected.checkpoint(),
            replayed_events: progress.replayed_events, total_events: progress.total_events,
            status: progress.status }
    }

    fn check_intent(&self, host: &FileOversight) -> Result<(), JournalError> {
        self.replay.check_status()?;
        if !Rc::ptr_eq(&self.replay.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        let pending = host.machine.pending_learned_reset().ok_or(Error::Incomplete)?;
        if pending.intent != self.expected { return Err(Error::Binding.into()); }
        host.machine.check_pending_learned_reset(self.expected.control().operation)?;
        Ok(())
    }

    /// At most max_events original reductions with the shared failure/unwind
    /// latch. A changed live cut refuses; no automatic rebase or cold retry.
    pub fn advance(&mut self, host: &FileOversight, expected_events: usize, max_events: usize)
        -> Result<FileLearnedResetPreparationProgress, JournalError>
    {
        self.check_intent(host)?;
        self.replay.advance(host, expected_events, max_events)?;
        Ok(self.progress())
    }

    /// Explicitly replay appended control history while retaining the entire
    /// original intent, including predecessor, binding, target ceiling and audit
    /// budget. A fence, changed authority or completed reset cannot be adopted.
    pub fn catch_up(&mut self, host: &FileOversight, expected_revision: u64,
        expected_events: usize, max_events: usize) -> Result<FileLearnedResetPreparationProgress, JournalError>
    {
        self.check_intent(host)?;
        self.replay.catch_up(host, expected_revision, expected_events, max_events)?;
        Ok(self.progress())
    }

    /// Run the original audit/restore once on this verified predecessor and
    /// persist its original witness. Outer Err exposes no acknowledged result;
    /// inner Err is an acknowledged native refusal, never a successful restore.
    /// The original poison-before-audit boundary is shared with synchronous
    /// completion. No second history replay, budget refill or liability refund.
    /// One audit, encoding and filesystem replacement remain synchronous.
    pub fn finish(self, host: &mut FileOversight)
        -> Result<Result<HostedLearnedResetReceipt, Error>, JournalError>
    {
        self.check_intent(host)?;
        self.replay.check_owner_cut(host)?;
        if self.replay.status != FileLearnedStepPreparationStatus::Ready { return Err(Error::Incomplete.into()); }
        let operation = self.expected.control().operation;
        if self.replay.operation != (Operation::ResetCompletion { operation }) {
            return Err(Error::Binding.into());
        }
        let retain_replay = host.learned_replay.is_some();
        host.persist_prepared_learned_reset(self.replay.revision, operation, self.replay.candidate, retain_replay)
    }
}
