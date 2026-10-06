//! Enforce configured forecast timing in the ORIGINAL live/replayed machine.
//! No second forecast state, caller-supplied timing claim or numerical executor.
use super::{BaseEvent, ConsistencyEvent, Error, Event, Machine};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;
use crate::action::consequence::oversight::learned_host::text::stream::Release;
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;

impl Machine {
    // Called from check_consistency_route in BOTH live preflight and apply_inner.
    pub(super) fn check_pre_output_route(&self, event: &Event) -> Result<(), Error> {
        if let Event::Consistency(ConsistencyEvent::Enable(config)) = event {
            if config.requires_pre_output_forecast() {
                let learned = self.learned_contract().ok_or(Error::Binding)?;
                if !learned.is_text() || learned.text_stream_profile() != config.stream_message_profile() {
                    return Err(Error::Binding);
                }
                // Existing hosted binding also validates the actual model/layer,
                // stream and lifetime-before-work boundary. Do not widen it.
            }
            return Ok(());
        }
        let Some(config) = self.consistency.as_ref().filter(|c| c.requires_pre_output_forecast())
            else { return Ok(()); };
        match event {
            Event::Consistency(ConsistencyEvent::Forecast(..) | ConsistencyEvent::ForecastRequest(..)
                | ConsistencyEvent::ForecastHosted(..)) | Event::Core(BaseEvent::Propose(..)) => {
                // The whole generated message has ONE external request binding.
                // General supplied-frame or unkeyed APIs cannot substitute it.
                Err(Error::Binding)
            }
            Event::Consistency(ConsistencyEvent::ForecastHostedRequest(..)) => {
                let numerical = self.broker.hosted_learned_generation()?;
                let original = self.broker.hosted_learned_original()?;
                if numerical.status == GenerationStatus::Generating
                    && numerical.position == original.spec().prompt().len() as u64
                    && numerical.work.sampling_attempts == 0
                    && numerical.cumulative_work.sampling_attempts == 0
                    && original.samples().is_empty() { return Ok(()); }
                // Only an independently forecast FINISH may use a later source.
                // Existing builder requires receipt-confirmed original output;
                // mere dispatch or visible-but-unreconciled publication refuses.
                if config.stream_message_profile().is_some() && matches!(numerical.status, GenerationStatus::Finished(_)) {
                    let now = self.broker.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
                    let deadline = ElapsedTick(now.0.checked_add(1).ok_or(Error::Overflow)?);
                    self.broker.learned_stream_spec(Release::Finish, deadline)?;
                    return Ok(());
                }
                Err(Error::WrongState)
            }
            Event::Core(BaseEvent::SubmitRequest(request, spec, _)) => {
                if self.requests.original_spec(*request).is_some() { return Ok(()); }
                let forecast = self.broker.consistency_deadline()?.ok_or(Error::Incomplete)?;
                if self.consistency_request != Some((*request, forecast.attempt)) { return Err(Error::Binding); }
                let prompt = self.broker.hosted_learned_original()?.spec().prompt().len() as u64;
                if config.stream_message_profile().is_some() {
                    let release = if forecast.source_sequence == prompt { Release::Message } else { Release::Finish };
                    if self.broker.learned_stream_spec(release, spec.deadline)? != *spec { return Err(Error::Binding); }
                } else {
                    if forecast.source_sequence != prompt { return Err(Error::Binding); }
                    let message = self.broker.hosted_learned_text_message(LearnedEvidenceLimits::default())?;
                    if message.bytes() != spec.payload.as_slice() { return Err(Error::Binding); }
                }
                // Age, coverage and policy refusals remain original observed
                // outcomes; do not turn them into another likelihood sample.
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // Called by the common learned-position preflight for BOTH Begin and Step,
    // including cooperative/cache finalizers. Refusal precedes inference, draw,
    // witness creation and canonical writes; an old pending intent is retained.
    pub(in super::super) fn check_pre_output_step(&self) -> Result<(), Error> {
        if !self.consistency.as_ref().is_some_and(|c| c.requires_pre_output_forecast()) { return Ok(()); }
        if self.broker.consistency_coverage_lost()? || self.broker.consistency_evidence()?.crossed() {
            return Err(Error::Incomplete);
        }
        let numerical = self.broker.hosted_learned_generation()?;
        if numerical.cumulative_work.sampling_attempts != numerical.work.sampling_attempts {
            return Err(Error::WrongState);
        }
        if numerical.status == GenerationStatus::Prefilling {
            return if self.broker.pending_forecast()?.is_none() { Ok(()) } else { Err(Error::WrongState) };
        }
        let forecast = self.broker.consistency_deadline()?.ok_or(Error::Incomplete)?;
        if !self.consistency_request.is_some_and(|(_, attempt)| attempt == forecast.attempt) {
            return Err(Error::Binding);
        }
        let prompt = self.broker.hosted_learned_original()?.spec().prompt().len() as u64;
        let sampled = numerical.position.checked_sub(prompt).ok_or(Error::Binding)?;
        if forecast.source_sequence != prompt
            || numerical.work.sampling_attempts != sampled
            || forecast.actor_revision.checked_add(sampled) != Some(numerical.actor_revision) {
            return Err(Error::Binding);
        }
        let control = self.broker.inspect();
        let now = control.ledger.elapsed.ok_or(Error::Incomplete)?;
        if control.ledger.epoch != forecast.authority_epoch
            || now < forecast.created_at || now >= forecast.expires_at { return Err(Error::Stale); }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
