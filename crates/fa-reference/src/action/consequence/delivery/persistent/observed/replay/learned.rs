//! Independently bind the original numerical/predictor recipes before audit.
//! This is read-only evidence extraction, not guarded recovery or role issuance.
use super::{FileOversight, FileOversightProfile, FileReviewReplay,
    FileReviewReplayCursor, JournalError, Path, cursor};
use super::super::decoder::learned::{FileLearnedConfig, bind_history};
use super::super::consistency::learned::{FileLearnedConsistencyConfig, owned::bind_owned_history};

impl FileOversight {
    /// Audit a completed round from a learned journal without reopening its live
    /// owner. The complete independently held generation recipe is compared
    /// BEFORE the first original event executes. This includes its recipe-owned
    /// raw or owned predictor when configured. Parsed model/source claims cannot
    /// supply runtime bindings, numerical outputs or a permitting substitute.
    ///
    /// All original inference, compression and monitoring used in the historical
    /// journal are recomputed and their retained witnesses checked. This consumes
    /// physical audit compute, not new live sampling/forecast budget. Every suffix
    /// event must verify before the original FileReviewReplay can be exported.
    pub fn begin_read_review_replay_with_learned_generation(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, generation: &FileLearnedConfig, round: u64)
        -> Result<FileReviewReplayCursor, JournalError>
    {
        let mut events = cursor::read_events(directory.as_ref(), profile)?;
        bind_history(&mut events, generation)?;
        FileReviewReplayCursor::new(profile, events, round, None)
    }

    /// Synchronous consumer of the same event-budgeted, nonmutating audit.
    pub fn read_review_replay_with_learned_generation(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, generation: &FileLearnedConfig, round: u64)
        -> Result<FileReviewReplay, JournalError>
    {
        Self::begin_read_review_replay_with_learned_generation(directory, profile,
            generation, round)?.complete()
    }

    /// Bind BOTH the generation and independently selected owned-K/V predictor
    /// using the original strict binder. Standalone and recipe-owned forecasting
    /// retain their distinct admission rules. A changed predictor or a supplied
    /// capture importer is not repaired by falling back to raw/generic replay.
    ///
    /// No capture map, alternate model, caller result, observer role or recovery
    /// fence is accepted/created. The original machine reconstructs the source
    /// from its actual token/K/V history and verifies the full remaining journal.
    pub fn begin_read_review_replay_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, generation: &FileLearnedConfig,
        predictor: &FileLearnedConsistencyConfig, round: u64)
        -> Result<FileReviewReplayCursor, JournalError>
    {
        let mut events = cursor::read_events(directory.as_ref(), profile)?;
        bind_owned_history(&mut events, generation, predictor)?;
        FileReviewReplayCursor::new(profile, events, round, None)
    }

    pub fn read_review_replay_with_owned_learned_consistency(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, generation: &FileLearnedConfig,
        predictor: &FileLearnedConsistencyConfig, round: u64)
        -> Result<FileReviewReplay, JournalError>
    {
        Self::begin_read_review_replay_with_owned_learned_consistency(directory, profile,
            generation, predictor, round)?.complete()
    }
}
