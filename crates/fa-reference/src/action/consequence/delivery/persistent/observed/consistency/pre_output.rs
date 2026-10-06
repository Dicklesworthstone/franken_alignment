//! A forecast acknowledged at the prompt boundary, before original sampling.
//! This supervisor-only continuation never admits or publishes an actor request.
use super::{FileConsistencyObserver, FileOversight, JournalError};
use crate::action::consequence::activation::consistency::Prediction;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{GenerationEvent, GenerationStatus};
use crate::action::consequence::oversight::consistency::ConsistencyDeadline;
use crate::action::consequence::oversight::learned_host::HostedLearnedInspection;
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// A process-local continuation of one durably forecast external request.
/// Constructed only at the original prompt/output boundary. It holds no Store,
/// generator, source writer, observer role, actor ticket or publication key.
/// It cannot be cloned, rebound, recovered from data or used with another owner.
///
/// Dropping it does NOT cancel the forecast, synthesize an outcome or refund
/// prediction work. The original pending forecast and expiry rules remain live.
/// A crash loses this continuation; original recovery preserves coverage loss
/// rather than supplying a replacement forecast or error budget.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::pre_output::FilePreOutputForecast;
/// fn duplicate(run: FilePreOutputForecast) { run.clone(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::consistency::pre_output::FilePreOutputForecast;
/// fn publish(run: FilePreOutputForecast) { run.authorize(); }
/// ```
#[must_use = "drive the original continuation or retain its unanswered forecast obligation"]
pub struct FilePreOutputForecast {
    issuer: Rc<()>,
    request: u64,
    deadline: ConsistencyDeadline,
    prediction: Prediction,
    numerical: HostedLearnedInspection,
}
impl fmt::Debug for FilePreOutputForecast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePreOutputForecast").field("request", &self.request)
            .field("position", &self.numerical.position).field("status", &self.numerical.status)
            .finish_non_exhaustive()
    }
}
impl FileConsistencyObserver {
    /// Freeze the original predictor's request BEFORE any continuation attempt.
    /// First advance the original generator through its complete fixed prompt.
    /// This call admits no new numerical step and consumes no sampling draw.
    ///
    /// The exact prompt-end residual supplies the existing ForecastHostedRequest
    /// transaction. An earlier prompt, a completed output, even one sampled
    /// token, or sampled work abandoned by a reset cannot enter this path.
    /// Owned-residual consistency must already be registered at bootstrap.
    ///
    /// Outer Err: no returned continuation. Inner Err: the ORIGINAL forecast
    /// refusal was acknowledged, including its retained work/coverage outcome.
    /// Success acknowledges only the forecast, not an actor request or either
    /// publication key. The existing unrestricted pre-action API is unchanged;
    /// only this explicitly selected path claims a pre-output boundary.
    pub fn begin_pre_output_request(&self, host: &mut FileOversight, revision: u64,
        request: u64, actor_revision: u64, position: u64)
        -> Result<Result<FilePreOutputForecast, Error>, JournalError>
    {
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if revision != host.revision() { return Err(Error::Stale.into()); }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        if host.hosted_consistency_layer()?.is_none() { return Err(Error::Binding.into()); }
        let inspected = host.learned_generation_inspection()?;
        if inspected.numerical.actor_revision != actor_revision || inspected.numerical.position != position {
            return Err(Error::Stale.into());
        }
        if inspected.paused || inspected.pending.is_some() || host.source_interrupted
            || host.machine.pending_learned_reset().is_some() { return Err(Error::Incomplete.into()); }
        let original = host.machine.broker.hosted_learned_original()?;
        if inspected.numerical.status != GenerationStatus::Generating
            || position != original.spec().prompt().len() as u64
            || inspected.numerical.work.sampling_attempts != 0
            || inspected.numerical.cumulative_work.sampling_attempts != 0
            || !original.samples().is_empty() { return Err(Error::WrongState.into()); }
        let prediction = match self.forecast_hosted_request(host, revision, request, actor_revision)? {
            Ok(prediction) => prediction,
            Err(error) => return Ok(Err(error)),
        };
        // These are the existing acknowledged forecast's immutable fields,
        // never a fresh timer or another allocator reservation.
        let deadline = host.machine.broker.consistency_deadline()?.ok_or(Error::Incomplete)?;
        if host.machine.consistency_request != Some((request, deadline.attempt))
            || deadline.actor_revision != actor_revision || deadline.source_sequence != position
        { return Err(Error::Binding.into()); }
        Ok(Ok(FilePreOutputForecast { issuer: Rc::clone(&host.issuer), request,
            deadline, prediction, numerical: inspected.numerical }))
    }
}
impl FilePreOutputForecast {
    pub fn request(&self) -> u64 { self.request }
    pub fn prediction(&self) -> &Prediction { &self.prediction }
    pub fn deadline(&self) -> ConsistencyDeadline { self.deadline }
    /// Last acknowledged numerical state, not current policy or effect authority.
    pub fn numerical(&self) -> &HostedLearnedInspection { &self.numerical }

    /// One ORIGINAL numerical step under the already acknowledged forecast.
    /// Intent and witnessed completion retain their two distinct durable writes.
    /// This method supplies no forced ID, alternative sampler, mutable source,
    /// policy snapshot, action payload or matched Boolean.
    ///
    /// Caller-observed journal revisions allow interleaved clocks and source
    /// refreshes. The original pending forecast, authority epoch, deadline and
    /// exact numerical predecessor must still match. Another observer cannot
    /// replace/consume the forecast unnoticed; another generation step cannot
    /// be silently adopted. A pending matching intent can finish normally.
    ///
    /// Once output completes, submit it through the existing source-only actor
    /// port under request(). That original admission observes the category once.
    /// Congress, both keys and current-source checks remain separate operations.
    pub fn advance(&mut self, host: &mut FileOversight, revision: u64)
        -> Result<Result<Rc<GenerationEvent>, Error>, JournalError>
    {
        self.check_current(host, revision)?;
        let result = host.advance_learned_generation(revision,
            self.numerical.actor_revision, self.numerical.position)?;
        // Both an accepted/held event and an inner numerical Err are committed
        // outcomes. Keep their actual state; never retry an earlier quiet prefix.
        self.numerical = host.learned_generation_inspection()?.numerical;
        Ok(result)
    }

    fn check_current(&self, host: &FileOversight, revision: u64) -> Result<(), JournalError> {
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        if host.revision() != revision { return Err(Error::Stale.into()); }
        if !self.numerical.status.is_active() { return Err(Error::WrongState.into()); }
        let current = host.learned_generation_inspection()?;
        if current.paused || host.source_interrupted || host.machine.pending_learned_reset().is_some()
            || !host.clock_ready() { return Err(Error::Incomplete.into()); }
        if current.numerical != self.numerical { return Err(Error::Stale.into()); }
        if host.machine.consistency_request != Some((self.request, self.deadline.attempt))
            || host.machine.broker.consistency_deadline()? != Some(self.deadline)
        { return Err(Error::Binding.into()); }
        let evidence = host.action_consistency_snapshot()?;
        if evidence.coverage_lost || evidence.evidence.crossed() { return Err(Error::Incomplete.into()); }
        let control = host.inspect().control;
        if control.suspended || host.inspect().stop.is_some() { return Err(Error::WrongState.into()); }
        let now = control.ledger.elapsed.ok_or(Error::Incomplete)?;
        if control.ledger.epoch != self.deadline.authority_epoch
            || now < self.deadline.created_at || now >= self.deadline.expires_at {
            return Err(Error::Stale.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod consumer_tests;
