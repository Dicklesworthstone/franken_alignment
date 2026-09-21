//! Durable offline credibility through the original broker and authority.
//! The journal retains inputs; receipts and balances are replayed, never imported.
//! This is an operator-controlled file profile, not authenticated evaluator data.

pub(super) mod codec;
#[cfg(test)]
mod tests;

pub use crate::action::consequence::gate::containment::session::policy::controller::credibility::{
    CredibilityActivation, CredibilityChange, CredibilityWithdrawal, CredibilityWithdrawalRequest,
};
use super::{Event, FileDelivery, JournalError, Machine, Transition};
use crate::Error;

impl Machine {
    pub(super) fn apply_credibility_activation(&mut self, request: &CredibilityActivation) -> Result<Transition, Error> {
        // Exact retries are served outside the append path. Duplicate journal
        // frames must not masquerade as new qualification or spend history slots.
        if self.broker.controller().credibility_changes().any(|change| change.operation == request.operation) {
            return Err(Error::Duplicate);
        }
        let change = self.broker.activate_credibility(request.clone())?;
        self.confirm_credibility_fence()?;
        Ok(Transition::CredibilityActivated(change))
    }

    pub(super) fn apply_credibility_withdrawal(&mut self, request: &CredibilityWithdrawalRequest) -> Result<Transition, Error> {
        if self.broker.controller().credibility_withdrawals().any(|change| change.request.operation == request.operation) {
            return Err(Error::Duplicate);
        }
        let change = self.broker.withdraw_credibility(request.clone())?;
        self.confirm_credibility_fence()?;
        Ok(Transition::CredibilityWithdrawn(change))
    }

    fn confirm_credibility_fence(&mut self) -> Result<(), Error> {
        // Both reducers are RAM-only inside the candidate. The existing single
        // canonical replacement publishes qualification, rights and endpoint
        // fence together. A failed replacement returns no candidate receipt.
        self.broker.confirm_fence(self.endpoint.install_fence(self.broker.fence_request())?)?;
        self.permits.clear();
        self.envelopes.clear(); // Old deliveries remain queryable, never resendable.
        // Do not manufacture a fresh clock after recovery or qualification.
        Ok(())
    }
}

impl FileDelivery {
    /// Explicit privileged activation, never an actor proposal. Exact healthy
    /// retries return the old receipt without appending, freshening evidence or
    /// reinstating a withdrawn generation, even after reopening the journal.
    pub fn activate_credibility(&mut self, revision: u64, request: CredibilityActivation)
        -> Result<CredibilityChange, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(previous) = self.events.iter().find_map(|event| match event {
            Event::ActivateCredibility(previous) if previous.operation == request.operation => Some(previous.as_ref()),
            _ => None,
        }) {
            if previous != &request { return Err(Error::Binding.into()); }
            return self.credibility_change(request.operation).cloned();
        }
        match self.transact(revision, Event::ActivateCredibility(Box::new(request)))? {
            Transition::CredibilityActivated(change) => Ok(change),
            _ => unreachable!("credibility activation transition"),
        }
    }

    /// Evidence loss is durable and uses the original restrictive transition.
    /// No missing status or withdrawal can establish endpoint nonexecution.
    pub fn withdraw_credibility(&mut self, revision: u64, request: CredibilityWithdrawalRequest)
        -> Result<CredibilityWithdrawal, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(previous) = self.events.iter().find_map(|event| match event {
            Event::WithdrawCredibility(previous) if previous.operation == request.operation => Some(previous),
            _ => None,
        }) {
            if previous != &request { return Err(Error::Binding.into()); }
            return self.credibility_withdrawal(request.operation).cloned();
        }
        match self.transact(revision, Event::WithdrawCredibility(request))? {
            Transition::CredibilityWithdrawn(change) => Ok(change),
            _ => unreachable!("credibility withdrawal transition"),
        }
    }

    /// Historical native receipt, not necessarily the currently active policy.
    pub fn credibility_change(&self, operation: u64) -> Result<&CredibilityChange, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.controller().credibility_changes()
            .find(|change| change.operation == operation).ok_or_else(|| Error::Missing.into())
    }

    pub fn credibility_withdrawal(&self, operation: u64) -> Result<&CredibilityWithdrawal, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.controller().credibility_withdrawals()
            .find(|change| change.request.operation == operation).ok_or_else(|| Error::Missing.into())
    }

    /// Checks current qualification only; Ok is neither a clock nor a permit.
    /// A never-configured legacy profile retains its original semantics.
    pub fn check_credibility(&self) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.controller().check_credibility().map_err(Into::into)
    }
}

/// Decoder/encoder resource accounting, not another authority ledger. This
/// prevents an archive from accumulating many individually bounded snapshots
/// before native replay gets a chance to enforce its lifetime evidence bound.
#[derive(Default)]
pub(super) struct HistoryBudget { activations: usize, observations: u64 }
impl HistoryBudget {
    pub(super) fn record(&mut self, event: &Event) -> Result<(), Error> {
        if let Event::ActivateCredibility(request) = event {
            self.record_activation(request)?;
        }
        Ok(())
    }

    /// The full-input journal shares this same aggregate evidence allowance.
    pub(super) fn record_activation(&mut self, request: &CredibilityActivation) -> Result<(), Error> {
        use crate::action::consequence::gate::containment::session::policy::controller::credibility::{
            MAX_CREDIBILITY_ACTIVATIONS, MAX_CREDIBILITY_OBSERVATIONS,
        };
        let count = request.snapshot.case_specs().len().checked_mul(request.snapshot.helpers().len())
            .ok_or(Error::Limit)?;
        let observations = self.observations.checked_add(u64::try_from(count).map_err(|_| Error::Limit)?)
            .ok_or(Error::Limit)?;
        if self.activations >= MAX_CREDIBILITY_ACTIVATIONS || observations > MAX_CREDIBILITY_OBSERVATIONS {
            return Err(Error::Limit);
        }
        self.activations += 1;
        self.observations = observations;
        Ok(())
    }
}
