//! Terminal-only selection never relaxes the shared live/receipt replay cursor.
//! Retained-prefix repair has its own sealed native consumer and bound Stop ID.
use super::{Config, Loaded, FileOversight, RecoveryReserve, requirements, debug};
use super::{cooperative::Progress, selected};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{JournalError, observed::{
    guarded::mediated::{FileMediatedRequirements,
        predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements},
        terminal::FileTerminalTopologyRecovery},
    shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory},
}};

enum Selected {
    Exact(selected::Cursor),
    Checkpoint(FileTerminalTopologyRecovery),
}

pub(super) struct Cursor { selected: Selected, operation: u64 }

impl Cursor {
    pub(super) fn begin(config: &Config, loaded: &Loaded, operation: u64,
        recover_checkpoint: bool) -> Result<Self, String>
    {
        let selected = if recover_checkpoint {
            let history = super::super::topology::history(loaded)?
                .ok_or("terminal checkpoint recovery requires an explicit topology profile")?;
            let saved = history.checkpoint()
                .ok_or("terminal checkpoint recovery requires an exact v2 source checkpoint")?;
            let anchor = loaded.recovery_anchor.as_ref()
                .ok_or("terminal checkpoint recovery requires an independent history anchor")?;
            if saved.history_anchor() != anchor {
                return Err("source checkpoint and history anchor describe different cuts".into());
            }
            let predictor = loaded.generation.required_pre_output_forecast()
                .map(|config| FileMediatedPredictor::Raw(config.clone()))
                .or_else(|| loaded.generation.required_owned_pre_output_forecast()
                    .map(|config| FileMediatedPredictor::Owned(config.clone())));
            let run = match predictor {
                Some(prediction) => {
                    let expected = FilePredictiveMediatedRequirements {
                        oversight: requirements(config, loaded), topology: history.requirement.clone(),
                        prediction, evaluation: None,
                    };
                    FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                        &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
                        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                        .and_then(|run| run.require_terminal_checkpoint(saved, operation))
                }
                None => {
                    let expected = FileMediatedRequirements { oversight: requirements(config, loaded),
                        topology: history.requirement.clone(), prediction: None, evaluation: None };
                    FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                        &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
                        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                        .and_then(|run| run.require_terminal_checkpoint(saved, operation))
                }
            }.map_err(debug)?;
            Selected::Checkpoint(run)
        } else {
            Selected::Exact(selected::Cursor::begin(config, loaded)?)
        };
        Ok(Self { selected, operation })
    }

    pub(super) fn progress(&self) -> Progress {
        match &self.selected {
            Selected::Exact(run) => run.progress(), Selected::Checkpoint(run) => run.progress(),
        }
    }

    pub(super) fn advance(&mut self, done: usize) -> Result<Progress, JournalError> {
        match &mut self.selected {
            Selected::Exact(run) => run.advance(done),
            Selected::Checkpoint(run) => run.advance(done, 1),
        }
    }

    pub(super) fn finish_admission_stop(self) -> Result<FileAdmissionStoppedHistory, JournalError> {
        match self.selected {
            Selected::Exact(run) => run.finish_admission_stop(self.operation),
            Selected::Checkpoint(run) => run.finish_admission_stop(),
        }
    }

    pub(super) fn finish_stopped_drain<F>(self, clock: F) -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        match self.selected {
            Selected::Exact(run) => run.finish_stopped_drain(self.operation, clock),
            Selected::Checkpoint(run) => run.finish_stopped_drain(clock),
        }
    }

    pub(super) fn try_finish_stopped_with_anchor<F>(self, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        match self.selected {
            Selected::Exact(run) => run.try_finish_stopped_with_anchor(self.operation, clock),
            Selected::Checkpoint(run) => run.try_finish_stopped_with_anchor(clock),
        }
    }
}
