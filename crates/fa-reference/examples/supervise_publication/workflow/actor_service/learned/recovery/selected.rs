//! Closed selection of the three ORIGINAL fully guarded learned replay cursors.
//! Live recovery and terminal recovery share the same independent admission.
use super::cooperative::Progress;
use super::{
    Config, Custody, FileOversight, Loaded, RecoveryReserve, TopologyCustody, debug, requirements,
};
use fa_reference::action::ElapsedTick;
#[cfg(test)]
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStoppedRecovery;
use fa_reference::action::consequence::delivery::persistent::requests::actor::receipt::FileLearnedReceiptActorPort;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    observed::{
        guarded::{
            learned_recovery::FileGuardedLearnedRecovery,
            mediated::{
                FileMediatedRequirements,
                learned::FileMediatedLearnedRecovery,
                predictive::{
                    FileMediatedPredictor, FilePredictiveMediatedLearnedRecovery,
                    FilePredictiveMediatedRequirements,
                },
            },
            predictive::{
                FilePredictiveRequirements,
                learned::FilePredictiveLearnedRecovery,
                owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements},
            },
        },
        shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory},
        stream::FileStreamProposal,
    },
};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};

pub(super) enum Cursor {
    Plain(FileGuardedLearnedRecovery),
    Raw(FilePredictiveLearnedRecovery),
    Owned(FileOwnedPredictiveRecovery),
    Mediated(FileMediatedLearnedRecovery),
    PredictiveMediated(FilePredictiveMediatedLearnedRecovery),
}

impl Cursor {
    pub(super) fn begin(config: &Config, loaded: &Loaded) -> Result<Self, String> {
        if let Some(history) = super::super::topology::history(loaded)? {
            let anchor = loaded
                .recovery_anchor
                .as_ref()
                .ok_or("learned topology recovery requires an independent history anchor")?;
            let predictor = loaded
                .generation
                .required_pre_output_forecast()
                .map(|config| FileMediatedPredictor::Raw(config.clone()))
                .or_else(|| {
                    loaded
                        .generation
                        .required_owned_pre_output_forecast()
                        .map(|config| FileMediatedPredictor::Owned(config.clone()))
                });
            let run = match predictor {
                Some(prediction) => {
                    let expected = FilePredictiveMediatedRequirements {
                        oversight: requirements(config, loaded), topology: history.requirement.clone(),
                        prediction, evaluation: None,
                    };
                    FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                        &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
                        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                        .map(Self::PredictiveMediated)
                }
                None => {
                    let expected = FileMediatedRequirements { oversight: requirements(config, loaded),
                        topology: history.requirement.clone(), prediction: None, evaluation: None };
                    FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                        &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
                        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                        .map(Self::Mediated)
                }
            }.map_err(debug)?;
            history.check_extent(run.progress().total_events)?;
            return Ok(run);
        }
        if let Some(prediction) = loaded.generation.required_pre_output_forecast() {
            let expected = FilePredictiveRequirements {
                oversight: requirements(config, loaded),
                prediction: prediction.clone(),
                evaluation: None,
            };
            let run = match loaded.recovery_anchor.as_ref() {
                Some(anchor) => {
                    FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
                        &config.store,
                        config.profile.clone(),
                        &expected,
                        &loaded.generation,
                        anchor,
                    )
                }
                None => FileOversight::begin_open_predictive_guarded_with_learned_generation(
                    &config.store,
                    config.profile.clone(),
                    &expected,
                    &loaded.generation,
                ),
            }
            .map_err(debug)?;
            return run
                .require_recovery_reserve(RecoveryReserve::terminal())
                .map(Self::Raw)
                .map_err(debug);
        }
        if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
            let expected = FileOwnedPredictiveRequirements {
                oversight: requirements(config, loaded),
                prediction: prediction.clone(),
                evaluation: None,
            };
            let run = match loaded.recovery_anchor.as_ref() {
                Some(anchor) => FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
                    &config.store, config.profile.clone(), &expected, &loaded.generation, anchor),
                None => FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
                    &config.store, config.profile.clone(), &expected, &loaded.generation),
            }.map_err(debug)?;
            return run
                .require_recovery_reserve(RecoveryReserve::terminal())
                .map(Self::Owned)
                .map_err(debug);
        }
        let expected = requirements(config, loaded);
        let run = match loaded.recovery_anchor.as_ref() {
            Some(anchor) => FileOversight::begin_open_guarded_anchored_with_learned_generation(
                &config.store,
                config.profile.clone(),
                &expected,
                &loaded.generation,
                anchor,
            ),
            None => FileOversight::begin_open_guarded_with_learned_generation(
                &config.store,
                config.profile.clone(),
                &expected,
                &loaded.generation,
            ),
        }
        .map_err(debug)?;
        run.require_recovery_reserve(RecoveryReserve::terminal())
            .map(Self::Plain)
            .map_err(debug)
    }

    pub(super) fn progress(&self) -> Progress {
        match self {
            Self::Plain(run) => run.progress(),
            Self::Raw(run) => run.progress(),
            Self::Owned(run) => run.progress(),
            Self::Mediated(run) => run.progress(),
            Self::PredictiveMediated(run) => run.progress(),
        }
    }

    pub(super) fn advance(&mut self, done: usize) -> Result<Progress, JournalError> {
        match self {
            Self::Plain(run) => run.advance(done, 1),
            Self::Raw(run) => run.advance(done, 1),
            Self::Owned(run) => run.advance(done, 1),
            Self::Mediated(run) => run.advance(done, 1),
            Self::PredictiveMediated(run) => run.advance(done, 1),
        }
    }

    /// Select historical identity on the SAME fully checked replay, before the
    /// existing finalizer can clean staging or consume the original fence.
    pub(super) fn receipt(
        &self,
        request: u64,
        after: Option<u64>,
    ) -> Result<FileStreamProposal, JournalError> {
        match (self, after) {
            (Self::Plain(run), None) => run.learned_text_stream_message_request(request),
            (Self::Raw(run), None) => run.learned_text_stream_message_request(request),
            (Self::Owned(run), None) => run.learned_text_stream_message_request(request),
            (Self::Mediated(run), None) => run.learned_text_stream_message_request(request),
            (Self::PredictiveMediated(run), None) => {
                run.learned_text_stream_message_request(request)
            }
            (Self::Plain(run), Some(after)) => {
                run.learned_text_stream_finish_request(after, request)
            }
            (Self::Raw(run), Some(after)) => run.learned_text_stream_finish_request(after, request),
            (Self::Owned(run), Some(after)) => {
                run.learned_text_stream_finish_request(after, request)
            }
            (Self::Mediated(run), Some(after)) => {
                run.learned_text_stream_finish_request(after, request)
            }
            (Self::PredictiveMediated(run), Some(after)) => {
                run.learned_text_stream_finish_request(after, request)
            }
        }
    }

    pub(super) fn finish(self) -> Result<Custody, JournalError> {
        self.finish_with_topology().map(|(custody, _)| custody)
    }

    pub(super) fn finish_with_topology(self) -> Result<TopologyCustody, JournalError> {
        // Keep each owned native cursor and its original result in a separate
        // frame. A single debug-build match reserved every branch's large
        // recovery/result temporaries together and exhausted a normal stack.
        match self {
            Self::Plain(run) => Self::finish_plain(run),
            Self::Raw(run) => Self::finish_raw(run),
            Self::Owned(run) => Self::finish_owned(run),
            Self::Mediated(run) => Self::finish_mediated(run),
            Self::PredictiveMediated(run) => Self::finish_predictive_mediated(run),
        }
    }

    fn finish_plain(run: FileGuardedLearnedRecovery) -> Result<TopologyCustody, JournalError> {
        let (host, roles) = run.finish()?;
        Ok(((host, roles.human, None, roles.identity_observer), None))
    }

    fn finish_raw(run: FilePredictiveLearnedRecovery) -> Result<TopologyCustody, JournalError> {
        let (host, roles) = run.finish()?;
        Ok((
            (
                host,
                roles.oversight.human,
                Some(roles.consistency_observer),
                roles.oversight.identity_observer,
            ),
            None,
        ))
    }

    fn finish_owned(run: FileOwnedPredictiveRecovery) -> Result<TopologyCustody, JournalError> {
        let (host, roles) = run.finish()?;
        Ok((
            (
                host,
                roles.oversight.human,
                Some(roles.consistency_observer),
                roles.oversight.identity_observer,
            ),
            None,
        ))
    }

    fn finish_mediated(run: FileMediatedLearnedRecovery) -> Result<TopologyCustody, JournalError> {
        let (host, roles) = run.finish()?;
        Ok((
            (
                host,
                roles.oversight.human,
                roles.consistency_observer,
                roles.oversight.identity_observer,
            ),
            Some(roles.topology_observer),
        ))
    }

    fn finish_predictive_mediated(
        run: FilePredictiveMediatedLearnedRecovery,
    ) -> Result<TopologyCustody, JournalError> {
        let (host, roles) = run.finish()?;
        Ok((
            (
                host,
                roles.oversight.human,
                roles.consistency_observer,
                roles.oversight.identity_observer,
            ),
            Some(roles.topology_observer),
        ))
    }

    pub(super) fn finish_receipt(
        self,
        request: u64,
        after: Option<u64>,
    ) -> Result<Knowledge<ActorOutcome>, JournalError> {
        match self {
            Self::Plain(run) => run.finish_receipt(request, after),
            Self::Raw(run) => run.finish_receipt(request, after),
            Self::Owned(run) => run.finish_receipt(request, after),
            Self::Mediated(run) => run.finish_receipt(request, after),
            Self::PredictiveMediated(run) => run.finish_receipt(request, after),
        }
    }

    pub(super) fn finish_reconciled_receipt<F>(
        self,
        request: u64,
        after: Option<u64>,
        clock: F,
    ) -> Result<Knowledge<ActorOutcome>, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        match self {
            Self::Plain(run) => run.finish_reconciled_receipt(request, after, clock),
            Self::Raw(run) => run.finish_reconciled_receipt(request, after, clock),
            Self::Owned(run) => run.finish_reconciled_receipt(request, after, clock),
            Self::Mediated(run) => run.finish_reconciled_receipt(request, after, clock),
            Self::PredictiveMediated(run) => run.finish_reconciled_receipt(request, after, clock),
        }
    }

    pub(super) fn finish_receipt_port(
        self,
        request: u64,
        after: Option<u64>,
    ) -> Result<FileLearnedReceiptActorPort, JournalError> {
        match self {
            Self::Plain(run) => run.finish_receipt_port(request, after),
            Self::Raw(run) => run.finish_receipt_port(request, after),
            Self::Owned(run) => run.finish_receipt_port(request, after),
            Self::Mediated(run) => run.finish_receipt_port(request, after),
            Self::PredictiveMediated(run) => run.finish_receipt_port(request, after),
        }
    }

    pub(super) fn finish_reconciled_receipt_port<F>(
        self,
        request: u64,
        after: Option<u64>,
        clock: F,
    ) -> Result<FileLearnedReceiptActorPort, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        match self {
            Self::Plain(run) => run.finish_reconciled_receipt_port(request, after, clock),
            Self::Raw(run) => run.finish_reconciled_receipt_port(request, after, clock),
            Self::Owned(run) => run.finish_reconciled_receipt_port(request, after, clock),
            Self::Mediated(run) => run.finish_reconciled_receipt_port(request, after, clock),
            Self::PredictiveMediated(run) => {
                run.finish_reconciled_receipt_port(request, after, clock)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn finish_stopped<F>(
        self,
        operation: u64,
        clock: F,
    ) -> Result<FileStoppedRecovery, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        match self {
            Self::Plain(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped(request, clock)
            }
            Self::Raw(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped(request, clock)
            }
            Self::Owned(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped(request, clock)
            }
            Self::Mediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped(request, clock)
            }
            Self::PredictiveMediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped(request, clock)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn finish_stopped_with_anchor<F>(
        self,
        operation: u64,
        clock: F,
    ) -> Result<FileStoppedHistory, JournalError>
    where
        F: FnOnce() -> ElapsedTick,
    {
        match self {
            Self::Plain(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_with_anchor(request, clock)
            }
            Self::Raw(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_with_anchor(request, clock)
            }
            Self::Owned(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_with_anchor(request, clock)
            }
            Self::Mediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_with_anchor(request, clock)
            }
            Self::PredictiveMediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_with_anchor(request, clock)
            }
        }
    }

    pub(super) fn finish_admission_stop(
        self,
        operation: u64,
    ) -> Result<FileAdmissionStoppedHistory, JournalError> {
        match self {
            Self::Plain(run) => {
                let request = run.stop_request(operation)?;
                run.finish_admission_stop(request)
            }
            Self::Raw(run) => {
                let request = run.stop_request(operation)?;
                run.finish_admission_stop(request)
            }
            Self::Owned(run) => {
                let request = run.stop_request(operation)?;
                run.finish_admission_stop(request)
            }
            Self::Mediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_admission_stop(request)
            }
            Self::PredictiveMediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_admission_stop(request)
            }
        }
    }

    pub(super) fn finish_stopped_drain<F>(
        self,
        operation: u64,
        clock: F,
    ) -> Result<FileStoppedHistory, JournalError>
    where
        F: FnOnce() -> Result<ElapsedTick, JournalError>,
    {
        match self {
            Self::Plain(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_drain(request, clock)
            }
            Self::Raw(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_drain(request, clock)
            }
            Self::Owned(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_drain(request, clock)
            }
            Self::Mediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_drain(request, clock)
            }
            Self::PredictiveMediated(run) => {
                let request = run.stop_request(operation)?;
                run.finish_stopped_drain(request, clock)
            }
        }
    }

    pub(super) fn try_finish_stopped_with_anchor<F>(
        self,
        operation: u64,
        clock: F,
    ) -> Result<FileStoppedHistory, JournalError>
    where
        F: FnOnce() -> Result<ElapsedTick, JournalError>,
    {
        match self {
            Self::Plain(run) => {
                let request = run.stop_request(operation)?;
                run.try_finish_stopped_with_anchor(request, clock)
            }
            Self::Raw(run) => {
                let request = run.stop_request(operation)?;
                run.try_finish_stopped_with_anchor(request, clock)
            }
            Self::Owned(run) => {
                let request = run.stop_request(operation)?;
                run.try_finish_stopped_with_anchor(request, clock)
            }
            Self::Mediated(run) => {
                let request = run.stop_request(operation)?;
                run.try_finish_stopped_with_anchor(request, clock)
            }
            Self::PredictiveMediated(run) => {
                let request = run.stop_request(operation)?;
                run.try_finish_stopped_with_anchor(request, clock)
            }
        }
    }
}
