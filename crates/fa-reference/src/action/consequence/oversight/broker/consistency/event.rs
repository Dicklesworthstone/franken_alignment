//! A fixed interpretation of the bytes predicted by the original binary model.
use super::{ForecastModel, OversightBroker};
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::Error;

/// A calibration-domain choice, not a detector result or effect permission.
/// Stream mode predicts whether the NEW proposed message has the registered
/// prefix. Prior messages and frame headers are not new observations. A valid
/// explicit finish is a negative event; missing/malformed frames are errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsistencyEventDomain {
    PayloadPrefix,
    StreamMessagePrefix(StreamProfile),
}
impl ConsistencyEventDomain {
    pub(super) fn classify(self, model: &ForecastModel, bytes: &[u8]) -> Result<bool, Error> {
        match self {
            Self::PayloadPrefix => Ok(model.event(bytes)),
            Self::StreamMessagePrefix(expected) => {
                // Parse the ENTIRE original frame, including every prior boundary
                // and UTF-8 span. A suffix or a valid-looking message alone is not
                // a complete release. Publication independently checks live history.
                let frame = ReleaseFrame::decode(bytes)?;
                if frame.profile() != expected { return Err(Error::Binding); }
                Ok(frame.message().is_some_and(|message| model.event(message.as_bytes())))
            }
        }
    }
}

impl OversightBroker {
    /// Fix the event domain before any prediction or proposal. This is separate
    /// from choosing supplied/owned activations and does not re-arm either lane.
    /// Requires an already configured, EXACT matching complete-message endpoint.
    /// There is no switch back to raw bytes or replacement after seeing outcomes.
    pub fn require_stream_message_consistency(&mut self, expected: StreamProfile) -> Result<(), Error> {
        let state = self.consistency.as_ref().ok_or(Error::Incomplete)?;
        if state.event_domain != ConsistencyEventDomain::PayloadPrefix { return Err(Error::Duplicate); }
        if state.jobs != 0 || state.pending.is_some() || !state.observations.is_empty()
            || state.coverage_lost || !self.inputs.is_empty() || !self.started_rounds.is_empty()
            || self.inspect().sequence != 0 || !self.inspect().ledger.stages.is_empty()
        { return Err(Error::WrongState); }
        if self.stream_state().map(|(_, view)| view.profile()) != Some(expected) { return Err(Error::Binding); }
        self.consistency.as_mut().expect("configured consistency lane").event_domain =
            ConsistencyEventDomain::StreamMessagePrefix(expected);
        Ok(())
    }

    /// Supervisor-only interpretation attached to this lifetime evidence process.
    pub fn consistency_event_domain(&self) -> Option<ConsistencyEventDomain> {
        self.consistency.as_ref().map(|state| state.event_domain)
    }
}

#[cfg(test)]
mod tests;
