//! Operator-only observations of the original owner's credibility and Andon
//! ledgers. A separately retained native grant supplies finite read work;
//! neither the facade nor an actor request supplies evaluation labels.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileGovernanceLedgerResult, FileSupervisorGovernanceLedgerPort,
};
use crate::action::consequence::oversight::actor::ActorError;
use crate::address_resolver::Address;

/// A summary from one acknowledged native owner, sealed to the original
/// registration and exact query. Public envelope fields cannot relabel it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedGovernanceLedger {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    native: FileGovernanceLedgerResult,
}

impl LinkedGovernanceLedger {
    pub fn registration(&self) -> &Address {
        &self.registration
    }

    pub fn query_address(&self) -> &Address {
        &self.query_address
    }

    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }

    pub fn query_idempotency_key(&self) -> Option<&str> {
        self.query_idempotency_key.as_deref()
    }

    pub fn native(&self) -> &FileGovernanceLedgerResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install only the explicitly granted ledger observation of this exact
    /// original owner. A failed replacement withdraws the old port first;
    /// rebuilding a facade never replenishes the native shared read allowance.
    pub fn set_governance_ledger(
        &mut self,
        port: Option<FileSupervisorGovernanceLedgerPort>,
    ) -> Result<(), TowerInstallError> {
        self.governance_ledger = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.governance_ledger = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_governance_ledger(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        // Common syntax, tenant and registration checks have already run.
        // An actor cannot use an address to discover an installed ledger.
        if !access.operator {
            return self.withheld(&request);
        }
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        }
        if request.arguments != ReadArguments::None {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.governance_ledger else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = port.ledger();
        let reason = match &native {
            Ok(ActorKnowledge::Known { .. }) => None,
            Ok(ActorKnowledge::Unknown {
                reason: ActorUnknownReason::ControllerUnavailable,
            })
            | Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
            Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_governance_ledger",
            }) => Some(ReasonCode::DeniedByPolicy),
            Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
            Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
        };
        let linked = LinkedGovernanceLedger {
            registration: self.mount.clone(),
            query_address: request.address.clone(),
            query_operation_id: request.operation_id.clone(),
            query_idempotency_key: request.idempotency_key.clone(),
            native,
        };
        // The original observation costs one finite query even when its full
        // projection exceeds this response budget. Never truncate or retry it.
        self.bounded_response(self.response(
            &request,
            PublicationResult::GovernanceLedger(linked),
            reason,
            true,
        ))
    }
}
