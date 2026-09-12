//! Terminal shutdown and paired hosted reset use distinct original transitions.

use super::SupervisedDriver;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::{StopProgress, StopReceipt, StopRequest, StopSweep};
use crate::action::consequence::oversight::helper_processes::ProcessStatus;
use crate::action::consequence::oversight::decoder_host::{HostedCheckpointHandle, HostedResetReceipt, HostedResetRequest};
use crate::Error;
use std::collections::BTreeMap;

/// Supervisor-only observation. Drained effects do not imply that a direct
/// helper child has exited or that a retained review job has been released.
/// Descendant containment is not provided by this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriverStopProgress {
    pub effects: StopProgress,
    pub review_released: bool,
    pub helpers: BTreeMap<String, ProcessStatus>,
    pub helpers_reaped: bool,
}

impl DriverStopProgress {
    pub fn quiesced(&self) -> bool {
        self.effects.drained() && self.review_released && self.helpers_reaped
    }
}

impl SupervisedDriver {
    /// First stop admission in the original ledger and actor mailbox, then drop
    /// the active review and request direct-child cleanup. No endpoint I/O, wait,
    /// helper retry, replacement permit or inferred nonexecution is hidden here.
    pub fn request_stop(&mut self, request: StopRequest) -> Result<StopReceipt, Error> {
        let result = self.supervisor.request_stop(request);
        // A local stop can precede a later projection error. Never let such an
        // error retain a driving job or leave helper cleanup unrequested.
        if self.supervisor.stop_receipt().is_some() { self.job = None; }
        self.reap_helpers();
        result
    }

    pub fn stop_progress(&self) -> Result<DriverStopProgress, Error> {
        Ok(DriverStopProgress {
            effects: self.supervisor.stop_progress()?, review_released: self.job.is_none(),
            helpers: self.helper_processes(), helpers_reaped: self.helpers_reaped(),
        })
    }

    /// Fresh host time, current endpoint fence, then a bounded seal/reconcile
    /// pass. Errors leave the stop latched and cleanup continues. No stored input
    /// is substituted for a missing observation because no new effect is sent.
    pub fn progress_stop(&mut self, now: ElapsedTick) -> Result<StopSweep, Error> {
        // Also handle a stop initiated through the trusted supervisor/broker.
        // Release its now-cancelled job before fallible clock or endpoint work.
        if self.supervisor.stop_receipt().is_some() { self.job = None; }
        let result = (|| {
            self.supervisor.stop_receipt().ok_or(Error::Incomplete)?;
            self.supervisor.synchronize()?;
            self.observe_time(now)?;
            self.supervisor.progress_stop(&mut self.endpoint)
        })();
        self.reap_helpers();
        result
    }

    pub fn capture_hosted_checkpoint(&mut self, id: u64, expected_actor_revision: u64)
        -> Result<HostedCheckpointHandle, Error>
    {
        self.supervisor.capture_hosted_checkpoint(id, expected_actor_revision)
    }

    /// The numerical/authority reset happens first. Only a successful original
    /// transition releases the active review and retained permit. Cleanup still
    /// recognizes an already cancelled job if a later projection returns error.
    pub fn reset_hosted_decoder(&mut self, request: HostedResetRequest) -> Result<HostedResetReceipt, Error> {
        let result = self.supervisor.reset_hosted_decoder(request);
        if result.is_ok() { self.job = None; }
        self.reap_helpers();
        result
    }
}
