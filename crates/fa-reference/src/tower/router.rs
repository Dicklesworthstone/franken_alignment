//! One host-provisioned entry point for snapshot and original publication reads.
//!
//! Routes are fixed at construction. Native addresses never fall back to an
//! imported snapshot, including after the original owner closes. The response
//! keeps its backend's exact epistemic type, basis, accounting and encoder.

use super::publication::{PublicationResponse, PublicationTower, valid_request_identity};
use super::{
    ReasonCode, ReferenceTower, ResponseStatus, TowerAccess, TowerEncodingError, TowerInstallError,
    TowerRequest, TowerResponse, decode_request,
};
use crate::address_resolver::{Address, Noun};
use crate::event_journal::JournalScope;
use std::collections::{BTreeMap, btree_map::Entry};

/// Fixed bound on original live owners registered with one tenant's router.
/// Each owner retains its own native query, request and operation allowances.
pub const MAX_PUBLICATION_MOUNTS: usize = 64;

/// The original backend response, without a synthetic common evidence basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutedResponse {
    Reference(TowerResponse),
    Publication(PublicationResponse),
}

impl RoutedResponse {
    /// Both native encoders already expose the same sixteen envelope fields.
    /// Forwarding preserves their distinct result types and output bounds.
    pub fn to_json(&self) -> Result<String, TowerEncodingError> {
        match self {
            Self::Reference(response) => response.to_json(),
            Self::Publication(response) => response.to_json(),
        }
    }

    pub fn status(&self) -> ResponseStatus {
        match self {
            Self::Reference(response) => response.envelope.status,
            Self::Publication(response) => response.envelope.status,
        }
    }

    pub fn reason_code(&self) -> Option<ReasonCode> {
        match self {
            Self::Reference(response) => response.envelope.reason_code,
            Self::Publication(response) => response.envelope.reason_code,
        }
    }
}

/// One immutable imported snapshot and bounded original live actor sessions.
/// The host selects every owner available to this caller and configures its
/// independent disclosure/control grants before moving the facades here. Tenant
/// access is not per-run authentication; hosts must not mount owners outside
/// the authenticated caller's audience. This is not a supervisor or a permit.
///
/// ```compile_fail,E0599
/// use fa_reference::tower::router::TowerRouter;
/// fn no_supervisor(router: TowerRouter) { let _ = router.host_mut(); }
/// ```
pub struct TowerRouter {
    reference: ReferenceTower,
    publications: BTreeMap<Address, PublicationTower>,
}

impl TowerRouter {
    /// A snapshot or isolated experiment route with no production actor port.
    pub fn new(reference: ReferenceTower) -> Self {
        Self {
            reference,
            publications: BTreeMap::new(),
        }
    }

    /// Combine an imported production-domain snapshot with a native actor port.
    /// A branch-scoped journal cannot acquire a live publication route. Snapshot
    /// and mount generations are independent and are never compared or coerced.
    pub fn with_publication(
        reference: ReferenceTower,
        publication: PublicationTower,
    ) -> Result<Self, TowerInstallError> {
        Self::with_publications(reference, vec![publication])
    }

    /// Register original owners once, without joining their clocks or ledgers.
    /// Run addresses omit principal, branch and authority, so even owners with
    /// distinct request namespaces cannot share a tenant/run mount. Reject that
    /// ambiguity regardless of generation, input order or native owner liveness.
    /// Construction performs no native reads, writes or allowance consumption.
    pub fn with_publications(
        reference: ReferenceTower,
        publications: Vec<PublicationTower>,
    ) -> Result<Self, TowerInstallError> {
        if publications.len() > MAX_PUBLICATION_MOUNTS {
            return Err(TowerInstallError::LimitExceeded);
        }
        if !publications.is_empty()
            && matches!(
                reference.snapshot.journal.scope(),
                JournalScope::Branch { .. }
            )
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let mut routes = BTreeMap::new();
        for publication in publications {
            let mount = publication.mount_address();
            if reference.domain_address().tenant != mount.tenant {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            let domain = mount.without_generation();
            match routes.entry(domain) {
                Entry::Vacant(entry) => {
                    entry.insert(publication);
                }
                Entry::Occupied(_) => return Err(TowerInstallError::DuplicateObject),
            }
        }
        Ok(Self {
            reference,
            publications: routes,
        })
    }

    pub fn snapshot_address(&self) -> &Address {
        self.reference.domain_address()
    }

    /// The unambiguous single-owner registration. Multiple-owner callers must
    /// use publication_addresses rather than implicitly selecting a first run.
    pub fn publication_address(&self) -> Option<&Address> {
        if self.publications.len() != 1 {
            return None;
        }
        self.publications
            .values()
            .next()
            .map(PublicationTower::mount_address)
    }

    /// Original registrations in canonical address order. This is host setup
    /// metadata, not a remotely disclosed situation or a liveness observation.
    pub fn publication_addresses(&self) -> impl ExactSizeIterator<Item = &Address> {
        self.publications
            .values()
            .map(PublicationTower::mount_address)
    }

    /// Naming a native request neither acquires a ticket nor reads its outcome.
    pub fn publication_request_address(&self, request: u64) -> Result<Address, TowerInstallError> {
        if self.publications.len() > 1 {
            return Err(TowerInstallError::InvalidAddress);
        }
        self.publications
            .values()
            .next()
            .ok_or(TowerInstallError::MissingSource)?
            .request_address(request)
    }

    /// Name a ticket under one exact, explicitly selected original registration.
    /// A stale or generationless mount cannot silently name another generation's
    /// request; naming does not acquire a ticket or read a native owner.
    pub fn publication_request_address_at(
        &self,
        registration: &Address,
        request: u64,
    ) -> Result<Address, TowerInstallError> {
        let publication = self
            .publications
            .get(&registration.without_generation())
            .ok_or(TowerInstallError::MissingSource)?;
        if publication.mount_address() != registration {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        publication.request_address(request)
    }

    /// Accept one complete bounded request using the host's existing grants.
    /// Malformed input is normalized before route lookup, so even a partially
    /// decoded native address cannot select an owner or spend its allowance.
    pub fn exchange(&mut self, access: &TowerAccess, document: &[u8]) -> RoutedResponse {
        match decode_request(document) {
            Ok(request) => self.dispatch(access, request),
            Err(error) => RoutedResponse::Reference(self.reference.malformed_request(error)),
        }
    }

    pub fn dispatch(&mut self, access: &TowerAccess, request: TowerRequest) -> RoutedResponse {
        // Invalid and unauthorized callers receive route-independent responses.
        // No route lookup, read-unit charge, native query or write precedes them.
        if !valid_request_identity(&request) {
            let normalized = TowerRequest::new(
                "<invalid-request>",
                Address::new("invalid", Noun::Run, "request", None).expect("constant address"),
                request.verb,
            );
            return RoutedResponse::Reference(self.reference.refusal(
                &normalized,
                ReasonCode::IntegrityFailure,
                false,
            ));
        }
        if access.scope.caller_tenant != self.reference.domain_address().tenant
            || access.scope.is_authorized_for(&request.address).is_err()
        {
            return RoutedResponse::Reference(self.reference.withheld(&request));
        }

        if let Some(publication) = self
            .publications
            .get_mut(&request.address.without_generation())
        {
            // Run registrations are unique without generation. Stale and
            // unsupported requests still reach only their original facade.
            return RoutedResponse::Publication(publication.dispatch(access, request));
        }
        if request.address.kind == Noun::Attempt {
            for publication in self.publications.values_mut() {
                if publication.owns_request_namespace(&request.address) {
                    // Delegate once, including unsupported verbs, stale generations,
                    // unknown outcomes and lost owners. Never retry another backend.
                    return RoutedResponse::Publication(publication.dispatch(access, request));
                }
            }
        }

        if request.address.kind == Noun::Attempt && request.address.id.starts_with("request-") {
            // A foreign, malformed or unmounted native request projection must
            // not impersonate an ordinary imported ledger attempt.
            return RoutedResponse::Reference(self.reference.refusal(
                &request,
                ReasonCode::MissingRequiredObservation,
                false,
            ));
        }
        RoutedResponse::Reference(self.reference.dispatch(access, request))
    }
}
