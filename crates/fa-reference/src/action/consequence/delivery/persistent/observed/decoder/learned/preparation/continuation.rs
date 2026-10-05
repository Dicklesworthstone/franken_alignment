//! Move a previously acknowledged ORIGINAL machine forward, never restore a
//! caller-supplied checkpoint or clone an authority. The tail still executes.
use super::{Error, FileLearnedIntentPreparation, FileLearnedStepPreparation,
    FileLearnedStepPreparationStatus, FileOversight, JournalError, LearnedStepIntent,
    Machine, Operation, Rc, fmt};

/// Opaque process-local replay custody, created only by an acknowledged learned
/// finalizer. This is the former live machine MOVED, not a clone, a deserialized
/// snapshot, a fresh observation, or an executable effect owner. It has no store
/// and exports no old broker, writer, helper session, reviewer, permit or sample.
/// Its verified prefix belongs to one append-only live owner's journal.
///
/// Dropping this value changes no journal, pending operation or effect balance.
/// Preparing consumes it even on refusal; losing this optimization is not losing
/// authority or refunding numerical work. Recovery must still independently
/// bind the recipe and replay the whole canonical journal under a fresh owner.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedReplayContinuation;
/// fn duplicate(carry: FileLearnedReplayContinuation) { carry.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedReplayContinuation;
/// fn restore_authority(carry: FileLearnedReplayContinuation) { carry.owner(); }
/// ```
#[must_use = "consume for another learned preparation or drop without changing live state"]
pub struct FileLearnedReplayContinuation {
    issuer: Rc<()>,
    through: usize,
    candidate: Machine,
}
impl fmt::Debug for FileLearnedReplayContinuation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedReplayContinuation")
            .field("verified_events", &self.through).finish_non_exhaustive()
    }
}
impl FileLearnedReplayContinuation {
    // Callers in this module family supply ONLY the actual retired machine and
    // its pre-write length, AFTER the same canonical acknowledgment succeeds.
    pub(super) fn new(issuer: Rc<()>, through: usize, candidate: Machine) -> Self {
        Self { issuer, through, candidate }
    }

    /// The prefix already represented by the original acknowledged machine.
    /// The event that retired it is NOT included and must still be replayed.
    pub fn verified_events(&self) -> usize { self.through }

    /// Prepare a new intent under the ORIGINAL next-position, pending, source,
    /// recovery and ordinary-capacity laws. Every event after the retained
    /// prefix, including the previous numerical outcome, is replayed normally.
    /// No next token or new intent is produced merely by accepting this value.
    pub fn prepare_intent(self, host: &FileOversight, revision: u64,
        actor_revision: u64, position: u64) -> Result<FileLearnedIntentPreparation, JournalError>
    {
        self.check_owner(host, revision)?;
        let intent = LearnedStepIntent { actor_revision, position };
        host.check_learned_intent_preparation(revision, intent)?;
        Ok(FileLearnedIntentPreparation { replay: self.into_preparation(host, intent, Operation::Intent) })
    }

    /// Prepare only the exact already-acknowledged pending operation. Retaining
    /// a pre-Begin machine does not waive the intent: its Begin event must pass
    /// the original reducer before this preparation becomes Ready.
    pub fn prepare_completion(self, host: &FileOversight, revision: u64,
        actor_revision: u64, position: u64) -> Result<FileLearnedStepPreparation, JournalError>
    {
        self.check_owner(host, revision)?;
        host.check_learned_completion(revision, actor_revision, position)?;
        Ok(self.into_preparation(host, LearnedStepIntent { actor_revision, position }, Operation::Completion))
    }

    fn check_owner(&self, host: &FileOversight, revision: u64) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if host.revision() != revision { return Err(Error::Stale.into()); }
        if self.through > host.events.len() { return Err(Error::Binding.into()); }
        Ok(())
    }

    fn into_preparation(self, host: &FileOversight, intent: LearnedStepIntent,
        operation: Operation) -> FileLearnedStepPreparation
    {
        let total = host.events.len();
        FileLearnedStepPreparation { issuer: self.issuer, revision: host.revision(), intent,
            total, replayed: self.through, candidate: self.candidate, operation,
            status: if self.through == total { FileLearnedStepPreparationStatus::Ready }
                else { FileLearnedStepPreparationStatus::Replaying } }
    }
}

impl FileOversight {
    /// Release only the process-local historical replay cache. No journal,
    /// numerical budget, pending intent, observation, epoch or effect changes.
    /// The next ordinary learned call can reconstruct from the original journal.
    pub fn clear_learned_replay_cache(&mut self) { self.learned_replay = None; }

    // Preserve stale-revision/fault refusal before consuming a cache. Other
    // admission failures may discard this optimization but cannot change the
    // original live state. In particular there is no cold retry after a failed
    // cached replay that could hide divergent history or repeat failed work.
    pub(in super::super) fn cached_learned_intent(&mut self, revision: u64,
        actor_revision: u64, position: u64) -> Result<FileLearnedIntentPreparation, JournalError>
    {
        self.check_cached_revision(revision)?;
        match self.learned_replay.take() {
            Some(carry) => carry.prepare_intent(self, revision, actor_revision, position),
            None => self.prepare_learned_step_intent(revision, actor_revision, position),
        }
    }

    pub(in super::super) fn cached_learned_completion(&mut self, revision: u64,
        actor_revision: u64, position: u64) -> Result<FileLearnedStepPreparation, JournalError>
    {
        self.check_cached_revision(revision)?;
        match self.learned_replay.take() {
            Some(carry) => carry.prepare_completion(self, revision, actor_revision, position),
            None => self.prepare_learned_step_completion(revision, actor_revision, position),
        }
    }

    fn check_cached_revision(&self, revision: u64) -> Result<(), JournalError> {
        if self.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        Ok(())
    }
}
