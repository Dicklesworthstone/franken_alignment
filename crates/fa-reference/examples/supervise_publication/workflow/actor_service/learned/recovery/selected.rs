//! Closed selection of the three ORIGINAL fully guarded learned replay cursors.
//! Live recovery and terminal recovery share the same independent admission.
use super::{Config, Loaded, Custody, FileOversight, RecoveryReserve, requirements, debug};
use super::cooperative::Progress;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{JournalError, observed::{
    guarded::{learned_recovery::FileGuardedLearnedRecovery,
        predictive::{FilePredictiveRequirements, learned::FilePredictiveLearnedRecovery,
            owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements}}},
    shutdown::recovery::{FileStoppedHistory, FileStoppedRecovery},
    stream::FileStreamProposal,
}};

pub(super) enum Cursor {
    Plain(FileGuardedLearnedRecovery),
    Raw(FilePredictiveLearnedRecovery),
    Owned(FileOwnedPredictiveRecovery),
}

impl Cursor {
    pub(super) fn begin(config: &Config, loaded: &Loaded) -> Result<Self, String> {
        if let Some(prediction) = loaded.generation.required_pre_output_forecast() {
            let expected = FilePredictiveRequirements { oversight: requirements(config, loaded),
                prediction: prediction.clone(), evaluation: None };
            let run = match loaded.recovery_anchor.as_ref() {
                Some(anchor) => FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
                    &config.store, config.profile.clone(), &expected, &loaded.generation, anchor),
                None => FileOversight::begin_open_predictive_guarded_with_learned_generation(
                    &config.store, config.profile.clone(), &expected, &loaded.generation),
            }.map_err(debug)?;
            return run.require_recovery_reserve(RecoveryReserve::terminal())
                .map(Self::Raw).map_err(debug);
        }
        if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
            let expected = FileOwnedPredictiveRequirements { oversight: requirements(config, loaded),
                prediction: prediction.clone(), evaluation: None };
            let run = match loaded.recovery_anchor.as_ref() {
                Some(anchor) => FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
                    &config.store, config.profile.clone(), &expected, &loaded.generation, anchor),
                None => FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
                    &config.store, config.profile.clone(), &expected, &loaded.generation),
            }.map_err(debug)?;
            return run.require_recovery_reserve(RecoveryReserve::terminal())
                .map(Self::Owned).map_err(debug);
        }
        let expected = requirements(config, loaded);
        let run = match loaded.recovery_anchor.as_ref() {
            Some(anchor) => FileOversight::begin_open_guarded_anchored_with_learned_generation(
                &config.store, config.profile.clone(), &expected, &loaded.generation, anchor),
            None => FileOversight::begin_open_guarded_with_learned_generation(
                &config.store, config.profile.clone(), &expected, &loaded.generation),
        }.map_err(debug)?;
        run.require_recovery_reserve(RecoveryReserve::terminal()).map(Self::Plain).map_err(debug)
    }

    pub(super) fn progress(&self) -> Progress {
        match self { Self::Plain(run) => run.progress(), Self::Raw(run) => run.progress(),
            Self::Owned(run) => run.progress() }
    }

    pub(super) fn advance(&mut self, done: usize) -> Result<Progress, JournalError> {
        match self { Self::Plain(run) => run.advance(done, 1), Self::Raw(run) => run.advance(done, 1),
            Self::Owned(run) => run.advance(done, 1) }
    }

    /// Select historical identity on the SAME fully checked replay, before the
    /// existing finalizer can clean staging or consume the original fence.
    pub(super) fn receipt(&self, request: u64, after: Option<u64>)
        -> Result<FileStreamProposal, JournalError>
    {
        match (self, after) {
            (Self::Plain(run), None) => run.learned_text_stream_message_request(request),
            (Self::Raw(run), None) => run.learned_text_stream_message_request(request),
            (Self::Owned(run), None) => run.learned_text_stream_message_request(request),
            (Self::Plain(run), Some(after)) => run.learned_text_stream_finish_request(after, request),
            (Self::Raw(run), Some(after)) => run.learned_text_stream_finish_request(after, request),
            (Self::Owned(run), Some(after)) => run.learned_text_stream_finish_request(after, request),
        }
    }

    pub(super) fn finish(self) -> Result<Custody, JournalError> {
        let (host, roles) = match self {
            Self::Plain(run) => {
                let (host, roles) = run.finish()?;
                return Ok((host, roles.human, None));
            }
            Self::Raw(run) => run.finish()?,
            Self::Owned(run) => run.finish()?,
        };
        Ok((host, roles.oversight.human, Some(roles.consistency_observer)))
    }

    pub(super) fn finish_stopped<F>(self, operation: u64, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self {
            Self::Plain(run) => { let request = run.stop_request(operation)?; run.finish_stopped(request, clock) }
            Self::Raw(run) => { let request = run.stop_request(operation)?; run.finish_stopped(request, clock) }
            Self::Owned(run) => { let request = run.stop_request(operation)?; run.finish_stopped(request, clock) }
        }
    }

    pub(super) fn finish_stopped_with_anchor<F>(self, operation: u64, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self {
            Self::Plain(run) => { let request = run.stop_request(operation)?; run.finish_stopped_with_anchor(request, clock) }
            Self::Raw(run) => { let request = run.stop_request(operation)?; run.finish_stopped_with_anchor(request, clock) }
            Self::Owned(run) => { let request = run.stop_request(operation)?; run.finish_stopped_with_anchor(request, clock) }
        }
    }
}
