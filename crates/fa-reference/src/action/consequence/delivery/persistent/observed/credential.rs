//! Process-local credential capability over a journaled nonsecret publication contract.
//! Secret bytes never enter the FileOversight journal. Recovery creates a new host
//! issuer, so every old credential permit becomes unusable after reopen.

#[cfg(test)]
mod storage_tests;

use super::{Event, FileOversight, JournalError, Transition};
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::action::consequence::delivery::credential_broker::{
    BrokerCredential, BrokerRouteBinding, CredentialChangeReceipt, CredentialRevocationRequest,
    CredentialRotationRequest, ProviderCredential,
};
use crate::action::consequence::oversight::CommitteeInput;
use crate::perimeter::{BypassDisposition, Mediation, PerimeterScope, ThreatClass, TrustDomain, MAX_TEXT_BYTES};
use crate::perimeter_inventory::{ActorCredentialDisposition, EffectKind, LoadedPerimeterInventory};
use crate::{Error, Snapshot};
use crate::action::ElapsedTick;
use std::fmt;
use std::rc::Rc;

pub const FILE_OVERSIGHT_CREDENTIAL_PROFILE: &str = "fa.file-oversight-publication";
pub const PERIMETER_EFFECT_PURPOSE: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredentialPolicy {
    pub family: String,
    pub route: String,
    pub credential: String,
    pub profile_generation: u64,
}
impl FileCredentialPolicy {
    pub(super) fn check(&self) -> Result<(), Error> {
        for value in [&self.family, &self.route, &self.credential] {
            if value.is_empty() { return Err(Error::InvalidInput); }
            if value.len() > MAX_TEXT_BYTES { return Err(Error::Limit); }
        }
        if self.profile_generation == 0 { return Err(Error::InvalidInput); }
        Ok(())
    }
    pub(super) fn resolve(inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding,
        scope: Scope, target: ResolvedTarget) -> Result<Self, Error>
    {
        if scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        let perimeter_scope = PerimeterScope { tenant: scope.tenant, principal: scope.principal, purpose: PERIMETER_EFFECT_PURPOSE };
        let route = inventory.route_for(perimeter_scope, &binding.family, &binding.route)?;
        let credential = inventory.broker_credential_for_route(perimeter_scope, &binding.family, &binding.route)?;
        if route.record().mediation != Mediation::BrokeredEffects
            || route.record().bypass != BypassDisposition::Blocked
            || route.record().threat != Some(ThreatClass::DirectCredentialOrEgress)
            || route.metadata().effect() != EffectKind::FileWrite
            || !matches!(route.metadata().actor_credential(), ActorCredentialDisposition::BrokerMediated)
            || !route.metadata().trust_path().contains(&TrustDomain::Enforcement)
            || route.metadata().profile().id() != FILE_OVERSIGHT_CREDENTIAL_PROFILE
            || route.metadata().profile().generation() != target.contract_version
        { return Err(Error::Binding); }
        let policy = Self { family: binding.family.clone(), route: binding.route.clone(),
            credential: credential.to_owned(), profile_generation: route.metadata().profile().generation() };
        policy.check()?;
        Ok(policy)
    }
    fn revalidate(&self, inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding,
        scope: Scope, target: ResolvedTarget) -> Result<(), Error>
    {
        let current = Self::resolve(inventory, binding, scope, target)?;
        if &current != self { return Err(Error::Binding); }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredentialStatus {
    pub policy: FileCredentialPolicy,
    pub generation: u64,
    pub revoked: bool,
    pub retained_changes: usize,
}

#[derive(Clone)]
pub(super) enum FileCredentialChange {
    Rotation { request: CredentialRotationRequest, receipt: CredentialChangeReceipt },
    Revocation { request: CredentialRevocationRequest, receipt: CredentialChangeReceipt },
}
impl FileCredentialChange {
    pub(super) fn operation(&self) -> u64 {
        match self { Self::Rotation { request, .. } => request.operation, Self::Revocation { request, .. } => request.operation }
    }
    pub(super) fn retry_rotation(&self, request: CredentialRotationRequest) -> Result<CredentialChangeReceipt, Error> {
        match self { Self::Rotation { request: original, receipt } if *original == request => Ok(*receipt), _ => Err(Error::Binding) }
    }
    pub(super) fn retry_revocation(&self, request: CredentialRevocationRequest) -> Result<CredentialChangeReceipt, Error> {
        match self { Self::Revocation { request: original, receipt } if *original == request => Ok(*receipt), _ => Err(Error::Binding) }
    }
    pub(super) fn receipt(&self) -> CredentialChangeReceipt {
        match self { Self::Rotation { receipt, .. } | Self::Revocation { receipt, .. } => *receipt }
    }
}

pub struct FileCredentialPermit {
    issuer: Rc<()>,
    policy: FileCredentialPolicy,
    generation: u64,
    broker: BrokerCredential,
    provider: ProviderCredential,
}
impl fmt::Debug for FileCredentialPermit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCredentialPermit").field("policy", &self.policy)
            .field("generation", &self.generation).finish_non_exhaustive()
    }
}
impl FileCredentialPermit {
    pub fn policy(&self) -> &FileCredentialPolicy { &self.policy }
    pub fn generation(&self) -> u64 { self.generation }
}

impl FileOversight {
    pub fn enable_credential_guard(&mut self, revision: u64,
        inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding) -> Result<FileCredentialPolicy, JournalError>
    {
        let policy = FileCredentialPolicy::resolve(inventory, binding, self.profile.delivery.scope, self.profile.delivery.target)?;
        match self.transact(revision, Event::CredentialGuard(policy.clone()))? {
            Transition::Unit => Ok(policy), _ => unreachable!("credential guard transition"),
        }
    }
    pub fn credential_policy(&self) -> Option<&FileCredentialPolicy> { self.machine.credential_policy.as_ref() }
    pub fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.credential_policy.as_ref().map(|policy| FileCredentialStatus {
            policy: policy.clone(), generation: self.machine.credential_generation,
            revoked: self.machine.credential_revoked, retained_changes: self.machine.credential_changes.len(),
        }))
    }
    pub fn rotate_credential_guard(&mut self, revision: u64, request: CredentialRotationRequest)
        -> Result<CredentialChangeReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(change) = self.machine.credential_change(request.operation) { return Ok(change.retry_rotation(request)?); }
        self.transact(revision, Event::CredentialRotate(request))?;
        Ok(self.machine.credential_change(request.operation).expect("committed rotation retained").receipt())
    }
    pub fn revoke_credential_guard(&mut self, revision: u64, request: CredentialRevocationRequest)
        -> Result<CredentialChangeReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(change) = self.machine.credential_change(request.operation) { return Ok(change.retry_revocation(request)?); }
        self.transact(revision, Event::CredentialRevoke(request))?;
        Ok(self.machine.credential_change(request.operation).expect("committed revocation retained").receipt())
    }
    pub fn credential_change(&self, operation: u64) -> Result<CredentialChangeReceipt, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.credential_change(operation).ok_or(Error::Missing)?.receipt())
    }
    pub fn bind_credential_pair(&self, expected_revision: u64,
        inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding,
        broker: BrokerCredential, provider: ProviderCredential) -> Result<FileCredentialPermit, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if expected_revision != self.revision() { return Err(Error::Stale.into()); }
        let policy = self.machine.credential_policy.as_ref().ok_or(Error::WrongState)?.clone();
        if self.machine.credential_revoked { return Err(Error::WrongState.into()); }
        policy.revalidate(inventory, binding, self.profile.delivery.scope, self.profile.delivery.target)?;
        if !broker.agrees_with(&provider) { return Err(Error::Binding.into()); }
        Ok(FileCredentialPermit { issuer: Rc::clone(&self.issuer), policy,
            generation: self.machine.credential_generation, broker, provider })
    }
    fn check_credential_permit(&self, permit: &FileCredentialPermit) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &permit.issuer) { return Err(Error::Binding.into()); }
        let policy = self.machine.credential_policy.as_ref().ok_or(Error::WrongState)?;
        if self.machine.credential_revoked { return Err(Error::WrongState.into()); }
        if self.machine.credential_generation != permit.generation
            || policy != &permit.policy || !permit.broker.agrees_with(&permit.provider)
        { return Err(Error::Binding.into()); }
        Ok(())
    }
    pub fn publish_checked_with_credential(&mut self, revision: u64, attempt: u64,
        current: Option<&CommitteeInput>, snapshot: Snapshot, now: ElapsedTick,
        permit: &FileCredentialPermit) -> Result<super::publication::CheckedPublication, JournalError>
    {
        self.check_credential_permit(permit)?;
        if let Some(input) = current { self.check_action(attempt, input.action())?; }
        let supplied = current.map(|input| input.views().clone());
        match self.transact(revision, Event::PublishCredentialed(attempt, supplied, snapshot, now))? {
            Transition::PublicationChecked(result) => Ok(result), _ => unreachable!("credentialed publication transition"),
        }
    }
}
