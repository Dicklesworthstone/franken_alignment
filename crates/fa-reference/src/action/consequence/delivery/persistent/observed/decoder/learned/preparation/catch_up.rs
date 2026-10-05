//! Explicit bounded catch-up over the SAME live owner's append-only journal.
//! Never skip a new transition or silently replace the requested numerical step.
use super::{Error, FileLearnedStepPreparation, FileLearnedStepPreparationProgress,
    FileLearnedStepPreparationStatus, FileOversight, JournalError, Operation, Rc,
    MAX_JOURNAL_EVENTS};

impl FileLearnedStepPreparation {
    /// Adopt exactly the caller-observed live journal revision and replay at
    /// most `max_events` transitions from the existing cursor. Unlike `advance`,
    /// this explicitly admits an appended tail without repeating verified work.
    /// `expected_revision` names the LIVE owner, not the previous task revision.
    ///
    /// Only the same append-only owner and the same still-admissible numerical
    /// operation qualify. New time observations, actor cancellations and source
    /// transactions must run through the original reducer before Ready. A new
    /// numerical predecessor, pending reset, fence/pause, source interruption or
    /// failed store cannot be repaired by adopting a larger revision number.
    ///
    /// Preflight refusals change neither cursor nor candidate. Once replay starts,
    /// original reducer failure/unwind remains sticky. No write, key or output
    /// occurs here. The strict advance/finish APIs never opt into catch-up by
    /// themselves; finishing still requires every adopted event and the exact
    /// current cut. This does not guarantee progress under unbounded write churn.
    pub fn catch_up(&mut self, host: &FileOversight, expected_revision: u64,
        expected_events: usize, max_events: usize)
        -> Result<FileLearnedStepPreparationProgress, JournalError>
    {
        self.check_status()?;
        if expected_events != self.replayed { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if expected_revision != host.revision() || expected_revision < self.revision {
            return Err(Error::Stale.into());
        }
        // Identity pins a single live owner. Its acknowledged event vector only
        // appends, so our already-verified prefix cannot be replaced underneath
        // this cursor. No disk bytes, new owner or deserialized prefix is adopted.
        // Recheck the complete original law BEFORE changing the captured cut.
        match self.operation {
            Operation::Intent => host.check_learned_intent_preparation(expected_revision, self.intent)?,
            Operation::Completion => host.check_learned_completion(expected_revision,
                self.intent.actor_revision, self.intent.position)?,
        }
        self.revision = expected_revision;
        self.total = host.events.len();
        if self.replayed < self.total { self.status = FileLearnedStepPreparationStatus::Replaying; }
        // One reducer and one cursor algorithm; this visits all adopted records,
        // including any unread portion of the original prefix, exactly once.
        self.advance(host, expected_events, max_events)
    }
}
