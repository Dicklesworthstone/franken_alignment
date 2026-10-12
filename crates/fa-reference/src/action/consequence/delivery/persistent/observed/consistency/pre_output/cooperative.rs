//! Bounded original-history quanta under an already acknowledged forecast.
//! No forecast, source, numerical owner, effect key or budget is recreated.
use super::{ConsistencyDeadline, Error, FileOversight, FilePreOutputForecast,
    GenerationEvent, HostedLearnedInspection, JournalError, Prediction, Rc, fmt};
use crate::action::consequence::activation::consistency::learned::{
    LearnedForecastReport, LearnedPrediction,
};
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS,
    observed::decoder::learned::{FileLearnedIntentPreparation,
        FileLearnedReplayContinuation, FileLearnedStepPreparation,
        FileLearnedStepPreparationProgress, FileLearnedStepPreparationStatus}};

/// One bounded slice of original history, followed by at most ONE original
/// finalization. IntentAcknowledged is not a numerical result. Only Completed
/// carries an acknowledged event (or an acknowledged native numerical error).
/// No variant implies review, human approval, dispatch or publication.
#[derive(Debug)]
#[must_use = "only Completed contains an acknowledged numerical outcome"]
pub enum FilePreOutputQuantum {
    ReplayingIntent {
        progress: FileLearnedStepPreparationProgress,
        replayed_events: usize,
    },
    ReplayingCompletion {
        progress: FileLearnedStepPreparationProgress,
        replayed_events: usize,
    },
    IntentAcknowledged {
        journal_revision: u64,
        replayed_events: usize,
    },
    Completed {
        result: Result<Rc<GenerationEvent>, Error>,
        replayed_events: usize,
    },
}
impl FilePreOutputQuantum {
    /// Actual historical reducer calls in THIS invocation. A retained verified
    /// prefix is not counted again. The one finalizer is excluded explicitly.
    pub fn replayed_events(&self) -> usize {
        match self {
            Self::ReplayingIntent { replayed_events, .. }
            | Self::ReplayingCompletion { replayed_events, .. }
            | Self::IntentAcknowledged { replayed_events, .. }
            | Self::Completed { replayed_events, .. } => *replayed_events,
        }
    }
}

#[derive(Debug)]
enum Preparation {
    Intent(FileLearnedIntentPreparation),
    Completion(FileLearnedStepPreparation),
}

/// Process-local scheduling of the SAME original forecast and numerical step.
/// Conversion consumes the synchronous handle; there is no inner-owner escape,
/// cloning, serialization or alternate source. Drop leaves any acknowledged
/// intent AND unanswered forecast in the original durable owner.
///
/// Each call revalidates the full original forecast before replay and again
/// before finalization. Appended clocks/source observations are caught up by the
/// original bounded replay algorithm; verified history is never restarted to
/// hide a failed reducer. A changed numerical predecessor cannot be adopted.
///
/// This is an EVENT-count bound, not a wall-time bound. One historical reducer,
/// one new numerical step, encoding and a storage replacement remain synchronous.
/// The first preparation starts cold, leaving the live owner's replay cache
/// available for intervening control/source transitions. Later stages move the
/// original retired machine and replay its tail. No executor admission is added.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::pre_output::FilePreOutputStepper;
/// fn duplicate(stepper: FilePreOutputStepper) { stepper.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::pre_output::FilePreOutputStepper;
/// fn escape(stepper: FilePreOutputStepper) { stepper.into_inner(); }
/// ```
#[must_use = "drive bounded quanta or retain the original unanswered forecast"]
pub struct FilePreOutputStepper<P = Prediction> {
    forecast: FilePreOutputForecast<P>,
    preparation: Option<Preparation>,
    carry: Option<FileLearnedReplayContinuation>,
    finalization_interrupted: bool,
}
impl<P> fmt::Debug for FilePreOutputStepper<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePreOutputStepper")
            .field("forecast", &self.forecast)
            .field("preparation", &self.preparation)
            .field("finalization_interrupted", &self.finalization_interrupted)
            .finish_non_exhaustive()
    }
}
impl<P> FilePreOutputForecast<P> {
    /// Move, never clone, this acknowledged forecast into cooperative scheduling.
    /// Conversion itself does no replay, I/O, inference or clock observation.
    /// The existing synchronous advance method and immutable journal stay intact.
    pub fn into_cooperative(self) -> FilePreOutputStepper<P> {
        FilePreOutputStepper { forecast: self, preparation: None, carry: None,
            finalization_interrupted: false }
    }
}
impl<P> FilePreOutputStepper<P> {
    pub fn request(&self) -> u64 { self.forecast.request() }
    pub fn prediction(&self) -> &P { self.forecast.prediction() }
    pub fn deadline(&self) -> ConsistencyDeadline { self.forecast.deadline() }
    /// Last acknowledged numerical state, not a fresh policy/authority judgment.
    pub fn numerical(&self) -> &HostedLearnedInspection { self.forecast.numerical() }

    /// Replay at most max_events historical transitions and finalize at most one
    /// original intent OR completion. A newly acknowledged intent always yields
    /// before completion, so the supervisor can service stop/time/source work.
    /// No mutable host borrow survives the return.
    ///
    /// revision is the caller-observed LIVE cut. Catch-up explicitly visits any
    /// appended tail; it cannot repair expired/consumed forecasts, lost coverage,
    /// source interruption, changed numerical state, revocation or recovery.
    /// Zero/oversized quanta and stale/foreign calls do no replay or write.
    ///
    /// A replay failure stays in its original preparation. Failure or unwind
    /// during a consuming finalizer makes this stepper unusable; it cannot cold
    /// retry an uncertain operation. Recovery, not this value, settles storage.
    /// Completed.result distinguishes an acknowledged numerical refusal from an
    /// outer storage/admission error. Neither can be treated as permitted output.
    pub fn advance_quantum(&mut self, host: &mut FileOversight, revision: u64,
        max_events: usize) -> Result<FilePreOutputQuantum, JournalError>
    {
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        self.forecast.check_current(host, revision)?;
        if self.finalization_interrupted { return Err(JournalError::Unavailable); }
        if self.preparation.is_none() {
            let actor = self.forecast.numerical.actor_revision;
            let position = self.forecast.numerical.position;
            let pending = host.learned_generation_inspection()?.pending.is_some();
            // Keep the LIVE owner's cache for control/source work between
            // quanta. Stealing it here would force that control path to replay
            // cold. This task starts cold once, then moves its own acknowledged
            // original predecessors; it never clones either owner's state.
            let carry = self.carry.take();
            self.preparation = Some(match (pending, carry) {
                (false, Some(carry)) => Preparation::Intent(
                    carry.prepare_intent(host, revision, actor, position)?),
                (false, None) => Preparation::Intent(
                    host.prepare_learned_step_intent(revision, actor, position)?),
                (true, Some(carry)) => Preparation::Completion(
                    carry.prepare_completion(host, revision, actor, position)?),
                (true, None) => Preparation::Completion(
                    host.prepare_learned_step_completion(revision, actor, position)?),
            });
        }
        let (intent, before, progress) = match self.preparation.as_mut()
            .expect("original preparation was installed") {
            Preparation::Intent(task) => {
                let before = task.progress().replayed_events;
                (true, before, task.catch_up(host, revision, before, max_events)?)
            }
            Preparation::Completion(task) => {
                let before = task.progress().replayed_events;
                (false, before, task.catch_up(host, revision, before, max_events)?)
            }
        };
        let replayed_events = progress.replayed_events.checked_sub(before).ok_or(Error::Binding)?;
        if progress.status != FileLearnedStepPreparationStatus::Ready {
            return Ok(if intent {
                FilePreOutputQuantum::ReplayingIntent { progress, replayed_events }
            } else {
                FilePreOutputQuantum::ReplayingCompletion { progress, replayed_events }
            });
        }
        self.forecast.check_current(host, revision)?;
        // Set BEFORE consuming the candidate, including before encoding/I/O.
        // On unwinding or an outer Err there is no reusable earlier candidate.
        self.finalization_interrupted = true;
        let outcome = match self.preparation.take().expect("fully replayed original preparation") {
            Preparation::Intent(task) => {
                self.carry = Some(task.finish_with_continuation(host)?);
                FilePreOutputQuantum::IntentAcknowledged {
                    journal_revision: host.revision(), replayed_events,
                }
            }
            Preparation::Completion(task) => {
                let (result, carry) = task.finish_with_continuation(host)?;
                self.forecast.numerical = host.learned_generation_inspection()?.numerical;
                self.carry = Some(carry);
                FilePreOutputQuantum::Completed { result, replayed_events }
            }
        };
        self.finalization_interrupted = false;
        Ok(outcome)
    }
}

/// Owned learned predictions keep their original typed report as well as the
/// common bounded scheduling checks. It cannot be mistaken for a raw forecast.
#[derive(Debug)]
#[must_use = "an unanswered owned forecast remains a durable obligation"]
pub struct FileOwnedPreOutputStepper {
    continuation: FilePreOutputStepper<LearnedPrediction>,
    report: LearnedForecastReport,
}
impl FileOwnedPreOutputStepper {
    pub(super) fn new(continuation: FilePreOutputStepper<LearnedPrediction>,
        report: LearnedForecastReport) -> Self {
        Self { continuation, report }
    }
    pub fn request(&self) -> u64 { self.continuation.request() }
    pub fn prediction(&self) -> &LearnedPrediction { self.continuation.prediction() }
    pub fn forecast_report(&self) -> &LearnedForecastReport { &self.report }
    pub fn deadline(&self) -> ConsistencyDeadline { self.continuation.deadline() }
    pub fn numerical(&self) -> &HostedLearnedInspection { self.continuation.numerical() }
    pub fn advance_quantum(&mut self, host: &mut FileOversight, revision: u64,
        max_events: usize) -> Result<FilePreOutputQuantum, JournalError> {
        self.continuation.advance_quantum(host, revision, max_events)
    }
}

#[cfg(test)]
mod tests;
