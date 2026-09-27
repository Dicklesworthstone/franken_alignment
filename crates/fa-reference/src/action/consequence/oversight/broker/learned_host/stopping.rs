//! Fixed learned-host supervision delegates to the original stop/fence protocol.
//! No helper judgment, endpoint acknowledgment, reset or refund is invented.
use super::{HostedLearnedInspection, LearnedAvailability, GenerationStatus, OversightBroker};
use super::super::decoder_host::HostedStopPolicy;
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::action::consequence::delivery::{StopReceipt, StopRequest};
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearnedHostStopCause {
    Monitoring(MonitorOutcome),
    /// Admitted numerical/preparation failure, actor-sync failure or interrupted
    /// composition. This is operational uncertainty, not a detected violation.
    Failure(Error),
}

/// The first actual owned trigger and original local stop result. It contains
/// no held candidate, cache or RNG words. Endpoint settlement remains separate.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::learned_host::LearnedHostStopIncident;
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// fn refund(incident: LearnedHostStopIncident) -> EndpointReceipt { incident }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LearnedHostStopIncident {
    policy: HostedStopPolicy,
    cause: LearnedHostStopCause,
    observed: HostedLearnedInspection,
    stream: u64,
    evaluation_origin: u64,
    receipt: Option<StopReceipt>,
    last_stop_error: Option<Error>,
}
impl LearnedHostStopIncident {
    pub fn policy(&self) -> HostedStopPolicy { self.policy }
    pub fn cause(&self) -> LearnedHostStopCause { self.cause }
    pub fn observed(&self) -> &HostedLearnedInspection { &self.observed }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn evaluation_origin(&self) -> u64 { self.evaluation_origin }
    pub fn stop_receipt(&self) -> Option<&StopReceipt> { self.receipt.as_ref() }
    pub fn last_stop_error(&self) -> Option<Error> { self.last_stop_error }
}

#[derive(Debug)]
pub(super) struct LearnedStopState {
    policy: HostedStopPolicy,
    incident: Option<LearnedHostStopIncident>,
}

impl OversightBroker {
    /// Reuse the original fixed supervisor-policy identity, installed before
    /// this owned generation executes its first token. No disable, retuning or
    /// retroactive installation after observing an outcome is provided.
    pub fn enable_learned_host_stop(&mut self, policy: HostedStopPolicy) -> Result<(), Error> {
        if self.inspect().suspended || self.stop_receipt().is_some() || self.inspect().sequence != 0
            || !self.inputs.is_empty() || !self.started_rounds.is_empty() { return Err(Error::WrongState); }
        let host = self.learned_host.as_mut().ok_or(Error::Incomplete)?;
        if host.automatic_stop.is_some() { return Err(Error::Duplicate); }
        if host.fault.is_some() || host.run.position() != 0 || !host.run.status().is_active()
            || host.run.observation().availability() != LearnedAvailability::Empty { return Err(Error::WrongState); }
        host.automatic_stop = Some(LearnedStopState { policy, incident: None });
        Ok(())
    }
    pub fn learned_host_stop_policy(&self) -> Option<HostedStopPolicy> {
        self.learned_host.as_ref()?.automatic_stop.as_ref().map(|state| state.policy)
    }
    pub fn learned_host_stop_incident(&self) -> Option<&LearnedHostStopIncident> {
        self.learned_host.as_ref()?.automatic_stop.as_ref()?.incident.as_ref()
    }

    /// Called before and after owned inference, or explicitly by the supervisor
    /// after a caught unwind. A failed stop is retryable but no more numerical
    /// work can occur. This cannot run while the process is crashed/unscheduled.
    /// It stops local authority; progress_stop separately fences the endpoint and
    /// reconciles real outcomes, preserving already-dispatched resource charges.
    pub fn enforce_learned_host_stop(&mut self) -> Result<Option<StopReceipt>, Error> {
        let Some(host) = self.learned_host.as_ref() else { return Ok(None); };
        let Some(state) = host.automatic_stop.as_ref() else { return Ok(None); };
        if state.incident.is_none() {
            if self.inspect().suspended { return Ok(None); }
            let cause = if let Some(error) = host.fault { LearnedHostStopCause::Failure(error) }
                else { match host.run.status() {
                    GenerationStatus::Held(outcome) if outcome != MonitorOutcome::NoAlarm =>
                        LearnedHostStopCause::Monitoring(outcome),
                    GenerationStatus::Failed(error) => LearnedHostStopCause::Failure(error),
                    GenerationStatus::Held(_) => LearnedHostStopCause::Failure(Error::Binding),
                    _ => {
                        if matches!(host.run.observation().availability(), LearnedAvailability::Empty | LearnedAvailability::Ready) {
                            return Ok(None);
                        }
                        LearnedHostStopCause::Failure(Error::Incomplete)
                    }
                }};
            let source = host.run.observation();
            let incident = LearnedHostStopIncident { policy: state.policy, cause,
                observed: self.hosted_learned_generation()?, stream: source.stream(),
                evaluation_origin: source.evaluation_origin(), receipt: None, last_stop_error: None };
            self.learned_host.as_mut().expect("owned learned source").automatic_stop.as_mut()
                .expect("frozen stop policy").incident = Some(incident);
        }
        // Another completed supervisor stop is sufficient; never replace its
        // operation identity or count a second authority transition.
        let result = match self.stop_receipt() {
            Some(receipt) => Ok(receipt.clone()),
            None => {
                let view = self.inspect();
                let policy = self.learned_host_stop_policy().expect("retained stop policy");
                self.request_stop(StopRequest { operation: policy.operation(),
                    expected_control_sequence: view.sequence, expected_authority_epoch: view.ledger.epoch })
            }
        };
        let incident = self.learned_host.as_mut().expect("owned learned source").automatic_stop.as_mut()
            .expect("retained stop policy").incident.as_mut().expect("retained trigger");
        match result {
            Ok(receipt) => {
                incident.receipt = Some(receipt.clone()); incident.last_stop_error = None;
                Ok(Some(receipt))
            }
            Err(error) => { incident.last_stop_error = Some(error); Err(error) }
        }
    }
}
