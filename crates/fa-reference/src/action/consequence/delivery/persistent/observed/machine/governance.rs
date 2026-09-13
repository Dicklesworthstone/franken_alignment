//! Preserve the original two-key authority across a durable policy generation.
use super::{Machine, Transition};
use super::super::super::governance::PolicyUpdate;
use crate::Error;

impl Machine {
    pub(super) fn apply_policy_update(&mut self, update: &PolicyUpdate) -> Result<Transition, Error> {
        self.policy_updates.preflight(update)?;
        let change = self.broker.replace_policy(update.expected_control_sequence(),
            update.expected_authority_epoch(), update.policy().clone())?;
        // The original transition cancelled every undispatched attempt. It did
        // NOT refund already-admitted effects or change their endpoint envelope.
        for id in &change.cancelled { self.automatic.remove(id); }
        self.sessions.clear();
        // All remaining pending/approved human keys refer to the old control
        // predecessor. Use the original withdrawal; consumed keys stay consumed.
        self.withdraw_keys()?;
        let receipt = self.policy_updates.record(update, change);
        Ok(Transition::PolicyUpdated(receipt))
    }
}
