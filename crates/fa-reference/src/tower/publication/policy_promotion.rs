//! Promote only the campaign approved by a separately retained native governor.
//! Neither candidate comparison nor this facade can approve another policy.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FilePolicyPromotionBasis, FilePolicyPromotionError, FilePolicyPromotionPlanResult,
    FilePolicyPromotionRequest, FilePolicyPromotionResult, FileSupervisorPolicyPromotionPort,
};
use crate::address_resolver::Address;
use crate::typestate_cx::PurposeTag;

/// Historical plan sealed to the original registration, campaign and query.
/// The executable action still needs the original independently granted permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedPolicyPromotionNext {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    campaign: u64,
    operation: u64,
    native: FilePolicyPromotionPlanResult,
}

impl LinkedPolicyPromotionNext {
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
    pub fn campaign(&self) -> u64 {
        self.campaign
    }
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn native(&self) -> &FilePolicyPromotionPlanResult {
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
            || basis.campaign != self.campaign
            || value.operation() != self.operation
            || native.operation() != self.operation
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let mut request = TowerRequest::new(
            format!("promote-policy-{}", self.operation),
            self.registration.clone(),
            Verb::Promote,
        );
        request.arguments = ReadArguments::Promote {
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
pub struct LinkedPolicyPromotion {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    attempted: FilePolicyPromotionRequest,
    native: FilePolicyPromotionResult,
}

impl LinkedPolicyPromotion {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }
    pub fn attempted(&self) -> &FilePolicyPromotionRequest {
        &self.attempted
    }
    pub fn native(&self) -> &FilePolicyPromotionResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install a weak port to one approved original campaign. Ownership is
    /// checked even for equal numeric scopes. Rejected replacement withdraws
    /// the previous port; neither installation approves or promotes anything.
    pub fn set_policy_promotion(
        &mut self,
        port: Option<FileSupervisorPolicyPromotionPort>,
    ) -> Result<(), TowerInstallError> {
        self.policy_promotion = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.policy_promotion = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_policy_promotion(
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
            ReadArguments::PolicyPromotionNext {
                campaign,
                operation,
            } if request.verb == Verb::Next => {
                if *campaign == 0
                    || *operation == 0
                    || request
                        .idempotency_key
                        .as_ref()
                        .is_some_and(|key| key != &operation.to_string())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Some(port) = &self.policy_promotion else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
                };
                let native = port.next(*campaign, *operation);
                let reason = promotion_reason(&native);
                let linked = LinkedPolicyPromotionNext {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    query_idempotency_key: request.idempotency_key.clone(),
                    campaign: *campaign,
                    operation: *operation,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::PolicyPromotionNext(linked),
                    reason,
                    true,
                ))
            }
            ReadArguments::Promote {
                operation,
                expected,
            } if request.verb == Verb::Promote => {
                if request.address != self.mount
                    || expected.scope != self.scope
                    || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Ok(attempted) = FilePolicyPromotionRequest::new(*operation, *expected) else {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                };
                let Some(port) = &self.policy_promotion else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
                };
                let native = port.promote(attempted.clone());
                let reason = promotion_reason(&native);
                let linked = LinkedPolicyPromotion {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    attempted,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::PolicyPromotion(linked),
                    reason,
                    false,
                ))
            }
            _ => self.refusal(&request, ReasonCode::IntegrityFailure, false),
        }
    }
}

fn promotion_reason<T>(
    result: &Result<ActorKnowledge<T, FilePolicyPromotionBasis>, FilePolicyPromotionError>,
) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_policy_promotion",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(FilePolicyPromotionError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(FilePolicyPromotionError::Unsupported) => Some(ReasonCode::CapabilityUnsupported),
        Err(FilePolicyPromotionError::StalePredecessor) => Some(ReasonCode::StaleGeneration),
        Err(FilePolicyPromotionError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
