//! Journal commands for actual owned residuals, not serialized stand-in captures.
use super::{ConsistencyEvent, FileConsistencyObserver, FileOversight, JournalError, Transition, Event};
use crate::action::consequence::activation::consistency::Prediction;
use crate::Error;
use std::rc::Rc;

impl FileConsistencyObserver {
    pub fn forecast_hosted_action(&self, host: &mut FileOversight, revision: u64,
        attempt: u64, actor_revision: u64) -> Result<Result<Prediction, Error>, JournalError>
    {
        self.forecast_owned(host, revision, ConsistencyEvent::ForecastHosted(attempt, actor_revision))
    }

    /// Bind the original allocator's next attempt to this external key before
    /// receiving a payload. No source, clock, model or sequence is supplied.
    /// Replay computes its source from original numerical input history. A native
    /// refusal is committed; outer storage failure returns no candidate result.
    pub fn forecast_hosted_request(&self, host: &mut FileOversight, revision: u64,
        request: u64, actor_revision: u64) -> Result<Result<Prediction, Error>, JournalError>
    {
        self.forecast_owned(host, revision, ConsistencyEvent::ForecastHostedRequest(request, actor_revision))
    }

    fn forecast_owned(&self, host: &mut FileOversight, revision: u64, event: ConsistencyEvent)
        -> Result<Result<Prediction, Error>, JournalError>
    {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        match host.transact(revision, Event::Consistency(event))? {
            Transition::ConsistencyForecast(result) => Ok(*result),
            _ => unreachable!("owned forecast transition"),
        }
    }
}
impl FileOversight {
    /// Source configuration only, never a raw activation accessor or live key.
    pub fn hosted_consistency_layer(&self) -> Result<Option<u64>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.hosted_consistency_layer())
    }
}

#[cfg(test)]
mod tests;
