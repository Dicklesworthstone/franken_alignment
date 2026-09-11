//! Dispatcher interruption with control and endpoint state retained.
//! This is not disk recovery or restoration of authority from actor checkpoints.

use super::super::*;

impl DeliveryBroker {
    pub fn restart_dispatcher(&mut self) -> Result<FenceRequest, Error> {
        let epoch = self.epoch.checked_add(1).ok_or(Error::Overflow)?;
        let inspection = self.inspect();
        let mut interrupted = Vec::new();
        for (id, record) in &self.records {
            let state = inspection.ledger.stages.get(id).copied().ok_or(Error::Missing)?;
            if record.resolution.is_some() {
                if !matches!(state, ActionState::Confirmed | ActionState::ConfirmedNotExecuted) { return Err(Error::WrongState); }
            } else {
                match state {
                    ActionState::Dispatching => interrupted.push(*id),
                    ActionState::Unknown | ActionState::IrrecoverablyUnknown => {}
                    _ => return Err(Error::WrongState),
                }
            }
        }
        for id in interrupted { self.controller.mark_unknown(id).expect("validated interrupted dispatch"); }
        self.epoch = epoch;
        self.fenced = false;
        Ok(self.fence_request())
    }

    pub fn pending_reconciliation(&self) -> Result<Vec<StatusQuery>, Error> {
        let inspection = self.inspect();
        let mut queries = Vec::new();
        for (id, record) in &self.records {
            if record.resolution.is_none()
                && matches!(inspection.ledger.stages.get(id), Some(ActionState::Dispatching | ActionState::Unknown))
            { queries.push(self.status_query(*id)?); }
        }
        Ok(queries)
    }

    pub fn reconcile_status(&mut self, query: &StatusQuery, status: EndpointStatus) -> Result<EndpointStatus, Error> {
        if !Rc::ptr_eq(&self.binding, &query.0.binding) { return Err(Error::Binding); }
        if query.0.epoch != self.epoch { return Err(Error::Stale); }
        let record = self.records.get(&query.0.attempt).ok_or(Error::Missing)?;
        if !query.0.request.matches_action(&record.action) || record.retained_until != query.0.retained_until { return Err(Error::Binding); }
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

    /// Only the crate's evaluated-oversight path calls this fenced transition.
    /// Outstanding endpoint obligations do not depend on the new helper weights.
    pub(crate) fn replace_congress_weights(
        &mut self, sequence: u64, epoch: u64,
        next: crate::action::consequence::congress::CongressPolicy,
    ) -> Result<crate::action::consequence::oversight::credibility::CongressChange, Error> {
        self.controller.replace_congress_weights(sequence, epoch, next)
    }

    /// Reuse the original authority's stop transaction for a latched identity
    /// incident. Returns sequence, floor, cancelled IDs and exact refund. This
    /// does NOT fence or settle envelopes already returned to an external caller.
    pub(crate) fn fence_identity(
        &mut self, sequence: u64, epoch: u64,
    ) -> Result<(u64, u64, Vec<u64>, u64), Error> {
        let minimum = epoch.checked_add(1).ok_or(Error::Overflow)?;
        let stopped = self.controller.fence_authority(sequence, epoch, minimum)?;
        Ok((stopped.sequence, stopped.revocation_floor, stopped.cancelled, stopped.refunded_units))
    }
}
