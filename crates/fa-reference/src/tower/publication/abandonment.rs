//! Explicit, request-specific permanent Unknown disposition under separate custody.
//! This consumes the original durable abandonment transition. It neither
//! reconciles an endpoint nor releases any of the original uncertain charge.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileAbandonmentError, FileAbandonmentPlanResult, FileAbandonmentRequest,
    FileAbandonmentResult, FileSupervisorAbandonmentPort,
};
use crate::address_resolver::Address;
use crate::typestate_cx::PurposeTag;

/// One native plan bound to its original caller correlation, registration and
/// request. The suggested command is data; it retains no owner, key or permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedAbandonmentNext {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    request: u64,
    operation: u64,
    native: FileAbandonmentPlanResult,
}

impl LinkedAbandonmentNext {
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
    pub fn operation(&self) -> u64 {
        self.operation
    }
    pub fn native(&self) -> &FileAbandonmentPlanResult {
        &self.native
    }

    /// Return the exact structured request proposed by the original native
    /// plan. Availability is rechecked when this request is actually executed.
    pub fn action(&self) -> Result<Option<TowerRequest>, TowerInstallError> {
        let Ok(ActorKnowledge::Known { value, basis }) = &self.native else {
            return Ok(None);
        };
        let Some(native) = value.request() else {
            return Ok(None);
        };
        if native.operation() != self.operation
            || native.expected() != *basis
            || value.basis() != *basis
            || basis.request != self.request
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        let address = super::request_address_for(basis.scope, &self.registration, basis.request)?;
        let mut request = TowerRequest::new(
            format!("abandon-unknown-{}", self.operation),
            address,
            Verb::Resolve,
        );
        request.arguments = ReadArguments::AbandonUnknown {
            operation: self.operation,
            expected: *basis,
        };
        request.idempotency_key = Some(self.operation.to_string());
        Ok(Some(request))
    }
}

/// An acknowledged permanent Unknown disposition or refusal bound to the exact attempted
/// operation. Its native retry cache exists only while the original grant lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedAbandonment {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    request: u64,
    attempted: FileAbandonmentRequest,
    native: FileAbandonmentResult,
}

impl LinkedAbandonment {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }
    pub fn request(&self) -> u64 {
        self.request
    }
    pub fn attempted(&self) -> FileAbandonmentRequest {
        self.attempted
    }
    pub fn native(&self) -> &FileAbandonmentResult {
        &self.native
    }
}

impl PublicationTower {
    /// Install a finite permanent Unknown disposition capability from this exact owner.
    /// A rejected replacement withdraws the prior installation. Possessing an
    /// observation port, equal numeric scope or Permit tag cannot supply it.
    pub fn set_abandonment(
        &mut self,
        port: Option<FileSupervisorAbandonmentPort>,
    ) -> Result<(), TowerInstallError> {
        self.abandonment = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.abandonment = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_abandonment(
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
        let Some(key) = self.request_key(&request.address) else {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, false);
        };
        match &request.arguments {
            ReadArguments::AbandonmentNext { operation } if request.verb == Verb::Next => {
                if *operation == 0
                    || request
                        .idempotency_key
                        .as_ref()
                        .is_some_and(|value| value != &operation.to_string())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Some(port) = &self.abandonment else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
                };
                let native = port.next(key, *operation);
                let reason = abandonment_reason(&native);
                let linked = LinkedAbandonmentNext {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    query_idempotency_key: request.idempotency_key.clone(),
                    request: key,
                    operation: *operation,
                    native,
                };
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::AbandonmentNext(linked),
                    reason,
                    true,
                ))
            }
            ReadArguments::AbandonUnknown {
                operation,
                expected,
            } if request.verb == Verb::Resolve => {
                if expected.scope != self.scope
                    || expected.request != key
                    || request.idempotency_key.as_deref() != Some(operation.to_string().as_str())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                let Ok(attempted) = FileAbandonmentRequest::new(*operation, *expected) else {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                };
                let Some(port) = &self.abandonment else {
                    return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
                };
                let native = port.abandon(attempted);
                let reason = abandonment_reason(&native);
                let linked = LinkedAbandonment {
                    registration: self.mount.clone(),
                    query_address: request.address.clone(),
                    query_operation_id: request.operation_id.clone(),
                    request: key,
                    attempted,
                    native,
                };
                // Retry identity is exact and scoped to the retained native grant.
                // No automatic retry is advised across transport or owner loss.
                self.bounded_response(self.response(
                    &request,
                    PublicationResult::Abandonment(linked),
                    reason,
                    false,
                ))
            }
            _ => self.refusal(&request, ReasonCode::IntegrityFailure, false),
        }
    }
}

fn abandonment_reason<T>(
    result: &Result<ActorKnowledge<T, crate::action::consequence::delivery::persistent::requests::actor::FileHoldDiagnosisBasis>, FileAbandonmentError>,
) -> Option<ReasonCode> {
    match result {
        Ok(ActorKnowledge::Known { .. }) => None,
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Withheld {
            authority_required: "supervisor_abandonment",
        }) => Some(ReasonCode::DeniedByPolicy),
        Err(FileAbandonmentError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(FileAbandonmentError::Unsupported) => Some(ReasonCode::CapabilityUnsupported),
        Err(FileAbandonmentError::StalePredecessor) => Some(ReasonCode::StaleGeneration),
        Err(FileAbandonmentError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
    }
}
