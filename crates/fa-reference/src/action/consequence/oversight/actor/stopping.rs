//! Supervising lifecycle transitions without losing the original actor mailbox.

use super::{ActorSupervisor, ActorOutcome, BasisSource, Projection};
use crate::action::consequence::delivery::{PublicationEndpoint, StopProgress, StopReceipt, StopRequest, StopSweep};
use crate::action::consequence::oversight::decoder_host::{HostedCheckpointHandle, HostedResetReceipt, HostedResetRequest};
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

    pub fn capture_hosted_checkpoint(&mut self, id: u64, expected_actor_revision: u64)
        -> Result<HostedCheckpointHandle, Error>
    {
        self.broker.capture_hosted_checkpoint(id, expected_actor_revision)
    }

    /// A restoring reset cancels queued work from the abandoned continuation
    /// without closing fresh intake or evicting its original keys. A suspending
    /// reset closes intake. Accepted/sent work is projected ONLY from the ledger.
    /// There is no ActorPort entrypoint or implicit upgrade of expected epochs.
    pub fn reset_hosted_decoder(&mut self, request: HostedResetRequest) -> Result<HostedResetReceipt, Error> {
        let mut state = self.mailbox.try_borrow_mut().map_err(|_| Error::WrongState)?;
        let result = self.broker.reset_hosted_decoder(request);
        if let Ok(receipt) = &result {
            if receipt.control.restored {
                while let Some(id) = state.queued.pop_front() {
                    state.entries.get_mut(&id).expect("queued actor request retained").project(
                        Projection::Terminal(ActorOutcome::CancelledBeforeDispatch, BasisSource::Intake),
                    );
                }
            } else {
                state.close_intake();
            }
        }
        drop(state);
        self.synchronize()?;
        result
    }
}
