//! Unreviewed stream proposals derived from receipt-confirmed audience history.
//! No helper, activation, human-key or dispatch check is bypassed by these builders.

use super::OversightBroker;
use crate::action::consequence::delivery::stream::StreamView;
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::Error;

impl OversightBroker {
    /// The confirmed prefix, not a claim that an unresolved message was unseen.
    pub fn stream_state(&self) -> Option<(ResolvedTarget, &StreamView)> {
        self.delivery.stream_state()
    }

    pub fn stream_pending(&self) -> Option<u64> { self.delivery.stream_pending() }

    /// Variable bytes in the additional structured history only. This excludes
    /// action/receipt copies, allocator overhead and the endpoint's own state.
    pub fn retained_stream_bytes(&self) -> usize {
        self.delivery.stream_state().map_or(0, |(_, view)| view.logical_bytes())
    }

    pub fn stream_message_spec(&self, message: &str, deadline: ElapsedTick) -> Result<ActionSpec, Error> {
        self.stream_spec(Some(message), deadline)
    }

    pub fn stream_finish_spec(&self, deadline: ElapsedTick) -> Result<ActionSpec, Error> {
        self.stream_spec(None, deadline)
    }

    fn stream_spec(&self, message: Option<&str>, deadline: ElapsedTick) -> Result<ActionSpec, Error> {
        if self.delivery.stream_pending().is_some() { return Err(Error::Incomplete); }
        let (target, view) = self.delivery.stream_state().ok_or(Error::Binding)?;
        let inspection = self.inspect();
        if inspection.suspended { return Err(Error::WrongState); }
        if inspection.ledger.elapsed.ok_or(Error::Incomplete)? >= deadline { return Err(Error::Stale); }
        let payload = match message {
            Some(message) => view.encode_message(message)?,
            None => view.encode_finish()?,
        };
        // Charge the complete execution-bearing frame, including repeated
        // cumulative context. New audience bytes are a smaller separate bound.
        let units = u64::try_from(payload.len()).map_err(|_| Error::Limit)?;
        Ok(ActionSpec {
            version: VERSION, scope: self.scope, target: Some(target), payload,
            required_witnesses: Vec::new(), policy_epoch: inspection.ledger.epoch,
            deadline, units,
        })
    }
}
