//! Request intake consumes the original learned-stream frame builder unchanged.
use super::{FileOversight, JournalError, ActionSpec, VERSION, Error};
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::LearnedTextRelease;
use crate::action::consequence::delivery::stream::ReleaseFrame;
use crate::action::consequence::oversight::learned_host::text::stream::Release;

impl FileOversight {
    pub(in crate::action::consequence::delivery::persistent) fn prepare_learned_text_stream_request(
        &self, request: u64, release: LearnedTextRelease, deadline: ElapsedTick,
    ) -> Result<ActionSpec, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if request == 0 { return Err(Error::InvalidInput.into()); }
        let profile = self.machine.learned_contract().and_then(|config| config.text_stream_profile())
            .ok_or(Error::Binding)?;
        if let Some(original) = self.machine.requests.original_spec(request) {
            // This is the exact request-book pre-policy spec, including refused
            // admissions. Do not derive another payload or target from today's
            // source, pending dispatch or receipt-confirmed audience frontier.
            let frame = ReleaseFrame::decode(&original.payload).map_err(|_| Error::Binding)?;
            if original.deadline != deadline || original.version != VERSION
                || original.scope != self.profile.delivery.scope || !original.required_witnesses.is_empty()
                || original.units != original.payload.len() as u64 || frame.profile() != profile
                || frame.is_finish() != (release == LearnedTextRelease::Finish) {
                return Err(Error::Binding.into());
            }
            return Ok(original.clone());
        }
        if self.source_interrupted || self.machine.pending_learned_reset().is_some() {
            return Err(Error::Incomplete.into());
        }
        let current = self.learned_generation_inspection()?;
        if !self.clock_ready() || current.paused || current.pending.is_some() {
            return Err(Error::Incomplete.into());
        }
        // Only this existing builder decides which original release is ready,
        // the full cumulative payload, target, epoch and entire frame charge.
        // In particular dispatch/visible publication is NOT a confirmed append.
        Ok(self.machine.broker.learned_stream_spec(match release {
            LearnedTextRelease::Message => Release::Message,
            LearnedTextRelease::Finish => Release::Finish,
        }, deadline)?)
    }
}
