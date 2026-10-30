//! The existing offline recipe loader, consumed by the publication situation CLI.
//! This extends Config without exposing its loader or live workflow internals.
use super::super::{Config, FileOversight, Loaded, Path, debug, fields, recipe};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::FileHistoryAnchor,
    replay::situation::FileSituationReplay,
};
use std::path::PathBuf;

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
        let replay = bind(self, &loaded, anchor)?;
        checkpoint()?;
        Ok(replay)
    }

    /// The same fleet loader used by shutdown intake, but only its numerical
    /// data admission: no recovery requirements, coordinator or roles are used.
    pub(crate) fn begin_learned_fleet_situation(&self, path: &Path,
        anchor: Option<&FileHistoryAnchor>, remaining: &mut usize, forbidden: &[PathBuf],
        checkpoint: &mut impl FnMut() -> Result<(), String>,
    ) -> Result<FileSituationReplay, String> {
        checkpoint()?;
        // A failed member's auxiliary reads must not replenish later members.
        // Reserve a bounded original asset allowance first; successful intake
        // refunds only unused bytes, while failure conservatively spends it.
        let reservation = (*remaining).min(fields::MAX_ASSET_BYTES);
        *remaining -= reservation;
        let mut available = reservation;
        let loaded = recipe::load_for_fleet(path, self, &mut available, forbidden);
        if loaded.is_ok() { *remaining += available; }
        checkpoint()?;
        let replay = bind(self, &loaded?, anchor)?;
        checkpoint()?;
        Ok(replay)
    }
}

fn bind(config: &Config, loaded: &Loaded, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileSituationReplay, String>
{
    match loaded.generation.required_owned_pre_output_forecast() {
        Some(predictor) => FileOversight::begin_read_situation_with_owned_learned_consistency(
            &config.store, &config.profile, &loaded.generation, predictor, anchor),
        None => FileOversight::begin_read_situation_with_learned_generation(
            &config.store, &config.profile, &loaded.generation, anchor),
    }.map_err(debug)
}
