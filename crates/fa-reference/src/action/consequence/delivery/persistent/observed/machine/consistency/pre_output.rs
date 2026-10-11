//! Enforce configured forecast timing in the ORIGINAL live/replayed machine.
//! No second forecast state, caller-supplied timing claim or numerical executor.
use super::{BaseEvent, ConsistencyEvent, Error, Event, Machine};
use super::super::super::consistency::FileConsistencyConfig;
use super::super::super::decoder::learned::FileLearnedTextContinuation;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationStatus;
use crate::action::consequence::delivery::stream::ReleaseFrame;
use crate::action::consequence::oversight::learned_host::text::stream::Release;
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;

impl Machine {
    // Called from check_consistency_route in BOTH live preflight and apply_inner.
    pub(super) fn check_pre_output_route(&self, event: &Event) -> Result<(), Error> {
        match event {
            Event::Consistency(ConsistencyEvent::Enable(config)) => {
                if config.requires_pre_output_forecast() { self.check_pre_output_configuration(config)?; }
                return Ok(());
            }
            Event::Consistency(ConsistencyEvent::EnableLearned(binding)) => {
                let config = binding.runtime()?;
                if config.requires_pre_output_forecast() {
                    self.check_pre_output_configuration(config.consistency())?;
                }
                return Ok(());
            }
            _ => {}
        }
        let Some(config) = self.pre_output_configuration() else { return Ok(()); };
        let owned = self.broker.owned_learned_consistency_required();
        match event {
            Event::Consistency(ConsistencyEvent::ForecastHostedRequest(request, ..)) if !owned => {
                self.check_pre_output_forecast_point(config, *request)
            }
            Event::Consistency(ConsistencyEvent::ForecastOwnedLearnedRequest(request, ..)) if owned => {
                self.check_pre_output_forecast_point(config, *request)
            }
            Event::Consistency(ConsistencyEvent::Forecast(..) | ConsistencyEvent::ForecastRequest(..)
                | ConsistencyEvent::ForecastHosted(..) | ConsistencyEvent::ForecastHostedRequest(..)
                | ConsistencyEvent::ForecastLearned(..) | ConsistencyEvent::ForecastLearnedRequest(..)
                | ConsistencyEvent::ForecastOwnedLearned(..) | ConsistencyEvent::ForecastOwnedLearnedRequest(..))
                | Event::Core(BaseEvent::Propose(..)) => Err(Error::Binding),
            Event::Core(BaseEvent::SubmitRequest(request, spec, _)) => {
                if self.requests.original_spec(*request).is_some() { return Ok(()); }
                let forecast = self.broker.consistency_deadline()?.ok_or(Error::Incomplete)?;
                if self.consistency_request != Some((*request, forecast.attempt)) { return Err(Error::Binding); }
                let prompt = self.broker.hosted_learned_original()?.spec().prompt().len() as u64;
                if config.stream_message_profile().is_some() {
                    // A later message also has a post-prompt source sequence.
                    // The original frame, current receipt cut and forecast cut
                    // distinguish it from an independently forecast finish.
                    let release = if ReleaseFrame::decode(&spec.payload)?.is_finish() {
                        let numerical = self.broker.hosted_learned_generation()?;
                        if forecast.source_sequence != numerical.position
                            || forecast.actor_revision != numerical.actor_revision { return Err(Error::Binding); }
                        Release::Finish
                    } else {
                        let boundary = self.pre_output_message_continuation();
                        let start = boundary.map_or(prompt, |command| command.position());
                        if forecast.source_sequence != start
                            || boundary.is_some_and(|command| command.request() != *request
                                || command.actor_revision() != forecast.actor_revision) {
                            return Err(Error::Binding);
                        }
                        Release::Message
                    };
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

    // Configuration selection only. Both representations use the SAME prompt,
    // request, source-sequence, deadline and sampling-history checks below.
    fn pre_output_configuration(&self) -> Option<&FileConsistencyConfig> {
        self.consistency.as_deref().filter(|config| config.requires_pre_output_forecast()
            || self.learned_consistency.as_ref().is_some_and(|config| config.requires_pre_output_forecast()))
    }

    fn check_pre_output_configuration(&self, config: &FileConsistencyConfig) -> Result<(), Error> {
        let learned = self.learned_contract().ok_or(Error::Binding)?;
        if (learned.allows_text_stream_continuation() && !learned.requires_forecast_text_stream_continuation())
            || !learned.is_text()
            || learned.text_stream_profile() != config.stream_message_profile() {
            return Err(Error::Binding);
        }
        // Source-specific bootstrap independently checks actual model, tensor,
        // layer, side, stream and lifetime-before-work. No relaxed binding here.
        Ok(())
    }

    fn check_pre_output_forecast_point(&self, config: &FileConsistencyConfig, request: u64) -> Result<(), Error> {
        let numerical = self.broker.hosted_learned_generation()?;
        if numerical.status == GenerationStatus::Generating {
            return self.check_pre_output_message_point(request);
        }
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

    fn pre_output_message_continuation(&self) -> Option<FileLearnedTextContinuation> {
        self.learned_contract().filter(|config| config.requires_forecast_text_stream_continuation())
            .and_then(|_| self.current_learned_message_continuation())
    }

    // Shared by the required timing route and the optional raw forecast handle.
    // Every later message is identified by the ORIGINAL acknowledged continuation
    // intent. Equality of positions alone cannot replace its request or actor cut.
    pub(in super::super::super) fn check_pre_output_message_point(&self, request: u64) -> Result<(), Error> {
        let numerical = self.broker.hosted_learned_generation()?;
        let original = self.broker.hosted_learned_original()?;
        if numerical.status != GenerationStatus::Generating { return Err(Error::WrongState); }
        let prompt = original.spec().prompt().len() as u64;
        if let Some(command) = self.pre_output_message_continuation() {
            if command.request() != request { return Err(Error::Binding); }
            if numerical.position != command.position() || numerical.actor_revision != command.actor_revision() {
                return Err(Error::WrongState);
            }
            let sampled = numerical.position.checked_sub(prompt).ok_or(Error::Binding)?;
            if numerical.work.sampling_attempts != sampled
                || numerical.cumulative_work.sampling_attempts != sampled
                || original.samples().len() as u64 != sampled
                || !self.broker.learned_message_forecast_source()? { return Err(Error::WrongState); }
            return Ok(());
        }
        if numerical.position != prompt || numerical.work.sampling_attempts != 0
            || numerical.cumulative_work.sampling_attempts != 0 || !original.samples().is_empty() {
            return Err(Error::WrongState);
        }
        Ok(())
    }

    // A pending finish forecast cannot be reinterpreted as the next message's
    // forecast by changing only the generator's status. Refuse before recording
    // the continuation; consumed likelihood samples and spent work remain intact.
    pub(in super::super) fn check_pre_output_message_continuation(&self) -> Result<(), Error> {
        if self.pre_output_configuration().is_none() { return Ok(()); }
        if self.consistency_request.is_some() || self.broker.pending_forecast()?.is_some() {
            return Err(Error::Incomplete);
        }
        if self.broker.consistency_coverage_lost()? || self.broker.consistency_evidence()?.crossed() {
            return Err(Error::Incomplete);
        }
        Ok(())
    }

    // Called by the common learned-position preflight for BOTH Begin and Step,
    // including cooperative/cache finalizers. Refusal precedes inference, draw,
    // witness creation and canonical writes; an old pending intent is retained.
    pub(in super::super) fn check_pre_output_step(&self) -> Result<(), Error> {
        if self.pre_output_configuration().is_none() { return Ok(()); }
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
        let boundary = self.pre_output_message_continuation();
        let start = boundary.map_or(prompt, |command| command.position());
        let total_sampled = numerical.position.checked_sub(prompt).ok_or(Error::Binding)?;
        let message_sampled = numerical.position.checked_sub(start).ok_or(Error::Binding)?;
        if forecast.source_sequence != start
            || numerical.work.sampling_attempts != total_sampled
            || forecast.actor_revision.checked_add(message_sampled) != Some(numerical.actor_revision)
            || boundary.is_some_and(|command| command.actor_revision() != forecast.actor_revision
                || self.consistency_request != Some((command.request(), forecast.attempt))) {
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
