//! Persist the original held-out qualification, not another score/rights ledger.
use super::super::{Machine, Transition};
use super::super::super::{FileOversight, JournalError, JournalFailure, JournalIo};
use super::super::super::credibility::CredibilityEvent;
use super::super::super::journal::Event;
use crate::action::consequence::delivery::persistent::credibility::{
    CredibilityActivation, CredibilityChange, CredibilityWithdrawal, CredibilityWithdrawalRequest,
};
use crate::action::consequence::oversight::CommitteeContract;
use crate::Error;

impl Machine {
    pub(super) fn activate_held_out(&mut self, request: &CredibilityActivation) -> Result<Transition, Error> {
        if self.broker.credibility_changes().any(|receipt| receipt.operation == request.operation) {
            return Err(Error::Duplicate);
        }
        if !self.publication_guard { return Err(Error::Incomplete); }
        let contracts = self.broker.contracts().clone();
        self.broker.activate_credibility(request.clone(), &contracts)?;
        self.finish_held_out_transition()?;
        Ok(Transition::Unit)
    }

    pub(super) fn withdraw_held_out(&mut self, request: &CredibilityWithdrawalRequest) -> Result<Transition, Error> {
        if self.broker.credibility_withdrawals().any(|receipt| receipt.request.operation == request.operation) {
            return Err(Error::Duplicate);
        }
        self.broker.withdraw_credibility(request.clone())?;
        self.finish_held_out_transition()?;
        Ok(Transition::Unit)
    }

    fn finish_held_out_transition(&mut self) -> Result<(), Error> {
        // This candidate and its endpoint exist only in RAM. All effects of the
        // original transitions are acknowledged in ONE canonical replacement.
        self.withdraw_keys()?;
        self.withdraw_identity()?;
        self.withdraw_policy_campaigns()?;
        self.broker.confirm_fence(self.endpoint.install_fence(self.broker.fence_request())?)?;
        self.clear_sendable();
        // No time observation, source capture or receipt settlement is invented.
        Ok(())
    }
}

impl FileOversight {
    /// Independently supplied complete committee binding plus the original
    /// native activation. The journal's bootstrap already retains those exact
    /// contracts; replay cannot substitute an alternate helper profile.
    pub fn activate_credibility(&mut self, revision: u64, request: CredibilityActivation,
        expected_contracts: &CommitteeContract) -> Result<CredibilityChange, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if expected_contracts != &self.profile.committee { return Err(Error::Binding.into()); }
        if let Some(previous) = self.events.iter().find_map(|event| match event {
            Event::Credibility(CredibilityEvent::ActivateHeldOut(previous))
                if previous.operation == request.operation => Some(previous.as_ref()),
            _ => None,
        }) {
            if previous != &request { return Err(Error::Binding.into()); }
            return self.credibility_change(request.operation).cloned();
        }
        let operation = request.operation;
        self.transact(revision, Event::Credibility(CredibilityEvent::ActivateHeldOut(Box::new(request))))?;
        self.credibility_change(operation).cloned()
    }

    /// A valid notification of loss cannot leave the live owner using an older
    /// qualification when encoding, capacity or replacement fails. Exact invalid
    /// caller/predecessor/operation requests refuse before poisoning the owner.
    pub fn withdraw_credibility(&mut self, revision: u64, request: CredibilityWithdrawalRequest)
        -> Result<CredibilityWithdrawal, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(previous) = self.events.iter().find_map(|event| match event {
            Event::Credibility(CredibilityEvent::WithdrawHeldOut(previous))
                if previous.operation == request.operation => Some(previous),
            _ => None,
        }) {
            if previous != &request { return Err(Error::Binding.into()); }
            return self.credibility_withdrawal(request.operation).cloned();
        }
        if request.operation == 0 { return Err(Error::InvalidInput.into()); }
        let inspection = self.machine.broker.inspect();
        if revision != self.revision() || request.expected_control_sequence != inspection.sequence
            || request.expected_epoch != inspection.ledger.epoch { return Err(Error::Stale.into()); }
        if self.machine.broker.stop_receipt().is_some() { return Err(Error::WrongState.into()); }
        let active = self.machine.broker.credibility_changes().last().ok_or(Error::Incomplete)?;
        if self.machine.broker.credibility_withdrawals().any(|loss| loss.sequence > active.sequence) {
            return Err(Error::WrongState.into());
        }
        let operation = request.operation;
        if let Err(error) = self.transact(revision, Event::Credibility(CredibilityEvent::WithdrawHeldOut(request))) {
            if self.fault.is_none() {
                self.fault = Some(JournalFailure { operation: JournalIo::Stage,
                    kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
            }
            return Err(error);
        }
        self.credibility_withdrawal(operation).cloned()
    }

    /// Historical acknowledged native transition, not a current permit.
    pub fn credibility_change(&self, operation: u64) -> Result<&CredibilityChange, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.credibility_changes().find(|receipt| receipt.operation == operation)
            .ok_or_else(|| Error::Missing.into())
    }

    pub fn credibility_withdrawal(&self, operation: u64) -> Result<&CredibilityWithdrawal, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.credibility_withdrawals().find(|receipt| receipt.request.operation == operation)
            .ok_or_else(|| Error::Missing.into())
    }

    pub fn check_credibility(&self) -> Result<(), JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.check_credibility().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
