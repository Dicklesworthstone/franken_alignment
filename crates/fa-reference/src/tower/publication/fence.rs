//! Single-domain admission fencing through a separately retained native grant.
//! This adapter cannot publish, drain an endpoint, or halt an external process.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileFenceError, FileFenceRequest, FileFenceResult, FileSupervisorFencePort,
};
use crate::address_resolver::Address;
use crate::typestate_cx::PurposeTag;

/// Sealed association between one exact Tower registration/request and the
/// original native fence result. A public envelope cannot relabel its subject,
/// operation key or predecessor. The grant, not this value, retains retries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedFence {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    attempted: FileFenceRequest,
    native: FileFenceResult,
}

impl LinkedFence {
    pub fn registration(&self) -> &Address { &self.registration }
    pub fn query_address(&self) -> &Address { &self.query_address }
    pub fn query_operation_id(&self) -> &str { &self.query_operation_id }
    pub fn attempted(&self) -> FileFenceRequest { self.attempted }
    pub fn native(&self) -> &FileFenceResult { &self.native }
}

impl PublicationTower {
    /// Install only a fence port issued by this exact original owner. A rejected
    /// replacement withdraws the previous installation. Equal numeric scope,
    /// operator status and Permit purpose cannot manufacture this grant.
    pub fn set_fence(
        &mut self,
        port: Option<FileSupervisorFencePort>,
    ) -> Result<(), TowerInstallError> {
        self.fence = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.fence = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_fence(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        if !access.operator {
            return self.withheld(&request);
        }
        if !access.allowed_purposes.contains(&PurposeTag::Permit) {
            return self.refusal(&request, ReasonCode::PurposeMismatch, false);
        }
        // The native request namespace and imported objects cannot widen this
        // owner-wide mutation to a different principal, run or authority.
        if request.address != self.mount {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, false);
        }
        let ReadArguments::Fence { operation, expected } = &request.arguments else {
            return self.refusal(&request, ReasonCode::IntegrityFailure, false);
        };
        if expected.scope != self.scope
            || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
        {
            return self.refusal(&request, ReasonCode::IntegrityFailure, false);
        }
        let Ok(attempted) = FileFenceRequest::new(*operation, *expected) else {
            return self.refusal(&request, ReasonCode::IntegrityFailure, false);
        };
        let Some(port) = &self.fence else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
        };
        let native = port.fence(attempted);
        let reason = match &native {
            Ok(ActorKnowledge::Known { .. }) => None,
            Ok(ActorKnowledge::Unknown { reason: ActorUnknownReason::ControllerUnavailable }) => {
                Some(ReasonCode::LowerLayerUnverified)
            }
            Ok(ActorKnowledge::Withheld { authority_required: "supervisor_fence" }) => {
                Some(ReasonCode::DeniedByPolicy)
            }
            Err(FileFenceError::Capacity) => Some(ReasonCode::BudgetExhausted),
            Err(FileFenceError::Unsupported) => Some(ReasonCode::CapabilityUnsupported),
            Err(FileFenceError::StalePredecessor) => Some(ReasonCode::StaleGeneration),
            Err(FileFenceError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
            Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
        };
        let linked = LinkedFence {
            registration: self.mount.clone(),
            query_address: request.address.clone(),
            query_operation_id: request.operation_id.clone(),
            attempted,
            native,
        };
        // No automatic retry after transport/owner loss. Exact same-grant
        // retries retain the original native result without another mutation.
        self.bounded_response(self.response(
            &request, PublicationResult::Fence(linked), reason, false,
        ))
    }
}
