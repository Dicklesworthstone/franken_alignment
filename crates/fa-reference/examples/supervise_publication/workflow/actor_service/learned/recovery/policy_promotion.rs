//! One guarded numerical replay for explicit offline campaign governance.
use super::{Config, FileOversight, Loaded, RecoveryReserve, debug, requirements};
use super::cooperative::{Progress, finish_replay};
use fa_reference::action::consequence::delivery::persistent::{JournalError, observed::{
    decoder::learned::{FileLearnedPolicyPromotionHistory, FileLearnedPolicyPromotionRequest,
        FileLearnedPolicyPromotionReview},
    driver::topology::checkpoint::TopologyCheckpoint,
    guarded::{learned_recovery::FileGuardedLearnedRecovery,
        mediated::{FileMediatedRequirements, learned::FileMediatedPolicyPromotionRecovery}},
}};

pub(in super::super) enum Run {
    Plain(FileGuardedLearnedRecovery),
    Mediated(FileMediatedPolicyPromotionRecovery),
}

impl Run {
    fn progress(&self) -> Progress {
        match self { Self::Plain(run) => run.progress(), Self::Mediated(run) => run.progress() }
    }
    fn advance(&mut self, done: usize) -> Result<Progress, JournalError> {
        match self {
            Self::Plain(run) => run.advance(done, 1),
            Self::Mediated(run) => run.advance(done, 1),
        }
    }
    pub(in super::super) fn policy_promotion_review(&self)
        -> Result<&FileLearnedPolicyPromotionReview, JournalError>
    {
        match self {
            Self::Plain(run) => run.policy_promotion_review(),
            Self::Mediated(run) => run.policy_promotion_review(),
        }
    }
    pub(in super::super) fn finish_promoted_policy(self,
        reviewed: &FileLearnedPolicyPromotionReview, accept_newly_reviewable: bool)
        -> Result<(FileLearnedPolicyPromotionHistory, Option<TopologyCheckpoint>), JournalError>
    {
        match self {
            Self::Plain(run) => run.finish_promoted_policy(reviewed, accept_newly_reviewable)
                .map(|history| (history, None)),
            Self::Mediated(run) => run.finish_promoted_policy(reviewed, accept_newly_reviewable)
                .map(|history| { let (promotion, saved) = history.into_parts(); (promotion, Some(saved)) }),
        }
    }
}

pub(in super::super) fn ready(config: &Config, loaded: &Loaded,
    request: FileLearnedPolicyPromotionRequest,
    checkpoint: &mut impl FnMut(Option<Progress>) -> Result<(), String>)
    -> Result<Run, String>
{
    super::super::policy_promotion::check_selection(loaded)?;
    checkpoint(None)?;
    let anchor = loaded.recovery_anchor.as_ref().ok_or("missing independent policy promotion history anchor")?;
    let run = match super::super::topology::history(loaded)? {
        Some(history) => {
            let saved = history.checkpoint().ok_or("policy promotion requires an exact v2 topology-source checkpoint")?;
            if saved.history_anchor() != anchor {
                return Err("policy promotion source checkpoint and history anchor describe different cuts".into());
            }
            let expected = FileMediatedRequirements { oversight: requirements(config, loaded),
                topology: history.requirement.clone(), prediction: None, evaluation: None };
            FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
                .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                .and_then(|run| run.require_policy_promotion_checkpoint(request, saved))
                .map(Run::Mediated)
        }
        None => FileOversight::begin_open_guarded_anchored_with_learned_generation(
            &config.store, config.profile.clone(), &requirements(config, loaded), &loaded.generation, anchor)
            .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
            .and_then(|run| run.require_policy_promotion(request)).map(Run::Plain),
    }.map_err(debug)?;
    finish_replay(run, checkpoint, Run::progress, Run::advance, Ok)
}
