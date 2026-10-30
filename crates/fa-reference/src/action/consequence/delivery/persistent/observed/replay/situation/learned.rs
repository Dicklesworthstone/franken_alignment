//! Bind independently supplied numerical recipes before original situation replay.
//! No runtime recipe is inferred from saved bytes, and no recovery role is issued.
use super::{FileHistoryAnchor, FileOversight, FileOversightProfile, FileSituationReplay,
    JournalError, Path, read_events};
use super::super::super::decoder::learned::{FileLearnedConfig, bind_history};
use super::super::super::consistency::learned::{
    FileLearnedConsistencyConfig, owned::bind_owned_history,
};

impl FileOversight {
    /// Read one canonical image with an independently selected exact generator.
    /// The original binder also admits a recipe-owned raw pre-output predictor;
    /// an owned-K/V predictor requires the separate typed entry point below.
    ///
    /// An optional independent anchor checks this SAME image before recipe
    /// binding. Numerical events are recomputed only when the returned cursor
    /// advances. Saved witnesses remain comparisons, never imported state. The
    /// complete history must pass before a situation can escape. No recovery
    /// fence, source refresh, helper execution, live role or effect is produced.
    pub fn begin_read_situation_with_learned_generation(
        directory: impl AsRef<Path>, profile: &FileOversightProfile,
        generation: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>,
    ) -> Result<FileSituationReplay, JournalError> {
        let mut events = read_events(directory.as_ref(), profile, anchor)?;
        bind_history(&mut events, generation)?;
        FileSituationReplay::new(profile, events, anchor.map(FileHistoryAnchor::revision))
    }

    /// Bind BOTH the original generator and owned-K/V predictor before replay.
    /// Neither equal-looking model dimensions nor a raw-predictor recipe can
    /// substitute for the original exact runtime identities. There is no retry
    /// through a weaker reader if either binding or later replay fails.
    ///
    /// The same finite event cursor, historical basis and immutable pagination
    /// apply. Replay can recompute original numerical work in private memory;
    /// it cannot advance the live sampler, spend rights or emit a new forecast.
    pub fn begin_read_situation_with_owned_learned_consistency(
        directory: impl AsRef<Path>, profile: &FileOversightProfile,
        generation: &FileLearnedConfig, predictor: &FileLearnedConsistencyConfig,
        anchor: Option<&FileHistoryAnchor>,
    ) -> Result<FileSituationReplay, JournalError> {
        let mut events = read_events(directory.as_ref(), profile, anchor)?;
        bind_owned_history(&mut events, generation, predictor)?;
        FileSituationReplay::new(profile, events, anchor.map(FileHistoryAnchor::revision))
    }
}
