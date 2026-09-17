//! Use the original predictor, sequential process and proposal authority.
use super::{Machine, Transition};
use super::super::consistency::{ConsistencyEvent, FileConsistencySnapshot};
use super::super::{Event, BaseEvent};
use crate::Error;
use super::super::super::requests::MAX_FILE_REQUESTS;

impl Machine {
    pub(in super::super) fn preflight_consistency(&self, event: &Event) -> Result<(), Error> {
        self.check_consistency_route(event)?;
        if let Event::Consistency(ConsistencyEvent::ForecastRequest(request, ..)
            | ConsistencyEvent::ForecastHostedRequest(request, ..)) = event {
            self.preflight_forecast_request(*request)?;
        }
        if self.consistency.is_none() { return Ok(()); }
        if matches!(event, Event::Consistency(ConsistencyEvent::Forecast(..) | ConsistencyEvent::ForecastRequest(..)
            | ConsistencyEvent::ForecastHosted(..) | ConsistencyEvent::ForecastHostedRequest(..))
            | Event::Core(BaseEvent::Propose(..) | BaseEvent::SubmitRequest(..))) {
            self.check_decoder_admission(event)?;
            if !self.clock_ready { return Err(Error::Incomplete); }
            if self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
        }
        if let Event::Core(BaseEvent::SubmitRequest(request, spec, _)) = event {
            // This is the original side-effect-free allocator preflight, before
            // poisoning for a NEW observed action. It does not reserve an ID.
            self.requests.prepare(*request, spec, self.scope, &self.broker.inspect(),
                self.broker.stop_receipt().is_some())?;
        }
        Ok(())
    }
    pub(super) fn apply_consistency(&mut self, event: &ConsistencyEvent) -> Result<Transition, Error> {
        match event {
            ConsistencyEvent::Enable(config) => {
                if self.consistency.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                self.broker.enable_action_consistency(config.build()?)?;
                if let Some(layer) = config.hosted_residual_layer() {
                    self.broker.require_hosted_action_consistency(layer)?;
                }
                if !self.publication_guard { self.enable_publication_guard()?; }
                self.consistency = Some(config.clone());
                Ok(Transition::Unit)
            }
            ConsistencyEvent::Forecast(attempt, revision, frame) => {
                if !self.clock_ready || self.decoder_paused() { return Err(Error::Incomplete); }
                if self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                // A native Err can consume a prediction job or record coverage
                // loss. It is a committed outcome, not a discarded candidate.
                Ok(Transition::ConsistencyForecast(Box::new(self.broker.forecast_action(*attempt, *revision, frame))))
            }
            ConsistencyEvent::ForecastRequest(request, revision, frame) => {
                let attempt = self.preflight_forecast_request(*request)?;
                let result = self.broker.forecast_action(attempt, *revision, frame);
                if result.is_ok() { self.consistency_request = Some((*request, attempt)); }
                Ok(Transition::ConsistencyForecast(Box::new(result)))
            }
            ConsistencyEvent::ForecastHosted(attempt, revision) => {
                if !self.clock_ready || self.decoder_paused() { return Err(Error::Incomplete); }
                if self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                Ok(Transition::ConsistencyForecast(Box::new(self.broker.forecast_hosted_action(*attempt, *revision))))
            }
            ConsistencyEvent::ForecastHostedRequest(request, revision) => {
                let attempt = self.preflight_forecast_request(*request)?;
                let result = self.broker.forecast_hosted_action(attempt, *revision);
                if result.is_ok() { self.consistency_request = Some((*request, attempt)); }
                Ok(Transition::ConsistencyForecast(Box::new(result)))
            }
            ConsistencyEvent::Unavailable => {
                self.broker.consistency_unavailable()?;
                Ok(Transition::Unit)
            }
        }
    }

    pub(in super::super) fn consistency_snapshot(&self, revision: u64) -> Result<FileConsistencySnapshot, Error> {
        Ok(FileConsistencySnapshot { journal_revision: revision,
            evidence: self.broker.consistency_evidence()?.clone(),
            pending_attempt: self.broker.pending_forecast()?,
            coverage_lost: self.broker.consistency_coverage_lost()? })
    }

    /// Route binding is checked before any action observation and on replay.
    /// No unrelated request or trusted raw proposal may consume a keyed forecast.
    pub(super) fn check_consistency_route(&self, event: &Event) -> Result<(), Error> {
        let Some((request, attempt)) = self.consistency_request else { return Ok(()); };
        match event {
            Event::Core(BaseEvent::Propose(..)) => Err(Error::Binding),
            Event::Core(BaseEvent::SubmitRequest(key, ..)) => {
                if *key != request || self.requests.next_attempt(&self.broker.inspect())? != attempt {
                    return Err(Error::Binding);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn preflight_forecast_request(&self, request: u64) -> Result<u64, Error> {
        if self.consistency.is_none() || !self.clock_ready || self.decoder_paused() { return Err(Error::Incomplete); }
        if request == 0 { return Err(Error::InvalidInput); }
        match self.requests.status(request) {
            Ok(_) => return Err(Error::Duplicate),
            Err(Error::Missing) => {}
            Err(error) => return Err(error),
        }
        if self.consistency_request.is_some() || self.broker.pending_forecast()?.is_some() {
            return Err(Error::WrongState);
        }
        let control = self.broker.inspect();
        if control.suspended || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
        if self.requests.len() >= MAX_FILE_REQUESTS { return Err(Error::Limit); }
        self.requests.next_attempt(&control)
    }

    pub(super) fn finish_consistency_request(&mut self, request: u64, attempt: u64) -> Result<(), Error> {
        if self.consistency_request != Some((request, attempt)) { return Ok(()); }
        match self.broker.pending_forecast()? {
            None => self.consistency_request = None,
            Some(id) if id == attempt => {
                // The original request is now terminal NotAdmitted without a
                // usable observation (e.g. expiry). Its pending outcome cannot
                // be recycled under another key or retried into a new sample.
                self.broker.consistency_unavailable()?;
            }
            Some(_) => return Err(Error::Binding),
        }
        Ok(())
    }

    pub(super) fn recover_consistency(&mut self) -> Result<(), Error> {
        // An outstanding forecast has an unknown outcome. Losing it must not
        // select a convenient subsequence or mint a fresh lifetime error budget.
        // Preserve the native pending record and its permanent coverage-loss
        // latch. A fully observed quiet history needs only fresh future forecasts.
        if self.consistency.is_some() && self.broker.pending_forecast()?.is_some() {
            self.broker.consistency_unavailable()?;
        }
        Ok(())
    }
}
