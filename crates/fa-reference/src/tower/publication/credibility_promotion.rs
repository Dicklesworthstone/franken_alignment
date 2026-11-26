//! Promote helper weights only through the original independently granted owner.
//! Plans expose complete native qualification without accepting caller labels or weights.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileCredibilityPromotionBasis, FileCredibilityPromotionError,
    FileCredibilityPromotionPlanResult, FileCredibilityPromotionRequest,
    FileCredibilityPromotionResult, FileSupervisorCredibilityPromotionPort,
};
use crate::action::{ElapsedTick, MAX_ATTEMPTS};
use crate::address_resolver::Address;
use crate::tower::request::credibility_promotion::encode_basis;
use crate::typestate_cx::PurposeTag;

/// Historical plan sealed to the original registration and exact query.
/// The executable action still needs the original independently granted permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedCredibilityPromotionNext {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    operation: u64,
    native: FileCredibilityPromotionPlanResult,
}

impl LinkedCredibilityPromotionNext {
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
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn native(&self) -> &FileCredibilityPromotionPlanResult {
        &self.native
    }

    pub fn action(&self) -> Result<Option<TowerRequest>, TowerInstallError> {
        let Ok(ActorKnowledge::Known { value, basis }) = &self.native else {
            return Ok(None);
        };
        let Some(native) = value.request() else {
            return Ok(None);
        };
        if value.basis() != *basis
            || native.expected() != *basis
            || value.operation() != self.operation
            || native.operation() != self.operation
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let mut request = TowerRequest::new(
            format!("promote-credibility-{}", self.operation),
            self.registration.clone(),
            Verb::Promote,
        );
        request.arguments = ReadArguments::PromoteCredibility {
            operation: self.operation,
            expected: *basis,
        };
        request.idempotency_key = Some(self.operation.to_string());
        Ok(Some(request))
    }
}

/// One acknowledged native promotion or refusal tied to the exact attempted cut.
/// A receipt is historical evidence, never an effect permit or governor role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedCredibilityPromotion {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    attempted: FileCredibilityPromotionRequest,
    native: FileCredibilityPromotionResult,
}

impl LinkedCredibilityPromotion {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }
    pub fn attempted(&self) -> &FileCredibilityPromotionRequest {
        &self.attempted
    }
    pub fn native(&self) -> &FileCredibilityPromotionResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install a weak port to one independently granted credibility operation.
    /// Even equal numeric scopes must share the original owner. A rejected
    /// replacement withdraws the previous port without promoting anything.
    pub fn set_credibility_promotion(
        &mut self,
        port: Option<FileSupervisorCredibilityPromotionPort>,
    ) -> Result<(), TowerInstallError> {
        self.credibility_promotion = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.credibility_promotion = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_credibility_promotion(
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
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, false);
        }
        match &request.arguments {
            ReadArguments::CredibilityPromotionNext { operation } if request.verb == Verb::Next => {
                if *operation == 0
                    || request
                        .idempotency_key
                        .as_ref()
                        .is_some_and(|key| key != &operation.to_string())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Some(port) = &self.credibility_promotion else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
                };
                let native = port.next(*operation);
                let reason = promotion_reason(&native);
                let linked = LinkedCredibilityPromotionNext {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    query_idempotency_key: request.idempotency_key.clone(),
                    operation: *operation,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::CredibilityPromotionNext(linked),
                    reason,
                    true,
                ))
            }
            ReadArguments::PromoteCredibility {
                operation,
                expected,
            } if request.verb == Verb::Promote => {
                if request.address != self.mount
                    || expected.scope != self.scope
                    || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Ok(attempted) = FileCredibilityPromotionRequest::new(*operation, *expected)
                else {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                };
                let Some(port) = &self.credibility_promotion else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
                };
                // Reserve enough output capacity for every bounded native
                // outcome before changing any journal, weights or holds.
                if !self.credibility_promotion_response_fits(&request, attempted) {
                    return self.refusal(&request, ReasonCode::BudgetExhausted, false);
                }
                let native = port.promote(attempted);
                let reason = promotion_reason(&native);
                let linked = LinkedCredibilityPromotion {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    attempted,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::CredibilityPromotion(linked),
                    reason,
                    false,
                ))
            }
            _ => self.refusal(&request, ReasonCode::IntegrityFailure, false),
        }
    }

    fn credibility_promotion_response_fits(
        &self,
        request: &TowerRequest,
        attempted: FileCredibilityPromotionRequest,
    ) -> bool {
        // This inert result encodes the actual registration, request labels,
        // predecessor and longest native-error envelope. It never calls a port.
        let probe = LinkedCredibilityPromotion {
            registration: self.mount.clone(),
            query_address: request.address.clone(),
            query_operation_id: request.operation_id.clone(),
            attempted,
            native: Err(FileCredibilityPromotionError::Unavailable),
        };
        let response = self.response(
            request,
            PublicationResult::CredibilityPromotion(probe),
            Some(ReasonCode::LowerLayerUnverified),
            false,
        );
        let Ok(encoded) = response.to_json() else {
            return false;
        };
        let upper_basis = FileCredibilityPromotionBasis {
            scope: attempted.expected().scope,
            journal_revision: u64::MAX,
            control_sequence: u64::MAX,
            authority_epoch: u64::MAX,
            evaluation_revision: u64::MAX,
            congress_generation: u64::MAX,
            policy_generation: u64::MAX,
            recorded_at: Some(ElapsedTick(u64::MAX)),
        };
        let basis = encode_basis(upper_basis);
        let cancelled = vec!["\"18446744073709551615\""; MAX_ATTEMPTS].join(",");
        // The complete maximal receipt is added without subtracting the inert
        // error. This also covers the longer error codes and knowledge key.
        // Successful envelopes are smaller: no missing/retry-address arrays.
        let receipt = format!(
            r#","knowledge":{{"state":"known","basis":{basis},"value":{{"before":{basis},"cancelled":[{cancelled}],"refunded_units":"18446744073709551615"}}}}"#
        );
        encoded
            .len()
            .checked_add(receipt.len())
            .is_some_and(|bytes| bytes <= self.limits.max_response_bytes)
    }
}

fn promotion_reason<T>(
    result: &Result<
        ActorKnowledge<T, FileCredibilityPromotionBasis>,
        FileCredibilityPromotionError,
    >,
) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_credibility_promotion",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(FileCredibilityPromotionError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(FileCredibilityPromotionError::Unsupported) => Some(ReasonCode::CapabilityUnsupported),
        Err(FileCredibilityPromotionError::StalePredecessor) => Some(ReasonCode::StaleGeneration),
        Err(FileCredibilityPromotionError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
