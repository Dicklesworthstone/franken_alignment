//! Endpoint interruption without losing the original supervisor, keys or jobs.
//! Reconnection is fenced; recovery does not rerun a helper or resend an effect.

use super::{Job, SupervisedDriver};
use super::super::helper_processes::{HelperChildren, ProcessStatus};
use crate::action::consequence::delivery::{EndpointReceipt, PublicationEndpoint};
use crate::action::consequence::oversight::{ReconciliationResults, actor::ActorSupervisor};
use crate::action::consequence::oversight::decoder_host::{HostedCheckpointHandle, HostedResetReceipt, HostedResetRequest};
use crate::action::ElapsedTick;
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;

/// The original controller while its endpoint is absent. This owns, rather than
/// copies, the outstanding permits and job. It has no driving/dispatch method.
/// The actor's existing port and its lifetime idempotency domain remain intact.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::supervised::OfflineDriver;
/// fn send(mut offline: OfflineDriver) { offline.step(); }
/// ```
pub struct OfflineDriver {
    supervisor: ActorSupervisor,
    job: Option<Job>,
    children: Option<HelperChildren>,
}

/// On failure both owners are returned for inspection or a fresh-clock retry.
/// Progress such as a raised fence or observed clock is never rolled back.
pub struct ReconnectFailure {
    pub error: Error,
    pub offline: OfflineDriver,
    pub endpoint: PublicationEndpoint,
}

impl fmt::Debug for ReconnectFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("ReconnectFailure").field("error", &self.error).finish_non_exhaustive()
    }
}

impl SupervisedDriver {
    /// Transfer, do not clone, the complete driver state. Dropping the returned
    /// file endpoint releases its existing lock so its retained recovery key can
    /// reopen it. Detachment alone is NOT an external fence or nonexecution proof.
    pub fn detach_endpoint(self) -> (OfflineDriver, PublicationEndpoint) {
        let Self { supervisor, endpoint, job, children } = self;
        (OfflineDriver { supervisor, job, children }, endpoint)
    }

    /// Existing obligations are processed without helper input or human keys,
    /// even while another job is being reviewed or awaiting co-signature. Inspect
    /// each per-attempt result; a partial sweep is not an all-or-nothing operation.
    pub fn reconcile_pending(&mut self, now: ElapsedTick) -> Result<ReconciliationResults, Error> {
        let result = (|| {
            self.observe_time(now)?;
            self.supervisor.reconcile_pending(&mut self.endpoint)
        })();
        self.reap_helpers();
        result
    }

    pub fn accept_receipt(&mut self, receipt: EndpointReceipt) -> Result<bool, Error> {
        let result = self.supervisor.accept_receipt(receipt);
        self.reap_helpers();
        result
    }
}

impl OfflineDriver {
    pub fn supervisor(&self) -> &ActorSupervisor { &self.supervisor }

    /// Close the original controller and mailbox while disconnected. This
    /// requests child cleanup but does not claim to have fenced the endpoint.
    pub fn request_stop(
        &mut self, request: crate::action::consequence::delivery::StopRequest,
    ) -> Result<crate::action::consequence::delivery::StopReceipt, Error> {
        let result = self.supervisor.request_stop(request);
        if self.supervisor.stop_receipt().is_some() { self.job = None; }
        self.reap_helpers();
        result
    }

    pub fn stop_receipt(&self) -> Option<&crate::action::consequence::delivery::StopReceipt> {
        self.supervisor.stop_receipt()
    }

    pub fn capture_hosted_checkpoint(&mut self, id: u64, expected_actor_revision: u64)
        -> Result<HostedCheckpointHandle, Error>
    {
        self.supervisor.capture_hosted_checkpoint(id, expected_actor_revision)
    }

    /// Reset the ORIGINAL numerical/control owner while disconnected. This
    /// does not establish an endpoint fence or settle a remote effect. Reconnect
    /// later moves the same checkpoint map, mailbox and outstanding obligations.
    pub fn reset_hosted_decoder(&mut self, request: HostedResetRequest) -> Result<HostedResetReceipt, Error> {
        let result = self.supervisor.reset_hosted_decoder(request);
        if result.is_ok() { self.job = None; }
        self.reap_helpers();
        result
    }

    pub fn helper_processes(&self) -> BTreeMap<String, ProcessStatus> {
        self.children.as_ref().map_or_else(BTreeMap::new, HelperChildren::statuses)
    }
    pub fn helpers_reaped(&self) -> bool {
        self.children.as_ref().is_none_or(HelperChildren::all_reaped)
    }
    /// Stop children belonging to a lower-level reset/cancelled job even when
    /// reconnection refuses, without discarding the pending driver outcome event.
    pub fn reap_helpers(&mut self) -> BTreeMap<String, ProcessStatus> {
        super::processes::maintain_owned(&self.supervisor, &mut self.children, &mut self.job)
    }
    pub fn stop_helper_processes(&mut self) -> BTreeMap<String, ProcessStatus> {
        self.children.as_mut().map_or_else(BTreeMap::new, HelperChildren::request_stop_all)
    }

    /// A real late receipt remains usable while no transport is connected.
    pub fn accept_receipt(&mut self, receipt: EndpointReceipt) -> Result<bool, Error> {
        let result = self.supervisor.accept_receipt(receipt);
        self.reap_helpers();
        result
    }

    pub fn cancel_active(&mut self) -> Result<(), Error> {
        let job = self.job.as_ref().ok_or(Error::Missing)?;
        self.supervisor.broker_mut().cancel(job.attempt)?;
        self.supervisor.synchronize()?;
        self.job = None;
        self.reap_helpers();
        Ok(())
    }

    /// The original endpoint brand is checked with the existing fence protocol,
    /// not equality of numeric resource IDs. Recovered files first need a fresh
    /// clock confirmation. Consequently a refused foreign candidate can observe
    /// this clock, but cannot change the original controller's state.
    ///
    /// Once identity is established, restart the original dispatcher, publish
    /// its Unknown projections, and require the new endpoint fence acknowledgment
    /// before returning a sending driver. This does not settle pending effects.
    pub fn reconnect(
        mut self, mut endpoint: PublicationEndpoint, now: ElapsedTick,
    ) -> Result<SupervisedDriver, ReconnectFailure> {
        self.reap_helpers();
        let result = (|| -> Result<(), Error> {
            endpoint.observe_time(now)?;
            endpoint.install_fence(self.supervisor.broker().fence_request())?;
            self.supervisor.broker_mut().observe_time(now)?;
            let fence = self.supervisor.broker_mut().restart_dispatcher()?;
            self.supervisor.synchronize()?;
            let acknowledgment = endpoint.install_fence(fence)?;
            self.supervisor.broker_mut().confirm_fence(acknowledgment)
        })();
        match result {
            Ok(()) => Ok(SupervisedDriver { supervisor: self.supervisor, endpoint, job: self.job, children: self.children }),
            Err(error) => Err(ReconnectFailure { error, offline: self, endpoint }),
        }
    }
}
