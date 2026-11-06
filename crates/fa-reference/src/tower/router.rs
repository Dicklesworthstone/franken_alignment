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

/// One immutable imported snapshot and at most one original live actor session.
/// The host supplies caller grants and configures any precheck disclosure before
/// moving the publication facade here. This is not a supervisor or a permit.
///
/// ```compile_fail,E0599
/// use fa_reference::tower::router::TowerRouter;
/// fn no_supervisor(router: TowerRouter) { let _ = router.host_mut(); }
/// ```
pub struct TowerRouter {
    reference: ReferenceTower,
    publication: Option<PublicationTower>,
}

impl TowerRouter {
    /// A snapshot or isolated experiment route with no production actor port.
    pub fn new(reference: ReferenceTower) -> Self {
        Self {
            reference,
            publication: None,
        }
    }

    /// Combine an imported production-domain snapshot with a native actor port.
    /// A branch-scoped journal cannot acquire a live publication route. Snapshot
    /// and mount generations are independent and are never compared or coerced.
    pub fn with_publication(
        reference: ReferenceTower,
        publication: PublicationTower,
    ) -> Result<Self, TowerInstallError> {
        if reference.domain_address().tenant != publication.mount_address().tenant
            || matches!(
                reference.snapshot.journal.scope(),
                JournalScope::Branch { .. }
            )
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        Ok(Self {
            reference,
            publication: Some(publication),
        })
    }

    pub fn snapshot_address(&self) -> &Address {
        self.reference.domain_address()
    }

    pub fn publication_address(&self) -> Option<&Address> {
        self.publication
            .as_ref()
            .map(PublicationTower::mount_address)
    }

    /// Naming a native request neither acquires a ticket nor reads its outcome.
    pub fn publication_request_address(&self, request: u64) -> Result<Address, TowerInstallError> {
        self.publication
            .as_ref()
            .ok_or(TowerInstallError::MissingSource)?
            .request_address(request)
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

        if let Some(publication) = &mut self.publication {
            let mount = publication.mount_address();
            let mounted_run = request.address.kind == mount.kind
                && request.address.tenant == mount.tenant
                && request.address.id == mount.id;
            if mounted_run || publication.owns_request_namespace(&request.address) {
                // Delegate once, including unsupported verbs, stale generations,
                // unknown outcomes and lost owners. Never retry another backend.
                return RoutedResponse::Publication(publication.dispatch(access, request));
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
