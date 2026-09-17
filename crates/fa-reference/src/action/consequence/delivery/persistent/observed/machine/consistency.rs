//! Use the original predictor, sequential process and proposal authority.
use super::{Machine, Transition};
use super::super::consistency::{ConsistencyEvent, FileConsistencySnapshot};
use super::super::{Event, BaseEvent};
use crate::Error;

impl Machine {
    pub(in super::super) fn preflight_consistency(&self, event: &Event) -> Result<(), Error> {
        if self.consistency.is_none() { return Ok(()); }
        if matches!(event, Event::Consistency(ConsistencyEvent::Forecast(..))
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
