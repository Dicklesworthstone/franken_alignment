//! Request-scoped discovery from the original acknowledged publication history.
//!
//! Discovery names completed review observations; their independent Explain
//! grant still owns replay, disclosure and numerical work. Nothing here
//! acquires a ticket, obtains source evidence or makes an effect eligible.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError, TowerRequest,
    Verb, request_address_for,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileRequestHistoryResult, FileSupervisorRequestHistoryPort,
};
use crate::action::consequence::oversight::actor::ActorError;
use crate::address_resolver::Address;

/// A historical relationship observation sealed to its exact query and original
/// registration. Public envelope fields cannot relabel either data or refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedRequestHistory {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    request: u64,
    native: FileRequestHistoryResult,
}

impl LinkedRequestHistory {
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

    pub fn request(&self) -> u64 {
        self.request
    }

    pub fn native(&self) -> &FileRequestHistoryResult {
        &self.native
    }

    /// Explicit, generation-pinned read requests for exactly the observed
    /// completions. Unknown, withheld and failed discovery cannot become an
    /// empty, supposedly complete review set. Calling this method performs no
    /// native work; each Explain still requires its separately retained grant.
    pub fn explain_requests(&self) -> Result<Vec<TowerRequest>, TowerEncodingError> {
        let Ok(ActorKnowledge::Known { value, basis }) = &self.native else {
            return Err(TowerEncodingError::InvalidKnowledge);
        };
        let address = request_address_for(basis.scope, &self.registration, basis.request)
            .map_err(|_| TowerEncodingError::InvalidKnowledge)?;
        if value.basis() != *basis
            || basis.request != self.request
            || address.without_generation() != self.query_address.without_generation()
            || self
                .query_address
                .generation
                .is_some_and(|generation| Some(generation) != self.registration.generation)
        {
            return Err(TowerEncodingError::InvalidKnowledge);
        }
        Ok(value
            .reviews()
            .iter()
            .map(|review| {
                let mut request = TowerRequest::new(
                    format!("related-explain-{}", review.round()),
                    address.clone(),
                    Verb::Explain,
                );
                request.arguments = ReadArguments::ReviewExplanation {
                    round: review.round(),
                };
                request
            })
            .collect())
    }
}

impl PublicationTower {
    /// Install only the separately granted metadata disclosure of this exact
    /// original owner. A failed replacement first withdraws the old port.
    /// Other port clones retain their original shared, finite query allowance.
    pub fn set_request_history(
        &mut self,
        port: Option<FileSupervisorRequestHistoryPort>,
    ) -> Result<(), TowerInstallError> {
        self.request_history = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.request_history = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_request_history(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        // Common syntax, tenant and registration checks have already run.
        // Actor access cannot discover grants or whether a request exists.
        if !access.operator {
            return self.withheld(&request);
        }
        let Some(key) = self.request_key(&request.address) else {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        };
        if request.arguments != ReadArguments::None {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let Some(port) = &self.request_history else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = port.history(key);
        let reason = match &native {
            Ok(ActorKnowledge::Known { .. }) => None,
            Ok(ActorKnowledge::Unknown {
                reason: ActorUnknownReason::ControllerUnavailable,
            })
            | Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
            Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_request_history",
            }) => Some(ReasonCode::DeniedByPolicy),
            Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
            Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
        };
        let linked = LinkedRequestHistory {
            registration: self.mount.clone(),
            query_address: request.address.clone(),
            query_operation_id: request.operation_id.clone(),
            query_idempotency_key: request.idempotency_key.clone(),
            request: key,
            native,
        };
        // Complete output or explicit refusal: never drop a recorded review,
        // run replay to fill a gap, repeat discovery or refund spent work.
        self.bounded_response(self.response(
            &request,
            PublicationResult::RequestHistory(linked),
            reason,
            true,
        ))
    }
}
