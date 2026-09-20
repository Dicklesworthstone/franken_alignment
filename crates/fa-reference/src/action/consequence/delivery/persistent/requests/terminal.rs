//! Terminal-only transactions over the existing journal publication endpoint.
//! No network or independently visible filesystem effect is replayed here.

use super::super::{Event, FileDelivery, FileStopSweep, JournalError, StopRequest, Transition};
use crate::action::ElapsedTick;

impl FileDelivery {
    /// Permanently stop admission and drain the original endpoint obligations
    /// with one canonical journal replacement.
    ///
    /// The original Stop and StopProgress reducers run in a private RAM
    /// projection. Undispatched reservations can be cancelled; dispatched
    /// charges are released only by accepted endpoint nonexecution evidence.
    /// Executed outcomes remain charged, and unresolved outcomes remain explicit
    /// in the returned sweep. No request is resent.
    ///
    /// Both events may spend an explicitly installed recovery reserve. This
    /// operation does not append an ordinary Time or Sweep event, so exhaustion
    /// of ordinary admission capacity does not prevent terminal recovery when
    /// the two recovery records still fit. Logical capacity is not reserved
    /// physical disk space.
    ///
    /// A fresh trusted elapsed observation is required. A stale observation,
    /// stale predecessor, rejected stop request or insufficient capacity leaves
    /// the canonical state unchanged. In particular, a rejected drain does not
    /// acknowledge a local stop: use request_stop alone when a trusted clock is
    /// unavailable and immediate, separate local-stop acknowledgment is needed.
    ///
    /// Storage failure returns no candidate receipt or refund and makes this
    /// owner unavailable until exclusive reopen. Even an exact StopRequest retry
    /// still requires the current journal revision and a valid new observation:
    /// this method advances drain progress rather than returning cached progress.
    /// It never reopens intake after an acknowledged stop.
    pub fn stop_and_drain(
        &mut self,
        revision: u64,
        request: StopRequest,
        observed_tick: ElapsedTick,
    ) -> Result<FileStopSweep, JournalError> {
        let mut transitions = self.commit_request_events(
            revision,
            &[Event::Stop(request), Event::StopProgress(observed_tick)],
        )?;
        match transitions.pop() {
            Some(Transition::StopProgressed(sweep)) => Ok(sweep),
            _ => unreachable!("terminal transaction ends with original stop progress"),
        }
    }
}
