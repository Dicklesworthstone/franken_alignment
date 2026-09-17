//! Bind pre-payload prediction to an external request, not a guessed attempt ID.
use super::{ConsistencyEvent, FileConsistencyObserver, FileOversight, Event, JournalError, Transition};
use crate::action::consequence::activation::{SourceFrame, consistency::Prediction};
use crate::Error;
use std::rc::Rc;

impl FileConsistencyObserver {
    /// Capture BEFORE receiving the actor payload. The original request book
    /// derives the internal attempt; this key alone may consume that forecast.
    /// This is not an effect reservation. All original admission checks remain.
    /// A pending or already admitted key cannot be forecast again. A failed
    /// storage acknowledgment requires explicit recovery, not blind repetition.
    pub fn forecast_request(&self, host: &mut FileOversight, revision: u64, request: u64,
        actor_revision: u64, frame: &SourceFrame) -> Result<Result<Prediction, Error>, JournalError>
    {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        match host.transact(revision,
            Event::Consistency(ConsistencyEvent::ForecastRequest(request, actor_revision, frame.clone())))? {
            Transition::ConsistencyForecast(result) => Ok(*result),
            _ => unreachable!("keyed consistency prediction"),
        }
    }
}
impl FileOversight {
    /// Supervisor-only identity of an outstanding keyed forecast. Raw-attempt
    /// forecasts return None here; inspect pending_attempt for their native ID.
    pub fn pending_forecast_request(&self) -> Result<Option<u64>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !self.action_consistency_required() { return Err(Error::Incomplete.into()); }
        Ok(self.machine.consistency_request.map(|(request, _)| request))
    }
}
