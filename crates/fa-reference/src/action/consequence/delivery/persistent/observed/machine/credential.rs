//! Replay semantics for the nonsecret credential publication contract.
use super::{Machine, Transition};
use super::super::credential::FileCredentialPolicy;
use crate::action::ActionState;
use crate::Error;

impl Machine {
    pub(super) fn enable_credential_guard(&mut self, policy: &FileCredentialPolicy)
        -> Result<Transition, Error>
    {
        policy.check()?;
        if self.credential_policy.is_some() { return Err(Error::Duplicate); }
        let control = self.broker.inspect();
        if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
            || !self.automatic.is_empty() || !self.human_keys.is_empty() || !self.envelopes.is_empty()
            || control.sequence != 0 || control.suspended || self.broker.stop_receipt().is_some()
            || control.ledger.stages.values().any(|stage| !matches!(stage, ActionState::Cancelled))
        { return Err(Error::WrongState); }
        // Credential mediation has no raw-publish fallback. The same bootstrap
        // transition therefore enables the existing fresh first-publication gate.
        self.publication_guard = true;
        self.credential_policy = Some(policy.clone());
        Ok(Transition::Unit)
    }
}
