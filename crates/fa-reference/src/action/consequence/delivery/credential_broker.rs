//! Narrow broker-mediated file publication over the ORIGINAL dispatch envelope.
//!
//! The loaded perimeter inventory is still a declaration, not authentication.
//! This module gives that declaration one concrete consumer: a broker-owned
//! credential and file-backed publication endpoint. Actors never receive the
//! credential, raw endpoint, or a raw-payload execution method.
#![cfg(unix)]

use super::{DispatchEnvelope, EndpointReceipt, EndpointStatus, FenceAcknowledgment, FenceRequest,
    FileEndpointRecovery, PublicationEndpoint, StatusQuery};
use super::filesystem::FilePublicationError;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::perimeter::{BypassDisposition, Mediation, PerimeterScope, ThreatClass, TrustDomain};
use crate::perimeter_inventory::{ActorCredentialDisposition, EffectKind, LoadedPerimeterInventory};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub const DISPOSABLE_FILE_PROFILE: &str = "fa.disposable-file-publication";
pub const MAX_BROKER_CREDENTIAL_BYTES: usize = 4_096;
pub const MAX_CREDENTIAL_CHANGES: usize = 64;
pub const PERIMETER_EFFECT_PURPOSE: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokerRouteBinding {
    pub family: String,
    pub route: String,
}

/// Secret material retained only by the enforcement-side broker. The bytes have
/// no public accessor and this type is neither Clone nor serializable.
pub struct BrokerCredential { secret: Vec<u8> }
impl BrokerCredential {
    pub fn new(secret: Vec<u8>) -> Result<Self, Error> {
        check_secret(&secret)?;
        Ok(Self { secret })
    }
}
impl fmt::Debug for BrokerCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("BrokerCredential { redacted: true }") }
}

/// Independently supplied provider-side expected credential. This is deliberately
/// a distinct capability from BrokerCredential: constructing one from the other
/// is not exposed. The reference compares these two trusted bootstrap inputs; it
/// does not claim provider authentication, encrypted storage, or constant-time
/// cryptographic verification.
///
/// ```compile_fail,E0616
/// use fa_reference::action::consequence::delivery::credential_broker::ProviderCredential;
/// fn leak(credential: ProviderCredential) -> Vec<u8> { credential.secret }
/// ```
pub struct ProviderCredential { secret: Vec<u8> }
impl ProviderCredential {
    pub fn new(secret: Vec<u8>) -> Result<Self, Error> {
        check_secret(&secret)?;
        Ok(Self { secret })
    }
}
impl fmt::Debug for ProviderCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("ProviderCredential { redacted: true }") }
}
fn check_secret(secret: &[u8]) -> Result<(), Error> {
    if secret.is_empty() { return Err(Error::InvalidInput); }
    if secret.len() > MAX_BROKER_CREDENTIAL_BYTES { return Err(Error::Limit); }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokerInspection {
    pub scope: Scope,
    pub family: String,
    pub route: String,
    pub target: ResolvedTarget,
    pub credential_generation: u64,
    pub credential_revoked: bool,
    pub credential_exercises: u64,
    pub endpoint_executions: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredentialRotationRequest {
    pub operation: u64,
    pub expected_generation: u64,
    pub next_generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredentialRevocationRequest {
    pub operation: u64,
    pub expected_generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredentialChangeReceipt {
    pub operation: u64,
    pub generation: u64,
    pub revoked: bool,
}
#[derive(Clone)]
enum StoredCredentialChange {
    Rotation { request: CredentialRotationRequest, secret: Vec<u8>, receipt: CredentialChangeReceipt },
    Revocation { request: CredentialRevocationRequest, receipt: CredentialChangeReceipt },
}
impl StoredCredentialChange {
    fn receipt(&self) -> CredentialChangeReceipt {
        match self { Self::Rotation { receipt, .. } | Self::Revocation { receipt, .. } => *receipt }
    }
}

/// Reference stand-in for an independently configured provider. Its expected
/// credential is supplied separately from the broker-held credential. The actor
/// cannot mutate or inspect it.
struct DisposableFileAdapter { endpoint: PublicationEndpoint, expected_secret: Vec<u8> }
impl DisposableFileAdapter {
    fn deliver(&mut self, credential: &BrokerCredential, message: &DispatchEnvelope) -> Result<EndpointReceipt, Error> {
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
    endpoint_binding: Rc<()>,
    credential_generation: u64,
    credential_revoked: bool,
    credential_changes: BTreeMap<u64, StoredCredentialChange>,
    credential_exercises: u64,
}
impl fmt::Debug for CredentialBroker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialBroker").field("inspection", &self.inspect()).finish()
    }
}

impl CredentialBroker {
    /// The broker-held and provider-held credentials are independent bootstrap
    /// inputs. A mismatch refuses before either owner is retained and before any
    /// endpoint operation. This removes the old self-fulfilling credential check
    /// where the provider expectation was cloned from the broker secret itself.
    pub fn new(inventory: LoadedPerimeterInventory, binding: BrokerRouteBinding, scope: Scope,
        credential: BrokerCredential, provider: ProviderCredential,
        endpoint: PublicationEndpoint) -> Result<Self, Error>
    {
        Self::validate_attachment(&inventory, &binding, scope, &endpoint)?;
        if credential.secret != provider.secret { return Err(Error::Binding); }
        let endpoint_binding = Rc::clone(&endpoint.binding);
        Ok(Self { inventory, binding, scope, credential,
            adapter: DisposableFileAdapter { endpoint, expected_secret: provider.secret }, endpoint_binding,
            credential_generation: 1, credential_revoked: false,
            credential_changes: BTreeMap::new(), credential_exercises: 0 })
    }

    pub fn inspect(&self) -> BrokerInspection {
        BrokerInspection { scope: self.scope, family: self.binding.family.clone(), route: self.binding.route.clone(),
            target: self.adapter.endpoint.target(), credential_generation: self.credential_generation,
            credential_revoked: self.credential_revoked, credential_exercises: self.credential_exercises,
            endpoint_executions: self.adapter.endpoint.execution_count() }
    }
    pub fn payload(&self) -> &[u8] { self.adapter.endpoint.payload() }
    pub fn target(&self) -> ResolvedTarget { self.adapter.endpoint.target() }
    pub fn inventory_family_count(&self) -> usize { self.inventory.family_count() }

    pub fn observe_time(&mut self, tick: ElapsedTick) -> Result<(), Error> { self.adapter.endpoint.observe_time(tick) }
    pub fn install_fence(&mut self, request: FenceRequest) -> Result<FenceAcknowledgment, Error> {
        self.adapter.endpoint.install_fence(request)
    }
    pub fn status(&self, query: &StatusQuery) -> Result<EndpointStatus, Error> { self.adapter.endpoint.status(query) }
    pub fn seal_unexecuted(&mut self, query: &StatusQuery) -> Result<EndpointReceipt, Error> {
        self.adapter.endpoint.seal_unexecuted(query)
    }
    pub fn resolve_expired(&mut self, query: &StatusQuery) -> Result<EndpointReceipt, Error> {
        self.adapter.endpoint.resolve_expired(query)
    }

    /// Rotate provider bootstrap material without changing action authority. The
    /// next broker and provider capabilities must independently agree before the
    /// transition mutates either side. Old dispatch envelopes retain their
    /// original authorization semantics and may still be sent using the newly
    /// provisioned pair. Rotation cannot reopen a terminally revoked broker.
    pub fn rotate_credential(&mut self, request: CredentialRotationRequest,
        next: BrokerCredential, provider: ProviderCredential) -> Result<CredentialChangeReceipt, Error>
    {
        if let Some(stored) = self.credential_changes.get(&request.operation) {
            return match stored {
                StoredCredentialChange::Rotation { request: original, secret, receipt }
                    if original == &request && secret == &next.secret && secret == &provider.secret => Ok(*receipt),
                _ => Err(Error::Binding),
            };
        }
        if request.operation == 0 { return Err(Error::InvalidInput); }
        if next.secret != provider.secret { return Err(Error::Binding); }
        if self.credential_revoked { return Err(Error::WrongState); }
        if request.expected_generation != self.credential_generation { return Err(Error::Stale); }
        let expected_next = self.credential_generation.checked_add(1).ok_or(Error::Overflow)?;
        if request.next_generation != expected_next { return Err(Error::Stale); }
        if self.credential_changes.len() >= MAX_CREDENTIAL_CHANGES { return Err(Error::Limit); }
        let receipt = CredentialChangeReceipt { operation: request.operation,
            generation: request.next_generation, revoked: false };
        let secret = next.secret.clone();
        self.adapter.expected_secret = provider.secret;
        self.credential = next;
        self.credential_generation = request.next_generation;
        self.credential_changes.insert(request.operation,
            StoredCredentialChange::Rotation { request, secret, receipt });
        Ok(receipt)
    }

    /// Terminally stop NEW credentialed deliveries. Endpoint status, sealing,
    /// expiry resolution, fencing and clock observation remain available so an
    /// already charged dispatch can still be reconciled without a credential.
    pub fn revoke_credential(&mut self, request: CredentialRevocationRequest)
        -> Result<CredentialChangeReceipt, Error>
    {
        if let Some(stored) = self.credential_changes.get(&request.operation) {
            return match stored {
                StoredCredentialChange::Revocation { request: original, receipt } if original == &request => Ok(*receipt),
                _ => Err(Error::Binding),
            };
        }
        if request.operation == 0 { return Err(Error::InvalidInput); }
        if self.credential_revoked { return Err(Error::WrongState); }
        if request.expected_generation != self.credential_generation { return Err(Error::Stale); }
        if self.credential_changes.len() >= MAX_CREDENTIAL_CHANGES { return Err(Error::Limit); }
        let receipt = CredentialChangeReceipt { operation: request.operation,
            generation: self.credential_generation, revoked: true };
        self.credential_revoked = true;
        self.credential_changes.insert(request.operation, StoredCredentialChange::Revocation { request, receipt });
        Ok(receipt)
    }
    pub fn credential_change(&self, operation: u64) -> Result<CredentialChangeReceipt, Error> {
        Ok(self.credential_changes.get(&operation).ok_or(Error::Missing)?.receipt())
    }

    /// The only method that presents the broker-held credential to the separately
    /// configured provider expectation.
    pub fn deliver(&mut self, message: &DispatchEnvelope) -> Result<EndpointReceipt, Error> {
        if self.credential_revoked { return Err(Error::WrongState); }
        self.check_envelope(message)?;
        self.credential_exercises = self.credential_exercises.checked_add(1).ok_or(Error::Overflow)?;
        self.adapter.deliver(&self.credential, message)
    }

    fn check_envelope(&self, message: &DispatchEnvelope) -> Result<(), Error> {
        if message.request().scope() != self.scope { return Err(Error::Binding); }
        let supplied = message.request().target(); let expected = self.adapter.endpoint.target();
        if supplied.adapter != expected.adapter || supplied.object != expected.object
            || supplied.contract_version != expected.contract_version || supplied.generation != expected.generation
        { return Err(Error::Binding); }
        let perimeter_scope = PerimeterScope { tenant: self.scope.tenant,
            principal: self.scope.principal, purpose: PERIMETER_EFFECT_PURPOSE };
        let route = self.inventory.route_for(perimeter_scope, &self.binding.family, &self.binding.route)?;
        self.inventory.broker_credential_for_route(perimeter_scope, &self.binding.family, &self.binding.route)?;
        if route.record().mediation != Mediation::BrokeredEffects || route.record().bypass != BypassDisposition::Blocked
            || !matches!(route.metadata().actor_credential(), ActorCredentialDisposition::BrokerMediated)
        { return Err(Error::Binding); }
        Ok(())
    }

    fn validate_attachment(inventory: &LoadedPerimeterInventory, binding: &BrokerRouteBinding,
        scope: Scope, endpoint: &PublicationEndpoint) -> Result<(), Error>
    {
        if scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        if [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority].contains(&0) { return Err(Error::InvalidInput); }
        if endpoint.file_storage_status().is_none() { return Err(Error::Binding); }
        let perimeter_scope = PerimeterScope { tenant: scope.tenant, principal: scope.principal,
            purpose: PERIMETER_EFFECT_PURPOSE };
        let route = inventory.route_for(perimeter_scope, &binding.family, &binding.route)?;
        // V1 declares credentials at family scope. Refuse concrete effect
        // attachment unless that declaration resolves to exactly one broker
        // credential rather than guessing among multiple provider secrets.
        inventory.broker_credential_for_route(perimeter_scope, &binding.family, &binding.route)?;
        if route.record().mediation != Mediation::BrokeredEffects || route.record().bypass != BypassDisposition::Blocked
            || route.record().threat != Some(ThreatClass::DirectCredentialOrEgress)
            || route.metadata().effect() != EffectKind::FileWrite
            || !matches!(route.metadata().actor_credential(), ActorCredentialDisposition::BrokerMediated)
            || !route.metadata().trust_path().contains(&TrustDomain::Enforcement)
            || route.metadata().profile().id() != DISPOSABLE_FILE_PROFILE
            || route.metadata().profile().generation() != endpoint.target().contract_version
        { return Err(Error::Binding); }
        Ok(())
    }
}

pub struct RecoverableCredentialBroker { broker: CredentialBroker, recovery: FileEndpointRecovery }
impl fmt::Debug for RecoverableCredentialBroker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoverableCredentialBroker").field("broker", &self.broker).finish_non_exhaustive()
    }
}
impl RecoverableCredentialBroker {
    pub fn new(inventory: LoadedPerimeterInventory, binding: BrokerRouteBinding, scope: Scope,
        credential: BrokerCredential, provider: ProviderCredential,
        endpoint: PublicationEndpoint, recovery: FileEndpointRecovery) -> Result<Self, Error>
    { Ok(Self { broker: CredentialBroker::new(inventory, binding, scope, credential, provider, endpoint)?, recovery }) }
    pub fn broker(&self) -> &CredentialBroker { &self.broker }
    pub fn broker_mut(&mut self) -> &mut CredentialBroker { &mut self.broker }
    pub fn into_offline(self) -> OfflineCredentialBroker {
        let Self { broker, recovery } = self;
        let CredentialBroker { inventory, binding, scope, credential, adapter, endpoint_binding,
            credential_generation, credential_revoked, credential_changes, credential_exercises } = broker;
        let DisposableFileAdapter { endpoint, expected_secret } = adapter; drop(endpoint);
        OfflineCredentialBroker { inventory, binding, scope, credential, expected_secret, endpoint_binding,
            credential_generation, credential_revoked, credential_changes, credential_exercises, recovery }
    }
}

#[derive(Debug)]
pub enum BrokerReconnectError { Endpoint(FilePublicationError), Contract(Error) }
impl From<FilePublicationError> for BrokerReconnectError { fn from(error: FilePublicationError) -> Self { Self::Endpoint(error) } }
impl From<Error> for BrokerReconnectError { fn from(error: Error) -> Self { Self::Contract(error) } }
pub struct BrokerReconnectFailure { pub error: BrokerReconnectError, pub offline: OfflineCredentialBroker }
impl fmt::Debug for BrokerReconnectFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerReconnectFailure").field("error", &self.error).finish_non_exhaustive()
    }
}

pub struct OfflineCredentialBroker {
    inventory: LoadedPerimeterInventory, binding: BrokerRouteBinding, scope: Scope,
    credential: BrokerCredential, expected_secret: Vec<u8>, endpoint_binding: Rc<()>,
    credential_generation: u64, credential_revoked: bool,
    credential_changes: BTreeMap<u64, StoredCredentialChange>, credential_exercises: u64,
    recovery: FileEndpointRecovery,
}
impl fmt::Debug for OfflineCredentialBroker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OfflineCredentialBroker").field("scope", &self.scope).field("binding", &self.binding)
            .field("credential_generation", &self.credential_generation).field("credential_revoked", &self.credential_revoked)
            .field("credential_exercises", &self.credential_exercises).finish_non_exhaustive()
    }
}
impl OfflineCredentialBroker {
    pub fn reopen(self) -> Result<RecoverableCredentialBroker, BrokerReconnectFailure> {
        let endpoint = match self.recovery.reopen() {
            Ok(endpoint) => endpoint,
            Err(error) => return Err(BrokerReconnectFailure { error: error.into(), offline: self }),
        };
        if !Rc::ptr_eq(&self.endpoint_binding, &endpoint.binding) {
            drop(endpoint); return Err(BrokerReconnectFailure { error: BrokerReconnectError::Contract(Error::Binding), offline: self });
        }
        if let Err(error) = CredentialBroker::validate_attachment(&self.inventory, &self.binding, self.scope, &endpoint) {
            drop(endpoint); return Err(BrokerReconnectFailure { error: error.into(), offline: self });
        }
        let OfflineCredentialBroker { inventory, binding, scope, credential, expected_secret, endpoint_binding,
            credential_generation, credential_revoked, credential_changes, credential_exercises, recovery } = self;
        let broker = CredentialBroker { inventory, binding, scope, credential,
            adapter: DisposableFileAdapter { endpoint, expected_secret }, endpoint_binding,
            credential_generation, credential_revoked, credential_changes, credential_exercises };
        Ok(RecoverableCredentialBroker { broker, recovery })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::VERSION;
    use crate::perimeter_inventory::LoadedPerimeterInventory;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            Self(std::env::temp_dir().join(format!("fa-broker-life-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
        }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
    fn target() -> ResolvedTarget { ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 } }
    fn inventory() -> LoadedPerimeterInventory {
        LoadedPerimeterInventory::from_json_bytes(format!(r#"{{"version":1,"families":[{{"scope":{{"tenant":1,"principal":2,"purpose":1}},
        "family":"publication","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
        "credentials":[{{"credential":"token","holder":"broker"}}],"routes":[{{"route":"adapter:disposable-file","effect":"file_write",
        "profile":{{"id":"{}","generation":1}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress",
        "actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["reference"]}}],
        "residual_nonclaims":["reference"]}}]}}"#, DISPOSABLE_FILE_PROFILE).as_bytes()).unwrap()
    }
    fn binding() -> BrokerRouteBinding { BrokerRouteBinding { family: "publication".into(), route: "adapter:disposable-file".into() } }
    fn fixture() -> (Temp, RecoverableCredentialBroker, DispatchEnvelope) {
        let root = Temp::new();
        let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(&root.0, target(), b"old".to_vec(), 200, 8,
            super::super::FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
        endpoint.attach(scope()).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
        let request = super::super::PublicationRequest { version: VERSION, scope: scope(), target: target(), payload: b"new".to_vec(),
            policy_epoch: 0, deadline: ElapsedTick(100), units: 16, approval: None };
        let message = DispatchEnvelope { binding: Rc::clone(&endpoint.binding), epoch: 0, attempt: 1, request, retained_until: ElapsedTick(150) };
        let broker = RecoverableCredentialBroker::new(inventory(), binding(), scope(),
            BrokerCredential::new(b"one".to_vec()).unwrap(), ProviderCredential::new(b"one".to_vec()).unwrap(),
            endpoint, recovery).unwrap();
        (root, broker, message)
    }

    #[test]
    fn bootstrap_refuses_broker_provider_secret_mismatch() {
        let root = Temp::new();
        let (mut endpoint, _recovery) = PublicationEndpoint::create_file_publication(&root.0, target(), b"old".to_vec(), 200, 8,
            super::super::FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
        endpoint.attach(scope()).unwrap();
        assert_eq!(CredentialBroker::new(inventory(), binding(), scope(),
            BrokerCredential::new(b"broker".to_vec()).unwrap(), ProviderCredential::new(b"provider".to_vec()).unwrap(),
            endpoint).unwrap_err(), Error::Binding);
    }

    #[test]
    fn ambiguous_family_credentials_refuse_concrete_broker_attachment() {
        let root = Temp::new();
        let (mut endpoint, _recovery) = PublicationEndpoint::create_file_publication(&root.0, target(), b"old".to_vec(), 200, 8,
            super::super::FilePublicationLimits { mutations: 128, bytes: 1_048_576 }).unwrap();
        endpoint.attach(scope()).unwrap();
        let ambiguous = String::from_utf8(inventory_json()).unwrap().replace(
            "{\"credential\":\"token\",\"holder\":\"broker\"}",
            "{\"credential\":\"token\",\"holder\":\"broker\"},{\"credential\":\"other\",\"holder\":\"broker\"}",
        );
        let inventory = LoadedPerimeterInventory::from_json_bytes(ambiguous.as_bytes()).unwrap();
        assert_eq!(CredentialBroker::new(inventory, binding(), scope(),
            BrokerCredential::new(b"one".to_vec()).unwrap(), ProviderCredential::new(b"one".to_vec()).unwrap(),
            endpoint).unwrap_err(), Error::Binding);
    }

    fn inventory_json() -> Vec<u8> {
        format!(r#"{{"version":1,"families":[{{"scope":{{"tenant":1,"principal":2,"purpose":1}},
        "family":"publication","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
        "credentials":[{{"credential":"token","holder":"broker"}}],"routes":[{{"route":"adapter:disposable-file","effect":"file_write",
        "profile":{{"id":"{}","generation":1}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress",
        "actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["reference"]}}],
        "residual_nonclaims":["reference"]}}]}}"#, DISPOSABLE_FILE_PROFILE).into_bytes()
    }

    #[test]
    fn rotation_is_idempotent_and_does_not_mint_or_change_dispatch_authority() {
        let (_root, mut broker, message) = fixture();
        let request = CredentialRotationRequest { operation: 1, expected_generation: 1, next_generation: 2 };
        let receipt = broker.broker_mut().rotate_credential(request,
            BrokerCredential::new(b"two".to_vec()).unwrap(), ProviderCredential::new(b"two".to_vec()).unwrap()).unwrap();
        assert_eq!(receipt, CredentialChangeReceipt { operation: 1, generation: 2, revoked: false });
        assert_eq!(broker.broker_mut().rotate_credential(request,
            BrokerCredential::new(b"two".to_vec()).unwrap(), ProviderCredential::new(b"two".to_vec()).unwrap()).unwrap(), receipt);
        assert_eq!(broker.broker().inspect().credential_generation, 2);
        assert_eq!(broker.broker_mut().deliver(&message).unwrap().outcome(), super::super::EndpointOutcome::Executed { resulting_version: 2 });
        assert_eq!(broker.broker().inspect().endpoint_executions, 1);
    }

    #[test]
    fn mismatched_rotation_is_atomic_and_cannot_replace_either_live_side() {
        let (_root, mut broker, message) = fixture();
        let request = CredentialRotationRequest { operation: 1, expected_generation: 1, next_generation: 2 };
        assert_eq!(broker.broker_mut().rotate_credential(request,
            BrokerCredential::new(b"two".to_vec()).unwrap(), ProviderCredential::new(b"wrong".to_vec()).unwrap()).unwrap_err(), Error::Binding);
        assert_eq!(broker.broker().inspect().credential_generation, 1);
        assert_eq!(broker.broker_mut().deliver(&message).unwrap().outcome(), super::super::EndpointOutcome::Executed { resulting_version: 2 });
    }

    #[test]
    fn conflicting_or_stale_rotation_cannot_replace_the_live_secret() {
        let (_root, mut broker, _message) = fixture();
        let request = CredentialRotationRequest { operation: 1, expected_generation: 1, next_generation: 2 };
        broker.broker_mut().rotate_credential(request,
            BrokerCredential::new(b"two".to_vec()).unwrap(), ProviderCredential::new(b"two".to_vec()).unwrap()).unwrap();
        assert_eq!(broker.broker_mut().rotate_credential(request,
            BrokerCredential::new(b"different".to_vec()).unwrap(), ProviderCredential::new(b"different".to_vec()).unwrap()).unwrap_err(), Error::Binding);
        assert_eq!(broker.broker_mut().rotate_credential(CredentialRotationRequest { operation: 2, expected_generation: 1, next_generation: 2 },
            BrokerCredential::new(b"three".to_vec()).unwrap(), ProviderCredential::new(b"three".to_vec()).unwrap()).unwrap_err(), Error::Stale);
        assert_eq!(broker.broker().inspect().credential_generation, 2);
    }

    #[test]
    fn revocation_blocks_delayed_execution_but_keeps_nonexecution_reconciliation() {
        let (_root, mut broker, message) = fixture();
        let receipt = broker.broker_mut().revoke_credential(CredentialRevocationRequest { operation: 9, expected_generation: 1 }).unwrap();
        assert!(receipt.revoked); assert_eq!(broker.broker_mut().deliver(&message).unwrap_err(), Error::WrongState);
        let query = StatusQuery(message.clone());
        assert_eq!(broker.broker().status(&query).unwrap(), EndpointStatus::AwaitingResolution);
        assert!(matches!(broker.broker_mut().seal_unexecuted(&query).unwrap().outcome(),
            super::super::EndpointOutcome::NotExecuted { reason: super::super::NonExecutionReason::Sealed }));
        assert_eq!(broker.broker().inspect().credential_exercises, 0);
    }

    #[test]
    fn rotation_and_terminal_revocation_survive_offline_reopen() {
        let (_root, mut broker, message) = fixture();
        broker.broker_mut().rotate_credential(CredentialRotationRequest { operation: 1, expected_generation: 1, next_generation: 2 },
            BrokerCredential::new(b"two".to_vec()).unwrap(), ProviderCredential::new(b"two".to_vec()).unwrap()).unwrap();
        let mut broker = broker.into_offline().reopen().unwrap(); broker.broker_mut().observe_time(ElapsedTick(2)).unwrap();
        assert_eq!(broker.broker().inspect().credential_generation, 2);
        broker.broker_mut().revoke_credential(CredentialRevocationRequest { operation: 2, expected_generation: 2 }).unwrap();
        let mut broker = broker.into_offline().reopen().unwrap(); broker.broker_mut().observe_time(ElapsedTick(3)).unwrap();
        assert!(broker.broker().inspect().credential_revoked);
        assert_eq!(broker.broker_mut().deliver(&message).unwrap_err(), Error::WrongState);
        assert_eq!(broker.broker().credential_change(1).unwrap().generation, 2);
        assert!(broker.broker().credential_change(2).unwrap().revoked);
    }
}