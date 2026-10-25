//! Replay semantics for the nonsecret credential publication contract and lifecycle.
use super::{Machine, Transition};
use super::super::credential::{FileCredentialChange, FileCredentialPolicy, FileCredentialStatus};
use crate::action::ActionState;
use crate::action::consequence::delivery::credential_broker::{
    CredentialChangeReceipt, CredentialRevocationRequest, CredentialRotationRequest,
    MAX_CREDENTIAL_CHANGES,
};
use crate::Error;

impl Machine {
    pub(in super::super) fn credential_status(&self) -> Option<FileCredentialStatus> {
        self.credential_policy.as_ref().map(|policy| FileCredentialStatus {
            policy: policy.clone(), generation: self.credential_generation,
            revoked: self.credential_revoked, retained_changes: self.credential_changes.len(),
        })
    }

    /// Inspect the original operation before recovery publishes a fence. An
    /// exact historical retry is distinct from a newly admissible transition.
    /// These queries never reserve work, mutate state or issue a capability.
    pub(in super::super) fn check_credential_rotation(&self, request: CredentialRotationRequest)
        -> Result<Option<CredentialChangeReceipt>, Error>
    {
        if let Some(change) = self.credential_change(request.operation) {
            return change.retry_rotation(request).map(Some);
        }
        self.check_new_credential_rotation(request)?;
        Ok(None)
    }

    pub(in super::super) fn check_credential_revocation(&self, request: CredentialRevocationRequest)
        -> Result<Option<CredentialChangeReceipt>, Error>
    {
        if let Some(change) = self.credential_change(request.operation) {
            return change.retry_revocation(request).map(Some);
        }
        self.check_new_credential_revocation(request)?;
        Ok(None)
    }

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
        self.publication_guard = true;
        self.credential_policy = Some(policy.clone());
        self.credential_generation = 1;
        self.credential_revoked = false;
        self.credential_changes.clear();
        Ok(Transition::Unit)
    }

    pub(super) fn rotate_credential(&mut self, request: CredentialRotationRequest)
        -> Result<Transition, Error>
    {
        self.check_new_credential_rotation(request)?;
        self.credential_changes.try_reserve(1).map_err(|_| Error::Limit)?;
        let receipt = CredentialChangeReceipt { operation: request.operation,
            generation: request.next_generation, revoked: false };
        self.credential_generation = request.next_generation;
        self.credential_changes.push(FileCredentialChange::Rotation { request, receipt });
        Ok(Transition::Unit)
    }

    // Shared with read-only guarded-recovery preflight. The replay transition
    // still rejects duplicate events, including an exact duplicate, as before.
    fn check_new_credential_rotation(&self, request: CredentialRotationRequest) -> Result<(), Error> {
        self.credential_policy.as_ref().ok_or(Error::WrongState)?;
        if request.operation == 0 { return Err(Error::InvalidInput); }
        if self.credential_change(request.operation).is_some() { return Err(Error::Duplicate); }
        if self.credential_revoked { return Err(Error::WrongState); }
        if request.expected_generation != self.credential_generation { return Err(Error::Stale); }
        let next = self.credential_generation.checked_add(1).ok_or(Error::Overflow)?;
        if request.next_generation != next { return Err(Error::Stale); }
        if self.credential_changes.len() >= MAX_CREDENTIAL_CHANGES { return Err(Error::Limit); }
        Ok(())
    }

    pub(super) fn revoke_credential(&mut self, request: CredentialRevocationRequest)
        -> Result<Transition, Error>
    {
        self.check_new_credential_revocation(request)?;
        self.credential_changes.try_reserve(1).map_err(|_| Error::Limit)?;
        let receipt = CredentialChangeReceipt { operation: request.operation,
            generation: self.credential_generation, revoked: true };
        self.credential_revoked = true;
        self.credential_changes.push(FileCredentialChange::Revocation { request, receipt });
        Ok(Transition::Unit)
    }

    fn check_new_credential_revocation(&self, request: CredentialRevocationRequest) -> Result<(), Error> {
        self.credential_policy.as_ref().ok_or(Error::WrongState)?;
        if request.operation == 0 { return Err(Error::InvalidInput); }
        if self.credential_change(request.operation).is_some() { return Err(Error::Duplicate); }
        if self.credential_revoked { return Err(Error::WrongState); }
        if request.expected_generation != self.credential_generation { return Err(Error::Stale); }
        if self.credential_changes.len() >= MAX_CREDENTIAL_CHANGES { return Err(Error::Limit); }
        Ok(())
    }

    pub(in super::super) fn credential_change(&self, operation: u64) -> Option<&FileCredentialChange> {
        self.credential_changes.iter().find(|change| change.operation() == operation)
    }
}
