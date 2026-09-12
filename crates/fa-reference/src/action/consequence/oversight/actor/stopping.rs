//! Close actor intake without dropping its original ticket/outcome mailbox.

use super::ActorSupervisor;
use crate::action::consequence::delivery::{PublicationEndpoint, StopProgress, StopReceipt, StopRequest, StopSweep};
use crate::Error;

impl ActorSupervisor {
    /// Stop the original control ledger and close queued intake in one local
    /// handoff. Existing tickets stay usable for polling and exact retry. Only
    /// queued or genuinely undispatched work becomes cancelled-before-dispatch.
    /// The actor cannot call this supervisor method through its ActorPort.
    pub fn request_stop(&mut self, request: StopRequest) -> Result<StopReceipt, Error> {
        let mut state = self.mailbox.try_borrow_mut().map_err(|_| Error::WrongState)?;
        let receipt = self.broker.request_stop(request)?;
        state.close_intake();
        drop(state);
        self.synchronize()?;
        Ok(receipt)
    }

    pub fn stop_receipt(&self) -> Option<&StopReceipt> { self.broker.stop_receipt() }
    pub fn stop_progress(&self) -> Result<StopProgress, Error> { self.broker.stop_progress() }

    pub fn progress_stop(&mut self, endpoint: &mut PublicationEndpoint) -> Result<StopSweep, Error> {
        let result = self.broker.progress_stop(endpoint);
        self.synchronize()?;
        result
    }
}
