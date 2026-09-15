//! Process-local credential capability over a journaled nonsecret publication contract.
//! Secret bytes never enter the FileOversight journal. Recovery creates a new host
//! issuer, so every old credential permit becomes unusable after reopen.

use super::{Event, FileOversight, JournalError, Transition};
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::action::consequence::delivery::credential_broker::{
    BrokerCredential, BrokerRouteBinding, ProviderCredential,
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

/// Persisted contract only: exact declared route and uniquely resolved broker
/// credential name. It contains no secret, permit or endpoint result.
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

    fn resolve(inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding,
        scope: Scope, target: ResolvedTarget) -> Result<Self, Error>
    {
        if scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        let perimeter_scope = PerimeterScope {
            tenant: scope.tenant, principal: scope.principal, purpose: PERIMETER_EFFECT_PURPOSE,
        };
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

/// Process-local provider capability. It owns BOTH independently supplied secret
/// roles so publication checks agreement at the effect boundary. It is non-clone,
/// non-serializable and branded to one live FileOversight owner.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::credential::FileCredentialPermit;
/// fn duplicate(key: FileCredentialPermit) { let _ = key.clone(); }
/// ```
pub struct FileCredentialPermit {
    issuer: Rc<()>,
    policy: FileCredentialPolicy,
    broker: BrokerCredential,
    provider: ProviderCredential,
}
impl fmt::Debug for FileCredentialPermit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCredentialPermit").field("policy", &self.policy).finish_non_exhaustive()
    }
}
impl FileCredentialPermit {
    pub fn policy(&self) -> &FileCredentialPolicy { &self.policy }
}

impl FileOversight {
    /// Irreversibly enable credential mediation before any proposal. This also
    /// enables the existing first-publication evidence guard, eliminating the raw
    /// publication fallback. Only nonsecret route/credential identity is journaled.
    pub fn enable_credential_guard(&mut self, revision: u64,
        inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding)
        -> Result<FileCredentialPolicy, JournalError>
    {
        let policy = FileCredentialPolicy::resolve(inventory, binding,
            self.profile.delivery.scope, self.profile.delivery.target)?;
        match self.transact(revision, Event::CredentialGuard(policy.clone()))? {
            Transition::Unit => Ok(policy),
            _ => unreachable!("credential guard transition"),
        }
    }

    pub fn credential_policy(&self) -> Option<&FileCredentialPolicy> {
        self.machine.credential_policy.as_ref()
    }

    /// Rebind secret material to THIS live owner. No journal event or authority
    /// transition is produced. The independently supplied inventory must still
    /// resolve to the exact persisted v1 credential contract. Reopen uses a fresh
    /// issuer, so every pre-recovery permit is rejected even if retained in RAM.
    pub fn bind_credential_pair(&self, expected_revision: u64,
        inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding,
        broker: BrokerCredential, provider: ProviderCredential)
        -> Result<FileCredentialPermit, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if expected_revision != self.revision() { return Err(Error::Stale.into()); }
        let policy = self.machine.credential_policy.as_ref().ok_or(Error::WrongState)?.clone();
        policy.revalidate(inventory, binding, self.profile.delivery.scope, self.profile.delivery.target)?;
        if !broker.agrees_with(&provider) { return Err(Error::Binding.into()); }
        Ok(FileCredentialPermit { issuer: Rc::clone(&self.issuer), policy, broker, provider })
    }

    fn check_credential_permit(&self, permit: &FileCredentialPermit) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &permit.issuer) { return Err(Error::Binding.into()); }
        let policy = self.machine.credential_policy.as_ref().ok_or(Error::WrongState)?;
        if policy != &permit.policy || !permit.broker.agrees_with(&permit.provider) {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    /// Credentialed first publication. A valid permit only proves the live
    /// provider boundary is present; all original evidence/two-key checks still
    /// run inside the same durable publication transition.
    pub fn publish_checked_with_credential(&mut self, revision: u64, attempt: u64,
        current: Option<&CommitteeInput>, snapshot: Snapshot, now: ElapsedTick,
        permit: &FileCredentialPermit) -> Result<super::publication::CheckedPublication, JournalError>
    {
        self.check_credential_permit(permit)?;
        if let Some(input) = current { self.check_action(attempt, input.action())?; }
        let supplied = current.map(|input| input.views().clone());
        match self.transact(revision, Event::PublishCredentialed(attempt, supplied, snapshot, now))? {
            Transition::PublicationChecked(result) => Ok(result),
            _ => unreachable!("credentialed publication transition"),
        }
    }
}
