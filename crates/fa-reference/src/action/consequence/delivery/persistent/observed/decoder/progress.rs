//! Acknowledge one original token at a time under an already durable request.
//! Recovery replays the cursor and budgets; callers cannot provide either one.
mod batch;
pub use batch::MAX_FILE_GENERATION_BATCH_STEPS;
pub(super) use batch::check_batch_steps;
use super::{DecoderEvent, FileDecoderConfig, FileDecoderInspection, FileOversight,
    FileOversightProfile, JournalError, Machine, Transition, Event, journal, storage};
use super::generation::{FileGenerationCommand, FileGenerationReceipt};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_GENERATION_TOKENS, incremental::GenerationProgress,
};
use crate::action::consequence::delivery::persistent::{FileDeliverySnapshot, JournalFailure, JournalIo};
use crate::Error;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone, Debug)]
enum Phase {
    Pending { command: Rc<FileGenerationCommand>, progress: Option<GenerationProgress> },
    Recorded(FileGenerationReceipt),
}

/// Supervisor-only data at an acknowledged boundary, not a token-publication
/// permit. Old-revision retries return CURRENT retained progress, so consumers
/// identify tokens by request plus output offset rather than appending twice.
/// A quiet partial prefix cannot be mistaken for a completed generation result.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::progress::FileGenerationProgress;
/// fn publish(progress: FileGenerationProgress) -> Permit { progress }
/// ```
#[derive(Clone, Debug)]
pub struct FileGenerationProgress {
    revision: u64,
    phase: Phase,
}
impl FileGenerationProgress {
    /// Number of acknowledged incremental transitions, not decoder position,
    /// journal revision, a successful-token count or a physical execution count.
    /// Legacy whole-request results have zero incremental transitions.
    pub fn generation_revision(&self) -> u64 { self.revision }
    pub fn command(&self) -> &FileGenerationCommand {
        match &self.phase { Phase::Pending { command, .. } => command, Phase::Recorded(receipt) => receipt.command() }
    }
    pub fn partial(&self) -> Option<&GenerationProgress> {
        match &self.phase { Phase::Pending { progress, .. } => progress.as_ref(), Phase::Recorded(_) => None }
    }
    pub fn receipt(&self) -> Option<&FileGenerationReceipt> {
        match &self.phase { Phase::Recorded(receipt) => Some(receipt), Phase::Pending { .. } => None }
    }
    pub fn is_complete(&self) -> bool { self.receipt().is_some() }
    pub fn finish(&self) -> Option<Result<GenerationFinish, Error>> {
        self.receipt().map(|receipt| receipt.result().map(|report| report.finish()))
    }
    pub fn tokens(&self) -> &[u32] {
        match &self.phase {
            Phase::Pending { progress: Some(progress), .. } => progress.tokens(),
            Phase::Pending { progress: None, .. } => &[],
            Phase::Recorded(receipt) => match receipt.result() { Ok(report) => report.tokens(), Err(_) => &[] },
        }
    }
    pub(in super::super) fn pending(command: Rc<FileGenerationCommand>, revision: u64,
        progress: Option<GenerationProgress>) -> Self
    { Self { revision, phase: Phase::Pending { command, progress } } }
    pub(in super::super) fn recorded(revision: u64, receipt: FileGenerationReceipt) -> Self {
        Self { revision, phase: Phase::Recorded(receipt) }
    }
}

/// The enclosing canonical image can be later than its requested progress. A
/// readable image is not an assertion that an ambiguous directory sync succeeded.
#[derive(Clone, Debug)]
pub struct FileGenerationProgressSnapshot {
    pub generation: FileGenerationProgress,
    pub publication: FileDeliverySnapshot,
    pub numerical: FileDecoderInspection,
}

impl FileOversight {
    pub fn decoder_generation_progress(&self, id: u64) -> Result<FileGenerationProgress, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.decoder_generation_progress(id)?)
    }

    /// Advance at most ONE original token of the pending frozen command, and
    /// acknowledge its exact state/witness before exposing any resulting output.
    /// begin_decoder_generation must have committed the intent first. No prompt,
    /// stop ID, budget, token override or new seed is accepted at this interface.
    ///
    /// An older generation revision is a read-only retry of CURRENT progress;
    /// a future revision refuses. New work additionally requires the current
    /// journal revision, usable source, fresh clock and unpaused numerical owner.
    /// Recovery preserves cursor progress and requires the existing explicit
    /// resume. A partially advanced command must continue here; the whole-run
    /// API refuses rather than reinterpreting its original numerical predecessor.
    ///
    /// All declared incremental event slots are checked before the FIRST step.
    /// This is not a reservation against intervening supervisor operations, nor
    /// a promise that every future byte witness fits. The original byte limits
    /// and recovery-reserve admission continue to apply at each canonical cut.
    /// Holds/failures preserve actual state and the original automatic stop.
    /// Encoding or storage failure exposes no candidate output and poisons this
    /// owner. A crash can repeat unacknowledged physical work, but never imports
    /// a saved cursor or replenishes acknowledged budgets. Every step still
    /// replays prior history, and writes full state comparison material; this
    /// optional path trades IO/storage for resumable boundaries, not throughput.
    pub fn advance_decoder_generation(&mut self, journal_revision: u64, id: u64,
        generation_revision: u64) -> Result<FileGenerationProgress, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let current = self.decoder_generation_progress(id)?;
        if generation_revision > current.generation_revision() { return Err(Error::Stale.into()); }
        if current.is_complete() || generation_revision < current.generation_revision() { return Ok(current); }
        if journal_revision != self.revision() { return Err(Error::Stale.into()); }
        check_progress_key(id, generation_revision)?;
        self.check_source_admission(&Event::Decoder(DecoderEvent::AdvanceGeneration {
            id, revision: generation_revision, witness: Rc::from(&b""[..]),
        }))?;
        self.machine.check_decoder_generation_progress(id, generation_revision)?;
        let events_needed = if generation_revision == 0 { current.command().check()? } else { 1 };
        if self.events.len().checked_add(events_needed).ok_or(Error::Overflow)? > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let event = Event::Decoder(candidate.prepare_decoder_generation_progress(id, generation_revision)?);
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        self.persist_candidate(event, bytes, candidate, Transition::Unit)?;
        self.decoder_generation_progress(id)
    }

    /// Read the COMPLETE canonical image with exact independently supplied model
    /// configuration checked before replay. No writer lock, cleanup, fence, fresh
    /// clock or role is created. Missing means absent in THIS image, not no work.
    /// The older read_decoder_generation API retains its pending/recorded shape;
    /// this explicit view additionally reports acknowledged incremental progress.
    pub fn read_decoder_generation_progress(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileDecoderConfig, id: u64) -> Result<FileGenerationProgressSnapshot, JournalError>
    {
        profile.delivery.limits.check()?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &identity, &bytes)?;
        let mut configs = events.iter().filter_map(|event| match event {
            Event::Decoder(DecoderEvent::Enable(config)) => Some(config.as_ref()), _ => None,
        });
        if configs.next() != Some(expected) || configs.next().is_some() { return Err(Error::Binding.into()); }
        let machine = Machine::replay(profile, &events)?;
        Ok(FileGenerationProgressSnapshot {
            generation: machine.decoder_generation_progress(id)?,
            publication: machine.snapshot(events.len()),
            numerical: FileDecoderInspection { journal_revision: events.len() as u64,
                paused: machine.decoder_paused(), numerical: machine.broker.hosted_decoder()? },
        })
    }
}

pub(in super::super) fn check_progress_key(id: u64, revision: u64) -> Result<(), Error> {
    if id == 0 { return Err(Error::InvalidInput); }
    // At most one transition for each declared request step. A terminal poll
    // does not encode an event and can read the final revision at this bound.
    if revision >= MAX_GENERATION_TOKENS as u64 { return Err(Error::Limit); }
    Ok(())
}

#[cfg(test)]
mod tests;
