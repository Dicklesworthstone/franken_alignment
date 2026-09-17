//! An opt-in terminal response to the ORIGINAL lifetime consistency process.
//! Surprise and missing coverage are distinct observations, never intent labels.
use super::{ConsistencyEventDomain, OversightBroker};
use crate::action::consequence::delivery::{StopReceipt, StopRequest};
use crate::Error;

/// Supervisor bootstrap, fixed before the first forecast or proposal. Both an
/// actual threshold crossing and recorded lost coverage permanently stop this
/// domain. The default without this policy remains the existing effect hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsistencyStopPolicy { id: u64, generation: u64, operation: u64 }
impl ConsistencyStopPolicy {
    pub fn new(id: u64, generation: u64, operation: u64) -> Result<Self, Error> {
        if [id, generation, operation].contains(&0) { return Err(Error::InvalidInput); }
        Ok(Self { id, generation, operation })
    }
    pub fn id(self) -> u64 { self.id }
    pub fn generation(self) -> u64 { self.generation }
    pub fn operation(self) -> u64 { self.operation }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsistencyStopCause {
    ThresholdCrossed { first_sample: usize },
    CoverageLost,
}

/// First actual trigger and the original local-stop result. A receipt from a
/// different, already-stopped path keeps ITS operation identity. No endpoint
/// settlement, restored key, or claim of deceptive intent follows from this data.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::consistency::ConsistencyStopIncident;
/// use fa_reference::action::consequence::delivery::EndpointReceipt;
/// fn refund(incident: ConsistencyStopIncident) -> EndpointReceipt { incident }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyStopIncident {
    pub policy: ConsistencyStopPolicy,
    pub cause: ConsistencyStopCause,
    pub event_domain: ConsistencyEventDomain,
    pub actor_revision: u64,
    pub observed_samples: usize,
    pub prediction_jobs: usize,
    pub pending_attempt: Option<u64>,
    pub coverage_lost: bool,
    pub receipt: Option<StopReceipt>,
    pub last_stop_error: Option<Error>,
}

#[derive(Debug)]
pub(super) struct ConsistencyStopState {
    policy: ConsistencyStopPolicy,
    incident: Option<ConsistencyStopIncident>,
}

impl OversightBroker {
    pub fn enable_consistency_stop(&mut self, policy: ConsistencyStopPolicy) -> Result<(), Error> {
        if self.stop_receipt().is_some() || self.inspect().suspended || self.inspect().sequence != 0
            || !self.inputs.is_empty() || !self.started_rounds.is_empty() { return Err(Error::WrongState); }
        let state = self.consistency.as_mut().ok_or(Error::Incomplete)?;
        if state.automatic_stop.is_some() { return Err(Error::Duplicate); }
        if state.jobs != 0 || state.pending.is_some() || !state.observations.is_empty()
            || state.coverage_lost || state.evidence.samples() != 0 { return Err(Error::WrongState); }
        state.automatic_stop = Some(ConsistencyStopState { policy, incident: None });
        Ok(())
    }
    pub fn consistency_stop_policy(&self) -> Option<ConsistencyStopPolicy> {
        self.consistency.as_ref()?.automatic_stop.as_ref().map(|state| state.policy)
    }
    pub fn consistency_stop_incident(&self) -> Option<&ConsistencyStopIncident> {
        self.consistency.as_ref()?.automatic_stop.as_ref()?.incident.as_ref()
    }

    /// Derive a trigger solely from the original process, not caller assertions.
    /// Refusals before observation do not become triggers. This is synchronous;
    /// it cannot run while the process is crashed, blocked or unscheduled.
    /// A failed stop retains the incident and can be retried, never re-armed.
    pub fn enforce_consistency_stop(&mut self) -> Result<Option<StopReceipt>, Error> {
        let Some(state) = self.consistency.as_ref() else { return Ok(None); };
        let Some(stopping) = &state.automatic_stop else { return Ok(None); };
        if stopping.incident.is_none() {
            let cause = match state.evidence.first_crossing() {
                Some(first_sample) => ConsistencyStopCause::ThresholdCrossed { first_sample },
                None if state.coverage_lost => ConsistencyStopCause::CoverageLost,
                None => return Ok(None),
            };
            let incident = ConsistencyStopIncident {
                policy: stopping.policy, cause, event_domain: state.event_domain,
                actor_revision: self.actor_revision(), observed_samples: state.evidence.samples(),
                prediction_jobs: state.jobs, pending_attempt: state.pending.as_ref().map(|p| p.attempt),
                coverage_lost: state.coverage_lost, receipt: None, last_stop_error: None,
            };
            self.consistency.as_mut().expect("configured consistency").automatic_stop.as_mut()
                .expect("configured stop policy").incident = Some(incident);
        }
        let result = match self.stop_receipt() {
            Some(receipt) => Ok(receipt.clone()),
            None => {
                let control = self.inspect();
                let operation = self.consistency_stop_policy().expect("configured stop policy").operation;
                self.request_stop(StopRequest { operation, expected_control_sequence: control.sequence,
                    expected_authority_epoch: control.ledger.epoch })
            }
        };
        let incident = self.consistency.as_mut().expect("configured consistency").automatic_stop.as_mut()
            .expect("configured stop policy").incident.as_mut().expect("retained trigger");
        match result {
            Ok(receipt) => {
                incident.receipt = Some(receipt.clone()); incident.last_stop_error = None;
                Ok(Some(receipt))
            }
            Err(error) => { incident.last_stop_error = Some(error); Err(error) }
        }
    }

    pub(super) fn with_consistency_stop<T>(&mut self,
        observe: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error>
    {
        if self.enforce_consistency_stop()?.is_some() { return Err(Error::WrongState); }
        let observed = observe(self);
        let stopped = self.enforce_consistency_stop();
        // Preserve the original observation failure. The independent containment
        // error remains in the incident, rather than replacing or erasing it.
        match observed {
            Err(error) => Err(error),
            Ok(value) => { stopped?; Ok(value) }
        }
    }
}

#[cfg(test)]
mod tests;
