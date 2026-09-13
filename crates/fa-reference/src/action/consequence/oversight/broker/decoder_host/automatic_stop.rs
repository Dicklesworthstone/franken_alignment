//! A fixed supervisor policy maps nonquiet owned inference to the original stop.
//! This is not a helper verdict, an endpoint acknowledgment, or a reset policy.

use super::{OversightBroker, MonitoringStatus, MonitoringWork};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::delivery::{StopReceipt, StopRequest};
use crate::Error;

/// Explicit trusted bootstrap. Every nonquiet review and admitted numerical
/// failure stops this run permanently. There is no actor-selected exception,
/// automatic reroll, budget enlargement, or policy edit after installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostedStopPolicy { id: u64, generation: u64, operation: u64 }
impl HostedStopPolicy {
    pub fn new(id: u64, generation: u64, operation: u64) -> Result<Self, Error> {
        if [id, generation, operation].contains(&0) { return Err(Error::InvalidInput); }
        Ok(Self { id, generation, operation })
    }
    pub fn id(self) -> u64 { self.id }
    pub fn generation(self) -> u64 { self.generation }
    pub fn operation(self) -> u64 { self.operation }
}

/// Operational uncertainty is not relabelled as a detected policy violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostedStopCause { Monitoring(MonitorOutcome), Numerical(Error) }

/// First observed trigger from the owned decoder. No submitted token, raw KV,
/// sampler words or logits are copied into this supervisor-only observation.
/// A local stop receipt never proves an endpoint has stopped or refunded a send.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::decoder_host::HostedStopIncident;
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// fn refund(incident: HostedStopIncident) -> EndpointReceipt { incident }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedStopIncident {
    policy: HostedStopPolicy,
    cause: HostedStopCause,
    actor_revision: u64,
    stream: u64,
    position: u64,
    sampled_draws: u64,
    monitoring: MonitoringWork,
    receipt: Option<StopReceipt>,
    last_stop_error: Option<Error>,
}
impl HostedStopIncident {
    pub fn policy(&self) -> HostedStopPolicy { self.policy }
    pub fn cause(&self) -> HostedStopCause { self.cause }
    pub fn actor_revision(&self) -> u64 { self.actor_revision }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn position(&self) -> u64 { self.position }
    pub fn sampled_draws(&self) -> u64 { self.sampled_draws }
    pub fn monitoring(&self) -> MonitoringWork { self.monitoring }
    pub fn stop_receipt(&self) -> Option<&StopReceipt> { self.receipt.as_ref() }
    pub fn last_stop_error(&self) -> Option<Error> { self.last_stop_error }
}

#[derive(Debug)]
pub(super) struct HostedStopState {
    policy: HostedStopPolicy,
    incident: Option<HostedStopIncident>,
}

impl OversightBroker {
    /// Configure only an already-owned, currently quiet numerical source, before
    /// proposals or reviews. Warm bootstrap is allowed; prior inference is not
    /// retroactively called policy-monitored. Existing manual-reset mode is the
    /// default and is unchanged unless this stricter policy is installed.
    pub fn enable_hosted_stop(&mut self, policy: HostedStopPolicy) -> Result<(), Error> {
        if self.inspect().suspended || self.stop_receipt().is_some() || self.inspect().sequence != 0
            || !self.inputs.is_empty() || !self.started_rounds.is_empty()
        { return Err(Error::WrongState); }
        let host = self.decoder_host.as_mut().ok_or(Error::Incomplete)?;
        if host.automatic_stop.is_some() { return Err(Error::Duplicate); }
        if host.run.status() != MonitoringStatus::Ready { return Err(Error::WrongState); }
        host.automatic_stop = Some(HostedStopState { policy, incident: None });
        Ok(())
    }

    pub fn hosted_stop_policy(&self) -> Option<HostedStopPolicy> {
        self.decoder_host.as_ref()?.automatic_stop.as_ref().map(|state| state.policy)
    }
    pub fn hosted_stop_incident(&self) -> Option<&HostedStopIncident> {
        self.decoder_host.as_ref()?.automatic_stop.as_ref()?.incident.as_ref()
    }

    /// Idempotent supervision boundary, with no caller-asserted cause. Ordinary
    /// hosted inference invokes this before and after the numerical operation.
    /// It also services a poisoned owner after a caller catches an unwind. This
    /// method cannot run while the host is not being scheduled or has crashed.
    /// Failure is retained and may be retried; it never reopens decoder evidence.
    /// Endpoint fencing and actual nonexecution proof remain separate operations.
    pub fn enforce_hosted_stop(&mut self) -> Result<Option<StopReceipt>, Error> {
        let Some(host) = self.decoder_host.as_ref() else { return Ok(None); };
        let Some(state) = host.automatic_stop.as_ref() else { return Ok(None); };
        if state.incident.is_none() {
            // A completed manual suspension (including incident escalation on
            // reset) is not a new numerical failure to be relabelled here.
            if self.inspect().suspended { return Ok(None); }
            let cause = match host.run.status() {
                MonitoringStatus::Ready => return Ok(None),
                MonitoringStatus::Failed(error) => HostedStopCause::Numerical(error),
                MonitoringStatus::Held => match host.run.last_review().map(|report| report.outcome()) {
                    Some(outcome) if outcome != MonitorOutcome::NoAlarm => HostedStopCause::Monitoring(outcome),
                    _ => HostedStopCause::Numerical(Error::Incomplete),
                },
            };
            let incident = HostedStopIncident {
                policy: state.policy, cause, actor_revision: self.actor_revision(),
                stream: host.run.observation().stream(), position: host.run.position(),
                sampled_draws: host.run.sampled_draws(), monitoring: host.run.monitoring_work(),
                receipt: None, last_stop_error: None,
            };
            self.decoder_host.as_mut().expect("owned numerical source").automatic_stop.as_mut()
                .expect("frozen stop policy").incident = Some(incident);
        }
        // A stop already requested by another trusted path is sufficient; do not
        // replace its operation identity or claim a second authority transition.
        let result = match self.stop_receipt() {
            Some(receipt) => Ok(receipt.clone()),
            None => {
                let inspection = self.inspect();
                let operation = self.hosted_stop_policy().expect("retained stop policy").operation;
                self.request_stop(StopRequest { operation, expected_control_sequence: inspection.sequence,
                    expected_authority_epoch: inspection.ledger.epoch })
            }
        };
        let incident = self.decoder_host.as_mut().expect("owned numerical source").automatic_stop.as_mut()
            .expect("retained stop policy").incident.as_mut().expect("retained trigger");
        match result {
            Ok(receipt) => {
                incident.receipt = Some(receipt.clone()); incident.last_stop_error = None;
                Ok(Some(receipt))
            }
            Err(error) => { incident.last_stop_error = Some(error); Err(error) }
        }
    }

    pub(super) fn with_hosted_stop<T>(
        &mut self, operation: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if self.enforce_hosted_stop()?.is_some() { return Err(Error::WrongState); }
        let result = operation(self);
        self.enforce_hosted_stop()?;
        result
    }
}
