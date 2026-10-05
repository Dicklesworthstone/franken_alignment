//! Cooperatively prepare the ORIGINAL write-ahead intent before completion.
//! Distinct finalizers prevent intent preparation from returning numerical output.
use super::{Error, Event, DecoderEvent, FileOversight, FileLearnedStepPreparation,
    FileLearnedStepPreparationProgress, FileLearnedStepPreparationStatus,
    JournalError, LearnedEvent, LearnedStepIntent, Machine, Operation, Rc};
use super::super::journal;

/// Reconstructed intent admission, not an already admitted numerical operation.
/// Dropping it before successful finish leaves NO new durable intent. After
/// finish, use prepare_learned_step_completion to prepare the separate outcome.
/// Both stages yield during original history replay; neither exposes live keys.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{FileLearnedIntentPreparation, FileLearnedStepPreparation};
/// fn skip_intent(task: FileLearnedIntentPreparation) -> FileLearnedStepPreparation { task }
/// ```
#[derive(Debug)]
#[must_use = "advance and finish intent admission, or drop it without recording an intent"]
pub struct FileLearnedIntentPreparation { replay: FileLearnedStepPreparation }

impl FileOversight {
    /// Check the ORIGINAL next-position and two-ordinary-slot admission rules,
    /// then return a private empty reconstruction. No numerical history, new
    /// token, journal intent or snapshot consumption occurs at construction.
    pub fn prepare_learned_step_intent(&self, revision: u64, actor_revision: u64,
        position: u64) -> Result<FileLearnedIntentPreparation, JournalError>
    {
        let intent = LearnedStepIntent { actor_revision, position };
        self.check_learned_intent_preparation(revision, intent)?;
        Ok(FileLearnedIntentPreparation { replay: FileLearnedStepPreparation {
            issuer: Rc::clone(&self.issuer), revision, intent, total: self.events.len(), replayed: 0,
            status: FileLearnedStepPreparationStatus::Replaying,
            candidate: Machine::new(&self.profile)?, operation: Operation::Intent,
        } })
    }

    fn check_learned_intent_preparation(&self, revision: u64, intent: LearnedStepIntent)
        -> Result<(), JournalError>
    {
        self.check_learned_event_capacity(revision, 2)?;
        let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Begin(intent)));
        self.machine.check_generated_text_origin(&event)?;
        self.check_source_admission(&event)?;
        self.machine.preflight_learned_intent(intent)?;
        Ok(())
    }
}

impl FileLearnedIntentPreparation {
    pub fn progress(&self) -> FileLearnedStepPreparationProgress { self.replay.progress() }

    /// Same exact-owner/cursor checks and original reducer as pending completion.
    /// Supervisor mutations invalidate this cut. No automatic restart or rebase.
    pub fn advance(&mut self, host: &FileOversight, expected_events: usize,
        max_events: usize) -> Result<FileLearnedStepPreparationProgress, JournalError>
    {
        self.replay.advance(host, expected_events, max_events)
    }

    /// Persist ONLY the original Begin event after complete reconstruction and
    /// fresh original preflight. History is not replayed again. No new inference
    /// runs here and no candidate state escapes before canonical acknowledgment.
    /// On storage error the owner is poisoned by the same persist_candidate cut;
    /// recovery decides whether the intent became visible. No outcome is implied.
    pub fn finish(self, host: &mut FileOversight) -> Result<(), JournalError> {
        let replay = self.replay;
        replay.check_status()?;
        replay.check_owner_cut(host)?;
        if replay.status != FileLearnedStepPreparationStatus::Ready { return Err(Error::Incomplete.into()); }
        host.check_learned_intent_preparation(replay.revision, replay.intent)?;
        let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Begin(replay.intent)));
        // Same encoding, consistency preflight, original apply and persistence
        // order as transact(Begin). Only its historical replay was scheduled.
        let bytes = journal::encode_appended(&host.profile, host.store.identity(), &host.events, &event)?;
        let mut candidate = replay.candidate;
        candidate.preflight_consistency(&event)?;
        let result = candidate.apply(&event)?;
        host.persist_candidate(event, bytes, candidate, result)?;
        Ok(())
    }
}
