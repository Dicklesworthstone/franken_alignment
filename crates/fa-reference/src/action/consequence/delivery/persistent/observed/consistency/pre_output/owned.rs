//! Original owned-K/V prediction, with the SAME checked numerical continuation.
//! LearnedPrediction is never relabeled as a raw residual Prediction.
use super::{FileConsistencyObserver, FileOversight, FilePreOutputForecast, JournalError,
    GenerationEvent, ConsistencyDeadline, HostedLearnedInspection, Error, Rc};
use crate::action::consequence::activation::consistency::learned::{
    LearnedForecastReport, LearnedPrediction,
};

/// Every variant follows the original forecast transaction's acknowledgment.
/// Only Ready carries a numerical continuation; neither other outcome becomes a
/// fabricated prediction. A storage error is returned separately as JournalError.
#[derive(Debug)]
pub enum FileOwnedPreOutputStart {
    Ready(Box<FileOwnedPreOutputForecast>),
    /// Preserve the full actual numerical report, including work and uncertainty.
    Inconclusive(Box<LearnedForecastReport>),
    /// An acknowledged native error. Original host accounting remains available.
    Refused(Error),
}

/// One owned-K/V forecast and the original process-local continuation checks.
/// No observer, generator, store, actor ticket or effect key can be extracted.
/// Dropping this owner leaves its original unanswered durable forecast in place.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::pre_output::FileOwnedPreOutputForecast;
/// fn copy(run: FileOwnedPreOutputForecast) { run.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::pre_output::FileOwnedPreOutputForecast;
/// fn permit(run: FileOwnedPreOutputForecast) { run.authorize(); }
/// ```
#[derive(Debug)]
#[must_use = "an unanswered forecast remains a durable obligation"]
pub struct FileOwnedPreOutputForecast {
    continuation: FilePreOutputForecast<LearnedPrediction>,
    report: LearnedForecastReport,
}

impl FileConsistencyObserver {
    /// Acknowledge the ORIGINAL owned-code forecast at the prompt boundary or
    /// an explicitly pinned receipt-bound message successor. No source, row,
    /// replacement model, probabilities or clock override are caller inputs.
    ///
    /// All source acquisition and its lifetime work remain in the original
    /// forecast_owned_learned_request transaction. This method does not retry a
    /// refusal or purchase refinement outside that fixed acquisition. An
    /// inconclusive report remains visible and yields no generation handle.
    pub fn begin_owned_pre_output_request(&self, host: &mut FileOversight, revision: u64,
        request: u64, actor_revision: u64, position: u64)
        -> Result<FileOwnedPreOutputStart, JournalError>
    {
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != host.revision() { return Err(Error::Stale.into()); }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        if !host.owned_learned_action_consistency_required() { return Err(Error::Binding.into()); }
        let inspected = host.learned_generation_inspection()?;
        if inspected.numerical.actor_revision != actor_revision || inspected.numerical.position != position {
            return Err(Error::Stale.into());
        }
        if inspected.paused || inspected.pending.is_some() || host.source_interrupted
            || host.machine.pending_learned_reset().is_some() { return Err(Error::Incomplete.into()); }
        host.machine.check_pre_output_message_point(request)?;
        let report = match self.forecast_owned_learned_request(host, revision, request, actor_revision)? {
            Ok(report) => report,
            Err(error) => return Ok(FileOwnedPreOutputStart::Refused(error)),
        };
        let prediction = match report.prediction() {
            Ok(prediction) => prediction.clone(),
            Err(_) => return Ok(FileOwnedPreOutputStart::Inconclusive(Box::new(report))),
        };
        let deadline = host.machine.broker.consistency_deadline()?.ok_or(Error::Incomplete)?;
        if host.machine.consistency_request != Some((request, deadline.attempt))
            || deadline.actor_revision != actor_revision || deadline.source_sequence != position {
            return Err(Error::Binding.into());
        }
        Ok(FileOwnedPreOutputStart::Ready(Box::new(FileOwnedPreOutputForecast {
            continuation: FilePreOutputForecast { issuer: Rc::clone(&host.issuer), request,
                deadline, prediction, numerical: inspected.numerical },
            report,
        })))
    }
}

impl FileOwnedPreOutputForecast {
    /// Consume this original forecast into a bounded replay driver, retaining
    /// its distinct learned acquisition report and the same pending obligation.
    pub fn into_cooperative(self) -> super::FileOwnedPreOutputStepper {
        super::FileOwnedPreOutputStepper::new(self.continuation.into_cooperative(), self.report)
    }

    pub fn request(&self) -> u64 { self.continuation.request() }
    pub fn prediction(&self) -> &LearnedPrediction { self.continuation.prediction() }
    /// Original acknowledged acquisition result, not a live permission or refund.
    pub fn forecast_report(&self) -> &LearnedForecastReport { &self.report }
    pub fn deadline(&self) -> ConsistencyDeadline { self.continuation.deadline() }
    pub fn numerical(&self) -> &HostedLearnedInspection { self.continuation.numerical() }

    /// Delegate to the SAME raw/owned continuation implementation. It rechecks
    /// owner identity, journal predecessor, full original numerical state,
    /// request, deadline, policy epoch and coverage before either durable step.
    pub fn advance(&mut self, host: &mut FileOversight, revision: u64)
        -> Result<Result<Rc<GenerationEvent>, Error>, JournalError>
    {
        self.continuation.advance(host, revision)
    }
}
