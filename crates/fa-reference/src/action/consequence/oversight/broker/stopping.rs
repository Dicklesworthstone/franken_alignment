//! Stopping is control-plane work, independent of current helper observations.

use super::OversightBroker;
use crate::action::consequence::delivery::{PublicationEndpoint, StopProgress, StopReceipt, StopRequest, StopSweep};
use crate::Error;

impl OversightBroker {
    pub fn request_stop(&mut self, request: StopRequest) -> Result<StopReceipt, Error> {
        let receipt = self.delivery.request_stop(request)?;
        for slot in self.inputs.values_mut() { slot.approved = None; }
        Ok(receipt)
    }

    pub fn stop_receipt(&self) -> Option<&StopReceipt> { self.delivery.stop_receipt() }
    pub fn stop_progress(&self) -> Result<StopProgress, Error> { self.delivery.stop_progress() }

    /// Neither helper inputs nor human approval can obstruct existing outcome
    /// obligations. A second-key grant is never reissued while stopping.
    pub fn progress_stop(&mut self, endpoint: &mut PublicationEndpoint) -> Result<StopSweep, Error> {
        self.delivery.progress_stop(endpoint)
    }
}
