//! Stop admission, then fence and settle the original endpoint obligations.
//! These are distinct transitions. A local stop is never a remote stop receipt.

use super::{DeliveryBroker, EndpointStatus, PublicationEndpoint};
use crate::action::ActionState;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Trusted supervisor operation, not an actor request or a resumable lease.
/// An exact retry returns the original receipt even after its predecessor moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopRequest {
    pub operation: u64,
    pub expected_control_sequence: u64,
    pub expected_authority_epoch: u64,
}

/// Evidence that THIS controller stopped admission. This contains no endpoint
/// acknowledgment and cannot be substituted for a receipt or an effect permit.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::{EndpointReceipt, StopReceipt};
/// fn refund(stop: StopReceipt) -> EndpointReceipt { stop }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopReceipt {
    request: StopRequest,
    control_sequence: u64,
    revocation_floor: u64,
    dispatcher_epoch: u64,
    cancelled: Vec<u64>,
    refunded_units: u64,
}

impl StopReceipt {
    pub fn request(&self) -> StopRequest { self.request }
    pub fn control_sequence(&self) -> u64 { self.control_sequence }
    pub fn revocation_floor(&self) -> u64 { self.revocation_floor }
    pub fn dispatcher_epoch(&self) -> u64 { self.dispatcher_epoch }
    pub fn cancelled(&self) -> &[u64] { &self.cancelled }
    pub fn refunded_units(&self) -> u64 { self.refunded_units }
}

/// Supervisor-only snapshot of stop progress, not a live capability. Executed
/// effects may remain charged after draining; unresolved charges cannot vanish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopProgress {
    pub receipt: StopReceipt,
    pub dispatcher_epoch: u64,
    pub endpoint_fenced: bool,
    pub unresolved: Vec<u64>,
    pub irrecoverable: Vec<u64>,
    pub reserved_units: u64,
    pub charged_units: u64,
}

impl StopProgress {
    /// True only at this observation, after a current-epoch acknowledgment and
    /// terminal endpoint evidence for every recorded dispatch. This is not a
    /// statement that already executed external effects were reversed.
    pub fn drained(&self) -> bool {
        self.endpoint_fenced && self.unresolved.is_empty() && self.reserved_units == 0
    }
}

/// A sweep is not an all-or-nothing endpoint transaction. Inspect every result;
/// one refused obligation does not erase progress made on a different one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopSweep {
    pub outcomes: BTreeMap<u64, Result<EndpointStatus, Error>>,
    pub progress: StopProgress,
}

impl DeliveryBroker {
    /// Close this authority domain permanently, refund only undispatched
    /// reservations, and require a NEW dispatcher fence. No endpoint I/O occurs.
    /// The original controller performs its own atomic rights/epoch transition.
    pub fn request_stop(&mut self, request: StopRequest) -> Result<StopReceipt, Error> {
        if request.operation == 0 { return Err(Error::InvalidInput); }
        if let Some(receipt) = &self.stop {
            return if receipt.request == request {
                Ok(receipt.clone())
            } else if receipt.request.operation == request.operation {
                Err(Error::Binding)
            } else {
                Err(Error::Duplicate)
            };
        }
        let inspection = self.inspect();
        if inspection.sequence != request.expected_control_sequence
            || inspection.ledger.epoch != request.expected_authority_epoch
        {
            return Err(Error::Stale);
        }
        // All delivery-side validation precedes the controller's stop. Its stop
        // leaves dispatched/unknown dispositions unchanged, so the following
        // mark_unknown operations cannot fail after successful preflight.
        let epoch = self.epoch.checked_add(1).ok_or(Error::Overflow)?;
        let minimum = request.expected_authority_epoch.checked_add(1).ok_or(Error::Overflow)?;
        let mut interrupted = Vec::new();
        for (id, record) in &self.records {
            let stage = inspection.ledger.stages.get(id).ok_or(Error::Missing)?;
            if record.resolution.is_some() {
                if !matches!(stage, ActionState::Confirmed | ActionState::ConfirmedNotExecuted) {
                    return Err(Error::WrongState);
                }
            } else {
                match stage {
                    ActionState::Dispatching => interrupted.push(*id),
                    ActionState::Unknown | ActionState::IrrecoverablyUnknown => {}
                    _ => return Err(Error::WrongState),
                }
            }
        }
        let fenced = self.controller.fence_authority(
            request.expected_control_sequence, request.expected_authority_epoch, minimum,
        )?;
        for id in interrupted {
            self.controller.mark_unknown(id).expect("prevalidated interrupted dispatch");
        }
        self.epoch = epoch;
        self.fenced = false;
        let receipt = StopReceipt {
            request, control_sequence: fenced.sequence, revocation_floor: fenced.revocation_floor,
            dispatcher_epoch: epoch, cancelled: fenced.cancelled, refunded_units: fenced.refunded_units,
        };
        self.stop = Some(receipt.clone());
        Ok(receipt)
    }

    pub fn stop_receipt(&self) -> Option<&StopReceipt> { self.stop.as_ref() }

    pub fn stop_progress(&self) -> Result<StopProgress, Error> {
        let receipt = self.stop.as_ref().ok_or(Error::Incomplete)?.clone();
        let inspection = self.inspect();
        let mut unresolved = Vec::new();
        let mut irrecoverable = Vec::new();
        for (id, record) in &self.records {
            if record.resolution.is_none() {
                unresolved.push(*id);
                if inspection.ledger.stages.get(id) == Some(&ActionState::IrrecoverablyUnknown) {
                    irrecoverable.push(*id);
                }
            }
        }
        Ok(StopProgress {
            receipt, dispatcher_epoch: self.epoch, endpoint_fenced: self.fenced,
            unresolved, irrecoverable, reserved_units: inspection.ledger.reserved,
            charged_units: inspection.ledger.charged,
        })
    }

    /// Install and acknowledge the CURRENT fence before resolving anything.
    /// Then seal every still-recoverable missing request, even before its original
    /// deadline: stopping explicitly withdraws further execution. Executed
    /// receipts win; expired retention and irrecoverable outcomes stay unresolved.
    /// Storage errors return without reopening admission or resending envelopes.
    /// The host must supply the endpoint's current trusted clock separately.
    pub fn progress_stop(&mut self, endpoint: &mut PublicationEndpoint) -> Result<StopSweep, Error> {
        self.stop.as_ref().ok_or(Error::Incomplete)?;
        if !Rc::ptr_eq(&self.binding, &endpoint.binding) { return Err(Error::Binding); }
        self.fenced = false;
        let acknowledgment = endpoint.install_fence(self.fence_request())?;
        self.confirm_fence(acknowledgment)?;
        let mut outcomes = BTreeMap::new();
        for query in self.pending_reconciliation()? {
            let status = endpoint.status(&query).and_then(|status| match status {
                EndpointStatus::AwaitingResolution => {
                    endpoint.seal_unexecuted(&query).map(EndpointStatus::Resolved)
                }
                terminal => Ok(terminal),
            });
            let outcome = match status {
                Ok(status) => self.reconcile_status(&query, status),
                // Sent requests already became Unknown in request_stop. No
                // failure here can change that disposition or release a charge.
                Err(error) => Err(error),
            };
            outcomes.insert(query.attempt(), outcome);
        }
        Ok(StopSweep { outcomes, progress: self.stop_progress()? })
    }

    pub(super) fn check_not_stopping(&self) -> Result<(), Error> {
        if self.stop.is_some() { Err(Error::WrongState) } else { Ok(()) }
    }
}
