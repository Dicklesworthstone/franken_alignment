//! Recovery of the effect DISPATCHER, with the control and endpoint state kept.
//!
//! Interruptions are modeled between public operations, not at disk writes or
//! instructions inside them. The protocol never reconstructs lost authority from
//! an actor checkpoint or claims that these in-memory states survived an OS crash.

use super::super::*;

impl DeliveryBroker {
    /// Advance the dispatcher incarnation, pessimistically classify every
    /// interrupted dispatch, and require a real endpoint fence acknowledgment
    /// before returning new sendable envelopes. This is not a policy change:
    /// already reserved, undispatched work can proceed after fencing and the
    /// normal current-policy/deadline checks. No effect rights are minted.
    pub fn restart_dispatcher(&mut self) -> Result<FenceRequest, Error> {
        let epoch = self.epoch.checked_add(1).ok_or(Error::Overflow)?;
        let inspection = self.inspect();
        let mut interrupted = Vec::new();
        for (id, record) in &self.records {
            let state = inspection.ledger.stages.get(id).copied().ok_or(Error::Missing)?;
            if record.resolution.is_some() {
                if !matches!(state, ActionState::Confirmed | ActionState::ConfirmedNotExecuted) {
                    return Err(Error::WrongState);
                }
            } else {
                match state {
                    ActionState::Dispatching => interrupted.push(*id),
                    ActionState::Unknown | ActionState::IrrecoverablyUnknown => {}
                    _ => return Err(Error::WrongState),
                }
            }
        }
        // Every fallible precondition was checked above against the same owned
        // ledger. No concurrent mutation can occur during this &mut transition.
        for id in interrupted {
            self.controller.mark_unknown(id).expect("validated interrupted dispatch");
        }
        self.epoch = epoch;
        self.fenced = false;
        Ok(self.fence_request())
    }

    /// Enumerate retained outstanding obligations, not fresh effect requests.
    /// Terminal and irrecoverably unknown attempts are not automatically retried.
    pub fn pending_reconciliation(&self) -> Result<Vec<StatusQuery>, Error> {
        let inspection = self.inspect();
        let mut queries = Vec::new();
        for (id, record) in &self.records {
            if record.resolution.is_none()
                && matches!(inspection.ledger.stages.get(id), Some(ActionState::Dispatching | ActionState::Unknown))
            {
                queries.push(self.status_query(*id)?);
            }
        }
        Ok(queries)
    }

    /// Consume a status response for its exact query. An absence or retention
    /// failure can only move an in-flight attempt to Unknown, never release its
    /// charge. A terminal response is accepted through the same receipt checker.
    /// Stale queries cannot contaminate a newer recovery incarnation.
    pub fn reconcile_status(
        &mut self, query: &StatusQuery, status: EndpointStatus,
    ) -> Result<EndpointStatus, Error> {
        if !Rc::ptr_eq(&self.binding, &query.0.binding) { return Err(Error::Binding); }
        if query.0.epoch != self.epoch { return Err(Error::Stale); }
        let record = self.records.get(&query.0.attempt).ok_or(Error::Missing)?;
        if record.action != query.0.action || record.retained_until != query.0.retained_until {
            return Err(Error::Binding);
        }
        match &status {
            EndpointStatus::Resolved(receipt) => {
                if receipt.attempt != query.0.attempt { return Err(Error::Binding); }
                self.accept_receipt(receipt.clone())?;
            }
            EndpointStatus::AwaitingResolution | EndpointStatus::RetentionExpired => {
                if record.resolution.is_some() { return Err(Error::WrongState); }
                self.acknowledgment_lost(query.0.attempt)?;
            }
        }
        Ok(status)
    }
}
