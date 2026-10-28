//! Closed selection of the three ORIGINAL fully guarded learned replay cursors.
//! Live recovery and terminal recovery share the same independent admission.
use super::cooperative::Progress;
use fa_reference::action::consequence::experiment::InterventionScope;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalInvestigation;
use super::{
    Config, Custody, FileOversight, Loaded, RecoveryReserve, TopologyCustody, debug, requirements,
};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt;
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::TopologyCheckpoint;
use fa_reference::action::consequence::delivery::credential_broker::CredentialChangeReceipt;
use fa_reference::action::consequence::delivery::persistent::observed::credential::FileCredentialStatus;
use super::super::credential_lifecycle::Request as CredentialRequest;
#[cfg(test)]
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStoppedRecovery;
use fa_reference::action::consequence::delivery::persistent::requests::actor::receipt::FileLearnedReceiptActorPort;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    abandonment::FileAbandonRequest,
    observed::{
        guarded::{
            recovery::FileAbandonedHistory,
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

#[derive(Clone, Copy)]
enum SourceCut {
    Exact,
    Abandonment,
}

/// This sealed operator cursor cannot finish into a live owner. Only its native
/// abandonment finalizer may defer the ordinary topology extent check, and that
/// finalizer checks the saved cut or one exact matching disposition successor.
pub(super) struct AbandonmentCursor<'a> {
    cursor: Cursor,
    checkpoint: Option<&'a TopologyCheckpoint>,
}

impl<'a> AbandonmentCursor<'a> {
    pub(super) fn begin(config: &Config, loaded: &'a Loaded) -> Result<Self, String> {
        let anchor = loaded.recovery_anchor.as_ref()
            .ok_or("learned abandonment requires an independent input history anchor")?;
        let checkpoint = super::super::topology::history(loaded)?
            .map(|history| {
                let saved = history.checkpoint().ok_or(
                    "learned abandonment requires an exact v2 topology checkpoint; manual source minima cannot establish custody")?;
                if saved.history_anchor() != anchor {
                    return Err("abandonment source checkpoint and input history anchor describe different cuts".into());
                }
                Ok::<_, String>(saved)
            }).transpose()?;
        let cursor = Cursor::begin_with_source_cut(config, loaded, SourceCut::Abandonment)?;
        Ok(Self { cursor, checkpoint })
    }

    pub(super) fn progress(&self) -> Progress { self.cursor.progress() }
    pub(super) fn advance(&mut self, done: usize) -> Result<Progress, JournalError> {
        self.cursor.advance(done)
    }
    pub(super) fn finish(self, request: FileAbandonRequest)
        -> Result<FileAbandonedHistory, JournalError>
    {
        macro_rules! finish {
            ($run:expr) => {
                match self.checkpoint {
                    Some(saved) => $run.finish_abandoned_with_checkpoint(request, saved),
                    None => $run.finish_abandoned_with_anchor(request),
                }
            };
        }
        match self.cursor {
            Cursor::Plain(run) => finish!(run),
            Cursor::Raw(run) => finish!(run),
            Cursor::Owned(run) => finish!(run),
            Cursor::Mediated(run) => finish!(run),
            Cursor::PredictiveMediated(run) => finish!(run),
        }
    }
}

/// Original mediated replay, sealed to receipt-cut recovery. This deliberately
/// does not pass through Cursor::begin or weaken its ordinary exact-extent rule.
/// The native wrapper alone admits [] or one literal Fence/Time/Sweep suffix.
pub(super) fn begin_receipt_checkpoint_repair(
    config: &Config, loaded: &Loaded, after: Option<u64>,
) -> Result<fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::receipt::FileReceiptTopologyRecovery, String> {
    let history = super::super::topology::history(loaded)?
        .ok_or("receipt checkpoint recovery requires an explicit v2 topology profile")?;
    let saved = history.checkpoint()
        .ok_or("receipt checkpoint recovery cannot use manual source minima")?;
    let anchor = loaded.recovery_anchor.as_ref()
        .ok_or("receipt checkpoint recovery requires an independent input history anchor")?;
    if saved.history_anchor() != anchor {
        return Err("receipt checkpoint and input history anchor describe different cuts".into());
    }
    let predictor = loaded.generation.required_pre_output_forecast()
        .map(|config| FileMediatedPredictor::Raw(config.clone()))
        .or_else(|| loaded.generation.required_owned_pre_output_forecast()
            .map(|config| FileMediatedPredictor::Owned(config.clone())));
    match predictor {
        Some(prediction) => {
            let expected = FilePredictiveMediatedRequirements {
                oversight: requirements(config, loaded), topology: history.requirement.clone(),
                prediction, evaluation: None,
            };
            FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
                .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                .and_then(|run| run.require_receipt_checkpoint(saved, loaded.request, after))
                .map_err(debug)
        }
        None => {
            let expected = FileMediatedRequirements { oversight: requirements(config, loaded),
                topology: history.requirement.clone(), prediction: None, evaluation: None };
            FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                &config.store, config.profile.clone(), &expected, &loaded.generation, anchor)
                .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                .and_then(|run| run.require_receipt_checkpoint(saved, loaded.request, after))
                .map_err(debug)
        }
    }
}

impl Cursor {
    pub(super) fn begin(config: &Config, loaded: &Loaded) -> Result<Self, String> {
        Self::begin_with_source_cut(config, loaded, SourceCut::Exact)
    }

    fn begin_with_source_cut(config: &Config, loaded: &Loaded, source_cut: SourceCut)
        -> Result<Self, String>
    {
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
            if matches!(source_cut, SourceCut::Exact) {
                history.check_extent(run.progress().total_events)?;
            }
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


    /// Register capture before event zero on each ORIGINAL guarded cursor.
    pub(super) fn require_request_proposal_investigation(self, request: u64,
        scope: InterventionScope) -> Result<Self, JournalError>
    {
        match self {
            Self::Plain(run) => run.require_request_proposal_investigation(request, scope).map(Self::Plain),
            Self::Raw(run) => run.require_request_proposal_investigation(request, scope).map(Self::Raw),
            Self::Owned(run) => run.require_request_proposal_investigation(request, scope).map(Self::Owned),
            Self::Mediated(run) => run.require_request_proposal_investigation(request, scope).map(Self::Mediated),
            Self::PredictiveMediated(run) => run.require_request_proposal_investigation(request, scope).map(Self::PredictiveMediated),
        }
    }

    /// Consume only an immutable investigation after complete expected guards.
    /// No recovery Fence, clock, cleanup, observer or live owner is issued.
    pub(super) fn finish_proposal_investigation(self) -> Result<FileProposalInvestigation, JournalError> {
        match self {
            Self::Plain(run) => run.finish_proposal_investigation(),
            Self::Raw(run) => run.finish_proposal_investigation(),
            Self::Owned(run) => run.finish_proposal_investigation(),
            Self::Mediated(run) => run.finish_proposal_investigation(),
            Self::PredictiveMediated(run) => run.finish_proposal_investigation(),
        }
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

    /// Validate the exact operation and independently selected current epoch on
    /// this same original Ready cursor, before staging cleanup or any Fence.
    /// Topology maintenance retains the paired source-checkpoint successor;
    /// these queries never create a source observer or refresh its evidence.
    pub(super) fn credential_preflight(&self, request: CredentialRequest)
        -> Result<(FileCredentialStatus, Option<CredentialChangeReceipt>), JournalError>
    {
        let (status, receipt) = match self {
            Self::Plain(run) => (run.credential_status()?, match request {
                CredentialRequest::Rotate(request) => run.check_credential_rotation(request)?,
                CredentialRequest::Revoke(request) => run.check_credential_revocation(request)?,
            }),
            Self::Raw(run) => (run.credential_status()?, match request {
                CredentialRequest::Rotate(request) => run.check_credential_rotation(request)?,
                CredentialRequest::Revoke(request) => run.check_credential_revocation(request)?,
            }),
            Self::Owned(run) => (run.credential_status()?, match request {
                CredentialRequest::Rotate(request) => run.check_credential_rotation(request)?,
                CredentialRequest::Revoke(request) => run.check_credential_revocation(request)?,
            }),
            Self::Mediated(run) => (run.credential_status()?, match request {
                CredentialRequest::Rotate(request) => run.check_credential_rotation(request)?,
                CredentialRequest::Revoke(request) => run.check_credential_revocation(request)?,
            }),
            Self::PredictiveMediated(run) => (run.credential_status()?, match request {
                CredentialRequest::Rotate(request) => run.check_credential_rotation(request)?,
                CredentialRequest::Revoke(request) => run.check_credential_revocation(request)?,
            }),
        };
        Ok((status.ok_or(fa_reference::Error::WrongState)?, receipt))
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

    /// The same complete guarded transaction, retaining its immutable history
    /// for independent custody without exposing a role or general owner.
    pub(super) fn finish_reconciled_receipt_with_history<F>(
        self, request: u64, after: Option<u64>, clock: F,
    ) -> Result<FileReconciledLearnedReceipt, JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self {
            Self::Plain(run) => run.finish_reconciled_receipt_with_history(request, after, clock),
            Self::Raw(run) => run.finish_reconciled_receipt_with_history(request, after, clock),
            Self::Owned(run) => run.finish_reconciled_receipt_with_history(request, after, clock),
            Self::Mediated(run) => run.finish_reconciled_receipt_with_history(request, after, clock),
            Self::PredictiveMediated(run) => run.finish_reconciled_receipt_with_history(request, after, clock),
        }
    }

    /// Validate selected source custody on this same original replay before
    /// the trusted clock or any acknowledgment can run.
    pub(super) fn finish_reconciled_receipt_with_checkpoint<F>(
        self, request: u64, after: Option<u64>, saved: &TopologyCheckpoint, clock: F,
    ) -> Result<FileReconciledLearnedReceipt, JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self {
            Self::Plain(run) => run.finish_reconciled_receipt_with_checkpoint(request, after, saved, clock),
            Self::Raw(run) => run.finish_reconciled_receipt_with_checkpoint(request, after, saved, clock),
            Self::Owned(run) => run.finish_reconciled_receipt_with_checkpoint(request, after, saved, clock),
            Self::Mediated(run) => run.finish_reconciled_receipt_with_checkpoint(request, after, saved, clock),
            Self::PredictiveMediated(run) => run.finish_reconciled_receipt_with_checkpoint(request, after, saved, clock),
        }
    }

    pub(super) fn finish_reconciled_receipt_port_with_history<F>(
        self, request: u64, after: Option<u64>, clock: F,
    ) -> Result<(FileLearnedReceiptActorPort, FileReconciledLearnedReceipt), JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self {
            Self::Plain(run) => run.finish_reconciled_receipt_port_with_history(request, after, clock),
            Self::Raw(run) => run.finish_reconciled_receipt_port_with_history(request, after, clock),
            Self::Owned(run) => run.finish_reconciled_receipt_port_with_history(request, after, clock),
            Self::Mediated(run) => run.finish_reconciled_receipt_port_with_history(request, after, clock),
            Self::PredictiveMediated(run) => run.finish_reconciled_receipt_port_with_history(request, after, clock),
        }
    }

    pub(super) fn finish_reconciled_receipt_port_with_checkpoint<F>(
        self, request: u64, after: Option<u64>, saved: &TopologyCheckpoint, clock: F,
    ) -> Result<(FileLearnedReceiptActorPort, FileReconciledLearnedReceipt), JournalError>
    where F: FnOnce() -> ElapsedTick {
        match self {
            Self::Plain(run) => run.finish_reconciled_receipt_port_with_checkpoint(request, after, saved, clock),
            Self::Raw(run) => run.finish_reconciled_receipt_port_with_checkpoint(request, after, saved, clock),
            Self::Owned(run) => run.finish_reconciled_receipt_port_with_checkpoint(request, after, saved, clock),
            Self::Mediated(run) => run.finish_reconciled_receipt_port_with_checkpoint(request, after, saved, clock),
            Self::PredictiveMediated(run) => run.finish_reconciled_receipt_port_with_checkpoint(request, after, saved, clock),
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

    /// Seal the SAME fully guarded Ready cursor to one explicit authenticated
    /// admission Stop. The transport receives no live owner or mutable role.
    #[cfg(target_os = "linux")]
    pub(super) fn into_recovery_stop(self, operation: u64)
        -> Result<fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop, JournalError>
    {
        match self {
            Self::Plain(run) => run.into_recovery_stop(operation),
            Self::Raw(run) => run.into_recovery_stop(operation),
            Self::Owned(run) => run.into_recovery_stop(operation),
            Self::Mediated(run) => run.into_recovery_stop(operation),
            Self::PredictiveMediated(run) => run.into_recovery_stop(operation),
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
