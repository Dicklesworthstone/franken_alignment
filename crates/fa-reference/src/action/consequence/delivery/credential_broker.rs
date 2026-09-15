//! Narrow broker-mediated file publication over the ORIGINAL dispatch envelope.
//!
//! The loaded perimeter inventory is still a declaration, not authentication.
//! This module gives that declaration one concrete consumer: a broker-owned
//! credential and file-backed publication endpoint. Actors never receive the
//! credential, raw endpoint, or a raw-payload execution method.
#![cfg(unix)]

use super::{DispatchEnvelope, EndpointReceipt, EndpointStatus, FenceAcknowledgment, FenceRequest,
    PublicationEndpoint, StatusQuery};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::perimeter::{BypassDisposition, Mediation, PerimeterScope, ThreatClass, TrustDomain};
use crate::perimeter_inventory::{ActorCredentialDisposition, EffectKind, LoadedPerimeterInventory};
use crate::Error;
use std::fmt;

pub const DISPOSABLE_FILE_PROFILE: &str = "fa.disposable-file-publication";
pub const MAX_BROKER_CREDENTIAL_BYTES: usize = 4_096;
pub const PERIMETER_EFFECT_PURPOSE: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokerRouteBinding {
    pub family: String,
    pub route: String,
}

/// Trusted bootstrap material. The bytes have no public accessor and this type is
/// neither Clone nor serializable. The current perimeter schema declares broker
/// credentials family-wide, so this reference profile does not claim a name-to-
/// secret authentication mechanism.
pub struct BrokerCredential {
    secret: Vec<u8>,
}
impl BrokerCredential {
    pub fn new(secret: Vec<u8>) -> Result<Self, Error> {
        if secret.is_empty() { return Err(Error::InvalidInput); }
        if secret.len() > MAX_BROKER_CREDENTIAL_BYTES { return Err(Error::Limit); }
        Ok(Self { secret })
    }
}
impl fmt::Debug for BrokerCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrokerCredential { redacted: true }")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokerInspection {
    pub scope: Scope,
    pub family: String,
    pub route: String,
    pub target: ResolvedTarget,
    pub credential_exercises: u64,
    pub endpoint_executions: u64,
}

struct DisposableFileAdapter {
    endpoint: PublicationEndpoint,
    expected_secret: Vec<u8>,
}
impl DisposableFileAdapter {
    fn deliver(&mut self, credential: &BrokerCredential, message: &DispatchEnvelope)
        -> Result<EndpointReceipt, Error>
    {
        if credential.secret != self.expected_secret { return Err(Error::Binding); }
        self.endpoint.deliver(message)
    }
}

/// Concrete enforcement-domain owner for one declared brokered file-write route.
/// No endpoint accessor, credential accessor, raw payload API, or actor-port
/// conversion is provided.
///
/// ```compile_fail,E0616
/// use fa_reference::action::consequence::delivery::credential_broker::CredentialBroker;
/// fn bypass(broker: &mut CredentialBroker) { let _ = &mut broker.adapter; }
/// ```
///
/// ```compile_fail,E0616
/// use fa_reference::action::consequence::delivery::credential_broker::BrokerCredential;
/// fn leak(credential: BrokerCredential) -> Vec<u8> { credential.secret }
/// ```
pub struct CredentialBroker {
    inventory: LoadedPerimeterInventory,
    binding: BrokerRouteBinding,
    scope: Scope,
    credential: BrokerCredential,
    adapter: DisposableFileAdapter,
    credential_exercises: u64,
}
impl fmt::Debug for CredentialBroker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialBroker").field("inspection", &self.inspect()).finish()
    }
}

impl CredentialBroker {
    /// Attach only after the endpoint has been attached to its original delivery
    /// authority. The endpoint must be the existing Unix file-publication profile.
    /// The route metadata must describe exactly this narrow mediation contract.
    pub fn new(
        inventory: LoadedPerimeterInventory, binding: BrokerRouteBinding, scope: Scope,
        credential: BrokerCredential, endpoint: PublicationEndpoint,
    ) -> Result<Self, Error> {
        if scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        if [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority].contains(&0) {
            return Err(Error::InvalidInput);
        }
        if endpoint.file_storage_status().is_none() { return Err(Error::Binding); }
        let perimeter_scope = PerimeterScope {
            tenant: scope.tenant, principal: scope.principal, purpose: PERIMETER_EFFECT_PURPOSE,
        };
        let route = inventory.route_for(perimeter_scope, &binding.family, &binding.route)?;
        if route.record().mediation != Mediation::BrokeredEffects
            || route.record().bypass != BypassDisposition::Blocked
            || route.record().threat != Some(ThreatClass::DirectCredentialOrEgress)
            || route.metadata().effect() != EffectKind::FileWrite
            || !matches!(route.metadata().actor_credential(), ActorCredentialDisposition::BrokerMediated)
            || !route.metadata().trust_path().contains(&TrustDomain::Enforcement)
            || route.metadata().profile().id() != DISPOSABLE_FILE_PROFILE
            || route.metadata().profile().generation() != endpoint.target().contract_version
        {
            return Err(Error::Binding);
        }
        let expected_secret = credential.secret.clone();
        Ok(Self {
            inventory, binding, scope, credential,
            adapter: DisposableFileAdapter { endpoint, expected_secret }, credential_exercises: 0,
        })
    }

    pub fn inspect(&self) -> BrokerInspection {
        BrokerInspection {
            scope: self.scope, family: self.binding.family.clone(), route: self.binding.route.clone(),
            target: self.adapter.endpoint.target(), credential_exercises: self.credential_exercises,
            endpoint_executions: self.adapter.endpoint.execution_count(),
        }
    }
    pub fn payload(&self) -> &[u8] { self.adapter.endpoint.payload() }
    pub fn target(&self) -> ResolvedTarget { self.adapter.endpoint.target() }
    pub fn inventory_family_count(&self) -> usize { self.inventory.family_count() }

    pub fn observe_time(&mut self, tick: ElapsedTick) -> Result<(), Error> {
        self.adapter.endpoint.observe_time(tick)
    }
    pub fn install_fence(&mut self, request: FenceRequest) -> Result<FenceAcknowledgment, Error> {
        self.adapter.endpoint.install_fence(request)
    }
    pub fn status(&self, query: &StatusQuery) -> Result<EndpointStatus, Error> {
        self.adapter.endpoint.status(query)
    }
    pub fn seal_unexecuted(&mut self, query: &StatusQuery) -> Result<EndpointReceipt, Error> {
        self.adapter.endpoint.seal_unexecuted(query)
    }
    pub fn resolve_expired(&mut self, query: &StatusQuery) -> Result<EndpointReceipt, Error> {
        self.adapter.endpoint.resolve_expired(query)
    }

    /// The only method that presents the broker-held credential to the adapter.
    /// Structural route/scope/target checks occur before the credential is used.
    /// Once presentation begins, the attempt counter is conserved even when the
    /// endpoint refuses or loses its acknowledgment.
    pub fn deliver(&mut self, message: &DispatchEnvelope) -> Result<EndpointReceipt, Error> {
        self.check_envelope(message)?;
        self.credential_exercises = self.credential_exercises.checked_add(1).ok_or(Error::Overflow)?;
        self.adapter.deliver(&self.credential, message)
    }

    fn check_envelope(&self, message: &DispatchEnvelope) -> Result<(), Error> {
        if message.request().scope() != self.scope { return Err(Error::Binding); }
        let supplied = message.request().target();
        let expected = self.adapter.endpoint.target();
        if supplied.adapter != expected.adapter || supplied.object != expected.object
            || supplied.contract_version != expected.contract_version
            || supplied.generation != expected.generation
        {
            return Err(Error::Binding);
        }
        // Recheck immutable declared mediation rather than turning attachment into
        // a new authority bit. The inventory itself remains an operator assumption.
        let route = self.inventory.route_for(PerimeterScope {
            tenant: self.scope.tenant, principal: self.scope.principal,
            purpose: PERIMETER_EFFECT_PURPOSE,
        }, &self.binding.family, &self.binding.route)?;
        if route.record().mediation != Mediation::BrokeredEffects
            || route.record().bypass != BypassDisposition::Blocked
            || !matches!(route.metadata().actor_credential(), ActorCredentialDisposition::BrokerMediated)
        { return Err(Error::Binding); }
        Ok(())
    }
}
