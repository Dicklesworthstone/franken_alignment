//! Offline held-out qualification through the ORIGINAL full-input broker.
//!
//! The caller explicitly associates the evidence generations with the complete
//! immutable committee contract. Equality is checked, not inferred from numeric
//! model/profile IDs. This remains a trusted governance/evaluator assertion.
//! Owned-round (including joint-replay) promotion is a separate, exclusive lane:
//! an offline campaign must never bypass its configured evidence requirements.

use super::OversightBroker;
use crate::action::consequence::gate::containment::session::policy::controller::credibility::{
    CredibilityActivation, CredibilityChange, CredibilityWithdrawal, CredibilityWithdrawalRequest,
};
use crate::action::consequence::oversight::CommitteeContract;
use crate::Error;
use crate::action::consequence::gate::containment::session::policy::controller::credibility::joint::{
    HeldOutJointPolicy, HeldOutJointReport,
};

impl OversightBroker {
    /// Select the immutable held-out joint guard before ANY empirical work.
    /// Owned-round evaluation is mutually exclusive, including before labels.
    pub fn enable_held_out_joint(&mut self, policy: HeldOutJointPolicy) -> Result<(), Error> {
        if self.credibility.is_some() { return Err(Error::Binding); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.stop_receipt().is_some() {
            return Err(Error::WrongState);
        }
        self.delivery.enable_held_out_joint(policy)
    }

    pub fn held_out_joint_policy(&self) -> Option<HeldOutJointPolicy> {
        self.delivery.controller().held_out_joint_policy()
    }

    pub fn held_out_joint_report(&self, operation: u64) -> Result<Option<&HeldOutJointReport>, Error> {
        self.delivery.controller().held_out_joint_report(operation)
    }

    /// Activate the original held-out promotion in ordinary bound review,
    /// authorization and both one-key/two-key dispatch paths. No helper input,
    /// human role, exact policy or approval threshold is replaced by this call.
    /// The complete expected contract is an independently supplied deployment
    /// binding; supplying only matching member names is not sufficient.
    pub fn activate_credibility(
        &mut self,
        request: CredibilityActivation,
        expected_contracts: &CommitteeContract,
    ) -> Result<CredibilityChange, Error> {
        if self.credibility.is_some() {
            return Err(Error::Binding);
        }
        if expected_contracts != &self.contracts {
            return Err(Error::Binding);
        }
        let before = self.inspect().sequence;
        let change = self.delivery.activate_credibility(request)?;
        if self.inspect().sequence != before {
            // Old automatic and human keys are bound to the old sequence/epoch.
            // Do not forge a reviewer revocation or rewrite a consumed key's
            // status: human status remains history and dispatch checks currentness.
            for slot in self.inputs.values_mut() {
                slot.approved = None;
            }
        }
        // Historical exact retries must not invalidate newer, valid approvals.
        Ok(change)
    }

    /// Withdraw positive use without requiring helper input, human availability,
    /// or a fresh clock. Only endpoint evidence can settle prior dispatches.
    pub fn withdraw_credibility(
        &mut self,
        request: CredibilityWithdrawalRequest,
    ) -> Result<CredibilityWithdrawal, Error> {
        let before = self.inspect().sequence;
        let change = self.delivery.withdraw_credibility(request)?;
        if self.inspect().sequence != before {
            for slot in self.inputs.values_mut() {
                slot.approved = None;
            }
        }
        Ok(change)
    }

    /// Read-only qualification state. Legacy never-configured profiles remain
    /// unchanged; successful qualification is not an automatic or human permit.
    pub fn check_credibility(&self) -> Result<(), Error> {
        self.delivery.controller().check_credibility()
    }

    /// Historical records are not claims of present qualification or execution.
    pub fn credibility_changes(&self) -> impl ExactSizeIterator<Item = &CredibilityChange> {
        self.delivery.controller().credibility_changes()
    }

    pub fn credibility_withdrawals(&self) -> impl Iterator<Item = &CredibilityWithdrawal> {
        self.delivery.controller().credibility_withdrawals()
    }
}

#[cfg(test)]
mod tests;
