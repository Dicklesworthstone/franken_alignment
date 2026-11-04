//! One guarded numerical replay for explicit offline campaign governance.
use super::{Config, FileOversight, Loaded, RecoveryReserve, debug, requirements};
use super::cooperative::{Progress, finish_replay};
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedPolicyPromotionRequest,
    guarded::learned_recovery::FileGuardedLearnedRecovery,
};

pub(in super::super) fn ready(config: &Config, loaded: &Loaded,
    request: FileLearnedPolicyPromotionRequest,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>)
    -> Result<FileGuardedLearnedRecovery, String>
{
    super::super::policy_promotion::check_selection(loaded)?;
    checkpoint(None)?;
    let anchor = loaded.recovery_anchor.as_ref().ok_or("missing independent policy promotion history anchor")?;
    let run = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        &config.store, config.profile.clone(), &requirements(config, loaded), &loaded.generation, anchor)
        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
        .and_then(|run| run.require_policy_promotion(request)).map_err(debug)?;
    finish_replay(run, checkpoint, |run| run.progress(), |run, done| run.advance(done, 1), Ok)
}
