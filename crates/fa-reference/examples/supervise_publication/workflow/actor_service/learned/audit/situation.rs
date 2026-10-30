//! The existing offline recipe loader, consumed by the publication situation CLI.
//! This extends Config without exposing its loader or live workflow internals.
use super::super::{Config, FileOversight, Path, debug, recipe};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::FileHistoryAnchor,
    replay::situation::FileSituationReplay,
};

impl Config {
    /// Admit original assets without a live native roster, then bind them to one
    /// historical image. Caller checkpoints retain the invocation's ORIGINAL
    /// elapsed allowance; loading and one event remain cooperative boundaries.
    pub(crate) fn begin_learned_situation(&self, path: &Path,
        anchor: Option<&FileHistoryAnchor>, checkpoint: &mut impl FnMut() -> Result<(), String>,
    ) -> Result<FileSituationReplay, String> {
        checkpoint()?;
        // Same checkpoint-source/tokenizer/fit/monitor/predictor admission used
        // by the offline review auditor. No helper model, evidence, credential
        // secret, control socket, live reviewer or authority owner is opened.
        let loaded = recipe::load(path, self, false)?;
        checkpoint()?;
        let replay = match loaded.generation.required_owned_pre_output_forecast() {
            Some(predictor) => FileOversight::begin_read_situation_with_owned_learned_consistency(
                &self.store, &self.profile, &loaded.generation, predictor, anchor),
            None => FileOversight::begin_read_situation_with_learned_generation(
                &self.store, &self.profile, &loaded.generation, anchor),
        }.map_err(debug)?;
        checkpoint()?;
        Ok(replay)
    }
}
